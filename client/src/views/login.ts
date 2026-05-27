// LoginView — phase 1 of the two-phase client. Riot-Client analogy: credentials
// + server, then a "produced" transition into the ERP shell.
import { login, type SessionInfo } from "../api";

const DEFAULT_SERVER = "http://localhost:8080";

export function renderLogin(
  root: HTMLElement,
  onSuccess: (session: SessionInfo, serverUrl: string) => void,
): void {
  root.innerHTML = `
    <div class="login-stage">
      <div class="login-card">
        <div class="brand">
          <div class="brand-mark">Rx</div>
          <div class="brand-text">
            <h1>PHARMA CLIENT</h1>
            <span>Servidor on-prem · offline-first</span>
          </div>
        </div>

        <form id="login-form" autocomplete="off" novalidate>
          <label class="field">
            <span>Servidor</span>
            <input id="f-server" type="text" value="${DEFAULT_SERVER}" spellcheck="false" />
          </label>
          <label class="field">
            <span>Sucursal (tenant)</span>
            <input id="f-tenant" type="text" placeholder="ej: principal" spellcheck="false" />
          </label>
          <label class="field">
            <span>Correo</span>
            <input id="f-email" type="email" placeholder="usuario@farmacia.cl" spellcheck="false" />
          </label>
          <label class="field">
            <span>Contraseña</span>
            <input id="f-password" type="password" placeholder="••••••••" />
          </label>

          <p id="login-error" class="error" role="alert" hidden></p>

          <button id="login-submit" type="submit" class="btn-primary">
            <span class="btn-label">ENTRAR</span>
          </button>
        </form>

        <footer class="login-foot">El token de sesión vive sólo en memoria.</footer>
      </div>
    </div>
  `;

  const form = root.querySelector<HTMLFormElement>("#login-form")!;
  const errEl = root.querySelector<HTMLParagraphElement>("#login-error")!;
  const btn = root.querySelector<HTMLButtonElement>("#login-submit")!;
  const label = btn.querySelector<HTMLSpanElement>(".btn-label")!;

  function showError(msg: string): void {
    errEl.textContent = msg;
    errEl.hidden = false;
  }

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    errEl.hidden = true;

    const serverUrl = (root.querySelector<HTMLInputElement>("#f-server")!.value || "").trim();
    const tenant = (root.querySelector<HTMLInputElement>("#f-tenant")!.value || "").trim();
    const email = (root.querySelector<HTMLInputElement>("#f-email")!.value || "").trim();
    const password = root.querySelector<HTMLInputElement>("#f-password")!.value;

    if (!serverUrl || !tenant || !email || !password) {
      showError("Completa todos los campos.");
      return;
    }

    btn.disabled = true;
    btn.classList.add("loading");
    label.textContent = "CONECTANDO…";

    try {
      const session = await login(serverUrl, tenant, email, password);
      label.textContent = "LISTO";
      // Brief "produced" beat before the shell takes over.
      root.querySelector(".login-card")?.classList.add("launch");
      window.setTimeout(() => onSuccess(session, serverUrl), 420);
    } catch (err) {
      // Tauri command errors arrive as the Spanish string we returned in Rust.
      showError(typeof err === "string" ? err : "Error inesperado al iniciar sesión.");
      btn.disabled = false;
      btn.classList.remove("loading");
      label.textContent = "ENTRAR";
    }
  });
}
