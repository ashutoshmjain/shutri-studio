// Shutri Studio — Tauri shell. All logic lives in shutri-core;
// these commands are thin, typed wrappers (no HTTP server, no ports).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use shutri_core::{bundle, channels, editor, episodes, git, ingest, jobs::Jobs, nostr, plan, project, syndication, transcribe};

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
fn ingest_python(path: String, source: String) -> CmdResult<episodes::Episode> {
    ingest::ingest_python_script(&pb(&path), &pb(&source)).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_episode(path: String, file_name: String) -> CmdResult<String> {
    episodes::delete_episode(&pb(&path), &file_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn ingest_markdown(path: String, source: String) -> CmdResult<episodes::Episode> {
    ingest::ingest_markdown_file(&pb(&path), &pb(&source)).map_err(|e| e.to_string())
}

#[tauri::command]
fn upload_audio(path: String, episode: u32, source: String) -> CmdResult<String> {
    transcribe::copy_audio(&pb(&path), episode, &pb(&source))
        .map(|d| d.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Transcribe with sherpa-onnx by default (word-level timestamps,
/// models auto-detected — no paths asked in the UI). `engine =
/// "whisper"` is the legacy whisper.cpp override from Advanced.
#[tauri::command]
fn transcribe_episode(path: String, episode: u32, audio_file: String, engine: Option<String>, binary: Option<String>, model: Option<String>) -> CmdResult<serde_json::Value> {
    use shutri_core::transcribe::Transcriber;
    let audio = bundle::episode_dir(&pb(&path), episode).join("audio").join(&audio_file);
    let binary = binary.unwrap_or_default();
    let words = if engine.as_deref() == Some("whisper") {
        let engine = transcribe::WhisperCpp { binary: if binary.trim().is_empty() { "whisper-cli".into() } else { binary } };
        engine.transcribe(&audio, &pb(&model.unwrap_or_default())).map_err(|e| e.to_string())?
    } else {
        let engine = transcribe::SherpaOnnxTranscriber {
            binary: if binary.trim().is_empty() { None } else { Some(binary) },
            ..Default::default()
        };
        engine.transcribe(&audio, &pb("")).map_err(|e| e.to_string())?
    };
    let n = transcribe::save_transcription(&pb(&path), episode, &words).map_err(|e| e.to_string())?;
    let text = words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    Ok(serde_json::json!({"words": n, "text": text}))
}

#[tauri::command]
fn plan_from_episode(path: String, episode: u32, target_secs: f64) -> CmdResult<plan::Plan> {
    let words = transcribe::load_words(&pb(&path), episode).map_err(|e| e.to_string())?;
    let mut p = plan::plan_from_words(&words, target_secs);
    for c in &mut p { c.sync_distribution_fields(); }
    let dest = bundle::episode_dir(&pb(&path), episode).join("plan.json");
    if let Some(parent) = dest.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    std::fs::write(&dest, serde_json::to_string_pretty(&p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(p)
}

/// Ledger Edit: open in the user's editor (preference first, then
/// gvim -> VS Code -> nvim -> vim -> Notepad, as in deepDive).
#[tauri::command]
fn open_episode_file(path: String, file_name: String, editor_pref: Option<String>) -> CmdResult<String> {
    if file_name.contains(['/', '\\']) || !file_name.ends_with(".md") {
        return Err("episode file must be a .md file name inside src/".into());
    }
    let full = pb(&path).join("src").join(&file_name);
    let used = editor::open_in_editor(&full, editor_pref.as_deref().unwrap_or("")).map_err(|e| e.to_string())?;
    Ok(format!("Opened {} in {used}.", full.display()))
}

#[tauri::command]
fn lock_episode(path: String, episode: u32) -> CmdResult<episodes::Episode> {
    episodes::lock_episode(&pb(&path), episode).map_err(|e| e.to_string())
}

#[tauri::command]
fn sync_video_carousel(path: String, episode: u32) -> CmdResult<usize> {
    let s = syndication::load_syndication(&pb(&path));
    syndication::sync_video_carousel(&pb(&path), episode, &s).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_episode(path: String, file_name: String) -> CmdResult<String> {
    episodes::read_episode(&pb(&path), &file_name).map_err(|e| e.to_string())
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

/// The one running `mdbook serve` preview child, if any.
struct PreviewState(std::sync::Mutex<Option<std::process::Child>>);

#[tauri::command]
fn start_preview(path: String, state: tauri::State<'_, PreviewState>) -> CmdResult<String> {
    let mut slot = state.0.lock().map_err(|_| "preview state poisoned".to_string())?;
    if let Some(child) = slot.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => *slot = None,
            Ok(None) => return Ok("http://127.0.0.1:3000".into()),
            Err(e) => return Err(e.to_string()),
        }
    }
    let mut child = std::process::Command::new("mdbook")
        .args(["serve", "--hostname", "127.0.0.1", "--port", "3000"])
        .current_dir(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start mdbook — is it installed and on PATH? ({e})"))?;
    // Wait briefly for the port to accept, so the browser opens a live page.
    let addr = "127.0.0.1:3000";
    let mut up = false;
    for _ in 0..40 {
        if std::net::TcpStream::connect(addr).is_ok() { up = true; break; }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("mdbook serve exited early ({status}) — run Build mdBook once and check its output"));
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    if !up {
        let _ = child.kill();
        return Err("mdbook serve did not open port 3000 within 10 seconds".into());
    }
    *slot = Some(child);
    open_in_browser("http://127.0.0.1:3000");
    Ok("http://127.0.0.1:3000".into())
}

#[tauri::command]
fn stop_preview(state: tauri::State<'_, PreviewState>) -> CmdResult<String> {
    let mut slot = state.0.lock().map_err(|_| "preview state poisoned".to_string())?;
    match slot.as_mut() {
        Some(child) => {
            child.kill().map_err(|e| e.to_string())?;
            let _ = child.wait();
            *slot = None;
            Ok("Preview server stopped.".into())
        }
        None => Ok("Preview server was not running.".into()),
    }
}

#[tauri::command]
fn preview_running(state: tauri::State<'_, PreviewState>) -> bool {
    let mut slot = match state.0.lock() { Ok(s) => s, Err(_) => return false };
    match slot.as_mut() {
        Some(child) => matches!(child.try_wait(), Ok(None)),
        None => false,
    }
}

fn open_in_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let spawned = std::process::Command::new("xdg-open").arg(url).spawn();
    let _ = spawned;
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(PreviewState(std::sync::Mutex::new(None)))
        .manage(Jobs::new())
        .invoke_handler(tauri::generate_handler![
            inspect_project,
            create_project,
            clone_project,
            list_episodes,
            create_draft,
            ingest_python,
            ingest_markdown,
            delete_episode,
            open_episode_file,
            lock_episode,
            sync_video_carousel,
            upload_audio,
            transcribe_episode,
            plan_from_episode,
            read_episode,
            promote_episode,
            build_book,
            start_preview,
            stop_preview,
            preview_running,
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
