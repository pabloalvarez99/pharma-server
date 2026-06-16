// LoginView — phase 1 of the two-phase client. Riot-Client analogy: credentials
// + server, then a "produced" transition into the ERP shell.
//
// Polish wave (feat/client-login-polish):
//  - Brand panel + tagline ("Tu negocio, listo.") in a two-column launcher layout.
//  - Server URL moved behind a collapsible "Conexión avanzada" disclosure.
//  - Password show/hide toggle.
//  - Field-level inline errors in Spanish (no toast spam).
//  - Loading uses a pulse track on the button, not a spinner emoji.
//  - Focus rings, real <label for>, AA contrast, full keyboard nav.
//  - Existing Tauri command + payload shapes (server_url/tenant/email/password)
//    and the onSuccess(session, serverUrl) callback are untouched.
//
// Onboarding UX hardening (feat/onboarding-ux-hardening):
//  - Server URL resolution / validation / connection-state come from first-run.ts
//    (the single source of truth for the first-run journey) — no drifting copy.
//  - "Probar conexión" wraps each probe in a hard timeout and retries an
//    unreachable server with bounded backoff (connFeedback narrates the wait)
//    instead of spinning forever — the retry loop runs over first-run's
//    connectionState so the operator-facing messages stay single-sourced.
//  - The chosen server is persisted (and re-validated on load) via the storage
//    helpers in onboarding-ux.ts.
import {
  login,
  serverHealth,
  setupStatus,
  setupAccount,
  type SessionInfo,
} from "../api";
import { RUBRO_CATALOG } from "../vertical";
import { resolveServerConfig, validateServerUrl, connectionState } from "./first-run";
import {
  withTimeout,
  connFeedback,
  connRetryDelay,
  shouldRetryConn,
  CONN_TIMEOUT_MS,
  loadStoredServer,
  saveStoredServer,
  loadStoredTenant,
  saveStoredTenant,
  DEFAULT_TENANT_SLUG,
  type KeyStore,
} from "./onboarding-ux";

// Generic, multi-rubro defaults. The old `tufarmacia` / `admin@tufarmacia.cl`
// pre-fills looked "already configured" on a fresh DB and pointed at a tenant
// that doesn't exist → "credenciales no coinciden" on first ENTRAR. "principal"
// matches the operator manual's suggested branch name; email is left blank (a
// guessed address is worse than an empty one).
const DEFAULT_EMAIL = "";

/** localStorage as a KeyStore, or undefined if the platform forbids it. */
function browserStore(): KeyStore | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

/** Last branch slug that logged in / was created at setup, persisted across
 *  launches so the Sucursal field pre-fills correctly (see onboarding-ux for the
 *  storage-level helper + the lock-out rationale). */
function storedTenant(): string {
  const store = browserStore();
  return store ? loadStoredTenant(store) : DEFAULT_TENANT_SLUG;
}

/** Remember the branch slug that worked, so the next launch pre-fills it. */
function rememberTenant(slug: string): void {
  const store = browserStore();
  if (store) saveStoredTenant(store, slug);
}

/** Build-time override baked by Vite (`VITE_SERVER_URL`), if any. Lets a
 *  field-install build ship pointing at the pharmacy's real server IP. */
function envServer(): string | undefined {
  // No vite/client types in this project, so reach `import.meta.env` via an
  // unknown cast rather than augmenting global ImportMeta.
  const env = (import.meta as unknown as { env?: Record<string, string | undefined> }).env;
  return env?.VITE_SERVER_URL?.trim() || undefined;
}

/** Last server the user logged into successfully, persisted + re-validated
 *  across launches (a corrupt stored value must not poison the field). */
function storedServer(): string | undefined {
  const store = browserStore();
  return store ? loadStoredServer(store) : undefined;
}

/** Resolve the server field's initial value and whether this looks like a
 *  never-configured first launch. Priority: persisted > build-time env >
 *  loopback. On a fresh install with no env, the loopback is only a guess — so
 *  we surface the "Conexión avanzada" panel by default so a LAN client on a
 *  different machine isn't silently pointed at its own localhost. */
function resolveServer(): { value: string; firstLaunch: boolean } {
  return resolveServerConfig({ stored: storedServer(), env: envServer() });
}

interface FieldId {
  input: string;
  err: string;
}

const FIELDS: Record<"tenant" | "email" | "password" | "server", FieldId> = {
  server: { input: "f-server", err: "e-server" },
  tenant: { input: "f-tenant", err: "e-tenant" },
  email: { input: "f-email", err: "e-email" },
  password: { input: "f-password", err: "e-password" },
};

export function renderLogin(
  root: HTMLElement,
  onSuccess: (session: SessionInfo, serverUrl: string) => void,
): void {
  const initialServer = resolveServer();
  const initialTenant = storedTenant();
  root.innerHTML = `
    <div class="login-stage" role="main">
      <div class="login-bg" aria-hidden="true">
        <div class="login-bg-drift"></div>
        <div class="login-bg-noise"></div>
      </div>

      <div class="login-frame">
        <aside class="login-brand-panel" aria-hidden="true">
          <div class="brand-stack">
            <div class="rb-mark" aria-label="RutBusiness">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M4 7V5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2"></path>
                <path d="M4 7h16v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z"></path>
                <path d="M9 11h6M9 15h4"></path>
              </svg>
              <span class="rb-dv"></span>
            </div>
            <div class="brand-wordmark">
              <span class="wm-main rb-wordmark">Rut<b>Business</b></span>
              <span class="wm-sub">EL SO DE TU NEGOCIO</span>
            </div>
          </div>

          <div class="brand-tagline">
            <h2 class="rb-display">Tu negocio, listo.</h2>
            <p>Tu catálogo, tus ventas y tu caja — siempre en tu local. Tu RUT es la llave.</p>
          </div>

          <ul class="brand-pillars">
            <li><span class="pillar-dot"></span> Funciona sin internet</li>
            <li><span class="pillar-dot"></span> Boleta y factura SII</li>
            <li><span class="pillar-dot"></span> POS en menos de 50 ms</li>
          </ul>
        </aside>

        <section class="login-card" aria-labelledby="login-title">
          <header class="login-card-head">
            <h1 id="login-title">Iniciar sesión</h1>
            <p class="login-card-sub">Ingresa con tu cuenta de sucursal.</p>
          </header>

          <form id="login-form" autocomplete="off" novalidate>
            <div class="field">
              <label for="${FIELDS.tenant.input}">Sucursal</label>
              <input
                id="${FIELDS.tenant.input}"
                name="tenant"
                type="text"
                inputmode="text"
                autocapitalize="none"
                autocomplete="organization"
                spellcheck="false"
                placeholder="ej: principal"
                value="${escapeAttr(initialTenant)}"
                aria-describedby="${FIELDS.tenant.err}"
                required
              />
              <p id="${FIELDS.tenant.err}" class="field-error" role="alert" hidden></p>
            </div>

            <div class="field">
              <label for="${FIELDS.email.input}">Correo</label>
              <input
                id="${FIELDS.email.input}"
                name="email"
                type="email"
                inputmode="email"
                autocapitalize="none"
                autocomplete="username"
                spellcheck="false"
                placeholder="usuario@minegocio.cl"
                value="${DEFAULT_EMAIL}"
                aria-describedby="${FIELDS.email.err}"
                required
              />
              <p id="${FIELDS.email.err}" class="field-error" role="alert" hidden></p>
            </div>

            <div class="field">
              <label for="${FIELDS.password.input}">Contraseña</label>
              <div class="field-pw">
                <input
                  id="${FIELDS.password.input}"
                  name="password"
                  type="password"
                  autocomplete="current-password"
                  placeholder="••••••••"
                  aria-describedby="${FIELDS.password.err}"
                  required
                />
                <button
                  type="button"
                  id="pw-toggle"
                  class="pw-toggle"
                  aria-label="Mostrar contraseña"
                  aria-pressed="false"
                  tabindex="0"
                >
                  <svg class="pw-eye" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M1 12s4-7 11-7 11 7 11 7-4 7-11 7-11-7-11-7z"></path>
                    <circle cx="12" cy="12" r="3"></circle>
                  </svg>
                  <svg class="pw-eye-off" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" hidden>
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-7-11-7a18.45 18.45 0 0 1 5.06-5.94"></path>
                    <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 7 11 7a18.5 18.5 0 0 1-2.16 3.19"></path>
                    <path d="M1 1l22 22"></path>
                    <path d="M14.12 14.12A3 3 0 0 1 9.88 9.88"></path>
                  </svg>
                </button>
              </div>
              <p id="${FIELDS.password.err}" class="field-error" role="alert" hidden></p>
            </div>

            <details class="advanced" id="adv-conn"${initialServer.firstLaunch ? " open" : ""}>
              <summary>
                <span>Conexión avanzada</span>
                <svg class="chev" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <polyline points="6 9 12 15 18 9"></polyline>
                </svg>
              </summary>
              <div class="advanced-body">
                <div class="field">
                  <label for="${FIELDS.server.input}">Servidor</label>
                  <input
                    id="${FIELDS.server.input}"
                    name="server"
                    type="text"
                    spellcheck="false"
                    value="${initialServer.value}"
                    aria-describedby="${FIELDS.server.err}"
                  />
                  <p id="${FIELDS.server.err}" class="field-error" role="alert" hidden></p>
                  <p class="field-hint">Si el servidor corre en otro equipo de la red, escribe su IP y puerto (ej: http://192.168.1.50:8080).</p>
                  <div class="conn-test">
                    <button type="button" id="conn-test-btn" class="btn-ghost">Probar conexión</button>
                    <span id="conn-test-status" class="conn-test-status" role="status" aria-live="polite"></span>
                  </div>
                </div>
              </div>
            </details>

            <p id="login-error" class="form-error" role="alert" hidden></p>

            <button id="login-submit" type="submit" class="btn-primary">
              <span class="btn-pulse" aria-hidden="true"></span>
              <span class="btn-label">ENTRAR</span>
            </button>
          </form>

          <footer class="login-foot">
            <span class="foot-line">Funciona sin internet — datos siempre en tu negocio.</span>
            <span class="foot-meta">El token de sesión vive sólo en memoria.</span>
          </footer>
        </section>
      </div>
    </div>
  `;

  const form = root.querySelector<HTMLFormElement>("#login-form")!;
  const btn = root.querySelector<HTMLButtonElement>("#login-submit")!;
  const btnLabel = btn.querySelector<HTMLSpanElement>(".btn-label")!;
  const formErr = root.querySelector<HTMLParagraphElement>("#login-error")!;

  const getInput = (id: string) => root.querySelector<HTMLInputElement>(`#${id}`)!;
  const getErr = (id: string) => root.querySelector<HTMLParagraphElement>(`#${id}`)!;

  function clearAllErrors(): void {
    formErr.hidden = true;
    formErr.textContent = "";
    Object.values(FIELDS).forEach(({ input, err }) => {
      getErr(err).hidden = true;
      getErr(err).textContent = "";
      getInput(input).classList.remove("invalid");
      getInput(input).removeAttribute("aria-invalid");
    });
  }

  function setFieldError(field: keyof typeof FIELDS, msg: string): void {
    const { input, err } = FIELDS[field];
    const errEl = getErr(err);
    errEl.textContent = msg;
    errEl.hidden = false;
    const inp = getInput(input);
    inp.classList.add("invalid");
    inp.setAttribute("aria-invalid", "true");
  }

  function setFormError(msg: string): void {
    formErr.textContent = msg;
    formErr.hidden = false;
  }

  // Password show/hide toggle — accessible (aria-pressed + label flip).
  const pwToggle = root.querySelector<HTMLButtonElement>("#pw-toggle")!;
  const pwInput = getInput(FIELDS.password.input);
  const pwEye = pwToggle.querySelector<SVGElement>(".pw-eye")!;
  const pwEyeOff = pwToggle.querySelector<SVGElement>(".pw-eye-off")!;
  pwToggle.addEventListener("click", () => {
    const showing = pwInput.type === "text";
    pwInput.type = showing ? "password" : "text";
    pwToggle.setAttribute("aria-pressed", showing ? "false" : "true");
    pwToggle.setAttribute(
      "aria-label",
      showing ? "Mostrar contraseña" : "Ocultar contraseña",
    );
    pwEye.toggleAttribute("hidden", !showing);
    pwEyeOff.toggleAttribute("hidden", showing);
  });

  const sleep = (ms: number) => new Promise<void>((r) => window.setTimeout(r, ms));

  // "Probar conexión" — pings the server's health endpoint so a LAN client can
  // confirm the URL before typing credentials (no auth needed; reachability
  // only). Each attempt is wrapped in a hard timeout (a hung socket must never
  // freeze the status line) and unreachable servers retry with bounded backoff
  // instead of spinning forever — connFeedback narrates "Reintentando en Ns…".
  // The reachable/degraded/unreachable classification + Spanish strings come
  // from first-run's connectionState so they stay single-sourced with submit.
  const connBtn = root.querySelector<HTMLButtonElement>("#conn-test-btn")!;
  const connStatus = root.querySelector<HTMLSpanElement>("#conn-test-status")!;
  connBtn.addEventListener("click", async () => {
    const check = validateServerUrl(getInput(FIELDS.server.input).value);
    if (!check.ok) {
      setFieldError("server", check.error);
      connStatus.className = "conn-test-status err";
      connStatus.textContent = "Revisa la URL antes de probar.";
      return;
    }
    const url = check.url;
    connBtn.disabled = true;
    let attempt = 1;
    for (;;) {
      connStatus.className = "conn-test-status";
      connStatus.textContent =
        attempt === 1 ? "Probando…" : `Probando (intento ${attempt})…`;
      try {
        const conn = connectionState(await withTimeout(serverHealth(url), CONN_TIMEOUT_MS));
        if (conn.kind === "ok") {
          connStatus.className = "conn-test-status ok";
          connStatus.textContent = conn.message;
          break;
        }
        if (conn.kind === "degraded") {
          // Responding but degraded — no point retrying, surface it as a warning.
          connStatus.className = "conn-test-status warn";
          connStatus.textContent = conn.message;
          break;
        }
        // unreachable (probe returned but server says not reachable) → retry.
        const fb = connFeedback(conn.message, attempt);
        connStatus.className = "conn-test-status err";
        connStatus.textContent = fb.message;
        if (!fb.willRetry || !shouldRetryConn(attempt)) break;
        await sleep(connRetryDelay(attempt + 1));
        attempt += 1;
      } catch (err) {
        const fb = connFeedback(err, attempt);
        connStatus.className = "conn-test-status err";
        connStatus.textContent = fb.message;
        if (!fb.willRetry || !shouldRetryConn(attempt)) break;
        await sleep(connRetryDelay(attempt + 1));
        attempt += 1;
      }
    }
    connBtn.disabled = false;
  });

  // Inline server-URL validation when the operator leaves the field. A malformed
  // URL is flagged on blur so they never hit a cryptic backend error (the generic
  // input handler below clears the error once they start fixing it).
  getInput(FIELDS.server.input).addEventListener("blur", () => {
    const inp = getInput(FIELDS.server.input);
    if (!inp.value.trim()) return; // empty handled on submit
    const v = validateServerUrl(inp.value);
    if (!v.ok) setFieldError("server", v.error);
  });

  // Strip stale field errors as the user fixes them.
  (["tenant", "email", "password", "server"] as const).forEach((k) => {
    getInput(FIELDS[k].input).addEventListener("input", () => {
      const errEl = getErr(FIELDS[k].err);
      if (!errEl.hidden) {
        errEl.hidden = true;
        getInput(FIELDS[k].input).classList.remove("invalid");
        getInput(FIELDS[k].input).removeAttribute("aria-invalid");
      }
      if (!formErr.hidden) formErr.hidden = true;
    });
  });

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    clearAllErrors();

    const rawServer = getInput(FIELDS.server.input).value;
    const tenant = getInput(FIELDS.tenant.input).value.trim();
    const email = getInput(FIELDS.email.input).value.trim();
    const password = getInput(FIELDS.password.input).value;

    let firstInvalid: keyof typeof FIELDS | null = null;
    if (!tenant) {
      setFieldError("tenant", "Indica tu sucursal.");
      firstInvalid ??= "tenant";
    }
    if (!email) {
      setFieldError("email", "Indica tu correo.");
      firstInvalid ??= "email";
    }
    if (!password) {
      setFieldError("password", "La contraseña es obligatoria.");
      firstInvalid ??= "password";
    }
    // Full URL validation (empty / malformed / bad scheme), not just non-empty —
    // so the operator never submits a URL the backend would only reject with a
    // cryptic transport error.
    const serverCheck = validateServerUrl(rawServer);
    if (!serverCheck.ok) {
      setFieldError("server", serverCheck.error);
      // Auto-open the disclosure so the user sees the error.
      const adv = root.querySelector<HTMLDetailsElement>("#adv-conn");
      if (adv) adv.open = true;
      firstInvalid ??= "server";
    }
    if (firstInvalid) {
      getInput(FIELDS[firstInvalid].input).focus();
      return;
    }
    // Past validation: serverCheck is ok, use the normalised URL.
    const serverUrl = serverCheck.ok ? serverCheck.url : rawServer.trim();

    btn.disabled = true;
    btn.classList.add("loading");
    btnLabel.textContent = "CONECTANDO";

    try {
      const session = await login(serverUrl, tenant, email, password);
      // Friendly tenant label for the shell (server /me only returns the record id).
      try { sessionStorage.setItem("tenant_slug", tenant); } catch { /* noop */ }
      // Persist the slug that worked so the next launch pre-fills Sucursal.
      rememberTenant(tenant);
      // Remember the server that worked so the next launch pre-fills it (and
      // the advanced panel stays collapsed — this machine is configured now).
      // Persisted in canonical form; survives restart (see onboarding-ux tests).
      const store = browserStore();
      if (store) saveStoredServer(store, serverUrl);
      btn.classList.remove("loading");
      btn.classList.add("ok");
      btnLabel.textContent = "LISTO";
      // Brief "produced" beat before the shell takes over (matches CSS 420ms).
      root.querySelector(".login-card")?.classList.add("launch");
      root.querySelector(".login-brand-panel")?.classList.add("launch");
      window.setTimeout(() => onSuccess(session, serverUrl), 280);
    } catch (err) {
      // Tauri command errors arrive as the Spanish string we returned in Rust.
      setFormError(typeof err === "string" ? err : "Error inesperado al iniciar sesión.");
      btn.disabled = false;
      btn.classList.remove("loading");
      btnLabel.textContent = "ENTRAR";
    }
  });

  // Sane initial focus: tenant first.
  getInput(FIELDS.tenant.input).focus();

  // First-run probe: on a fresh install (no account in the whole DB) swap the
  // login card for an in-app account-creation form, so the lone operator never
  // has to drop to the terminal (`pharma tenant-create` / `user-create`) just to
  // get past the login screen. Best-effort: if the server is unreachable or too
  // old to expose /setup, we silently stay on the login form.
  void (async () => {
    const probe = validateServerUrl(getInput(FIELDS.server.input).value);
    if (!probe.ok) return;
    try {
      const st = await setupStatus(probe.url);
      if (st.needs_setup) renderFirstRunSetup(root, probe.url, onSuccess);
    } catch {
      /* unreachable / route missing → leave login form as-is */
    }
  })();
}

/** First-run account creation — shown only when the server reports it has no
 *  account yet. Creates the first tenant + owner and logs straight in, so a
 *  fresh install goes app → account → dashboard with zero CLI. */
export function renderFirstRunSetup(
  root: HTMLElement,
  serverUrl: string,
  onSuccess: (session: SessionInfo, serverUrl: string) => void,
): void {
  const rubroOptions = RUBRO_CATALOG.map(
    (r) =>
      `<option value="${r.value}">${escapeAttr(r.icon)} ${escapeAttr(r.label)}</option>`,
  ).join("");

  root.innerHTML = `
    <div class="login-stage" role="main">
      <div class="login-bg" aria-hidden="true">
        <div class="login-bg-drift"></div>
        <div class="login-bg-noise"></div>
      </div>
      <div class="login-frame">
        <aside class="login-brand-panel" aria-hidden="true">
          <div class="brand-stack">
            <div class="rb-mark" aria-label="RutBusiness">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M4 7V5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v2"></path>
                <path d="M4 7h16v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z"></path>
                <path d="M9 11h6M9 15h4"></path>
              </svg>
              <span class="rb-dv"></span>
            </div>
            <div class="brand-wordmark">
              <span class="wm-main rb-wordmark">Rut<b>Business</b></span>
              <span class="wm-sub">EL SO DE TU NEGOCIO</span>
            </div>
          </div>
          <div class="brand-tagline">
            <h2 class="rb-display">Bienvenido.</h2>
            <p>Es la primera vez que abres este servidor. Crea tu cuenta de dueño y empieza a vender — sin instalar nada más.</p>
          </div>
        </aside>

        <section class="login-card" aria-labelledby="setup-title">
          <header class="login-card-head">
            <h1 id="setup-title">Crea tu cuenta</h1>
            <p class="login-card-sub">Primer inicio — esta cuenta será la dueña del negocio.</p>
          </header>

          <form id="setup-form" autocomplete="off" novalidate>
            <div class="field">
              <label for="s-name">Nombre del negocio</label>
              <input id="s-name" type="text" placeholder="ej: Almacén Don José" aria-describedby="es-name" required />
              <p id="es-name" class="field-error" role="alert" hidden></p>
            </div>

            <div class="field">
              <label for="s-rubro">Rubro</label>
              <select id="s-rubro" class="rb-input">${rubroOptions}</select>
              <p class="field-hint">Define qué módulos se muestran (las recetas sólo en Farmacia). Lo puedes cambiar luego en Configuración.</p>
            </div>

            <div class="field">
              <label for="s-email">Correo</label>
              <input id="s-email" type="email" inputmode="email" autocapitalize="none" spellcheck="false" placeholder="dueno@minegocio.cl" aria-describedby="es-email" required />
              <p id="es-email" class="field-error" role="alert" hidden></p>
            </div>

            <div class="field">
              <label for="s-password">Contraseña</label>
              <input id="s-password" type="password" autocomplete="new-password" placeholder="mínimo 8 caracteres" aria-describedby="es-password" required />
              <p id="es-password" class="field-error" role="alert" hidden></p>
            </div>

            <details class="advanced" id="s-adv-conn">
              <summary>
                <span>Conexión avanzada</span>
                <svg class="chev" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="6 9 12 15 18 9"></polyline></svg>
              </summary>
              <div class="advanced-body">
                <div class="field">
                  <label for="s-server">Servidor</label>
                  <input id="s-server" type="text" spellcheck="false" value="${escapeAttr(serverUrl)}" aria-describedby="es-server" />
                  <p id="es-server" class="field-error" role="alert" hidden></p>
                </div>
              </div>
            </details>

            <p id="setup-error" class="form-error" role="alert" hidden></p>

            <button id="setup-submit" type="submit" class="btn-primary">
              <span class="btn-pulse" aria-hidden="true"></span>
              <span class="btn-label">CREAR CUENTA Y ENTRAR</span>
            </button>
          </form>

          <footer class="login-foot">
            <span class="foot-line">Tus datos viven sólo en este equipo.</span>
            <span class="foot-meta">¿Ya tienes cuenta? Reinicia la app para iniciar sesión.</span>
          </footer>
        </section>
      </div>
    </div>
  `;

  const form = root.querySelector<HTMLFormElement>("#setup-form")!;
  const btn = root.querySelector<HTMLButtonElement>("#setup-submit")!;
  const btnLabel = btn.querySelector<HTMLSpanElement>(".btn-label")!;
  const formErr = root.querySelector<HTMLParagraphElement>("#setup-error")!;
  const q = <T extends HTMLElement>(id: string) => root.querySelector<T>(`#${id}`)!;

  function fieldError(id: string, msg: string): void {
    const el = q<HTMLParagraphElement>(id);
    el.textContent = msg;
    el.hidden = false;
  }
  function clearErrors(): void {
    formErr.hidden = true;
    ["es-name", "es-email", "es-password", "es-server"].forEach((id) => {
      const el = q<HTMLParagraphElement>(id);
      el.hidden = true;
      el.textContent = "";
    });
  }

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    clearErrors();

    const name = q<HTMLInputElement>("s-name").value.trim();
    const rubro = q<HTMLSelectElement>("s-rubro").value;
    const email = q<HTMLInputElement>("s-email").value.trim();
    const password = q<HTMLInputElement>("s-password").value;
    const rawServer = q<HTMLInputElement>("s-server").value;

    let firstBad: string | null = null;
    if (!name) {
      fieldError("es-name", "Indica el nombre de tu negocio.");
      firstBad ??= "s-name";
    }
    if (!email) {
      fieldError("es-email", "Indica tu correo.");
      firstBad ??= "s-email";
    } else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
      fieldError("es-email", "El correo no tiene un formato válido.");
      firstBad ??= "s-email";
    }
    if (password.length < 8) {
      fieldError("es-password", "La contraseña debe tener al menos 8 caracteres.");
      firstBad ??= "s-password";
    }
    const serverCheck = validateServerUrl(rawServer);
    if (!serverCheck.ok) {
      fieldError("es-server", serverCheck.error);
      const adv = root.querySelector<HTMLDetailsElement>("#s-adv-conn");
      if (adv) adv.open = true;
      firstBad ??= "s-server";
    }
    if (firstBad) {
      q<HTMLElement>(firstBad).focus();
      return;
    }
    const url = serverCheck.ok ? serverCheck.url : rawServer.trim();

    btn.disabled = true;
    btn.classList.add("loading");
    btnLabel.textContent = "CREANDO";
    try {
      const session = await setupAccount(url, {
        businessName: name,
        email,
        password,
        vertical: rubro,
      });
      try { sessionStorage.setItem("tenant_slug", session.tenant_slug); } catch { /* noop */ }
      // The server derives the slug from the business name — persist it so the
      // owner isn't locked out on the next launch by a wrong "principal" pre-fill.
      rememberTenant(session.tenant_slug);
      btn.classList.remove("loading");
      btn.classList.add("ok");
      btnLabel.textContent = "LISTO";
      root.querySelector(".login-card")?.classList.add("launch");
      root.querySelector(".login-brand-panel")?.classList.add("launch");
      window.setTimeout(() => onSuccess(session, url), 280);
    } catch (err) {
      formErr.textContent =
        typeof err === "string" ? err : "No se pudo crear la cuenta. Intenta nuevamente.";
      formErr.hidden = false;
      btn.disabled = false;
      btn.classList.remove("loading");
      btnLabel.textContent = "CREAR CUENTA Y ENTRAR";
    }
  });

  q<HTMLInputElement>("s-name").focus();
}

/** Minimal attribute escaper for values interpolated into the setup template. */
function escapeAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
