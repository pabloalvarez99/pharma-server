// Auto-update (P0.1): silent check on boot against the CDN manifest
// (tauri.conf.json → plugins.updater.endpoints). On a new version: download +
// install in the background, then tell the operator to restart (passive
// installMode applies it on next launch). Any failure is swallowed — an
// offline LAN or missing manifest must never block the login flow.
import { check } from "@tauri-apps/plugin-updater";

export async function checkForUpdates(): Promise<void> {
  try {
    const update = await check();
    if (!update) return;
    await update.downloadAndInstall();
    // installMode "passive": the new MSI runs silently; the old binary keeps
    // running until the app is closed. Ask for a restart, don't force one —
    // killing the app mid-sale would be worse than running stale for a day.
    window.alert(
      `Hay una nueva versión de RutBusiness (${update.version}) instalada.\n` +
        "Cierra y vuelve a abrir la aplicación para usarla.",
    );
  } catch {
    // Offline / CDN down / unsigned build: ignore by design.
  }
}
