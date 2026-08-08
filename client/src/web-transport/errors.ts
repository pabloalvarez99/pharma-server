// Port of client/src-tauri/src/http.rs for the web build: same Spanish error
// strings, same envelope parsing, same status fallbacks. The desktop client and
// the browser MUST show identical copy (SP3 ley #3).

/** Max total time for a regular API call (http.rs API_TIMEOUT = 30s). */
export const API_TIMEOUT_MS = 30_000;
/** The health probe should feel instant on the login screen (5s). */
export const HEALTH_TIMEOUT_MS = 5_000;

/** Trim trailing slashes so `serverUrl` + "/path" never doubles up. */
export function base(serverUrl: string): string {
  return serverUrl.replace(/\/+$/, "");
}

/** Server error envelope (`crates/api/src/error.rs`):
 *  `{ "error": { "code", "message", "details"? } }`. */
interface ErrorEnvelope {
  error?: { code?: unknown; message?: unknown };
}

/** Connection-level failures (server down, wrong URL, timeout) → friendly
 *  Spanish copy. fetch rejects with TypeError on network failure and with a
 *  DOMException (TimeoutError/AbortError) on AbortSignal.timeout. */
export function connError(e: unknown): string {
  const aborted =
    e instanceof DOMException && (e.name === "TimeoutError" || e.name === "AbortError");
  if (aborted || e instanceof TypeError) {
    return "No se pudo conectar al servidor. Verifica la URL y que pharma-server esté corriendo.";
  }
  return `Error de red: ${e instanceof Error ? e.message : String(e)}`;
}

async function envelopeOf(resp: Response): Promise<{ code: string; message: string } | null> {
  const body = await resp.text().catch(() => "");
  try {
    const env = JSON.parse(body) as ErrorEnvelope;
    if (env && env.error && typeof env.error.message === "string") {
      return {
        code: typeof env.error.code === "string" ? env.error.code : "",
        message: env.error.message,
      };
    }
  } catch {
    /* non-JSON body → status fallback */
  }
  return null;
}

/** Status→Spanish fallback. `coded` adds the 422 arm `coded_error` has and
 *  `error_message` doesn't (http.rs keeps them separate). */
function statusFallback(status: number, coded: boolean): string {
  switch (status) {
    case 401:
      return "Credenciales inválidas.";
    case 403:
      return "Permiso denegado para esta operación.";
    case 404:
      return "Recurso no encontrado en el servidor.";
    case 422:
      if (coded) return "No se pudo procesar la venta.";
      break;
    case 503:
      return "Servicio no disponible. Intenta nuevamente.";
  }
  return `Error del servidor (${status}).`;
}

/** Map a non-2xx response to a Spanish message (http.rs `error_message`). */
export async function errorMessage(resp: Response): Promise<string> {
  const env = await envelopeOf(resp);
  return env ? env.message : statusFallback(resp.status, false);
}

/** Like `errorMessage` but preserves the server `code` as `"CODE|message"`
 *  (http.rs `coded_error`); no envelope → `"|message"`. */
export async function codedError(resp: Response): Promise<string> {
  const env = await envelopeOf(resp);
  if (env) return `${env.code}|${env.message}`;
  return `|${statusFallback(resp.status, true)}`;
}
