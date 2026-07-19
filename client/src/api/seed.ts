// Demo seeding wrapper, multi-rubro onboarding (client/src-tauri/src/commands/seed.rs).
import { invoke } from "@tauri-apps/api/core";

/** Outcome of `POST /api/v1/admin/seed-demo` (`domain::seed::SeedSummary`). */
export interface SeedSummary {
  vertical: string;
  products_created: number;
  batches_created: number;
  movements_emitted: number;
  wiped: number;
}

/** Sentinel the `seed_demo` command rejects with when demo data already exists
 *  and `force` was false (server 409) — the view offers a "regenerar" confirm. */
export const SEED_ALREADY_EXISTS = "SEED_ALREADY_EXISTS";

/** POST /api/v1/admin/seed-demo (Bearer, admin/owner) — fill the tenant with a
 *  believable DEMO catalog for `vertical` (`pharmacy` | `minimarket`). `force`
 *  wipes the prior demo pack before re-seeding. Rejects with
 *  {@link SEED_ALREADY_EXISTS} on a 409, or a Spanish string otherwise. */
export function seedDemo(
  serverUrl: string,
  vertical: string,
  force: boolean,
): Promise<SeedSummary> {
  return invoke<SeedSummary>("seed_demo", { serverUrl, vertical, force });
}
