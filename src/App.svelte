<script lang="ts">
  import { onMount } from "svelte";
  import { api, inTauri, type Channel, type Clip, type Episode, type GitStatus, type ProjectInfo } from "./lib/api";

  type View = "article" | "reels" | "publish" | "identity";
  let view: View = "article";

  let projectPath = "";
  let projectName = "MyResearch";
  let cloneUrl = "";
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

  async function run<T>(fn: () => Promise<T>): Promise<T | null> {
    busy = true; error = ""; notice = "";
    try { return await fn(); }
    catch (e) { error = e instanceof Error ? e.message : String(e); return null; }
    finally { busy = false; }
  }

  async function refresh() {
    if (!projectPath) return;
    const eps = await run(() => api.listEpisodes(projectPath));
    if (eps) episodes = eps;
    const g = await run(() => api.gitStatus(projectPath));
    if (g) git = g;
    const id = await run(() => api.loadIdentity(projectPath));
    identity = id ?? null;
  }

  async function openProject() {
    const p = await run(() => api.inspectProject(projectPath));
    if (p) { project = p; await refresh(); }
  }
  async function createProject() {
    const p = await run(() => api.createProject(projectPath, projectName));
    if (p) { project = p; notice = "Project created and initialised as a Git repo."; await refresh(); }
  }
  async function cloneProject() {
    const p = await run(() => api.cloneProject(cloneUrl, projectPath));
    if (p) { project = p; await refresh(); }
  }
  async function createDraft() {
    const ep = await run(() => api.createDraft(projectPath, slug, title, body));
    if (ep) { notice = `Draft _${ep.slug}.md is in the mempool.`; slug = ""; title = ""; body = ""; await refresh(); }
  }
  async function promote(ep: Episode) {
    const mined = await run(() => api.promoteEpisode(projectPath, ep.slug));
    if (mined) { notice = `Mined as episode ${mined.number}.`; await refresh(); }
  }
  async function build() {
    const log = await run(() => api.buildBook(projectPath));
    if (log !== null) notice = "mdBook build finished. Preview is the book/ output.";
  }
  async function makePlan() {
    const p = await run(() => api.planFromTranscript(transcript, targetSecs));
    if (p) plan = p;
  }
  async function savePlan() {
    const out = await run(() => api.savePlan(projectPath, planEpisode, plan));
    if (out) notice = `Plan saved to ${out}`;
  }
  async function loadPlan() {
    const p = await run(() => api.loadPlan(projectPath, planEpisode));
    if (p) plan = p;
  }
  function addMusic(clip: Clip) {
    clip.segments = [
      { type: "music", music_file: musicFile, duration: 5.5, crossfade: 1.3, volume: 1 },
      ...clip.segments,
    ];
    plan = [...plan];
  }
  function removeSegment(clip: Clip, i: number) {
    clip.segments = clip.segments.filter((_, j) => j !== i);
    plan = [...plan];
  }
  async function publish() {
    const msg = `publish: episode ${planEpisode} via Shutri Studio`;
    const out = await run(() => api.gitPublish(projectPath, msg));
    if (out) { notice = out; await refresh(); }
  }
  async function makeIdentity() {
    const id = await run(() => api.generateIdentity(projectPath, password));
    if (id) { identity = { npub: id.npub, pubkey_hex: id.pubkey_hex }; backupNsec = id.nsec_backup; }
  }
  async function sign() {
    const ev = await run(() => api.signNarrative(projectPath, password, planEpisode, `# Episode ${planEpisode}\n\n${transcript}`));
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
  <section class="project-bar">
    <div class="grow">
      <label for="pp">Project folder (your own repo — never deepDive by default)</label>
      <input id="pp" bind:value={projectPath} placeholder="C:\\Users\\you\\MyResearch" />
    </div>
    <button class="primary" disabled={busy || !projectPath} onclick={openProject}>Open</button>
    <button disabled={busy || !projectPath} onclick={createProject}>Create new</button>
    <div class="grow">
      <label for="cu">…or clone your repo</label>
      <input id="cu" bind:value={cloneUrl} placeholder="git@github.com:you/research.git" />
    </div>
    <button disabled={busy || !projectPath || !cloneUrl} onclick={cloneProject}>Clone</button>
  </section>
  {#if project}
    <p class="small mono">{project.path} · episodes {project.episode_count} · mempool {project.mempool_count} · {project.is_git_repo ? `git: ${project.remote ?? "no remote"}` : "not a git repo"}</p>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if notice}<p class="ok" role="status">{notice}</p>{/if}

  {#if view === "article"}
    <div class="grid">
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
        <h2>New mempool draft</h2>
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
</style>
