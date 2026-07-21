// Web port of commands/auth.rs: login, first-run setup, logout, health.
// The token flow mirrors the desktop exactly: login/setup POST → GET /me to
// enrich identity → store the JWT (here: memory + sessionStorage).

import { base, errorMessage, HEALTH_TIMEOUT_MS } from "../errors";
import {
  type CommandArgs,
  type CommandHandler,
  doFetch,
  JSON_HEADERS,
  parseJson,
} from "../core";
import { clearToken, storeToken } from "../session";

interface LoginResponse {
  token: string;
  expires_in: number;
}

interface MeResponse {
  sub: string;
  tenant_id: string;
  roles: string[];
}

interface SetupResponse extends LoginResponse {
  tenant_slug: string;
}

async function fetchMe(b: string, token: string): Promise<MeResponse> {
  const resp = await doFetch(`${b}/api/v1/me`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson<MeResponse>(resp, "Respuesta de sesión inválida del servidor");
}

async function login(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/login`, {
    method: "POST",
    headers: JSON_HEADERS,
    body: JSON.stringify({ tenant: a.tenant, email: a.email, password: a.password }),
  });
  if (!resp.ok) throw await errorMessage(resp);
  const loginResp = await parseJson<LoginResponse>(
    resp,
    "Respuesta de login inválida del servidor",
  );
  const me = await fetchMe(b, loginResp.token);
  storeToken(loginResp.token);
  return {
    user_id: me.sub,
    tenant_id: me.tenant_id,
    roles: me.roles,
    expires_in: loginResp.expires_in,
  };
}

async function setupStatus(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/setup/status`);
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de estado de instalación inválida");
}

async function setupAccount(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/setup`, {
    method: "POST",
    headers: JSON_HEADERS,
    body: JSON.stringify({
      business_name: a.businessName,
      tenant_slug: a.tenantSlug ?? null,
      email: a.email,
      password: a.password,
      vertical: a.vertical ?? null,
    }),
  });
  if (!resp.ok) throw await errorMessage(resp);
  const setup = await parseJson<SetupResponse>(
    resp,
    "Respuesta de instalación inválida del servidor",
  );
  const me = await fetchMe(b, setup.token);
  storeToken(setup.token);
  return {
    user_id: me.sub,
    tenant_id: me.tenant_id,
    roles: me.roles,
    expires_in: setup.expires_in,
    tenant_slug: setup.tenant_slug,
  };
}

async function serverHealth(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/health/ready`, { timeoutMs: HEALTH_TIMEOUT_MS });
  const ok = resp.ok;
  try {
    const r = (await resp.json()) as { status?: unknown; checks?: { db?: unknown } };
    if (typeof r?.status === "string" && typeof r?.checks?.db === "string") {
      return { status: r.status, db: r.checks.db, reachable: true };
    }
  } catch {
    /* same fallback as the desktop: unparseable body is still "reachable" */
  }
  return { status: ok ? "ok" : "degraded", db: "desconocido", reachable: true };
}

export const authCommands: Record<string, CommandHandler> = {
  login,
  setup_status: setupStatus,
  setup_account: setupAccount,
  server_health: serverHealth,
  logout: async () => {
    clearToken();
    return null;
  },
};
