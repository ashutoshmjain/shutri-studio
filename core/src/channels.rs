//! Channels are adapters over the episode bundle. Building the interface
//! now; all integrations except mdBook stay on hold until the core is solid.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Web,       // mdBook (KaTeX), Git-published
    Narrative, // Nostr NIP-23 (primary), LinkedIn, Substack
    Reels,     // Instagram, TikTok, YouTube Shorts (9:16 from 740x740)
    Audio,     // podcast hosts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub kind: ChannelKind,
    /// `live` = wired and usable; `on_hold` = interface only, by design.
    pub status: String,
}

pub fn registry() -> Vec<Channel> {
    vec![
        Channel { id: "mdbook".into(), name: "Web (mdBook)".into(), kind: ChannelKind::Web, status: "live".into() },
        Channel { id: "nostr".into(), name: "Nostr long-form (NIP-23)".into(), kind: ChannelKind::Narrative, status: "on_hold".into() },
        Channel { id: "linkedin".into(), name: "LinkedIn".into(), kind: ChannelKind::Narrative, status: "on_hold".into() },
        Channel { id: "substack".into(), name: "Substack".into(), kind: ChannelKind::Narrative, status: "on_hold".into() },
        Channel { id: "instagram".into(), name: "Instagram Reels".into(), kind: ChannelKind::Reels, status: "on_hold".into() },
        Channel { id: "tiktok".into(), name: "TikTok".into(), kind: ChannelKind::Reels, status: "on_hold".into() },
        Channel { id: "youtube".into(), name: "YouTube Shorts".into(), kind: ChannelKind::Reels, status: "on_hold".into() },
        Channel { id: "podcast".into(), name: "Podcast (RSS)".into(), kind: ChannelKind::Audio, status: "on_hold".into() },
    ]
}

/// A per-episode, per-channel publish receipt (written under syndication/).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub episode: u32,
    pub channel: String,
    pub status: String,
    pub reference: Option<String>,
    pub at_unix: u64,
}
