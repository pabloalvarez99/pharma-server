//! Compliance CL (V3): **libro de compras** + **resumen IVA (F29)**.
//!
//! El ERP ya emitía el libro de VENTAS (DTE/SII); faltaba el otro lado: qué
//! compró el negocio y cuánto IVA crédito puede usar. El libro se arma con las
//! OC recepcionadas del período, por fecha de FACTURA del proveedor cuando el
//! operador la capturó (migración 0040), derivando neto/IVA del total al 19% CL
//! mientras no la declare — así el dueño tiene una cifra desde el día uno y ve
//! qué filas le faltan completar.

pub mod model;
pub mod repo;

pub use model::*;
