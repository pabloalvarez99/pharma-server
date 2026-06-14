// Entry point: `npm run e2e`. Provisions a throwaway DB, seeds both verticals,
// boots pharma-api, runs the golden paths, tears everything down, and exits
// non-zero on any failed assertion. See ./README.md for the design rationale.
import { existsSync } from "node:fs";
import {
  PHARMA,
  PHARMA_API,
  makeEnv,
  cleanup,
  migrate,
  createTenant,
  createAdmin,
  seedDemo,
  startServer,
  stopServer,
  summary,
} from "./harness.mjs";
import { runVertical } from "./scenarios.mjs";

function requireBins() {
  const missing = [PHARMA, PHARMA_API].filter((p) => !existsSync(p));
  if (missing.length) {
    console.error(
      `Missing built binaries:\n${missing.map((m) => "  " + m).join("\n")}\n` +
        `Build them from the repo root:  cargo build -p api -p cli`
    );
    process.exit(2);
  }
}

async function main() {
  requireBins();
  const env = makeEnv();
  let server = null;

  try {
    console.log("• migrate + seed (pharmacy, minimarket)…");
    migrate(env);
    createTenant(env, "Farmacia Demo", "farmacia-demo");
    createTenant(env, "Minimarket Demo", "minimarket-demo");
    createAdmin(env, "farmacia-demo", "admin@farmacia.cl");
    createAdmin(env, "minimarket-demo", "admin@minimarket.cl");
    seedDemo(env, "farmacia-demo", "pharmacy");
    seedDemo(env, "minimarket-demo", "minimarket");

    console.log("• boot pharma-api…");
    server = await startServer(env);

    console.log("• scenarios: pharmacy");
    await runVertical("pharmacy");
    console.log("• scenarios: minimarket");
    await runVertical("minimarket");
  } catch (e) {
    console.error(`\nE2E harness error: ${e.message}`);
    stopServer(server);
    cleanup();
    process.exit(1);
  }

  stopServer(server);
  cleanup();
  process.exit(summary() ? 0 : 1);
}

main();
