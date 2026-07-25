//! Stock OPERATIVO por sucursal + transferencias entre locales — V2.
//!
//! La migración 0032 dejó `branch`/`register` como records de configuración; el
//! stock seguía siendo global. Acá vive la capa que lo hace operativo sobre el
//! ledger existente, sin inventar un segundo contador que mantener a mano.
//!
//! El on-hand de una sucursal es `SUM(stock_movement.delta)` de los movimientos
//! cuya `branch` es esa sucursal (`NONE` = casa matriz / sitio único),
//! materializado incrementalmente por el evento `product_branch_stock_maint`
//! (migración 0041). Como cada movimiento sigue contando en el ledger global,
//! vale por construcción el invariante V2:
//!
//! ```text
//! Σ_sucursal product_branch_stock.stock == product.stock == Σ stock_movement.delta
//! ```
//!
//! La **transferencia** entre locales es atómica: dos movimientos de suma cero
//! (`-qty` en el origen, `+qty` en el destino) en una sola transacción. El evento
//! reparte el stock entre buckets y `product.stock` queda intacto — cambia la
//! distribución, nunca el total.
//!
//! Concurrencia: la transferencia toma el MISMO lock por tenant que la venta y
//! la devolución ([`crate::locks::tenant_stock_lock`]) — ver ahí por qué un lock
//! por flujo no alcanza.
//!
//! Fuera de alcance de esta capa (lane siguiente): lotes físicos por sucursal
//! (`product_batch.branch`) y FEFO acotado al local. Hoy esto es un ledger de
//! CANTIDADES: la sucursal no puede vender ni transferir más de lo que tiene,
//! pero la elección de lote por vencimiento sigue siendo global.

pub mod model;
pub mod repo;
pub mod service;
