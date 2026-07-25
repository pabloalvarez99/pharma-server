// Sucursales — stock POR local + transferencias entre locales (V2).
//
// Responde las dos preguntas que el dueño con más de un local se hace todos los
// días: "¿cuánto tengo de esto en cada parte?" y "¿cómo mando 20 del local 1 al
// local 2?". El stock por sucursal sale del ledger de movimientos, así que lo que
// se ve acá es exactamente lo que el POS puede vender en cada local.
//
// La columna "Total" es el stock global del producto — si alguna vez no cuadra
// con la suma de las columnas, hay un bug de inventario, no de esta vista.
//
// Móvil-first: la tabla scrollea horizontal dentro de su tarjeta y la
// transferencia es un modal de 4 campos con teclado (Enter confirma, Esc cierra).
import {
  listSucursales,
  stockPorSucursalReporte,
  transferirStock,
  sucursalActiva,
  CASA_MATRIZ,
  type Sucursal,
  type StockSucursalReporte,
} from "../api";
import { num } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./view-blocks";
import { emptyState, errorState } from "./ui";
import { bindModalKeys } from "./modal-keys";
import "./rutbrand.css";

/** Columna del pivote: casa matriz primero, después cada sucursal activa. */
interface Columna {
  id: string; // CASA_MATRIZ | branch:<key>
  label: string;
}

function columnas(sucursales: Sucursal[]): Columna[] {
  return [
    { id: CASA_MATRIZ, label: "Casa matriz" },
    ...sucursales.map((s) => ({ id: s.id, label: s.name })),
  ];
}

/** Stock de una fila del reporte en una columna. `CASA_MATRIZ` = branch null. */
function stockEn(row: StockSucursalReporte, col: string): number {
  const slice = row.by_branch.find((s) =>
    col === CASA_MATRIZ ? s.branch === null : s.branch === col,
  );
  return slice?.stock ?? 0;
}

export function renderSucursales(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-sucursales">
      <div class="view-head">
        <div>
          <h2 class="rb-display">Sucursales</h2>
          <p class="muted">Cuánto tienes en cada local y cómo moverlo entre ellos.</p>
        </div>
        <div class="suc-actions">
          <label class="suc-toggle">
            <input id="suc-nonzero" type="checkbox" checked />
            Sólo con stock
          </label>
          <button id="suc-transferir" type="button" class="rb-btn primary">Transferir stock</button>
        </div>
      </div>

      <div class="table-card rb-card">
        <div id="suc-table-host">${tableSkeleton(6)}</div>
      </div>
      <div id="suc-modal"></div>
      <div id="suc-toast" class="toast" hidden></div>
    </section>
  `;

  const tableHost = host.querySelector<HTMLElement>("#suc-table-host")!;
  const modalHost = host.querySelector<HTMLElement>("#suc-modal")!;
  const nonZeroEl = host.querySelector<HTMLInputElement>("#suc-nonzero")!;
  const transferBtn = host.querySelector<HTMLButtonElement>("#suc-transferir")!;
  const toastEl = host.querySelector<HTMLElement>("#suc-toast")!;

  // Confirmación visible de cada acción (vara UX #8: nunca dejar al operador sin
  // saber si pasó algo). Mismo patrón local que Caja.
  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  let sucursales: Sucursal[] = [];
  let filas: StockSucursalReporte[] = [];

  async function load(): Promise<void> {
    tableHost.innerHTML = tableSkeleton(6);
    try {
      [sucursales, filas] = await Promise.all([
        listSucursales(serverUrl, true),
        stockPorSucursalReporte(serverUrl, { nonZero: nonZeroEl.checked }),
      ]);
      paint();
    } catch (err) {
      tableHost.innerHTML = errorState(asMessage(err), {
        retry: { id: "suc-retry", label: "Reintentar" },
      });
      tableHost
        .querySelector<HTMLButtonElement>("#suc-retry")
        ?.addEventListener("click", () => void load());
    }
  }

  function paint(): void {
    if (sucursales.length === 0) {
      tableHost.innerHTML = emptyState({
        title: "Todavía tienes un solo local",
        hint: "Crea tus sucursales en Configuración y acá vas a ver cuánto stock hay en cada una y podrás moverlo entre ellas.",
      });
      transferBtn.disabled = true;
      return;
    }
    transferBtn.disabled = false;
    if (filas.length === 0) {
      tableHost.innerHTML = emptyState({
        title: nonZeroEl.checked ? "Sin stock en ningún local" : "Sin productos con inventario",
        hint: nonZeroEl.checked
          ? "Destilda «Sólo con stock» para ver también los productos en cero."
          : "Carga inventario desde Compras o Inventario y va a aparecer acá.",
      });
      return;
    }

    const cols = columnas(sucursales);
    const activa = sucursalActiva();
    tableHost.innerHTML = `
      <div class="table-scroll">
        <table class="data-table suc-table">
          <thead>
            <tr>
              <th scope="col">Producto</th>
              ${cols
                .map(
                  (c) =>
                    `<th scope="col" class="num ${c.id === activa ? "suc-col-activa" : ""}">${escapeHtml(c.label)}</th>`,
                )
                .join("")}
              <th scope="col" class="num">Total</th>
            </tr>
          </thead>
          <tbody>
            ${filas
              .map(
                (r) => `
              <tr>
                <td>${escapeHtml(r.product_name)}</td>
                ${cols
                  .map((c) => {
                    const n = stockEn(r, c.id);
                    return `<td class="num ${c.id === activa ? "suc-col-activa" : ""} ${n <= 0 ? "muted" : ""}">${num(n)}</td>`;
                  })
                  .join("")}
                <td class="num"><strong>${num(r.total)}</strong></td>
              </tr>`,
              )
              .join("")}
          </tbody>
        </table>
      </div>
    `;
  }

  nonZeroEl.addEventListener("change", () => void load());
  transferBtn.addEventListener("click", () => {
    openTransferModal(modalHost, serverUrl, sucursales, filas, load, toast);
  });

  void load();
}

// --- transferencia ---------------------------------------------------------

function openTransferModal(
  modalHost: HTMLElement,
  serverUrl: string,
  sucursales: Sucursal[],
  filas: StockSucursalReporte[],
  onDone: () => Promise<void> | void,
  toast: (msg: string) => void,
): void {
  const cols = columnas(sucursales);
  const activa = sucursalActiva();
  // Destino por defecto = donde estoy parado; origen = el primer lugar distinto.
  const destinoDefault = activa;
  const origenDefault = cols.find((c) => c.id !== destinoDefault)?.id ?? CASA_MATRIZ;

  const opciones = (sel: string) =>
    cols
      .map(
        (c) =>
          `<option value="${escapeHtml(c.id)}" ${c.id === sel ? "selected" : ""}>${escapeHtml(c.label)}</option>`,
      )
      .join("");

  modalHost.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal rb-card" role="dialog" aria-modal="true" aria-labelledby="tr-title">
        <h3 id="tr-title" class="rb-display">Transferir stock</h3>
        <p class="muted">Mueve mercadería de un local a otro. El total del negocio no cambia: sólo cambia dónde está.</p>

        <label class="field">Producto
          <select id="tr-product" class="view-select">
            ${filas
              .map(
                (r) =>
                  `<option value="${escapeHtml(r.product)}">${escapeHtml(r.product_name)}</option>`,
              )
              .join("")}
          </select>
        </label>

        <div class="tr-row">
          <label class="field">Desde
            <select id="tr-from" class="view-select">${opciones(origenDefault)}</select>
          </label>
          <label class="field">Hacia
            <select id="tr-to" class="view-select">${opciones(destinoDefault)}</select>
          </label>
        </div>

        <p id="tr-disponible" class="muted"></p>

        <label class="field">Cantidad
          <input id="tr-qty" type="number" inputmode="numeric" min="1" step="1" value="1" />
        </label>
        <label class="field">Nota (opcional)
          <input id="tr-notes" type="text" maxlength="120" placeholder="Ej: reposición del fin de semana" />
        </label>

        <p id="tr-error" class="form-error" hidden></p>
        <div class="modal-actions">
          <button id="tr-cancel" type="button" class="rb-btn ghost">Cancelar</button>
          <button id="tr-confirm" type="button" class="rb-btn primary">Transferir</button>
        </div>
      </div>
    </div>
  `;

  const productEl = modalHost.querySelector<HTMLSelectElement>("#tr-product")!;
  const fromEl = modalHost.querySelector<HTMLSelectElement>("#tr-from")!;
  const toEl = modalHost.querySelector<HTMLSelectElement>("#tr-to")!;
  const qtyEl = modalHost.querySelector<HTMLInputElement>("#tr-qty")!;
  const notesEl = modalHost.querySelector<HTMLInputElement>("#tr-notes")!;
  const dispEl = modalHost.querySelector<HTMLElement>("#tr-disponible")!;
  const errEl = modalHost.querySelector<HTMLElement>("#tr-error")!;
  const confirmBtn = modalHost.querySelector<HTMLButtonElement>("#tr-confirm")!;

  const detachKeys = bindModalKeys(
    () => close(),
    () => confirmBtn.click(),
  );
  const close = () => {
    detachKeys();
    modalHost.innerHTML = "";
  };

  /** Cuánto hay realmente en el origen elegido — el operador no debería tener
   *  que adivinar ni descubrirlo con un error del servidor. */
  function disponible(): number {
    const row = filas.find((r) => r.product === productEl.value);
    return row ? stockEn(row, fromEl.value) : 0;
  }

  function refreshDisponible(): void {
    const d = disponible();
    const origen = cols.find((c) => c.id === fromEl.value)?.label ?? "el origen";
    dispEl.textContent =
      d > 0
        ? `Disponible en ${origen}: ${num(d)}`
        : `${origen} no tiene stock de este producto.`;
    qtyEl.max = String(Math.max(d, 1));
  }

  [productEl, fromEl].forEach((el) => el.addEventListener("change", refreshDisponible));
  refreshDisponible();

  modalHost.querySelector<HTMLElement>(".modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  modalHost.querySelector<HTMLButtonElement>("#tr-cancel")!.addEventListener("click", close);
  qtyEl.focus();
  qtyEl.select();

  confirmBtn.addEventListener("click", async () => {
    const qty = Math.trunc(Number(qtyEl.value));
    const fail = (msg: string) => {
      errEl.textContent = msg;
      errEl.hidden = false;
    };
    if (!productEl.value) return fail("Elige el producto a transferir.");
    if (fromEl.value === toEl.value) {
      return fail("El origen y el destino tienen que ser locales distintos.");
    }
    if (!Number.isFinite(qty) || qty <= 0) return fail("Ingresa una cantidad mayor a 0.");
    const disp = disponible();
    if (qty > disp) {
      return fail(
        `No alcanza: en el origen hay ${num(disp)} y estás moviendo ${num(qty)}.`,
      );
    }

    errEl.hidden = true;
    confirmBtn.disabled = true;
    confirmBtn.classList.add("loading");
    try {
      const res = await transferirStock(serverUrl, {
        product: productEl.value,
        fromBranch: fromEl.value,
        toBranch: toEl.value,
        qty,
        notes: notesEl.value.trim() || undefined,
      });
      close();
      await onDone();
      const destino = cols.find((c) => c.id === toEl.value)?.label ?? "el destino";
      toast(
        `${num(res.qty)} × ${res.product_name} a ${destino}. Quedan ${num(res.from_stock)} en el origen.`,
      );
    } catch (err) {
      // El server manda `CODE|mensaje`; INSUFFICIENT_STOCK significa que alguien
      // más movió o vendió mientras el modal estaba abierto.
      const raw = asMessage(err);
      const [code, ...rest] = raw.split("|");
      fail(
        code === "INSUFFICIENT_STOCK"
          ? "El origen ya no tiene esa cantidad (alguien vendió o movió stock recién). Revisa el disponible y reintenta."
          : rest.join("|") || raw,
      );
      confirmBtn.disabled = false;
      confirmBtn.classList.remove("loading");
    }
  });
}
