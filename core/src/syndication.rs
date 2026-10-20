//! Mempool -> template publication blocks, ported from deepDive's
//! cockpit (`ensure_syndication_and_wallet_blocks` and
//! `sync_episode_clips_and_markdown` in `ingest/server.py`).
//!
//! Mining an episode (mempool -> template) must, in order:
//!   1. assign the next episode number (the master key),
//!   2. add the donation wallet (Twentyuno Lightning widget),
//!   3. add the audio platform links (Spotify / Apple / Fountain),
//! all inserted directly above Works Cited. The video carousel and
//! video platform links (TikTok / Instagram / YouTube) cannot exist
//! at mine time — they are added later by [`sync_video_carousel`]
//! once clip videos have actually been built.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;

/// Per-project platform/wallet settings. Defaults match deepDive;
/// override per project in `.studio/syndication.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Syndication {
    pub lightning_address: String,
    pub spotify_url: String,
    pub apple_podcasts_url: String,
    pub fountain_url: String,
    pub tiktok_url: String,
    pub instagram_url: String,
    pub youtube_url: String,
}

impl Default for Syndication {
    fn default() -> Self {
        Self {
            lightning_address: "shutosha@primal.net".into(),
            spotify_url: "https://open.spotify.com/show/7doWf0GON9JsG6r8igc7RE".into(),
            apple_podcasts_url: "https://podcasts.apple.com/us/podcast/deep-dive-with-gemini/id1844532251".into(),
            fountain_url: "https://fountain.fm/show/7LBvZT6ffpGyubvk8aSF".into(),
            tiktok_url: "https://www.tiktok.com/@shutoshabot".into(),
            instagram_url: "https://www.instagram.com/shutoshabot/".into(),
            youtube_url: "https://www.youtube.com/playlist?list=PLIX4sFsmu37q8rU8HKTLhdLPZQadcvx-K".into(),
        }
    }
}

/// Save per-project settings to `.studio/syndication.json`
/// (local app state, git-ignored). Trims whitespace; refuses to save
/// a record whose lightning address and every URL are all empty.
pub fn save_syndication(project: &Path, s: &Syndication) -> Result<()> {
    let clean = Syndication {
        lightning_address: s.lightning_address.trim().to_string(),
        spotify_url: s.spotify_url.trim().to_string(),
        apple_podcasts_url: s.apple_podcasts_url.trim().to_string(),
        fountain_url: s.fountain_url.trim().to_string(),
        tiktok_url: s.tiktok_url.trim().to_string(),
        instagram_url: s.instagram_url.trim().to_string(),
        youtube_url: s.youtube_url.trim().to_string(),
    };
    if [&clean.lightning_address, &clean.spotify_url, &clean.apple_podcasts_url,
        &clean.fountain_url, &clean.tiktok_url, &clean.instagram_url, &clean.youtube_url]
        .iter().all(|v| v.is_empty())
    {
        return crate::msg("settings are empty — enter at least one wallet address or platform URL");
    }
    let dir = project.join(".studio");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("syndication.json"), serde_json::to_string_pretty(&clean)?)?;
    Ok(())
}

/// Load `.studio/syndication.json` when present, else the defaults.
pub fn load_syndication(project: &Path) -> Syndication {
    let path = project.join(".studio").join("syndication.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn button(url: &str, label: &str, last: bool) -> String {
    let margin = if last { "margin-top: 10px;" } else { "margin-top: 10px; margin-right: 10px;" };
    format!("<a href=\"{url}\" target=\"_blank\" style=\"background-color: #2E2E2E; color: white; padding: 10px 20px; text-align: center; text-decoration: none; display: inline-block; border-radius: 5px; {margin}\">{label}</a>")
}

/// The wallet + audio-platform block (deepDive's social block).
pub fn syndication_block(s: &Syndication) -> String {
    format!(
        "---\n\n### Tips and Donations\n\nIf you enjoyed this research, consider supporting the project with a tip in **Sats**. It's a simple, global way to support independent research.\n\n<!-- SOCIALS_START -->\n\n<center>\n<lightning-widget\n  name=\"Thanks for supporting the publication\"\n  accent=\"#f9ce00\"\n  to=\"{}\"\n/>\n</center>\n<script src=\"https://embed.twentyuno.net/js/app.js\"></script>\n\n<center>{}{}{}</center>\n\n<!-- SOCIALS_END -->\n\nTo send Sats, you'll need a [lightning wallet](https://lightningaddress.com/).\n\n---",
        s.lightning_address,
        button(&s.spotify_url, "Spotify", false),
        button(&s.apple_podcasts_url, "Apple Podcasts", false),
        button(&s.fountain_url, "Fountain.fm", true),
    )
}

/// Guarantee the wallet + audio links exist, directly above Works
/// Cited / References / Sources / Bibliography. Idempotent.
pub fn ensure_syndication_and_wallet_blocks(content: &str, s: &Syndication) -> String {
    if content.contains("lightning-widget") && content.contains("Tips and Donations") {
        return content.to_string();
    }
    // Drop an orphan SOCIALS span so it is never duplicated.
    let mut cleaned = content.to_string();
    if let (Some(a), Some(b)) = (cleaned.find("<!-- SOCIALS_START -->"), cleaned.find("<!-- SOCIALS_END -->")) {
        if b > a {
            let end = b + "<!-- SOCIALS_END -->".len();
            cleaned = format!("{}{}", cleaned[..a].trim_end(), cleaned[end..].trim_start_matches('\n'));
        }
    }
    let block = syndication_block(s);
    for heading in ["## Works Cited", "## References", "## Sources", "## Bibliography", "# Works Cited"] {
        if let Some(idx) = cleaned.find(heading) {
            return format!("{}\n\n{}\n\n{}", cleaned[..idx].trim_end(), block, cleaned[idx..].trim_start());
        }
    }
    format!("{}\n\n{}\n", cleaned.trim_end(), block)
}

/// Build the `VIDEO_STRIP` carousel for clips already on disk.
/// Relative src: `src/<N>.md` -> `../episodes/<N>/clips/<file>`.
pub fn video_carousel_block(episode: u32, clip_names: &[String], s: &Syndication) -> String {
    let mut cards = String::new();
    for name in clip_names {
        let label = name.trim_end_matches(".mp4");
        cards.push_str(&format!(
            "  <div style=\"flex: 0 0 60%; scroll-snap-align: center; position: relative; border-radius: 12px; overflow: hidden; background: #000; aspect-ratio: 1/1; display: flex; flex-direction: column; box-shadow: 0 4px 15px rgba(0,0,0,0.3);\">\n    <video src=\"../episodes/{episode}/clips/{name}\" style=\"width: 100%; height: 85%; object-fit: contain;\" playsinline loop preload=\"metadata\" muted></video>\n    <div style=\"height: 15%; background: #1a1a1a; color: #ccc; display: flex; align-items: center; justify-content: center; font-family: monospace; font-size: 12px; border-top: 1px solid #333;\">{label}</div>\n  </div>\n"
        ));
    }
    format!(
        "<!-- VIDEO_STRIP_START -->\n<div class=\"video-carousel-container\" style=\"display: flex; overflow-x: auto; scroll-snap-type: x mandatory; gap: 15px; padding: 20px 0; scroll-behavior: smooth;\">\n{cards}</div>\n\n<center>{}{}{}</center>\n<!-- VIDEO_STRIP_END -->",
        button(&s.tiktok_url, "▶ TikTok ◀", false),
        button(&s.instagram_url, "◈ Instagram ◈", false),
        button(&s.youtube_url, "⫸ YouTube ⫷", true),
    )
}

/// Insert or replace the carousel in an episode article. With no
/// clips on disk this is a no-op — an empty carousel is never added.
pub fn sync_video_carousel(project: &Path, episode: u32, s: &Syndication) -> Result<usize> {
    let clips_dir = crate::bundle::episode_dir(project, episode).join("clips");
    let mut names: Vec<String> = Vec::new();
    if clips_dir.is_dir() {
        for e in std::fs::read_dir(&clips_dir)? {
            let p = e?.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.ends_with(".mp4") && !name.contains("-original") && !name.contains("-mosaic-") {
                names.push(name);
            }
        }
    }
    names.sort_by(|a, b| {
        let key = |n: &str| {
            n.trim_end_matches(".mp4")
                .rsplit('-')
                .next()
                .and_then(|x| x.parse::<u32>().ok())
                .unwrap_or(0)
        };
        key(a).cmp(&key(b)).then(a.cmp(b))
    });
    if names.is_empty() {
        return Ok(0);
    }
    let path = project.join("src").join(format!("{episode}.md"));
    let content = std::fs::read_to_string(&path)?;
    let block = video_carousel_block(episode, &names, s);
    let updated = if let (Some(a), Some(b)) = (content.find("<!-- VIDEO_STRIP_START -->"), content.find("<!-- VIDEO_STRIP_END -->")) {
        let end = b + "<!-- VIDEO_STRIP_END -->".len();
        format!("{}{}{}", &content[..a], block, &content[end..])
    } else if let Some(nl) = content.find('\n') {
        // Directly under the H1, as in deepDive.
        format!("{}\n\n{}\n{}", content[..nl].trim_end(), block, &content[nl..])
    } else {
        format!("{content}\n\n{block}\n")
    };
    if updated != content {
        std::fs::write(&path, updated)?;
    }
    Ok(names.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_save_load_round_trips_and_reject_empty() {
        let d = tempfile::tempdir().unwrap();
        let mut s = Syndication::default();
        s.lightning_address = "  me@example.com  ".into();
        save_syndication(d.path(), &s).unwrap();
        let loaded = load_syndication(d.path());
        assert_eq!(loaded.lightning_address, "me@example.com");
        let empty = Syndication { lightning_address: String::new(), spotify_url: String::new(), apple_podcasts_url: String::new(), fountain_url: String::new(), tiktok_url: String::new(), instagram_url: String::new(), youtube_url: String::new() };
        assert!(save_syndication(d.path(), &empty).is_err());
    }

    #[test]
    fn wallet_and_audio_links_go_above_works_cited_and_are_idempotent() {
        let s = Syndication::default();
        let md = "# 1 : Topic\n\nBody.\n\n## Works Cited\n\n[^1]: x\n";
        let out = ensure_syndication_and_wallet_blocks(md, &s);
        assert!(out.contains("lightning-widget"), "{out}");
        assert!(out.contains("Tips and Donations"));
        assert!(out.contains("Spotify") && out.contains("Apple Podcasts") && out.contains("Fountain.fm"));
        assert!(out.find("Tips and Donations").unwrap() < out.find("## Works Cited").unwrap());
        assert_eq!(ensure_syndication_and_wallet_blocks(&out, &s), out);
    }

    #[test]
    fn carousel_only_lists_real_clips_and_replaces_itself() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("src")).unwrap();
        std::fs::write(d.path().join("src/7.md"), "# 7 : Topic\n\nBody.\n").unwrap();
        let s = Syndication::default();
        // No clips yet: no carousel, file untouched (mine-time state).
        assert_eq!(sync_video_carousel(d.path(), 7, &s).unwrap(), 0);
        assert!(!std::fs::read_to_string(d.path().join("src/7.md")).unwrap().contains("VIDEO_STRIP"));
        let clips = crate::bundle::episode_dir(d.path(), 7).join("clips");
        std::fs::create_dir_all(&clips).unwrap();
        std::fs::write(clips.join("7-2.mp4"), b"x").unwrap();
        std::fs::write(clips.join("7-1.mp4"), b"x").unwrap();
        std::fs::write(clips.join("7-1-original.mp4"), b"x").unwrap();
        assert_eq!(sync_video_carousel(d.path(), 7, &s).unwrap(), 2);
        let md = std::fs::read_to_string(d.path().join("src/7.md")).unwrap();
        assert!(md.contains("../episodes/7/clips/7-1.mp4"), "{md}");
        assert!(md.find("7-1.mp4").unwrap() < md.find("7-2.mp4").unwrap());
        assert!(!md.contains("-original"));
        assert!(md.contains("TikTok") && md.contains("Instagram") && md.contains("YouTube"));
        // Re-sync replaces, never duplicates.
        assert_eq!(sync_video_carousel(d.path(), 7, &s).unwrap(), 2);
        let md2 = std::fs::read_to_string(d.path().join("src/7.md")).unwrap();
        assert_eq!(md2.matches("VIDEO_STRIP_START").count(), 1);
    }
}
