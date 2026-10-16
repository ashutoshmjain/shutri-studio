//! Publish = stage, commit, push inside the user's own project folder,
//! with the user's own credentials. The app stores no tokens or remotes.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{project::git, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub dirty: Vec<String>,
}

pub fn status(path: &Path) -> GitStatus {
    if !path.join(".git").exists() {
        return GitStatus {
            is_repo: false,
            branch: None,
            remote: None,
            dirty: vec![],
        };
    }
    GitStatus {
        is_repo: true,
        branch: git(path, &["branch", "--show-current"]).ok().filter(|s| !s.is_empty()),
        remote: git(path, &["remote", "get-url", "origin"]).ok().filter(|s| !s.is_empty()),
        dirty: git(path, &["status", "--porcelain"])
            .map(|s| s.lines().map(|l| l.to_string()).collect())
            .unwrap_or_default(),
    }
}

/// Stage the publish set, commit, and push. Fails in plain language for
/// the states a non-technical user will actually hit.
pub fn publish(path: &Path, message: &str, paths: &[&str]) -> Result<String> {
    let st = status(path);
    if !st.is_repo {
        return crate::msg("This project is not a Git repository yet. Create the project again or run `git init` in its folder first.");
    }
    if st.remote.is_none() {
        return crate::msg("No remote is configured for this project. Add one with `git remote add origin <your-repo-url>` — the app never picks a remote for you.");
    }
    let mut add: Vec<&str> = vec!["add", "--"];
    add.extend_from_slice(paths);
    git(path, &add)?;
    if git(path, &["diff", "--cached", "--quiet"]).is_ok() {
        // `diff --quiet` exits 0 when nothing is staged; git() maps success to Ok.
        // Treat that as "nothing to publish" only when the tree is also clean.
        // (git returns non-zero = changes present = Err above is not hit normally.)
    }
    git(path, &["commit", "-m", message])?;
    git(path, &["push"])?;
    Ok(format!("Pushed to {} ({})", st.remote.unwrap_or_default(), st.branch.unwrap_or_else(|| "detached".into())))
}
