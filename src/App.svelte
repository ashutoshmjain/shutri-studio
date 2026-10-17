<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, pickFolder, pickPayloadFile, type Channel, type Clip, type Episode, type GitStatus, type ProjectInfo } from "./lib/api";

  type View = "guide" | "article" | "reels" | "publish" | "identity";
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

  let projectPath = "";
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
    busy = true; activity = label; error = ""; notice = "";
    try { return await fn(); }
    catch (e) { error = `${label} failed: ${e instanceof Error ? e.message : String(e)}`; return null; }
    finally { busy = false; activity = ""; }
  }

  async function refresh() {
    if (!projectPath) return;
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
      notice = `In mempool: "${ep.title}" as _${ep.slug}.md — KaTeX preserved. Find it in the Ledger; Mine & lock it when the audio is final.`;
    }
  }
  async function openProject() {
    const p = await run("Opening project", () => api.inspectProject(projectPath));
    if (p) { project = p; await refresh(); }
  }
  async function createProject() {
    const p = await run("Creating fresh project (folders, SUMMARY.md, git init)", () => api.createProject(projectPath, projectName));
    if (p) { project = p; await refresh(); notice = "Fresh project created: book.toml, src/SUMMARY.md (cover · mempool · template · chain), a _welcome mempool draft and an episodes/ folder — initialised as a Git repo. Next: Article tab, extract your Python payload."; }
  }
  async function createDraft() {
    const ep = await run("Adding draft to mempool", () => api.createDraft(projectPath, slug, title, body));
    if (ep) { slug = ""; title = ""; body = ""; await refresh(); notice = `Draft _${ep.slug}.md is in the mempool — see the Ledger.`; }
  }
  async function promote(ep: Episode) {
    const mined = await run("Mining episode & locking number", () => api.promoteEpisode(projectPath, ep.slug));
    if (mined) { await refresh(); notice = `Mined as episode ${mined.number} — permanent URL assigned. Never renumbered.`; }
  }
  async function build() {
    const log = await run("Building mdBook (this can take a moment)", () => api.buildBook(projectPath));
    if (log !== null) notice = "mdBook build finished. Preview is the book/ output.";
  }
  async function makePlan() {
    const p = await run("Breaking transcript into clips", () => api.planFromTranscript(transcript, targetSecs));
    if (p) plan = p;
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

  onMount(async () => { channels = await api.listChannels().catch(() => []); });
</script>

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

  <div class="statusbar" role="status" aria-live="polite">
    {#if busy}
      <span class="spinner" aria-hidden="true"></span><span>{activity}… please wait.</span>
    {:else if error}
      <span class="error">{error}</span>
    {:else if notice}
      <span class="ok">{notice}</span>
    {:else if !project}
      <span class="small">Ready. Start in the Guide: choose a folder, create a fresh project, then bring your research in from the Article tab.</span>
    {:else}
      <span class="small">Ready — project open. Article brings research in, Reels makes clips, Publish ships them.</span>
    {/if}
  </div>

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
          <li><strong>Mine &amp; lock</strong> when the audio is final: the draft becomes a numbered episode — the permanent URL the Spotify description will link to.</li>
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
        </div>
        {#if !projectPath}<p class="small">Needs an open project — create one in the Guide tab first.</p>
        {:else if !payloadPath}<p class="small">Choose the <span class="mono">.py</span> payload to enable extraction.</p>{/if}
      </section>

      <section class="panel">
        <h2>Ledger</h2>
        {#if episodes.length === 0}<p class="small">Open or create a project to see its mempool and mined episodes.</p>{/if}
        {#each episodes as ep}
          <article class="episode">
            <div>
              <strong>{ep.number ? `#${ep.number} : ` : ""}{ep.title}</strong>
              <div class="small">{ep.stage}{ep.has_clips ? " · has clips" : ""} · <span class="mono">{ep.path}</span></div>
            </div>
            {#if ep.number === null}
              <button class="accent" disabled={busy} onclick={() => promote(ep)}>Mine &amp; lock</button>
            {/if}
          </article>
        {/each}
        <button disabled={busy || !projectPath} onclick={build}>Build mdBook (web channel)</button>
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
        <p class="small">Research is morphed into a narrative conversation first; the KaTeX article derives from it.</p>
      </section>
    </div>
  {/if}

  {#if view === "reels"}
    <div class="grid">
      <section class="panel">
        <h2>Narrative → clips</h2>
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
              <strong>Clip {clip.num}: {clip.title}</strong>
              <button disabled={busy} onclick={() => addMusic(clip)}>+ Music segment</button>
            </header>
            {#each clip.segments as seg, i}
              <div class="segment">
                {#if seg.type === "music"}
                  <span>Music · <span class="mono">{seg.music_file}</span> · {seg.duration}s · fade {seg.crossfade}s</span>
                {:else}
                  <span>Speech · {seg.start.toFixed(1)}–{seg.end.toFixed(1)}s · {seg.text.slice(0, 90)}…</span>
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
</main>

<style>
  .topbar { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 14px 22px; background: var(--panel); border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .brand { font-family: Georgia, serif; font-size: 22px; font-weight: 700; }
  .tagline { font-size: 12px; color: var(--muted); }
  nav { display: flex; gap: 8px; flex-wrap: wrap; }
  nav button.active { background: var(--ink); color: #fff; border-color: var(--ink); }
  .demo-banner { background: #fef3c7; color: var(--accent-ink); padding: 8px 22px; font-size: 13px; border-bottom: 1px solid var(--line); }
  main { max-width: 1180px; margin: 0 auto; padding: 18px 22px 40px; }
  .project-bar { display: flex; gap: 10px; align-items: end; flex-wrap: wrap; background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 12px 14px; }
  .grow { flex: 1 1 220px; }
  .project-bar label { margin-top: 0; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-top: 16px; }
  @media (max-width: 820px) { .grid { grid-template-columns: 1fr; } }
  .panel { background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 16px 18px; }
  .panel h2 { margin: 0 0 10px; font-size: 19px; }
  .episode, .clip { border-top: 1px solid var(--line); padding: 10px 0; }
  .episode { display: flex; justify-content: space-between; gap: 12px; align-items: center; }
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
  .statusbar { display: flex; align-items: center; gap: 9px; margin-top: 12px; background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 9px 14px; font-size: 13px; min-height: 38px; }
  .spinner { width: 14px; height: 14px; flex: none; border-radius: 50%; border: 2px solid var(--line); border-top-color: var(--accent); animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
  .project-strip { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 9px 14px; }
  .pathrow { display: flex; gap: 8px; }
  .pathrow input { flex: 1; }
  .pathrow button { white-space: nowrap; }
  .stages { padding-left: 20px; }
  .stages li { margin: 7px 0; line-height: 1.45; }
  .panel h3 { margin: 16px 0 6px; font-size: 16px; }
</style>
