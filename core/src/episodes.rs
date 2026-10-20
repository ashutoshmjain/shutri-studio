//! Episode lifecycle: mempool drafts (`_slug.md`) -> mined numbered episodes.
//! Episode number is the master key; an episode with clips is never renumbered.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Episode {
    /// `None` while in the mempool.
    pub number: Option<u32>,
    pub slug: String,
    pub title: String,
    pub path: PathBuf,
    pub stage: Stage,
    pub has_clips: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Mempool,
    Template,
    /// Locked into the chain: sealed, permanent, immutable.
    Chain,
}

/// Marker written into a numbered article when it is locked.
pub const LOCKED_MARKER: &str = "<!-- studio:locked -->";

fn title_of(text: &str, fallback: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| {
            // Strip leading "NNN :" / "NNN -" numbering.
            let t = t.trim();
            match t.split_once(|c| c == ':' || c == '-') {
                Some((n, rest)) if n.trim().chars().all(|c| c.is_ascii_digit()) => {
                    rest.trim().to_string()
                }
                _ => t.to_string(),
            }
        })
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub fn list_episodes(project: &Path) -> Result<Vec<Episode>> {
    let src = project.join("src");
    let mut out = Vec::new();
    if !src.is_dir() {
        return Ok(out);
    }
    for e in std::fs::read_dir(&src)? {
        let p = e?.path();
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !name.ends_with(".md") || name == "SUMMARY.md" {
            continue;
        }
        let stem = name.trim_end_matches(".md").to_string();
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        if let Some(slug) = stem.strip_prefix('_') {
            out.push(Episode {
                number: None,
                slug: slug.to_string(),
                title: title_of(&text, slug),
                has_clips: false,
                path: p,
                stage: Stage::Mempool,
            });
        } else if let Ok(n) = stem.parse::<u32>() {
            let locked = text.contains(LOCKED_MARKER);
            out.push(Episode {
                number: Some(n),
                slug: stem.clone(),
                title: title_of(&text, &stem),
                has_clips: project
                    .join("episodes")
                    .join(&stem)
                    .join("plan.json")
                    .exists(),
                path: p,
                stage: if locked { Stage::Chain } else { Stage::Template },
            });
        }
    }
    out.sort_by_key(|e| e.number.unwrap_or(0));
    Ok(out)
}

/// Read one episode/draft file from `src/` for the in-app reader.
/// Only a bare `.md` file name is accepted — never a path outside src/.
pub fn read_episode(project: &Path, file_name: &str) -> Result<String> {
    if file_name.contains(['/', '\\']) || !file_name.ends_with(".md") {
        return crate::msg("episode file must be a .md file name inside src/");
    }
    Ok(std::fs::read_to_string(project.join("src").join(file_name))?)
}

pub fn next_number(project: &Path) -> Result<u32> {
    Ok(list_episodes(project)?
        .iter()
        .filter_map(|e| e.number)
        .max()
        .unwrap_or(0)
        + 1)
}

pub fn create_draft(project: &Path, slug: &str, title: &str, body: &str) -> Result<Episode> {
    let slug = slug.trim().trim_start_matches('_');
    if slug.is_empty() || slug.chars().any(|c| !(c.is_alphanumeric() || c == '-' || c == '_')) {
        return crate::msg("slug must be alphanumeric, '-' or '_'");
    }
    let path = project.join("src").join(format!("_{slug}.md"));
    if path.exists() {
        return crate::msg(format!("draft _{slug}.md already exists"));
    }
    std::fs::create_dir_all(project.join("src"))?;
    std::fs::write(&path, format!("# {title}\n\n{body}\n"))?;
    sync_summary(project)?;
    list_episodes(project)?
        .into_iter()
        .find(|e| e.slug == slug && e.number.is_none())
        .ok_or_else(|| CoreError::Msg("draft vanished after create".into()))
}

/// Mine a mempool draft into the template: assign the next episode
/// number, rename, add the donation wallet + audio platform links
/// (see `syndication`), and update SUMMARY. The video carousel is
/// NOT added here — it arrives later, once clips exist.
/// Refuses to touch an episode that already has clips.
pub fn promote(project: &Path, slug: &str) -> Result<Episode> {
    let eps = list_episodes(project)?;
    let draft = eps
        .iter()
        .find(|e| e.slug == slug && e.number.is_none())
        .ok_or_else(|| CoreError::Msg(format!("no mempool draft _{slug}")))?;
    if draft.has_clips {
        return crate::msg("episode with clips must not be renumbered");
    }
    let n = next_number(project)?;
    let dest = project.join("src").join(format!("{n}.md"));
    let mut text = std::fs::read_to_string(&draft.path)?;
    // Rewrite the H1 with the number.
    if text.starts_with("# ") {
        let rest = text.splitn(2, '\n').nth(1).unwrap_or("");
        text = format!("# {n} : {}\n{rest}", draft.title);
    } else {
        text = format!("# {n} : {}\n\n{text}", draft.title);
    }
    let syndication = crate::syndication::load_syndication(project);
    let text = crate::syndication::ensure_syndication_and_wallet_blocks(&text, &syndication);
    std::fs::write(&dest, text)?;
    std::fs::remove_file(&draft.path)?;
    sync_summary(project)?;
    list_episodes(project)?
        .into_iter()
        .find(|e| e.number == Some(n))
        .ok_or_else(|| CoreError::Msg("episode vanished after promote".into()))
}

/// Lock a template episode into the chain: seal it in place
/// (its file and permanent URL do not move), mark it locked, and
/// list it under `chain` in SUMMARY. Locked episodes are immutable:
/// deletion is refused. Only template episodes can be locked.
pub fn lock_episode(project: &Path, number: u32) -> Result<Episode> {
    let eps = list_episodes(project)?;
    let ep = eps
        .iter()
        .find(|e| e.number == Some(number))
        .ok_or_else(|| CoreError::Msg(format!("no episode {number}")))?;
    match ep.stage {
        Stage::Chain => return crate::msg(format!("episode {number} is already locked in the chain")),
        Stage::Mempool => return crate::msg("only a mined (template) episode can be locked"),
        Stage::Template => {}
    }
    let mut text = std::fs::read_to_string(&ep.path)?;
    if !text.contains(LOCKED_MARKER) {
        text = format!("{}\n{}\n", text.trim_end(), LOCKED_MARKER);
        std::fs::write(&ep.path, text)?;
    }
    sync_summary(project)?;
    list_episodes(project)?
        .into_iter()
        .find(|e| e.number == Some(number))
        .ok_or_else(|| CoreError::Msg("episode vanished after lock".into()))
}

/// Regenerate the managed ledger block of SUMMARY.md from disk.
/// Everything outside `<!-- studio:episodes -->` … `<!-- /studio:episodes -->`
/// (cover link, hand-written notes, sealed chain blocks) is preserved verbatim.
pub fn sync_summary(project: &Path) -> Result<()> {
    let eps = list_episodes(project)?;
    let path = project.join("src/SUMMARY.md");
    let existing = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| "# Summary\n\n- [cover](cover.md)\n".into());
    let (head, tail) = match existing.split_once("<!-- studio:episodes -->") {
        Some((h, rest)) => {
            let t = rest
                .split_once("<!-- /studio:episodes -->")
                .map(|(_, after)| after.trim_start_matches('\n').to_string())
                .unwrap_or_default();
            (h.trim_end().to_string(), t)
        }
        None => (existing.trim_end().to_string(), String::new()),
    };
    let mut block = String::from("<!-- studio:episodes -->\n");
    block.push_str("- [mempool](mempool.md)\n");
    for e in eps.iter().filter(|e| e.number.is_none()) {
        block.push_str(&format!("    - [{}](_{}.md)\n", e.title, e.slug));
    }
    block.push_str("- [template](template.md)\n");
    for e in eps.iter().filter(|e| e.stage == Stage::Template) {
        block.push_str(&format!(
            "    - [{} : {}]({}.md)\n",
            e.number.unwrap(),
            e.title,
            e.number.unwrap()
        ));
    }
    block.push_str("- [chain](chain.md)\n");
    for e in eps.iter().filter(|e| e.stage == Stage::Chain) {
        block.push_str(&format!(
            "    - [{} : {}]({}.md)\n",
            e.number.unwrap(),
            e.title,
            e.number.unwrap()
        ));
    }
    block.push_str("<!-- /studio:episodes -->\n");
    let mut out = format!("{head}\n\n{block}");
    if !tail.is_empty() {
        out.push('\n');
        out.push_str(&tail);
    }
    std::fs::write(&path, out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("src")).unwrap();
        d
    }

    #[test]
    fn draft_then_promote() {
        let d = proj();
        create_draft(d.path(), "quantum-memory", "Quantum Memory", "body").unwrap();
        let eps = list_episodes(d.path()).unwrap();
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].number, None);
        let ep = promote(d.path(), "quantum-memory").unwrap();
        assert_eq!(ep.number, Some(1));
        assert!(d.path().join("src/1.md").exists());
        let summary = std::fs::read_to_string(d.path().join("src/SUMMARY.md")).unwrap();
        assert!(summary.contains("    - [1 : Quantum Memory](1.md)"), "{summary}");
        assert!(summary.contains("- [template](template.md)"), "{summary}");
        assert!(!summary.contains("_quantum-memory.md"), "{summary}");
    }

    #[test]
    fn mine_adds_wallet_and_audio_links_but_no_carousel() {
        let d = proj();
        create_draft(d.path(), "topic", "Topic", "Body.\n\n## Works Cited\n\n[^1]: x").unwrap();
        let ep = promote(d.path(), "topic").unwrap();
        assert_eq!(ep.number, Some(1));
        assert_eq!(ep.stage, Stage::Template);
        let md = std::fs::read_to_string(d.path().join("src/1.md")).unwrap();
        assert!(md.contains("lightning-widget"), "{md}");
        assert!(md.contains("Spotify") && md.contains("Fountain.fm"), "{md}");
        assert!(!md.contains("VIDEO_STRIP"), "carousel waits for clips: {md}");
    }

    #[test]
    fn lock_moves_template_to_chain_and_seals_it() {
        let d = proj();
        create_draft(d.path(), "topic", "Topic", "body").unwrap();
        let ep = promote(d.path(), "topic").unwrap();
        let n = ep.number.unwrap();
        let locked = lock_episode(d.path(), n).unwrap();
        assert_eq!(locked.stage, Stage::Chain);
        let summary = std::fs::read_to_string(d.path().join("src/SUMMARY.md")).unwrap();
        let chain_at = summary.find("- [chain](chain.md)").unwrap();
        assert!(summary[chain_at..].contains(&format!("({n}.md)")), "{summary}");
        assert!(lock_episode(d.path(), n).is_err(), "re-lock refused");
        assert!(delete_episode(d.path(), &format!("{n}.md")).is_err(), "chain is immutable");
    }

    #[test]
    fn delete_mempool_and_template_but_not_clipped() {
        let d = proj();
        let ep = create_draft(d.path(), "to-delete", "To Delete", "body").unwrap();
        assert_eq!(delete_episode(d.path(), "_to-delete.md").unwrap(), "To Delete");
        assert!(!d.path().join("src/_to-delete.md").exists());
        let summary = std::fs::read_to_string(d.path().join("src/SUMMARY.md")).unwrap();
        assert!(!summary.contains("to-delete.md"));
        let mined = promote(d.path(), "quantum-memory").unwrap_or_else(|_| {
            create_draft(d.path(), "qm2", "QM Two", "b").unwrap();
            promote(d.path(), "qm2").unwrap()
        });
        let fname = format!("{}.md", mined.number.unwrap());
        assert!(delete_episode(d.path(), &fname).is_ok());
        let _ = ep;
    }

    #[test]
    fn read_episode_stays_inside_src() {
        let d = proj();
        std::fs::write(d.path().join("src/_a.md"), "# A\n\nbody $x^2$\n").unwrap();
        assert!(read_episode(d.path(), "_a.md").unwrap().contains("$x^2$"));
        assert!(read_episode(d.path(), "../SUMMARY.md").is_err());
        assert!(read_episode(d.path(), "a.txt").is_err());
    }

    #[test]
    fn sync_summary_preserves_handwritten_tail() {
        let d = proj();
        std::fs::write(
            d.path().join("src/SUMMARY.md"),
            "# Summary\n\n- [cover](cover.md)\n\n<!-- studio:episodes -->\n- [stale](stale.md)\n<!-- /studio:episodes -->\n\n- [block 1](block1.md)\n",
        )
        .unwrap();
        create_draft(d.path(), "fresh-draft", "Fresh Draft", "body").unwrap();
        let summary = std::fs::read_to_string(d.path().join("src/SUMMARY.md")).unwrap();
        assert!(summary.contains("- [block 1](block1.md)"), "{summary}");
        assert!(summary.contains("    - [Fresh Draft](_fresh-draft.md)"), "{summary}");
        assert!(!summary.contains("stale.md"), "{summary}");
    }
}

/// Delete a mempool draft or a template episode (its article file).
/// Chain content is immutable by construction — sealed episodes are not
/// listed from src/ files this function can reach with clips attached:
/// an episode that already has a plan/clips is refused, never orphaned.
pub fn delete_episode(project: &Path, file_name: &str) -> Result<String> {
    let eps = list_episodes(project)?;
    let ep = eps
        .iter()
        .find(|e| e.path.file_name().map(|n| n.to_string_lossy() == file_name).unwrap_or(false))
        .ok_or_else(|| crate::CoreError::Msg(format!("no such episode file {file_name}")))?;
    if ep.stage == Stage::Chain {
        return crate::msg("this episode is locked in the chain — sealed chain content is immutable and cannot be deleted.");
    }
    if ep.has_clips {
        return crate::msg("this episode already has a clip plan/bundle — refusing to delete it and orphan episodes/<N>/. Remove the bundle first if you really mean it.");
    }
    std::fs::remove_file(&ep.path)?;
    sync_summary(project)?;
    Ok(ep.title.clone())
}
