// LoginView — phase 1 of the two-phase client. Riot-Client analogy: credentials
// + server, then a "produced" transition into the ERP shell.
//
// Polish wave (feat/client-login-polish):
//  - Brand panel + tagline ("Tu farmacia, lista.") in a two-column launcher layout.
//  - Server URL moved behind a collapsible "Conexión avanzada" disclosure.
//  - Password show/hide toggle.
//  - Field-level inline errors in Spanish (no toast spam).
//  - Loading uses a pulse track on the button, not a spinner emoji.
//  - Focus rings, real <label for>, AA contrast, full keyboard nav.
//  - Existing Tauri command + payload shapes (server_url/tenant/email/password)
//    and the onSuccess(session, serverUrl) callback are untouched.
import { login, type SessionInfo } from "../api";

const DEFAULT_SERVER = "http://127.0.0.1:8080";
const DEFAULT_TENANT = "tufarmacia";
const DEFAULT_EMAIL = "admin@tufarmacia.cl";

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
  root.innerHTML = `
    <div class="login-stage" role="main">
      <div class="login-bg" aria-hidden="true">
        <div class="login-bg-drift"></div>
        <div class="login-bg-noise"></div>
      </div>

      <div class="login-frame">
        <aside class="login-brand-panel" aria-hidden="true">
          <div class="brand-stack">
            <div class="brand-mark" aria-label="Tu Farmacia">
              <img src="/tu-farmacia-logo.jpeg" alt="Tu Farmacia" width="44" height="44" />
            </div>
            <div class="brand-wordmark">
              <span class="wm-main">Tu Farmacia</span>
              <span class="wm-sub">COQUIMBO · CHILE</span>
            </div>
          </div>

          <div class="brand-tagline">
            <h2>Tu farmacia, lista.</h2>
            <p>Tu catálogo, tus ventas y tus recetas — siempre en tu local.</p>
          </div>

          <ul class="brand-pillars">
            <li><span class="pillar-dot"></span> Funciona sin internet</li>
            <li><span class="pillar-dot"></span> Boleta SII · ISP · recetas</li>
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
                value="${DEFAULT_TENANT}"
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
                placeholder="usuario@farmacia.cl"
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

            <details class="advanced" id="adv-conn">
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
                    value="${DEFAULT_SERVER}"
                    aria-describedby="${FIELDS.server.err}"
                  />
                  <p id="${FIELDS.server.err}" class="field-error" role="alert" hidden></p>
                  <p class="field-hint">LAN local por defecto. Cambia solo si tu servidor corre en otra IP o puerto.</p>
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
            <span class="foot-line">Funciona sin internet — datos siempre en tu farmacia.</span>
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

    const serverUrl = getInput(FIELDS.server.input).value.trim();
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
    if (!serverUrl) {
      setFieldError("server", "La URL del servidor no puede estar vacía.");
      // Auto-open the disclosure so the user sees the error.
      const adv = root.querySelector<HTMLDetailsElement>("#adv-conn");
      if (adv) adv.open = true;
      firstInvalid ??= "server";
    }
    if (firstInvalid) {
      getInput(FIELDS[firstInvalid].input).focus();
      return;
    }

    btn.disabled = true;
    btn.classList.add("loading");
    btnLabel.textContent = "CONECTANDO";

    try {
      const session = await login(serverUrl, tenant, email, password);
      // Friendly tenant label for the shell (server /me only returns the record id).
      try { sessionStorage.setItem("tenant_slug", tenant); } catch { /* noop */ }
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
}
