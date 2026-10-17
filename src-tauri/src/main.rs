// Shutri Studio — Tauri shell. All logic lives in shutri-core;
// these commands are thin, typed wrappers (no HTTP server, no ports).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use shutri_core::{bundle, channels, episodes, git, jobs::Jobs, nostr, plan, project};

type CmdResult<T> = Result<T, String>;

fn pb(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[tauri::command]
fn inspect_project(path: String) -> CmdResult<project::ProjectInfo> {
    project::inspect(&pb(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_project(path: String, name: String) -> CmdResult<project::ProjectInfo> {
    project::create(&pb(&path), &name, &project::Template::default()).map_err(|e| e.to_string())
}

#[tauri::command]
fn clone_project(url: String, dest: String) -> CmdResult<project::ProjectInfo> {
    project::clone(&url, &pb(&dest)).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_episodes(path: String) -> CmdResult<Vec<episodes::Episode>> {
    episodes::list_episodes(&pb(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_draft(path: String, slug: String, title: String, body: String) -> CmdResult<episodes::Episode> {
    episodes::create_draft(&pb(&path), &slug, &title, &body).map_err(|e| e.to_string())
}

#[tauri::command]
fn promote_episode(path: String, slug: String) -> CmdResult<episodes::Episode> {
    episodes::promote(&pb(&path), &slug).map_err(|e| e.to_string())
}

#[tauri::command]
fn build_book(path: String) -> CmdResult<String> {
    project::build_book(&pb(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn git_status(path: String) -> git::GitStatus {
    git::status(&pb(&path))
}

#[tauri::command]
fn git_publish(path: String, message: String) -> CmdResult<String> {
    git::publish(&pb(&path), &message, &["src/", "episodes/", "book.toml"]).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_channels() -> Vec<channels::Channel> {
    channels::registry()
}

fn plan_path(project: &Path, episode: u32) -> PathBuf {
    shutri_core::bundle::episode_dir(project, episode).join("plan.json")
}

#[tauri::command]
fn load_plan(path: String, episode: u32) -> CmdResult<plan::Plan> {
    let p = plan_path(&pb(&path), episode);
    if !p.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e: serde_json::Error| e.to_string())
}

#[tauri::command]
fn save_plan(path: String, episode: u32, mut plan: plan::Plan) -> CmdResult<String> {
    for c in &mut plan { c.sync_distribution_fields(); }
    let p = plan_path(&pb(&path), episode);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into())
}

/// Publisher preflight for an episode bundle (strategy §6.1/§2): read-only.
#[tauri::command]
fn validate_bundle(path: String, episode: u32) -> CmdResult<bundle::BundleReport> {
    bundle::validate_bundle(&pb(&path), episode, true).map_err(|e| e.to_string())
}

#[tauri::command]
fn plan_from_transcript(text: String, target_secs: f64) -> plan::Plan {
    // Estimate word timing at ~2.5 words/sec when no ASR timing exists yet;
    // real timing comes from the transcription contract (word timestamps).
    let words: Vec<shutri_core::narrative::Word> = text
        .split_whitespace()
        .enumerate()
        .map(|(i, t)| shutri_core::narrative::Word {
            text: t.to_string(),
            start: i as f64 * 0.4,
            end: i as f64 * 0.4 + 0.36,
        })
        .collect();
    plan::plan_from_words(&words, target_secs)
}

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    npub: String,
    pubkey_hex: String,
    ncrypt: String,
}

fn identity_file(project: &Path) -> PathBuf {
    project.join(".studio/identity.json")
}

#[tauri::command]
fn generate_identity(path: String, password: String) -> CmdResult<serde_json::Value> {
    let proj = pb(&path);
    let kp = nostr::generate_keypair().map_err(|e| e.to_string())?;
    let blob = nostr::encrypt_nsec(&kp.nsec, &password).map_err(|e| e.to_string())?;
    let stored = StoredIdentity {
        npub: kp.identity.npub.clone(),
        pubkey_hex: kp.identity.pubkey_hex.clone(),
        ncrypt: blob,
    };
    let file = identity_file(&proj);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, serde_json::to_string_pretty(&stored).unwrap())
        .map_err(|e| e.to_string())?;
    // nsec is returned exactly once, for the backup flow. Never stored raw.
    Ok(serde_json::json!({
        "npub": kp.identity.npub,
        "pubkey_hex": kp.identity.pubkey_hex,
        "nsec_backup": kp.nsec,
    }))
}

#[tauri::command]
fn load_identity(path: String) -> CmdResult<Option<serde_json::Value>> {
    let file = identity_file(&pb(&path));
    if !file.exists() {
        return Ok(None);
    }
    let stored: StoredIdentity =
        serde_json::from_str(&std::fs::read_to_string(&file).map_err(|e| e.to_string())?)
            .map_err(|e: serde_json::Error| e.to_string())?;
    Ok(Some(serde_json::json!({"npub": stored.npub, "pubkey_hex": stored.pubkey_hex})))
}

#[tauri::command]
fn sign_narrative(path: String, password: String, episode: u32, markdown: String) -> CmdResult<nostr::Event> {
    let proj = pb(&path);
    let stored: StoredIdentity = serde_json::from_str(
        &std::fs::read_to_string(identity_file(&proj)).map_err(|e| e.to_string())?,
    )
    .map_err(|e: serde_json::Error| e.to_string())?;
    let kp = nostr::decrypt_nsec(&stored.ncrypt, &password).map_err(|e| e.to_string())?;
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    nostr::sign_event(
        &kp,
        30023,
        vec![
            vec!["d".into(), episode.to_string()],
            vec!["title".into(), format!("Episode {episode}")],
        ],
        &markdown,
        created,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_jobs(state: tauri::State<'_, Jobs>) -> Vec<shutri_core::jobs::Job> {
    state.list()
}

fn main() {
    tauri::Builder::default()
        .manage(Jobs::new())
        .invoke_handler(tauri::generate_handler![
            inspect_project,
            create_project,
            clone_project,
            list_episodes,
            create_draft,
            promote_episode,
            build_book,
            git_status,
            git_publish,
            list_channels,
            load_plan,
            save_plan,
            validate_bundle,
            plan_from_transcript,
            generate_identity,
            load_identity,
            sign_narrative,
            list_jobs
        ])
        .run(tauri::generate_context!())
        .expect("error while running Shutri Studio");
}
