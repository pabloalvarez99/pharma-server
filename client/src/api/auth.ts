// Auth + session wrappers around the Rust Tauri commands (client/src-tauri/src/commands/auth.rs).
// Field shapes mirror the server contract in crates/api.
import { invoke } from "@tauri-apps/api/core";

export interface SessionInfo {
  user_id: string;
  tenant_id: string;
  roles: string[];
  expires_in: number;
}

export interface HealthInfo {
  status: string; // "ok" | "degraded"
  db: string;
  reachable: boolean;
}

/** POST /api/v1/login + GET /api/v1/me. Throws a Spanish error string on failure. */
export function login(
  serverUrl: string,
  tenant: string,
  email: string,
  password: string,
): Promise<SessionInfo> {
  return invoke<SessionInfo>("login", {
    serverUrl,
    tenant,
    email,
    password,
  });
}

/** GET /api/v1/setup/status — UNAUTHENTICATED. `needs_setup=true` on a fresh
 *  install with no account yet (login screen offers in-app account creation). */
export interface SetupStatusInfo {
  needs_setup: boolean;
}
export function setupStatus(serverUrl: string): Promise<SetupStatusInfo> {
  return invoke<SetupStatusInfo>("setup_status", { serverUrl });
}

/** Input for the first-run account-creation form. */
export interface SetupInput {
  businessName: string;
  /** Optional branch slug; server derives one from the name when omitted. */
  tenantSlug?: string;
  email: string;
  password: string;
  /** Chosen rubro (e.g. "farmacia" | "minimarket" | "otro"). */
  vertical?: string;
}

/** A live session plus the slug the server assigned (for "Sucursal" pre-fill). */
export interface SetupSession extends SessionInfo {
  tenant_slug: string;
}

/** POST /api/v1/setup — create the first tenant+owner, then log straight in.
 *  Throws a Spanish error string on failure (e.g. 409 if already configured). */
export function setupAccount(serverUrl: string, input: SetupInput): Promise<SetupSession> {
  return invoke<SetupSession>("setup_account", {
    serverUrl,
    businessName: input.businessName,
    tenantSlug: input.tenantSlug ?? null,
    email: input.email,
    password: input.password,
    vertical: input.vertical ?? null,
  });
}

/** GET /health/ready. */
export function serverHealth(serverUrl: string): Promise<HealthInfo> {
  return invoke<HealthInfo>("server_health", { serverUrl });
}

/** Forget the in-memory JWT. The token is a `SecretString` in Rust — zeroed on
 *  drop. Server-side the JWT is stateless (TTL 3600s); revocation is pending. */
export function logout(): Promise<void> {
  return invoke<void>("logout");
}
