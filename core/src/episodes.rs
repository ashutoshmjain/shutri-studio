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
}

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
            out.push(Episode {
                number: Some(n),
                slug: stem.clone(),
                title: title_of(&text, &stem),
                has_clips: project
                    .join("src/ddma/docs/episodes")
                    .join(&stem)
                    .join("plan.json")
                    .exists(),
                path: p,
                stage: Stage::Template,
            });
        }
    }
    out.sort_by_key(|e| e.number.unwrap_or(0));
    Ok(out)
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

/// Mine a mempool draft: assign the next number, rename, update SUMMARY.
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
    std::fs::write(&dest, text)?;
    std::fs::remove_file(&draft.path)?;
    sync_summary(project)?;
    list_episodes(project)?
        .into_iter()
        .find(|e| e.number == Some(n))
        .ok_or_else(|| CoreError::Msg("episode vanished after promote".into()))
}

/// Regenerate the mempool/template sections of SUMMARY.md from disk.
/// Deliberately simple and deterministic; hand-curated sections are kept.
pub fn sync_summary(project: &Path) -> Result<()> {
    let eps = list_episodes(project)?;
    let path = project.join("src/SUMMARY.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_else(|_| "# Summary\n".into());
    // Keep everything before our managed block, regenerate the block.
    let head = existing
        .split("<!-- studio:episodes -->")
        .next()
        .unwrap_or("")
        .trim_end()
        .to_string();
    let mut block = String::from("<!-- studio:episodes -->\n");
    block.push_str("\n# The Mempool (Unconfirmed)\n");
    for e in eps.iter().filter(|e| e.number.is_none()) {
        block.push_str(&format!("- [{}](_{}.md)\n", e.title, e.slug));
    }
    block.push_str("\n# Recent\n");
    for e in eps.iter().filter(|e| e.number.is_some()) {
        block.push_str(&format!("- [{} : {}]({}.md)\n", e.number.unwrap(), e.title, e.number.unwrap()));
    }
    block.push_str("\n<!-- /studio:episodes -->\n");
    std::fs::write(&path, format!("{head}\n\n{block}"))?;
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
        assert!(summary.contains("1.md"));
    }
}
