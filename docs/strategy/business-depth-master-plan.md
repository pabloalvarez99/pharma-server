# Business Depth Master Plan — "El negocio chileno completo"

> **Directiva fundador (2026-06-21):** seguir mejorando el programa (LLM diferido). Este
> es el horizonte post-config-center: de "cubre el mostrador" a "cubre el NEGOCIO real
> chileno". Ultra-plan que paxoloop dispara como olas tras W6. Compañero de
> [`product-improvement-master-plan.md`](./product-improvement-master-plan.md) +
> [`professional-completeness-master-plan.md`](./professional-completeness-master-plan.md).

---

## 0. Tesis

El ERP ya cubre el **mostrador**: POS, inventario (lotes/vencimiento/FEFO/WAC), caja,
devoluciones, compras, gastos, recetas, DTE/SII, reportes con insight, agente (lee+actúa),
config center, multi-rubro (4 reales). Falta la **profundidad del negocio real chileno** —
lo que un almacenero/farmacéutico/dueño hace todos los días y hoy no puede:

- **Fiar** a un cliente habitual y llevarle la cuenta. (No existe.)
- Operar **2+ locales** con stock real por sucursal y transferencias. (branches W5 = solo
  records; el stock NO está por sucursal.)
- Sacar el **libro de compras** y un resumen de **IVA (F29)**. (Solo hay libro de ventas.)
- Que el POS **imprima** la boleta en la térmica y abra el cajón. (Hoy es en pantalla.)

Cerrar esto convierte "buen ERP de mostrador" en "el sistema del negocio".

## 1. Vectores (ranked por valor × buildability, todo offline, sin LLM/creds SII)

### V1 — CUENTA CORRIENTE / FIADO ⭐ (el #1 esencial CL)
El retail chico chileno vive del "fiado" al cliente habitual. Sin esto, el almacenero
sigue con el cuaderno. Saldo por cliente (ledger inmutable), vender "a cuenta" (fiar),
registrar abonos, estado de cuenta imprimible, límite de crédito opcional. El agente
puede "¿cuánto me debe Juan?" / "registrá un abono de $5.000 de Juan".
- Backend: tabla `customer_account` + `account_movement` (cargo venta / abono), tenant-
  scoped, mig nueva; servicio saldo/cargo/abono atómico (patrón cash-session lock).
- Cliente: en POS opción "Fiar a cuenta" (cliente con saldo) + en Clientes el saldo +
  estado de cuenta + registrar abono.

### V2 — MULTI-SUCURSAL OPERATIVO (desbloquea Business tier)
W5 dejó `branch`/`register` como records de config; el **stock sigue siendo global**.
Hacerlo operativo: stock POR sucursal, **transferencias** entre locales (con movimiento
auditado en ambas puntas), ventas/caja atadas a la sucursal activa, reportes por sucursal.
- Backend: stock por (producto, sucursal) — mig + repo; transferencia = 2 movimientos
  atómicos; sale/caja toman `branch` activo. Invariante: Σ stock-por-sucursal == stock total.
- Cliente: selector de sucursal (shell), stock/dashboard/reportes filtrados por sucursal,
  UI de transferencia.

### V3 — COMPLIANCE: libro de compras + resumen IVA/F29
Hoy solo libro de ventas. Agregar **libro de compras** (registro de OC/facturas recibidas)
+ **resumen IVA**: débito fiscal (ventas) − crédito fiscal (compras) = el número que el
dueño lleva a su F29. **100% local, sin creds SII** (es agregación de datos propios).
- Backend: agregación libro-compras desde compras/recepciones + endpoint resumen IVA mensual.
- Cliente: vista Reportes → Libro de compras + Resumen IVA del mes, exportable.

### V4 — INVENTARIO INTELIGENTE
- **Auto-reorder**: min/máx por producto + velocidad de venta → sugerencia "comprá N de X"
  (alimenta una OC draft con 1 click). 
- **Tomas de inventario / ajustes**: cycle count, ajuste con motivo auditado, kardex por
  producto (historial de movimientos legible).

### V5 — HARDWARE: impresora térmica ESC/POS + cajón
Un POS real imprime. Boleta/comanda a impresora térmica (ESC/POS por USB/serial/red) +
apertura de cajón (pulse). La sección Hardware del config center se vuelve real (elegir
impresora, test print). Vertical: comanda para restaurant, boleta para retail.

### V6 — DOCUMENTOS PDF / IMPRIMIBLES
Boletas, estados de cuenta (V1), OC, reportes a PDF/impresión limpia. Soporta V1 (estado
de cuenta) y V3 (libro/resumen). Vendor-agnostic (el dueño se lleva sus papeles).

## 2. Ola 7 propuesta (top 5, off el tip post-W6, scope disjunto)

| Worker | Vector | Lane | Scope |
|---|---|---|---|
| **milton** | V1 backend | `feat/cuenta-corriente` | `customer_account` ledger (saldo/cargo/abono atómico) + mig + intents agente |
| **paul** | V1 cliente | `feat/pos-fiado` | POS "fiar a cuenta" + Clientes saldo/abono/estado (pos/clientes) |
| **marvin** | V2 backend | `feat/stock-multisucursal` | stock por sucursal + transferencias (domain + mig) |
| **ye** | V2 cliente | `feat/sucursal-operativa` | selector sucursal (shell) + stock/dashboard por sucursal + UI transferencia |
| **bob** | V3 | `feat/libro-compras-iva` | libro compras + resumen IVA/F29 + UI reportes + e2e |

Coord: ledger (milton) ↔ POS/clientes UI (paul) contrato congelado · stock-multisucursal
(marvin) ↔ selector/UI (ye) contrato congelado · mig: milton=0034 (account), marvin=0035
(stock-branch) · src-tauri solo donde haga falta (coordinar paul/ye) · format.ts append=bob.

## 3. Definición de hecho

- [ ] El almacenero **fía** una venta a un cliente y le **cobra** el fiado después; ve el saldo.
- [ ] El dueño ve **stock por sucursal**, **transfiere** entre 2 locales, vende en la sucursal activa.
- [ ] Saca **libro de compras** + **resumen IVA del mes** (lo que va al F29), exportable.
- [ ] *(V4/V5/V6 en olas siguientes)* auto-reorder, impresión térmica, PDFs.
- [ ] Todo offline-first, multi-rubro, GATE verde, e2e. Sensación: "esto ES el sistema de mi negocio".

> Norte: el dueño deja el cuaderno de fiados y la planilla de IVA. El programa ES su negocio,
> no solo su caja.
