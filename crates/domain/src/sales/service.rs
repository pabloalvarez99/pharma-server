//! Sales service (Fase 4). Public surface lands in the next slice. This
//! placeholder pins the public function names so the `api` crate's
//! `/api/v1/pos/...` router can compile against stable identifiers as we
//! port the logic from Tu Farmacia
//! `apps/web/src/app/api/admin/pos/sale/route.ts`.
//!
//! Planned signatures (next slice):
//! * `post_sale(db, tenant, sold_by, idempotency_key, req: PosSaleRequest)
//!     -> DomainResult<PosSaleResponse>`
//! * `list_orders(db, tenant, filters) -> DomainResult<Vec<OrderDto>>`
//! * `get_order(db, tenant, id) -> DomainResult<(OrderDto, Vec<OrderItemDto>)>`
//! * `create_devolucion(db, tenant, user, new: NewDevolucion)
//!     -> DomainResult<DevolucionDto>`
//! * `get_setting(db, tenant, key) -> DomainResult<Option<AdminSettingDto>>`
//! * `set_setting(db, tenant, key, value) -> DomainResult<AdminSettingDto>`
