// @vitest-environment happy-dom
//
// V1 fiado — behavioral wiring under a real DOM (the pure producers live in
// cuenta-fiado.test.ts). Drives the REAL renderClientes / renderPos with the api
// mocked (no Tauri/network), proving: the cuenta panel renders saldo +
// movimientos, the abono modal POSTs the parsed amount, the panel DEGRADES
// GRACEFULLY when the backend lacks the surface, and the POS fiar rail is gated
// to a selected customer.
import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return {
    ...actual,
    customerSearch: vi.fn(),
    customerDetail: vi.fn(),
    customerHistory: vi.fn(),
    cuentaCorriente: vi.fn(),
    registrarAbono: vi.fn(),
    listProducts: vi.fn(),
  };
});
vi.mock("../vertical", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../vertical")>();
  return { ...actual, loadRubro: vi.fn().mockResolvedValue("minimarket") };
});

import { renderClientes } from "./clientes";
import { renderPos } from "./pos";
import {
  customerSearch,
  customerDetail,
  customerHistory,
  cuentaCorriente,
  registrarAbono,
  listProducts,
  CUENTA_MODULE_MISSING,
  type Customer,
  type CustomerDetail,
  type CuentaCorriente,
} from "../api";

const mSearch = vi.mocked(customerSearch);
const mDetail = vi.mocked(customerDetail);
const mHistory = vi.mocked(customerHistory);
const mCuenta = vi.mocked(cuentaCorriente);
const mAbono = vi.mocked(registrarAbono);
const mProducts = vi.mocked(listProducts);

const URL = "http://localhost:8080";

const CUST: Customer = {
  id: "customer:1",
  name: "Juan Pérez",
  rut: "11.111.111-1",
  phone: null,
  email: null,
  loyalty_points: 120,
  active: true,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
};
const DETAIL: CustomerDetail = { ...CUST, total_spent: "50000", visit_count: 4 };
const CUENTA: CuentaCorriente = {
  saldo: 12345,
  limite: 50000,
  movimientos: [
    { id: "m1", fecha: "2026-06-20T12:00:00Z", tipo: "cargo", monto: 18000, glosa: "Venta boleta 41", ref: "order:41" },
    { id: "m2", fecha: "2026-06-21T09:00:00Z", tipo: "abono", monto: 5655, glosa: "Abono efectivo", ref: null },
  ],
};

const wait = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));
const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
  mSearch.mockResolvedValue([CUST]);
  mDetail.mockResolvedValue(DETAIL);
  mHistory.mockResolvedValue([]);
  mCuenta.mockResolvedValue(CUENTA);
  mAbono.mockResolvedValue({ id: "m3", fecha: "2026-06-22T10:00:00Z", tipo: "abono", monto: 10000, glosa: "Abono", ref: null });
  mProducts.mockResolvedValue([]);
  // happy-dom has no print() — stub it so the print button never throws.
  (window as unknown as { print: () => void }).print = vi.fn();
});

function mountClientes(): HTMLElement {
  const host = document.createElement("div");
  document.body.appendChild(host);
  renderClientes(host, URL);
  return host;
}

/** Type into the search box, let the debounce fire, click the first result, and
 *  let the detail + cuenta loads settle. */
async function selectFirstCustomer(host: HTMLElement): Promise<void> {
  const search = host.querySelector<HTMLInputElement>("#cli-search")!;
  search.value = "juan";
  search.dispatchEvent(new Event("input"));
  await wait(300); // debounce (240ms) + the mocked search resolve
  host.querySelector<HTMLButtonElement>(".cli-result")!.click();
  await wait(30); // loadDetail (Promise.all) + loadCuenta settle
}

describe("Clientes — cuenta corriente panel", () => {
  it("renders the saldo (debe) and the movimientos after selecting a customer", async () => {
    const host = mountClientes();
    await selectFirstCustomer(host);

    const cuenta = host.querySelector<HTMLElement>("#cli-cuenta")!;
    expect(mCuenta).toHaveBeenCalledWith(URL, CUST.id);
    expect(cuenta.textContent).toContain("Debe");
    expect(cuenta.querySelector(".pill-danger")).not.toBeNull();
    expect(cuenta.textContent).toContain("Venta boleta 41");
    expect(cuenta.textContent).toContain("Abono efectivo");
    // The actions are present and wired.
    expect(cuenta.querySelector("#cli-abono-btn")).not.toBeNull();
    expect(cuenta.querySelector("#cli-cuenta-print")).not.toBeNull();
  });

  it("DEGRADES GRACEFULLY when the backend lacks the cuenta surface", async () => {
    mCuenta.mockRejectedValueOnce(CUENTA_MODULE_MISSING);
    const host = mountClientes();
    await selectFirstCustomer(host);

    const cuenta = host.querySelector<HTMLElement>("#cli-cuenta")!;
    expect(cuenta.textContent?.toLowerCase()).toContain("próximamente");
    // It degrades, never a hard error state.
    expect(cuenta.querySelector('[role="alert"]')).toBeNull();
  });

  it("the abono modal POSTs the parsed amount + medio and reloads the cuenta", async () => {
    const host = mountClientes();
    await selectFirstCustomer(host);

    expect(mCuenta).toHaveBeenCalledTimes(1);
    host.querySelector<HTMLButtonElement>("#cli-abono-btn")!.click();

    const monto = host.querySelector<HTMLInputElement>("#abono-monto")!;
    expect(monto).not.toBeNull(); // modal opened
    monto.value = "$10.000";
    host.querySelector<HTMLButtonElement>("#abono-save")!.click();
    await flush();

    expect(mAbono).toHaveBeenCalledTimes(1);
    expect(mAbono).toHaveBeenCalledWith(URL, CUST.id, 10000, "efectivo");
    await wait(30);
    // Reloaded after the abono → cuentaCorriente fetched again.
    expect(mCuenta).toHaveBeenCalledTimes(2);
    // Modal closed.
    expect(host.querySelector("#abono-monto")).toBeNull();
  });

  it("a zero/blank abono is rejected client-side (never POSTed)", async () => {
    const host = mountClientes();
    await selectFirstCustomer(host);

    host.querySelector<HTMLButtonElement>("#cli-abono-btn")!.click();
    host.querySelector<HTMLInputElement>("#abono-monto")!.value = "0";
    host.querySelector<HTMLButtonElement>("#abono-save")!.click();
    await flush();

    expect(mAbono).not.toHaveBeenCalled();
    const err = host.querySelector<HTMLElement>("#abono-error")!;
    expect(err.hidden).toBe(false);
    expect(err.textContent?.toLowerCase()).toContain("monto");
  });
});

describe("POS — fiar rail gated to a selected customer", () => {
  function mountPos(): HTMLElement {
    const host = document.createElement("div");
    document.body.appendChild(host);
    renderPos(host, URL);
    return host;
  }

  it("offers the fiado method but DISABLED until a customer is picked", () => {
    const host = mountPos();
    const fiar = host.querySelector<HTMLButtonElement>('[data-method="pos_cuenta"]')!;
    expect(fiar).not.toBeNull();
    expect(fiar.textContent).toContain("Cuenta corriente (fiar)");
    expect(fiar.disabled).toBe(true);
  });

  it("enables the fiado method once a customer is selected", async () => {
    const host = mountPos();
    const fiar = host.querySelector<HTMLButtonElement>('[data-method="pos_cuenta"]')!;
    expect(fiar.disabled).toBe(true);

    const custSearch = host.querySelector<HTMLInputElement>("#pos-cust-search")!;
    custSearch.value = "juan";
    custSearch.dispatchEvent(new Event("input"));
    await wait(300); // debounce + mocked search resolve
    host.querySelector<HTMLButtonElement>(".pos-cust-result")!.click();
    await flush();

    expect(fiar.disabled).toBe(false);
  });
});
