// Shared E2E primitives: process control, the pharma CLI, an HTTP client bound
// to one tenant's bearer token, and a tiny assertion tally. Dependency-free
// (Node ESM only) so `npm run e2e` needs nothing installed.
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, existsSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const TMP_BASE = join(HERE, ".tmp"); // client/e2e/.tmp — gitignored.
// client/e2e → repo root is two up.
export const REPO_ROOT = resolve(HERE, "..", "..");
const EXE = process.platform === "win32" ? ".exe" : "";
export const PHARMA = join(REPO_ROOT, "target", "debug", `pharma${EXE}`);
export const PHARMA_API = join(REPO_ROOT, "target", "debug", `pharma-api${EXE}`);
const MIGRATIONS = join(REPO_ROOT, "migrations");

export const PORT = Number(process.env.E2E_PORT ?? 18099);
export const BASE = `http://127.0.0.1:${PORT}`;
export const JWT_SECRET = "e2e-deterministic-secret";
export const PASSWORD = "e2e-pass-1234";

// --- assertion tally -------------------------------------------------------
let passed = 0;
const failures = [];

/** Assert `cond`, recording the outcome under `label`. Never throws — a failed
 *  step keeps the scenario going so one run surfaces every break at once. */
export function check(label, cond, detail = "") {
  if (cond) {
    passed += 1;
    console.log(`  ✓ ${label}`);
  } else {
    failures.push(label + (detail ? ` — ${detail}` : ""));
    console.error(`  ✗ ${label}${detail ? ` — ${detail}` : ""}`);
  }
}

export function summary() {
  console.log(`\n${passed} passed, ${failures.length} failed`);
  for (const f of failures) console.error(`  FAIL: ${f}`);
  return failures.length === 0;
}

// --- env / temp DB ---------------------------------------------------------
let tmpRoot = null;

/** Env shared by every `pharma*` process: throwaway DB, fixed jwt + bind,
 *  rate-limit + docs off so the harness isn't throttled. */
export function makeEnv() {
  mkdirSync(TMP_BASE, { recursive: true });
  tmpRoot = mkdtempSync(join(TMP_BASE, "run-"));
  const dbPath = join(tmpRoot, "surreal");
  return {
    ...process.env,
    PHARMA__DB__PATH: dbPath,
    PHARMA__JWT__SECRET: JWT_SECRET,
    PHARMA__BIND: `127.0.0.1:${PORT}`,
    PHARMA__RATE_LIMIT__ENABLED: "false",
    PHARMA__DOCS__ENABLED: "false",
    PHARMA__CRL__URL: "",
  };
}

export function cleanup() {
  if (tmpRoot && existsSync(tmpRoot)) {
    try {
      rmSync(tmpRoot, { recursive: true, force: true });
    } catch (e) {
      console.warn(`temp cleanup failed (${tmpRoot}): ${e.message}`);
    }
  }
}

// --- pharma CLI ------------------------------------------------------------
/** Run a `pharma` subcommand synchronously; throws on non-zero so setup fails
 *  fast. `password` is passed via PHARMA_PASSWORD env, never argv. */
export function cli(args, env, { password } = {}) {
  const childEnv = password ? { ...env, PHARMA_PASSWORD: password } : env;
  const r = spawnSync(PHARMA, args, {
    env: childEnv,
    cwd: REPO_ROOT,
    encoding: "utf8",
  });
  if (r.status !== 0) {
    throw new Error(
      `pharma ${args.join(" ")} exited ${r.status}\n${r.stdout}\n${r.stderr}`
    );
  }
  return r.stdout;
}

export function migrate(env) {
  cli(["migrate", "--dir", MIGRATIONS], env);
}

export function createTenant(env, name, slug) {
  cli(["tenant-create", name, "--slug", slug], env);
}

export function createAdmin(env, slug, email) {
  cli(
    ["user-create", "--tenant", slug, "--email", email, "--roles", "admin,cashier"],
    env,
    { password: PASSWORD }
  );
}

export function seedDemo(env, slug, vertical) {
  cli(["seed-demo", "--tenant", slug, "--vertical", vertical], env);
}

// --- server lifecycle ------------------------------------------------------
/** Boot pharma-api and resolve once `/health/ready` returns 200, or reject. */
export async function startServer(env) {
  const proc = spawn(PHARMA_API, [], { env, cwd: REPO_ROOT, stdio: "pipe" });
  let stderr = "";
  proc.stderr.on("data", (d) => (stderr += d.toString()));
  proc.on("exit", (code) => {
    if (code !== 0 && code !== null) {
      console.error(`pharma-api exited early (${code}):\n${stderr}`);
    }
  });

  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (proc.exitCode !== null) {
      throw new Error(`pharma-api died during boot:\n${stderr}`);
    }
    try {
      const res = await fetch(`${BASE}/health/ready`);
      if (res.status === 200) return proc;
    } catch {
      // not listening yet
    }
    await sleep(400);
  }
  proc.kill();
  throw new Error(`pharma-api never became ready:\n${stderr}`);
}

export function stopServer(proc) {
  if (proc && proc.exitCode === null) proc.kill();
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// --- HTTP client (one bearer per tenant) -----------------------------------
export class Client {
  constructor() {
    this.token = null;
  }

  async req(method, path, body) {
    const headers = { "content-type": "application/json" };
    if (this.token) headers.authorization = `Bearer ${this.token}`;
    const res = await fetch(`${BASE}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const text = await res.text();
    let json;
    try {
      json = text ? JSON.parse(text) : null;
    } catch {
      json = text;
    }
    return { status: res.status, body: json };
  }

  get = (p) => this.req("GET", p);
  post = (p, b) => this.req("POST", p, b);
  put = (p, b) => this.req("PUT", p, b);

  async login(tenant, email) {
    const r = await this.post("/api/v1/login", { tenant, email, password: PASSWORD });
    if (r.status !== 200) {
      throw new Error(`login ${tenant}/${email} → ${r.status}: ${JSON.stringify(r.body)}`);
    }
    this.token = r.body.token;
    return r.body;
  }
}
