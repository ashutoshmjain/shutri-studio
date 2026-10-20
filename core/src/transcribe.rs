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

/// One raw engine token, before subword fragments are merged.
#[derive(Debug, Clone, PartialEq)]
pub struct RawToken {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// Merge subword (BPE) fragments into whole words: a token that does
/// not start with whitespace continues the previous word, so
/// `[" Inter", "nal"]` becomes one word `Internal` spanning both
/// token timestamps — never two clip-boundary targets mid-syllable.
pub fn aggregate_tokens(tokens: &[RawToken]) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    for tok in tokens {
        if tok.text.trim().is_empty() || tok.text.trim_start().starts_with("[_") {
            continue;
        }
        let continues = !tok.text.starts_with(|c: char| c.is_whitespace()) && !words.is_empty();
        if continues {
            let last = words.last_mut().unwrap();
            last.text.push_str(tok.text.trim());
            last.end = tok.end.max(last.end);
        } else {
            words.push(Word { text: tok.text.trim().to_string(), start: tok.start, end: tok.end });
        }
    }
    words
}

fn check_monotonic(words: &[Word]) -> Result<()> {
    if words.iter().any(|w| w.end < w.start) || words.windows(2).any(|p| p[1].start < p[0].start) {
        return crate::msg("word timestamps are not monotonic");
    }
    Ok(())
}

/// Parse whisper.cpp `-ojf` output into words. Tokens like `[_BEG_]` are dropped.
pub fn parse_whisper_json(json: &str) -> Result<Vec<Word>> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let mut raw = Vec::new();
    let segs = v
        .get("transcription")
        .and_then(|t| t.as_array())
        .ok_or_else(|| crate::CoreError::Msg("no transcription in whisper output".into()))?;
    for seg in segs {
        if let Some(tokens) = seg.get("tokens").and_then(|t| t.as_array()) {
            for tok in tokens {
                // Keep the raw (untrimmed) text: its leading space is
                // what tells aggregate_tokens a new word has begun.
                let text = tok.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let off = tok.get("offsets");
                let (from, to) = (
                    off.and_then(|o| o.get("from")).and_then(|x| x.as_i64()).unwrap_or(0),
                    off.and_then(|o| o.get("to")).and_then(|x| x.as_i64()).unwrap_or(0),
                );
                raw.push(RawToken { text: text.to_string(), start: from as f64 / 1000.0, end: to as f64 / 1000.0 });
            }
        }
    }
    let words = aggregate_tokens(&raw);
    check_monotonic(&words)?;
    Ok(words)
}

/// A speech region from voice-activity detection (Silero VAD in the
/// sherpa-onnx pipeline). Words outside every region are music,
/// silence or sting bleed and must not become clip boundaries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeechSegment {
    pub start: f64,
    pub end: f64,
}

/// Keep only words overlapping a speech region (50 ms ear padding,
/// below the ~30-50 ms clipping threshold is not attempted).
pub fn retain_speech(words: &[Word], segments: &[SpeechSegment]) -> Vec<Word> {
    if segments.is_empty() {
        return words.to_vec();
    }
    words
        .iter()
        .filter(|w| segments.iter().any(|s| w.end >= s.start - 0.05 && w.start <= s.end + 0.05))
        .cloned()
        .collect()
}

/// Parse sherpa-onnx offline output (`{"text":…, "tokens":[…],
/// "timestamps":[…]}`; timestamps are per-token start seconds).
/// Token ends come from the next token's start.
pub fn parse_sherpa_json(json: &str) -> Result<Vec<Word>> {
    // The binary logs to stdout too: take the last JSON object line.
    let candidate = json
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{') && l.contains("\"tokens\""))
        .unwrap_or(json);
    let v: serde_json::Value = serde_json::from_str(candidate)?;
    let tokens = v.get("tokens").and_then(|t| t.as_array())
        .ok_or_else(|| crate::CoreError::Msg("no tokens in sherpa-onnx output".into()))?;
    let stamps: Vec<f64> = v
        .get("timestamps")
        .and_then(|t| t.as_array())
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect())
        .unwrap_or_default();
    if !stamps.is_empty() && stamps.len() != tokens.len() {
        return crate::msg("sherpa-onnx token/timestamp count mismatch");
    }
    let mut raw = Vec::new();
    for (i, tok) in tokens.iter().enumerate() {
        let start = stamps.get(i).copied().unwrap_or(0.0);
        let end = stamps.get(i + 1).copied().unwrap_or(start + 0.4);
        raw.push(RawToken {
            text: tok.as_str().unwrap_or("").to_string(),
            start,
            end,
        });
    }
    let words = aggregate_tokens(&raw);
    check_monotonic(&words)?;
    Ok(words)
}

/// Default sherpa-onnx model store:
/// `%LOCALAPPDATA%\ShutriStudio\models` on Windows,
/// `$XDG_DATA_HOME/shutri-studio/models` (or `~/.local/share/…`)
/// elsewhere. `SHERPA_ONNX_MODELS_DIR` overrides it.
pub fn sherpa_models_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("SHERPA_ONNX_MODELS_DIR") {
        return PathBuf::from(d);
    }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("ShutriStudio").join("models");
        }
    }
    if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(x).join("shutri-studio").join("models");
    }
    if let Some(h) = std::env::var_os("HOME") {
        return PathBuf::from(h).join(".local").join("share").join("shutri-studio").join("models");
    }
    PathBuf::from("models")
}

fn find_model(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    // Models land either flat or in their release sub-folder.
    let mut dirs = vec![dir.to_path_buf()];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                dirs.push(e.path());
            }
        }
    }
    for d in &dirs {
        for n in names {
            let p = d.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// (encoder, decoder, tokens) for the bundled Whisper ONNX model,
/// or the names of the pieces still missing.
pub fn sherpa_model_files(dir: &Path) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    (
        find_model(dir, &["tiny.en-encoder.int8.onnx", "tiny.en-encoder.onnx"]),
        find_model(dir, &["tiny.en-decoder.int8.onnx", "tiny.en-decoder.onnx"]),
        find_model(dir, &["tiny.en-tokens.txt"]),
    )
}

pub fn missing_sherpa_models(dir: &Path) -> Vec<&'static str> {
    let (e, d, t) = sherpa_model_files(dir);
    let mut missing = Vec::new();
    if e.is_none() { missing.push("tiny.en-encoder(.int8).onnx"); }
    if d.is_none() { missing.push("tiny.en-decoder(.int8).onnx"); }
    if t.is_none() { missing.push("tiny.en-tokens.txt"); }
    // Silero VAD ships alongside the recognizer models.
    if find_model(dir, &["silero_vad.onnx"]).is_none() { missing.push("silero_vad.onnx"); }
    missing
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    for dir in std::env::var_os("PATH").map(|v| std::env::split_paths(&v).collect::<Vec<_>>()).unwrap_or_default() {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
        if cfg!(windows) {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

/// sherpa-onnx native engine (the default): Silero VAD +
/// Whisper ONNX + token-to-word aggregation, no Python, no
/// whisper.cpp CLI and no hand-picked `.bin` model path in the UI.
/// Runs the `sherpa-onnx-offline` binary from PATH (or
/// `SHERPA_ONNX_BIN`) against models auto-detected in
/// [`sherpa_models_dir`]; model argument is ignored.
pub struct SherpaOnnxTranscriber {
    pub binary: Option<String>,
    pub models_dir: PathBuf,
}

impl Default for SherpaOnnxTranscriber {
    fn default() -> Self {
        Self { binary: None, models_dir: sherpa_models_dir() }
    }
}

impl Transcriber for SherpaOnnxTranscriber {
    fn name(&self) -> &str {
        "sherpa-onnx"
    }

    fn transcribe(&self, audio: &Path, _model: &Path) -> Result<Vec<Word>> {
        let missing = missing_sherpa_models(&self.models_dir);
        if !missing.is_empty() {
            return crate::msg(format!(
                "sherpa-onnx models are not installed in {} (missing: {}). \
                 Download sherpa-onnx-whisper-tiny.en.tar.bz2 and silero_vad.onnx into that folder, then try again.",
                self.models_dir.display(),
                missing.join(", ")
            ));
        }
        let binary = self
            .binary
            .clone()
            .or_else(|| std::env::var("SHERPA_ONNX_BIN").ok())
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(|| {
                let p = self.models_dir.parent()?.join("bin").join("sherpa-onnx-offline.exe");
                if p.is_file() { Some(p) } else { None }
            })
            .or_else(|| {
                let p = self.models_dir.parent()?.join("bin").join("sherpa-onnx-offline");
                if p.is_file() { Some(p) } else { None }
            })
            .or_else(|| find_on_path("sherpa-onnx-offline"));
        let binary = match binary {
            Some(b) => b,
            None => {
                return crate::msg(
                    "sherpa-onnx-offline was not found on PATH. Install sherpa-onnx, or set SHERPA_ONNX_BIN to its full path (Advanced settings).",
                )
            }
        };
        let (encoder, decoder, tokens) = sherpa_model_files(&self.models_dir);

        // sherpa-onnx's wave-reader strictly expects a 16-bit PCM RIFF .wav file.
        // Convert any .m4a, .mp3, .ogg, or non-compliant .wav to a 16kHz mono WAV via ffmpeg.
        let temp_wav = std::env::temp_dir().join(format!("studio-sherpa-{}.wav", std::process::id()));
        let conv = Command::new("ffmpeg")
            .args([
                "-y",
                "-i",
                &audio.to_string_lossy(),
                "-ar",
                "16000",
                "-ac",
                "1",
                "-c:a",
                "pcm_s16le",
                &temp_wav.to_string_lossy(),
            ])
            .output();

        let wav_path = match conv {
            Ok(c) if c.status.success() => temp_wav.clone(),
            _ => audio.to_path_buf(),
        };

        let out = Command::new(&binary)
            .arg(format!("--whisper-encoder={}", encoder.unwrap().display()))
            .arg(format!("--whisper-decoder={}", decoder.unwrap().display()))
            .arg(format!("--tokens={}", tokens.unwrap().display()))
            .arg(&wav_path)
            .output();

        let _ = std::fs::remove_file(&temp_wav);

        let out = match out {
            Ok(o) => o,
            Err(e) => return crate::msg(format!("could not run {} ({e})", binary.display())),
        };
        if !out.status.success() {
            return crate::msg(format!("sherpa-onnx failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        parse_sherpa_json(&String::from_utf8_lossy(&out.stdout))
    }
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

    #[test]
    fn subword_fragments_merge_into_one_word() {
        // The whisper.cpp defect from the tracker: [" Inter", "nal"]
        // must be ONE word spanning both timestamps, not two.
        let j = r#"{"transcription":[{"tokens":[
          {"text":" Inter","offsets":{"from":0,"to":300}},
          {"text":"nal","offsets":{"from":300,"to":600}},
          {"text":" drowning","offsets":{"from":600,"to":1100}}]}]}"#;
        let w = parse_whisper_json(j).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].text, "Internal");
        assert_eq!((w[0].start, w[0].end), (0.0, 0.6));
    }

    #[test]
    fn sherpa_tokens_aggregate_with_token_timestamps() {
        let j = "log line\n{\"text\":\" Internal drowning\",\"tokens\":[\" Inter\",\"nal\",\" drowning\"],\"timestamps\":[0.0,0.3,0.6]}";
        let w = parse_sherpa_json(j).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].text, "Internal");
        assert_eq!((w[0].start, w[0].end), (0.0, 0.6));
        assert_eq!(w[1].text, "drowning");
    }

    #[test]
    fn vad_drops_words_in_music_stings() {
        let words = vec![
            Word { text: "sting".into(), start: 0.0, end: 0.4 },
            Word { text: "Hello".into(), start: 5.0, end: 5.4 },
        ];
        let kept = retain_speech(&words, &[SpeechSegment { start: 4.9, end: 6.0 }]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].text, "Hello");
        assert_eq!(retain_speech(&words, &[]).len(), 2);
    }

    #[test]
    fn missing_models_are_named_not_silent() {
        let d = tempfile::tempdir().unwrap();
        let missing = missing_sherpa_models(d.path());
        assert_eq!(missing.len(), 4);
        assert!(missing.iter().any(|m| m.contains("silero_vad")));
        let engine = SherpaOnnxTranscriber { binary: None, models_dir: d.path().to_path_buf() };
        let err = engine.transcribe(Path::new("x.mp3"), Path::new("")).unwrap_err().to_string();
        assert!(err.contains("missing:"), "{err}");
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
