// Typed bridge to the Rust core. Inside Tauri these are in-process
// commands; in a plain browser a small in-memory demo stands in so the
// UI can be tried without the desktop shell (clearly labelled in-app).
import { invoke } from "@tauri-apps/api/core";

export interface ProjectInfo {
  path: string;
  is_git_repo: boolean;
  remote: string | null;
  episode_count: number;
  mempool_count: number;
}
export interface Episode {
  number: number | null;
  slug: string;
  title: string;
  path: string;
  stage: "mempool" | "template";
  has_clips: boolean;
}
export type Segment =
  | { type: "music"; music_file: string; duration: number; crossfade: number; volume: number }
  | { type: "audio"; start: number; end: number; text: string; duration?: number };
export interface Clip {
  num: number;
  title: string;
  bridge_text: string[];
  segments: Segment[];
  locked: boolean;
  hidden?: boolean;
  mosaic_run_id?: string;
  start?: number;
  end?: number;
  duration?: number;
  rendered_duration?: number;
  audio_duration?: number;
  intro_offset?: number;
  outro_duration?: number;
}
export interface BundleReport {
  episode: number;
  dir: string;
  plan_clips: number;
  present_clips: number[];
  missing_clips: number[];
  sha256: Record<number, string>;
  issues: string[];
}
export interface Channel {
  id: string;
  name: string;
  kind: string;
  status: string;
}
export interface GitStatus {
  is_repo: boolean;
  branch: string | null;
  remote: string | null;
  dirty: string[];
}

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Native folder picker (Windows dialog, new-folder button included).
 * Returns null when cancelled or in browser demo mode (type the path). */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false, title: "Choose or create your project folder" });
  return typeof picked === "string" ? picked : null;
}

export async function pickFile(title: string, name: string, extensions: string[]): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: false, multiple: false, title, filters: [{ name, extensions }] });
  return typeof picked === "string" ? picked : null;
}

/** Native picker for the LLM packaging payload (.py). */
export async function pickPayloadFile(): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({
    directory: false,
    multiple: false,
    title: "Choose your research payload (.py from the LLM)",
    filters: [{ name: "Python payload", extensions: ["py"] }],
  });
  return typeof picked === "string" ? picked : null;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return invoke<T>(cmd, args);
  return demo(cmd, args ?? {}) as Promise<T>;
}

// ---- Browser demo (no Tauri): in-memory only, nothing touches disk ----
let demoEpisodes: Episode[] = [
  { number: null, slug: "quantum-memory", title: "Quantum Memory", path: "demo/src/_quantum-memory.md", stage: "mempool", has_clips: false },
  { number: 247, slug: "247", title: "Internal Drowning", path: "demo/src/247.md", stage: "template", has_clips: true },
];
let demoIdentity: { npub: string; pubkey_hex: string } | null = null;

function demoPlan(text: string, target: number): Clip[] {
  const words = text.split(/\s+/).filter(Boolean);
  const stamps = words.map((t, i) => ({ text: t, start: i * 0.4, end: i * 0.4 + 0.36 }));
  const sentences = text.match(/[^.!?]+[.!?]+/g) ?? [text];
  const clips: Clip[] = [];
  let wi = 0;
  let cur: string[] = [];
  let start: number | null = null;
  let end = 0;
  const flush = () => {
    if (!cur.length) return;
    const joined = cur.join(" ");
    clips.push({
      num: clips.length + 1,
      title: joined.split(/\s+/).slice(0, 4).join(" "),
      bridge_text: [],
      segments: [{ type: "audio", start: start ?? 0, end, text: joined }],
      locked: false,
    });
    cur = [];
    start = null;
  };
  for (const s of sentences) {
    const n = s.split(/\s+/).filter(Boolean).length;
    const span = stamps.slice(wi, wi + n);
    wi += n;
    if (start === null && span[0]) start = span[0].start;
    if (span.length) end = span[span.length - 1].end;
    cur.push(s.trim());
    if (end - (start ?? 0) >= target) flush();
  }
  flush();
  return clips;
}

async function demo(cmd: string, a: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "inspect_project":
    case "create_project":
      return { path: String(a.path ?? "demo"), is_git_repo: true, remote: "git@github.com:you/research.git", episode_count: 1, mempool_count: 1 };
    case "clone_project":
      return { path: String(a.dest ?? "demo"), is_git_repo: true, remote: String(a.url ?? ""), episode_count: 0, mempool_count: 0 };
    case "list_episodes":
      return demoEpisodes;
    case "delete_episode":
      return "Demo Draft";
    case "ingest_markdown": {
      const ep: Episode = { number: null, slug: "uploaded-notes", title: "Uploaded Notes (demo)", path: "demo/src/_uploaded-notes.md", stage: "mempool", has_clips: false };
      demoEpisodes = [...demoEpisodes, ep];
      return ep;
    }
    case "open_episode_file":
      return "Demo: would open the file in your editor.";
    case "upload_audio":
      return "demo/episodes/1/audio/demo.mp3";
    case "transcribe_episode":
      return { words: 42, text: "Demo transcript words with timing." };
    case "plan_from_episode":
      return demoPlan("Demo clip one from real timing. And a second sentence here for the plan.", 20);
    case "read_episode":
      return "# Demo episode\n\nRendered preview needs the desktop app. Inline math $E = mc^2$ renders with KaTeX there.\n";
    case "ingest_python": {
      const ep: Episode = { number: null, slug: "ingested-research", title: "Ingested Research (demo)", path: "demo/src/_ingested-research.md", stage: "mempool", has_clips: false };
      demoEpisodes = [...demoEpisodes, ep];
      return ep;
    }
    case "create_draft": {
      const ep: Episode = { number: null, slug: String(a.slug), title: String(a.title), path: `demo/src/_${a.slug}.md`, stage: "mempool", has_clips: false };
      demoEpisodes = [...demoEpisodes, ep];
      return ep;
    }
    case "promote_episode": {
      const n = Math.max(0, ...demoEpisodes.map((e) => e.number ?? 0)) + 1;
      demoEpisodes = demoEpisodes.map((e) =>
        e.slug === a.slug && e.number === null ? { ...e, number: n, slug: String(n), stage: "template" as const } : e,
      );
      return demoEpisodes.find((e) => e.number === n);
    }
    case "build_book":
      return "Demo build OK (mdbook runs in the desktop app).";
    case "start_preview":
      return "http://127.0.0.1:3000";
    case "stop_preview":
      return "Preview server stopped.";
    case "preview_running":
      return false;
    case "git_status":
      return { is_repo: true, branch: "main", remote: "git@github.com:you/research.git", dirty: [] };
    case "git_publish":
      return "Demo: would push to git@github.com:you/research.git (main)";
    case "list_channels":
      return [
        { id: "mdbook", name: "Web (mdBook)", kind: "web", status: "live" },
        { id: "nostr", name: "Nostr long-form (NIP-23)", kind: "narrative", status: "on_hold" },
        { id: "linkedin", name: "LinkedIn", kind: "narrative", status: "on_hold" },
        { id: "substack", name: "Substack", kind: "narrative", status: "on_hold" },
        { id: "instagram", name: "Instagram Reels", kind: "reels", status: "on_hold" },
        { id: "tiktok", name: "TikTok", kind: "reels", status: "on_hold" },
        { id: "youtube", name: "YouTube Shorts", kind: "reels", status: "on_hold" },
        { id: "podcast", name: "Podcast (RSS)", kind: "audio", status: "on_hold" },
      ];
    case "load_plan":
      return [];
    case "save_plan":
      return "demo/plan.json";
    case "validate_bundle":
      return { episode: Number(a.episode), dir: "demo/episodes", plan_clips: 0, present_clips: [], missing_clips: [], sha256: {}, issues: ["demo mode: no bundle on disk"] };
    case "plan_from_transcript":
      return demoPlan(String(a.text ?? ""), Number(a.target_secs ?? 110));
    case "generate_identity":
      demoIdentity = { npub: "npub1demo" + Math.random().toString(16).slice(2, 24), pubkey_hex: "demo" };
      return { ...demoIdentity, nsec_backup: "nsec1demo-backup-only-shown-once" };
    case "load_identity":
      return demoIdentity;
    case "sign_narrative":
      return { id: "demo-event-id", pubkey: demoIdentity?.pubkey_hex ?? "", created_at: Math.floor(Date.now() / 1000), kind: 30023, tags: [["d", String(a.episode)]], content: String(a.markdown), sig: "demo-signature" };
    case "list_jobs":
      return [];
    default:
      throw new Error(`unknown command ${cmd}`);
  }
}

export const api = {
  inspectProject: (path: string) => call<ProjectInfo>("inspect_project", { path }),
  createProject: (path: string, name: string) => call<ProjectInfo>("create_project", { path, name }),
  cloneProject: (url: string, dest: string) => call<ProjectInfo>("clone_project", { url, dest }),
  listEpisodes: (path: string) => call<Episode[]>("list_episodes", { path }),
  createDraft: (path: string, slug: string, title: string, body: string) =>
    call<Episode>("create_draft", { path, slug, title, body }),
  ingestPython: (path: string, source: string) =>
    call<Episode>("ingest_python", { path, source }),
  readEpisode: (path: string, fileName: string) =>
    call<string>("read_episode", { path, fileName }),
  deleteEpisode: (path: string, fileName: string) =>
    call<string>("delete_episode", { path, fileName }),
  ingestMarkdown: (path: string, source: string) =>
    call<Episode>("ingest_markdown", { path, source }),
  openEpisodeFile: (path: string, fileName: string) =>
    call<string>("open_episode_file", { path, fileName }),
  uploadAudio: (path: string, episode: number, source: string) =>
    call<string>("upload_audio", { path, episode, source }),
  transcribeEpisode: (path: string, episode: number, audioFile: string, binary: string, model: string) =>
    call<{ words: number; text: string }>("transcribe_episode", { path, episode, audioFile, binary, model }),
  planFromEpisode: (path: string, episode: number, targetSecs: number) =>
    call<Clip[]>("plan_from_episode", { path, episode, targetSecs }),
  promoteEpisode: (path: string, slug: string) => call<Episode>("promote_episode", { path, slug }),
  buildBook: (path: string) => call<string>("build_book", { path }),
  startPreview: (path: string) => call<string>("start_preview", { path }),
  stopPreview: () => call<string>("stop_preview"),
  previewRunning: () => call<boolean>("preview_running"),
  gitStatus: (path: string) => call<GitStatus>("git_status", { path }),
  gitPublish: (path: string, message: string) => call<string>("git_publish", { path, message }),
  listChannels: () => call<Channel[]>("list_channels"),
  loadPlan: (path: string, episode: number) => call<Clip[]>("load_plan", { path, episode }),
  savePlan: (path: string, episode: number, plan: Clip[]) => call<string>("save_plan", { path, episode, plan }),
  validateBundle: (path: string, episode: number) => call<BundleReport>("validate_bundle", { path, episode }),
  planFromTranscript: (text: string, target_secs: number) => call<Clip[]>("plan_from_transcript", { text, target_secs }),
  generateIdentity: (path: string, password: string) =>
    call<{ npub: string; pubkey_hex: string; nsec_backup: string }>("generate_identity", { path, password }),
  loadIdentity: (path: string) => call<{ npub: string; pubkey_hex: string } | null>("load_identity", { path }),
  signNarrative: (path: string, password: string, episode: number, markdown: string) =>
    call<unknown>("sign_narrative", { path, password, episode, markdown }),
};
