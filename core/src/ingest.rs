//! Ingest the LLM packaging payload: a self-extracting Python script whose
//! `raw_markdown` string holds the complete research (Master Packaging
//! Prompt). Markdown/base64 exports mangle KaTeX; the Python payload is the
//! one transport that survives byte-for-byte.

use std::path::Path;

use crate::episodes::{create_draft, Episode};
use crate::Result;

/// Pull the `raw_markdown` triple-quoted string out of a payload script.
/// Accepts `"""` or `'''`, with an optional `r` prefix on the literal.
pub fn extract_raw_markdown(source: &str) -> Result<String> {
    let assign = source
        .find("raw_markdown")
        .ok_or_else(|| crate::CoreError::Msg("no raw_markdown found — is this a packaging payload .py?".into()))?;
    let rest = &source[assign..];
    let (delim, start) = ["\"\"\"", "'''"]
        .iter()
        .filter_map(|d| rest.find(d).map(|i| (*d, i)))
        .min_by_key(|(_, i)| *i)
        .ok_or_else(|| crate::CoreError::Msg("raw_markdown is not a triple-quoted string".into()))?;
    let body_start = start + delim.len();
    let body = &rest[body_start..];
    let end = body
        .find(delim)
        .ok_or_else(|| crate::CoreError::Msg("raw_markdown string is never closed".into()))?;
    let text = body[..end].trim_matches('\n').to_string();
    if text.trim().is_empty() {
        return crate::msg("raw_markdown is empty");
    }
    Ok(text)
}

fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').chars().take(60).collect::<String>().trim_matches('-').to_string()
}

/// Ingest a payload file into the mempool: extract, take the H1 as the
/// title, derive the slug, and create `_slug.md` (SUMMARY synced).
pub fn ingest_python_script(project: &Path, payload_path: &Path) -> Result<Episode> {
    let source = std::fs::read_to_string(payload_path)?;
    let markdown = extract_raw_markdown(&source)?;
    let title = markdown
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| crate::CoreError::Msg("payload research has no '# Title' H1 line".into()))?;
    let slug = slugify(&title);
    if slug.is_empty() {
        return crate::msg("could not derive a slug from the payload title");
    }
    // create_draft re-adds the H1, so pass the body after it.
    let body = markdown
        .split_once('\n')
        .map(|(_, rest)| rest.trim_start_matches('\n').to_string())
        .unwrap_or_default();
    create_draft(project, &slug, &title, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = "raw_markdown = \"\"\"\n# Quantum Memory Limits\n\nInline $E = mc^2$ and block:\n\n$$\\int_0^1 x^2 dx$$\n\nCost is \\$100M.\n\n## Works Cited\n\n[^1]: [Doe, Paper, 2026](https://example.com)\n\"\"\"\n\nif __name__ == '__main__':\n    print('x')\n";

    #[test]
    fn extracts_katex_verbatim() {
        let md = extract_raw_markdown(PAYLOAD).unwrap();
        assert!(md.contains("$E = mc^2$"));
        assert!(md.contains("$$\\int_0^1 x^2 dx$$"));
        assert!(md.contains("\\$100M"));
    }

    #[test]
    fn rejects_non_payload() {
        assert!(extract_raw_markdown("print('hello')").is_err());
        assert!(extract_raw_markdown("raw_markdown = 'short'").is_err());
    }

    #[test]
    fn ingests_into_mempool() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("src")).unwrap();
        let payload = d.path().join("payload.py");
        std::fs::write(&payload, PAYLOAD).unwrap();
        let ep = ingest_python_script(d.path(), &payload).unwrap();
        assert_eq!(ep.number, None);
        assert_eq!(ep.slug, "quantum-memory-limits");
        assert_eq!(ep.title, "Quantum Memory Limits");
        let draft = std::fs::read_to_string(d.path().join("src/_quantum-memory-limits.md")).unwrap();
        assert!(draft.starts_with("# Quantum Memory Limits"));
        assert!(draft.contains("$$\\int_0^1 x^2 dx$$"));
        let summary = std::fs::read_to_string(d.path().join("src/SUMMARY.md")).unwrap();
        assert!(summary.contains("_quantum-memory-limits.md"));
    }
}
