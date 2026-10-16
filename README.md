# Shutri Studio — Research to Reels

One Windows desktop app that turns research into a narrative, the
narrative into clips, and the episode into channels. Design of
record: `../ResearchToReels-Architecture.md`.

- Research → narrative (conversation + word timing, the canonical artifact)
- Narrative → episode bundle (KaTeX article, audio, `plan.json` clips
  whose speech/music segments are editable from the transcript and
  from uploaded music files)
- Bundle → channels: mdBook web channel is live; Nostr (NIP-23),
  LinkedIn, Substack, Instagram, TikTok, YouTube and podcast channels
  are adapters on hold until the core is solid.

## Layout

| Path | What |
|---|---|
| `core/` | Rust core library (episodes, plan, git, nostr, media) — fully tested headlessly |
| `src-tauri/` | Tauri 2 shell; thin typed commands over the core, no server, no ports |
| `src/` | Svelte + TypeScript frontend (Article / Reels / Publish / Identity) |
| `template/` | Bundled new-project scaffold |

## Run

```sh
# Frontend in a browser (demo mode, in-memory data):
npm install
npm run dev            # http://localhost:1420

# Desktop app (Windows target):
npm run tauri          # tauri dev
npm run tauri:build    # NSIS/MSI installer

# Core tests:
cargo test --manifest-path core/Cargo.toml
```

Identity: a Nostr keypair can be created locally in the app. The
public `npub` signs narratives and reels; the secret is stored only
NIP-49-encrypted on the device. It is a publishing identity, not an
app login — the app has no account, and Git/OAuth channels keep
their own credentials.
