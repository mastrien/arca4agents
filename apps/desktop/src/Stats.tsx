// Página de gráficos: tudo é calculado aqui a partir do histórico, das fotos diárias de
// estante/carrinho e do board sem filtro.
// É uma versão demonstrativa; o que sobreviver à avaliação vai também para o celular.
import { useEffect, useMemo, useState } from "react";
import { api, Board, HistoryView, projectColor, StatusSnapshot } from "./api";
import Usage from "./Usage";

type Granularity = "day" | "week" | "month";

interface DoneEntry {
  day: string; // AAAA-MM-DD
  project: string | null;
  agent: string | null;
  daily: boolean;
}

const MONTHS = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
const WEEKDAYS = ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"];
const NO_PROJECT = "sem projeto";

// ---------- Datas (sempre no fuso local, sem horas) ----------

function parseDay(s: string): Date {
  const [y, m, d] = s.slice(0, 10).split("-").map(Number);
  return new Date(y, m - 1, d);
}

function dayKey(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

function addDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

function startOfWeek(d: Date): Date {
  return addDays(d, -((d.getDay() + 6) % 7)); // semana começa na segunda
}

function ddmm(d: Date): string {
  return `${String(d.getDate()).padStart(2, "0")}/${String(d.getMonth() + 1).padStart(2, "0")}`;
}

interface Bucket {
  label: string; // eixo
  tip: string; // tooltip
  count: number;
}

type Series = "shelf" | "cart" | "done";

// Ordem da pilha, de baixo para cima. As cores (slots 1-3 da paleta validada) ficam no CSS.
const SERIES: { key: Series; label: string }[] = [
  { key: "shelf", label: "Estante" },
  { key: "cart", label: "Carrinho" },
  { key: "done", label: "Concluídas" },
];

interface TimeBucket {
  label: string;
  tip: string;
  done: number; // concluídas durante o período
  shelf: number | null; // tamanho no fim do período; null antes da primeira foto
  cart: number | null;
}

function seriesValue(b: TimeBucket, key: Series): number {
  return (key === "done" ? b.done : b[key]) ?? 0;
}

// Estado da lista no fim do dia `key`: a foto desse dia ou a última antes dele, porque um dia
// sem foto é um dia sem escritas nas tarefas.
function snapshotAt(snapshots: StatusSnapshot[], key: string): StatusSnapshot | null {
  let found: StatusSnapshot | null = null;
  for (const s of snapshots) {
    if (s.date > key) break;
    found = s;
  }
  return found;
}

function timeline(entries: DoneEntry[], snapshots: StatusSnapshot[], today: Date, granularity: Granularity): TimeBucket[] {
  const counts = new Map<string, number>();
  const keyOf = (d: Date) =>
    granularity === "day" ? dayKey(d) : granularity === "week" ? dayKey(startOfWeek(d)) : `${d.getFullYear()}-${d.getMonth()}`;
  for (const e of entries) {
    const k = keyOf(parseDay(e.day));
    counts.set(k, (counts.get(k) ?? 0) + 1);
  }

  const out: TimeBucket[] = [];
  // `start` identifica o período; `end` é o dia cuja foto vale para ele (no máximo hoje).
  const push = (label: string, tip: string, start: Date, end: Date) => {
    const snap = snapshotAt(snapshots, dayKey(end > today ? today : end));
    out.push({ label, tip, done: counts.get(keyOf(start)) ?? 0, shelf: snap?.shelf ?? null, cart: snap?.cart ?? null });
  };
  if (granularity === "day") {
    for (let i = 29; i >= 0; i--) {
      const d = addDays(today, -i);
      push(ddmm(d), `${WEEKDAYS[(d.getDay() + 6) % 7]}, ${ddmm(d)}`, d, d);
    }
  } else if (granularity === "week") {
    const current = startOfWeek(today);
    for (let i = 11; i >= 0; i--) {
      const d = addDays(current, -7 * i);
      push(ddmm(d), `semana de ${ddmm(d)} a ${ddmm(addDays(d, 6))}`, d, addDays(d, 6));
    }
  } else {
    for (let i = 11; i >= 0; i--) {
      const d = new Date(today.getFullYear(), today.getMonth() - i, 1);
      const label = d.getMonth() === 0 || i === 11 ? `${MONTHS[d.getMonth()]} ${String(d.getFullYear()).slice(2)}` : MONTHS[d.getMonth()];
      push(label, `${MONTHS[d.getMonth()]} de ${d.getFullYear()}`, d, new Date(d.getFullYear(), d.getMonth() + 1, 0));
    }
  }
  return out;
}

// Escala do eixo: passos de 1, 2 ou 5 × 10^k, uns 4 intervalos.
function niceScale(max: number): { top: number; ticks: number[] } {
  if (max <= 0) return { top: 4, ticks: [0, 2, 4] };
  const raw = max / 4;
  const pow = 10 ** Math.floor(Math.log10(raw));
  const step = Math.max(1, [1, 2, 5, 10].map((m) => m * pow).find((s) => s >= raw) ?? pow * 10);
  const top = Math.ceil(max / step) * step;
  const ticks: number[] = [];
  for (let t = 0; t <= top; t += step) ticks.push(t);
  return { top, ticks };
}

function percent(part: number, total: number): string {
  if (!total) return "0%";
  const p = (part / total) * 100;
  return p > 0 && p < 1 ? "<1%" : `${Math.round(p)}%`;
}

// ---------- Página ----------

export default function Stats({ onBack }: { onBack: () => void }) {
  const [history, setHistory] = useState<HistoryView[] | null>(null);
  const [snapshots, setSnapshots] = useState<StatusSnapshot[] | null>(null);
  const [board, setBoard] = useState<Board | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [granularity, setGranularity] = useState<Granularity>("week");
  const [includeDaily, setIncludeDaily] = useState(true);
  const [visible, setVisible] = useState<Record<Series, boolean>>({ shelf: true, cart: true, done: true });

  useEffect(() => {
    Promise.all([api.history(), api.statusSnapshots(), api.board({ authors: "all" })])
      .then(([h, s, b]) => {
        setHistory(h);
        setSnapshots(s);
        setBoard(b);
      })
      .catch((e) => setError(String(e)));
  }, []);

  const data = useMemo(() => {
    if (!history || !snapshots || !board) return null;
    const today = parseDay(board.today);
    // O histórico só recebe as concluídas na virada do dia; as de hoje vêm do board.
    const all: DoneEntry[] = [
      ...history.map((h) => ({ day: h.date.slice(0, 10), project: h.project, agent: h.agent, daily: h.daily })),
      ...board.done.map((t) => ({ day: board.today, project: t.project, agent: t.agent, daily: t.daily })),
    ];
    const done = includeDaily ? all : all.filter((e) => !e.daily);

    const thisWeek = dayKey(startOfWeek(today));
    const lastWeek = dayKey(addDays(startOfWeek(today), -7));
    const weekKey = (e: DoneEntry) => dayKey(startOfWeek(parseDay(e.day)));
    const eightWeeksAgo = dayKey(addDays(startOfWeek(today), -56));

    const days = new Set(done.map((e) => e.day));
    let streak = 0;
    let cursor = days.has(board.today) ? today : addDays(today, -1); // hoje ainda pode entrar
    while (days.has(dayKey(cursor))) {
      streak++;
      cursor = addDays(cursor, -1);
    }

    const byWeekday = WEEKDAYS.map((label, i) => ({ label, tip: label, count: 0, i }));
    for (const e of done) byWeekday[(parseDay(e.day).getDay() + 6) % 7].count++;

    return {
      total: done.length,
      thisWeek: done.filter((e) => weekKey(e) === thisWeek).length,
      lastWeek: done.filter((e) => weekKey(e) === lastWeek).length,
      weeklyAverage: done.filter((e) => e.day >= eightWeeksAgo && weekKey(e) !== thisWeek).length / 8,
      streak,
      byAgent: done.filter((e) => e.agent).length,
      timeline: timeline(done, snapshots, today, granularity),
      trackedSince: snapshots[0]?.date ?? null,
      byWeekday,
      doneByProject: groupCount(done.map((e) => e.project)),
      doneByAuthor: groupCount(done.map((e) => e.agent ?? "você")),
      cartByProject: groupCount(board.cart.map((t) => t.project)),
      shelfByProject: groupCount(board.shelf.map((t) => t.project)),
      cartTotal: board.cart.length,
      shelfTotal: board.shelf.length,
    };
  }, [history, snapshots, board, granularity, includeDaily]);

  return (
    <div className="stats">
      <div className="stats-head">
        <button onClick={onBack}>← Quadro</button>
        <h1>Gráficos</h1>
        <span className="muted small">versão de avaliação: diga o que fica e o que sai</span>
        <label className="field stats-daily">
          <input type="checkbox" checked={includeDaily} onChange={(e) => setIncludeDaily(e.target.checked)} />
          contar diárias
        </label>
      </div>

      {error && <div className="error">{error}</div>}
      {!data && !error && <p className="muted">Carregando…</p>}

      {data && (
        <>
          <section className="tiles" aria-label="Resumo">
            <Tile label="Esta semana" value={data.thisWeek} note={`semana passada: ${data.lastWeek}`} />
            <Tile label="Média por semana" value={data.weeklyAverage.toLocaleString("pt-BR", { maximumFractionDigits: 1 })} note="últimas 8 semanas completas" />
            <Tile label="Sequência" value={data.streak} note={data.streak === 1 ? "dia seguido concluindo algo" : "dias seguidos concluindo algo"} />
            <Tile label="Concluídas no total" value={data.total} note={`${percent(data.byAgent, data.total)} criadas por agentes`} />
            <Tile label="Abertas" value={data.cartTotal + data.shelfTotal} note={`${data.cartTotal} no carrinho · ${data.shelfTotal} na estante`} />
          </section>

          <Usage />

          {/* Esquerda: os gráficos de barras verticais. Direita: todos os cards de percentual, empilhados. */}
          <div className="stats-layout">
            <div className="stats-main">
              <section className="panel chart-panel">
                <div className="panel-head">
                  <h2>Ao longo do tempo</h2>
                  <div className="series-toggles" role="group" aria-label="Categorias">
                    {SERIES.map((s) => (
                      <button
                        key={s.key}
                        className={`series-toggle ${visible[s.key] ? "on" : ""}`}
                        aria-pressed={visible[s.key]}
                        onClick={() => setVisible({ ...visible, [s.key]: !visible[s.key] })}
                      >
                        <span className={`swatch ${s.key}`} />
                        {s.label}
                      </button>
                    ))}
                  </div>
                  <div className="segmented small-seg" role="group" aria-label="Agrupar por">
                    {(
                      [
                        ["day", "Dia"],
                        ["week", "Semana"],
                        ["month", "Mês"],
                      ] as [Granularity, string][]
                    ).map(([value, label]) => (
                      <button key={value} className={granularity === value ? "active" : ""} onClick={() => setGranularity(value)}>
                        {label}
                      </button>
                    ))}
                  </div>
                </div>
                <StackedChart buckets={data.timeline} visible={visible} labelEvery={granularity === "day" ? 5 : granularity === "week" ? 2 : 1} />
                {data.trackedSince && (visible.shelf || visible.cart) && (
                  <p className="muted small chart-note">
                    Estante e carrinho: tamanho da lista no fim de cada período, registrado desde {data.trackedSince.split("-").reverse().join("/")}. Concluídas: histórico
                    inteiro.
                  </p>
                )}
              </section>

              <section className="panel chart-panel">
                <div className="panel-head">
                  <h2>Por dia da semana</h2>
                  <span className="hint">desde o início</span>
                </div>
                <BarChart buckets={data.byWeekday} labelEvery={1} compact />
              </section>
            </div>

            <aside className="stats-side">
              <section className="panel chart-panel">
                <div className="panel-head">
                  <h2>Carrinho por projeto</h2>
                  <span className="count">{data.cartTotal}</span>
                </div>
                <RankedBars rows={data.cartByProject} total={data.cartTotal} empty="Carrinho vazio." />
              </section>
              <section className="panel chart-panel">
                <div className="panel-head">
                  <h2>Estante por projeto</h2>
                  <span className="count">{data.shelfTotal}</span>
                </div>
                <RankedBars rows={data.shelfByProject} total={data.shelfTotal} empty="Estante vazia." />
              </section>
              <section className="panel chart-panel">
                <div className="panel-head">
                  <h2>Concluídas por projeto</h2>
                  <span className="hint">desde o início</span>
                </div>
                <RankedBars rows={data.doneByProject} total={data.total} empty="Nada concluído ainda." />
              </section>
              <section className="panel chart-panel">
                <div className="panel-head">
                  <h2>Concluídas por autor da tarefa</h2>
                  <span className="hint">quem criou</span>
                </div>
                <RankedBars rows={data.doneByAuthor} total={data.total} empty="Nada concluído ainda." neutral />
              </section>
            </aside>
          </div>
        </>
      )}
    </div>
  );
}

function groupCount(keys: (string | null)[]): { name: string; count: number; isNone: boolean }[] {
  const map = new Map<string, number>();
  for (const k of keys) map.set(k ?? "", (map.get(k ?? "") ?? 0) + 1);
  return [...map.entries()]
    .map(([name, count]) => ({ name: name || NO_PROJECT, count, isNone: !name }))
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
}

// ---------- Peças ----------

function Tile({ label, value, note }: { label: string; value: number | string; note: string }) {
  return (
    <div className="tile">
      <span className="tile-label">{label}</span>
      <span className="tile-value">{value}</span>
      <span className="muted small">{note}</span>
    </div>
  );
}

// Eixo y, grade e rótulos do eixo x; cada filho é uma coluna (slot) do gráfico.
function ChartFrame({ top, ticks, labels, labelEvery, compact, children }: { top: number; ticks: number[]; labels: string[]; labelEvery: number; compact?: boolean; children: React.ReactNode }) {
  // Rótulos contados a partir do fim: a coluna mais recente sempre tem o seu.
  const showLabel = (i: number) => (labels.length - 1 - i) % labelEvery === 0;
  return (
    <div className={`bar-chart ${compact ? "compact" : ""}`}>
      <div className="bar-y" aria-hidden="true">
        {[...ticks].reverse().map((t) => (
          <span key={t} style={{ bottom: `${(t / top) * 100}%` }}>
            {t}
          </span>
        ))}
      </div>
      <div className="bar-plot">
        {ticks.map((t) => (
          <div key={t} className={`bar-grid ${t === 0 ? "base" : ""}`} style={{ bottom: `${(t / top) * 100}%` }} />
        ))}
        <div className="bar-row">{children}</div>
      </div>
      <div />
      <div className="bar-x" aria-hidden="true">
        {labels.map((label, i) => (
          <span key={i}>{showLabel(i) ? label : ""}</span>
        ))}
      </div>
    </div>
  );
}

// Nas pontas o tooltip abre para dentro, para não sair do painel.
function tipSide(i: number, n: number): string {
  return i < n * 0.2 ? "start" : i >= n * 0.8 ? "end" : "";
}

function BarChart({ buckets, labelEvery, compact }: { buckets: Bucket[]; labelEvery: number; compact?: boolean }) {
  const { top, ticks } = niceScale(Math.max(...buckets.map((b) => b.count)));
  return (
    <ChartFrame top={top} ticks={ticks} labels={buckets.map((b) => b.label)} labelEvery={labelEvery} compact={compact}>
      {buckets.map((b, i) => (
        <div key={i} className="bar-slot" tabIndex={0} aria-label={`${b.tip}: ${b.count}`}>
          <div className="bar" style={{ height: `${(b.count / top) * 100}%` }} />
          <div className={`bar-tip ${tipSide(i, buckets.length)}`} role="tooltip">
            <div>
              <strong>{b.count}</strong> {b.count === 1 ? "concluída" : "concluídas"}
            </div>
            <span>{b.tip}</span>
          </div>
        </div>
      ))}
    </ChartFrame>
  );
}

// Uma coluna por período, com as categorias visíveis empilhadas (estante embaixo).
function StackedChart({ buckets, visible, labelEvery }: { buckets: TimeBucket[]; visible: Record<Series, boolean>; labelEvery: number }) {
  const shown = SERIES.filter((s) => visible[s.key]);
  const totals = buckets.map((b) => shown.reduce((sum, s) => sum + seriesValue(b, s.key), 0));
  const { top, ticks } = niceScale(Math.max(0, ...totals));
  const shownValue = (b: TimeBucket, key: Series) => (key !== "done" && b[key] === null ? "sem registro" : String(seriesValue(b, key)));
  return (
    <ChartFrame top={top} ticks={ticks} labels={buckets.map((b) => b.label)} labelEvery={labelEvery}>
      {buckets.map((b, i) => (
        <div key={i} className="bar-slot" tabIndex={0} aria-label={`${b.tip}: ${shown.map((s) => `${s.label} ${shownValue(b, s.key)}`).join(", ")}`}>
          <div className="stack" style={{ height: `${(totals[i] / top) * 100}%` }}>
            {shown
              .filter((s) => seriesValue(b, s.key) > 0)
              .map((s) => (
                <div key={s.key} className={`seg ${s.key}`} style={{ flexGrow: seriesValue(b, s.key) }} />
              ))}
          </div>
          {shown.length > 0 && (
            <div className={`bar-tip ${tipSide(i, buckets.length)}`} role="tooltip">
              <span>{b.tip}</span>
              {[...shown].reverse().map((s) => (
                <div key={s.key} className="tip-row">
                  <span className={`swatch ${s.key}`} />
                  {s.label}
                  <strong>{shownValue(b, s.key)}</strong>
                </div>
              ))}
            </div>
          )}
        </div>
      ))}
    </ChartFrame>
  );
}

function RankedBars({ rows, total, empty, neutral }: { rows: { name: string; count: number; isNone: boolean }[]; total: number; empty: string; neutral?: boolean }) {
  if (rows.length === 0) return <p className="muted">{empty}</p>;
  const max = Math.max(...rows.map((r) => r.count));
  return (
    <div className="ranked">
      {rows.map((r) => {
        const color = neutral ? "var(--accent)" : r.isNone ? "var(--idle)" : projectColor(r.name);
        return (
          <div key={r.name} className="ranked-row" title={`${r.name}: ${r.count} (${percent(r.count, total)})`}>
            <span className={`ranked-name ${r.isNone ? "muted" : ""}`}>
              {!neutral && <span className="project-dot" style={{ background: color }} />}
              {r.name}
            </span>
            <span className="ranked-track">
              <span className="ranked-fill" style={{ width: `${(r.count / max) * 100}%`, background: color }} />
            </span>
            <span className="ranked-value mono">
              {percent(r.count, total)} <span className="muted">· {r.count}</span>
            </span>
          </div>
        );
      })}
    </div>
  );
}
