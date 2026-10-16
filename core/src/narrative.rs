//! The narrative is the canonical artifact: a conversation with speakers
//! and word-level timing, from which article, audio and clips derive.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Turn {
    pub speaker: String,
    pub text: String,
    #[serde(default)]
    pub words: Vec<Word>,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Narrative {
    pub title: String,
    #[serde(default)]
    pub turns: Vec<Turn>,
}

impl Narrative {
    /// Plain transcript text (speaker turns joined).
    pub fn transcript(&self) -> String {
        self.turns
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Render as a Markdown conversation for the article / Nostr narrative.
    pub fn to_markdown(&self) -> String {
        let mut out = format!("# {}\n", self.title);
        for t in &self.turns {
            out.push_str(&format!("\n**{}:** {}\n", t.speaker, t.text));
        }
        out
    }

    /// All words flattened in order, for clip planning.
    pub fn words(&self) -> Vec<&Word> {
        self.turns.iter().flat_map(|t| t.words.iter()).collect()
    }
}

/// Split text into sentences (keeps terminal punctuation).
pub fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let s = cur.trim().to_string();
            if !s.is_empty() {
                out.push(s);
            }
            cur = String::new();
        }
    }
    let s = cur.trim().to_string();
    if !s.is_empty() {
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_and_sentences() {
        let n = Narrative {
            title: "T".into(),
            turns: vec![Turn {
                speaker: "Host".into(),
                text: "Hello world. Again?".into(),
                words: vec![],
                start: 0.0,
                end: 1.0,
            }],
        };
        assert!(n.to_markdown().contains("**Host:** Hello"));
        assert_eq!(sentences(&n.transcript()).len(), 2);
    }
}
