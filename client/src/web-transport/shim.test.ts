// @vitest-environment happy-dom
//
// SP3 shim unit tests: representative command mappings (invoke → fetch) with a
// mocked `fetch`. Covers query/body/header construction, the server error
// envelope → Spanish copy, the coded `"CODE|message"` shape, the settings 404 →
// null soft path, the login token flow, and the desktop-only degradation.
import { describe, it, expect, vi, beforeEach } from "vitest";

import { invoke } from "./index";
import { clearToken, storeToken } from "./session";

const fetchMock = vi.fn();
vi.stubGlobal("fetch", fetchMock);

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

beforeEach(() => {
  fetchMock.mockReset();
  clearToken();
});

describe("login (auth)", () => {
  it("POST /login + GET /me, stores the token and returns SessionInfo", async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ token: "jwt-1", token_type: "Bearer", expires_in: 3600 }))
      .mockResolvedValueOnce(jsonResponse({ sub: "user:1", tenant_id: "tenant:t1", roles: ["owner"], exp: 1 }));

    const session = await invoke("login", {
      serverUrl: "http://127.0.0.1:8090/",
      tenant: "demo",
      email: "a@b.cl",
      password: "x",
    });

    expect(session).toEqual({
      user_id: "user:1",
      tenant_id: "tenant:t1",
      roles: ["owner"],
      expires_in: 3600,
    });
    // Trailing slash trimmed; body snake_case.
    expect(fetchMock.mock.calls[0][0]).toBe("http://127.0.0.1:8090/api/v1/login");
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      tenant: "demo",
      email: "a@b.cl",
      password: "x",
    });
    expect(fetchMock.mock.calls[1][1].headers.Authorization).toBe("Bearer jwt-1");

    // The stored token now backs authenticated commands.
    fetchMock.mockResolvedValueOnce(jsonResponse([]));
    await invoke("list_products", { serverUrl: "http://s", search: "", limit: 5 });
    expect(fetchMock.mock.calls[2][0]).toBe("http://s/api/v1/products?limit=5");
    expect(fetchMock.mock.calls[2][1].headers.Authorization).toBe("Bearer jwt-1");
  });
});

describe("list_products (catalog)", () => {
  it("throws the Spanish no-session copy without a token", async () => {
    await expect(invoke("list_products", { serverUrl: "http://s" })).rejects.toBe(
      "No hay sesión activa. Inicia sesión primero.",
    );
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("maps the server error envelope to its message", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ error: { code: "FORBIDDEN", message: "Permiso denegado personalizado." } }, 403),
    );
    await expect(invoke("list_products", { serverUrl: "http://s" })).rejects.toBe(
      "Permiso denegado personalizado.",
    );
  });

  it("falls back to the status copy on a non-JSON body", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(new Response("boom", { status: 503 }));
    await expect(invoke("list_products", { serverUrl: "http://s" })).rejects.toBe(
      "Servicio no disponible. Intenta nuevamente.",
    );
  });
});

describe("pos_sale (coded errors + idempotency)", () => {
  it("rejects an empty cart with the coded empty-cart copy", async () => {
    await expect(invoke("pos_sale", { serverUrl: "http://s", items: [] })).rejects.toBe(
      "|El carrito está vacío.",
    );
  });

  it("sends snake_case tender fields and a fresh Idempotency-Key", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(jsonResponse({ order: { id: "order:1" } }));
    await invoke("pos_sale", {
      serverUrl: "http://s",
      items: [{ product: "product:1", product_name: "P", quantity: 1, unit_price: "1000" }],
      paymentMethod: "pos_mixed",
      cashAmount: "500",
      cardAmount: "500",
      customer: "",
      discount: undefined,
    });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://s/api/v1/pos/sale");
    expect(init.headers["Idempotency-Key"]).toMatch(/^[0-9a-f-]{36}$/);
    expect(JSON.parse(init.body)).toEqual({
      items: [{ product: "product:1", product_name: "P", quantity: 1, unit_price: "1000" }],
      payment_method: "pos_mixed",
      cash_amount: "500",
      card_amount: "500",
    });
  });

  it("rejects coded 'CODE|message' on a server envelope", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ error: { code: "INSUFFICIENT_STOCK", message: "Stock insuficiente." } }, 422),
    );
    await expect(
      invoke("pos_sale", {
        serverUrl: "http://s",
        items: [{ product: "p", product_name: "P", quantity: 1, unit_price: "1" }],
        paymentMethod: "pos_cash",
      }),
    ).rejects.toBe("INSUFFICIENT_STOCK|Stock insuficiente.");
  });
});

describe("get_setting (settings)", () => {
  it("maps 404 to null (unset key)", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(new Response("", { status: 404 }));
    await expect(invoke("get_setting", { serverUrl: "http://s", key: "business.vertical" })).resolves.toBeNull();
  });
});

describe("customer_search (customers)", () => {
  it("maps 404 to the CUSTOMERS_MODULE_MISSING sentinel", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(new Response("", { status: 404 }));
    await expect(invoke("customer_search", { serverUrl: "http://s", q: "ana" })).rejects.toBe(
      "CUSTOMERS_MODULE_MISSING",
    );
    expect(fetchMock.mock.calls[0][0]).toBe("http://s/api/v1/customers/search?q=ana");
  });
});

describe("fiado / cuenta corriente (credit)", () => {
  it("customer_account pega a /cuenta con Bearer", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        customer: "customer:1",
        balance: "3000",
        total_charged: "5000",
        total_paid: "2000",
        entries: [],
      }),
    );
    const acct = await invoke<{ balance: string }>("customer_account", {
      serverUrl: "http://s",
      id: "customer:1",
    });
    expect(acct.balance).toBe("3000");
    expect(fetchMock.mock.calls[0][0]).toBe("http://s/api/v1/customers/customer:1/cuenta");
    expect(fetchMock.mock.calls[0][1].headers.Authorization).toBe("Bearer jwt-x");
  });

  it("record_abono manda amount + snake_case cash_session y omite vacíos", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(jsonResponse({ id: "l:1", kind: "abono", amount: "2000" }));
    await invoke("record_abono", {
      serverUrl: "http://s",
      id: "customer:1",
      amount: "2000",
      cashSession: "cash_register_session:9",
      note: "",
    });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://s/api/v1/customers/customer:1/abono");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual({
      amount: "2000",
      cash_session: "cash_register_session:9",
    });
  });

  it("propaga el mensaje del servidor cuando el abono supera la deuda", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(
      jsonResponse(
        { error: { code: "INVALID_INPUT", message: "el abono (1500) supera la deuda pendiente (1000)" } },
        400,
      ),
    );
    await expect(
      invoke("record_abono", { serverUrl: "http://s", id: "customer:1", amount: "1500" }),
    ).rejects.toBe("el abono (1500) supera la deuda pendiente (1000)");
  });
});

describe("libro de compras / IVA (compliance)", () => {
  it("libro_compras manda el período como query y omite vacío", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ period: "2026-07", rows: [], total_neto: "0", total_iva: "0", total: "0", pending_declaration: 0 }),
    );
    await invoke("libro_compras", { serverUrl: "http://s", period: "2026-07" });
    expect(fetchMock.mock.calls[0][0]).toBe("http://s/api/v1/reports/libro-compras?period=2026-07");

    fetchMock.mockResolvedValueOnce(
      jsonResponse({ period: "2026-07", rows: [], total_neto: "0", total_iva: "0", total: "0", pending_declaration: 0 }),
    );
    await invoke("libro_compras", { serverUrl: "http://s", period: "" });
    expect(fetchMock.mock.calls[1][0]).toBe("http://s/api/v1/reports/libro-compras");
  });

  it("set_po_invoice hace PATCH con los campos declarados", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ period: "2026-07", rows: [], total_neto: "0", total_iva: "0", total: "0", pending_declaration: 0 }),
    );
    await invoke("set_po_invoice", {
      serverUrl: "http://s",
      id: "purchase_order:1",
      folio: "A-9912",
      tipo: 33,
      neto: "10000",
      iva: "1900",
      total: "11900",
      date: "",
    });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://s/api/v1/purchase-orders/purchase_order:1/factura");
    expect(init.method).toBe("PATCH");
    expect(JSON.parse(init.body)).toEqual({
      folio: "A-9912",
      tipo: 33,
      neto: "10000",
      iva: "1900",
      total: "11900",
    });
  });

  it("iva_summary propaga el error del servidor", async () => {
    storeToken("jwt-x");
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ error: { code: "INVALID_INPUT", message: "período inválido: 2026-99 (usa YYYY-MM)" } }, 400),
    );
    await expect(invoke("iva_summary", { serverUrl: "http://s", period: "2026-99" })).rejects.toBe(
      "período inválido: 2026-99 (usa YYYY-MM)",
    );
  });
});

describe("desktop-only degradation", () => {
  it("print_ticket and unknown commands reject with the controlled copy", async () => {
    await expect(invoke("print_ticket", { printer: "X" })).rejects.toBe(
      "Disponible en la app de escritorio",
    );
    await expect(invoke("plugin:updater|check", {})).rejects.toBe(
      "Disponible en la app de escritorio",
    );
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
