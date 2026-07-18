// Command palette (Ctrl/Cmd+K) + keyboard cheatsheet (`?`) — the UX accelerator
// that turns the ERP keyboard-first. A single overlay searches every view and
// the most common actions, runs them on Enter, and surfaces the agent
// ("Pregúntale a tu negocio"). Pure matching/model lives in ./keymap; this file
// is the DOM mount + global hotkey wiring, mounted from shell.ts with one line
// (`installCommandPalette()`), the same way the "/" agent shortcut is.
//
// All dynamic text is escaped (escapeHtml) before it touches innerHTML — command
// titles can come from rubro-renamed nav labels, so they are treated as data.
import { escapeHtml } from "./view-blocks";
import {
  filterCommands,
  nextIndex,
  resolveGoto,
  SHORTCUTS,
  type Command,
  type Shortcut,
} from "./keymap";

/** A navigable destination the palette can jump to (mirrors a shell nav item). */
export interface NavTarget {
  id: string;
  label: string;
  hint?: string;
}

/** Everything the palette needs from the shell, injected so the logic is
 *  testable without the real DOM. `installCommandPalette` builds the live
 *  version from the rendered shell; tests pass fakes. */
export interface PaletteDeps {
  navTargets: () => NavTarget[];
  go: (id: string) => void;
  focusAgent: () => void;
  openCheatsheet: () => void;
}

/** Assemble the full command list: one navigation command per visible nav
 *  target (so it auto-respects rubro-hidden modules) plus the curated action,
 *  agent and help commands. Authored order is what an empty query shows. */
export function buildCommands(deps: PaletteDeps): Command[] {
  const nav: Command[] = deps.navTargets().map((t) => ({
    id: `nav:${t.id}`,
    title: `Ir a ${t.label}`,
    subtitle: t.hint,
    section: "Navegación",
    keywords: [t.id],
    run: () => deps.go(t.id),
  }));

  const actions: Command[] = [
    {
      id: "act:venta",
      title: "Nueva venta",
      subtitle: "Abrir el punto de venta",
      section: "Acciones",
      keywords: ["vender", "boleta", "ticket", "pos", "cobrar"],
      run: () => deps.go("pos"),
    },
    {
      id: "act:caja",
      title: "Abrir o cerrar caja",
      subtitle: "Apertura y arqueo del turno",
      section: "Acciones",
      keywords: ["apertura", "arqueo", "cierre", "turno"],
      run: () => deps.go("caja"),
    },
    {
      id: "act:importar",
      title: "Importar productos",
      subtitle: "Carga masiva por CSV",
      section: "Acciones",
      keywords: ["csv", "carga", "masiva", "subir"],
      run: () => deps.go("importar"),
    },
    {
      id: "act:exportar",
      title: "Exportar datos",
      subtitle: "Descargar tu información en CSV",
      section: "Acciones",
      keywords: ["csv", "respaldo", "descargar", "backup"],
      run: () => deps.go("importar"),
    },
  ];

  const agent: Command = {
    id: "agent:ask",
    title: "Preguntarle a tu negocio",
    subtitle: "Tu agente responde con tus propios datos",
    section: "Agente",
    keywords: ["agente", "ia", "preguntar", "asistente", "negocio"],
    run: () => deps.focusAgent(),
  };

  const help: Command = {
    id: "help:shortcuts",
    title: "Ver atajos de teclado",
    subtitle: "Lista completa de accesos rápidos",
    section: "Ayuda",
    keywords: ["ayuda", "atajos", "teclado", "shortcuts", "ayuda"],
    run: () => deps.openCheatsheet(),
  };

  return [agent, ...actions, ...nav, help];
}

/** Render the results `<li>` rows (pure HTML, every field escaped). Empty list
 *  → empty string; the caller shows the empty-state element instead. */
export function renderResults(commands: Command[], selected: number): string {
  return commands
    .map((c, i) => {
      const active = i === selected;
      const sub = c.subtitle
        ? `<span class="cmdk-item-sub">${escapeHtml(c.subtitle)}</span>`
        : "";
      return `
      <li class="cmdk-item${active ? " is-active" : ""}" role="option" id="cmdk-opt-${i}"
          data-i="${i}" aria-selected="${active ? "true" : "false"}">
        <span class="cmdk-item-body">
          <span class="cmdk-item-title">${escapeHtml(c.title)}</span>
          ${sub}
        </span>
        <span class="cmdk-item-sec">${escapeHtml(c.section)}</span>
      </li>`;
    })
    .join("");
}

/** Handle to an open overlay. */
export interface OverlayHandle {
  close: () => void;
}

// Only one overlay (palette OR cheatsheet) is ever open. Tracked so the global
// hotkeys can toggle/guard and so opening one replaces the other.
let activeOverlay: OverlayHandle | null = null;
function closeActiveOverlay(): void {
  activeOverlay?.close();
}

function isTypingTarget(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  if (!el || !el.tagName) return false;
  return (
    el.tagName === "INPUT" ||
    el.tagName === "TEXTAREA" ||
    el.tagName === "SELECT" ||
    el.isContentEditable === true
  );
}

const SEARCH_ICON = `
  <svg class="cmdk-search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <circle cx="11" cy="11" r="7"></circle><path d="m21 21-4.3-4.3"></path>
  </svg>`;

/** Open the command palette overlay wired to `deps`. Keyboard-first: type to
 *  filter, ↑/↓ to move (wrap), Enter to run, Esc to close, Tab trapped to the
 *  input. Returns a handle whose `close()` tears everything down. */
export function openCommandPalette(deps: PaletteDeps): OverlayHandle {
  closeActiveOverlay();
  const prevFocus = document.activeElement as HTMLElement | null;
  const commands = buildCommands(deps);

  const root = document.createElement("div");
  root.className = "cmdk-backdrop";
  root.innerHTML = `
    <div class="cmdk" role="dialog" aria-modal="true" aria-label="Paleta de comandos">
      <div class="cmdk-search">
        ${SEARCH_ICON}
        <input class="cmdk-input" type="text" role="combobox" aria-expanded="true"
               aria-controls="cmdk-list" aria-autocomplete="list" autocomplete="off"
               spellcheck="false" placeholder="Buscar una vista, una acción o preguntar…"
               aria-label="Buscar un comando" />
        <kbd class="cmdk-esc-hint">Esc</kbd>
      </div>
      <ul class="cmdk-list" id="cmdk-list" role="listbox" aria-label="Resultados"></ul>
      <div class="cmdk-empty" hidden>Sin resultados. Prueba con otra palabra.</div>
      <footer class="cmdk-foot">
        <span><kbd>↑</kbd><kbd>↓</kbd> moverte</span>
        <span><kbd>Enter</kbd> abrir</span>
        <span><kbd>Esc</kbd> cerrar</span>
        <span class="cmdk-foot-grow"></span>
        <span class="cmdk-foot-hint"><kbd>?</kbd> ver atajos</span>
      </footer>
    </div>`;
  document.body.appendChild(root);

  const input = root.querySelector<HTMLInputElement>(".cmdk-input")!;
  const list = root.querySelector<HTMLUListElement>(".cmdk-list")!;
  const empty = root.querySelector<HTMLElement>(".cmdk-empty")!;

  let results: Command[] = [];
  let selected = 0;

  const syncActive = (): void => {
    if (results.length > 0) input.setAttribute("aria-activedescendant", `cmdk-opt-${selected}`);
    else input.removeAttribute("aria-activedescendant");
    list
      .querySelector<HTMLElement>(".cmdk-item.is-active")
      ?.scrollIntoView?.({ block: "nearest" });
  };

  const repaintList = (): void => {
    list.innerHTML = renderResults(results, selected);
    syncActive();
  };

  const refilter = (): void => {
    results = filterCommands(commands, input.value);
    selected = 0;
    const has = results.length > 0;
    empty.hidden = has;
    list.hidden = !has;
    repaintList();
  };

  const move = (dir: 1 | -1): void => {
    const n = nextIndex(selected, results.length, dir);
    if (n < 0) return;
    selected = n;
    repaintList();
  };

  const handle: OverlayHandle = {
    close(): void {
      document.removeEventListener("keydown", onKey, true);
      root.remove();
      if (activeOverlay === handle) activeOverlay = null;
      prevFocus?.focus?.();
    },
  };

  const exec = (): void => {
    const cmd = results[selected];
    if (!cmd) return;
    handle.close();
    cmd.run();
  };

  const onKey = (e: KeyboardEvent): void => {
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        handle.close();
        break;
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "Enter":
        e.preventDefault();
        exec();
        break;
      case "Tab":
        // Trap focus to the input — items move via the arrow keys, not Tab.
        e.preventDefault();
        input.focus();
        break;
    }
  };

  document.addEventListener("keydown", onKey, true);
  input.addEventListener("input", refilter);
  list.addEventListener("click", (e) => {
    const li = (e.target as HTMLElement).closest<HTMLElement>(".cmdk-item");
    if (!li) return;
    selected = Number(li.dataset.i);
    exec();
  });
  root.addEventListener("mousedown", (e) => {
    if (e.target === root) handle.close(); // click the backdrop to dismiss
  });

  refilter();
  input.focus();
  activeOverlay = handle;
  return handle;
}

interface ShortcutGroup {
  group: string;
  items: Shortcut[];
}

/** Group the flat shortcut catalog by `group`, preserving first-seen order. */
function groupShortcuts(shortcuts: Shortcut[]): ShortcutGroup[] {
  const out: ShortcutGroup[] = [];
  for (const s of shortcuts) {
    let g = out.find((x) => x.group === s.group);
    if (!g) {
      g = { group: s.group, items: [] };
      out.push(g);
    }
    g.items.push(s);
  }
  return out;
}

function keysHtml(keys: string[]): string {
  return keys
    .map((k) => `<kbd>${escapeHtml(k)}</kbd>`)
    .join('<span class="cmdk-plus" aria-hidden="true">+</span>');
}

const CLOSE_ICON = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
    <path d="M6 6l12 12M18 6 6 18"></path>
  </svg>`;

/** Open the discoverable keyboard cheatsheet (the `?` overlay). Lists every
 *  global shortcut grouped for scanning; Esc / backdrop / button close it. */
export function openCheatsheet(): OverlayHandle {
  closeActiveOverlay();
  const prevFocus = document.activeElement as HTMLElement | null;

  const groups = groupShortcuts(SHORTCUTS);
  const root = document.createElement("div");
  root.className = "cmdk-backdrop";
  root.innerHTML = `
    <div class="cmdk-sheet" role="dialog" aria-modal="true" aria-label="Atajos de teclado">
      <header class="cmdk-sheet-head">
        <h2 class="cmdk-sheet-title">Atajos de teclado</h2>
        <button class="cmdk-sheet-close" data-close type="button" aria-label="Cerrar">${CLOSE_ICON}</button>
      </header>
      <div class="cmdk-sheet-body">
        ${groups
          .map(
            (g) => `
          <section class="cmdk-sheet-group">
            <h3 class="cmdk-sheet-group-title">${escapeHtml(g.group)}</h3>
            <dl class="cmdk-sheet-list">
              ${g.items
                .map(
                  (s) => `
                <div class="cmdk-sheet-row">
                  <dt class="cmdk-keys">${keysHtml(s.keys)}</dt>
                  <dd class="cmdk-sheet-label">${escapeHtml(s.label)}</dd>
                </div>`,
                )
                .join("")}
            </dl>
          </section>`,
          )
          .join("")}
      </div>
      <footer class="cmdk-foot">
        <span><kbd>Esc</kbd> cerrar</span>
        <span class="cmdk-foot-grow"></span>
        <span class="cmdk-foot-hint"><kbd>Ctrl</kbd><span class="cmdk-plus" aria-hidden="true">+</span><kbd>K</kbd> paleta</span>
      </footer>
    </div>`;
  document.body.appendChild(root);

  const handle: OverlayHandle = {
    close(): void {
      document.removeEventListener("keydown", onKey, true);
      root.remove();
      if (activeOverlay === handle) activeOverlay = null;
      prevFocus?.focus?.();
    },
  };

  const onKey = (e: KeyboardEvent): void => {
    if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      if (e.key === "Escape") handle.close();
    }
  };
  document.addEventListener("keydown", onKey, true);
  root.querySelector<HTMLButtonElement>("[data-close]")?.addEventListener("click", () => handle.close());
  root.addEventListener("mousedown", (e) => {
    if (e.target === root) handle.close();
  });

  root.querySelector<HTMLButtonElement>("[data-close]")?.focus();
  activeOverlay = handle;
  return handle;
}

// --- live shell wiring ------------------------------------------------------

/** Focus the agent ask-bar from anywhere — mirrors the "/" shortcut: if the
 *  ask-bar isn't mounted (operator is off Panel), jump to Panel then focus once
 *  the dashboard has rendered it. */
function focusAgent(): void {
  const tryFocus = (): boolean => {
    const el = document.querySelector<HTMLInputElement>(".agent-ask-input");
    if (!el) return false;
    el.focus();
    el.select?.();
    return true;
  };
  if (tryFocus()) return;
  document.querySelector<HTMLButtonElement>('.nav-item[data-nav="dashboard"]')?.click();
  window.setTimeout(tryFocus, 60);
}

/** Build the live deps from the rendered shell DOM. Navigation reuses the nav
 *  buttons' own click handler (shell's router), so the palette stays in lockstep
 *  with shell behaviour (active state, no re-fetch) and rubro-hidden modules
 *  simply don't appear. */
function liveDeps(): PaletteDeps {
  return {
    navTargets: () =>
      Array.from(document.querySelectorAll<HTMLElement>(".nav-item[data-nav]")).map((btn) => ({
        id: btn.dataset.nav!,
        label: btn.querySelector(".nav-label")?.textContent?.trim() || btn.dataset.nav!,
        hint: btn.querySelector(".nav-hint")?.textContent?.trim() || undefined,
      })),
    go: (id) => document.querySelector<HTMLButtonElement>(`.nav-item[data-nav="${id}"]`)?.click(),
    focusAgent,
    openCheatsheet: () => {
      openCheatsheet();
    },
  };
}

let installed = false;

/** Install the global hotkeys once per process: Ctrl/Cmd+K toggles the palette
 *  (works even inside inputs), `?` opens the cheatsheet, and a `g`-chord jumps
 *  to a view — the last two only when the operator isn't typing in a field. */
export function installCommandPalette(): void {
  if (installed) return;
  installed = true;

  let chordArmed = false;
  let chordTimer = 0;

  document.addEventListener("keydown", (e) => {
    // Ctrl/Cmd+K — deliberate combo, honoured even while typing.
    if ((e.ctrlKey || e.metaKey) && (e.key === "k" || e.key === "K")) {
      e.preventDefault();
      if (activeOverlay) closeActiveOverlay();
      else openCommandPalette(liveDeps());
      return;
    }

    if (activeOverlay) return; // an open overlay owns the keyboard
    if (isTypingTarget(e.target)) return; // never hijack a field

    if (e.key === "?") {
      e.preventDefault();
      openCheatsheet();
      return;
    }

    if (chordArmed) {
      chordArmed = false;
      window.clearTimeout(chordTimer);
      const navId = resolveGoto(e.key);
      if (navId) {
        e.preventDefault();
        liveDeps().go(navId);
      }
      return;
    }

    if (e.key === "g" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      chordArmed = true;
      chordTimer = window.setTimeout(() => {
        chordArmed = false;
      }, 1200);
    }
  });
}
