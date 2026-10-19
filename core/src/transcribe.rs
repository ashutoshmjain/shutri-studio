//! Transcription contract: word-level timestamps are mandatory, because
//! clip boundaries snap to them. Engines sit behind this trait.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::narrative::Word;
use crate::Result;

pub trait Transcriber {
    fn name(&self) -> &str;
    fn transcribe(&self, audio: &Path, model: &Path) -> Result<Vec<Word>>;
}

/// whisper.cpp CLI (`whisper-cli -ojf` full JSON with word timestamps).
pub struct WhisperCpp {
    pub binary: String,
}

impl Transcriber for WhisperCpp {
    fn name(&self) -> &str {
        "whisper.cpp"
    }

    fn transcribe(&self, audio: &Path, model: &Path) -> Result<Vec<Word>> {
        let out_base = std::env::temp_dir().join("studio-transcript");
        let out = Command::new(&self.binary)
            .args(["-m", &model.to_string_lossy(), "-f", &audio.to_string_lossy(), "-ojf", "-of", &out_base.to_string_lossy()])
            .output();
        let out = match out {
            Ok(o) => o,
            Err(e) => {
                return crate::msg(format!(
                    "could not run {} ({e}). Install whisper.cpp and point the app at whisper-cli, or use a cloud transcriber.",
                    self.binary
                ))
            }
        };
        if !out.status.success() {
            return crate::msg(format!("whisper failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        let json = std::fs::read_to_string(std::env::temp_dir().join("studio-transcript.json"))?;
        parse_whisper_json(&json)
    }
}

/// Parse whisper.cpp `-ojf` output into words. Tokens like `[_BEG_]` are dropped.
pub fn parse_whisper_json(json: &str) -> Result<Vec<Word>> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let mut words = Vec::new();
    let segs = v
        .get("transcription")
        .and_then(|t| t.as_array())
        .ok_or_else(|| crate::CoreError::Msg("no transcription in whisper output".into()))?;
    for seg in segs {
        if let Some(tokens) = seg.get("tokens").and_then(|t| t.as_array()) {
            for tok in tokens {
                let text = tok.get("text").and_then(|t| t.as_str()).unwrap_or("").trim();
                if text.is_empty() || text.starts_with("[_") {
                    continue;
                }
                let off = tok.get("offsets");
                let (from, to) = (
                    off.and_then(|o| o.get("from")).and_then(|x| x.as_i64()).unwrap_or(0),
                    off.and_then(|o| o.get("to")).and_then(|x| x.as_i64()).unwrap_or(0),
                );
                words.push(Word {
                    text: text.to_string(),
                    start: from as f64 / 1000.0,
                    end: to as f64 / 1000.0,
                });
            }
        }
    }
    // Contract: timestamps exist and are monotonic.
    if words.iter().any(|w| w.end < w.start) || words.windows(2).any(|p| p[1].start < p[0].start) {
        return crate::msg("word timestamps are not monotonic");
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_words() {
        let j = r#"{"transcription":[{"tokens":[
          {"text":" Hello","offsets":{"from":0,"to":400}},
          {"text":" world","offsets":{"from":400,"to":900}}]}]}"#;
        let w = parse_whisper_json(j).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].start, 0.0);
    }
}

/// Copy an uploaded audio file into `episodes/<N>/audio/` (the Studio
/// input is the ORIGINAL NotebookLM audio; the Audacity podcast edit
/// lives outside the app).
pub fn copy_audio(project: &Path, episode: u32, source: &Path) -> Result<PathBuf> {
    let ext = source.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !["mp3", "wav", "m4a", "flac", "ogg"].contains(&ext.as_str()) {
        return crate::msg("audio must be .mp3, .wav, .m4a, .flac or .ogg");
    }
    let name = source.file_name().ok_or_else(|| crate::CoreError::Msg("audio file has no name".into()))?;
    let dir = crate::bundle::episode_dir(project, episode).join("audio");
    std::fs::create_dir_all(&dir)?;
    let dest: PathBuf = dir.join(name);
    std::fs::copy(source, &dest)?;
    Ok(dest)
}

/// Persist word timing as the bundle transcription.json (sentence-grouped
/// segments, bundle contract shape) plus narrative.md (single-narrator
/// conversation the Narrative/clip steps build on). Returns word count.
pub fn save_transcription(project: &Path, episode: u32, words: &[Word]) -> Result<usize> {
    if words.is_empty() {
        return crate::msg("transcription produced no words");
    }
    let dir = crate::bundle::episode_dir(project, episode);
    std::fs::create_dir_all(&dir)?;
    let text = words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    let mut segments = Vec::new();
    let mut cur: Vec<&Word> = Vec::new();
    let flush = |cur: &mut Vec<&Word>, segments: &mut Vec<serde_json::Value>| {
        if cur.is_empty() { return; }
        segments.push(serde_json::json!({
            "start": cur.first().unwrap().start,
            "end": cur.last().unwrap().end,
            "text": cur.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "),
            "words": cur.iter().map(|w| serde_json::json!({"word": w.text, "start": w.start, "end": w.end})).collect::<Vec<_>>(),
        }));
        cur.clear();
    };
    for w in words {
        cur.push(w);
        if [".", "?", "!"].iter().any(|p| w.text.ends_with(p)) { flush(&mut cur, &mut segments); }
    }
    flush(&mut cur, &mut segments);
    let doc = serde_json::json!({"text": text, "language": "en", "segments": segments});
    crate::bundle::validate_transcription(&doc)?;
    std::fs::write(dir.join("transcription.json"), serde_json::to_string_pretty(&doc)?)?;
    let narrative = crate::narrative::Narrative {
        title: format!("Episode {episode}"),
        turns: vec![crate::narrative::Turn {
            speaker: "Narrator".into(),
            text: text.clone(),
            words: words.to_vec(),
            start: words.first().map(|w| w.start).unwrap_or(0.0),
            end: words.last().map(|w| w.end).unwrap_or(0.0),
        }],
    };
    std::fs::write(dir.join("narrative.md"), narrative.to_markdown())?;
    Ok(words.len())
}

/// Load saved word timing for an episode (clip planning with REAL timing).
pub fn load_words(project: &Path, episode: u32) -> Result<Vec<Word>> {
    let path = crate::bundle::episode_dir(project, episode).join("transcription.json");
    let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    let mut words = Vec::new();
    if let Some(segs) = doc.get("segments").and_then(|s| s.as_array()) {
        for seg in segs {
            if let Some(ws) = seg.get("words").and_then(|w| w.as_array()) {
                for w in ws {
                    words.push(Word {
                        text: w.get("word").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                        start: w.get("start").and_then(|t| t.as_f64()).unwrap_or(0.0),
                        end: w.get("end").and_then(|t| t.as_f64()).unwrap_or(0.0),
                    });
                }
            }
        }
    }
    if words.is_empty() {
        return crate::msg("transcription.json has no word timestamps — transcribe the audio first");
    }
    Ok(words)
}

#[cfg(test)]
mod save_tests {
    use super::*;

    fn words() -> Vec<Word> {
        vec![
            Word { text: "Hello".into(), start: 0.0, end: 0.4 },
            Word { text: "world.".into(), start: 0.4, end: 0.9 },
            Word { text: "Again".into(), start: 1.0, end: 1.4 },
        ]
    }

    #[test]
    fn save_then_load_round_trips() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(save_transcription(d.path(), 7, &words()).unwrap(), 3);
        assert!(crate::bundle::episode_dir(d.path(), 7).join("narrative.md").exists());
        let loaded = load_words(d.path(), 7).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[1].text, "world.");
    }

    #[test]
    fn copy_audio_rejects_video() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("clip.mp4");
        std::fs::write(&f, b"x").unwrap();
        assert!(copy_audio(d.path(), 7, &f).is_err());
    }
}
