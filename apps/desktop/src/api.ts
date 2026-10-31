// Tipos e chamadas do backend (src-tauri/src/lib.rs → arca_core::api).
import { invoke } from "@tauri-apps/api/core";

export type Status = "SHELF" | "CART" | "DONE";
export type Policy = "NONE" | "OWN" | "ALL";
export type Authors = "user" | "all" | "agents";

export interface TaskView {
  id: number;
  title: string;
  status: Status;
  daily: boolean;
  due_date: string | null;
  due_label: string | null;
  due_state: "overdue" | "today" | "upcoming" | null;
  project: string | null;
  created_by: string;
  agent: string | null;
  working_agents: string[];
  done_time: string | null;
}

export interface SessionView {
  session_id: string;
  agent: string;
  project: string | null;
  cwd: string;
  activity: "working" | "possibly_stopped" | "waiting" | "ended";
  activity_label: string;
  last_seen_label: string;
  task_id: number | null;
  task_title: string | null;
}

export interface SuggestionView {
  id: number;
  task_id: number;
  task_title: string;
  agent: string;
  summary: string;
  reason: string | null;
  created_label: string;
}

export interface ProjectView {
  id: number;
  name: string;
  policy: Policy;
  active_window: number | null;
  folders: string[];
  shelf: number;
  cart: number;
  done: number;
}

export interface Board {
  today: string;
  shelf: TaskView[];
  cart: TaskView[];
  done: TaskView[];
  sessions: SessionView[];
  suggestions: SuggestionView[];
  projects: ProjectView[];
  window_minutes: number;
}

export interface HistoryView {
  id: number;
  title: string;
  daily: boolean;
  date: string;
  due_date: string | null;
  project: string | null;
  agent: string | null;
}

// Tamanho da estante e do carrinho no fim de um dia; um dia ausente repete o anterior.
export interface StatusSnapshot {
  date: string;
  shelf: number;
  cart: number;
}

// Uma leitura de cota de um agente (Claude Code pela statusline, Codex pelo app-server).
// Horários locais no formato AAAA-MM-DD HH:MM:SS.
export interface UsageSample {
  agent: string;
  window_key: string;
  used_percent: number;
  window_minutes: number | null;
  resets_at: string | null;
  sampled_at: string;
}

export interface SyncStatus {
  running: boolean;
  port: number;
  name: string;
  device: string;
  urls: string[];
  log: string[];
  clients: { name: string; last_seen_label: string }[];
  error: string | null;
}

export interface TaskInput {
  title: string;
  due?: string;
  project?: string;
  daily?: boolean;
  status?: Status;
}

export interface TaskChanges {
  title?: string;
  due?: string;
  project?: string;
  daily?: boolean;
}

export interface ProjectChanges {
  name?: string;
  policy?: Policy;
  active_window?: number;
}

export const api = {
  board: (filter: { project?: string; authors?: Authors }) => invoke<Board>("board", { filter }),
  addTask: (input: TaskInput) => invoke<number>("add_task", { input }),
  editTask: (id: number, changes: TaskChanges) => invoke<void>("edit_task", { id, changes }),
  moveTask: (id: number, status: Status) => invoke<void>("move_task", { id, status }),
  deleteTask: (id: number) => invoke<void>("delete_task", { id }),
  approve: (id: number) => invoke<void>("approve_suggestion", { id }),
  reject: (id: number) => invoke<void>("reject_suggestion", { id }),
  history: () => invoke<HistoryView[]>("history"),
  statusSnapshots: () => invoke<StatusSnapshot[]>("status_snapshots"),
  usageSamples: (days: number) => invoke<UsageSample[]>("usage_samples", { days }),
  refreshCodexUsage: () => invoke<void>("refresh_codex_usage"),
  addProject: (name: string, policy: Policy) => invoke<number>("add_project", { name, policy }),
  editProject: (id: number, changes: ProjectChanges) => invoke<void>("edit_project", { id, changes }),
  deleteProject: (id: number) => invoke<void>("delete_project", { id }),
  linkFolder: (id: number, folder: string) => invoke<void>("link_folder", { id, folder }),
  unlinkFolder: (folder: string) => invoke<void>("unlink_folder", { folder }),
  syncStatus: () => invoke<SyncStatus>("sync_status"),
  setSyncEnabled: (enabled: boolean) => invoke<void>("set_sync_enabled", { enabled }),
  pairing: () => invoke<{ svg: string; code: string }>("pairing"),
  rotateToken: () => invoke<void>("rotate_token"),
};

// Cor estável por projeto, derivada do nome (o app do celular usa a mesma conta).
const PROJECT_COLORS = ["#3B6FD8", "#C2691B", "#7A4FC4", "#1F8A63", "#C23B6B", "#2A8C9C", "#8C6A1F", "#5A615C"];

export function projectColor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) & 0x7fffffff;
  return PROJECT_COLORS[hash % PROJECT_COLORS.length];
}
