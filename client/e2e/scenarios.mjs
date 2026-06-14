// Golden-path scenarios, run once per vertical against a live server. Each
// mirrors a view's click-path and asserts the cashier-visible result. Money is
// always a STRING on the wire (rust_decimal::serde::str) — send and read it so.
import { Client, check } from "./harness.mjs";

const TENANTS = {
  pharmacy: { slug: "farmacia-demo", email: "admin@farmacia.cl" },
  minimarket: { slug: "minimarket-demo", email: "admin@minimarket.cl" },
};

/** login → open caja → venta → boleta boundary → devolución → cierre. */
async function counterDay(c, vertical) {
  // Pick a seeded product to sell (DEMO-* products carry stock + a lot).
  const prods = await c.get("/api/v1/products");
  check(`[${vertical}] products seeded`, prods.status === 200 && prods.body.length > 0,
    `status=${prods.status} n=${prods.body?.length}`);
  const sellable = prods.body.find((p) => p.stock > 0 && p.prescription_type === "direct");
  check(`[${vertical}] has an OTC/direct product in stock`, !!sellable);
  if (!sellable) return;

  // Open cash session.
  const open = await c.post("/api/v1/cash-sessions", {
    register_name: "caja-e2e",
    opening_cash: "10000",
    notes: "e2e",
  });
  check(`[${vertical}] caja abierta`, open.status === 201, `status=${open.status}`);
  const sessionId = open.body?.id;

  // POS sale of 1 unit, cash. POS methods: pos_cash|pos_debit|pos_credit|pos_mixed.
  const sale = await c.post("/api/v1/pos/sale", {
    payment_method: "pos_cash",
    cash_amount: sellable.price,
    items: [
      {
        product: sellable.id,
        product_name: sellable.name,
        quantity: 1,
        unit_price: sellable.price,
      },
    ],
  });
  check(`[${vertical}] venta POS ok`, sale.status === 201 || sale.status === 200,
    `status=${sale.status} body=${JSON.stringify(sale.body).slice(0, 200)}`);
  const order = sale.body?.order;
  check(`[${vertical}] orden queda 'paid'`, order?.status === "paid",
    `status=${order?.status}`);
  const orderId = order?.id;

  // Boleta boundary: reached with auth+cashier, blocked only by emisor/CAF
  // config (no clinical/license gate). cert_passphrase is required by the
  // request schema; on a demo tenant load_emisor fails first → 400.
  if (orderId) {
    const boleta = await c.post("/api/v1/dte/boletas", {
      order_id: orderId,
      cert_passphrase: "e2e-no-cert",
    });
    check(
      `[${vertical}] boleta-emit alcanzable, falla sólo por emisor/CAF`,
      [400, 409].includes(boleta.status),
      `status=${boleta.status} body=${JSON.stringify(boleta.body).slice(0, 200)}`
    );
    check(
      `[${vertical}] boleta-emit NO bloqueada por rol/licencia (no 401/403/402)`,
      ![401, 402, 403].includes(boleta.status),
      `status=${boleta.status}`
    );
  }

  // Devolución (refund) of the sale.
  if (orderId) {
    // devolucion.tipo enum: venta|cancelacion|garantia|error.
    const refund = await c.post("/api/v1/pos/returns", {
      order: orderId,
      tipo: "cancelacion",
      motivo: "e2e devolución",
      metodo_reembolso: "cash",
      items: [
        {
          product: sellable.id,
          product_name: sellable.name,
          quantity: 1,
          unit_price: sellable.price,
          restock: true,
        },
      ],
    });
    check(`[${vertical}] devolución ok`, refund.status === 201 || refund.status === 200,
      `status=${refund.status} body=${JSON.stringify(refund.body).slice(0, 200)}`);
  }

  // Cierre de caja.
  if (sessionId) {
    const close = await c.post(`/api/v1/cash-sessions/${encodeURIComponent(sessionId)}/close`, {
      closing_cash_counted: "10000",
      notes: "cierre e2e",
    });
    check(`[${vertical}] cierre de caja ok`, close.status === 200, `status=${close.status}`);
    check(`[${vertical}] cierre expone arqueo`,
      close.body?.session?.status === "closed" || close.body?.cash_sales !== undefined,
      JSON.stringify(close.body).slice(0, 200));
  }
}

/** producto+lote → recepción OC → stock subió. */
async function restock(c, vertical) {
  const prod = await c.post("/api/v1/products", {
    name: `E2E Restock ${vertical}`,
    price: "1990",
    cost_price: "1000",
    stock: 0,
    prescription_type: "direct",
  });
  check(`[${vertical}] producto creado`, prod.status === 201 || prod.status === 200,
    `status=${prod.status} body=${JSON.stringify(prod.body).slice(0, 200)}`);
  const productId = prod.body?.id;
  if (!productId) return;

  // Lote inicial (trazabilidad).
  const batch = await c.post("/api/v1/batches", {
    product: productId,
    batch_code: "E2E-L1",
    expiry_date: new Date(Date.now() + 180 * 86400_000).toISOString(),
    stock: 0,
    cost: "1000",
  });
  check(`[${vertical}] lote creado`, batch.status === 201 || batch.status === 200,
    `status=${batch.status} body=${JSON.stringify(batch.body).slice(0, 200)}`);

  // Supplier + purchase order.
  const sup = await c.post("/api/v1/suppliers", { name: `Prov E2E ${vertical}` });
  check(`[${vertical}] proveedor creado`, sup.status === 201 || sup.status === 200,
    `status=${sup.status}`);
  const supplierId = sup.body?.id;
  if (!supplierId) return;

  const po = await c.post("/api/v1/purchase-orders", {
    supplier: supplierId,
    items: [{ product: productId, product_name: prod.body.name, quantity: 12, unit_cost: "1000" }],
  });
  check(`[${vertical}] OC creada`, po.status === 201 || po.status === 200,
    `status=${po.status} body=${JSON.stringify(po.body).slice(0, 200)}`);
  const poId = po.body?.id;
  let lineId = po.body?.items?.[0]?.id;
  if (!lineId && poId) {
    const detail = await c.get(`/api/v1/purchase-orders/${encodeURIComponent(poId)}`);
    lineId = detail.body?.items?.[0]?.id;
  }
  check(`[${vertical}] OC tiene línea recepcionable`, !!lineId);
  if (!poId || !lineId) return;

  // KNOWN GAP: a PO is created `draft` and `receive` only accepts
  // `sent`/`approved`/`partially_received`, but no HTTP endpoint transitions a
  // PO out of draft (even crates/api/tests/po_receiving.rs DB-pokes the status).
  // So the receive guard *correctly* rejects a fresh draft — we assert the
  // guard fires, and report the missing approve/send endpoint (see README).
  const recv = await c.post(`/api/v1/purchase-orders/${encodeURIComponent(poId)}/receive`, {
    lines: [{ po_line_id: lineId, qty_received: 12 }],
    notes: "recepción e2e",
  });
  check(`[${vertical}] guard recepción: draft no recepcionable (falta endpoint approve/send)`,
    recv.status === 409, `status=${recv.status} body=${JSON.stringify(recv.body).slice(0, 200)}`);

  // Restock via the supported inventory primitive: a +delta stock-movement
  // materializes product.stock in the same tx. This is the API path that
  // actually raises stock today.
  const mv = await c.post("/api/v1/stock-movements", {
    product: productId,
    delta: 12,
    reason: "recepcion_e2e",
    ref: poId,
  });
  check(`[${vertical}] stock-movement +12 aplicado`, mv.status === 201 || mv.status === 200,
    `status=${mv.status} body=${JSON.stringify(mv.body).slice(0, 200)}`);

  const after = await c.get(`/api/v1/products/${encodeURIComponent(productId)}`);
  check(`[${vertical}] stock subió a 12 tras el movimiento`, after.body?.stock === 12,
    `stock=${after.body?.stock}`);
}

/** GET /reports/margins-daily → 402 FEATURE_REQUIRES_UPGRADE en Free. */
async function free402(c, vertical) {
  const r = await c.get("/api/v1/reports/margins-daily");
  check(`[${vertical}] margins-daily 402 en Free`, r.status === 402, `status=${r.status}`);
  check(`[${vertical}] 402 trae code FEATURE_REQUIRES_UPGRADE`,
    r.body?.error?.code === "FEATURE_REQUIRES_UPGRADE",
    JSON.stringify(r.body).slice(0, 200));
}

/** Minimarket: ningún producto sembrado exige receta/controlado. */
async function minimarketNoReceta(c) {
  const prods = await c.get("/api/v1/products");
  const demo = prods.body.filter((p) => (p.external_id ?? "").startsWith("DEMO-"));
  check("[minimarket] hay productos DEMO", demo.length > 0, `n=${demo.length}`);
  const allDirect = demo.every((p) => p.prescription_type === "direct");
  check("[minimarket] NINGÚN demo exige receta", allDirect,
    demo.filter((p) => p.prescription_type !== "direct").map((p) => p.name).join(","));
  const noClinical = demo.every((p) => !p.active_ingredient);
  check("[minimarket] demos sin ingrediente activo (sin controlado)", noClinical);
}

/** Pharmacy: el surface clínico SÍ existe (recetados + un controlado). */
async function pharmacyHasClinical(c) {
  const prods = await c.get("/api/v1/products");
  const demo = prods.body.filter((p) => (p.external_id ?? "").startsWith("DEMO-"));
  const hasReceta = demo.some((p) => p.prescription_type !== "direct");
  check("[pharmacy] el pack incluye recetados/controlado", hasReceta);
}

export async function runVertical(vertical) {
  const { slug, email } = TENANTS[vertical];
  const c = new Client();
  const sess = await c.login(slug, email);
  check(`[${vertical}] login emite token`, !!sess.token);

  await counterDay(c, vertical);
  await restock(c, vertical);
  await free402(c, vertical);

  if (vertical === "minimarket") await minimarketNoReceta(c);
  if (vertical === "pharmacy") await pharmacyHasClinical(c);
}
