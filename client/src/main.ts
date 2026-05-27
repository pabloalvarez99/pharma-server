// Entry point + the two-phase router (LoginView -> AppShell). ONE app, TWO
// phases — no second binary, no full SPA framework.
import { renderLogin } from "./views/login";
import { renderShell } from "./views/shell";
import type { SessionInfo } from "./api";

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
