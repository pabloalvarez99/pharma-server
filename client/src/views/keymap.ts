// Keymap — the pure, DOM-free core of the command palette (Ctrl/Cmd+K) and the
// discoverable keyboard cheatsheet (`?`). Holds the command model, the es-CL
// accent-insensitive fuzzy matcher, the keyboard-wrap helper, the global
// "go-to" chord table, and the shortcut catalog. Kept side-effect free so every
// piece is unit-testable without a browser (see keymap.test.ts); the DOM mount
// lives in command-palette.ts.

/** Where a command shows up in the grouped palette. */
export type CommandSection = "Navegación" | "Acciones" | "Agente" | "Ayuda";

/** A single runnable entry in the palette. `run` performs the action (navigate,
 *  focus the agent, open the cheatsheet…); the palette never inspects what it
 *  does, it just calls it on Enter/click. `keywords` widen what text finds the
 *  command without bloating its visible title. */
export interface Command {
  id: string;
  title: string;
  subtitle?: string;
  section: CommandSection;
  keywords?: string[];
  run: () => void;
}

/** One row in the `?` cheatsheet: the key combo to press and what it does. */
export interface Shortcut {
  keys: string[];
  label: string;
  group: string;
}

/** Fold a string to a comparable form: lowercase + strip Spanish diacritics so
 *  "Configuración" and "configuracion" match, and trim the edges. */
export function normalize(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .trim();
}

function isSubsequence(needle: string, haystack: string): boolean {
  if (needle.length === 0) return true;
  let i = 0;
  for (let j = 0; j < haystack.length && i < needle.length; j++) {
    if (haystack[j] === needle[i]) i++;
  }
  return i === needle.length;
}

function wordStartsWith(title: string, token: string): boolean {
  return title.split(/\s+/).some((w) => w.startsWith(token));
}

/** Score how well `query` matches `command`. Higher is better; -1 means no
 *  match (the command is dropped from results). An empty query matches every
 *  command with a flat neutral score so the list keeps its authored order.
 *  Multi-word queries are AND: every whitespace-separated token must land
 *  somewhere (title or keywords), otherwise the command is excluded. */
export function scoreCommand(command: Command, query: string): number {
  const q = normalize(query);
  if (q.length === 0) return 1; // neutral — keeps input order in filterCommands

  const title = normalize(command.title);
  const hay = normalize(
    [command.title, command.subtitle ?? "", ...(command.keywords ?? [])].join(" "),
  );

  let total = 0;
  for (const tok of q.split(/\s+/).filter(Boolean)) {
    let s: number;
    if (title.startsWith(tok)) s = 100;
    else if (wordStartsWith(title, tok)) s = 70;
    else if (title.includes(tok)) s = 45;
    else if (hay.includes(tok)) s = 25;
    else if (isSubsequence(tok, hay)) s = 10;
    else return -1; // a token matched nothing → exclude the command
    total += s;
  }
  // Tie-break toward shorter (more specific) titles, without crossing tiers.
  return total - title.length * 0.01;
}

/** Filter + rank commands for a query. Best score first; ties keep input order
 *  (stable), so an empty query returns the commands exactly as authored. */
export function filterCommands(commands: Command[], query: string): Command[] {
  return commands
    .map((command, i) => ({ command, i, score: scoreCommand(command, query) }))
    .filter((e) => e.score >= 0)
    .sort((a, b) => b.score - a.score || a.i - b.i)
    .map((e) => e.command);
}

/** Move a selection index by `dir` with wrap-around. Returns -1 for an empty
 *  list so callers can render the "no selection" state. */
export function nextIndex(current: number, len: number, dir: 1 | -1): number {
  if (len <= 0) return -1;
  return (current + dir + len) % len;
}

/** Global "go-to" chord table: press `g` then one of these keys to jump
 *  straight to a view. Curated, unambiguous mnemonics — everything else is
 *  reachable through the palette's search. Documented verbatim in SHORTCUTS so
 *  there is nothing to guess. */
export const GOTO: Record<string, string> = {
  d: "dashboard",
  p: "pos",
  c: "caja",
  i: "inventory",
  r: "reports",
  s: "configuracion",
};

/** Resolve a `g`-chord second key to its nav id, or null if unmapped. */
export function resolveGoto(key: string): string | null {
  return GOTO[key.toLowerCase()] ?? null;
}

/** The catalog rendered by the `?` cheatsheet overlay — the single source of
 *  truth for every global shortcut, grouped for scanning. */
export const SHORTCUTS: Shortcut[] = [
  { keys: ["Ctrl", "K"], label: "Abrir la paleta de comandos", group: "General" },
  { keys: ["?"], label: "Mostrar estos atajos de teclado", group: "General" },
  { keys: ["/"], label: "Preguntarle a tu negocio (agente)", group: "General" },
  { keys: ["Esc"], label: "Cerrar la ventana o el diálogo actual", group: "General" },
  { keys: ["↑", "↓"], label: "Moverte entre los resultados", group: "Paleta" },
  { keys: ["Enter"], label: "Ejecutar el comando seleccionado", group: "Paleta" },
  { keys: ["g", "d"], label: "Ir al Panel", group: "Ir a…" },
  { keys: ["g", "p"], label: "Ir al POS (vender)", group: "Ir a…" },
  { keys: ["g", "c"], label: "Ir a Caja", group: "Ir a…" },
  { keys: ["g", "i"], label: "Ir a Inventario", group: "Ir a…" },
  { keys: ["g", "r"], label: "Ir a Reportes", group: "Ir a…" },
  { keys: ["g", "s"], label: "Ir a Configuración", group: "Ir a…" },
];
