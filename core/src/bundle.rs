//! Episode bundle: the repo contract the publisher watches (strategy §1).
//! `src/ddma/docs/episodes/<N>/{plan.json, transcription.json, cover.webp, clips/<N>-<k>.mp4}`
//! The kitchen writes this; the publisher never writes back into it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::plan::Plan;
use crate::Result;

pub fn episode_dir(project: &Path, episode: u32) -> PathBuf {
    project.join("src/ddma/docs/episodes").join(episode.to_string())
}
pub fn clip_filename(episode: u32, clip: u32) -> String {
    format!("{episode}-{clip}.mp4")
}
pub fn clip_path(project: &Path, episode: u32, clip: u32) -> PathBuf {
    episode_dir(project, episode).join("clips").join(clip_filename(episode, clip))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

/// Validate transcription.json shape (strategy §1): text + segments[] with words[].
/// Lossless: callers pass the raw JSON value, we never reserialize unknown fields away.
pub fn validate_transcription(v: &serde_json::Value) -> Result<()> {
    if !v.get("text").map(|t| t.is_string()).unwrap_or(false) {
        return crate::msg("transcription.json: missing string field `text`");
    }
    let segs = v.get("segments").and_then(|s| s.as_array())
        .ok_or_else(|| crate::CoreError::Msg("transcription.json: missing segments[]".into()))?;
    for s in segs {
        for k in ["start", "end", "text"] {
            if s.get(k).is_none() {
                return crate::msg(format!("transcription.json: segment missing `{k}`"));
            }
        }
        if !s.get("words").map(|w| w.is_array()).unwrap_or(false) {
            return crate::msg("transcription.json: segment missing words[] (word timestamps required)");
        }
    }
    Ok(())
}

/// Write a new episode bundle. Refuses to overwrite an episode that already
/// has rendered clips, so historical episodes cannot be clobbered.
pub fn write_bundle(
    project: &Path,
    episode: u32,
    plan: &Plan,
    transcription: &serde_json::Value,
    cover_webp: Option<&[u8]>,
) -> Result<PathBuf> {
    validate_transcription(transcription)?;
    let dir = episode_dir(project, episode);
    let clips_dir = dir.join("clips");
    if clips_dir.is_dir() {
        let has_mp4 = std::fs::read_dir(&clips_dir)?.any(|e| {
            e.map(|x| x.path().extension().map(|s| s == "mp4").unwrap_or(false)).unwrap_or(false)
        });
        if has_mp4 {
            return crate::msg(format!("episode {episode} already has rendered clips; refusing to overwrite its bundle"));
        }
    }
    let mut plan = plan.clone();
    for c in &mut plan { c.sync_distribution_fields(); }
    std::fs::create_dir_all(&clips_dir)?;
    std::fs::write(dir.join("plan.json"), serde_json::to_string_pretty(&plan)?)?;
    std::fs::write(dir.join("transcription.json"), serde_json::to_string(transcription)?)?;
    if let Some(cover) = cover_webp {
        std::fs::write(dir.join("cover.webp"), cover)?;
    }
    Ok(dir)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleReport {
    pub episode: u32,
    pub dir: PathBuf,
    pub plan_clips: usize,
    pub present_clips: Vec<u32>,
    pub missing_clips: Vec<u32>,
    pub sha256: BTreeMap<u32, String>,
    pub issues: Vec<String>,
}

impl BundleReport {
    /// Publisher preflight (strategy §6.1): all three files parse and every planned clip exists.
    pub fn ready(&self) -> bool { self.issues.is_empty() && self.missing_clips.is_empty() }
}

/// Inspect a bundle without modifying it. When `probe` is true and ffprobe is
/// available, rendered_duration is reconciled against each mp4 (strategy §2).
pub fn validate_bundle(project: &Path, episode: u32, probe: bool) -> Result<BundleReport> {
    let dir = episode_dir(project, episode);
    let mut issues = Vec::new();
    let plan_path = dir.join("plan.json");
    let plan: Plan = if plan_path.exists() {
        match std::fs::read_to_string(&plan_path).map_err(crate::CoreError::from)
            .and_then(|s| serde_json::from_str(&s).map_err(crate::CoreError::from)) {
            Ok(p) => p,
            Err(e) => { issues.push(format!("plan.json: {e}")); Plan::new() }
        }
    } else { issues.push("plan.json missing".into()); Plan::new() };
    match std::fs::read_to_string(dir.join("transcription.json")) {
        Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(v) => if let Err(e) = validate_transcription(&v) { issues.push(e.to_string()); },
            Err(e) => issues.push(format!("transcription.json: {e}")),
        },
        Err(_) => issues.push("transcription.json missing".into()),
    }
    if !dir.join("cover.webp").exists() { issues.push("cover.webp missing".into()); }
    let (mut present, mut missing, mut sha256) = (Vec::new(), Vec::new(), BTreeMap::new());
    for clip in &plan {
        let p = clip_path(project, episode, clip.num);
        if p.exists() {
            present.push(clip.num);
            if let Ok(h) = sha256_file(&p) { sha256.insert(clip.num, h); }
            if probe {
                if let Ok(dur) = crate::media::probe_duration_secs(&p) {
                    if let Err(e) = clip.reconcile(dur) { issues.push(e.to_string()); }
                }
            }
        } else { missing.push(clip.num); }
    }
    Ok(BundleReport { episode, dir, plan_clips: plan.len(), present_clips: present, missing_clips: missing, sha256, issues })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{Clip, Segment};

    fn plan() -> Plan {
        let mut c = Clip { num: 1, title: "Hook title".into(), bridge_text: vec!["Why?".into()], ..Default::default() };
        c.segments = vec![Segment::Audio { start: 10.0, end: 70.0, text: "body".into(), duration: None }];
        c.sync_distribution_fields();
        vec![c]
    }
    fn transcript() -> serde_json::Value {
        serde_json::json!({"text":"body","language":"en","segments":[{"id":0,"start":10.0,"end":70.0,"text":"body","words":[{"word":"body","start":10.0,"end":10.4}]}]})
    }

    #[test]
    fn write_then_validate_bundle_ready_after_clip_lands() {
        let d = tempfile::tempdir().unwrap();
        let dir = write_bundle(d.path(), 248, &plan(), &transcript(), Some(b"webp")).unwrap();
        assert!(dir.join("plan.json").exists() && dir.join("transcription.json").exists() && dir.join("cover.webp").exists());
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("plan.json")).unwrap()).unwrap();
        assert_eq!(v[0]["start"], 10.0); assert_eq!(v[0]["intro_offset"], 1.5);
        let r = validate_bundle(d.path(), 248, false).unwrap();
        assert_eq!(r.missing_clips, vec![1]); assert!(!r.ready());
        std::fs::write(clip_path(d.path(), 248, 1), b"fake-mp4").unwrap();
        let r = validate_bundle(d.path(), 248, false).unwrap();
        assert!(r.ready()); assert_eq!(r.sha256.len(), 1);
        // Historical protection: cannot rewrite once clips exist.
        assert!(write_bundle(d.path(), 248, &plan(), &transcript(), None).is_err());
    }

    #[test]
    fn rejects_transcription_without_word_timestamps() {
        let d = tempfile::tempdir().unwrap();
        let bad = serde_json::json!({"text":"x","segments":[{"start":0,"end":1,"text":"x"}]});
        assert!(write_bundle(d.path(), 249, &plan(), &bad, None).is_err());
    }

    #[test]
    fn read_only_golden_247_contract() {
        // Read-only check against the live reference episode, when present.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deepDive");
        if !root.join("src/ddma/docs/episodes/247/plan.json").exists() { return; }
        let r = validate_bundle(&root, 247, false).unwrap();
        assert_eq!(r.plan_clips, 24);
        assert!(r.present_clips.contains(&11));
        assert!(r.issues.iter().all(|i| !i.contains("plan.json:")), "plan issues: {:?}", r.issues);
    }
}
