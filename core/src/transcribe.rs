//! Transcription contract: word-level timestamps are mandatory, because
//! clip boundaries snap to them. Engines sit behind this trait.

use std::path::Path;
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
        let out = Command::new(&self.binary)
            .args(["-m", &model.to_string_lossy(), "-f", &audio.to_string_lossy(), "-ojf", "-of", "studio-transcript"])
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
        let json = std::fs::read_to_string("studio-transcript.json")?;
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
