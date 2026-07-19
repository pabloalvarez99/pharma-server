// License status wrapper (client/src-tauri/src/commands/license.rs).
import { invoke } from "@tauri-apps/api/core";

export interface LicenseSummary {
  tier: string;
  status: string; // "active" | "grace" | "expired"
  license_id: string;
  features: string[];
  expires_at: string | null;
  key_id: string;
  seat_count: number;
}

/** GET /api/v1/admin/license/status (Bearer, in-memory token). */
export function licenseStatus(serverUrl: string): Promise<LicenseSummary> {
  return invoke<LicenseSummary>("license_status", { serverUrl });
}
