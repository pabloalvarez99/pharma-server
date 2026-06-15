// Golden-path flows, run per vertical. Each mirrors the operator's daily loop
// through the exact endpoints the Tauri views call. Money is a STRING end to end
// (rust_decimal); we never round-trip a float price back to the server.
//
// The multi-rubro contract under test:
//   - login -> open caja -> sell -> receipt -> boleta -> return -> close caja
//     must work in BOTH verticals (pharmacy AND minimarket).
//   - boleta (DTE SII) is UNIVERSAL: every CL business emits. On Free with no
//     CAF/cert it must fail CLEANLY (coded 4xx upsell, never a 5xx/crash).
//   - recetas/controlados are PHARMACY-ONLY: a minimarket sale must NOT require a
//     prescription. We assert a plain minimarket sale closes with no receta step.

import { Client, check, eq, section, knownBug } from "./lib/harness.mjs";

/** Run the full golden path for one tenant/vertical. */
export async function goldenPath({ tenant, email, password, vertical }) {
  section(`vertical=${vertical} tenant=${tenant}`);
  const c = new Client();

  // 1. login --------------------------------------------------------------
  const sess = await c.login(tenant, email, password);
  check(typeof sess.token === "string" && sess.token.length > 20, "login returns JWT");

  // 2. seed demo data (same service the in-app button uses) ----------------
  const seed = await c.post("/admin/seed-demo", { vertical, force: true });
  check(seed.ok, `seed-demo ${vertical} ok`);

  // 3. catalog populated; minimarket must carry NO clinical fields ---------
  const products = (await c.get("/products")).body;
  const list = Array.isArray(products) ? products : (products?.items ?? []);
  check(list.length > 0, `catalog non-empty (${list.length} products)`);
  const sellable = list.find((p) => p.stock > 0 && p.active !== false);
  check(!!sellable, "found a sellable product (stock > 0)");

  if (vertical === "minimarket") {
    const clinical = list.filter((p) => p.active_ingredient);
    eq(clinical.length, 0, "minimarket catalog has NO active_ingredient (no clinical pack)");
  }

  // 4. open caja ----------------------------------------------------------
  const open = await c.post("/cash-sessions", {
    register_name: "Caja E2E",
    opening_cash: "0",
  });
  const sessionId = open.body.id;
  check(!!sessionId, "cash session opened");

  // 5. sell one unit, cash, exact tender ----------------------------------
  const stockBefore = sellable.stock;
  const sale = await c.post(
    "/pos/sale",
    {
      items: [
        {
          product: sellable.id,
          product_name: sellable.name,
          quantity: 1,
          unit_price: String(sellable.price),
        },
      ],
      payment_method: "pos_cash",
      cash_amount: String(sellable.price),
    },
    { headers: { "Idempotency-Key": `e2e-${vertical}-${Date.now()}` } },
  );
  eq(sale.status, 201, "POS sale created (201) — no receta required");
  // PosSaleResponse nests the order (mirrors api.ts: res.order.id -> orderId).
  const orderId = sale.body.order?.id;
  check(!!orderId, "sale returns order id");

  // stock decremented by exactly 1 (FEFO consistency, BUG-003/004 invariant)
  const afterSale = (await c.get(`/products/${encodeURIComponent(sellable.id)}`)).body;
  eq(afterSale.stock, stockBefore - 1, "stock decremented by 1 after sale");

  // 6. printable receipt --------------------------------------------------
  const receipt = (await c.get(`/orders/${orderId}/receipt`)).body;
  check(!!receipt && (receipt.items?.length ?? 0) >= 1, "receipt has at least one line");

  // 7. emit boleta (SII) — UNIVERSAL endpoint, Free = clean gate ----------
  //    No CAF/cert on a fresh install -> expect a coded 4xx (upsell), NEVER a
  //    5xx/connection drop. Success is also fine (if a CAF happened to load).
  const boleta = await c.post(
    "/dte/boletas",
    { order_id: orderId, cert_passphrase: "e2e" },
    { expectOk: false },
  );
  check(
    boleta.status < 500,
    `boleta emit handled cleanly (status ${boleta.status}, no 5xx/crash)`,
  );
  if (!boleta.ok) {
    const code = boleta.body?.error?.code ?? "(none)";
    console.log(`    ↳ Free-tier boleta gate: ${boleta.status} code=${code}`);
  }

  // 8. devolución (return) — restock the unit -----------------------------
  //    Mirrors devoluciones.ts verbatim: it derives tipo "total"|"parcial" from
  //    the quantities and sends that. BUG-bob-001: the devolucion schema
  //    (migrations/0007_sales.surql:66) asserts tipo IN
  //    ['venta','cancelacion','garantia','error'], so EVERY refund from the app
  //    500s (DB_ERROR). xfail until paul/paxoloop reconcile client↔schema; when
  //    fixed, refund.ok flips true and the restock assertion runs for real.
  const refund = await c.post(
    "/pos/returns",
    {
      order: orderId,
      tipo: "total", // what devoluciones.ts deriveTipo() sends
      motivo: "E2E devolución de prueba",
      items: [
        {
          product: sellable.id,
          product_name: sellable.name,
          quantity: 1,
          unit_price: String(sellable.price),
          restock: true,
        },
      ],
      metodo_reembolso: "pos_cash",
    },
    { expectOk: false },
  );
  if (refund.ok) {
    check(true, "devolución created");
    const afterRefund = (await c.get(`/products/${encodeURIComponent(sellable.id)}`)).body;
    eq(afterRefund.stock, stockBefore, "stock restored after restock=true return");
  } else {
    knownBug(
      `BUG-bob-001 devoluciones.ts: tipo "total/parcial" rechazado por schema ` +
        `0007_sales.surql (tipo IN venta/cancelacion/garantia/error) → ${refund.status} ` +
        `${refund.body?.error?.code ?? ""}`,
    );
  }

  // 9. arqueo (close preview) + cierre ------------------------------------
  const arqueo = (await c.get(`/cash-sessions/${sessionId}/arqueo`)).body;
  check(arqueo != null, "arqueo preview returned");
  const close = await c.post(`/cash-sessions/${sessionId}/close`, {
    closing_cash_counted: "0",
  });
  check(close.ok, "cash session closed");
}
