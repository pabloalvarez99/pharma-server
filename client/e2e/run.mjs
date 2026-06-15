// E2E entrypoint — `npm run e2e`.
//
// One command: build (cached) -> temp DB -> bootstrap two tenants/users via the
// CLI -> boot pharma-api -> run the golden path for BOTH verticals (pharmacy +
// minimarket) over real HTTP -> tear everything down. Exit non-zero on any
// assertion failure so it works as a local gate (CI is billing-walled — this is
// a LOCAL gate, see client/e2e/README.md).

import {
  buildBinaries,
  cli,
  startServer,
  waitReady,
  makeTempDb,
  cleanTempDb,
  summary,
  section,
} from "./lib/harness.mjs";
import { goldenPath, goodsReceiptFlow, complianceFlow } from "./flows.mjs";

const PASSWORD = "e2e-pass-1234";
const EMAIL = "admin@e2e.cl";
const TENANTS = [
  { slug: "e2e-farmacia", name: "Farmacia E2E", vertical: "pharmacy" },
  { slug: "e2e-mini", name: "Minimarket E2E", vertical: "minimarket" },
];

let server;
let dbPath;

async function main() {
  await buildBinaries();

  dbPath = makeTempDb();
  console.log(`• temp DB: ${dbPath}`);

  // Bootstrap with the server DOWN (SurrealKv single-writer file lock).
  section("bootstrap (CLI, server down)");
  await cli(["migrate"], dbPath);
  console.log("  ✓ migrations applied");
  for (const t of TENANTS) {
    await cli(["tenant-create", t.name, "--slug", t.slug], dbPath);
    await cli(
      [
        "user-create",
        "--tenant",
        t.slug,
        "--email",
        EMAIL,
        "--roles",
        "admin,owner",
        "--password",
        PASSWORD,
      ],
      dbPath,
    );
    console.log(`  ✓ tenant + admin user: ${t.slug}`);
  }

  // Boot once; both tenants share the multi-tenant server.
  section("boot pharma-api");
  server = startServer(dbPath);
  await waitReady(server);
  console.log("  ✓ server ready");

  for (const t of TENANTS) {
    const ctx = { tenant: t.slug, email: EMAIL, password: PASSWORD, vertical: t.vertical };
    await goldenPath(ctx);
    await goodsReceiptFlow(ctx);
    await complianceFlow(ctx);
  }
}

main()
  .then(async () => {
    const ok = summary();
    if (server) await server.stop();
    cleanTempDb(dbPath);
    process.exit(ok ? 0 : 1);
  })
  .catch(async (e) => {
    console.error("\nE2E ABORTED:", e.message);
    if (server) {
      const log = server.getLog().trim().split("\n").slice(-25).join("\n");
      if (log) console.error("\n--- server log (tail) ---\n" + log);
      await server.stop();
    }
    cleanTempDb(dbPath);
    process.exit(1);
  });
