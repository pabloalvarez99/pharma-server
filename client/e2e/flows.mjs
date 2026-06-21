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

/** First integer captured by `<tag>NNN</tag>` in an XML string, or null. */
function xmlInt(xml, tag) {
  const m = xml.match(new RegExp(`<${tag}>\\s*(-?\\d+)\\s*</${tag}>`));
  return m ? Number(m[1]) : null;
}

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
  //    BUG-bob-001 RESOLVED: devoluciones.ts now sends `tipo: DEFAULT_RETURN_MOTIVO`
  //    ("venta"), the MOTIVO axis the schema (migrations/0007_sales.surql) asserts
  //    (venta|cancelacion|garantia|error) — the total/parcial scope it used to send
  //    (which 500'd every refund) is now a presentational badge only. This flow
  //    mirrors the wire payload verbatim and asserts the refund succeeds + restocks.
  const refund = await c.post("/pos/returns", {
    order: orderId,
    tipo: "venta", // motivo axis — what createRefund sends (DEFAULT_RETURN_MOTIVO)
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
  });
  check(refund.ok, "devolución created");
  const afterRefund = (await c.get(`/products/${encodeURIComponent(sellable.id)}`)).body;
  eq(afterRefund.stock, stockBefore, "stock restored after restock=true return");

  // 9. arqueo (close preview) + cierre ------------------------------------
  const arqueo = (await c.get(`/cash-sessions/${sessionId}/arqueo`)).body;
  check(arqueo != null, "arqueo preview returned");
  const close = await c.post(`/cash-sessions/${sessionId}/close`, {
    closing_cash_counted: "0",
  });
  check(close.ok, "cash session closed");

  // 10. reporte — close the full day: the sale just made must surface in the
  //     core (Free) sales-daily report. Closes the operator's daily loop
  //     login → … → cierre → reporte, both verticals. The report is the
  //     after-action view the operator opens to confirm the day's takings.
  const salesDaily = await c.get("/reports/sales-daily");
  check(
    Array.isArray(salesDaily.body) && salesDaily.body.length > 0,
    "sales-daily report shows the day's sale",
  );
}

/** Multi-tender (split-payment) flow — the cashier splits one sale across cash
 *  AND card, over-tendering the cash side, then prints the receipt and emits a
 *  boleta. Pins the regression that F-paul-pay-001 closed: the vuelto on a MIXED
 *  sale is `(cash + card) − total`, the overpayment falling on the cash side (a
 *  card is never over-charged) — the cashier must see the vuelto on a mixed sale,
 *  not just on pos_cash. Universal across verticals (split pago + boleta both
 *  apply to pharmacy AND minimarket). Assumes goldenPath already seeded the
 *  tenant; runs against a fresh caja it opens + closes itself. */
export async function multiTenderFlow({ tenant, email, password, vertical }) {
  section(`multi-tender (split pago) vertical=${vertical} tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  const products = (await c.get("/products")).body;
  const list = Array.isArray(products) ? products : (products?.items ?? []);
  const sellable = list.find((p) => p.stock > 0 && p.active !== false);
  check(!!sellable, "multi-tender: found a sellable product");

  // Fresh caja so the mixed sale lands in an open session (goldenPath closed its
  // own). Closed at the end so the run leaves nothing open behind it.
  const open = await c.post("/cash-sessions", {
    register_name: "Caja Mixto E2E",
    opening_cash: "0",
  });
  const sessionId = open.body.id;
  check(!!sessionId, "multi-tender: cash session opened");

  // total = unit price (qty 1, no discount). Split it: the card settles EXACTLY
  // for part of it (cards are never over-charged), cash covers the remainder PLUS
  // an overpayment, so the cashier owes vuelto on the cash side.
  const total = Math.round(Number(sellable.price));
  const card = Math.floor(total / 2); // exact card portion
  const OVERPAY = 500; // cash over-tender → expected vuelto
  const cash = total - card + OVERPAY; // remainder + overpay

  const sale = await c.post(
    "/pos/sale",
    {
      items: [
        {
          product: sellable.id,
          product_name: sellable.name,
          quantity: 1,
          unit_price: String(total),
        },
      ],
      payment_method: "pos_mixed",
      cash_amount: String(cash),
      card_amount: String(card),
    },
    { headers: { "Idempotency-Key": `e2e-mixed-${vertical}-${Date.now()}` } },
  );
  eq(sale.status, 201, "multi-tender: pos_mixed sale created (201)");
  const orderId = sale.body.order?.id;
  check(!!orderId, "multi-tender: sale returns order id");

  // boleta (SII) is UNIVERSAL — Free no-CAF/cert → coded 4xx upsell, never 5xx.
  const boleta = await c.post(
    "/dte/boletas",
    { order_id: orderId, cert_passphrase: "e2e" },
    { expectOk: false },
  );
  check(
    boleta.status < 500,
    `multi-tender: boleta handled cleanly (status ${boleta.status}, no 5xx)`,
  );

  // Receipt: the mixed sale records both tenders and the correct vuelto.
  const receipt = (await c.get(`/orders/${encodeURIComponent(orderId)}/receipt`)).body;
  eq(receipt.payment_method, "pos_mixed", "receipt records pos_mixed");
  eq(Number(receipt.total), total, "receipt total == unit price (qty 1, no discount)");
  eq(Number(receipt.cash_amount), cash, "receipt cash_amount == cash tendered");
  eq(Number(receipt.card_amount), card, "receipt card_amount == card tendered");
  // F-paul-pay-001: vuelto on a mixed sale = (cash + card) − total = the overpay.
  eq(
    Number(receipt.change),
    OVERPAY,
    "receipt vuelto = (cash + card) − total (overpay on the cash side)",
  );

  // Close the caja so the run leaves no open session.
  const close = await c.post(`/cash-sessions/${sessionId}/close`, {
    closing_cash_counted: "0",
  });
  check(close.ok, "multi-tender: cash session closed");
}

/** Goods-receipt path (charter: producto+lote → recepción → stock). Mirrors
 *  compras.ts verbatim: create a supplier, create a draft PO, then try to receive
 *  goods against it. A receipt must bump on-hand stock by the quantity received.
 *
 *  Assumes goldenPath already seeded this tenant (run order in run.mjs). */
export async function goodsReceiptFlow({ tenant, email, password, vertical }) {
  section(`goods-receipt vertical=${vertical} tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // Target a real catalogued product so the receipt recomputes WAC + stock.
  const products = (await c.get("/products")).body;
  const list = Array.isArray(products) ? products : (products?.items ?? []);
  const target = list[0];
  check(!!target, "have a catalogued product to receive against");
  const stockBefore = target.stock;

  // 1. supplier (compras.ts createSupplier — only `name` is required) ---------
  const supplier = await c.post("/suppliers", { name: "Proveedor E2E" });
  const supplierId = supplier.body?.id;
  check(!!supplierId, "supplier created");

  // 2. draft PO with one catalogued line (compras.ts createPurchaseOrder) -----
  const QTY = 5;
  const po = await c.post("/purchase-orders", {
    supplier: supplierId,
    items: [
      {
        product: target.id,
        product_name: target.name,
        quantity: QTY,
        unit_cost: "1000",
      },
    ],
  });
  const poId = po.body?.id;
  check(!!poId, "purchase order created (draft)");

  // 3. read back the PO line id the receipt addresses (compras.ts getPO) ------
  const detail = (await c.get(`/purchase-orders/${encodeURIComponent(poId)}`)).body;
  const poLineId = detail?.items?.[0]?.id;
  check(!!poLineId, "PO detail returns a line id");

  // 4. probe for a draft→sent/approved transition (the MISSING link) ----------
  //    BUG-bob-002: create makes a `draft`, but the wired POST /receive
  //    (service::receive_purchase_order_lines) only accepts
  //    sent/approved/partially_received, and NO route issues a draft→sent
  //    transition (purchasing.rs exposes only create/receive/payments/cancel —
  //    no /send, /approve, /submit; create can't set status). So the receipt
  //    409s and on-hand stock NEVER moves — the whole goods-receipt pillar is
  //    unreachable through the app.
  //
  //    This probe is SELF-HEALING: it tries each plausible transition verb. If
  //    one lands on origin, it advances the PO and the real receive+stock
  //    assertions below run for keeps; until then it pins the exact gap so the
  //    xfail flips red the moment a transition ships.
  const TRANSITION_VERBS = ["send", "approve", "submit"];
  let advanced = false;
  for (const verb of TRANSITION_VERBS) {
    const r = await c.post(
      `/purchase-orders/${encodeURIComponent(poId)}/${verb}`,
      {},
      { expectOk: false },
    );
    if (r.ok) {
      advanced = true;
      check(true, `draft→sent transition landed via /${verb} (BUG-bob-002 fixed)`);
      break;
    }
  }

  // 5. receive the goods — the operator's "Recibir" button --------------------
  const recv = await c.post(
    `/purchase-orders/${encodeURIComponent(poId)}/receive`,
    // Lot-tracked products (the seed creates batches) require a lot + expiry on
    // receipt: a lotless line on a batched product is a clean 409 (BUG-marvin-004
    // guard, #231) so product.stock can't desync from Σ batch.stock. Send one so
    // the real lot path runs end-to-end.
    {
      lines: [
        {
          po_line_id: poLineId,
          qty_received: QTY,
          lot: "E2E-LOT-1",
          expiry_date: "2027-12-31T00:00:00Z",
        },
      ],
    },
    { expectOk: false },
  );
  if (recv.ok) {
    check(true, "purchase order received");
    const after = (await c.get(`/products/${encodeURIComponent(target.id)}`)).body;
    eq(after.stock, stockBefore + QTY, "stock increased by received quantity");
  } else if (advanced) {
    // A transition landed but receive still failed → that's a NEW, real bug.
    check(false, `receive failed after a successful transition → ${recv.status} ${recv.body?.error?.code ?? ""}`);
  } else {
    // receive must 409 a draft cleanly (coded conflict, never a 5xx/crash).
    check(
      recv.status === 409,
      `draft receive rejected with a clean 409 (got ${recv.status}, no 5xx)`,
    );
    knownBug(
      `BUG-bob-002 compras: draft PO no se puede recibir — no existe transición ` +
        `draft→sent (probé /send /approve /submit, ninguna existe) y POST /receive ` +
        `exige sent/approved/partially_received → ${recv.status} ${recv.body?.error?.code ?? ""}`,
    );
  }
}

/** Compliance + reporting path (charter: boleta/factura/DTE = UNIVERSAL, must
 *  work in minimarket too; reports must not crash). Free tier: paid surfaces
 *  (margins-daily Pro-gate, factura/libro without CAF/cert) must fail CLEANLY
 *  with a coded 4xx upsell — NEVER a 5xx/crash. Runs after goldenPath so a sale
 *  already exists for sales-daily/top-products to report on. */
export async function complianceFlow({ tenant, email, password, vertical }) {
  section(`compliance vertical=${vertical} tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // Core/free reports — must return a JSON array, both verticals -------------
  for (const path of [
    "/reports/sales-daily",
    "/reports/top-products",
    "/reports/stock-rotation",
    "/reports/near-expiry",
  ]) {
    const r = await c.get(path, { expectOk: false });
    check(r.status === 200 && Array.isArray(r.body), `${path} → 200 array`);
  }

  // margins-daily is Pro-gated — Free = clean 402 upsell, never a 5xx --------
  const margins = await c.get("/reports/margins-daily", { expectOk: false });
  check(margins.status < 500, `margins-daily handled cleanly (status ${margins.status})`);
  if (!margins.ok) {
    console.log(`    ↳ Pro-gate: ${margins.status} code=${margins.body?.error?.code ?? "(none)"}`);
  }

  // factura (DTE 33) is UNIVERSAL — every CL business emits, incl. minimarket.
  // Free with no CAF/cert/emisor → coded 4xx, NEVER 5xx/crash (boleta-gate
  // contract for the full document path).
  const factura = await c.post(
    "/dte/documentos",
    {
      tipo: 33,
      cert_passphrase: "e2e",
      receptor: {
        rut: "76086428-5",
        razon_social: "Cliente E2E SpA",
        giro: "Comercio",
        direccion: "Av Siempre Viva 742",
        comuna: "Coquimbo",
      },
      items: [{ nombre: "Item E2E", cantidad: "1", precio_unitario: "1190" }],
    },
    { expectOk: false },
  );
  check(factura.status < 500, `factura emit handled cleanly (status ${factura.status}, no 5xx)`);
  if (!factura.ok) {
    console.log(`    ↳ Free factura gate: ${factura.status} code=${factura.body?.error?.code ?? "(none)"}`);
  }

  // libro de ventas (monthly sales book) — admin read, clean handling --------
  const period = new Date().toISOString().slice(0, 7); // YYYY-MM
  const libro = await c.get(`/dte/libro-ventas?period=${period}`, { expectOk: false });
  check(libro.status < 500, `libro-ventas handled cleanly (status ${libro.status}, no 5xx)`);
}

// Emisor + cert RUT the DTE-lifecycle tenant is bootstrapped with (run.mjs).
// Must match the CAF <RE> and the test cert's --rut so the TED + CAF line up.
export const DTE_EMISOR_RUT = "76123456-7";
export const DTE_CERT_PASS = "test1234"; // crates/dte/tests/assets/test-cert.pfx

/** Full DTE document lifecycle over the live stack (charter ola-6 spina vendible).
 *  Runs against a tenant that run.mjs pre-loaded (server down) with a digital cert
 *  and CAFs for boleta(39, folios 1..2) + factura(33, 1..1) + nota-crédito(61,
 *  1..10), and NO CAF for guía(52). Drives REAL signed emission and asserts:
 *    1. sin CAF       → emitting a tipo with no CAF fails CLEANLY (coded 409
 *                       FOLIO_EXHAUSTED, never a 5xx). [charter said "422"; the
 *                       real coded status is 409 — see MULTI-RUBRO/BUG notes.]
 *    2. folio burn    → each emit consumes one folio; caf-status next_folio
 *                       advances and `restantes` drops.
 *    3. reference chain→ a nota-crédito referencing the factura persists
 *                       <TpoDocRef>/<FolioRef> pointing at that exact folio.
 *    4. monto coherence→ factura desglose nets out: MntNeto + IVA == MntTotal.
 *    5. CAF agotado   → a second factura on a 1-folio CAF → clean 409, restantes 0.
 *    6. aggregate     → GET /dte lists every emitted doc with matching folios +
 *                       montos (the day's book cuadra); libro-ventas XML renders
 *                       cleanly (empty offline — only SII-'accepted' docs land in
 *                       the libro, by design; see note below). */
export async function dteLifecycleFlow({ tenant, email, password }) {
  section(`dte-lifecycle tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // Catalog + a paid order to hang the boleta on (boleta = order-derived).
  await c.post("/admin/seed-demo", { vertical: "pharmacy", force: true });
  await c.put("/settings/dte.emisor", {
    value: JSON.stringify({
      rut: DTE_EMISOR_RUT,
      razon_social: "FARMACIA TEST SPA",
      giro: "FARMACIA",
      direccion: "CALLE 123",
      comuna: "COQUIMBO",
      ciudad: "COQUIMBO",
      acteco: 477310,
    }),
  });

  const products = (await c.get("/products")).body;
  const list = Array.isArray(products) ? products : (products?.items ?? []);
  const sellable = list.find((p) => p.stock > 0 && p.active !== false);
  check(!!sellable, "dte: have a sellable product to bill");
  await c.post("/cash-sessions", { register_name: "Caja DTE", opening_cash: "0" });
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
    { headers: { "Idempotency-Key": `e2e-dte-${Date.now()}` } },
  );
  eq(sale.status, 201, "dte: paid order created for boleta");
  const orderId = sale.body.order?.id;
  check(!!orderId, "dte: sale returns order id");

  const receptor = {
    rut: "76086428-5",
    razon_social: "Cliente DTE SpA",
    giro: "Comercio",
    direccion: "Av Siempre Viva 742",
    comuna: "Coquimbo",
  };
  const today = new Date().toISOString().slice(0, 10);

  // --- 1. sin CAF: guía(52) has no CAF → clean 409 FOLIO_EXHAUSTED -----------
  //     The spec validates first (valid receptor + ind_traslado), so reaching
  //     the folio step proves it gated on folio supply, not on a bad request.
  const guia = await c.post(
    "/dte/documentos",
    {
      tipo: 52,
      cert_passphrase: DTE_CERT_PASS,
      receptor,
      ind_traslado: 1,
      items: [{ nombre: "Traslado E2E", cantidad: "1", precio_unitario: "1000" }],
    },
    { expectOk: false },
  );
  check(guia.status < 500, `sin-CAF guía handled cleanly (status ${guia.status}, no 5xx)`);
  eq(guia.status, 409, "sin-CAF guía → 409 (coded folio gate, charter approx 422)");
  eq(guia.body?.error?.code, "FOLIO_EXHAUSTED", "sin-CAF guía code FOLIO_EXHAUSTED");

  // --- 2. boleta(39): emit consumes a folio; next_folio advances -------------
  const caf39Before = (await c.get("/dte/caf-status?tipo=39")).body;
  eq(caf39Before.cafs?.[0]?.next_folio, 1, "boleta CAF starts at folio 1");
  const boleta = await c.post(
    "/dte/boletas",
    { order_id: orderId, cert_passphrase: DTE_CERT_PASS },
    { expectOk: false },
  );
  eq(boleta.status, 201, "boleta 39 emitted (signed) with a real CAF/cert");
  eq(boleta.body?.folio, 1, "boleta took folio 1");
  const caf39After = (await c.get("/dte/caf-status?tipo=39")).body;
  eq(caf39After.cafs?.[0]?.next_folio, 2, "boleta CAF advanced to folio 2");
  eq(caf39After.cafs?.[0]?.restantes, 1, "boleta CAF has 1 folio left");

  // --- 3+4. factura(33): emit, monto desglose coherent -----------------------
  const factura = await c.post(
    "/dte/documentos",
    {
      tipo: 33,
      cert_passphrase: DTE_CERT_PASS,
      receptor,
      // precio IVA-incluido (retail CL): 1190 → neto 1000 + IVA 190.
      items: [{ nombre: "Item Factura E2E", cantidad: "1", precio_unitario: "1190" }],
    },
    { expectOk: false },
  );
  eq(factura.status, 201, "factura 33 emitted (signed)");
  const facturaFolio = factura.body?.folio;
  check(typeof facturaFolio === "number", "factura returned a folio");
  const facturaTotal = Number(factura.body?.monto_total);
  const facturaXml = (await c.get(`/dte/${encodeURIComponent(factura.body.id)}/xml`)).body;
  const neto = xmlInt(facturaXml, "MntNeto");
  const iva = xmlInt(facturaXml, "IVA");
  const total = xmlInt(facturaXml, "MntTotal");
  eq(neto, 1000, "factura MntNeto = 1000 (1190 / 1.19)");
  eq(iva, 190, "factura IVA = 190");
  eq(neto + iva, total, "factura desglose cuadra: MntNeto + IVA == MntTotal");
  eq(total, facturaTotal, "factura XML MntTotal == DTO monto_total");

  // --- 3. nota-crédito(61) referencing the factura ---------------------------
  const nc = await c.post(
    "/dte/documentos",
    {
      tipo: 61,
      cert_passphrase: DTE_CERT_PASS,
      receptor,
      referencias: [
        {
          tipo_doc_ref: "33",
          folio_ref: String(facturaFolio),
          fecha_ref: today,
          cod_ref: 1, // anula
          razon_ref: `Anula factura ${facturaFolio}`,
        },
      ],
      items: [{ nombre: "Anulación factura E2E", cantidad: "1", precio_unitario: "1190" }],
    },
    { expectOk: false },
  );
  eq(nc.status, 201, "nota-crédito 61 emitted referencing the factura");
  const ncXml = (await c.get(`/dte/${encodeURIComponent(nc.body.id)}/xml`)).body;
  eq(xmlInt(ncXml, "TpoDocRef"), 33, "NC reference TpoDocRef points at factura (33)");
  eq(xmlInt(ncXml, "FolioRef"), facturaFolio, "NC reference FolioRef = factura folio");

  // --- 3b. nota-débito(56) referencing the factura — completes the NC/ND chain.
  //     56 = nota de débito per SII (crates/dte/src/lib.rs); it corrects amounts
  //     upward (cod_ref 3) and, like the NC, MUST carry a <Referencia> to the doc
  //     it adjusts (emit.rs gates 56/61 on a reference). Same reference contract:
  //     the persisted XML points <TpoDocRef>/<FolioRef> at the factura's folio.
  const nd = await c.post(
    "/dte/documentos",
    {
      tipo: 56,
      cert_passphrase: DTE_CERT_PASS,
      receptor,
      referencias: [
        {
          tipo_doc_ref: "33",
          folio_ref: String(facturaFolio),
          fecha_ref: today,
          cod_ref: 3, // corrige montos (recargo)
          razon_ref: `Recargo factura ${facturaFolio}`,
        },
      ],
      items: [{ nombre: "Recargo factura E2E", cantidad: "1", precio_unitario: "1190" }],
    },
    { expectOk: false },
  );
  eq(nd.status, 201, "nota-débito 56 emitted referencing the factura");
  const ndXml = (await c.get(`/dte/${encodeURIComponent(nd.body.id)}/xml`)).body;
  eq(xmlInt(ndXml, "TpoDocRef"), 33, "ND reference TpoDocRef points at factura (33)");
  eq(xmlInt(ndXml, "FolioRef"), facturaFolio, "ND reference FolioRef = factura folio");

  // --- 5. CAF agotado: the 1-folio factura CAF is now spent → clean 409 ------
  const factura2 = await c.post(
    "/dte/documentos",
    {
      tipo: 33,
      cert_passphrase: DTE_CERT_PASS,
      receptor,
      items: [{ nombre: "Segunda factura E2E", cantidad: "1", precio_unitario: "1190" }],
    },
    { expectOk: false },
  );
  check(factura2.status < 500, `CAF-agotado factura handled cleanly (status ${factura2.status}, no 5xx)`);
  eq(factura2.status, 409, "CAF agotado → 409");
  eq(factura2.body?.error?.code, "FOLIO_EXHAUSTED", "CAF agotado code FOLIO_EXHAUSTED");
  const caf33 = (await c.get("/dte/caf-status?tipo=33")).body;
  eq(caf33.folios_restantes, 0, "factura CAF exhausted (0 folios left)");

  // --- 6. aggregate: every emitted doc is queryable + montos cuadran --------
  const all = (await c.get("/dte")).body;
  const rows = Array.isArray(all) ? all : (all?.items ?? []);
  const byTipo = (t) => rows.filter((d) => d.tipo === t);
  eq(byTipo(39).length, 1, "ledger has exactly the 1 boleta emitted");
  eq(byTipo(33).length, 1, "ledger has exactly the 1 factura emitted");
  eq(byTipo(61).length, 1, "ledger has exactly the 1 nota-crédito emitted");
  eq(byTipo(56).length, 1, "ledger has exactly the 1 nota-débito emitted");
  const billed = byTipo(39)[0].folio === 1 && byTipo(33)[0].folio === facturaFolio;
  check(billed, "ledger folios match what we emitted (day's book cuadra)");

  // libro-ventas XML: renders cleanly and carries the right shape/period. It
  // aggregates only SII-'accepted' docs; offline (no SII send/poll) everything
  // stays 'signed', so the BODY is empty by design — we assert the envelope is
  // well-formed and stamped for the period we asked, not that it lists drafts.
  const period = today.slice(0, 7); // YYYY-MM
  const libro = await c.get(`/dte/libro-ventas?period=${period}`, { expectOk: false });
  check(libro.status === 200, `libro-ventas renders (status ${libro.status})`);
  const libroXml = typeof libro.body === "string" ? libro.body : "";
  check(libroXml.includes("LibroCompraVenta"), "libro-ventas is well-formed XML");
  check(libroXml.includes("<Caratula"), "libro-ventas carries a <Caratula>");
  check(
    libroXml.includes(`<PeriodoTributario>${period}</PeriodoTributario>`),
    `libro-ventas PeriodoTributario == ${period}`,
  );
  check(
    libroXml.includes(`<RutEmisorLibro>${DTE_EMISOR_RUT}</RutEmisorLibro>`),
    "libro-ventas RutEmisorLibro == emisor RUT",
  );
}

/** 402 matrix: every Pro-gated report returns a clean 402 FEATURE_REQUIRES_UPGRADE
 *  on Free (never a 5xx, never silent 200), while the core/free reports return a
 *  200 JSON array. Pins the freemium gate contract (ADR-0005: core gratis, paid
 *  surfaces upsell). Runs on any seeded tenant — no DTE setup needed. */
export async function reports402Matrix({ tenant, email, password }) {
  section(`reports-402-matrix tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // Core (Free) reports — 200 + JSON array, both verticals. This is exactly the
  // feed set the Reportes "insight strip" reads (sales delta, money-at-risk by
  // expiry, top seller, stalled stock) — `/products` is the price source the
  // money-at-risk/stalled math joins to value the units. A green row here means
  // the actionable-insight view never hits a missing/5xx endpoint on Free.
  for (const path of [
    "/reports/sales-daily",
    "/reports/top-products",
    "/reports/stock-rotation",
    "/reports/near-expiry",
    "/products",
  ]) {
    const r = await c.get(path, { expectOk: false });
    check(r.status === 200 && Array.isArray(r.body), `core ${path} → 200 array`);
  }

  // Pro-gated reports — clean 402 FEATURE_REQUIRES_UPGRADE on Free.
  for (const path of ["/reports/margins-daily"]) {
    const r = await c.get(path, { expectOk: false });
    eq(r.status, 402, `gated ${path} → 402`);
    eq(r.body?.error?.code, "FEATURE_REQUIRES_UPGRADE", `gated ${path} code FEATURE_REQUIRES_UPGRADE`);
  }

  // Dashboard stays 200 on Free but degrades the gated margin field to null
  // (never a 402 — the executive view must still load). ADR-0005 invariant.
  const dash = await c.get("/reports/dashboard", { expectOk: false });
  check(dash.status === 200, `dashboard → 200 on Free (status ${dash.status})`);
  check(
    dash.body != null && (dash.body.margen_hoy === null || dash.body.margen_hoy === undefined),
    "dashboard margen_hoy degraded to null on Free (no 402)",
  );
}

/** Multi-rubro contract for a NON-pharmacy rubro (minimarket): the operator must
 *  never be forced through prescription / controlled-substance (Ley 20.000)
 *  machinery, yet boleta (DTE SII) stays UNIVERSAL — every CL business emits.
 *  Caller runs this for minimarket ONLY (see run.mjs); it is a no-op contract
 *  for pharmacy, where recetas/controlados legitimately apply. */
export async function noPrescriptionFlow({ tenant, email, password, vertical }) {
  section(`no-receta (minimarket) vertical=${vertical} tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // 1. catalog carries NO clinical/controlled product — no pharmacy pack leaked
  const products = (await c.get("/products")).body;
  const list = Array.isArray(products) ? products : (products?.items ?? []);
  check(list.length > 0, `minimarket catalog non-empty (${list.length} products)`);
  const clinical = list.filter((p) => p.active_ingredient);
  eq(clinical.length, 0, "no product carries active_ingredient (no clinical pack)");
  // prescription_type defaults to 'direct' (OTC / venta directa, migration
  // 0003_catalog.surql). A minimarket catalog must be all OTC — nothing that
  // demands a receta (anything other than 'direct' would force one).
  const needsReceta = list.filter(
    (p) => p.prescription_type && p.prescription_type !== "direct",
  );
  eq(needsReceta.length, 0, "every product is OTC (prescription_type 'direct', no receta required)");

  // 2. a plain sale closes with NO receta field — the POS must not demand one.
  const sellable = list.find((p) => p.stock > 0 && p.active !== false);
  check(!!sellable, "found a sellable minimarket product");
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
    { headers: { "Idempotency-Key": `e2e-noreceta-${Date.now()}` } },
  );
  eq(sale.status, 201, "minimarket sale created (201) with NO prescription attached");
  const orderId = sale.body.order?.id;
  check(!!orderId, "sale returns order id");

  // 3. the receta libro endpoint is UNIVERSAL but stays empty in a minimarket —
  //    reachable (no crash) and carries no dispensed controlados.
  const recetas = await c.get("/prescriptions", { expectOk: false });
  check(recetas.status < 500, `prescriptions list handled cleanly (status ${recetas.status})`);
  if (recetas.status === 200 && Array.isArray(recetas.body)) {
    eq(recetas.body.length, 0, "no recetas/controlados dispensed in minimarket");
  }

  // 4. boleta (DTE SII) is UNIVERSAL — a minimarket emits too. Free with no
  //    CAF/cert → coded 4xx upsell, NEVER a 5xx/crash (boleta-gate contract).
  const boleta = await c.post(
    "/dte/boletas",
    { order_id: orderId, cert_passphrase: "e2e" },
    { expectOk: false },
  );
  check(
    boleta.status < 500,
    `minimarket boleta still emits/gates cleanly (status ${boleta.status}, no 5xx)`,
  );
}

// --- rubro showcase (the "elige tu rubro" configurator, live) ----------------

/** The catalog the onboarding rubro grid renders (mirrors client/src/vertical.ts
 *  RUBRO_CATALOG). `seed` = a demo pack exists; `service` = a service rubro that
 *  sells without physical stock (its UI hides inventario/compras). Hardcoded here
 *  because the harness is plain node (.mjs) and can't import the TS source — the
 *  rubro-gating-matrix vitest already pins the TS table; this asserts the SERVER
 *  honours the value the configurator persists. */
const RUBRO_CATALOG_E2E = [
  { value: "farmacia", seed: true, service: false },
  { value: "minimarket", seed: true, service: false },
  { value: "restaurant", seed: false, service: false },
  { value: "cafe", seed: false, service: false },
  { value: "tienda", seed: false, service: false },
  { value: "belleza", seed: false, service: true },
  { value: "servicios", seed: false, service: true },
  { value: "otro", seed: false, service: false },
];

/** Rubro-select showcase end-to-end (the founder's vitrina RutBusiness, ULTRA-PLAN
 *  docs/strategy/rubro-select-experience.md; P1 = the 2-panel configurator + live
 *  preview). The preview is pure client logic (covered by first-run.test.ts +
 *  rubro-gating-matrix); what only a LIVE run can prove is that the value the
 *  configurator SAVES round-trips through the real settings API and that the ERP
 *  the preview promises actually works over HTTP for ANY rubro — including a
 *  service rubro the onboarding flow can land on. Two contracts:
 *
 *   (A) configurator persistence — every one of the 8 catalog rubros persists
 *       to `business.vertical` and reads back EXACTLY (raw catalog key, not folded
 *       to "otro"): the gate (loadRubro/featuresForRubro) keys off the stored
 *       value, so an extra rubro silently coerced server-side would mis-gate the
 *       whole ERP. Plus `business.name` round-trips (the branded wordmark).
 *
 *   (B) agnostic core under a SERVICE rubro — pick belleza (a service rubro: the
 *       preview shows NO inventario/compras), then drive the operator's whole day
 *       login → caja → sale → receipt → boleta gate → cierre → reporte against a
 *       manually-created service line. Proves choosing a non-pharmacy, no-inventory
 *       rubro never breaks the daily loop, boleta stays UNIVERSAL, recetas are
 *       never forced, and reports gate cleanly. Runs on its own tenant. */
export async function rubroShowcaseFlow({ tenant, email, password }) {
  section(`rubro-showcase tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // (A) Configurator persistence — every catalog rubro round-trips raw. -------
  for (const r of RUBRO_CATALOG_E2E) {
    const put = await c.put("/settings/business.vertical", { value: r.value });
    check(put.ok, `configurador guarda rubro "${r.value}" (PUT ok)`);
    const got = (await c.get("/settings/business.vertical")).body;
    eq(got?.value, r.value, `rubro "${r.value}" persiste crudo (no se pliega a otro)`);
  }
  // Branded business name persists alongside the rubro (the sidebar wordmark).
  const NAME = "Salón Showcase E2E";
  await c.put("/settings/business.name", { value: NAME });
  eq((await c.get("/settings/business.name")).body?.value, NAME, "nombre del negocio persiste");

  // (B) Land on a SERVICE rubro (belleza) — exactly what an operator pins in the
  // configurator — and prove the whole daily loop works with NO seed/clinical
  // pack, selling a manual service line.
  await c.put("/settings/business.vertical", { value: "belleza" });
  eq((await c.get("/settings/business.vertical")).body?.value, "belleza", "rubro activo = belleza (servicio)");

  // A service line: priced, NO active_ingredient/laboratory/lote (the agnostic
  // core — a peluquería sells services, not clinical SKUs). Stock is still the
  // server's sale guard (the *UI* is what hides inventory for a service rubro),
  // so we stock it enough to sell.
  const created = (await c.post("/products", {
    name: "Corte de pelo",
    price: "12000",
    stock: 50,
  })).body;
  const svcId = created?.id;
  check(!!svcId, "servicio creado (producto sin lote/clínica)");
  check(!created?.active_ingredient, "servicio NO lleva principio activo (no pack clínico)");

  const open = await c.post("/cash-sessions", { register_name: "Caja Belleza E2E", opening_cash: "0" });
  const sessionId = open.body?.id;
  check(!!sessionId, "caja abierta en rubro servicio");

  const stockBefore = created.stock;
  const sale = await c.post(
    "/pos/sale",
    {
      items: [{ product: svcId, product_name: created.name, quantity: 1, unit_price: "12000" }],
      payment_method: "pos_cash",
      cash_amount: "12000",
    },
    { headers: { "Idempotency-Key": `e2e-belleza-${Date.now()}` } },
  );
  eq(sale.status, 201, "venta de servicio creada (201) — sin receta, sin inventario obligatorio");
  const orderId = sale.body.order?.id;
  check(!!orderId, "venta de servicio devuelve order id");
  const afterSale = (await c.get(`/products/${encodeURIComponent(svcId)}`)).body;
  eq(afterSale.stock, stockBefore - 1, "stock del servicio baja en 1 tras la venta");

  const receipt = (await c.get(`/orders/${orderId}/receipt`)).body;
  check(!!receipt && (receipt.items?.length ?? 0) >= 1, "boleta/recibo del servicio tiene línea");

  // Boleta (DTE SII) is UNIVERSAL — a peluquería emits too. Free no-CAF → coded
  // 4xx upsell, NEVER a 5xx/crash (boleta-gate contract holds in a service rubro).
  const boleta = await c.post(
    "/dte/boletas",
    { order_id: orderId, cert_passphrase: "e2e" },
    { expectOk: false },
  );
  check(boleta.status < 500, `servicio: boleta gestionada limpio (status ${boleta.status}, no 5xx)`);

  // Recetas/controlados are NEVER forced on a service rubro: the libro is
  // reachable (no crash) and empty.
  const recetas = await c.get("/prescriptions", { expectOk: false });
  check(recetas.status < 500, `servicio: recetas accesible sin crash (status ${recetas.status})`);
  if (recetas.status === 200 && Array.isArray(recetas.body)) {
    eq(recetas.body.length, 0, "servicio: cero recetas/controlados dispensados");
  }

  // Core reports stay 200 arrays; the Pro-gated one stays a clean 402 — the
  // preview's "Reportes" category is honest in a service rubro too.
  for (const path of ["/reports/sales-daily", "/reports/top-products"]) {
    const rr = await c.get(path, { expectOk: false });
    check(rr.status === 200 && Array.isArray(rr.body), `servicio: ${path} → 200 array`);
  }
  const margins = await c.get("/reports/margins-daily", { expectOk: false });
  eq(margins.status, 402, "servicio: margins-daily → 402 (Pro-gate, sin 5xx)");

  // Close the day — no open session left behind; the loop has no dead-end.
  const close = await c.post(`/cash-sessions/${sessionId}/close`, { closing_cash_counted: "12000" });
  check(close.ok, "caja del servicio cerrada (cierre del día completo)");
  const salesDaily = await c.get("/reports/sales-daily");
  check(
    Array.isArray(salesDaily.body) && salesDaily.body.length > 0,
    "servicio: la venta del día aparece en sales-daily",
  );
}

// --- business agent ask-bar (read-only) + reports consistency ----------------

/** Fold TODAY's row of /reports/sales-daily into `{revenue, orders}`. The seed
 *  imports historic orders on PAST dates, so we must isolate today's bucket to
 *  compare apples-to-apples with the agent's "ventas hoy" (which sums the same
 *  `sales_daily` rollup over today's UTC range). `DailySalesRow.date` is
 *  "YYYY-MM-DD" in UTC — match it to the UTC date so day boundaries align with
 *  the agent's `today_range`. Money is a STRING on the wire
 *  (rust_decimal::serde::str) and the agent answers with `rev.to_string()` too,
 *  so we fold via Number() — "1234" vs "1234.00" must compare equal. */
async function salesDailyTotals(c) {
  const today = new Date().toISOString().slice(0, 10); // UTC YYYY-MM-DD
  const rows = (await c.get("/reports/sales-daily")).body;
  const list = Array.isArray(rows) ? rows : [];
  let revenue = 0;
  let orders = 0;
  for (const r of list) {
    if (r.date !== today) continue;
    revenue += Number(r.revenue);
    orders += r.orders;
  }
  return { revenue, orders };
}

/** Business agent ("Pregúntale a tu negocio", ADR-0016) end-to-end over HTTP —
 *  the W2 ask-bar (`POST /api/v1/assist/ask`). READ-ONLY: the agent parses a
 *  Spanish question into a deterministic intent and answers from the tenant's own
 *  data. Runs for BOTH verticals AFTER the sales flows have seeded + sold, so the
 *  numbers are non-trivial. Three things only a LIVE run proves:
 *
 *   (1) the three headline questions ("ventas hoy", "qué se vence", "stock de X")
 *       parse to the right intent and answer 200 with grounded `data` (never a
 *       crash, never an empty body);
 *   (2) CONSISTENCY — the figure the agent gives for "ventas hoy" is the SAME
 *       number /reports/sales-daily reports (the agent's `ventas` executor and the
 *       report endpoint both fold `expenses::service::sales_daily`, so a drift here
 *       means someone forked the source — the bug paxoloop must hear about, not a
 *       number to patch on one side). Same for "stock de X" vs /products.
 *   (3) the agent NEVER 402s in the owner's face: the Pro-gated margin question
 *       DEGRADES to a friendly upgrade nudge on Free (ADR-0005 inv. 1), the mirror
 *       of /reports/margins-daily's hard 402 (reports402Matrix). */
export async function agentAskFlow({ tenant, email, password, vertical }) {
  section(`agent ask-bar (read-only) vertical=${vertical} tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // Empty question is a clean 400 (never a 5xx / silent empty answer).
  const empty = await c.post("/assist/ask", { question: "   " }, { expectOk: false });
  eq(empty.status, 400, "pregunta vacía → 400");

  // (1)+(2) "ventas hoy" — right intent, grounded data, and the number MATCHES
  // the report (same source). Everything sold in this run happened today, so the
  // full /reports/sales-daily total equals the agent's "hoy" figure.
  const ventas = await c.post("/assist/ask", { question: "¿cuánto vendí hoy?" });
  eq(ventas.status, 200, "agente responde «ventas hoy» (200)");
  eq(ventas.body?.intent, "ventas_hoy", "intent = ventas_hoy");
  check(ventas.body?.data != null, "respuesta trae data estructurada");
  check(
    typeof ventas.body?.text === "string" && ventas.body.text.length > 0,
    "respuesta trae prosa es-CL no vacía",
  );
  const totals = await salesDailyTotals(c);
  eq(
    Number(ventas.body.data.revenue),
    totals.revenue,
    "CONSISTENCIA: ventas-hoy del agente == total de /reports/sales-daily (misma fuente)",
  );
  eq(
    ventas.body.data.orders,
    totals.orders,
    "CONSISTENCIA: nº de órdenes del agente == /reports/sales-daily",
  );

  // (1) "qué se vence" — parses to por_vencer, answers with a count (0 is a valid
  // answer; a minimarket carries no batches, a pharmacy may). No crash either way.
  const vence = await c.post("/assist/ask", { question: "¿qué se vence?" });
  eq(vence.status, 200, "agente responde «qué se vence» (200)");
  eq(vence.body?.intent, "por_vencer", "intent = por_vencer");
  check(
    typeof vence.body?.data?.count === "number",
    "«qué se vence» trae un conteo numérico",
  );

  // (1)+(2) "stock de X" — pick a real catalog product and assert the agent's
  // reported stock MATCHES /products (the catalog is the single source). The
  // agent searches by term and returns up to 5 matches; we locate ours by exact
  // name so ranking order can't flake the assertion.
  const products = (await c.get("/products")).body;
  const list = Array.isArray(products) ? products : (products?.items ?? []);
  const probe = list.find((p) => p.stock > 0 && p.active !== false);
  check(!!probe, "hay un producto con stock para consultar");
  const stockAsk = await c.post("/assist/ask", { question: `stock de ${probe.name}` });
  eq(stockAsk.status, 200, "agente responde «stock de <producto>» (200)");
  eq(stockAsk.body?.intent, "stock_producto", "intent = stock_producto");
  const matches = stockAsk.body?.data?.matches ?? [];
  const mine = matches.find((m) => m.name === probe.name);
  check(!!mine, `el producto consultado «${probe.name}» aparece en las coincidencias`);
  if (mine) {
    eq(
      mine.stock,
      probe.stock,
      "CONSISTENCIA: stock que reporta el agente == /products (mismo catálogo)",
    );
  }

  // (3) Pro-gated margin DEGRADES (not 402) on Free — the agent must always
  // answer something. Mirror of /reports/margins-daily's hard 402.
  const margen = await c.post("/assist/ask", { question: "¿cuál fue mi margen del mes?" }, { expectOk: false });
  eq(margen.status, 200, "agente «margen del mes» NO 402 en Free (degrada amable)");
  eq(margen.body?.intent, "margen_mes", "intent = margen_mes (aun degradado)");
  check(
    typeof margen.body?.text === "string" && /pro/i.test(margen.body.text),
    "«margen» en Free nudge-ea al plan Pro en vez de 402",
  );

  // Read-only guarantee: asking changed nothing — the probe's stock is intact.
  const after = (await c.get(`/products/${encodeURIComponent(probe.id)}`)).body;
  eq(after.stock, probe.stock, "el agente es read-only: el stock no cambió tras preguntar");
}

// --- business agent WRITE actions (W3, two-step propose/confirm) -------------

/** Business agent WRITE-action flow ("el agente actúa", ADR-0016 Wave 3) over
 *  HTTP — the two-step `propose → confirm` handshake the ask-bar uses to let the
 *  agent register an expense or draft a purchase order WITHOUT ever auto-writing.
 *  This is the live counterpart to the unit tests in crates/assist: only a real
 *  run proves the token round-trips through `/assist/ask` → `/assist/act`, the
 *  domain write lands, and the security gates hold over HTTP. Four contracts:
 *
 *   (1) PROPOSE WRITES NOTHING — asking to register a gasto returns a proposal
 *       with a `confirm_token` but the expense count does NOT move until `/act`.
 *   (2) CONFIRM EXECUTES — posting the token to `/assist/act` creates the real
 *       expense (visible in GET /expenses) and the OC draft (GET /purchase-orders).
 *   (3) SINGLE-USE TOKEN — replaying the same token is rejected; no double-write.
 *   (4) ROLE GATE — a cashier may NOT execute (`/assist/act` is admin/owner only
 *       → 403), and a cashier's write *ask* degrades to a friendly note carrying
 *       NO token (the agent's hands are gated, never the read questions).
 *
 *  Runs on its own tenant with an extra cashier user (run.mjs bootstraps both). */
export async function agentActionFlow({
  tenant,
  email,
  password,
  cashierEmail,
  cashierPassword,
}) {
  section(`agent ACTION (write, two-step) tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // Seed so a real catalog exists (the OC names a product line); harmless to
  // expenses, whose baseline we capture next.
  await c.post("/admin/seed-demo", { vertical: "pharmacy", force: true });

  const countExpenses = async () => {
    const body = (await c.get("/expenses")).body;
    return Array.isArray(body) ? body.length : 0;
  };
  const nBefore = await countExpenses();

  // (1) registrar_gasto — PROPOSE. Returns a token but writes nothing. --------
  const propose = await c.post("/assist/ask", {
    question: "registra un gasto de 5000 en arriendo",
  });
  eq(propose.status, 200, "ask (registrar gasto) → 200");
  eq(propose.body?.intent, "accion_propuesta", "ask write → intent accion_propuesta");
  const proposal = propose.body?.action;
  check(!!proposal, "la respuesta trae una propuesta de acción");
  eq(proposal?.name, "registrar_gasto", "propuesta name = registrar_gasto");
  check(
    typeof proposal?.confirm_token === "string" && proposal.confirm_token.length > 10,
    "la propuesta trae un confirm_token opaco",
  );
  eq(
    await countExpenses(),
    nBefore,
    "PROPONER NO ESCRIBE: el nº de gastos no cambia hasta confirmar",
  );

  // (2) registrar_gasto — CONFIRM. The expense is created for real. -----------
  const act = await c.post("/assist/act", { confirm_token: proposal.confirm_token });
  eq(act.status, 200, "act (confirmar gasto) → 200");
  eq(act.body?.action, "registrar_gasto", "outcome action = registrar_gasto");
  check(!!act.body?.data?.id, "el outcome trae el id del gasto creado");
  eq(Number(act.body?.data?.amount), 5000, "el outcome reporta el monto 5000");
  const expensesNow = (await c.get("/expenses")).body;
  const expList = Array.isArray(expensesNow) ? expensesNow : [];
  eq(expList.length, nBefore + 1, "CONFIRMAR ESCRIBE: se creó exactamente 1 gasto");
  check(
    expList.some((e) => Number(e.amount) === 5000),
    "el gasto de 5000 existe en /expenses tras confirmar",
  );

  // (3) SINGLE-USE token — a replay is rejected and creates no second gasto. ---
  const replay = await c.post(
    "/assist/act",
    { confirm_token: proposal.confirm_token },
    { expectOk: false },
  );
  check(
    replay.status >= 400 && replay.status < 500,
    `token de un solo uso: replay rechazado limpio (${replay.status}, no 5xx)`,
  );
  eq(
    await countExpenses(),
    nBefore + 1,
    "el replay NO crea un segundo gasto (sin doble escritura)",
  );

  // (2b) crear_orden_compra_draft — propose + confirm. -----------------------
  //      build() resolves the supplier by name, so it must exist first.
  const SUP = "Proveedor Agente E2E";
  const sup = await c.post("/suppliers", { name: SUP });
  check(!!sup.body?.id, "proveedor para la OC creado");
  const proposeOc = await c.post("/assist/ask", {
    question: `crea una orden de compra de 10 paracetamol a ${SUP} a $500`,
  });
  eq(proposeOc.status, 200, "ask (crear OC) → 200");
  const ocProposal = proposeOc.body?.action;
  check(!!ocProposal, "la respuesta trae una propuesta de OC");
  eq(ocProposal?.name, "crear_orden_compra_draft", "propuesta name = crear_orden_compra_draft");

  const actOc = await c.post("/assist/act", { confirm_token: ocProposal.confirm_token });
  eq(actOc.status, 200, "act (confirmar OC) → 200");
  eq(actOc.body?.action, "crear_orden_compra_draft", "outcome action = crear_orden_compra_draft");
  const ocId = actOc.body?.data?.id;
  check(!!ocId, "el outcome trae el id de la OC creada");
  eq(actOc.body?.data?.status, "draft", "la OC se crea en estado draft (no compromete stock)");
  const ocDetail = (await c.get(`/purchase-orders/${encodeURIComponent(ocId)}`)).body;
  eq(ocDetail?.status, "draft", "la OC existe en /purchase-orders como draft (efecto real)");

  // (4) ROLE GATE — a cashier may NOT execute write actions. ------------------
  const cashier = new Client();
  await cashier.login(tenant, cashierEmail, cashierPassword);
  // /assist/act is admin/owner only (route layer) → 403 before any token check.
  const forbidden = await cashier.post(
    "/assist/act",
    { confirm_token: "no-importa" },
    { expectOk: false },
  );
  eq(forbidden.status, 403, "el cashier NO puede ejecutar /assist/act → 403");
  // A cashier's write *ask* degrades to a friendly note with NO token (the
  // agent's hands are gated at propose time too — it never even mints a token).
  const cashierAsk = await cashier.post("/assist/ask", {
    question: "registra un gasto de 3000 en luz",
  });
  eq(cashierAsk.status, 200, "el ask de escritura del cashier → 200 (nota amable, no 403)");
  check(cashierAsk.body?.action == null, "el cashier no recibe confirm_token (sin manos)");
  eq(
    await countExpenses(),
    nBefore + 1,
    "el ask del cashier no escribió nada (cuenta de gastos intacta)",
  );
}

// --- service sale: physical_stock = false (W2) -------------------------------

/** A SERVICE rubro sells items that are NOT physical goods — a haircut has no
 *  inventory, no lot, no expiry (migración 0031, `product.physical_stock`). W2
 *  landed the honest flag; this proves the contract end-to-end on a tenant seeded
 *  with the `servicios` pack (every item `physical_stock = false`, `stock = 0`):
 *
 *   - a service line sells with stock 0 → 201 (the sale path SKIPS the
 *     `stock < qty` pre-check for a non-physical item — a service never runs out);
 *   - the sale moves NO inventory: `stock_movements` is empty AND the product's
 *     stock stays 0 (no decrement, no FEFO);
 *   - boleta (DTE SII) stays UNIVERSAL — a salón emits too (Free no-CAF → clean
 *     4xx, never a 5xx);
 *   - services DON'T pollute inventory alerts: with every product at stock 0 the
 *     inventory summary still reports `out_of_stock = 0` (migración 0031 excludes
 *     `physical_stock = false` from low/out-of-stock), cross-checked through the
 *     agent's "resumen de inventario" — tying the ask-bar to the stock model.
 *
 *  Distinct from rubroShowcaseFlow's belleza leg, which sells a STOCKED service
 *  created via POST /products (físico by DB default) — that proves the agnostic
 *  daily loop; THIS proves the physical_stock = false inventory contract. */
export async function serviceSaleFlow({ tenant, email, password }) {
  section(`service sale (physical_stock=false) tenant=${tenant}`);
  const c = new Client();
  await c.login(tenant, email, password);

  // Seed the servicios pack — same service the in-app button / CLI use.
  const seed = await c.post("/admin/seed-demo", { vertical: "servicios", force: true });
  check(seed.ok, "seed-demo servicios ok");
  eq(seed.body?.vertical, "servicios", "summary reporta vertical = servicios");

  // The catalog is all services: no physical stock, no clinical fields.
  const products = (await c.get("/products")).body;
  const list = Array.isArray(products) ? products : (products?.items ?? []);
  check(list.length > 0, `catálogo de servicios no vacío (${list.length} ítems)`);
  const svc = list.find((p) => p.physical_stock === false);
  check(!!svc, "hay un ítem servicio (physical_stock = false)");
  eq(svc.stock, 0, "el servicio se siembra con stock 0 (sin inventario)");
  check(!svc.active_ingredient, "el servicio NO lleva principio activo (sin pack clínico)");

  const open = await c.post("/cash-sessions", { register_name: "Caja Servicio E2E", opening_cash: "0" });
  const sessionId = open.body?.id;
  check(!!sessionId, "caja abierta para venta de servicio");

  // The core: sell a service that has stock 0. The sale path skips the stock
  // pre-check for a non-physical item, so this must succeed (a haircut never
  // runs out of inventory).
  const sale = await c.post(
    "/pos/sale",
    {
      items: [{ product: svc.id, product_name: svc.name, quantity: 1, unit_price: String(svc.price) }],
      payment_method: "pos_cash",
      cash_amount: String(svc.price),
    },
    { headers: { "Idempotency-Key": `e2e-svc-${Date.now()}` } },
  );
  eq(sale.status, 201, "venta de servicio con stock 0 → 201 (salta el chequeo de stock)");
  const orderId = sale.body.order?.id;
  check(!!orderId, "venta de servicio devuelve order id");

  // No inventory moved: no stock_movement AND stock stays 0 (no decrement/FEFO).
  const movements = sale.body.stock_movements ?? [];
  eq(movements.length, 0, "la venta de servicio NO genera stock_movement (no toca inventario)");
  const afterSale = (await c.get(`/products/${encodeURIComponent(svc.id)}`)).body;
  eq(afterSale.stock, 0, "el stock del servicio sigue en 0 tras la venta (sin decremento)");

  // Boleta (DTE SII) is UNIVERSAL — a salón emits too. Free no-CAF → clean 4xx.
  const receipt = (await c.get(`/orders/${orderId}/receipt`)).body;
  check(!!receipt && (receipt.items?.length ?? 0) >= 1, "recibo del servicio tiene línea");
  const boleta = await c.post(
    "/dte/boletas",
    { order_id: orderId, cert_passphrase: "e2e" },
    { expectOk: false },
  );
  check(boleta.status < 500, `servicio: boleta gestionada limpio (status ${boleta.status}, no 5xx)`);

  // Services don't pollute inventory alerts: every product is stock 0 yet the
  // inventory summary reports 0 out-of-stock (migración 0031 excludes services).
  // Cross-checked via the agent's "resumen de inventario" — same stat source.
  const resumen = await c.post("/assist/ask", { question: "resumen de inventario" });
  eq(resumen.body?.intent, "resumen_inventario", "intent = resumen_inventario");
  eq(
    resumen.body?.data?.out_of_stock,
    0,
    "servicios NO cuentan como agotados (out_of_stock = 0 pese a stock 0)",
  );
  check(resumen.body?.data?.total >= 1, "el resumen cuenta los servicios como productos vendibles");

  const close = await c.post(`/cash-sessions/${sessionId}/close`, { closing_cash_counted: String(svc.price) });
  check(close.ok, "caja del servicio cerrada");
}
