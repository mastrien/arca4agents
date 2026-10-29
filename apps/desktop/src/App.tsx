import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, Update } from "@tauri-apps/plugin-updater";
import {
  api,
  Authors,
  Board,
  HistoryView,
  Policy,
  ProjectView,
  SessionView,
  Status,
  SyncStatus,
  TaskView,
} from "./api";
import "./App.css";

// O board é relido periodicamente: hooks, MCP, CLI e a sincronização escrevem no mesmo banco.
const REFRESH_MS = 3000;

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
  const [project, setProject] = useState("");
  const [authors, setAuthors] = useState<Authors>("user");
  const [modal, setModal] = useState<Modal>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setBoard(await api.board({ project: project || undefined, authors }));
    } catch (e) {
      setError(errorText(e));
    }
  }, [project, authors]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, REFRESH_MS);
    return () => clearInterval(timer);
  }, [refresh]);

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

  return (
    <div className="app">
      <header className="topbar">
        <h1>Arca</h1>
        <select value={project} onChange={(e) => setProject(e.target.value)} title="Projeto">
          <option value="">Todos os projetos</option>
          {board.projects.map((p) => (
            <option key={p.id} value={p.name}>
              {p.name}
            </option>
          ))}
        </select>
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
        <div className="spacer" />
        <button onClick={() => setModal({ kind: "history" })}>Histórico</button>
        <button onClick={() => setModal({ kind: "projects" })}>Projetos</button>
        <button onClick={() => setModal({ kind: "sync" })}>Sincronização</button>
      </header>

      <UpdateBanner />

      {error && (
        <div className="error" onClick={() => setError(null)} title="Fechar">
          {error}
        </div>
      )}

      <AddBar projects={board.projects} defaultProject={project} onAdd={(input) => run(() => api.addTask(input))} />

      <main className="layout">
        <section className="columns">
          <Column title="Estante" hint="o que você decidiu não priorizar agora" status="SHELF" tasks={board.shelf} run={run} onEdit={(task) => setModal({ kind: "edit", task })} showProject={!project} />
          <Column title="Carrinho" hint="o que você quer manter por perto" status="CART" tasks={board.cart} run={run} onEdit={(task) => setModal({ kind: "edit", task })} showProject={!project} />
          <Column title="Feito hoje" hint="vai para o histórico na virada do dia" status="DONE" tasks={board.done} run={run} onEdit={(task) => setModal({ kind: "edit", task })} showProject={!project} />
        </section>
        <aside className="side">
          <Sessions sessions={board.sessions} />
          <Suggestions board={board} run={run} />
        </aside>
      </main>

      {modal?.kind === "edit" && <EditTask task={modal.task} projects={board.projects} onClose={() => setModal(null)} run={run} />}
      {modal?.kind === "projects" && <Projects projects={board.projects} onClose={() => setModal(null)} run={run} />}
      {modal?.kind === "sync" && <Sync onClose={() => setModal(null)} />}
      {modal?.kind === "history" && <History onClose={() => setModal(null)} />}
    </div>
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
        <button className="primary" onClick={install}>
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
  const [status, setStatus] = useState<Status>("CART");

  useEffect(() => setProject(defaultProject), [defaultProject]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim()) return;
    await onAdd({ title, due: due || undefined, project: project || undefined, daily, status });
    setTitle("");
    setDue("");
    setDaily(false);
  }

  return (
    <form className="addbar" onSubmit={submit}>
      <input className="grow" placeholder="Nova tarefa…" value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
      <input className="due" placeholder="prazo: +3d, amanhã, 2026-11-05" value={due} onChange={(e) => setDue(e.target.value)} />
      <select value={project} onChange={(e) => setProject(e.target.value)}>
        <option value="">sem projeto</option>
        {projects.map((p) => (
          <option key={p.id} value={p.name}>
            {p.name}
          </option>
        ))}
      </select>
      <select value={status} onChange={(e) => setStatus(e.target.value as Status)}>
        <option value="CART">no carrinho</option>
        <option value="SHELF">na estante</option>
      </select>
      <label className="check">
        <input type="checkbox" checked={daily} onChange={(e) => setDaily(e.target.checked)} /> diária
      </label>
      <button type="submit" className="primary">
        Adicionar
      </button>
    </form>
  );
}

// ---------- Colunas e cartões ----------

type Run = (action: () => Promise<unknown>) => Promise<void>;

function Column({ title, hint, status, tasks, run, onEdit, showProject }: { title: string; hint: string; status: Status; tasks: TaskView[]; run: Run; onEdit: (task: TaskView) => void; showProject: boolean }) {
  const [over, setOver] = useState(false);
  return (
    <div
      className={`column ${over ? "over" : ""}`}
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
        <h2>
          {title} <span className="count">{tasks.length}</span>
        </h2>
        <p>{hint}</p>
      </div>
      <div className="cards">
        {tasks.length === 0 && <div className="empty">vazio</div>}
        {tasks.map((task) => (
          <Card key={task.id} task={task} run={run} onEdit={onEdit} showProject={showProject} />
        ))}
      </div>
    </div>
  );
}

function Card({ task, run, onEdit, showProject }: { task: TaskView; run: Run; onEdit: (task: TaskView) => void; showProject: boolean }) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const done = task.status === "DONE";

  return (
    <article
      className={`card ${done ? "done" : ""} ${task.agent ? "by-agent" : ""}`}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("text/arca-task", String(task.id));
        e.dataTransfer.setData("text/arca-status", task.status);
      }}
      onDoubleClick={() => onEdit(task)}
    >
      <div className="card-title">{task.title}</div>
      <div className="tags">
        {task.daily && <span className="tag daily">diária</span>}
        {task.due_label && <span className={`tag due ${task.due_state}`}>{task.due_label}</span>}
        {showProject && task.project && <span className="tag project">@{task.project}</span>}
        {task.agent && <span className="tag agent">{task.agent}</span>}
        {task.working_agents.map((a) => (
          <span key={a} className="tag working">
            ⚙ {a}
          </span>
        ))}
      </div>
      <div className="actions">
        {task.status === "SHELF" && <button onClick={() => run(() => api.moveTask(task.id, "CART"))} title="Pegar para o carrinho">→ carrinho</button>}
        {task.status === "CART" && <button onClick={() => run(() => api.moveTask(task.id, "SHELF"))} title="Devolver à estante">← estante</button>}
        {!done ? (
          <button onClick={() => run(() => api.moveTask(task.id, "DONE"))}>✓ feita</button>
        ) : (
          <button onClick={() => run(() => api.moveTask(task.id, "CART"))}>reabrir</button>
        )}
        <button onClick={() => onEdit(task)}>editar</button>
        {confirmDelete ? (
          <button className="danger" onClick={() => run(() => api.deleteTask(task.id))} onMouseLeave={() => setConfirmDelete(false)}>
            apagar?
          </button>
        ) : (
          <button onClick={() => setConfirmDelete(true)}>apagar</button>
        )}
      </div>
    </article>
  );
}

// ---------- Painel lateral ----------

function Sessions({ sessions }: { sessions: SessionView[] }) {
  const groups = useMemo(() => {
    const map = new Map<string, SessionView[]>();
    for (const s of sessions) {
      const key = s.project ?? "(sem projeto)";
      map.set(key, [...(map.get(key) ?? []), s]);
    }
    return [...map.entries()];
  }, [sessions]);

  return (
    <section className="panel">
      <h3>Agentes</h3>
      {groups.length === 0 && <p className="muted">Nenhuma sessão recente.</p>}
      {groups.map(([name, list]) => (
        <div key={name} className="session-group">
          <div className="session-project">{name}</div>
          {list.map((s) => (
            <div key={s.session_id} className={`session ${s.activity}`}>
              <span className="dot" />
              <div>
                <div>
                  <strong>{s.agent}</strong> {s.activity_label}
                </div>
                <div className="muted small">
                  última atividade {s.last_seen_label}
                  {s.task_title && (
                    <>
                      {" · em "}
                      <em>{s.task_title}</em>
                    </>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      ))}
    </section>
  );
}

function Suggestions({ board, run }: { board: Board; run: Run }) {
  return (
    <section className="panel">
      <h3>
        Sugestões <span className="count">{board.suggestions.length}</span>
      </h3>
      {board.suggestions.length === 0 && <p className="muted">Nada esperando você.</p>}
      {board.suggestions.map((s) => (
        <div key={s.id} className="suggestion">
          <div>
            <strong>{s.agent}</strong> propõe <strong>{s.summary}</strong> em <em>{s.task_title}</em>
          </div>
          {s.reason && <div className="muted small">“{s.reason}”</div>}
          <div className="actions">
            <button className="primary" onClick={() => run(() => api.approve(s.id))}>
              Aprovar
            </button>
            <button onClick={() => run(() => api.reject(s.id))}>Rejeitar</button>
            <span className="muted small">{s.created_label}</span>
          </div>
        </div>
      ))}
    </section>
  );
}

// ---------- Janelas ----------

function Dialog({ title, onClose, children, wide }: { title: string; onClose: () => void; children: React.ReactNode; wide?: boolean }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="backdrop" onMouseDown={onClose}>
      <div className={`dialog ${wide ? "wide" : ""}`} onMouseDown={(e) => e.stopPropagation()}>
        <div className="dialog-head">
          <h2>{title}</h2>
          <button onClick={onClose} aria-label="Fechar">
            ✕
          </button>
        </div>
        {children}
      </div>
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
    <Dialog title={`Tarefa #${task.id}`} onClose={onClose}>
      <form className="form" onSubmit={save}>
        <label>
          Título
          <input value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
        </label>
        <label>
          Prazo (vazio para remover)
          <input value={due} onChange={(e) => setDue(e.target.value)} placeholder="2026-11-05, +3d, amanhã" />
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
        <label className="check">
          <input type="checkbox" checked={daily} onChange={(e) => setDaily(e.target.checked)} /> diária
        </label>
        {task.agent && <p className="muted small">Criada por {task.agent}.</p>}
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
          await run(() => api.addProject(name, policy));
          setName("");
        }}
      >
        <input className="grow" placeholder="Novo projeto" value={name} onChange={(e) => setName(e.target.value)} />
        <select value={policy} onChange={(e) => setPolicy(e.target.value as Policy)}>
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
        <input className="grow" value={name} onChange={(e) => setName(e.target.value)} onBlur={() => name !== project.name && run(() => api.editProject(project.id, { name }))} />
        <select value={project.policy} onChange={(e) => run(() => api.editProject(project.id, { policy: e.target.value as Policy }))}>
          {(Object.keys(POLICY_LABEL) as Policy[]).map((p) => (
            <option key={p} value={p}>
              {POLICY_LABEL[p]}
            </option>
          ))}
        </select>
        <input
          className="minutes"
          placeholder="janela global"
          title='Minutos de "ativo agora" (vazio = janela global)'
          value={windowMinutes}
          onChange={(e) => setWindowMinutes(e.target.value.replace(/\D/g, ""))}
          onBlur={() => run(() => api.editProject(project.id, { active_window: Number(windowMinutes || 0) }))}
        />
        {confirmDelete ? (
          <button className="danger" onClick={() => run(() => api.deleteProject(project.id))} onMouseLeave={() => setConfirmDelete(false)}>
            apagar? (as tarefas ficam)
          </button>
        ) : (
          <button onClick={() => setConfirmDelete(true)}>apagar</button>
        )}
      </div>
      <div className="muted small">
        estante {project.shelf} · carrinho {project.cart} · feitas {project.done}
      </div>
      <div className="folders">
        {project.folders.map((f) => (
          <span key={f} className="folder">
            {f}
            <button onClick={() => run(() => api.unlinkFolder(f))} title="Desvincular">
              ✕
            </button>
          </span>
        ))}
        <button onClick={onChooseFolder}>+ pasta</button>
      </div>
    </div>
  );
}

function Sync({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [pairing, setPairing] = useState<{ svg: string; code: string } | null>(null);
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
    <Dialog title="Sincronização na rede de casa" onClose={onClose} wide>
      {error && <div className="error">{error}</div>}
      {status.error && <div className="error">O servidor parou: {status.error}</div>}
      <label className="check">
        <input
          type="checkbox"
          checked={status.running}
          onChange={async (e) => {
            await api.setSyncEnabled(e.target.checked);
            load();
          }}
        />{" "}
        Servidor ligado (porta {status.port}) em <strong>{status.name}</strong>
      </label>

      {pairing && (
        <div className="pairing">
          <div className="qr" dangerouslySetInnerHTML={{ __html: pairing.svg }} />
          <div>
            <p>No app do celular, leia este QR code para parear. Os dois aparelhos precisam estar na mesma rede.</p>
            <p className="muted small">Endereços: {status.urls.join(", ") || "nenhum encontrado"}</p>
            <details>
              <summary>Código em texto (para `arca sync pair` em outro PC)</summary>
              <textarea readOnly value={pairing.code} rows={4} onFocus={(e) => e.target.select()} />
            </details>
            <button
              onClick={async () => {
                await api.rotateToken();
                load();
              }}
            >
              Trocar token (desfaz os pareamentos)
            </button>
          </div>
        </div>
      )}

      <h3>Atividade</h3>
      <div className="log">{status.log.length === 0 ? <span className="muted">Nada ainda.</span> : [...status.log].reverse().map((l, i) => <div key={i}>{l}</div>)}</div>
    </Dialog>
  );
}

function History({ onClose }: { onClose: () => void }) {
  const [entries, setEntries] = useState<HistoryView[] | null>(null);
  useEffect(() => {
    api.history().then(setEntries);
  }, []);
  return (
    <Dialog title="Histórico" onClose={onClose} wide>
      {!entries && <p className="muted">Carregando…</p>}
      {entries?.length === 0 && <p className="muted">Nada concluído ainda.</p>}
      <div className="history">
        {entries?.map((h) => (
          <div key={h.id} className="history-row">
            <span className="muted small">{h.date}</span>
            <span>{h.title}</span>
            <span className="tags">
              {h.daily && <span className="tag daily">diária</span>}
              {h.project && <span className="tag project">@{h.project}</span>}
              {h.agent && <span className="tag agent">{h.agent}</span>}
            </span>
          </div>
        ))}
      </div>
    </Dialog>
  );
}
