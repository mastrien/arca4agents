// Cota de uso dos agentes: um card por janela de limite (Claude Code 5h/7d, Codex ...), com o
// valor atual, quando zera e a linha em degraus ao longo do tempo. Uma série por gráfico, então
// sem legenda: o título do card diz o que é.
import { useEffect, useMemo, useState } from "react";
import { api, UsageSample } from "./api";

type Range = "day" | "week" | "month";

const RANGES: { key: Range; label: string; hours: number }[] = [
  { key: "day", label: "24 h", hours: 24 },
  { key: "week", label: "7 dias", hours: 24 * 7 },
  { key: "month", label: "30 dias", hours: 24 * 30 },
];

const AGENT_NAMES: Record<string, string> = { "claude-code": "Claude Code", codex: "Codex" };
const WEEKDAYS = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
const HIGH = 80; // a partir daqui o card avisa que está perto do limite

// ---------- Tempo ----------

function parseTime(s: string): number {
  const [d, t = "00:00:00"] = s.split(" ");
  const [y, m, day] = d.split("-").map(Number);
  const [h, min, sec] = t.split(":").map(Number);
  return new Date(y, m - 1, day, h, min, sec).getTime();
}

const p2 = (n: number) => String(n).padStart(2, "0");

function hhmm(t: number): string {
  const d = new Date(t);
  return `${p2(d.getHours())}:${p2(d.getMinutes())}`;
}

function dayLabel(t: number): string {
  const d = new Date(t);
  return `${WEEKDAYS[d.getDay()]} ${p2(d.getDate())}/${p2(d.getMonth() + 1)}`;
}

function duration(ms: number): string {
  const minutes = Math.max(0, Math.round(ms / 60000));
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor((minutes % 1440) / 60);
  const mins = minutes % 60;
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${mins}min`;
  return `${mins}min`;
}

function windowLabel(key: string, minutes: number | null): string {
  if (minutes === null) return key;
  const base = minutes % 1440 === 0 ? `${minutes / 1440} dias` : minutes % 60 === 0 ? `${minutes / 60} h` : `${minutes} min`;
  const extra = key.includes(":") ? key.split(":")[0] : key.startsWith("seven_day_") ? key.slice("seven_day_".length) : null;
  return extra ? `${base} · ${extra}` : base;
}

// ---------- Séries ----------

interface Point {
  t: number;
  v: number;
}

interface Series {
  id: string;
  agent: string;
  title: string;
  windowMinutes: number;
  latest: UsageSample;
  // Degraus: o valor de cada ponto vale até o próximo. As viradas conhecidas (resets_at)
  // entram como pontos em 0, porque a cota zera de fato nessa hora.
  points: Point[];
}

function buildSeries(samples: UsageSample[], now: number): Series[] {
  const groups = new Map<string, UsageSample[]>();
  for (const s of samples) {
    const id = `${s.agent}|${s.window_key}`;
    groups.set(id, [...(groups.get(id) ?? []), s]);
  }
  const out: Series[] = [];
  for (const [id, list] of groups) {
    const points: Point[] = [];
    list.forEach((s, i) => {
      const t = parseTime(s.sampled_at);
      points.push({ t, v: s.used_percent });
      const reset = s.resets_at ? parseTime(s.resets_at) : null;
      const next = i + 1 < list.length ? parseTime(list[i + 1].sampled_at) : now;
      if (reset !== null && reset > t && reset < next) points.push({ t: reset, v: 0 });
    });
    const latest = list[list.length - 1];
    out.push({
      id,
      agent: latest.agent,
      title: `${AGENT_NAMES[latest.agent] ?? latest.agent} · ${windowLabel(latest.window_key, latest.window_minutes)}`,
      windowMinutes: latest.window_minutes ?? Number.MAX_SAFE_INTEGER,
      latest,
      points,
    });
  }
  return out.sort((a, b) => a.agent.localeCompare(b.agent) || a.windowMinutes - b.windowMinutes);
}

// Valor em `t`: o último ponto em ou antes dele; null antes da primeira leitura.
function valueAt(points: Point[], t: number): number | null {
  let found: number | null = null;
  for (const p of points) {
    if (p.t > t) break;
    found = p.v;
  }
  return found;
}

// ---------- Componente ----------

export default function Usage() {
  const [samples, setSamples] = useState<UsageSample[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [codexError, setCodexError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [range, setRange] = useState<Range>("week");
  const [now, setNow] = useState(Date.now());

  const load = () =>
    api
      .usageSamples(30)
      .then((s) => {
        setSamples(s);
        setNow(Date.now());
      })
      .catch((e) => setError(String(e)));

  const refreshCodex = () => {
    setRefreshing(true);
    setCodexError(null);
    api
      .refreshCodexUsage()
      .catch((e) => setCodexError(String(e)))
      .finally(() => {
        setRefreshing(false);
        load();
      });
  };

  useEffect(() => {
    load();
    refreshCodex();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const series = useMemo(() => (samples ? buildSeries(samples, now) : []), [samples, now]);
  const hours = RANGES.find((r) => r.key === range)!.hours;
  const from = now - hours * 3600 * 1000;

  return (
    <section className="panel chart-panel usage-panel">
      <div className="panel-head">
        <h2>Cota dos agentes</h2>
        <span className="hint">percentual usado de cada janela de limite</span>
        <div className="segmented small-seg" role="group" aria-label="Período">
          {RANGES.map((r) => (
            <button key={r.key} className={range === r.key ? "active" : ""} onClick={() => setRange(r.key)}>
              {r.label}
            </button>
          ))}
        </div>
        <button className="small" onClick={refreshCodex} disabled={refreshing}>
          {refreshing ? "Lendo o Codex…" : "Ler o Codex agora"}
        </button>
      </div>

      {error && <div className="error">{error}</div>}
      {samples && series.length === 0 && (
        <p className="muted">
          Nenhuma leitura ainda. O Claude Code manda a cota pela statusline (<code>arca usage statusline</code>); o Codex é lido pelo app a cada 15 min.
        </p>
      )}

      {series.length > 0 && (
        <div className="usage-grid">
          {series.map((s) => (
            <UsageCard key={s.id} series={s} from={from} now={now} range={range} />
          ))}
        </div>
      )}
      {codexError && <p className="muted small chart-note">Codex: {codexError}</p>}
    </section>
  );
}

function UsageCard({ series, from, now, range }: { series: Series; from: number; now: number; range: Range }) {
  const { latest } = series;
  const reset = latest.resets_at ? parseTime(latest.resets_at) : null;
  const resetPassed = reset !== null && reset <= now;
  // Depois da virada a cota está em 0 até a próxima leitura.
  const current = resetPassed ? 0 : latest.used_percent;
  const high = current >= HIGH;
  const read = parseTime(latest.sampled_at);

  return (
    <div className="usage-card">
      <div className="usage-card-head">
        <span className="tile-label">{series.title}</span>
        {high && <span className="usage-warn">▲ perto do limite</span>}
      </div>
      <div className="usage-value">
        <span className="tile-value">{Math.round(current)}%</span>
        <span className="muted small">
          {reset === null ? "" : resetPassed ? `zerou às ${hhmm(reset)}` : `zera em ${duration(reset - now)}`}
        </span>
      </div>
      <div className="usage-meter" role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(current)} aria-label={series.title}>
        <span className={high ? "high" : ""} style={{ width: `${Math.min(100, current)}%` }} />
      </div>
      <StepChart points={series.points} from={from} now={now} range={range} label={series.title} />
      <span className="muted small">lido há {duration(now - read)}</span>
    </div>
  );
}

const W = 300;
const H = 90;

function StepChart({ points, from, now, range, label }: { points: Point[]; from: number; now: number; range: Range; label: string }) {
  const [hover, setHover] = useState<number | null>(null); // fração 0-1 do eixo x
  const x = (t: number) => ((t - from) / (now - from)) * W;
  const y = (v: number) => H - (Math.min(100, Math.max(0, v)) / 100) * H;

  // Começa no valor de antes do período (se houver) e vai em degraus até agora.
  const startValue = valueAt(points, from);
  const inside = points.filter((p) => p.t > from && p.t <= now);
  const steps: Point[] = [...(startValue !== null ? [{ t: from, v: startValue }] : []), ...inside];
  let d = "";
  steps.forEach((p, i) => {
    d += i === 0 ? `M${x(p.t)},${y(p.v)}` : `H${x(p.t)}V${y(p.v)}`;
  });
  if (steps.length) d += `H${W}`;
  const area = steps.length ? `${d}V${H}H${x(steps[0].t)}Z` : "";

  const ticks = useMemo(() => {
    // Três rótulos (início, meio, agora): os cards são estreitos.
    return [0, 0.5, 1].map((k) => {
      const t = from + (now - from) * k;
      const d = new Date(t);
      return { t, label: k === 1 ? "agora" : range === "day" ? hhmm(t) : `${p2(d.getDate())}/${p2(d.getMonth() + 1)}` };
    });
  }, [from, now, range]);

  const hoverT = hover === null ? null : from + hover * (now - from);
  const hoverV = hoverT === null ? null : valueAt(points, hoverT);

  return (
    <div className="step-chart">
      <div className="step-y" aria-hidden="true">
        <span style={{ bottom: "100%" }}>100</span>
        <span style={{ bottom: "50%" }}>50</span>
        <span style={{ bottom: "0%" }}>0</span>
      </div>
      <div
        className="step-plot"
        tabIndex={0}
        aria-label={`${label}: linha do percentual usado no período`}
        onMouseMove={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          setHover(Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)));
        }}
        onMouseLeave={() => setHover(null)}
      >
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden="true">
          <line className="step-grid" x1={0} x2={W} y1={y(50)} y2={y(50)} vectorEffect="non-scaling-stroke" />
          <line className="step-grid" x1={0} x2={W} y1={y(HIGH)} y2={y(HIGH)} vectorEffect="non-scaling-stroke" />
          <line className="step-base" x1={0} x2={W} y1={H} y2={H} vectorEffect="non-scaling-stroke" />
          {area && <path className="step-area" d={area} />}
          {d && <path className="step-line" d={d} vectorEffect="non-scaling-stroke" />}
          {hover !== null && <line className="step-cross" x1={hover * W} x2={hover * W} y1={0} y2={H} vectorEffect="non-scaling-stroke" />}
        </svg>
        {hover !== null && hoverV !== null && <span className="step-dot" style={{ left: `${hover * 100}%`, top: `${(y(hoverV) / H) * 100}%` }} />}
        {hover !== null && hoverT !== null && (
          <div className={`bar-tip step-tip ${hover < 0.25 ? "start" : hover > 0.75 ? "end" : ""}`} style={hover >= 0.25 && hover <= 0.75 ? { left: `${hover * 100}%` } : undefined} role="tooltip">
            <div>
              <strong>{hoverV === null ? "sem leitura" : `${Math.round(hoverV)}%`}</strong>
            </div>
            <span>
              {dayLabel(hoverT)} {hhmm(hoverT)}
            </span>
          </div>
        )}
        {!d && <span className="step-empty muted small">sem leituras neste período</span>}
      </div>
      <div />
      <div className="step-x" aria-hidden="true">
        {ticks.map((t, i) => (
          <span key={i}>{t.label}</span>
        ))}
      </div>
    </div>
  );
}
