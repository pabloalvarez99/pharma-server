// E2E harness primitives — build the server, bootstrap tenants/users via the
// CLI, boot pharma-api on a throwaway SurrealKv tempdir, and talk to it over the
// same /api/v1 endpoints the Tauri client calls (client/src-tauri/src/lib.rs).
//
// Shape = API-level E2E (no webview): the lightest thing that runs on this
// Windows box and still exercises the real server + real DB end-to-end. The
// click-path is mirrored request-for-request from the Tauri command layer, so a
// green run means a real operator flow works against a real build.
//
// CLI bootstrap (migrate + tenant/user create) MUST finish before the server
// boots — SurrealKv is a single-writer file lock, so CLI and server can never
// hold ./data/surreal at once. Seeding happens AFTER boot, over HTTP.

import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
// e2e/lib -> client -> repo root (the cargo workspace).
export const REPO_ROOT = resolve(__dirname, "..", "..", "..");
const TARGET_RELEASE = join(REPO_ROOT, "target", "release");
const EXE = process.platform === "win32" ? ".exe" : "";
export const SERVER_BIN = join(TARGET_RELEASE, `pharma-api${EXE}`);
export const CLI_BIN = join(TARGET_RELEASE, `pharma${EXE}`);

const PORT = Number(process.env.E2E_PORT ?? 18080);
export const BASE_URL = `http://127.0.0.1:${PORT}`;
export const API = `${BASE_URL}/api/v1`;

// --- process helpers --------------------------------------------------------

/** Run a command to completion, streaming nothing unless it fails. Resolves with
 *  {code, stdout, stderr}; rejects on a non-zero exit so bootstrap stops loudly. */
export function run(cmd, args, { cwd = REPO_ROOT, env = {}, allowFail = false } = {}) {
  return new Promise((res, rej) => {
    const p = spawn(cmd, args, {
      cwd,
      env: { ...process.env, ...env },
      shell: false,
    });
    let out = "";
    let err = "";
    p.stdout.on("data", (d) => (out += d));
    p.stderr.on("data", (d) => (err += d));
    p.on("error", rej);
    p.on("close", (code) => {
      if (code === 0 || allowFail) res({ code, stdout: out, stderr: err });
      else rej(new Error(`${cmd} ${args.join(" ")} exited ${code}\n${err || out}`));
    });
  });
}

/** Build pharma-api + pharma (release) once. Skipped if both binaries exist and
 *  E2E_REBUILD is unset — a clean rebuild every run would make the gate glacial. */
export async function buildBinaries() {
  if (!process.env.E2E_REBUILD && existsSync(SERVER_BIN) && existsSync(CLI_BIN)) {
    console.log("• binaries present (set E2E_REBUILD=1 to force) —", TARGET_RELEASE);
    return;
  }
  console.log("• building pharma-api + pharma (release)… first build is slow");
  await new Promise((res, rej) => {
    const p = spawn("cargo", ["build", "--release", "-p", "api", "-p", "cli"], {
      cwd: REPO_ROOT,
      env: process.env,
      stdio: "inherit",
      shell: false,
    });
    p.on("error", rej);
    p.on("close", (c) => (c === 0 ? res() : rej(new Error(`cargo build exited ${c}`))));
  });
}

// --- server lifecycle -------------------------------------------------------

/** Run a `pharma` CLI subcommand against the temp DB. `extraEnv` layers on top
 *  of the DB/JWT env (e.g. a passphrase var for `cert import --passphrase-env`). */
export function cli(args, dbPath, extraEnv = {}) {
  return run(CLI_BIN, args, { env: { ...dbEnv(dbPath), ...extraEnv } });
}

function dbEnv(dbPath) {
  return {
    PHARMA__DB__PATH: dbPath,
    PHARMA__JWT__SECRET: "e2e-test-secret-not-for-prod",
  };
}

/** Boot the server on the temp DB. Returns {proc, dbPath, stop()}.
 *  Caller must `await waitReady()` before hitting endpoints. */
export function startServer(dbPath) {
  const proc = spawn(SERVER_BIN, [], {
    cwd: REPO_ROOT,
    env: {
      ...process.env,
      ...dbEnv(dbPath),
      PHARMA__BIND: `127.0.0.1:${PORT}`,
      PHARMA__RATE_LIMIT__ENABLED: "false",
      PHARMA__DOCS__ENABLED: "false",
      RUST_LOG: process.env.RUST_LOG ?? "warn",
    },
    shell: false,
  });
  let log = "";
  proc.stdout.on("data", (d) => (log += d));
  proc.stderr.on("data", (d) => (log += d));
  const stop = () =>
    new Promise((res) => {
      if (proc.exitCode !== null) return res();
      proc.on("close", () => res());
      proc.kill();
    });
  return { proc, dbPath, stop, getLog: () => log };
}

/** Poll /health/ready until the server reports up (migrations applied) or the
 *  timeout blows. Throws with the server log so a boot failure is diagnosable. */
export async function waitReady(server, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (server.proc.exitCode !== null) {
      throw new Error(`server exited early (${server.proc.exitCode})\n${server.getLog()}`);
    }
    try {
      const r = await fetch(`${BASE_URL}/health/ready`);
      if (r.ok) return;
    } catch {
      /* not listening yet */
    }
    await sleep(300);
  }
  throw new Error(`server not ready in ${timeoutMs}ms\n${server.getLog()}`);
}

export function makeTempDb() {
  return mkdtempSync(join(tmpdir(), "pharma-e2e-"));
}

export function cleanTempDb(dbPath) {
  try {
    rmSync(dbPath, { recursive: true, force: true });
  } catch {
    /* best-effort; Windows may hold a handle briefly */
  }
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// --- authenticated HTTP client ----------------------------------------------

/** Thin client mirroring the Tauri command layer: Bearer token + JSON, and the
 *  same bodies/headers the views send. `expectOk` controls whether a non-2xx is
 *  an assertion failure (true) or a value to inspect (false, e.g. boleta gate). */
export class Client {
  constructor() {
    this.token = null;
  }

  async login(tenant, email, password) {
    const r = await fetch(`${API}/login`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ tenant, email, password }),
    });
    const body = await r.json().catch(() => ({}));
    if (!r.ok || !body.token) {
      throw new Error(`login failed (${r.status}): ${JSON.stringify(body)}`);
    }
    this.token = body.token;
    return body;
  }

  async req(method, path, { body, headers = {}, expectOk = true } = {}) {
    const h = { ...headers };
    if (this.token) h.authorization = `Bearer ${this.token}`;
    if (body !== undefined) h["content-type"] = "application/json";
    const r = await fetch(`${API}${path}`, {
      method,
      headers: h,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
    const text = await r.text();
    let json;
    try {
      json = text ? JSON.parse(text) : null;
    } catch {
      json = text;
    }
    if (expectOk && !r.ok) {
      throw new Error(`${method} ${path} -> ${r.status}: ${text}`);
    }
    return { status: r.status, ok: r.ok, body: json };
  }

  get(path, opts) {
    return this.req("GET", path, opts);
  }
  post(path, body, opts = {}) {
    return this.req("POST", path, { ...opts, body });
  }
  put(path, body, opts = {}) {
    return this.req("PUT", path, { ...opts, body });
  }
}

// --- assertions + tiny runner ------------------------------------------------

let passed = 0;
let failed = 0;
const failures = [];
const knownBugs = [];

/** Characterize a known, already-reported bug (xfail): logged loudly and listed
 *  in the summary, but does NOT fail the gate. The caller guards the broken path
 *  so that when the bug is fixed the real assertion runs again and turns the run
 *  red — the signal to delete the xfail. Mirrors `#[ignore] + note + report`. */
export function knownBug(msg) {
  knownBugs.push(msg);
  console.log(`  ⚠ KNOWN BUG (xfail): ${msg}`);
}

export function check(cond, msg) {
  if (cond) {
    passed++;
    console.log(`  ✓ ${msg}`);
  } else {
    failed++;
    failures.push(msg);
    console.log(`  ✗ ${msg}`);
  }
}

export function eq(actual, expected, msg) {
  check(actual === expected, `${msg} (expected ${expected}, got ${actual})`);
}

export function section(name) {
  console.log(`\n── ${name} ──`);
}

export function summary() {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`E2E: ${passed} passed, ${failed} failed, ${knownBugs.length} known-bug xfail`);
  if (knownBugs.length) {
    console.log("KNOWN BUGS (xfail — see teamwork_op.txt BUG LOG):");
    for (const b of knownBugs) console.log(`  ⚠ ${b}`);
  }
  if (failed) {
    console.log("FAILURES:");
    for (const f of failures) console.log(`  - ${f}`);
  }
  return failed === 0;
}
