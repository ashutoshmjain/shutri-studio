//! Clip plan: the editable episode artifact. Clips are lists of segments;
//! a segment is speech (timed transcript) or music (uploaded file).

use serde::{Deserialize, Serialize};

use crate::narrative::{sentences, Word};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Segment {
    Music {
        music_file: String,
        duration: f64,
        #[serde(default)]
        crossfade: f64,
        #[serde(default = "one")]
        volume: f64,
    },
    Audio {
        start: f64,
        end: f64,
        text: String,
        /// Segment duration as written by DDMA; derived from start/end when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration: Option<f64>,
    },
}

fn one() -> f64 {
    1.0
}

impl Segment {
    pub fn duration(&self) -> f64 {
        match self {
            Segment::Music { duration, .. } => *duration,
            Segment::Audio { start, end, duration, .. } => {
                duration.unwrap_or_else(|| (end - start).max(0.0))
            }
        }
    }
}

pub const INTRO_OFFSET: f64 = 1.5;
pub const OUTRO_DURATION: f64 = 5.0;
/// Max |rendered_duration - probed mp4 duration| before the build halts (strategy §2).
pub const RECONCILE_TOLERANCE_SECS: f64 = 2.0;

fn intro_offset() -> f64 { INTRO_OFFSET }
fn outro_duration() -> f64 { OUTRO_DURATION }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Clip {
    pub num: u32,
    pub title: String,
    #[serde(default)]
    pub bridge_text: Vec<String>,
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mosaic_run_id: Option<String>,
    /// Source coordinates into the episode master. Never overwrite with
    /// clip-relative values once set (strategy §2 coordinate model).
    #[serde(rename = "start", default, skip_serializing_if = "Option::is_none")]
    pub source_start: Option<f64>,
    #[serde(rename = "end", default, skip_serializing_if = "Option::is_none")]
    pub source_end: Option<f64>,
    /// Source-range duration (end - start), not the rendered duration.
    #[serde(rename = "duration", default, skip_serializing_if = "Option::is_none")]
    pub source_duration: Option<f64>,
    /// Measured duration of the rendered square clip; set only after render.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendered_duration: Option<f64>,
    /// Sum of segment durations (the audio timeline length).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_duration: Option<f64>,
    #[serde(default = "intro_offset")]
    pub intro_offset: f64,
    #[serde(default = "outro_duration")]
    pub outro_duration: f64,
}

impl Default for Clip {
    fn default() -> Self {
        Self {
            num: 0, title: String::new(), bridge_text: vec![], segments: vec![],
            locked: false, hidden: false, mosaic_run_id: None,
            source_start: None, source_end: None, source_duration: None,
            rendered_duration: None, audio_duration: None,
            intro_offset: INTRO_OFFSET, outro_duration: OUTRO_DURATION,
        }
    }
}

impl Clip {
    pub fn duration(&self) -> f64 {
        self.segments.iter().map(|s| s.duration()).sum()
    }
    pub fn start(&self) -> f64 {
        self.source_start.unwrap_or_else(|| {
            self.segments.iter().find_map(|s| match s {
                Segment::Audio { start, .. } => Some(*start),
                _ => None,
            }).unwrap_or(0.0)
        })
    }
    pub fn end(&self) -> f64 {
        self.source_end.unwrap_or_else(|| {
            self.segments.iter().rev().find_map(|s| match s {
                Segment::Audio { end, .. } => Some(*end),
                _ => None,
            }).unwrap_or(0.0)
        })
    }
    /// Fill distribution fields from segments without overwriting source
    /// coordinates that are already set.
    pub fn sync_distribution_fields(&mut self) {
        if self.source_start.is_none() {
            let v = self.segments.iter().find_map(|s| match s {
                Segment::Audio { start, .. } => Some(*start), _ => None,
            });
            self.source_start = v;
        }
        if self.source_end.is_none() {
            let v = self.segments.iter().rev().find_map(|s| match s {
                Segment::Audio { end, .. } => Some(*end), _ => None,
            });
            self.source_end = v;
        }
        if self.source_duration.is_none() {
            if let (Some(a), Some(b)) = (self.source_start, self.source_end) {
                self.source_duration = Some((b - a).max(0.0));
            }
        }
        self.audio_duration = Some(self.duration());
    }
    /// Strategy §2 reconciliation: halt when rendered vs probed disagree.
    pub fn reconcile(&self, mp4_duration: f64) -> crate::Result<()> {
        if let Some(r) = self.rendered_duration {
            if (r - mp4_duration).abs() > RECONCILE_TOLERANCE_SECS {
                return crate::msg(format!(
                    "clip {}: rendered_duration {r:.2}s vs mp4 {mp4_duration:.2}s exceeds {:.0}s tolerance",
                    self.num, RECONCILE_TOLERANCE_SECS
                ));
            }
        }
        Ok(())
    }
}

pub type Plan = Vec<Clip>;

/// Build a plan from timed words: group sentences into ~target-second clips,
/// snapping boundaries to sentence ends (hook-snapping contract).
pub fn plan_from_words(words: &[Word], target_secs: f64) -> Plan {
    let full: String = words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    let sents = sentences(&full);
    // Map each sentence to its word time span by sequential word consumption.
    let mut clips: Vec<Clip> = Vec::new();
    let mut wi = 0usize;
    let mut cur_text: Vec<String> = Vec::new();
    let mut cur_start: Option<f64> = None;
    let mut cur_end = 0.0f64;

    let flush = |clips: &mut Vec<Clip>, text: &mut Vec<String>, start: Option<f64>, end: f64| {
        if text.is_empty() {
            return;
        }
        let joined = text.join(" ");
        let title: String = joined.split_whitespace().take(4).collect::<Vec<_>>().join(" ");
        let s = start.unwrap_or(0.0);
        let mut clip = Clip {
            num: (clips.len() + 1) as u32,
            title,
            bridge_text: vec![],
            segments: vec![Segment::Audio { start: s, end, text: joined, duration: None }],
            ..Default::default()
        };
        clip.sync_distribution_fields();
        clips.push(clip);
        text.clear();
    };

    for sent in &sents {
        let count = sent.split_whitespace().count();
        let span: Vec<&Word> = words.iter().skip(wi).take(count).collect();
        wi += count;
        let (s, e) = match (span.first(), span.last()) {
            (Some(a), Some(b)) => (a.start, b.end),
            _ => (cur_end, cur_end),
        };
        if cur_start.is_none() {
            cur_start = Some(s);
        }
        cur_end = e;
        cur_text.push(sent.clone());
        if cur_end - cur_start.unwrap_or(s) >= target_secs {
            flush(&mut clips, &mut cur_text, cur_start, cur_end);
            cur_start = None;
        }
    }
    flush(&mut clips, &mut cur_text, cur_start, cur_end);
    clips
}

/// Insert a music segment at the start of a clip (typical sting placement).
pub fn insert_music(clip: &mut Clip, music_file: String, duration: f64, crossfade: f64) {
    clip.segments.insert(
        0,
        Segment::Music {
            music_file,
            duration,
            crossfade,
            volume: 1.0,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<Word> {
        text.split_whitespace()
            .enumerate()
            .map(|(i, t)| Word {
                text: t.into(),
                start: i as f64,
                end: i as f64 + 0.9,
            })
            .collect()
    }

    #[test]
    fn plans_on_sentence_boundaries() {
        let w = words("One two three four. Five six seven eight. Nine ten eleven twelve.");
        let plan = plan_from_words(&w, 5.0);
        assert!(plan.len() >= 2);
        for c in &plan {
            assert!(c.duration() > 0.0);
        }
    }

    #[test]
    fn music_segment_inserts() {
        let mut c = Clip { num: 1, title: "t".into(), ..Default::default() };
        insert_music(&mut c, "sting.mp3".into(), 5.5, 1.3);
        assert_eq!(c.duration(), 5.5);
    }

    #[test]
    fn round_trips_ddma_schema_without_dropping_source_coords() {
        // Shape of a real DDMA clip (episode 247 style), read-only fixture.
        let j = r#"[{"num":11,"title":"T","start":1448.72,"end":1599.54,"duration":150.82,
          "bridge_text":["hook?"],"locked":true,"hidden":false,"mosaic_run_id":"abc",
          "segments":[{"type":"audio","start":1481.54,"end":1498.5,"duration":16.96,"text":"hi"},
                      {"type":"music","music_file":"s.mp3","duration":4.5,"crossfade":0,"volume":1}]}]"#;
        let plan: Plan = serde_json::from_str(j).unwrap();
        let out = serde_json::to_string(&plan).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["start"], 1448.72);
        assert_eq!(v[0]["end"], 1599.54);
        assert_eq!(v[0]["duration"], 150.82);
        assert_eq!(v[0]["mosaic_run_id"], "abc");
        assert_eq!(v[0]["intro_offset"], 1.5);
        assert_eq!(v[0]["outro_duration"], 5.0);
    }

    #[test]
    fn sync_never_overwrites_source_coords_and_reconcile_halts() {
        let mut c = Clip { num: 1, title: "t".into(), source_start: Some(100.0), source_end: Some(250.0), ..Default::default() };
        c.segments = vec![Segment::Audio { start: 0.0, end: 10.0, text: "x".into(), duration: None }];
        c.sync_distribution_fields();
        assert_eq!(c.source_start, Some(100.0));
        assert_eq!(c.source_duration, Some(150.0));
        c.rendered_duration = Some(100.0);
        assert!(c.reconcile(101.0).is_ok());
        assert!(c.reconcile(120.0).is_err());
    }
}
