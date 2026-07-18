// Entry point + the two-phase router (LoginView -> AppShell). ONE app, TWO
// phases — no second binary, no full SPA framework.
import "./brand.css"; // RutBusiness design system (tokens + rb-* components).
import { renderLogin } from "./views/login";
import { renderShell } from "./views/shell";
import { currentTheme, applyTheme } from "./views/configuracion";
import { checkForUpdates } from "./updater";
import type { SessionInfo } from "./api";

// Adopt the RutBusiness brand surface + restore the operator's theme before the
// first paint (dark by default — the app's native palette).
document.body.classList.add("rb");
applyTheme(currentTheme());

const root = document.querySelector<HTMLDivElement>("#app")!;

function showLogin(): void {
  renderLogin(root, (session: SessionInfo, serverUrl: string) => {
    showShell(session, serverUrl);
  });
}

function showShell(session: SessionInfo, serverUrl: string): void {
  renderShell(root, session, serverUrl, () => {
    showLogin();
  });
}

// Phase 1 on boot.
showLogin();

// Silent auto-update check (P0.1) — never blocks login, failures ignored.
void checkForUpdates();
