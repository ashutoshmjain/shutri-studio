//! User project: open / clone / create. The app never targets deepDive
//! on a user's behalf; deepDive is only the author's own project instance.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: PathBuf,
    pub is_git_repo: bool,
    pub remote: Option<String>,
    pub episode_count: usize,
    pub mempool_count: usize,
}

pub fn inspect(path: &Path) -> Result<ProjectInfo> {
    let eps = crate::episodes::list_episodes(path)?;
    Ok(ProjectInfo {
        path: path.to_path_buf(),
        is_git_repo: path.join(".git").exists(),
        remote: git_remote(path),
        episode_count: eps.iter().filter(|e| e.number.is_some()).count(),
        mempool_count: eps.iter().filter(|e| e.number.is_none()).count(),
    })
}

fn git_remote(path: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

/// Create a fresh Studio project: a self-contained mdBook ledger with no
/// legacy structure. Layout:
/// `book.toml`, `src/{SUMMARY,cover,mempool,template,chain}.md`,
/// `src/_<slug>.md` mempool drafts, `src/<N>.md` mined episodes,
/// `episodes/<N>/` media bundles, `.studio/` local app state (git-ignored).
pub fn create(path: &Path, name: &str, template: &Template) -> Result<ProjectInfo> {
    if path.join("src").exists() {
        return crate::msg("target already contains a src/ directory");
    }
    std::fs::create_dir_all(path.join("src"))?;
    std::fs::create_dir_all(path.join("episodes"))?;
    std::fs::create_dir_all(path.join(".studio"))?;
    std::fs::write(path.join("book.toml"), template.book_toml.replace("{name}", name))?;
    std::fs::create_dir_all(path.join("theme"))?;
    std::fs::write(
        path.join("theme/studio.css"),
        "/* Shutri Studio: mdBook's automatic section numbers (1., 2.1., 3.1.)\n   duplicate the ledger's own episode numbering — hide them. */\n.chapter-item > a > strong[aria-hidden=\"true\"],\n.chapter-item > div > strong[aria-hidden=\"true\"] { display: none; }\n",
    )?;
    std::fs::write(path.join(".gitignore"), "book/\n.studio/\n")?;
    std::fs::write(
        path.join("README.md"),
        format!(
            "# {name}\n\nA Shutri Studio project: LLM research published as an mdBook, \
             narrated audio, short clips and channel posts. Drafts live in the mempool \
             (`src/_<slug>.md`); mined episodes are `src/<N>.md` with media in \
             `episodes/<N>/`. Build the book with `mdbook build`.\n"
        ),
    )?;
    std::fs::write(
        path.join("src/cover.md"),
        format!("# {name}\n\nResearch, published whole. Drafts wait in the mempool; \
                 finished episodes are mined into the template and sealed into the chain, \
                 21 episodes per block.\n"),
    )?;
    std::fs::write(
        path.join("src/mempool.md"),
        "# mempool\n\nUnmined research drafts (`_<slug>.md`). Publish research here \
         as-is while the audio is still being made. Nothing in the mempool has a \
         permanent URL yet.\n",
    )?;
    std::fs::write(
        path.join("src/template.md"),
        "# template\n\nMined, numbered episodes with permanent URLs. The block holds \
         at most 21 episodes (the Law of 21); a full block is sealed into the chain.\n",
    )?;
    std::fs::write(
        path.join("src/chain.md"),
        "# chain\n\nSealed blocks of 21 episodes. Permanent and immutable.\n",
    )?;
    std::fs::write(
        path.join("src/SUMMARY.md"),
        "# Summary\n\n- [cover](cover.md)\n",
    )?;
    std::fs::write(
        path.join("src/_welcome.md"),
        format!("# Welcome\n\nYour first mempool draft in {name}. Mine it to assign episode 1.\n"),
    )?;
    crate::episodes::sync_summary(path)?;
    git(path, &["init"])?;
    inspect(path)
}

#[derive(Debug, Clone)]
pub struct Template {
    pub book_toml: String,
}

impl Default for Template {
    fn default() -> Self {
        Self {
            book_toml: "[book]\ntitle = \"{name}\"\nlanguage = \"en\"\nsrc = \"src\"\n\n[preprocessor.katex]\n\n[output.html]\nadditional-css = [\"theme/studio.css\"]\n".into(),
        }
    }
}

/// Clone the user's own remote into `dest`.
pub fn clone(url: &str, dest: &Path) -> Result<ProjectInfo> {
    if url.trim().is_empty() {
        return crate::msg("remote URL is empty");
    }
    if dest.exists() && std::fs::read_dir(dest).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return crate::msg("clone destination is not empty");
    }
    let out = Command::new("git")
        .args(["clone", url, &dest.to_string_lossy()])
        .output()?;
    if !out.status.success() {
        return crate::msg(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    inspect(dest)
}

pub fn git(path: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").args(args).current_dir(path).output()?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        crate::msg(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub fn build_book(path: &Path) -> Result<String> {
    let out = Command::new("mdbook")
        .args(["build"])
        .current_dir(path)
        .output()?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() {
        Ok(log)
    } else {
        crate::msg(format!("mdbook build failed: {log}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_project() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("MyResearch");
        let info = create(&p, "MyResearch", &Template::default()).unwrap();
        assert_eq!(info.mempool_count, 1);
        assert!(info.is_git_repo);
        // Fresh-repo layout: no legacy (deepDive/ddma) structure anywhere.
        for f in [
            "book.toml",
            "README.md",
            ".gitignore",
            "src/cover.md",
            "src/mempool.md",
            "src/template.md",
            "src/chain.md",
            "src/_welcome.md",
            "theme/studio.css",
        ] {
            assert!(p.join(f).exists(), "missing {f}");
        }
        assert!(p.join("episodes").is_dir());
        assert!(!p.join("src/ddma").exists());
        assert!(std::fs::read_to_string(p.join("book.toml")).unwrap().contains("additional-css"));
        let summary = std::fs::read_to_string(p.join("src/SUMMARY.md")).unwrap();
        assert!(summary.contains("- [cover](cover.md)"), "{summary}");
        assert!(summary.contains("- [mempool](mempool.md)"), "{summary}");
        assert!(summary.contains("    - [Welcome](_welcome.md)"), "{summary}");
        assert!(summary.contains("- [template](template.md)"), "{summary}");
        assert!(summary.contains("- [chain](chain.md)"), "{summary}");
    }
}
