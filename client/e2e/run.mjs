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
  REPO_ROOT,
} from "./lib/harness.mjs";
import {
  goldenPath,
  goodsReceiptFlow,
  complianceFlow,
  noPrescriptionFlow,
  dteLifecycleFlow,
  reports402Matrix,
  DTE_EMISOR_RUT,
  DTE_CERT_PASS,
} from "./flows.mjs";
import { writeCaf } from "./lib/caf.mjs";
import { join } from "node:path";

const PASSWORD = "e2e-pass-1234";
const EMAIL = "admin@e2e.cl";
const TENANTS = [
  { slug: "e2e-farmacia", name: "Farmacia E2E", vertical: "pharmacy" },
  { slug: "e2e-mini", name: "Minimarket E2E", vertical: "minimarket" },
];
// Dedicated tenant for the DTE document lifecycle: it gets a digital cert + CAFs
// wired in below (server down), so its boleta/factura/nota emit for REAL. Kept
// separate so the golden-path tenants above still exercise the *Free, no-CAF*
// clean-gate contract (a fresh install with nothing configured).
const DTE_TENANT = { slug: "e2e-dte", name: "DTE Lifecycle E2E", vertical: "pharmacy" };
const TEST_PFX = join(REPO_ROOT, "crates", "dte", "tests", "assets", "test-cert.pfx");

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
  for (const t of [...TENANTS, DTE_TENANT]) {
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

  // DTE lifecycle prerequisites (server still down — CLI holds the file lock):
  // a digital cert + folio CAFs the live emit path needs. Boleta 39 gets a
  // 2-folio range (advance assertion), factura 33 a single folio (exhaustion
  // assertion), nota-crédito 61 a wider range; guía 52 gets NONE (sin-CAF case).
  section("dte fixtures (cert + CAF, server down)");
  await cli(
    [
      "cert", "import", TEST_PFX,
      "--tenant", DTE_TENANT.slug,
      "--passphrase-env", "E2E_PFX_PASS",
      "--rut", DTE_EMISOR_RUT,
      "--from", "2020-01-01",
      "--to", "2035-12-31",
    ],
    dbPath,
    { E2E_PFX_PASS: DTE_CERT_PASS },
  );
  console.log("  ✓ digital cert imported");
  const cafs = [
    { tipo: 39, desde: 1, hasta: 2 },
    { tipo: 33, desde: 1, hasta: 1 },
    { tipo: 61, desde: 1, hasta: 10 },
  ];
  for (const spec of cafs) {
    const path = writeCaf(dbPath, { rut: DTE_EMISOR_RUT, ...spec });
    await cli(["caf", "import", path, "--tenant", DTE_TENANT.slug], dbPath);
    console.log(`  ✓ CAF tipo ${spec.tipo} folios ${spec.desde}..${spec.hasta}`);
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
    await reports402Matrix(ctx);
    // Multi-rubro: a non-pharmacy rubro must never be forced through
    // receta/controlados machinery, yet boleta stays universal.
    if (t.vertical === "minimarket") await noPrescriptionFlow(ctx);
  }

  // DTE document lifecycle on its dedicated, fully-provisioned tenant.
  await dteLifecycleFlow({
    tenant: DTE_TENANT.slug,
    email: EMAIL,
    password: PASSWORD,
    vertical: DTE_TENANT.vertical,
  });
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
