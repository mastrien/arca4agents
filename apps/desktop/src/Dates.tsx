import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  BirthdayView,
  DatesOverview,
  projectColor,
  ProjectView,
  UpcomingItemView,
  UpcomingKind,
} from "./api";

interface DatesProps {
  projects: ProjectView[];
  onBack: () => void;
  onBoardChanged: () => void;
  initialTab?: "upcoming" | "birthdays";
}

type HorizonFilter = "all" | "7" | "30" | "90";
type KindFilter = "all" | UpcomingKind;

const KIND_LABELS: Record<UpcomingKind, string> = {
  event: "Evento",
  task: "Tarefa",
  birthday: "Aniversário",
};

interface BirthdayFormState {
  mode: "add" | "edit";
  id?: number;
  name: string;
  group: string;
  dateText: string;
  note: string;
}

interface EventFormState {
  id: number;
  title: string;
  date: string;
  project: string;
  note: string;
}

function parseBirthdayInput(raw: string): { day: number; month: number; year?: number } | null {
  const s = raw.trim();
  const dmy = /^(\d{1,2})\/(\d{1,2})(?:\/(\d{4}))?$/.exec(s);
  if (dmy) {
    return {
      day: Number(dmy[1]),
      month: Number(dmy[2]),
      year: dmy[3] ? Number(dmy[3]) : undefined,
    };
  }
  const ymd = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(s);
  if (ymd) {
    return {
      year: Number(ymd[1]),
      month: Number(ymd[2]),
      day: Number(ymd[3]),
    };
  }
  return null;
}

function horizonSections(items: UpcomingItemView[]): { title: string; items: UpcomingItemView[] }[] {
  const buckets: { title: string; match: (days: number) => boolean }[] = [
    { title: "Atrasados", match: (d) => d < 0 },
    { title: "Hoje", match: (d) => d === 0 },
    { title: "Próximos 7 dias", match: (d) => d >= 1 && d <= 7 },
    { title: "Este mês", match: (d) => d >= 8 && d <= 30 },
    { title: "Mais adiante", match: (d) => d > 30 },
  ];
  return buckets
    .map((b) => ({ title: b.title, items: items.filter((item) => b.match(item.days_until)) }))
    .filter((b) => b.items.length > 0);
}

export default function Dates({ projects, onBack, onBoardChanged, initialTab = "upcoming" }: DatesProps) {
  const [tab, setTab] = useState<"upcoming" | "birthdays">(initialTab);
  const [overview, setOverview] = useState<DatesOverview | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [horizon, setHorizon] = useState<HorizonFilter>("all");
  const [kindFilter, setKindFilter] = useState<KindFilter>("all");

  const [newTitle, setNewTitle] = useState("");
  const [newDate, setNewDate] = useState("");
  const [newProject, setNewProject] = useState("");
  const [newNote, setNewNote] = useState("");
  const [busy, setBusy] = useState(false);

  const [groupFilter, setGroupFilter] = useState("");
  const [byDate, setByDate] = useState(false);
  const [bdayForm, setBdayForm] = useState<BirthdayFormState | null>(null);
  const [eventForm, setEventForm] = useState<EventFormState | null>(null);
  const [modalError, setModalError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const maxDays = tab === "upcoming" && horizon !== "all" ? Number(horizon) : undefined;
      const data = await api.datesOverview(maxDays);
      setOverview(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [tab, horizon]);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setBdayForm(null);
        setEventForm(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const filteredUpcoming = useMemo(() => {
    if (!overview) return [];
    if (kindFilter === "all") return overview.upcoming;
    return overview.upcoming.filter((item) => item.kind === kindFilter);
  }, [overview, kindFilter]);

  const sections = useMemo(() => horizonSections(filteredUpcoming), [filteredUpcoming]);

  const distinctGroups = useMemo(() => {
    if (!overview) return [];
    const counts = new Map<string, number>();
    for (const b of overview.birthdays) {
      counts.set(b.group, (counts.get(b.group) ?? 0) + 1);
    }
    return Array.from(counts.entries()).sort((a, b) => a[0].localeCompare(b[0], "pt-BR"));
  }, [overview]);

  const filteredBirthdays = useMemo(() => {
    if (!overview) return [];
    if (!groupFilter) return overview.birthdays;
    return overview.birthdays.filter((b) => b.group === groupFilter);
  }, [overview, groupFilter]);

  const groupedBirthdays = useMemo(() => {
    const map = new Map<string, BirthdayView[]>();
    for (const b of filteredBirthdays) {
      const list = map.get(b.group);
      if (list) list.push(b);
      else map.set(b.group, [b]);
    }
    return Array.from(map.entries()).sort((a, b) => a[0].localeCompare(b[0], "pt-BR"));
  }, [filteredBirthdays]);

  const handleAddEvent = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTitle.trim() || !newDate.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.addTrackedDate({
        title: newTitle.trim(),
        date: newDate.trim(),
        project: newProject || undefined,
        note: newNote.trim() || undefined,
      });
      setNewTitle("");
      setNewDate("");
      setNewProject("");
      setNewNote("");
      await load();
      onBoardChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleDismissEvent = async (id: number) => {
    setBusy(true);
    try {
      await api.deleteTrackedDate(id);
      await load();
      onBoardChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleCompleteTask = async (id: number) => {
    setBusy(true);
    try {
      await api.moveTask(id, "DONE");
      await load();
      onBoardChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const openAddBirthday = () => {
    setModalError(null);
    setBdayForm({
      mode: "add",
      name: "",
      group: groupFilter || "",
      dateText: "",
      note: "",
    });
  };

  const openEditBirthday = (b: BirthdayView) => {
    setModalError(null);
    setBdayForm({
      mode: "edit",
      id: b.id,
      name: b.name,
      group: b.group,
      dateText: b.date_label,
      note: b.note ?? "",
    });
  };

  const saveBirthday = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!bdayForm || busy) return;
    const parsed = parseBirthdayInput(bdayForm.dateText);
    if (!parsed) {
      setModalError("Use o formato DD/MM ou DD/MM/AAAA.");
      return;
    }
    setBusy(true);
    setModalError(null);
    try {
      if (bdayForm.mode === "add") {
        await api.addBirthday({
          name: bdayForm.name.trim(),
          group: bdayForm.group.trim() || undefined,
          month: parsed.month,
          day: parsed.day,
          year: parsed.year,
          note: bdayForm.note.trim() || undefined,
        });
      } else if (bdayForm.id !== undefined) {
        await api.editBirthday(bdayForm.id, {
          name: bdayForm.name.trim(),
          group: bdayForm.group.trim(),
          month: parsed.month,
          day: parsed.day,
          year: parsed.year ?? 0,
          note: bdayForm.note.trim(),
        });
      }
      setBdayForm(null);
      await load();
      onBoardChanged();
    } catch (err) {
      setModalError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const deleteBirthday = async (id: number) => {
    setBusy(true);
    try {
      await api.deleteBirthday(id);
      setBdayForm(null);
      await load();
      onBoardChanged();
    } catch (err) {
      setModalError(String(err));
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const saveEvent = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!eventForm || busy) return;
    setBusy(true);
    setModalError(null);
    try {
      await api.editTrackedDate(eventForm.id, {
        title: eventForm.title.trim(),
        date: eventForm.date.trim(),
        project: eventForm.project,
        note: eventForm.note.trim(),
      });
      setEventForm(null);
      await load();
      onBoardChanged();
    } catch (err) {
      setModalError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="dates-page">
      <div className="stats-head">
        <button className="btn-ghost" type="button" onClick={onBack}>
          ← Quadro
        </button>
        <h2>Datas</h2>
        <div className="segmented" role="tablist" aria-label="Modo de visualização">
          <button
            type="button"
            className={tab === "upcoming" ? "active" : ""}
            onClick={() => setTab("upcoming")}
          >
            Próximas datas
          </button>
          <button
            type="button"
            className={tab === "birthdays" ? "active" : ""}
            onClick={() => setTab("birthdays")}
          >
            Aniversários
          </button>
        </div>
        {tab === "birthdays" && (
          <button className="btn-primary dates-head-cta" type="button" onClick={openAddBirthday}>
            + Aniversário
          </button>
        )}
      </div>

      {error && <div className="dates-error">{error}</div>}

      {tab === "upcoming" ? (
        <>
          <form className="panel dates-quickadd" onSubmit={handleAddEvent}>
            <input
              type="text"
              placeholder="Nova data ou evento (ex.: Consulta médica, Entrega...)"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
            />
            <div className="dates-quickadd-date">
              <input
                type="text"
                placeholder="Data (hoje, +7d, 25/12/2026)"
                value={newDate}
                onChange={(e) => setNewDate(e.target.value)}
              />
              <div className="dates-presets">
                {["hoje", "amanhã", "+7d"].map((preset) => (
                  <button key={preset} type="button" onClick={() => setNewDate(preset)}>
                    {preset}
                  </button>
                ))}
              </div>
            </div>
            <select value={newProject} onChange={(e) => setNewProject(e.target.value)}>
              <option value="">Sem projeto</option>
              {projects.map((p) => (
                <option key={p.id} value={p.name}>
                  {p.name}
                </option>
              ))}
            </select>
            <input
              type="text"
              placeholder="Nota (opcional)"
              value={newNote}
              onChange={(e) => setNewNote(e.target.value)}
            />
            <button className="btn-primary" type="submit" disabled={busy || !newTitle.trim() || !newDate.trim()}>
              Adicionar
            </button>
          </form>

          <div className="dates-filters">
            <div className="dates-chips" role="group" aria-label="Horizonte">
              {(
                [
                  ["all", "Todos"],
                  ["7", "Até 7d"],
                  ["30", "Até 30d"],
                  ["90", "Até 90d"],
                ] as [HorizonFilter, string][]
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  className={`chip ${horizon === value ? "active" : ""}`}
                  onClick={() => setHorizon(value)}
                >
                  {label}
                </button>
              ))}
            </div>

            <div className="dates-chips" role="group" aria-label="Tipo">
              {(
                [
                  ["all", "Tudo"],
                  ["event", "Eventos"],
                  ["task", "Tarefas"],
                  ["birthday", "Aniversários"],
                ] as [KindFilter, string][]
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  className={`chip ${kindFilter === value ? "active" : ""}`}
                  onClick={() => setKindFilter(value)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          {!overview ? (
            <div className="panel dates-empty">Carregando datas…</div>
          ) : sections.length === 0 ? (
            <div className="panel dates-empty">
              Nenhuma data próxima nesse filtro. Adicione um evento acima ou defina prazos nas tarefas.
            </div>
          ) : (
            <div className="dates-sections">
              {sections.map((section) => (
                <section key={section.title} className="panel dates-group">
                  <div className="panel-head">
                    <h3>{section.title}</h3>
                    <span className="dates-count">{section.items.length}</span>
                  </div>
                  <div className="dates-list">
                    {section.items.map((item) => {
                      const bdayMatch =
                        item.kind === "birthday"
                          ? overview.birthdays.find((b) => b.id === item.id)
                          : undefined;
                      return (
                        <div key={`${item.kind}-${item.id}`} className="dates-row">
                          <span className={`kind-badge kind-${item.kind}`}>{KIND_LABELS[item.kind]}</span>
                          <div className="dates-row-main">
                            <div className="dates-row-top">
                              <span className="dates-row-title">{item.title}</span>
                              {item.project && (
                                <span
                                  className="dates-pill"
                                  style={{ "--pill": projectColor(item.project) } as React.CSSProperties}
                                >
                                  <span className="dot" />
                                  {item.project}
                                </span>
                              )}
                              {item.group && <span className="dates-group-pill">{item.group}</span>}
                              {item.turning_age !== null && (
                                <span className="dates-meta">faz {item.turning_age} anos</span>
                              )}
                            </div>
                            {item.note && <div className="dates-row-note">{item.note}</div>}
                          </div>
                          <span className="dates-date">{item.date_label}</span>
                          <span className={`due ${item.due_state}`}>{item.due_label}</span>
                          <div className="dates-row-actions">
                            {item.kind === "event" && (
                              <>
                                <button
                                  type="button"
                                  className="btn-ghost btn-sm"
                                  onClick={() => {
                                    setModalError(null);
                                    setEventForm({
                                      id: item.id,
                                      title: item.title,
                                      date: item.date,
                                      project: item.project ?? "",
                                      note: item.note ?? "",
                                    });
                                  }}
                                >
                                  Editar
                                </button>
                                <button
                                  type="button"
                                  className="btn-ghost btn-sm"
                                  onClick={() => handleDismissEvent(item.id)}
                                  disabled={busy}
                                >
                                  Dispensar
                                </button>
                              </>
                            )}
                            {item.kind === "task" && (
                              <button
                                type="button"
                                className="btn-ghost btn-sm"
                                onClick={() => handleCompleteTask(item.id)}
                                disabled={busy}
                              >
                                Concluir
                              </button>
                            )}
                            {item.kind === "birthday" && bdayMatch && (
                              <button
                                type="button"
                                className="btn-ghost btn-sm"
                                onClick={() => openEditBirthday(bdayMatch)}
                              >
                                Editar
                              </button>
                            )}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </section>
              ))}
            </div>
          )}
        </>
      ) : (
        <>
          <div className="dates-filters">
            <div className="dates-chips" role="group" aria-label="Filtrar por grupo">
              <button
                type="button"
                className={`chip ${groupFilter === "" ? "active" : ""}`}
                onClick={() => setGroupFilter("")}
              >
                Todos ({overview?.birthdays.length ?? 0})
              </button>
              {distinctGroups.map(([name, count]) => (
                <button
                  key={name}
                  type="button"
                  className={`chip ${groupFilter === name ? "active" : ""}`}
                  onClick={() => setGroupFilter(name)}
                >
                  {name} ({count})
                </button>
              ))}
            </div>

            <div className="segmented" role="group" aria-label="Agrupamento">
              <button type="button" className={!byDate ? "active" : ""} onClick={() => setByDate(false)}>
                Por grupo
              </button>
              <button type="button" className={byDate ? "active" : ""} onClick={() => setByDate(true)}>
                Por proximidade
              </button>
            </div>
          </div>

          {!overview ? (
            <div className="panel dates-empty">Carregando aniversários…</div>
          ) : filteredBirthdays.length === 0 ? (
            <div className="panel dates-empty">
              Nenhum aniversário cadastrado ainda. Clique em <strong>+ Aniversário</strong> para começar.
            </div>
          ) : byDate ? (
            <section className="panel dates-group">
              <div className="panel-head">
                <h3>Próximos aniversários</h3>
                <span className="dates-count">{filteredBirthdays.length}</span>
              </div>
              <div className="dates-list">
                {filteredBirthdays.map((b) => (
                  <BirthdayRow key={b.id} item={b} showGroup onEdit={openEditBirthday} onDelete={deleteBirthday} />
                ))}
              </div>
            </section>
          ) : (
            <div className="dates-sections">
              {groupedBirthdays.map(([group, people]) => (
                <section key={group} className="panel dates-group">
                  <div className="panel-head">
                    <h3>{group}</h3>
                    <span className="dates-count">{people.length}</span>
                  </div>
                  <div className="dates-list">
                    {people.map((b) => (
                      <BirthdayRow
                        key={b.id}
                        item={b}
                        showGroup={false}
                        onEdit={openEditBirthday}
                        onDelete={deleteBirthday}
                      />
                    ))}
                  </div>
                </section>
              ))}
            </div>
          )}
        </>
      )}

      {bdayForm && (
        <div className="modal-backdrop" onClick={() => setBdayForm(null)}>
          <form className="dialog" onClick={(e) => e.stopPropagation()} onSubmit={saveBirthday}>
            <h3>{bdayForm.mode === "add" ? "Novo aniversário" : "Editar aniversário"}</h3>
            {modalError && <div className="dates-error">{modalError}</div>}
            <label>
              Nome
              <input
                type="text"
                autoFocus
                required
                placeholder="Nome da pessoa"
                value={bdayForm.name}
                onChange={(e) => setBdayForm({ ...bdayForm, name: e.target.value })}
              />
            </label>
            <label>
              Data (DD/MM ou DD/MM/AAAA)
              <input
                type="text"
                required
                placeholder="ex.: 15/03 ou 15/03/1995"
                value={bdayForm.dateText}
                onChange={(e) => setBdayForm({ ...bdayForm, dateText: e.target.value })}
              />
            </label>
            <label>
              Grupo
              <input
                type="text"
                list="birthday-groups-datalist"
                placeholder="Geral (ou Família, Amigos, Trabalho…)"
                value={bdayForm.group}
                onChange={(e) => setBdayForm({ ...bdayForm, group: e.target.value })}
              />
              <datalist id="birthday-groups-datalist">
                {distinctGroups.map(([g]) => (
                  <option key={g} value={g} />
                ))}
              </datalist>
            </label>
            <label>
              Nota
              <input
                type="text"
                placeholder="Ideia de presente, observação..."
                value={bdayForm.note}
                onChange={(e) => setBdayForm({ ...bdayForm, note: e.target.value })}
              />
            </label>
            <div className="dialog-actions">
              {bdayForm.mode === "edit" && bdayForm.id !== undefined && (
                <button
                  type="button"
                  className="btn-danger"
                  onClick={() => deleteBirthday(bdayForm.id!)}
                  disabled={busy}
                >
                  Apagar
                </button>
              )}
              <button type="button" className="btn-ghost" onClick={() => setBdayForm(null)}>
                Cancelar
              </button>
              <button type="submit" className="btn-primary" disabled={busy}>
                Salvar
              </button>
            </div>
          </form>
        </div>
      )}

      {eventForm && (
        <div className="modal-backdrop" onClick={() => setEventForm(null)}>
          <form className="dialog" onClick={(e) => e.stopPropagation()} onSubmit={saveEvent}>
            <h3>Editar data</h3>
            {modalError && <div className="dates-error">{modalError}</div>}
            <label>
              Título
              <input
                type="text"
                autoFocus
                required
                value={eventForm.title}
                onChange={(e) => setEventForm({ ...eventForm, title: e.target.value })}
              />
            </label>
            <label>
              Data (AAAA-MM-DD, DD/MM/AAAA, hoje, +3d)
              <input
                type="text"
                required
                value={eventForm.date}
                onChange={(e) => setEventForm({ ...eventForm, date: e.target.value })}
              />
            </label>
            <label>
              Projeto
              <select
                value={eventForm.project}
                onChange={(e) => setEventForm({ ...eventForm, project: e.target.value })}
              >
                <option value="">Sem projeto</option>
                {projects.map((p) => (
                  <option key={p.id} value={p.name}>
                    {p.name}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Nota
              <input
                type="text"
                placeholder="Opcional"
                value={eventForm.note}
                onChange={(e) => setEventForm({ ...eventForm, note: e.target.value })}
              />
            </label>
            <div className="dialog-actions">
              <button
                type="button"
                className="btn-danger"
                onClick={async () => {
                  const id = eventForm.id;
                  setEventForm(null);
                  await handleDismissEvent(id);
                }}
                disabled={busy}
              >
                Dispensar
              </button>
              <button type="button" className="btn-ghost" onClick={() => setEventForm(null)}>
                Cancelar
              </button>
              <button type="submit" className="btn-primary" disabled={busy}>
                Salvar
              </button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}

function BirthdayRow({
  item,
  showGroup,
  onEdit,
  onDelete,
}: {
  item: BirthdayView;
  showGroup: boolean;
  onEdit: (b: BirthdayView) => void;
  onDelete: (id: number) => void;
}) {
  return (
    <div className="dates-row">
      <div className="dates-row-main">
        <div className="dates-row-top">
          <span className="dates-row-title">{item.name}</span>
          {showGroup && <span className="dates-group-pill">{item.group}</span>}
          {item.turning_age !== null && <span className="dates-meta">faz {item.turning_age} anos</span>}
        </div>
        {item.note && <div className="dates-row-note">{item.note}</div>}
      </div>
      <span className="dates-date">{item.date_label}</span>
      <span className={`due ${item.due_state === "today" ? "today" : ""}`}>{item.due_label}</span>
      <div className="dates-row-actions">
        <button type="button" className="btn-ghost btn-sm" onClick={() => onEdit(item)}>
          Editar
        </button>
        <button type="button" className="btn-ghost btn-sm" onClick={() => onDelete(item.id)}>
          Apagar
        </button>
      </div>
    </div>
  );
}
