import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { PARTY_HAT_PATHS } from "./Dates";

export type PaletteGroup = "Criar e focar" | "Ir para" | "Filtrar quadro" | "Projetos" | "Tarefas ativas";

export type PaletteIconName =
  | "plus"
  | "board"
  | "calendar"
  | "party"
  | "chart"
  | "folder"
  | "history"
  | "sync"
  | "filter"
  | "edit";

export interface PaletteCommand {
  id: string;
  group: PaletteGroup;
  label: string;
  hint?: string;
  keywords?: string;
  icon: PaletteIconName;
  run: () => void;
}

const LISTBOX_ID = "command-palette-results";

const optionId = (command: PaletteCommand) => `palette-${command.id.replace(/[^a-z0-9-]/gi, "-")}`;

/**
 * Converte texto para minúsculas sem acentos, permitindo buscar "aniversario" e encontrar "Aniversário".
 */
export function normalizeForSearch(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

/**
 * Filtra os comandos pelas palavras digitadas em label, hint ou keywords.
 * Com busca vazia, oculta "Tarefas ativas" para manter ações, navegação e projetos em destaque.
 */
export function filterPaletteCommands(commands: PaletteCommand[], query: string, limit = 50): PaletteCommand[] {
  const typed = normalizeForSearch(query.trim());
  if (!typed) {
    return commands.filter((c) => c.group !== "Tarefas ativas").slice(0, limit);
  }
  const words = typed.split(/\s+/);
  const matches = commands.filter((c) => {
    const text = normalizeForSearch(`${c.label} ${c.hint ?? ""} ${c.keywords ?? ""}`);
    return words.every((word) => text.includes(word));
  });
  const startsWithQuery = (c: PaletteCommand) => (normalizeForSearch(c.label).startsWith(typed) ? 0 : 1);
  return [...matches].sort((a, b) => startsWithQuery(a) - startsWithQuery(b)).slice(0, limit);
}

/**
 * Ctrl+K (Cmd+K no macOS) abre ou fecha a paleta de atalhos de qualquer tela, mesmo dentro de inputs.
 */
export function useCommandPaletteShortcut(onToggle: () => void): void {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== "k" || !(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey) return;
      event.preventDefault();
      onToggle();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onToggle]);
}

const PALETTE_ICONS: Record<PaletteIconName, React.ReactNode> = {
  plus: <path d="M12 5v14M5 12h14" />,
  board: (
    <>
      <rect x="3" y="4" width="5" height="16" rx="1.5" />
      <rect x="10" y="4" width="5" height="12" rx="1.5" />
      <rect x="17" y="4" width="4" height="14" rx="1.5" />
    </>
  ),
  calendar: (
    <>
      <rect x="3" y="4" width="18" height="17" rx="2" />
      <path d="M8 2v4M16 2v4M3 10h18" />
    </>
  ),
  party: PARTY_HAT_PATHS,
  chart: (
    <>
      <path d="M4 20h16" />
      <path d="M7 16v-5M12 16V6M17 16v-8" />
    </>
  ),
  folder: <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />,
  history: (
    <>
      <path d="M3 12a9 9 0 1 0 3-6.7" />
      <path d="M3 4v4h4" />
      <path d="M12 8v4l3 2" />
    </>
  ),
  sync: (
    <>
      <path d="M20 12a8 8 0 0 0-14-5.3" />
      <path d="M4 4v4h4" />
      <path d="M4 12a8 8 0 0 0 14 5.3" />
      <path d="M20 20v-4h-4" />
    </>
  ),
  filter: <path d="M3 5h18l-7 8v5l-4 2v-7L3 5z" />,
  edit: (
    <>
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
    </>
  ),
};

function OptionIcon({ icon }: { icon: PaletteIconName }) {
  return (
    <svg
      width={16}
      height={16}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className="palette-option__icon"
    >
      {PALETTE_ICONS[icon]}
    </svg>
  );
}

export function PaletteButton({ onOpen }: { onOpen: () => void }) {
  return (
    <button
      type="button"
      className="palette-button"
      onClick={onOpen}
      aria-label="Atalhos e busca rápida (Ctrl+K)"
      aria-keyshortcuts="Control+K"
      title="Atalhos e busca rápida (Ctrl+K)"
    >
      <svg
        width={14}
        height={14}
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <circle cx="7" cy="7" r="4.5" />
        <path d="M10.5 10.5L14 14" />
      </svg>
      <kbd>Ctrl K</kbd>
    </button>
  );
}

function nextIndex(key: string, index: number, count: number): number | null {
  if (count === 0) return null;
  if (key === "ArrowDown") return (index + 1) % count;
  if (key === "ArrowUp") return (index - 1 + count) % count;
  return null;
}

export function CommandPalette({ commands, onClose }: { commands: PaletteCommand[]; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const results = useMemo(() => filterPaletteCommands(commands, query), [commands, query]);
  const safeIndex = results.length === 0 ? 0 : Math.min(activeIndex, results.length - 1);
  const active = results[safeIndex];

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      onClose();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  useEffect(() => {
    if (!active) return;
    document.getElementById(optionId(active))?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const pick = (command: PaletteCommand) => {
    onClose();
    command.run();
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    const next = nextIndex(event.key, safeIndex, results.length);
    if (next !== null) {
      setActiveIndex(next);
      event.preventDefault();
      return;
    }
    if (event.key === "Enter" && active) {
      event.preventDefault();
      pick(active);
    }
  };

  return (
    <div className="palette-backdrop" onMouseDown={onClose}>
      <section
        role="dialog"
        aria-modal="true"
        aria-label="Atalhos rápidos"
        className="palette-dialog"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="palette-head">
          <svg
            width={16}
            height={16}
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <circle cx="7" cy="7" r="4.5" />
            <path d="M10.5 10.5L14 14" />
          </svg>
          <input
            ref={inputRef}
            autoFocus
            className="palette-input"
            role="combobox"
            aria-expanded="true"
            aria-controls={LISTBOX_ID}
            aria-activedescendant={active ? optionId(active) : undefined}
            aria-label="Digite o que deseja fazer, tela ou tarefa"
            placeholder="Digite o que deseja fazer, tela ou tarefa…"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setActiveIndex(0);
            }}
            onKeyDown={onKeyDown}
          />
          <kbd className="palette-kbd">Esc</kbd>
        </div>

        {results.length === 0 ? (
          <p className="palette-empty">Nada encontrado para “{query}”.</p>
        ) : (
          <ul id={LISTBOX_ID} role="listbox" aria-label="Resultados" className="palette-results">
            {results.map((command, index) => (
              <Fragment key={command.id}>
                {command.group !== results[index - 1]?.group && (
                  <li role="presentation" className="palette-group">
                    {command.group}
                  </li>
                )}
                <li
                  id={optionId(command)}
                  role="option"
                  aria-selected={index === safeIndex}
                  className="palette-option"
                  onMouseMove={() => setActiveIndex(index)}
                  onClick={() => pick(command)}
                >
                  <OptionIcon icon={command.icon} />
                  <span className="palette-option__label">{command.label}</span>
                  {command.hint && <span className="palette-option__hint">{command.hint}</span>}
                </li>
              </Fragment>
            ))}
          </ul>
        )}

        <div className="palette-foot" aria-hidden="true">
          <span>
            <kbd className="palette-kbd">↑</kbd> <kbd className="palette-kbd">↓</kbd> navegar
          </span>
          <span>
            <kbd className="palette-kbd">Enter</kbd> executar
          </span>
          <span>
            <kbd className="palette-kbd">Esc</kbd> fechar
          </span>
        </div>
      </section>
    </div>
  );
}
