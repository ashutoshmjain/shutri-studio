//! Media rendering via the bundled FFmpeg sidecar — the real engine,
//! driven directly by the Rust core (no Python wrapper).

use std::path::Path;
use std::process::Command;

use crate::Result;

fn run(tool: &str, args: &[String]) -> Result<String> {
    let out = Command::new(tool).args(args).output()?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        crate::msg(format!("{tool} failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

pub fn probe_duration_secs(media: &Path) -> Result<f64> {
    let out = run(
        "ffprobe",
        &[
            "-v".into(),
            "error".into(),
            "-show_entries".into(),
            "format=duration".into(),
            "-of".into(),
            "default=noprint_wrappers=1:nokey=1".into(),
            media.to_string_lossy().into(),
        ],
    )?;
    out.trim()
        .parse::<f64>()
        .map_err(|_| crate::CoreError::Msg(format!("could not parse duration from {out}")))
}

/// Render one 740x740 clip: cut [start,end] of the master audio, lay it
/// over a cover image (or generated colour), loudness-safe AAC, <20MB target.
pub fn render_clip(
    master_audio: &Path,
    cover: Option<&Path>,
    start: f64,
    end: f64,
    out_path: &Path,
) -> Result<String> {
    let dur = (end - start).max(0.1);
    let mut args: Vec<String> = vec!["-y".into()];
    match cover {
        Some(c) => {
            args.extend(["-loop".into(), "1".into(), "-i".into(), c.to_string_lossy().into()]);
        }
        None => {
            args.extend([
                "-f".into(), "lavfi".into(), "-i".into(),
                "color=c=0x1c1917:s=740x740".into(),
            ]);
        }
    }
    args.extend([
        "-ss".into(), format!("{start}"),
        "-t".into(), format!("{dur}"),
        "-i".into(), master_audio.to_string_lossy().into(),
        "-vf".into(), "scale=740:740:force_original_aspect_ratio=increase,crop=740:740,format=yuv420p".into(),
        "-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(),
        "-c:a".into(), "aac".into(), "-b:a".into(), "128k".into(),
        "-shortest".into(), "-movflags".into(), "+faststart".into(),
        out_path.to_string_lossy().into(),
    ]);
    run("ffmpeg", &args)?;
    Ok(out_path.to_string_lossy().into())
}
