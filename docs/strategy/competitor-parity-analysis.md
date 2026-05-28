# Competidores Chile — análisis paridad (2026-05-21)

Análisis de competencia directa para informar [ADR-0010](../adr/0010-roadmap-fase-9-parity.md). Foco: software farmacia CL on-prem o cloud que un cliente real evaluaría junto a pharma-server.

## Mapa de competidores

| Competidor | Tipo | Pricing público | Pillar único |
|---|---|---|---|
| **Defontana ERP** | Cloud B2B genérico, 10k clientes CL, 25 años | Sales-quoted | Vertical farmacia es adaptación, no especialista |
| **Bsale** | POS cloud SMB no-farmacia | Standard $76k+IVA/mes, Omnicanal $117k+IVA, +1 UF/sucursal | POS móvil con boleta electrónica desde celular |
| **GOLAN** | Farmacia + droguería específico | Custom-quoted por tamaño | Offline-first declarado + multi-OS (Win/Mac/Linux) + integraciones masivas (Transbank, BCI Pagos, WooCommerce, Shopify, Prestashop, Remedia, PedidosYa, ZeroQ, YAPP) |
| **SICO** | Farmacia específico CL | Demo gratis + custom | On-premises puro, email delivery, X/Z fiscales completos, multi-bodega, multi-caja |
| **t-Farmacias** (tejesoft) | Cloud SaaS multi-platform | "Desde 1 user, 1000 DTE, 5000 SKU" modular + 30-day money-back | Alternativa terapéutica + auto-PO max/min stock + roles segmentados (admin/químico) |
| **iFarmacias** (ACHIFARP oficial) | Farmacia comunal, 800k beneficiarios, 52 estab. | 10 UF setup + 0.65 UF/user + 2 UF fraccionamiento + 3 UF facturación + 4.1 UF RAYEN | Fractionamiento dispensación + RAYEN integration + segmento municipal lockeado |
| **ControlMagistral** (Asisma) | Recetario magistral cloud | Custom | Cálculo precio receta por PA + etiquetas ZPL/PDF + cumplimiento ISP completo recetario |
| **ERP Fusion** | Farmacia comunal + BI | Custom | BI integrado |

## Features observadas — clasificación por tier impacto

### Tier S — game-changers (no tenemos, competidores SÍ)

1. **Fractionamiento dispensación** (iFarmacias) — venta medio blister, pastillas sueltas. Bloqueador farmacia popular/comunal.
2. **Integración RAYEN** (iFarmacias) — sistema clínico MINSAL oficial APS. Bloqueador farmacia municipal.
3. **ISAPRE convenios** (GOLAN, SICO, t-Farmacias) — cobranza directa al pagador. Bloqueador segmento premium urbano.
4. **Recetario magistral cálculo PA** (ControlMagistral) — preparados magistrales 30-50% revenue boticas serias.
5. **Alternativa terapéutica** (t-Farmacias) — sugerir equivalente cuando no hay stock pedido. Clínico + operativo.
6. **Webpay POS integration** (todos) — cobro tarjeta físico desde POS. Bloqueador comercial CL.
7. **DTEs completos** (todos) — boleta + factura + NC + ND + GD + reportes X/Z fiscales. Tenemos parcial.
8. **Multi-bodega + transfers** (SICO, GOLAN, t-Farmacias) — cadenas chicas 3-10 locales.
9. **Max/min stock + auto-PO** (t-Farmacias) — reposición automática.
10. **Etiquetas ZPL impresora Zebra** (ControlMagistral) — estándar farmacia.

### Tier A — diferenciadores comerciales fuertes (no tenemos)

11. **YAPP / ChileSalud reembolsos** (GOLAN) — seguros complementarios crecientes.
12. **Promo kits / combos** (t-Farmacias, SICO) — bundling campañas.
13. **Multi-caja apertura/cierre/arqueo** (SICO) — operativo POS estándar.
14. **WooCommerce / Shopify / Prestashop sync** (GOLAN) — omnicanal.
15. **PedidosYa / Remedia** (GOLAN) — delivery apps.
16. **POS móvil con boleta electrónica desde celular** (Bsale) — celular = caja.
17. **30-day money-back guarantee** (t-Farmacias) — reduce fricción decisión.
18. **BI dashboard ejecutivo** (ERP Fusion, Defontana) — reportes gerenciales.

### Tier B — nice-to-have

19. **ZeroQ appointment** (GOLAN) — vacunaciones agendamiento.
20. **Multi-OS Linux/Mac** (GOLAN) — target CL ≈ 100% Windows, baja prioridad.

## Lo que solo pharma-server tiene (defender + capitalizar)

1. **MSI 1-click install puro Windows** — competencia hace web/cloud o setup técnico (GOLAN). Solo SICO se acerca pero email-delivery + zip manual.
2. **Offline real garantizado** — GOLAN dice "funciona sin Internet" pero requiere DTEs online y license cloud. Pharma-server core 100% offline (DTE eventual sync futuro).
3. **License Ed25519 offline-first** — nadie tiene. Tier control sin internet → opera en sitios sin conectividad estable (rural CL común).
4. **Sin lock-in datos** (Free incluye export CSV/JSON full) — Bsale/Defontana/iFarmacias secuestran data en cloud. Comprobar real en SICO/GOLAN; probable sí lock-in propietario.
5. **Compromiso continuidad** ([ADR-0005](../adr/0005-core-gratis-no-locked-in.md)) — última release funcional perpetua si empresa cierra. Único en mercado.
6. **Federación B2B Fase 13** — nadie cerca. Quote+PO firmado Ed25519 entre farmacias/proveedores.
7. **Freemium permanente real** — competencia mínimo $60-90k CLP/mes; Free tier pharma-server = disruptivo.

## Pricing benchmark sintetizado

| Tier típico CL | Costo mensual aprox | Quién |
|---|---|---|
| Free | $0 | nadie en farmacia hoy |
| Setup mínimo | $25-90k | Bsale básico, iFarmacias user adicional |
| POS funcional | $76-90k | Bsale Standard |
| Multi-canal | $117-150k | Bsale Omnicanal |
| Cadena chica (3-10 sucursales) | $200-500k | GOLAN/Defontana custom |
| Farmacia popular típica iFarmacias | $745k setup + $78k/mes | iFarmacias |

### Posicionamiento ideal pharma-server (propuesta)

- **Free** permanente — disruptivo. Competencia mínima $60k/mes.
- **Pro** ≈ $39-49k/mes (mitad Bsale Standard).
- **Business** ≈ $80-119k/mes (≈ Bsale Omnicanal, on-prem + features adicionales).
- **Enterprise** custom.
- **Microtx** one-time ($9.990-29.990 CLP) — desbloqueo features especializadas (magistral, RAYEN, ISAPRE individuales).

Validación pricing pendiente con prospects reales (Fase 9.5).

## Implicaciones roadmap

Roadmap actual (Fase 10-14 mergeable) bien direccionado pero **falta paridad tabla stake**. Sin Tier S #1-10 implementados, pharma-server NO es vendible vs SICO/GOLAN para farmacia operativa real.

Solución: ejecutar Fase 9.x secuenciada antes de Fase 12 (sync) o Fase 13 (marketplace). Detalle: [ADR-0010](../adr/0010-roadmap-fase-9-parity.md).

## Sources

- [TOP 10 Software para Farmacias en Chile (ComparaSoftware)](https://www.comparasoftware.cl/farmacias)
- [t-Farmacias (tejesoft)](https://www.tejesoft.com/page/farmacias)
- [SICO Chile — Software Farmacia Botica](https://sicochile.com/productos/software-farmacias-chile/)
- [GOLAN Software Farmacias y Droguerías](https://www.golan.cl/)
- [iFarmacias (ACHIFARP)](https://www.ifarmacias.cl/)
- [ControlMagistral (Asisma)](https://www.controlmagistral.cl/)
- [Sistema POS Chile 2026 comparativa (Webiados)](https://webiados.com/blog/sistema-pos-chile-2026-comparativa)
- [Bsale planes y precios](https://www.bsale.cl/product/plan-basico)
- [Defontana Chile productos](https://www.defontana.com/cl/productos/)
