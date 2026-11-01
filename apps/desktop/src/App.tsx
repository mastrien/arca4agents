import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, Update } from "@tauri-apps/plugin-updater";
import {
  api,
  Authors,
  Board,
  HistoryView,
  Policy,
  projectColor,
  ProjectView,
  SessionView,
  Status,
  SyncStatus,
  TaskView,
} from "./api";
import Stats from "./Stats";
import "./App.css";

// O board é relido periodicamente: hooks, MCP, CLI e a sincronização escrevem no mesmo banco.
const REFRESH_MS = 3000;
const AUTHORS_KEY = "arca.authors";

type Modal =
  | { kind: "edit"; task: TaskView }
  | { kind: "projects" }
  | { kind: "sync" }
  | { kind: "history" }
  | null;

function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

export default function App() {
  const [board, setBoard] = useState<Board | null>(null);
  const [sync, setSync] = useState<SyncStatus | null>(null);
  const [project, setProject] = useState("");
  const [authors, setAuthors] = useState<Authors>(() => (localStorage.getItem(AUTHORS_KEY) as Authors) || "all");
  const [modal, setModal] = useState<Modal>(null);
  const [view, setView] = useState<"board" | "stats">("board");
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [b, s] = await Promise.all([api.board({ project: project || undefined, authors }), api.syncStatus()]);
      setBoard(b);
      setSync(s);
    } catch (e) {
      setError(errorText(e));
    }
  }, [project, authors]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, REFRESH_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  useEffect(() => localStorage.setItem(AUTHORS_KEY, authors), [authors]);

  // Toda ação passa por aqui: mostra o erro e relê o board.
  const run = useCallback(
    async (action: () => Promise<unknown>) => {
      try {
        await action();
        setError(null);
      } catch (e) {
        setError(errorText(e));
      }
      await refresh();
    },
    [refresh],
  );

  if (!board) {
    return <div className="loading">{error ?? "Abrindo o Arca…"}</div>;
  }

  const edit = (task: TaskView) => setModal({ kind: "edit", task });

  return (
    <div className={`app ${view === "board" ? "board-view" : ""}`}>
      <header className="topbar">
        <div className="brand">
          <ArcaMark />
          <span className="wordmark">
            Arca<span>4agents</span>
          </span>
          {version && <span className="version">v{version}</span>}
        </div>

        {view === "board" && (
          <>
            <nav className="chips" aria-label="Filtrar por projeto">
              <button className={project === "" ? "active" : ""} onClick={() => setProject("")}>
                Todos
              </button>
              {board.projects.map((p) => (
                <button key={p.id} className={project === p.name ? "active" : ""} onClick={() => setProject(p.name)}>
                  <ProjectDot name={p.name} />
                  {p.name}
                </button>
              ))}
            </nav>

            <div className="segmented" role="group" aria-label="Autores">
              {(
                [
                  ["user", "Minhas"],
                  ["all", "+ agentes"],
                  ["agents", "Só agentes"],
                ] as [Authors, string][]
              ).map(([value, label]) => (
                <button key={value} className={authors === value ? "active" : ""} onClick={() => setAuthors(value)}>
                  {label}
                </button>
              ))}
            </div>
          </>
        )}

        <div className="spacer" />

        <SyncPill status={sync} onClick={() => setModal({ kind: "sync" })} />

        <div className="icon-buttons">
          <button className="icon" aria-label="Projetos" title="Projetos" onClick={() => setModal({ kind: "projects" })}>
            <Icon name="folder" />
          </button>
          <button className="icon" aria-label="Histórico" title="Histórico" onClick={() => setModal({ kind: "history" })}>
            <Icon name="history" />
          </button>
          <button
            className={`icon ${view === "stats" ? "active" : ""}`}
            aria-label="Gráficos"
            title="Gráficos"
            aria-pressed={view === "stats"}
            onClick={() => setView(view === "stats" ? "board" : "stats")}
          >
            <Icon name="chart" />
          </button>
        </div>
      </header>

      <UpdateBanner />

      <main className="content">
        {error && (
          <button className="error" onClick={() => setError(null)} title="Fechar">
            {error}
          </button>
        )}

        {view === "stats" ? (
          <Stats onBack={() => setView("board")} />
        ) : (
          <>
            <AddBar projects={board.projects} defaultProject={project} onAdd={(input) => run(() => api.addTask(input))} />

            <div className="layout">
              <section className="columns" aria-label="Quadro">
                <Column kind="shelf" title="Estante" hint="fica pra depois" status="SHELF" tasks={board.shelf} run={run} onEdit={edit} showProject={!project} />
                <Column kind="cart" title="Carrinho" hint="o que fica por perto" status="CART" tasks={board.cart} run={run} onEdit={edit} showProject={!project} />
                <Column kind="done" title="Feito hoje" hint="vai pro histórico amanhã" status="DONE" tasks={board.done} run={run} onEdit={edit} showProject={!project} />
              </section>
              <aside className="side">
                <Sessions sessions={board.sessions} windowMinutes={board.window_minutes} />
                <Suggestions board={board} run={run} />
              </aside>
            </div>
          </>
        )}
      </main>

      {modal?.kind === "edit" && <EditTask task={modal.task} projects={board.projects} onClose={() => setModal(null)} run={run} />}
      {modal?.kind === "projects" && <Projects projects={board.projects} onClose={() => setModal(null)} run={run} />}
      {modal?.kind === "sync" && <Sync onClose={() => setModal(null)} />}
      {modal?.kind === "history" && <History onClose={() => setModal(null)} />}
    </div>
  );
}

// ---------- Peças pequenas ----------

function ArcaMark() {
  return (
    <svg className="mark" width="26" height="26" viewBox="0 0 26 26" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 10h20v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      <path d="M3 10l3-6h14l3 6" />
      <path d="M10 15h6" />
    </svg>
  );
}

const ICONS: Record<string, React.ReactNode> = {
  folder: <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />,
  history: (
    <>
      <path d="M3 12a9 9 0 1 0 3-6.7" />
      <path d="M3 4v4h4" />
      <path d="M12 8v4l3 2" />
    </>
  ),
  close: <path d="M6 6l12 12M18 6L6 18" />,
  check: <path d="M5 12l5 5L20 7" />,
  repeat: (
    <>
      <path d="M20 12a8 8 0 1 1-2.3-5.7" />
      <path d="M20 4v5h-5" />
    </>
  ),
  bot: (
    <>
      <rect x="5" y="8" width="14" height="11" rx="3" />
      <path d="M12 4v4M9 13h.01M15 13h.01" />
    </>
  ),
  chart: (
    <>
      <path d="M4 20h16" />
      <path d="M7 16v-5M12 16V6M17 16v-8" />
    </>
  ),
  phone: (
    <>
      <rect x="7" y="2" width="10" height="20" rx="2" />
      <path d="M11 18h2" />
    </>
  ),
};

function Icon({ name, size = 18 }: { name: keyof typeof ICONS; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {ICONS[name]}
    </svg>
  );
}

function ProjectDot({ name }: { name: string }) {
  return <span className="project-dot" style={{ background: projectColor(name) }} />;
}

function ProjectTag({ name }: { name: string }) {
  return (
    <span className="meta-project">
      <ProjectDot name={name} />
      {name}
    </span>
  );
}

// Tag com o tom do projeto, usada no histórico.
function ProjectPill({ name }: { name: string }) {
  return (
    <span className="project-pill" style={{ "--project": projectColor(name) } as React.CSSProperties}>
      <ProjectDot name={name} />
      {name}
    </span>
  );
}

function AgentChip({ agent }: { agent: string }) {
  return (
    <span className="agent-chip" title={`Criada por ${agent}`}>
      <Icon name="bot" size={11} />
      {agent}
    </span>
  );
}

function SyncPill({ status, onClick }: { status: SyncStatus | null; onClick: () => void }) {
  if (!status) return null;
  const last = status.clients[0];
  let tone = "ok";
  let label = "Sincronizado";
  let detail = last ? `${last.name} · ${last.last_seen_label}` : "aguardando o celular";
  if (status.error) {
    tone = "warn";
    label = "Sincronização parou";
    detail = "abrir detalhes";
  } else if (!status.running) {
    tone = "off";
    label = "Sincronização desligada";
    detail = "";
  } else if (!last) {
    label = "Sincronização ligada";
  }
  return (
    <button className={`sync-pill ${tone}`} onClick={onClick}>
      <span className="dot" />
      <strong>{label}</strong>
      {detail && <span className="muted">{detail}</span>}
    </button>
  );
}

// ---------- Atualização automática ----------

// Procura uma versão nova no último release do GitHub ao abrir e a cada 6 h.
const UPDATE_CHECK_MS = 6 * 60 * 60 * 1000;

function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<string | null>(null);

  useEffect(() => {
    const look = () => check().then(setUpdate).catch(() => {}); // sem rede: tenta de novo depois
    look();
    const timer = setInterval(look, UPDATE_CHECK_MS);
    return () => clearInterval(timer);
  }, []);

  if (!update) return null;

  async function install() {
    if (!update) return;
    let total = 0;
    let received = 0;
    setProgress("baixando…");
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          received += event.data.chunkLength;
          if (total) setProgress(`baixando ${Math.round((received / total) * 100)}%`);
        }
        if (event.event === "Finished") setProgress("instalando…");
      });
      await relaunch();
    } catch (e) {
      setProgress(`falhou: ${errorText(e)}`);
    }
  }

  return (
    <div className="update">
      <span>
        Nova versão <strong>{update.version}</strong> disponível.
      </span>
      {progress ? (
        <span className="muted">{progress}</span>
      ) : (
        <button className="primary small" onClick={install}>
          Atualizar e reiniciar
        </button>
      )}
    </div>
  );
}

// ---------- Adicionar ----------

function AddBar({ projects, defaultProject, onAdd }: { projects: ProjectView[]; defaultProject: string; onAdd: (input: Parameters<typeof api.addTask>[0]) => Promise<void> }) {
  const [title, setTitle] = useState("");
  const [due, setDue] = useState("");
  const [project, setProject] = useState(defaultProject);
  const [daily, setDaily] = useState(false);

  useEffect(() => setProject(defaultProject), [defaultProject]);

  async function add(status: Status) {
    if (!title.trim()) return;
    await onAdd({ title, due: due || undefined, project: project || undefined, daily, status });
    setTitle("");
    setDue("");
    setDaily(false);
  }

  return (
    <form
      className="addbar"
      onSubmit={(e) => {
        e.preventDefault();
        add("CART");
      }}
    >
      <label htmlFor="nova" className="addbar-label">
        Nova tarefa
      </label>
      <input
        id="nova"
        className="addbar-title"
        placeholder="O que vai para o carrinho?"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && e.shiftKey) {
            e.preventDefault();
            add("SHELF");
          }
        }}
        title="Enter: carrinho · Shift+Enter: estante"
        autoFocus
      />
      <div className="addbar-options">
        <label className="field">
          Prazo
          <input className="mono" placeholder="+3d" value={due} onChange={(e) => setDue(e.target.value)} title="AAAA-MM-DD, hoje, amanhã, +3d, +2w" />
        </label>
        <label className="field select">
          {project && <ProjectDot name={project} />}
          <select value={project} onChange={(e) => setProject(e.target.value)} aria-label="Projeto">
            <option value="">sem projeto</option>
            {projects.map((p) => (
              <option key={p.id} value={p.name}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <input type="checkbox" checked={daily} onChange={(e) => setDaily(e.target.checked)} />
          diária
        </label>
        <button type="button" onClick={() => add("SHELF")}>
          Na estante
        </button>
        <button type="submit" className="primary">
          Adicionar ao carrinho
        </button>
      </div>
    </form>
  );
}

// ---------- Colunas e cartões ----------

type Run = (action: () => Promise<unknown>) => Promise<void>;

function Column({ kind, title, hint, status, tasks, run, onEdit, showProject }: { kind: string; title: string; hint: string; status: Status; tasks: TaskView[]; run: Run; onEdit: (task: TaskView) => void; showProject: boolean }) {
  const [over, setOver] = useState(false);
  return (
    <div
      className={`column ${kind} ${over ? "over" : ""}`}
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setOver(false);
        const id = Number(e.dataTransfer.getData("text/arca-task"));
        const from = e.dataTransfer.getData("text/arca-status");
        if (id && from !== status) run(() => api.moveTask(id, status));
      }}
    >
      <div className="column-head">
        <h2>{title}</h2>
        <span className="count">{tasks.length}</span>
        <span className="hint">{hint}</span>
      </div>
      {/* A lista cresce até o espaço disponível e depois rola; o cabeçalho fica fixo. */}
      <div className="column-body">
        {tasks.length === 0 && <div className="empty">vazio</div>}
        {tasks.map((task, i) =>
          status === "DONE" ? (
            <DoneRow key={task.id} task={task} run={run} showProject={showProject} />
          ) : (
            <Card key={task.id} task={task} position={status === "CART" ? i : null} run={run} onEdit={onEdit} showProject={showProject} />
          ),
        )}
      </div>
    </div>
  );
}

function dragProps(task: TaskView) {
  return {
    draggable: true,
    onDragStart: (e: React.DragEvent) => {
      e.dataTransfer.setData("text/arca-task", String(task.id));
      e.dataTransfer.setData("text/arca-status", task.status);
    },
  };
}

function Card({ task, position, run, onEdit, showProject }: { task: TaskView; position: number | null; run: Run; onEdit: (task: TaskView) => void; showProject: boolean }) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const working = task.working_agents.length > 0;

  return (
    <article className={`card ${working ? "working" : ""}`} {...dragProps(task)} onDoubleClick={() => onEdit(task)}>
      <div className="card-title">
        {position !== null && <span className="position">{position}</span>}
        <span>{task.title}</span>
      </div>
      <div className="meta">
        {showProject && task.project && <ProjectTag name={task.project} />}
        {task.daily && (
          <span className="meta-daily">
            <Icon name="repeat" size={12} />
            diária
          </span>
        )}
        {task.due_label && <span className={`due ${task.due_state ?? ""}`}>{task.due_label}</span>}
        {task.agent && <AgentChip agent={task.agent} />}
        <span className="mono id">#{task.id}</span>
      </div>
      {working && (
        <div className="working-strip">
          <span className="dot" />
          <span>
            <strong>{task.working_agents.join(", ")}</strong> trabalhando nisto
          </span>
        </div>
      )}
      <div className="actions">
        {task.status === "SHELF" && <button onClick={() => run(() => api.moveTask(task.id, "CART"))}>→ carrinho</button>}
        {task.status === "CART" && <button onClick={() => run(() => api.moveTask(task.id, "SHELF"))}>← estante</button>}
        <button className="primary" onClick={() => run(() => api.moveTask(task.id, "DONE"))}>
          Concluir
        </button>
        <button onClick={() => onEdit(task)}>Editar</button>
        {confirmDelete ? (
          <button className="danger" onClick={() => run(() => api.deleteTask(task.id))} onMouseLeave={() => setConfirmDelete(false)}>
            Apagar?
          </button>
        ) : (
          <button onClick={() => setConfirmDelete(true)}>Apagar</button>
        )}
      </div>
    </article>
  );
}

function DoneRow({ task, run, showProject }: { task: TaskView; run: Run; showProject: boolean }) {
  const details = [
    showProject ? task.project : null,
    task.daily ? "diária · volta amanhã" : null,
    task.agent ? `por ${task.agent}` : null,
    task.done_time,
  ].filter(Boolean);
  return (
    <article className="done-row" {...dragProps(task)}>
      <span className="check">
        <Icon name="check" size={16} />
      </span>
      <div className="done-text">
        <span>{task.title}</span>
        {details.length > 0 && <span className="muted small">{details.join(" · ")}</span>}
      </div>
      <button className="reopen" onClick={() => run(() => api.moveTask(task.id, "CART"))} title="Voltar ao carrinho">
        reabrir
      </button>
    </article>
  );
}

// ---------- Painel lateral ----------

function Sessions({ sessions, windowMinutes }: { sessions: SessionView[]; windowMinutes: number }) {
  const waiting = sessions.filter((s) => s.activity === "waiting");
  const groups = useMemo(() => {
    const map = new Map<string, SessionView[]>();
    for (const s of sessions) {
      const key = s.project ?? "";
      map.set(key, [...(map.get(key) ?? []), s]);
    }
    return [...map.entries()];
  }, [sessions]);

  return (
    <section className="panel">
      <div className="panel-head">
        <h2>Agentes</h2>
        <span className="hint">janela de {windowMinutes} min</span>
      </div>

      {/* Como nas colunas: a lista rola dentro do card, com o cabeçalho fixo. */}
      <div className="panel-body">
        {waiting.map((s) => (
          <div key={s.session_id} className="attention">
            <span className="dot" />
            <div>
              <div>
                <strong>{s.agent}</strong> está esperando você
              </div>
              <div className="small" title={s.cwd}>
                {s.project ?? s.origin ?? "sem projeto"} · {s.last_seen_label}
              </div>
              <RemoteLink session={s} />
            </div>
          </div>
        ))}

        {groups.length === 0 && <p className="muted">Nenhuma sessão recente.</p>}
        {groups.map(([name, list]) => (
          <div key={name} className="session-group">
            <div className="session-project">
              {name ? (
                <>
                  <ProjectDot name={name} />
                  {name}
                </>
              ) : (
                "sem projeto"
              )}
            </div>
            {list.map((s) => (
              <div key={s.session_id} className={`session ${s.activity}`}>
                <span className="dot" />
                <div className="session-text">
                  <div>
                    <strong>{s.agent}</strong> <span className="state">{s.activity_label}</span>
                  </div>
                  <div className="muted small">
                    {s.task_id && s.task_title ? `em #${s.task_id} ${s.task_title} · ` : ""}
                    {s.activity === "possibly_stopped" ? `último sinal ${s.last_seen_label}` : s.last_seen_label}
                  </div>
                  {s.origin && (
                    <div className="muted small session-origin" title={s.cwd}>
                      origem: {s.origin}
                    </div>
                  )}
                  {s.activity !== "ended" && <RemoteLink session={s} />}
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>
    </section>
  );
}

// Abre a sessão no controle remoto nativo do agente (hoje, o Remote Control do Claude Code).
function RemoteLink({ session }: { session: SessionView }) {
  if (!session.remote_url) return null;
  const url = session.remote_url;
  return (
    <button className="link-button small" title={url} onClick={() => openUrl(url).catch(() => {})}>
      Abrir no Claude ↗
    </button>
  );
}

function Suggestions({ board, run }: { board: Board; run: Run }) {
  return (
    <section className="panel">
      <div className="panel-head">
        <h2>Sugestões</h2>
        <span className="count">{board.suggestions.length}</span>
      </div>
      <div className="panel-body">
        {board.suggestions.length === 0 && <p className="muted">Nada esperando você.</p>}
        {board.suggestions.map((s) => (
          <div key={s.id} className="suggestion">
            <div>
              <strong>{s.agent}</strong> propõe <strong>{s.summary}</strong> #{s.task_id} {s.task_title}
            </div>
            {s.reason && <div className="reason">“{s.reason}”</div>}
            <div className="actions always">
              <button className="primary" onClick={() => run(() => api.approve(s.id))}>
                Aprovar
              </button>
              <button onClick={() => run(() => api.reject(s.id))}>Rejeitar</button>
              <span className="muted small">{s.created_label}</span>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

// ---------- Janelas ----------

function Dialog({ title, subtitle, onClose, children, wide }: { title: string; subtitle?: string; onClose: () => void; children: React.ReactNode; wide?: boolean }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="backdrop" onMouseDown={onClose}>
      <section role="dialog" aria-label={title} className={`dialog ${wide ? "wide" : ""}`} onMouseDown={(e) => e.stopPropagation()}>
        <div className="dialog-head">
          <h2>{title}</h2>
          {subtitle && <span className="muted">{subtitle}</span>}
          <button className="icon" onClick={onClose} aria-label="Fechar">
            <Icon name="close" />
          </button>
        </div>
        <div className="dialog-body">{children}</div>
      </section>
    </div>
  );
}

function EditTask({ task, projects, onClose, run }: { task: TaskView; projects: ProjectView[]; onClose: () => void; run: Run }) {
  const [title, setTitle] = useState(task.title);
  const [due, setDue] = useState(task.due_date ?? "");
  const [project, setProject] = useState(task.project ?? "");
  const [daily, setDaily] = useState(task.daily);

  async function save(e: React.FormEvent) {
    e.preventDefault();
    await run(() => api.editTask(task.id, { title, due, project, daily }));
    onClose();
  }

  return (
    <Dialog title={`Tarefa #${task.id}`} subtitle={task.agent ? `criada por ${task.agent}` : undefined} onClose={onClose}>
      <form className="form" onSubmit={save}>
        <label>
          Título
          <input value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
        </label>
        <label>
          Prazo <span className="muted">· só referência; vazio para remover</span>
          <input className="mono" value={due} onChange={(e) => setDue(e.target.value)} placeholder="2026-11-05, +3d, amanhã" />
        </label>
        <label>
          Projeto
          <select value={project} onChange={(e) => setProject(e.target.value)}>
            <option value="">sem projeto</option>
            {projects.map((p) => (
              <option key={p.id} value={p.name}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <label className="inline">
          <input type="checkbox" checked={daily} onChange={(e) => setDaily(e.target.checked)} /> Diária: volta ao carrinho todo dia depois de feita
        </label>
        <div className="dialog-actions">
          <button type="button" onClick={onClose}>
            Cancelar
          </button>
          <button type="submit" className="primary">
            Salvar
          </button>
        </div>
      </form>
    </Dialog>
  );
}

const POLICY_LABEL: Record<Policy, string> = {
  NONE: "NONE: agentes só leem",
  OWN: "OWN: agentes só mexem no que criaram",
  ALL: "ALL: agentes gerenciam tudo",
};

function Projects({ projects, onClose, run }: { projects: ProjectView[]; onClose: () => void; run: Run }) {
  const [name, setName] = useState("");
  const [policy, setPolicy] = useState<Policy>("OWN");

  async function chooseFolder(project: ProjectView) {
    const folder = await openDialog({ directory: true, title: `Vincular pasta a ${project.name}` });
    if (typeof folder === "string") await run(() => api.linkFolder(project.id, folder));
  }

  return (
    <Dialog title="Projetos" onClose={onClose} wide>
      <form
        className="row"
        onSubmit={async (e) => {
          e.preventDefault();
          if (!name.trim()) return;
          await run(() => api.addProject(name, policy));
          setName("");
        }}
      >
        <input className="grow" placeholder="Novo projeto" value={name} onChange={(e) => setName(e.target.value)} aria-label="Nome do novo projeto" />
        <select value={policy} onChange={(e) => setPolicy(e.target.value as Policy)} aria-label="Política de agentes">
          {(Object.keys(POLICY_LABEL) as Policy[]).map((p) => (
            <option key={p} value={p}>
              {POLICY_LABEL[p]}
            </option>
          ))}
        </select>
        <button type="submit" className="primary">
          Criar
        </button>
      </form>

      {projects.length === 0 && <p className="muted">Nenhum projeto ainda.</p>}
      {projects.map((p) => (
        <ProjectRow key={p.id} project={p} run={run} onChooseFolder={() => chooseFolder(p)} />
      ))}
    </Dialog>
  );
}

function ProjectRow({ project, run, onChooseFolder }: { project: ProjectView; run: Run; onChooseFolder: () => void }) {
  const [name, setName] = useState(project.name);
  const [windowMinutes, setWindowMinutes] = useState(project.active_window ? String(project.active_window) : "");
  const [confirmDelete, setConfirmDelete] = useState(false);

  return (
    <div className="project">
      <div className="row">
        <ProjectDot name={project.name} />
        <input className="grow" value={name} onChange={(e) => setName(e.target.value)} onBlur={() => name !== project.name && run(() => api.editProject(project.id, { name }))} aria-label="Nome do projeto" />
        <select value={project.policy} onChange={(e) => run(() => api.editProject(project.id, { policy: e.target.value as Policy }))} aria-label="Política de agentes">
          {(Object.keys(POLICY_LABEL) as Policy[]).map((p) => (
            <option key={p} value={p}>
              {POLICY_LABEL[p]}
            </option>
          ))}
        </select>
        <input
          className="minutes mono"
          placeholder="global"
          title='Minutos de "trabalhando" (vazio = janela global)'
          aria-label="Janela de atividade em minutos"
          value={windowMinutes}
          onChange={(e) => setWindowMinutes(e.target.value.replace(/\D/g, ""))}
          onBlur={() => run(() => api.editProject(project.id, { active_window: Number(windowMinutes || 0) }))}
        />
        {confirmDelete ? (
          <button className="danger" onClick={() => run(() => api.deleteProject(project.id))} onMouseLeave={() => setConfirmDelete(false)}>
            Apagar? (as tarefas ficam)
          </button>
        ) : (
          <button onClick={() => setConfirmDelete(true)}>Apagar</button>
        )}
      </div>
      <div className="muted small">
        estante {project.shelf} · carrinho {project.cart} · feitas {project.done}
      </div>
      <div className="folders">
        {project.folders.map((f) => (
          <span key={f} className="folder mono">
            {f}
            <button className="icon tiny" onClick={() => run(() => api.unlinkFolder(f))} aria-label={`Desvincular ${f}`}>
              <Icon name="close" size={12} />
            </button>
          </span>
        ))}
        <button className="small" onClick={onChooseFolder}>
          + pasta
        </button>
      </div>
    </div>
  );
}

function Sync({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [pairing, setPairing] = useState<{ svg: string; code: string } | null>(null);
  const [confirmRotate, setConfirmRotate] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const s = await api.syncStatus();
      setStatus(s);
      setPairing(s.running ? await api.pairing() : null);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  useEffect(() => {
    load();
    const timer = setInterval(load, REFRESH_MS);
    return () => clearInterval(timer);
  }, [load]);

  if (!status) return null;

  return (
    <Dialog title="Sincronização" subtitle="só na rede de casa" onClose={onClose} wide>
      {error && <div className="error">{error}</div>}
      {status.error && <div className="error">O servidor parou: {status.error}</div>}

      <label className="toggle-row">
        <span className="toggle-text">
          <strong>Este PC recebe sincronizações</strong>
          <span className="muted">
            Escutando na porta <span className="mono">{status.port}</span> · o celular sincroniza ao abrir e a cada minuto
          </span>
        </span>
        <input
          type="checkbox"
          className="switch"
          checked={status.running}
          onChange={async (e) => {
            await api.setSyncEnabled(e.target.checked);
            load();
          }}
        />
      </label>

      {pairing && (
        <div className="pairing">
          <div className="qr" role="img" aria-label="QR code de pareamento" dangerouslySetInnerHTML={{ __html: pairing.svg }} />
          <div className="pairing-text">
            <h3>Parear o celular</h3>
            <ol>
              <li>
                No celular, abra o Arca › Mais › <strong>Ler QR code do PC</strong>.
              </li>
              <li>Os dois aparelhos precisam estar no mesmo Wi-Fi.</li>
              <li>Depois disso, sincroniza sozinho.</li>
            </ol>
            <div className="addresses">
              <span className="label">Endereços deste PC</span>
              <div>
                {status.urls.length === 0 && <span className="muted">nenhum encontrado</span>}
                {status.urls.map((u) => (
                  <span key={u} className="address mono">
                    {u.replace(/^https?:\/\//, "")}
                  </span>
                ))}
              </div>
            </div>
            <details>
              <summary>
                Código em texto, para <span className="mono">arca sync pair</span> em outro PC
              </summary>
              <textarea className="mono" readOnly value={pairing.code} rows={4} onFocus={(e) => e.target.select()} />
            </details>
          </div>
        </div>
      )}

      <h3>Aparelhos que sincronizaram</h3>
      {status.clients.length === 0 && <p className="muted">Nenhum desde que o app abriu.</p>}
      {status.clients.map((c) => (
        <div key={c.name} className="client">
          <Icon name="phone" size={20} />
          <div className="client-text">
            <span>{c.name}</span>
            <span className="muted small">última sincronização {c.last_seen_label}</span>
          </div>
        </div>
      ))}

      <details className="log-details">
        <summary>Atividade</summary>
        <div className="log mono">{status.log.length === 0 ? <span className="muted">Nada ainda.</span> : [...status.log].reverse().map((l, i) => <div key={i}>{l}</div>)}</div>
      </details>

      <div className="danger-zone">
        <span className="muted">Perdeu o celular? Um novo token desfaz todos os pareamentos.</span>
        {confirmRotate ? (
          <button
            className="danger"
            onClick={async () => {
              await api.rotateToken();
              setConfirmRotate(false);
              load();
            }}
            onMouseLeave={() => setConfirmRotate(false)}
          >
            Confirmar novo token
          </button>
        ) : (
          <button className="warn" onClick={() => setConfirmRotate(true)}>
            Gerar novo token
          </button>
        )}
      </div>
    </Dialog>
  );
}

const HISTORY_PAGE = 20;

function History({ onClose }: { onClose: () => void }) {
  const [entries, setEntries] = useState<HistoryView[] | null>(null);
  const [page, setPage] = useState(0);
  useEffect(() => {
    api.history().then(setEntries);
  }, []);

  const pages = entries ? Math.max(1, Math.ceil(entries.length / HISTORY_PAGE)) : 1;
  const shown = entries?.slice(page * HISTORY_PAGE, (page + 1) * HISTORY_PAGE);

  return (
    <Dialog title="Histórico" subtitle={entries?.length ? `${entries.length} concluídas` : undefined} onClose={onClose} wide>
      {!entries && <p className="muted">Carregando…</p>}
      {entries?.length === 0 && <p className="muted">Nada concluído ainda.</p>}
      <div className="history">
        {shown?.map((h) => (
          <div key={h.id} className="history-row">
            <span className="mono muted small" title={h.date}>
              {h.date.slice(0, 10).split("-").reverse().join("/")}
            </span>
            <span className="history-title">
              {h.title}
              {h.project && <ProjectPill name={h.project} />}
            </span>
            <span className="meta">
              {h.daily && <span className="meta-daily">diária</span>}
              {h.agent && <AgentChip agent={h.agent} />}
            </span>
          </div>
        ))}
      </div>
      {entries && pages > 1 && (
        <nav className="pager" aria-label="Páginas do histórico">
          <button className="small" disabled={page === 0} onClick={() => setPage(0)} aria-label="Primeira página">
            «
          </button>
          <button className="small" disabled={page === 0} onClick={() => setPage(page - 1)}>
            ‹ Anterior
          </button>
          <span className="muted small mono">
            {page * HISTORY_PAGE + 1}–{Math.min((page + 1) * HISTORY_PAGE, entries.length)} de {entries.length}
          </span>
          <button className="small" disabled={page >= pages - 1} onClick={() => setPage(page + 1)}>
            Próxima ›
          </button>
          <button className="small" disabled={page >= pages - 1} onClick={() => setPage(pages - 1)} aria-label="Última página">
            »
          </button>
        </nav>
      )}
    </Dialog>
  );
}
