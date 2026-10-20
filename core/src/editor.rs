//! External editor launch for Ledger "Edit".
//!
//! Mirrors deepDive's cockpit (`external_editor`, default `gvim`):
//! the user's preferred editor is tried first, then gvim, VS Code,
//! nvim, vim, and finally Notepad (Windows) / the system opener.
//! A full vim mode inside the app is deliberately not built; the
//! user's real editor, with their config, is the better tool.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Result;

/// Ordered, de-duplicated editor candidates for a preference.
/// `"default"` / empty means: no preference, use the fallback chain.
pub fn editor_candidates(preference: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let pref = preference.trim();
    if !pref.is_empty() && !pref.eq_ignore_ascii_case("default") {
        out.push(pref.to_string());
    }
    for ed in ["gvim", "code", "nvim", "vim", "notepad"] {
        if !out.iter().any(|e| e.eq_ignore_ascii_case(ed)) {
            out.push(ed.to_string());
        }
    }
    out
}

fn which(name: &str) -> Option<PathBuf> {
    // A preference may be a full path.
    let p = Path::new(name);
    if p.components().count() > 1 || name.contains(['/', '\\']) {
        return if p.exists() { Some(p.to_path_buf()) } else { None };
    }
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".bat", ".cmd", ""] } else { &[""] };
    for dir in std::env::var_os("PATH").map(|v| std::env::split_paths(&v).collect::<Vec<_>>()).unwrap_or_default() {
        for ext in exts {
            let cand = dir.join(format!("{name}{ext}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    // Windows per-user Vim install (deepDive does the same lookup).
    if cfg!(windows) && name.eq_ignore_ascii_case("gvim") {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let cand = PathBuf::from(local).join("Programs").join("Vim").join("gvim.exe");
            if cand.exists() {
                return Some(cand);
            }
        }
    }
    None
}

/// First candidate that actually exists on this machine.
pub fn resolve_editor(preference: &str) -> Option<(String, PathBuf)> {
    editor_candidates(preference)
        .into_iter()
        .find_map(|name| which(&name).map(|path| (name, path)))
}

/// Open `file` in the resolved editor. Returns which editor was used.
pub fn open_in_editor(file: &Path, preference: &str) -> Result<String> {
    if !file.exists() {
        return crate::msg(format!("{} does not exist", file.display()));
    }
    let tried = editor_candidates(preference);
    match resolve_editor(preference) {
        Some((name, path)) => {
            Command::new(&path)
                .arg(file)
                .spawn()
                .map_err(|e| crate::CoreError::Msg(format!("could not launch {name} ({path:?}): {e}")))?;
            Ok(name)
        }
        None => crate::msg(format!(
            "no editor found (tried {}). Install gVim or set your editor in the Ledger.",
            tried.join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_first_then_gvim_chain() {
        assert_eq!(editor_candidates("notepad"), vec!["notepad", "gvim", "code", "nvim", "vim"]);
        assert_eq!(editor_candidates("gvim"), vec!["gvim", "code", "nvim", "vim", "notepad"]);
        assert_eq!(editor_candidates(""), vec!["gvim", "code", "nvim", "vim", "notepad"]);
        assert_eq!(editor_candidates("default"), editor_candidates(""));
        assert_eq!(editor_candidates("  code "), vec!["code", "gvim", "nvim", "vim", "notepad"]);
    }

    #[test]
    fn open_missing_file_errors() {
        assert!(open_in_editor(Path::new("/no/such/file.md"), "gvim").is_err());
    }
}
