// Admin settings wrappers (client/src-tauri/src/commands/settings.rs).
import { invoke } from "@tauri-apps/api/core";

/** A key/value admin setting (`/settings/{key}`). `value` is always a STRING;
 *  its meaning depends on the key (boolean "true"/"false", number, free text). */
export interface AdminSetting {
  key: string;
  value: string;
  updated_at: string;
}

/** GET /api/v1/settings/{key} (Bearer). Unset key → `null` (404 mapped). */
export function getSetting(
  serverUrl: string,
  key: string,
): Promise<AdminSetting | null> {
  return invoke<AdminSetting | null>("get_setting", { serverUrl, key });
}

/** PUT /api/v1/settings/{key} (Bearer, admin+). Upserts and returns the value. */
export function setSetting(
  serverUrl: string,
  key: string,
  value: string,
): Promise<AdminSetting> {
  return invoke<AdminSetting>("set_setting", { serverUrl, key, value });
}
