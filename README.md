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
| `src/` | Svelte + TypeScript frontend (Guide / Article / Reels / Publish / Identity) |
| `template/` | Bundled new-project scaffold |

## Fresh project layout (what Create new makes)

Studio always starts from a fresh repo — it never imports or syncs an
existing publication. A new project is a self-contained mdBook ledger:

```text
MyResearch/
  book.toml            (KaTeX preprocessor enabled)
  README.md
  src/
    SUMMARY.md         cover · mempool · template · chain (managed block
                       between <!-- studio:episodes --> markers; anything
                       outside the markers is preserved verbatim)
    cover.md  mempool.md  template.md  chain.md
    _<slug>.md         mempool drafts (no permanent URL yet)
    <N>.md             mined episodes (permanent URLs, Law of 21 per block)
  episodes/
    <N>/  audio/  transcription.json  narrative.md
          plan.json  cover.webp  clips/  combined/
  .studio/             local app state, git-ignored
```

The episode number is the master key across article, bundle and posts.

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
