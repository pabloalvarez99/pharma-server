// @vitest-environment happy-dom
//
// SCAFFOLD — Centro de Configuración e2e (W5, follow-up of ye's Pilar A lane).
//
// The config hub (nav + per-section cards, human labels, search-in-settings,
// save feedback) is NOT in the base yet — `configuracion.ts` is still the flat
// "Parámetros del servidor" key/value list. Per the W5 charter, when ye's hub
// PR lands in feature/erp-parity these `it.todo`s become real DOM assertions
// against `renderConfiguracion(host, serverUrl)`. Kept as todos (not skipped
// bodies) so they never assert against the soon-to-be-replaced flat surface and
// never go red, while keeping the slot discoverable.
//
// Contract to cover once landed (professional-completeness-master-plan §Pilar A):
import { describe, it } from "vitest";

describe("Centro de Configuración (config hub)", () => {
  it.todo("renders a lateral nav with the planned sections (Negocio, Usuarios, SII, Respaldo, Licencia, Preferencias, Agente)");
  it.todo("shows human labels, never the raw admin_setting cfg-key (cero 'de dev')");
  it.todo("filters sections/fields via the in-settings search box");
  it.todo("saves a setting and surfaces clear success feedback (produced state, not bare text)");
  it.todo("validates inline and blocks an invalid value with a readable message");
  it.todo("mounts the rubro-select vitrina inside the hub");
  it.todo("renders empty/loading/error through the ui.ts design-system helpers");
});
