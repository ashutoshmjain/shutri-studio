<script lang="ts">
  import { onMount, tick } from "svelte";
  import { marked } from "marked";
  import renderMathInElement from "katex/contrib/auto-render";
  import "katex/dist/katex.min.css";
  import { api, inTauri, pickFile, pickFolder, pickPayloadFile, type Channel, type Clip, type Episode, type GitStatus, type ProjectInfo, type Syndication } from "./lib/api";

  type View = "guide" | "article" | "reels" | "publish" | "identity" | "settings";
  let view: View = "guide";

  const MASTER_PROMPT = `Objective: Package the complete exhaustive research from this session into a self-extracting Python script without summarizing or pruning.

1. Packaging Requirements:
* Full Fidelity: Package the exhaustive research in its entirety. Do not summarize or prune.
* Absolute KaTeX: Wrap all mathematical notation and symbols in Absolute KaTeX delimiters: $...$ for inline and $$...$$ for block displays. Never leave whitespace adjacent to '$'. Escape financial dollar amounts as \\$ (e.g. \\$100M).
* Hyperlinked Bibliography: Format every entry in the "Works Cited" section as a clickable Markdown link: [^N]: [Author, Title, Year](URL).
* Title Header: Ensure the document begins with a strict H1 title adhering to a 5-word maximum limit: # [Title].

2. Python Script Format:
Create an executable Python script where the full exhaustive research markdown is stored in raw_markdown:

raw_markdown = """
# [Title]

[Insert complete exhaustive markdown research here with $inline$ and $$block$$ math and [^1] citations...]
"""

if __name__ == '__main__':
    with open('final_research.md', 'w', encoding='utf-8') as f:
        f.write(raw_markdown)
    print("Extracted final_research.md successfully.")`;

  async function copyPrompt() {
    try {
      await navigator.clipboard.writeText(MASTER_PROMPT);
      notice = "Master Packaging Prompt copied. Paste it into Gemini, Claude or ChatGPT at the end of your research session.";
    } catch {
      error = "Clipboard unavailable in this window; select the prompt text and copy it manually.";
    }
  }

  let projectPath = typeof localStorage !== "undefined" ? (localStorage.getItem("studio-last-project-path") ?? "") : "";
  let projectName = "MyResearch";
  let project: ProjectInfo | null = null;
  let episodes: Episode[] = [];
  let channels: Channel[] = [];
  let git: GitStatus | null = null;
  let busy = false;
  let error = "";
  let notice = "";

  let slug = "";
  let title = "";
  let body = "";

  let transcript = "Welcome to the deep dive. Today we look at a hard problem. The evidence is strong, and the math matters. Here is the second idea, with an equation worth slowing down for. Finally, what should a careful reader do next?";
  let targetSecs = 20;
  let planEpisode = 247;
  let plan: Clip[] = [];
  let musicFile = "sting.mp3";

  let identity: { npub: string; pubkey_hex: string } | null = null;
  let password = "";
  let backupNsec = "";
  let signedEvent = "";

  let activity = "";
  async function run<T>(label: string, fn: () => Promise<T>): Promise<T | null> {
    busy = true; activity = label; error = ""; notice = ""; detail = "";
    try { return await fn(); }
    catch (e) { error = `${label} failed: ${e instanceof Error ? e.message : String(e)}`; return null; }
    finally { busy = false; activity = ""; }
  }

  let verbose = typeof localStorage !== "undefined" ? localStorage.getItem("studio-verbose") !== "0" : true;
  let detail = "";
  let help = "";
  let previewOn = false;
  const HELP: Record<string, string> = {
    build: "Build mdBook turns your src/ Markdown into a static website in the book/ folder (HTML, search index, KaTeX-rendered equations). Nothing goes online. It is the same output that will be published when you push. Rebuild after editing any draft or SUMMARY.md.",
    preview: "Preview starts mdbook serve on http://127.0.0.1:3000 and opens it in your browser — your book exactly as readers will see it, served locally. It rebuilds itself when files change. Stop it when done; it only runs while this app keeps it running.",
    mine: "Mine moves a draft from the mempool to the template: it assigns the next episode number, renames _draft.md to <N>.md, adds the donation wallet and the Spotify / Apple / Fountain audio links above Works Cited, and rewrites SUMMARY.md. Do this only when the podcast audio is final: Spotify needs this episode's permanent URL, and a numbered episode is never renumbered. The video carousel is not added now — it arrives when clips exist (Sync video carousel in Reels).",
    lock: "Lock moves a mined episode from the template into the chain: the file and its permanent URL stay exactly where they are, the episode is marked locked in SUMMARY.md under chain, and it becomes immutable — it can no longer be deleted. Lock only when the episode is truly finished.",
    extract: "Extract reads the raw_markdown string out of your LLM's Python payload and writes it as a mempool draft (_slug.md), byte-for-byte, so $...$ equations survive. The title and slug come from the paper's # H1 line.",
    publish: "Publish stages src/, episodes/ and book.toml in your project repo, commits with an automatic message, and pushes to its configured remote. It uses your own Git credentials; the app stores no tokens.",
    audio: "Upload audio into episode COPIES your NotebookLM original (mp3/wav/m4a/flac/ogg) into episodes/<N>/audio/ inside the project — it is not pasted into the research article and never goes through the chat. The file can be well over 100 MB; it lives in your project repo and ships with it when you Publish, so keep the original somewhere safe and treat the episode bundle as the heavy part of the project.",
    transcribe: "Transcribe Episode runs openai-whisper (deepDive's time-tested small.en, word_timestamps=True) on the uploaded audio and writes episodes/<N>/transcription.json (every word timed) plus narrative.md. Requires `pip install openai-whisper` in the Python on PATH (or set SHUTRI_PYTHON). sherpa-onnx and whisper.cpp remain available under Advanced / Developer as overrides — they need their own models/binaries. The engine fails loudly if no word timestamps are returned — flat zero timing must never become a clip plan.",
    clips: "Break into clips (real timing) groups the transcript's sentences into ~target-second clips, snapping boundaries to sentence ends using the saved word timestamps, and writes plan.json. If you only get 1 clip covering the whole episode, the transcription behind it has no usable timing — re-transcribe and check that transcription.json words carry distinct start/end times. Then retitle clips, nudge speech start/end seconds, add music segments with crossfades, remove what drags, and Save plan.json.",
    carousel: "Sync video carousel rebuilds the VIDEO_STRIP carousel plus the TikTok / Instagram / YouTube links in src/<N>.md from the .mp4 files actually present in episodes/<N>/clips/. It replaces the carousel, never duplicates it, and does nothing when no clips exist yet. Your video platform URLs come from the Settings tab.",
    settings: "Settings stores YOUR donation wallet (Lightning address) and YOUR platform URLs (Spotify / Apple / Fountain audio, TikTok / Instagram / YouTube video) in this project. Mine uses them for the wallet + audio block, Sync carousel uses them for the video links. Defaults match the deepDive project; a project with no saved settings shows those defaults, and saving writes .studio/syndication.json (local, not published).",
  };
  function showHelp(topic: string) { help = HELP[topic] ?? ""; }
  function say(short: string, long = "") {
    notice = short;
    detail = verbose ? long : "";
  }
  function toggleVerbose() {
    verbose = !verbose;
    try { localStorage.setItem("studio-verbose", verbose ? "1" : "0"); } catch {}
    if (!verbose) detail = "";
    notice = verbose
      ? "Messages set to Verbose: I will explain what happened, why, and what to do next."
      : "Messages set to Quiet: one-line confirmations only. Tap any i button for the full explanation.";
  }

  let reading: Episode | null = null;
  let readerHTML = "";
  let readerEl: HTMLDivElement | null = null;

  async function openEpisode(ep: Episode) {
    const fileName = ep.path.replace(/\\/g, "/").split("/").pop() ?? "";
    const text = await run("Loading episode for reading", () => api.readEpisode(projectPath, fileName));
    if (text === null) return;
    reading = ep;
    readerHTML = marked.parse(text) as string;
    await tick();
    if (readerEl) {
      renderMathInElement(readerEl, {
        delimiters: [
          { left: "$$", right: "$$", display: true },
          { left: "$", right: "$", display: false },
        ],
        throwOnError: false,
      });
    }
    readerEl?.scrollIntoView({ behavior: "smooth", block: "start" });
    notice = `Now reading "${ep.title}" at the top of the Article tab — rendered with KaTeX, as it will look on the web.`;
  }

  async function refresh() {
    if (!projectPath) return;
    const info = await run("Refreshing project", () => api.inspectProject(projectPath));
    if (info) project = info;
    const eps = await run("Loading episodes", () => api.listEpisodes(projectPath));
    if (eps) episodes = eps;
    const g = await run("Loading Git status", () => api.gitStatus(projectPath));
    if (g) git = g;
    const id = await run("Loading identity", () => api.loadIdentity(projectPath));
    identity = id ?? null;
  }

  async function browseFolder() {
    error = ""; notice = "";
    try {
      const picked = await pickFolder();
      if (picked) {
        projectPath = picked;
        try { localStorage.setItem("studio-last-project-path", picked); } catch {}
        const base = picked.replace(/[\\/]+$/, "").split(/[\\/]/).pop();
        if (base && (projectName === "MyResearch" || !projectName)) projectName = base;
        notice = `Folder chosen: ${picked}. Now click "Create new project" (or "Reopen existing" if it is already a Studio project).`;
      } else if (!inTauri) {
        notice = "Folder picker needs the desktop app; in this browser demo, type the path instead.";
      } else {
        notice = "Folder selection cancelled — no folder was chosen.";
      }
    } catch (e) {
      error = `Folder picker failed: ${e instanceof Error ? e.message : String(e)}. You can still type the folder path manually.`;
    }
  }
  let payloadPath = "";
  async function browsePayload() {
    error = ""; notice = "";
    try {
      const picked = await pickPayloadFile();
      if (picked) {
        payloadPath = picked;
        notice = `Payload chosen: ${picked}. Now click "Extract into mempool".`;
      } else if (!inTauri) {
        notice = "File picker needs the desktop app; in this browser demo the extract step is simulated.";
      } else {
        notice = "Payload selection cancelled — no file was chosen.";
      }
    } catch (e) {
      error = `Payload picker failed: ${e instanceof Error ? e.message : String(e)}`;
    }
  }
  async function ingestPayload() {
    const ep = await run("Extracting Python payload into the mempool", () => api.ingestPython(projectPath, payloadPath));
    if (ep) {
      payloadPath = "";
      view = "article";
      await refresh();
      say(`In mempool: "${ep.title}" as _${ep.slug}.md — KaTeX preserved.`,
        "What just happened: Studio pulled the raw_markdown out of your Python file and stored it in src/ as a mempool draft, and SUMMARY.md now lists it. It has no permanent URL yet. Read it in the Ledger, make the NotebookLM audio, and Mine only when that audio is final.");
    }
  }
  let mdPath = "";
  async function browseMarkdown() {
    error = ""; notice = "";
    try {
      const picked = await pickFile("Choose a Markdown file", "Markdown", ["md", "markdown"]);
      if (picked) { mdPath = picked; notice = `Markdown chosen: ${picked}. Now click "Upload into mempool".`; }
      else if (!inTauri) notice = "File picker needs the desktop app; in the demo, type a path and the upload is simulated.";
      else notice = "Markdown selection cancelled.";
    } catch (e) { error = `Markdown picker failed: ${e instanceof Error ? e.message : String(e)}`; }
  }
  async function uploadMarkdown() {
    const ep = await run("Uploading Markdown into the mempool", () => api.ingestMarkdown(projectPath, mdPath));
    if (ep) { mdPath = ""; await refresh(); say(`Uploaded "${ep.title}" into the mempool as _${ep.slug}.md.`, "The file was copied into src/ and listed in SUMMARY.md — the original stays where it was. Read it in the Ledger; edit it there with the Edit button (opens it in your chosen editor — gVim by default) at any time."); }
  }
  async function openProject() {
    try { localStorage.setItem("studio-last-project-path", projectPath); } catch {}
    const p = await run("Opening project", () => api.inspectProject(projectPath));
    if (p) { project = p; await refresh(); }
  }
  async function createProject() {
    try { localStorage.setItem("studio-last-project-path", projectPath); } catch {}
    const p = await run("Creating fresh project (folders, SUMMARY.md, git init)", () => api.createProject(projectPath, projectName));
    if (p) { project = p; await refresh(); say("Fresh project created and initialised as a Git repo.", "What you have now: book.toml (the book settings), src/SUMMARY.md listing cover · mempool · template · chain, a _welcome mempool draft to experiment with, and an episodes/ folder for audio, plans and clips. Nothing exists online yet. Next step: open the Article tab and extract your first Python payload."); }
  }
  async function createDraft() {
    const ep = await run("Adding draft to mempool", () => api.createDraft(projectPath, slug, title, body));
    if (ep) { slug = ""; title = ""; body = ""; await refresh(); notice = `Draft _${ep.slug}.md is in the mempool — see the Ledger.`; }
  }
  async function promote(ep: Episode) {
    const mined = await run("Mining episode into the template", () => api.promoteEpisode(projectPath, ep.slug));
    if (mined) { await refresh(); say(`Mined as episode ${mined.number} — permanent URL assigned.`, "What just happened: the draft was renamed to " + mined.number + ".md, its title was numbered, the donation wallet and Spotify / Apple / Fountain links were added above Works Cited, and SUMMARY.md moved it from mempool to template. This is the URL to paste into your Spotify episode description. It will never be renumbered, even if you edit the text. Lock it into the chain only when it is completely finished."); }
  }
  async function lock(ep: Episode) {
    if (ep.number === null) return;
    if (!window.confirm(`Lock episode ${ep.number} ("${ep.title}") into the chain?\n\nIts file and permanent URL stay put, but it becomes immutable: it can no longer be deleted. Do this only when the episode is completely finished.`)) return;
    const locked = await run("Locking episode into the chain", () => api.lockEpisode(projectPath, ep.number!));
    if (locked) { await refresh(); say(`Episode ${locked.number} is locked in the chain.`, "What just happened: SUMMARY.md now lists this episode under chain instead of template, and the article carries the locked marker. It is sealed — Studio will refuse to delete it."); }
  }
  let editorPref = typeof localStorage !== "undefined" ? (localStorage.getItem("studio-editor") ?? "gvim") : "gvim";
  function setEditorPref(v: string) {
    editorPref = v;
    try { localStorage.setItem("studio-editor", v); } catch {}
    notice = v === "default" ? "Edit will use the first editor found: gVim, VS Code, nvim, vim, then Notepad." : `Edit will open files in ${v} first (falling back to gVim, VS Code, nvim, vim, Notepad).`;
  }
  function fileNameOf(ep: Episode) { return ep.path.replace(/\\/g, "/").split("/").pop() ?? ""; }
  async function editEpisode(ep: Episode) {
    const out = await run("Opening file in your editor", () => api.openEpisodeFile(projectPath, fileNameOf(ep), editorPref));
    if (out) say(out, "Edit and save in your editor, then Read again here or Build to refresh the book. Studio re-reads the file from disk every time — there is no second copy to sync.");
  }
  async function removeEpisode(ep: Episode) {
    if (!window.confirm(`Delete "${ep.title}" (${fileNameOf(ep)})?\n\nThis removes the article file and its SUMMARY.md entry. ${ep.number ? "It is a numbered template episode; its episodes/ bundle, if any, is left untouched only when no plan exists." : "It is a mempool draft."} This cannot be undone except through Git.`)) return;
    const title = await run("Deleting article", () => api.deleteEpisode(projectPath, fileNameOf(ep)));
    if (title !== null) {
      if (reading && fileNameOf(reading) === fileNameOf(ep)) { reading = null; readerHTML = ""; }
      await refresh();
      say(`Deleted "${title}".`, "The file and its SUMMARY.md entry are gone. If the project is a Git repo you can still recover it from history until you commit the deletion. Sealed chain episodes are never deletable here.");
    }
  }
  async function build() {
    const log = await run("Building mdBook (this can take a moment)", () => api.buildBook(projectPath));
    if (log !== null) say("mdBook build finished — the website is in the book/ folder.",
      "What just happened: every page in src/ became HTML in book/, with equations rendered by KaTeX and a search index built. Nothing is online yet. Click Preview to read it at localhost:3000 exactly as readers will see it, or go to Publish when you are ready to push.");
  }
  async function startPreview() {
    const url = await run("Starting preview server (mdbook serve)", () => api.startPreview(projectPath));
    if (url) {
      previewOn = true;
      say(`Preview is live at ${url} — your browser should have opened it.`,
        "What just happened: mdbook serve rebuilt the book and is serving it on your own machine at port 3000. Only you can see it. It rebuilds automatically when files change. Use Stop preview when finished; closing the preview does not publish anything.");
    }
  }
  async function stopPreview() {
    const out = await run("Stopping preview server", () => api.stopPreview());
    if (out !== null) { previewOn = false; say(out, "Port 3000 is free again. Your book/ output and project files are untouched."); }
  }
  let audioPath = "";
  let uploadedAudioName = "";
  // Default is now deepDive's time-tested openai-whisper (PythonWhisper
  // small.en, word_timestamps=True). sherpa-onnx and whisper.cpp are
  // Advanced overrides only.
  let transcribeEngine = typeof localStorage !== "undefined" ? (() => { const v = localStorage.getItem("studio-transcribe-engine"); return v === "sherpa" ? "python" : (v ?? "python"); })() : "python";
  let engineBinary = typeof localStorage !== "undefined" ? (localStorage.getItem("studio-engine-bin") ?? "") : "";
  let whisperModel = typeof localStorage !== "undefined" ? (localStorage.getItem("studio-whisper-model") ?? "small.en") : "small.en";
  async function browseAudio() {
    error = ""; notice = "";
    try {
      const picked = await pickFile("Choose the original NotebookLM audio", "Audio", ["mp3", "wav", "m4a", "flac", "ogg"]);
      if (picked) { audioPath = picked; notice = `Audio chosen: ${picked}. Now click "Upload audio into episode".`; }
      else if (!inTauri) notice = "File picker needs the desktop app; in the demo, uploads are simulated.";
      else notice = "Audio selection cancelled.";
    } catch (e) { error = `Audio picker failed: ${e instanceof Error ? e.message : String(e)}`; }
  }
  async function doUploadAudio() {
    const dest = await run("Copying audio into the episode bundle", () => api.uploadAudio(projectPath, planEpisode, audioPath));
    if (dest) {
      uploadedAudioName = dest.replace(/\\/g, "/").split("/").pop() ?? "";
      say(`Audio stored for episode ${planEpisode}: ${uploadedAudioName}`,
        "What just happened: the ORIGINAL NotebookLM audio was copied into episodes/" + planEpisode + "/audio/. Your Audacity podcast edit stays outside Studio. Next: run Transcribe to get word-level timing for the Narrative and clips.");
    }
  }
  async function browseModel() {
    const picked = await pickFile("Choose the Whisper model file (.bin)", "Whisper model", ["bin"]).catch(() => null);
    if (picked) { whisperModel = picked; try { localStorage.setItem("studio-whisper-model", picked); } catch {} notice = `Whisper model set: ${picked}`; }
  }
  async function doTranscribe() {
    try { localStorage.setItem("studio-transcribe-engine", transcribeEngine); localStorage.setItem("studio-engine-bin", engineBinary); localStorage.setItem("studio-whisper-model", whisperModel); } catch {}
    const out = await run("Transcribing episode (word-level — can take minutes)", () => api.transcribeEpisode(projectPath, planEpisode, uploadedAudioName, transcribeEngine, engineBinary, whisperModel));
    if (out) {
      transcript = out.text;
      say(`Transcribed ${out.words} words with timestamps for episode ${planEpisode}.`,
        "What you have now: episodes/" + planEpisode + "/transcription.json (every word timed) and narrative.md (the readable conversation). The transcript is shown below. Set the target seconds and click Break into clips (real timing) — boundaries will snap to real word ends, then edit each segment and Save plan.json.");
    }
  }
  async function doSyncCarousel() {
    const n = await run("Syncing video carousel into the episode article", () => api.syncVideoCarousel(projectPath, planEpisode));
    if (n !== null) {
      await refresh();
      if (n > 0) say(`Video carousel updated: ${n} clip(s) now embedded in episode ${planEpisode}.`, "What just happened: the VIDEO_STRIP carousel and the TikTok / Instagram / YouTube links in src/" + planEpisode + ".md were rebuilt from the .mp4 files actually present in episodes/" + planEpisode + "/clips/. Re-run this whenever new clips land; it replaces the carousel, never duplicates it.");
      else say(`No clips found for episode ${planEpisode} yet — nothing embedded.`, "The carousel is only added once clip videos exist in episodes/" + planEpisode + "/clips/. Build the clips first, then sync again.");
    }
  }
  async function doPlanReal() {
    const p = await run("Breaking episode into clips from real word timing", () => api.planFromEpisode(projectPath, planEpisode, targetSecs));
    if (p) { plan = p; say(`${p.length} clips planned for episode ${planEpisode} and saved to plan.json.`, "Boundaries snapped to sentence ends using the real word timestamps — not estimates. Now the creative edit: retitle clips, nudge speech start/end seconds, add music segments with crossfades, remove what drags. Save plan.json after editing."); }
  }
  async function makePlan() {
    const p = await run("Breaking transcript into clips (estimated timing)", () => api.planFromTranscript(transcript, targetSecs));
    if (p) { plan = p; say(`${p.length} clips planned (estimated timing) and shown below.`, "This is a dry-run estimate (~2.5 words/sec) without real audio timing. For accurate boundaries, use Break into clips (real timing) after Transcribe Episode. Now edit, then Save plan.json."); }
  }
  async function savePlan() {
    const out = await run("Saving plan.json", () => api.savePlan(projectPath, planEpisode, plan));
    if (out) notice = `Plan saved to ${out}`;
  }
  async function loadPlan() {
    const p = await run("Loading saved plan", () => api.loadPlan(projectPath, planEpisode));
    if (p) plan = p;
  }
  function addMusic(clip: Clip) {
    clip.segments = [
      { type: "music", music_file: musicFile, duration: 5.5, crossfade: 1.3, volume: 1 },
      ...clip.segments,
    ];
    plan = [...plan];
    notice = `Music segment added to clip ${clip.num}. Adjust it in plan.json order; Save plan.json to persist.`;
  }
  function removeSegment(clip: Clip, i: number) {
    clip.segments = clip.segments.filter((_, j) => j !== i);
    plan = [...plan];
    notice = `Segment removed from clip ${clip.num}. Save plan.json to persist.`;
  }
  async function publish() {
    const msg = `publish: episode ${planEpisode} via Shutri Studio`;
    const out = await run("Staging, committing & pushing", () => api.gitPublish(projectPath, msg));
    if (out) { await refresh(); notice = out; }
  }
  async function makeIdentity() {
    const id = await run("Creating Nostr identity", () => api.generateIdentity(projectPath, password));
    if (id) { identity = { npub: id.npub, pubkey_hex: id.pubkey_hex }; backupNsec = id.nsec_backup; }
  }
  async function sign() {
    const ev = await run("Signing narrative", () => api.signNarrative(projectPath, password, planEpisode, `# Episode ${planEpisode}\n\n${transcript}`));
    if (ev) signedEvent = JSON.stringify(ev, null, 2);
  }
  let synd: Syndication = { lightning_address: "", spotify_url: "", apple_podcasts_url: "", fountain_url: "", tiktok_url: "", instagram_url: "", youtube_url: "" };
  async function loadSettings() {
    const s = await run("Loading settings", () => api.loadSyndication(projectPath));
    if (s) synd = s;
  }
  async function saveSettings() {
    const out = await run("Saving settings", () => api.saveSyndication(projectPath, synd));
    if (out) { notice = out; await loadSettings(); }
  }

  onMount(async () => { channels = await api.listChannels().catch(() => []); previewOn = await api.previewRunning().catch(() => false); });
</script>

<svelte:window onkeydown={(e: KeyboardEvent) => { if (e.key === "Escape") help = ""; }} />
<header class="topbar">
  <div>
    <div class="brand">Shutri Studio</div>
    <div class="tagline">Research to Reels</div>
  </div>
  <nav>
    <button class:active={view === "guide"} onclick={() => (view = "guide")}>Guide</button>
    <button class:active={view === "article"} onclick={() => (view = "article")}>Article</button>
    <button class:active={view === "reels"} onclick={() => (view = "reels")}>Reels</button>
    <button class:active={view === "publish"} onclick={() => (view = "publish")}>Publish</button>
    <button class:active={view === "identity"} onclick={() => (view = "identity")}>Identity &amp; Channels</button>
    <button class:active={view === "settings"} onclick={() => (view = "settings")}>Settings</button>
    <button title="Toggle how much the app explains" onclick={toggleVerbose}>Messages: {verbose ? "Verbose" : "Quiet"}</button>
  </nav>
</header>

{#if !inTauri}
  <div class="demo-banner">Browser demo mode — in-memory data only. Run <span class="mono">npm run tauri</span> on Windows for the real desktop app.</div>
{/if}

<main>
  {#if view === "guide"}
  <section class="project-bar">
    <div class="grow">
      <label for="pp">New project folder (a fresh repo — Studio sets up the whole structure)</label>
      <div class="pathrow">
        <input id="pp" bind:value={projectPath} placeholder="C:\Users\you\MyResearch" />
        <button disabled={busy} onclick={browseFolder}>Browse…</button>
      </div>
      <p class="small">Browse opens the Windows folder dialog — pick a folder, or create a new one inside it. The project is created in the folder you choose.</p>
    </div>
    <div>
      <label for="pn">Project name</label>
      <input id="pn" bind:value={projectName} placeholder="MyResearch" />
    </div>
    <button class="primary" disabled={busy || !projectPath} title={!projectPath ? "Choose a folder first (Browse… or type a path)" : "Create the fresh project in this folder"} onclick={createProject}>Create new project</button>
    <button disabled={busy || !projectPath} title={!projectPath ? "Choose a folder first (Browse… or type a path)" : "Open this folder as an existing Studio project"} onclick={openProject}>Reopen existing</button>
  </section>
  <p class="small">Studio always starts from a fresh repo: <span class="mono">book.toml</span>, <span class="mono">src/SUMMARY.md</span> with cover · mempool · template · chain, and an <span class="mono">episodes/</span> folder for audio, plans and clips. It does not import or sync an existing publication. {#if !projectPath}First step: choose a folder above — both buttons stay disabled until you do.{/if}</p>
  {:else}
  <section class="project-strip">
    {#if project}
      <span class="mono">{project.path}</span>
      <span class="small">episodes {project.episode_count} · mempool {project.mempool_count} · {project.is_git_repo ? `git: ${project.remote ?? "no remote"}` : "not a git repo"}</span>
    {:else}
      <span class="small">No project open yet — Article, Reels and Publish need one.</span>
    {/if}
    <button disabled={busy} onclick={() => (view = "guide")}>{project ? "Switch project" : "Create or open a project"}</button>
  </section>
  {/if}


  {#if view === "guide"}
    <div class="grid">
      <section class="panel">
        <h2>The problem</h2>
        <p>Research is getting lost inside LLMs. A Gemini session can hand you a web page link, or you can file the work into a knowledge graph, but it stays locked inside one tool's silo — rarely citable, rarely readable by anyone else, and gone when the session is.</p>
        <p>At the same time, research itself has arguably become the easy part. The hard part has moved: <strong>getting attention for your ideas</strong>. Publishing work into mass media is still genuinely difficult, and every platform mangles a different thing.</p>
        <h2>What we are doing</h2>
        <p>Shutri Studio is a deliberately opinionated, workflow-shaped app — Research to Reels. It takes one research session and walks it to people: a permanent article, a podcast episode, readable narratives, and short clips, fixing each platform's format quirks along the way instead of fighting them by hand every time.</p>
        <h3>Format quirks this app handles for you</h3>
        <ul class="stages">
          <li><strong>Research needs real mathematics.</strong> Equations and symbols are the content, not decoration — so the research is published on the <strong>open web</strong> (your own mdBook site, full KaTeX), not on Substack or LinkedIn, which strip or break math.</li>
          <li><strong>Each platform gets the version it deserves.</strong> The exhaustive paper lives on the web; the watered-down Narrative goes to LinkedIn, Nostr and later Substack; the podcast RSS carries the audio to Spotify, Apple and Fountain.</li>
          <li><strong>Shorts have hard limits.</strong> A clip must run under 3 minutes to qualify as a YouTube Short — clip planning targets that, in a square 740×740 frame with music ducked under speech, captions and titles placed where each platform's interface does not cover them.</li>
          <li><strong>Permanence needs a stable link.</strong> Spotify's description can only point at a numbered episode, so mining happens exactly when the audio is final — never before, never renumbered after.</li>
        </ul>
        <h3>The pipeline</h3>
        <ol class="stages">
          <li><strong>Research</strong> in Gemini or similar — iterate freely; this is creative work.</li>
          <li><strong>Package</strong> the session with the Master Packaging Prompt (right) into a self-extracting Python script.</li>
          <li><strong>Publish as-is</strong> into the mempool (<span class="mono">_slug.md</span>) and build the book. No permanent URL yet.</li>
          <li><strong>Audio</strong> in NotebookLM, then your manual Audacity edit for the podcast. Studio receives the original NotebookLM audio.</li>
          <li><strong>Mine</strong> when the audio is final: the draft becomes a numbered episode in the template — the permanent URL the Spotify description will link to — with the donation wallet and audio platform links added. <strong>Lock</strong> it into the chain only when it is completely finished; locked episodes are immutable.</li>
          <li><strong>Transcribe</strong> the audio word-by-word; the transcript becomes the Narrative, the readable version for LinkedIn, Nostr and later Substack.</li>
          <li><strong>Clip</strong> the Narrative: edit speech and music segments, crossfades and volumes, intro page and outro questions, then infographics via Mosaic.</li>
          <li><strong>Distribute</strong> finished clips from GitHub, and combine them into the long-form YouTube video (and a combined audio to listen back and check).</li>
        </ol>
        <p class="small">Three steps stay manual and creative by design: the research, the NotebookLM/Audacity audio, and the clip edit. Studio automates everything around them.</p>
      </section>
      <section class="panel">
        <h2>Why equations break — and the Python fix</h2>
        <p>Publishing LLM research on the web fails in predictable ways. Copy-pasted Markdown mangles <span class="mono">$...$</span> delimiters, chat exports drop or summarize content, platforms strip math entirely, and a single space next to a <span class="mono">$</span> silently breaks KaTeX. Base64 survives transport but is opaque — you cannot read, diff or fix it. A month of trying those routes leads to one reliable answer: <strong>LLMs are excellent at emitting a Python script</strong>. Ask for the whole session inside a <span class="mono">raw_markdown</span> string in a self-extracting <span class="mono">.py</span> file, run it, and the research lands on disk byte-for-byte, math included.</p>
        <div class="row">
          <button class="primary" onclick={copyPrompt}>Copy the Master Packaging Prompt</button>
        </div>
        <pre class="mono event prompt">{MASTER_PROMPT}</pre>
        <h3>Your fresh repo</h3>
        <pre class="mono event">MyResearch/
  book.toml
  README.md
  src/
    SUMMARY.md   cover · mempool · template · chain
    cover.md  mempool.md  template.md  chain.md
    _draft.md    mempool drafts
    1.md         mined episodes (permanent URLs)
  episodes/
    1/  audio/  transcription.json  narrative.md
        plan.json  cover.webp  clips/  combined/</pre>
      </section>
    </div>
  {/if}

  {#if view === "article"}
    {#if reading}
      <section class="panel reader reader-top">
        <div class="reader-head">
          <div>
            <h2>{reading.number ? `#${reading.number} : ` : ""}{reading.title}</h2>
            <p class="small">{reading.stage} · rendered as it will appear on the web · equations via KaTeX</p>
          </div>
          <button disabled={busy} onclick={() => { reading = null; readerHTML = ""; }}>Close reader</button>
        </div>
        <div class="article-body" bind:this={readerEl}>{@html readerHTML}</div>
      </section>
    {/if}
    <div class="grid">
      <section class="panel">
        <h2>Bring research in — Python payload</h2>
        <p class="small">Finish your LLM session with the Master Packaging Prompt (Guide tab), save the <span class="mono">.py</span> it gives you, then extract it here. The research lands in the mempool exactly as written — KaTeX intact. This is the normal way research enters Studio.</p>
        <div class="pathrow">
          <input bind:value={payloadPath} placeholder="C:\Users\you\Downloads\research_payload.py" aria-label="Python payload file" />
          <button disabled={busy} onclick={browsePayload}>Browse .py…</button>
        </div>
        <div class="row">
          <button class="primary" disabled={busy || !projectPath || !payloadPath} title={!projectPath ? "Open a project first (Guide tab)" : !payloadPath ? "Choose a .py payload first" : "Extract raw_markdown into the mempool"} onclick={ingestPayload}>Extract into mempool</button>
          <button class="ibtn" title="How does extraction work?" onclick={() => showHelp("extract")}>i</button>
        </div>
        {#if !projectPath}<p class="small">Needs an open project — create one in the Guide tab first.</p>
        {:else if !payloadPath}<p class="small">Choose the <span class="mono">.py</span> payload to enable extraction.</p>{/if}
      </section>

      <section class="panel">
        <div class="ledger-head">
          <h2>Ledger</h2>
          <div class="ledger-tools">
            <label for="editor-pref">Edit opens in</label>
            <select id="editor-pref" value={editorPref} onchange={(e) => setEditorPref((e.target as HTMLSelectElement).value)}>
              <option value="gvim">gVim</option>
              <option value="notepad">Notepad</option>
              <option value="code">VS Code</option>
              <option value="default">First found</option>
            </select>
          </div>
        </div>
        {#if episodes.length === 0}<p class="small">Open or create a project to see its mempool and mined episodes.</p>{/if}
        {#each episodes as ep}
          <article class="episode">
            <div class="ep-main">
              <div class="ep-head">
                <strong class="ep-title">{ep.number ? `#${ep.number} : ` : ""}{ep.title}</strong>
                <span class="pill" data-stage={ep.stage}>{ep.stage}</span>
                {#if ep.has_clips}<span class="pill pill-clips">clips</span>{/if}
              </div>
              <div class="ep-meta mono" title={ep.path}>{fileNameOf(ep)}</div>
            </div>
            <div class="ep-actions">
              <button disabled={busy} title="Read this episode rendered, with equations" onclick={() => openEpisode(ep)}>Read</button>
              {#if ep.stage !== "chain"}
                <button disabled={busy} title={`Open the .md file in ${editorPref === "default" ? "your editor (gVim first)" : editorPref}`} onclick={() => editEpisode(ep)}>Edit</button>
              {/if}
              {#if ep.number === null}
                <button class="accent" disabled={busy} title="Mine: mempool → template (assign number, add wallet + audio links)" onclick={() => promote(ep)}>Mine</button>
                <button class="ibtn" title="What does Mine do?" onclick={() => showHelp("mine")}>i</button>
              {:else if ep.stage === "template"}
                <button class="accent" disabled={busy} title="Lock: template → chain (seal it, immutable)" onclick={() => lock(ep)}>Lock</button>
                <button class="ibtn" title="What does Lock do?" onclick={() => showHelp("lock")}>i</button>
              {:else}
                <span class="ep-locked">Locked</span>
              {/if}
              {#if ep.stage !== "chain"}
                <button class="danger" disabled={busy} title="Delete this article (mempool and template only — sealed chain content is immutable)" onclick={() => removeEpisode(ep)}>Delete</button>
              {/if}
            </div>
          </article>
        {/each}
        <div class="row ledger-foot">
          <button disabled={busy || !projectPath} onclick={build}>Build mdBook (web channel)</button>
          <button class="ibtn" title="What does Build do?" onclick={() => showHelp("build")}>i</button>
          {#if previewOn}
            <button class="accent" disabled={busy} onclick={stopPreview}>Stop preview</button>
          {:else}
            <button class="accent" disabled={busy || !projectPath} title={!projectPath ? "Open a project first" : "Serve the book on localhost:3000 and open it in your browser"} onclick={startPreview}>Preview site</button>
          {/if}
          <button class="ibtn" title="What does Preview do?" onclick={() => showHelp("preview")}>i</button>
        </div>
        <p class="small">{previewOn ? "Preview is running at http://127.0.0.1:3000 (reopen that address in your browser any time)." : "Build makes the static site in book/; Preview serves it on localhost:3000 so you can read it as readers will."}</p>
      </section>

      <section class="panel">
        <h2>…or write a draft by hand</h2>
        <label for="slug">Slug</label>
        <input id="slug" bind:value={slug} placeholder="quantum-memory" />
        <label for="ttl">Title</label>
        <input id="ttl" bind:value={title} placeholder="Quantum Memory" />
        <label for="bdy">Narrative / notes</label>
        <textarea id="bdy" bind:value={body} placeholder="The conversation this episode will become…"></textarea>
        <button class="primary" disabled={busy || !projectPath || !slug || !title} onclick={createDraft}>Add to mempool</button>
        <h3>…or upload a Markdown file</h3>
        <p class="small">Already have the paper as <span class="mono">.md</span> (for example from an external editor)? Upload it and it becomes a mempool draft with its title taken from the <span class="mono"># H1</span>.</p>
        <div class="pathrow">
          <input bind:value={mdPath} placeholder="C:\Users\you\Documents\paper.md" aria-label="Markdown file to upload" />
          <button disabled={busy} onclick={browseMarkdown}>Browse .md…</button>
        </div>
        <div class="row">
          <button disabled={busy || !projectPath || !mdPath} title={!projectPath ? "Open a project first" : !mdPath ? "Choose a .md file first" : "Upload as a mempool draft"} onclick={uploadMarkdown}>Upload into mempool</button>
        </div>
        <p class="small">Research is morphed into a narrative conversation first; the KaTeX article derives from it.</p>
      </section>
    </div>

      <p class="small reader-hint">Tip: click <strong>Read</strong> on any Ledger entry — it opens at the top of this tab, rendered with equations, before you decide to Mine it.</p>
  {/if}

  {#if view === "reels"}
    <div class="grid">
      <section class="panel">
        <h2>Episode audio → transcription</h2>
        <p class="small">Upload the <strong>original NotebookLM audio</strong> for a mined episode (your Audacity podcast edit stays outside Studio). Transcription (openai-whisper small.en, word-level — same as deepDive) turns it into word timing — the basis of the Narrative and of every clip boundary.</p>
        <div class="row">
          <div><label for="pe">Episode number</label><input id="pe" type="number" min="1" bind:value={planEpisode} /></div>
          <div><label for="tg">Target seconds per clip (under 180 for Shorts)</label><input id="tg" type="number" min="10" max="180" bind:value={targetSecs} /></div>
        </div>
        <label>Audio file (NotebookLM original)</label>
        <div class="pathrow">
          <input bind:value={audioPath} placeholder="C:\Users\you\Downloads\episode-audio.mp3" aria-label="Audio file" />
          <button disabled={busy} onclick={browseAudio}>Browse audio…</button>
        </div>
        <div class="lens-actions">
          <button class="primary" disabled={busy || !projectPath || !audioPath} title={!projectPath ? "Open a project first" : !audioPath ? "Choose an audio file first" : "Copy the audio into episodes/<N>/audio/"} onclick={doUploadAudio}>Upload audio into episode</button>
          <button class="ibtn" title="What does Upload audio do?" onclick={() => showHelp("audio")}>i</button>
          <button class="primary" disabled={busy || !projectPath || !uploadedAudioName} title={!uploadedAudioName ? "Upload the audio first" : "Transcribe with openai-whisper (small.en) and save transcription.json + narrative.md"} onclick={doTranscribe}>Transcribe Episode</button>
          <button class="ibtn" title="What does Transcribe do?" onclick={() => showHelp("transcribe")}>i</button>
        </div>
        <div class="lens-actions">
          <button class="accent" disabled={busy || !projectPath} title="Plan clips from the saved transcription.json (real word timing) and save plan.json" onclick={doPlanReal}>Break into clips (real timing)</button>
          <button class="ibtn" title="What does Break into clips do?" onclick={() => showHelp("clips")}>i</button>
          <button disabled={busy || !projectPath} title="Embed the video carousel + TikTok/Instagram/YouTube links for clips already built" onclick={doSyncCarousel}>Sync video carousel</button>
          <button class="ibtn" title="What does Sync video carousel do?" onclick={() => showHelp("carousel")}>i</button>
        </div>
        {#if !uploadedAudioName}<p class="small">Transcribe enables after the audio upload. Default is openai-whisper (deepDive's time-tested <span class="mono">small.en</span>) — just needs <span class="mono">pip install openai-whisper</span> in the Python on PATH. Alternatives live under Advanced.</p>{/if}
        <details>
          <summary class="small">Advanced / Developer — transcription engine override</summary>
          <label for="engine">Engine</label>
          <select id="engine" bind:value={transcribeEngine}>
            <option value="python">openai-whisper (default, deepDive parity)</option>
            <option value="sherpa">sherpa-onnx (Zipformer, needs models)</option>
            <option value="whisper">whisper.cpp (legacy, needs .bin)</option>
          </select>
          {#if transcribeEngine === "python"}
            <label for="wb">Python override (optional, else PATH)</label>
            <input id="wb" bind:value={engineBinary} placeholder="python / python3" />
            <label for="wm">Whisper model</label>
            <input id="wm" bind:value={whisperModel} placeholder="small.en" />
            <p class="small">Leave Python empty to use the first <span class="mono">python</span> on PATH. Set <span class="mono">SHUTRI_PYTHON</span> env to pin it. Model downloads on first use (~1 GB for small.en).</p>
          {:else}
            <label for="wb">Engine program (optional override)</label>
            <input id="wb" bind:value={engineBinary} placeholder={transcribeEngine === "whisper" ? "whisper-cli" : "sherpa-onnx-offline"} />
          {/if}
          {#if transcribeEngine === "whisper"}
            <label>Whisper model file (.bin)</label>
            <div class="pathrow">
              <input bind:value={whisperModel} placeholder="C:\whisper\ggml-large-v3.bin" aria-label="Whisper model file" />
              <button disabled={busy} onclick={browseModel}>Browse model…</button>
            </div>
          {/if}
        </details>
        <h3>…or paste a transcript (estimated timing)</h3>
        <label for="tr">Transcript (conversation)</label>
        <textarea id="tr" class="tall" bind:value={transcript}></textarea>
        <div class="row">
          <div><label for="tg">Target seconds per clip</label><input id="tg" type="number" min="10" max="180" bind:value={targetSecs} /></div>
          <div><label for="pe">Episode number</label><input id="pe" type="number" min="1" bind:value={planEpisode} /></div>
        </div>
        <div class="row">
          <button class="primary" disabled={busy} onclick={makePlan}>Break into clips</button>
          <button disabled={busy || !projectPath} onclick={loadPlan}>Load saved plan</button>
          <button class="accent" disabled={busy || !projectPath || plan.length === 0} onclick={savePlan}>Save plan.json</button>
        </div>
        <label for="mf">Music file for new segments (uploaded music lives with the episode)</label>
        <input id="mf" bind:value={musicFile} />
      </section>

      <section class="panel">
        <h2>Clips ({plan.length})</h2>
        {#if plan.length === 0}<p class="small">No plan yet. Break the transcript into clips — boundaries snap to sentence ends using word timing.</p>{/if}
        {#each plan as clip}
          <article class="clip">
            <header>
              <span>Clip {clip.num}:</span><input class="clip-title" bind:value={clip.title} aria-label="Clip title" />
              <button disabled={busy} onclick={() => addMusic(clip)}>+ Music segment</button>
            </header>
            {#each clip.segments as seg, i}
              <div class="segment">
                {#if seg.type === "music"}
                  <span>Music · <span class="mono">{seg.music_file}</span> · {seg.duration}s · fade {seg.crossfade}s</span>
                {:else}
                  <span class="seg-edit">Speech
                    <input type="number" step="0.1" min="0" bind:value={seg.start} aria-label="Segment start seconds" />–
                    <input type="number" step="0.1" min="0" bind:value={seg.end} aria-label="Segment end seconds" />s
                    <span class="seg-text">{seg.text.slice(0, 80)}…</span>
                  </span>
                {/if}
                <button aria-label="Remove segment" onclick={() => removeSegment(clip, i)}>Remove</button>
              </div>
            {/each}
          </article>
        {/each}
      </section>
    </div>
  {/if}

  {#if view === "publish"}
    <div class="grid">
      <section class="panel">
        <h2>Publish to your repo</h2>
        {#if git}
          <p class="small mono">branch: {git.branch ?? "—"} · remote: {git.remote ?? "none"} · dirty files: {git.dirty.length}</p>
          {#each git.dirty as d}<div class="small mono">{d}</div>{/each}
        {:else}<p class="small">Open a project to see its Git state.</p>{/if}
        <button class="primary" disabled={busy || !projectPath} onclick={publish}>Stage, commit &amp; push</button>
        <button class="ibtn" title="What does Publish do?" onclick={() => showHelp("publish")}>i</button>
        <p class="small">Uses your own Git credentials. The app stores no tokens and never picks a remote for you.</p>
      </section>
      <section class="panel">
        <h2>Channels</h2>
        {#each channels as c}
          <div class="channel"><span>{c.name}</span><span class="badge" data-s={c.status}>{c.status === "live" ? "live" : "on hold"}</span></div>
        {/each}
        <p class="small">mdBook is the web channel. Every other channel is an adapter over the same episode bundle, on hold until the core app is solid.</p>
      </section>
    </div>
  {/if}

  {#if view === "identity"}
    <div class="grid">
      <section class="panel">
        <h2>Nostr publishing identity</h2>
        <p class="small">This signs your narratives and reels. It is not an app login — the app itself has no account.</p>
        {#if identity}
          <label>Your npub</label>
          <p class="mono">{identity.npub}</p>
        {:else}<p class="small">No identity in this project yet.</p>{/if}
        <label for="pw">Identity password (encrypts the secret on this device)</label>
        <input id="pw" type="password" bind:value={password} autocomplete="new-password" />
        <button class="primary" disabled={busy || !projectPath || password.length < 8} onclick={makeIdentity}>Create Nostr identity</button>
        {#if backupNsec}
          <label>Back this up now — shown once, never stored in plain text</label>
          <p class="mono backup">{backupNsec}</p>
          <p class="error">Losing this means losing the identity. There is no reset.</p>
        {/if}
        <button disabled={busy || !identity || password.length < 8} onclick={sign}>Sign episode narrative (NIP-23)</button>
        {#if signedEvent}<pre class="mono event">{signedEvent}</pre>{/if}
      </section>
      <section class="panel">
        <h2>How identity is scoped</h2>
        <p class="small">Git publishing uses your Git credentials; LinkedIn, Substack, Instagram and TikTok will each use their own sign-in when those channels leave hold. One npub ties your public work together across all of them.</p>
      </section>
    </div>
  {/if}

  {#if view === "settings"}
    <div class="grid">
      <section class="panel">
        <h2>Your wallet &amp; platform links</h2>
        <p class="small">These are yours, per project — not the deepDive defaults. <strong>Mine</strong> uses the wallet + audio links for the block above Works Cited; <strong>Sync video carousel</strong> uses the video links. A project with no saved settings falls back to the built-in defaults.</p>
        {#if !projectPath}<p class="small">Open a project first — then load, edit and save its settings.</p>{/if}
        <div class="lens-actions">
          <button disabled={busy || !projectPath} title={!projectPath ? "Open a project first" : "Load this project's saved settings (or the defaults)"} onclick={loadSettings}>Load settings</button>
          <button class="primary" disabled={busy || !projectPath} title={!projectPath ? "Open a project first" : "Save these values to .studio/syndication.json (local, not published)"} onclick={saveSettings}>Save settings</button>
          <button class="ibtn" title="How do settings work?" onclick={() => showHelp("settings")}>i</button>
        </div>
        <label for="set-wallet">Lightning address (donation wallet)</label>
        <input id="set-wallet" bind:value={synd.lightning_address} placeholder="you@primal.net" autocomplete="off" />
        <h3>Audio platforms (added at Mine)</h3>
        <label for="set-spotify">Spotify show URL</label>
        <input id="set-spotify" bind:value={synd.spotify_url} placeholder="https://open.spotify.com/show/…" autocomplete="off" />
        <label for="set-apple">Apple Podcasts URL</label>
        <input id="set-apple" bind:value={synd.apple_podcasts_url} placeholder="https://podcasts.apple.com/…" autocomplete="off" />
        <label for="set-fountain">Fountain.fm URL</label>
        <input id="set-fountain" bind:value={synd.fountain_url} placeholder="https://fountain.fm/show/…" autocomplete="off" />
        <h3>Video platforms (added by Sync video carousel)</h3>
        <label for="set-tiktok">TikTok profile URL</label>
        <input id="set-tiktok" bind:value={synd.tiktok_url} placeholder="https://www.tiktok.com/…" autocomplete="off" />
        <label for="set-instagram">Instagram profile URL</label>
        <input id="set-instagram" bind:value={synd.instagram_url} placeholder="https://www.instagram.com/…" autocomplete="off" />
        <label for="set-youtube">YouTube playlist URL</label>
        <input id="set-youtube" bind:value={synd.youtube_url} placeholder="https://www.youtube.com/playlist?…" autocomplete="off" />
      </section>
      <section class="panel">
        <h2>Where settings live</h2>
        <p class="small">Saved to <span class="mono">.studio/syndication.json</span> in the open project — local app state, like the Nostr identity, and never published to the book or pushed by Publish stages (only <span class="mono">src/</span>, <span class="mono">episodes/</span> and <span class="mono">book.toml</span> are staged). Old episodes are not rewritten: saving new settings only changes what future Mines and carousel syncs embed.</p>
      </section>
    </div>
  {/if}
</main>
<footer class="statusbar" role="status" aria-live="polite">
  {#if busy}
    <span class="spinner" aria-hidden="true"></span><span>{activity}… please wait.</span>
  {:else if error}
    <span class="error">{error}</span>
  {:else if notice}
    <span class="ok">{notice}</span>{#if detail}<span class="detail">{detail}</span>{/if}
  {:else if !project}
    <span class="small">Ready. Start in the Guide: choose a folder, create a fresh project, then bring your research in from the Article tab.</span>
  {:else}
    <span class="small">Ready — project open. Article brings research in, Reels makes clips, Publish ships them.</span>
  {/if}
</footer>
{#if help}
  <div class="helppanel" role="note">
    <p>{help}</p>
    <button onclick={() => (help = "")}>Close help</button>
  </div>
{/if}


<style>
  .topbar { position: sticky; top: 0; z-index: 20; display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 14px 22px; background: var(--panel); border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .brand { font-family: Georgia, serif; font-size: 22px; font-weight: 700; }
  .tagline { font-size: 12px; color: var(--muted); }
  nav { display: flex; gap: 8px; flex-wrap: wrap; }
  nav button.active { background: var(--ink); color: #fff; border-color: var(--ink); }
  .demo-banner { background: #fef3c7; color: var(--accent-ink); padding: 8px 22px; font-size: 13px; border-bottom: 1px solid var(--line); }
  main { max-width: 1180px; margin: 0 auto; padding: 18px 22px 110px; }
  .project-bar { display: flex; gap: 10px; align-items: end; flex-wrap: wrap; background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 12px 14px; }
  .grow { flex: 1 1 220px; }
  .project-bar label { margin-top: 0; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-top: 16px; }
  @media (max-width: 820px) { .grid { grid-template-columns: 1fr; } }
  .panel { background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 16px 18px; }
  .panel h2 { margin: 0 0 10px; font-size: 19px; }
  .clip { border-top: 1px solid var(--line); padding: 10px 0; }
  .ledger-head { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; margin-bottom: 4px; }
  .ledger-head h2 { margin: 0; }
  .ledger-tools { display: flex; align-items: center; gap: 8px; }
  .ledger-tools label { margin: 0; white-space: nowrap; }
  .ledger-tools select { width: auto; height: 32px; padding: 0 8px; font-size: 13px; }
  .episode { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 12px 0; border-top: 1px solid var(--line); }
  .ep-main { min-width: 0; flex: 1 1 auto; }
  .ep-head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .ep-title { font-size: 15px; line-height: 1.3; overflow-wrap: break-word; }
  .ep-meta { color: var(--muted); margin-top: 3px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pill { font-size: 11px; font-weight: 600; line-height: 1; padding: 4px 8px; border-radius: 999px; background: var(--wash); color: var(--muted); text-transform: capitalize; flex: none; }
  .pill[data-stage="mempool"] { background: #fef3c7; color: var(--accent-ink); }
  .pill[data-stage="template"] { background: #dcfce7; color: var(--good); }
  .pill[data-stage="chain"] { background: #e7e5e4; color: var(--ink-2); }
  .pill-clips { background: var(--wash); color: var(--ink-2); }
  .ep-actions { display: flex; align-items: center; gap: 6px; flex: none; flex-wrap: wrap; justify-content: flex-end; }
  .ep-actions button { height: 32px; min-height: 32px; padding: 0 12px; margin: 0; font-size: 13px; line-height: 1; border-radius: 7px; display: inline-flex; align-items: center; justify-content: center; white-space: nowrap; }
  .ep-actions .ibtn { width: 32px; height: 32px; min-height: 32px; padding: 0; border-radius: 7px; flex: none; }
  .ep-actions button.danger { margin-left: 6px; }
  .ep-locked { font-size: 12px; font-weight: 600; color: var(--muted); padding: 0 4px; white-space: nowrap; }
  .ledger-foot { align-items: center; gap: 6px; margin-top: 8px; }
  .ledger-foot button { height: 32px; min-height: 32px; padding: 0 12px; margin: 0; font-size: 13px; line-height: 1; border-radius: 7px; display: inline-flex; align-items: center; }
  .ledger-foot .ibtn { width: 32px; height: 32px; min-height: 32px; padding: 0; border-radius: 7px; }
  .lens-actions { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; margin: 8px 0; }
  .lens-actions button { height: 32px; min-height: 32px; padding: 0 12px; margin: 0; font-size: 13px; line-height: 1; border-radius: 7px; display: inline-flex; align-items: center; justify-content: center; white-space: nowrap; }
  .lens-actions .ibtn { width: 32px; height: 32px; min-height: 32px; padding: 0; border-radius: 7px; flex: none; }
  @media (max-width: 640px) {
    .episode { flex-direction: column; align-items: stretch; gap: 10px; }
    .ep-actions { justify-content: flex-start; }
    .ep-actions button.danger { margin-left: 0; }
  }
  .clip header { display: flex; justify-content: space-between; gap: 10px; align-items: center; flex-wrap: wrap; }
  .segment { display: flex; justify-content: space-between; gap: 10px; font-size: 13px; padding: 6px 0; border-top: 1px dashed var(--line); }
  .row { display: flex; gap: 10px; flex-wrap: wrap; align-items: end; }
  .row > div { flex: 1 1 140px; }
  .row button { margin-top: 10px; }
  .tall { min-height: 150px; }
  .channel { display: flex; justify-content: space-between; padding: 7px 0; border-top: 1px solid var(--line); font-size: 14px; }
  .badge { font-size: 11px; font-weight: 700; padding: 2px 8px; border-radius: 999px; background: var(--wash); color: var(--muted); }
  .badge[data-s="live"] { background: #dcfce7; color: var(--good); }
  .backup { background: #fef3c7; padding: 10px; border-radius: 8px; }
  .event { white-space: pre-wrap; background: var(--wash); padding: 10px; border-radius: 8px; max-height: 240px; overflow: auto; }
  .prompt { max-height: 300px; text-align: left; }
  .statusbar { position: fixed; left: 0; right: 0; bottom: 0; z-index: 40; display: flex; align-items: center; gap: 9px; flex-wrap: wrap; background: var(--panel); border-top: 1px solid var(--line); padding: 9px 18px; font-size: 13px; min-height: 42px; box-shadow: 0 -4px 14px rgba(28, 25, 23, 0.06); }
  .statusbar .detail { flex-basis: 100%; color: var(--ink-2); line-height: 1.45; }
  .helppanel { position: fixed; left: 50%; transform: translateX(-50%); bottom: 58px; z-index: 41; width: min(680px, calc(100vw - 32px)); background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 12px 16px; box-shadow: 0 10px 30px rgba(28, 25, 23, 0.14); font-size: 14px; line-height: 1.5; }
  .helppanel p { margin: 0 0 8px; }
  .ibtn { width: 26px; height: 26px; padding: 0; border-radius: 50%; font-family: Georgia, serif; font-style: italic; font-weight: 700; flex: none; }
  button.danger { color: #b91c1c; border-color: #fecaca; }
  button.danger:hover { background: #fef2f2; }
  .clip-title { flex: 1; font-weight: 600; }
  .seg-edit { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .seg-edit input { width: 84px; }
  .seg-text { color: var(--muted); }
  .spinner { width: 14px; height: 14px; flex: none; border-radius: 50%; border: 2px solid var(--line); border-top-color: var(--accent); animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
  .project-strip { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 9px 14px; }
  .reader { margin-top: 16px; }
  .reader-top { margin-top: 0; margin-bottom: 16px; }
  .reader-head { position: sticky; top: 0; z-index: 10; display: flex; justify-content: space-between; gap: 12px; align-items: start; border-bottom: 1px solid var(--line); padding-bottom: 10px; margin-bottom: 6px; background: var(--panel); }
  .reader-hint { margin-top: 12px; }
  .article-body { line-height: 1.65; font-size: 15px; overflow-wrap: break-word; }
  .article-body h1, .article-body h2, .article-body h3 { line-height: 1.25; }
  .article-body img, .article-body video { max-width: 100%; }
  .article-body pre { background: var(--wash); padding: 12px; border-radius: 8px; overflow: auto; }
  .article-body table { border-collapse: collapse; display: block; overflow-x: auto; }
  .article-body th, .article-body td { border: 1px solid var(--line); padding: 5px 9px; }
  .article-body blockquote { border-left: 3px solid var(--line); margin-left: 0; padding-left: 14px; color: var(--ink-2); }
  .pathrow { display: flex; gap: 8px; }
  .pathrow input { flex: 1; }
  .pathrow button { white-space: nowrap; }
  .stages { padding-left: 20px; }
  .stages li { margin: 7px 0; line-height: 1.45; }
  .panel h3 { margin: 16px 0 6px; font-size: 16px; }
</style>
