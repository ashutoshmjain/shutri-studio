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

/// Create a new project from the bundled template text.
pub fn create(path: &Path, name: &str, template: &Template) -> Result<ProjectInfo> {
    if path.join("src").exists() {
        return crate::msg("target already contains a src/ directory");
    }
    std::fs::create_dir_all(path.join("src"))?;
    std::fs::create_dir_all(path.join(".studio"))?;
    std::fs::write(path.join("book.toml"), template.book_toml.replace("{name}", name))?;
    std::fs::write(path.join("src/SUMMARY.md"), "# Summary\n")?;
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
            book_toml: "[book]\ntitle = \"{name}\"\nlanguage = \"en\"\nsrc = \"src\"\n\n[preprocessor.katex]\n\n[output.html]\n".into(),
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
    }
}
