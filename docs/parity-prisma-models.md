# Prisma Schema Models — Complete Inventory

Source: `C:\Users\Administrator\Documents\GitHub\build-and-deploy-webdev-asap\pharmacy-ecommerce\apps\web\prisma\schema.prisma`

Used as input for `docs/parity-schema-mapping.md` (Prisma → SurrealDB) and per-phase migrations `migrations/NNNN_*.surql`.

## Global Configuration

- Datasource: PostgreSQL
- Generator: Prisma Client JS
- No enums defined
- No `@@schema` directive

---

### 1. profiles (postgres: profiles)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @db.Uuid |
| name | String? | @db.VarChar(255) |
| rut | String? | @db.VarChar(20) |
| phone | String? | @db.VarChar(20) |
| role | String? | @default("user") @db.VarChar(50) |
| loyalty_points | Int | @default(0) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |

**Indexes:** None

**Relations:**
- loyalty_transactions (1:N, backref)
- push_subscriptions (1:N, backref)

---

### 2. push_subscriptions (postgres: push_subscriptions)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| user_id | String? | @db.Uuid |
| endpoint | String | @unique |
| p256dh | String | |
| auth | String | |
| user_agent | String? | |
| created_at | DateTime? | @default(now()) @db.Timestamptz(6) |
| last_used_at | DateTime? | @db.Timestamptz(6) |

**Indexes:**
- @@index([user_id])
- @@index([created_at(sort: Desc)])

**Relations:**
- profiles (N:1, FK: user_id → profiles.id, onDelete: Cascade)

---

### 3. categories (postgres: categories)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| name | String | @db.VarChar(255) |
| slug | String | @unique @db.VarChar(255) |
| description | String? | |
| image_url | String? | @db.VarChar(500) |
| active | Boolean | @default(true) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |

**Indexes:** None

**Relations:**
- products (1:N, backref)

---

### 4. products (postgres: products)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| name | String | @db.VarChar(255) |
| slug | String | @unique @db.VarChar(255) |
| description | String? | |
| price | Decimal | @db.Decimal(10, 2) |
| cost_price | Decimal? | @db.Decimal(10, 2) |
| stock | Int | @default(0) |
| category_id | String? | @db.Uuid |
| image_url | String? | @db.VarChar(500) |
| active | Boolean | @default(true) |
| external_id | String? | @db.VarChar(50) |
| laboratory | String? | @db.VarChar(255) |
| therapeutic_action | String? | @db.VarChar(255) |
| active_ingredient | String? | @db.VarChar(500) |
| prescription_type | String? | @default("direct") @db.VarChar(50) |
| presentation | String? | @db.VarChar(255) |
| discount_percent | Int? | |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |

**Indexes:**
- @@index([category_id])
- @@index([active])
- @@index([laboratory])
- @@index([therapeutic_action])
- @@index([prescription_type])
- @@index([external_id])
- @@index([stock])
- @@index([created_at(sort: Desc)])

**Relations:**
- categories (N:1, FK: category_id → categories.id, onDelete: SetNull)
- order_items (1:N, backref)
- stock_movements (1:N, backref)
- purchase_order_items (1:N, backref)
- supplier_product_mappings (1:N, backref)
- product_barcodes (1:N, backref)
- faltas (1:N, backref)
- product_batches (1:N, backref)
- supplier_price_lists (1:N, backref)
- prescription_records (1:N, backref)

---

### 5. barcode_catalog (postgres: barcode_catalog)

| Field | Type | Attributes |
|-------|------|-----------|
| external_id | String | @db.VarChar(50) |
| barcode | String | @id @db.VarChar(50) |

**Indexes:**
- @@index([external_id])

**Relations:** None

---

### 6. product_barcodes (postgres: product_barcodes)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| product_id | String | @db.Uuid |
| barcode | String | @unique @db.VarChar(50) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([product_id])

**Relations:**
- products (N:1, FK: product_id → products.id, onDelete: Cascade)

---

### 7. orders (postgres: orders)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| user_id | String? | @db.Uuid |
| status | String | @default("pending") @db.VarChar(50) |
| total | Decimal | @db.Decimal(10, 2) |
| payment_provider | String? | @default("store") @db.VarChar(50) |
| cash_amount | Decimal? | @db.Decimal(10, 2) |
| card_amount | Decimal? | @db.Decimal(10, 2) |
| guest_email | String? | @db.VarChar(255) |
| guest_session_id | String? | @db.VarChar(255) |
| guest_name | String? | @db.VarChar(255) |
| guest_surname | String? | @db.VarChar(255) |
| customer_phone | String? | @db.VarChar(20) |
| pickup_code | String? | @db.VarChar(10) |
| tracking_token | String? | @unique @db.VarChar(64) |
| reservation_expires_at | DateTime? | @db.Timestamptz(6) |
| shipping_address | String? | |
| notes | String? | |
| sold_by_user_id | String? | @db.VarChar(255) |
| sold_by_name | String? | @db.VarChar(255) |
| mercadopago_preference_id | String? | @db.VarChar(255) |
| mercadopago_payment_id | String? | @db.VarChar(255) |
| stripe_checkout_session_id | String? | @db.VarChar(255) |
| stripe_payment_intent_id | String? | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |

**Indexes:**
- @@index([user_id])
- @@index([status])
- @@index([guest_email])
- @@index([pickup_code])
- @@index([reservation_expires_at])
- @@index([created_at(sort: Desc)])

**Relations:**
- order_items (1:N, backref)
- devoluciones (1:N, backref)
- prescription_records (1:N, backref)

---

### 8. order_items (postgres: order_items)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| order_id | String | @db.Uuid |
| product_id | String? | @db.Uuid |
| product_name | String | @db.VarChar(255) |
| quantity | Int | |
| price_at_purchase | Decimal | @db.Decimal(10, 2) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([order_id])
- @@index([product_id])

**Relations:**
- orders (N:1, FK: order_id → orders.id, onDelete: Cascade)
- products (N:1, FK: product_id → products.id, onDelete: SetNull)

---

### 9. therapeutic_category_mapping (postgres: therapeutic_category_mapping)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| therapeutic_action | String | @db.VarChar(255) |
| category_slug | String | @db.VarChar(100) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:** None

**Relations:** None

---

### 10. admin_settings (postgres: admin_settings)

| Field | Type | Attributes |
|-------|------|-----------|
| key | String | @id @db.VarChar(100) |
| value | String | |

**Indexes:** None

**Relations:** None

---

### 11. stock_movements (postgres: stock_movements)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| product_id | String? | @db.Uuid |
| delta | Int | |
| reason | String | @db.VarChar(50) |
| admin_id | String | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([product_id])
- @@index([reason])
- @@index([created_at(sort: Desc)])

**Relations:**
- products (N:1, FK: product_id → products.id, onDelete: Cascade)

---

### 12. suppliers (postgres: suppliers)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| name | String | @db.VarChar(255) |
| rut | String? | @db.VarChar(20) |
| contact_name | String? | @db.VarChar(255) |
| contact_email | String? | @db.VarChar(255) |
| contact_phone | String? | @db.VarChar(20) |
| website | String? | @db.VarChar(255) |
| notes | String? | |
| active | Boolean | @default(true) |
| default_invoice_format | String? | @db.VarChar(20) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |

**Indexes:** None

**Relations:**
- purchase_orders (1:N, backref)
- supplier_product_mappings (1:N, backref)
- supplier_price_lists (1:N, backref)

---

### 13. purchase_orders (postgres: purchase_orders)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| supplier_id | String | @db.Uuid |
| invoice_number | String? | @db.VarChar(100) |
| invoice_date | DateTime? | @db.Date |
| status | String | @default("draft") @db.VarChar(20) |
| total_cost | Decimal? | @db.Decimal(10, 2) |
| subtotal_net | Decimal? | @db.Decimal(10, 2) |
| tax_amount | Decimal? | @db.Decimal(10, 2) |
| invoice_format | String? | @db.VarChar(20) |
| po_reference | String? | @db.VarChar(100) |
| notes | String? | |
| image_url | String? | @db.VarChar(500) |
| ocr_raw | String? | |
| created_by | String | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |
| paid | Boolean | @default(false) |
| paid_at | DateTime? | @db.Timestamptz(6) |
| payment_method_ap | String? | @db.VarChar(50) |
| due_date | DateTime? | @db.Date |

**Indexes:** None

**Relations:**
- suppliers (N:1, FK: supplier_id → suppliers.id)
- purchase_order_items (1:N, backref)
- purchase_payments (1:N, backref)

---

### 14. purchase_order_items (postgres: purchase_order_items)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| purchase_order_id | String | @db.Uuid |
| product_id | String? | @db.Uuid |
| supplier_product_code | String? | @db.VarChar(100) |
| product_name_invoice | String? | @db.VarChar(255) |
| quantity | Int | |
| unit_cost | Decimal | @db.Decimal(10, 2) |
| subtotal | Decimal | @db.Decimal(10, 2) |
| batch_code | String? | @db.VarChar(100) |
| expiry_date | DateTime? | @db.Date |

**Indexes:** None

**Relations:**
- purchase_orders (N:1, FK: purchase_order_id → purchase_orders.id, onDelete: Cascade)
- products (N:1, FK: product_id → products.id, onDelete: SetNull)

---

### 15. supplier_product_mappings (postgres: supplier_product_mappings)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| supplier_id | String | @db.Uuid |
| supplier_code | String | @db.VarChar(100) |
| product_id | String | @db.Uuid |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:** None

**Unique Constraints:**
- @@unique([supplier_id, supplier_code])

**Relations:**
- suppliers (N:1, FK: supplier_id → suppliers.id, onDelete: Cascade)
- products (N:1, FK: product_id → products.id, onDelete: Cascade)

---

### 16. loyalty_transactions (postgres: loyalty_transactions)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| user_id | String | @db.Uuid |
| order_id | String? | @db.Uuid |
| points | Int | |
| reason | String | @db.VarChar(50) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:** None

**Relations:**
- profiles (N:1, FK: user_id → profiles.id, onDelete: Cascade)

---

### 17. faltas (postgres: faltas)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| product_id | String? | @db.Uuid |
| product_name | String | @db.VarChar(255) |
| customer_name | String? | @db.VarChar(255) |
| customer_phone | String? | @db.VarChar(20) |
| quantity | Int | @default(1) |
| status | String | @default("pending") @db.VarChar(20) |
| notes | String? | |
| notified_at | DateTime? | @db.Timestamptz(6) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:** None

**Relations:**
- products (N:1, FK: product_id → products.id, onDelete: SetNull)

---

### 18. product_batches (postgres: product_batches)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| product_id | String | @db.Uuid |
| batch_code | String? | @db.VarChar(100) |
| expiry_date | DateTime | @db.Date |
| quantity | Int | |
| notes | String? | |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:** None

**Relations:**
- products (N:1, FK: product_id → products.id, onDelete: Cascade)

---

### 19. supplier_price_lists (postgres: supplier_price_lists)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| supplier_id | String | @db.Uuid |
| product_id | String | @db.Uuid |
| unit_price | Decimal | @db.Decimal(10, 2) |
| valid_from | DateTime | @db.Date |
| valid_until | DateTime? | @db.Date |
| notes | String? | |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([supplier_id])
- @@index([product_id])

**Relations:**
- suppliers (N:1, FK: supplier_id → suppliers.id, onDelete: Cascade)
- products (N:1, FK: product_id → products.id, onDelete: Cascade)

---

### 20. caja_cierres (postgres: caja_cierres)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| turno_inicio | DateTime | @db.Timestamptz(6) |
| turno_fin | DateTime | @default(now()) @db.Timestamptz(6) |
| fondo_inicial | Decimal | @default(0) @db.Decimal(10, 2) |
| ventas_efectivo | Decimal | @default(0) @db.Decimal(10, 2) |
| ventas_debito | Decimal | @default(0) @db.Decimal(10, 2) |
| ventas_credito | Decimal | @default(0) @db.Decimal(10, 2) |
| ventas_total | Decimal | @default(0) @db.Decimal(10, 2) |
| num_transacciones | Int | @default(0) |
| efectivo_esperado | Decimal | @default(0) @db.Decimal(10, 2) |
| efectivo_contado | Decimal | @default(0) @db.Decimal(10, 2) |
| diferencia | Decimal | @default(0) @db.Decimal(10, 2) |
| notas | String? | @db.Text |
| cerrado_por | String? | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:** None

**Relations:** None

---

### 21. devoluciones (postgres: devoluciones)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| order_id | String? | @db.Uuid |
| tipo | String | @default("venta") @db.VarChar(20) |
| motivo | String | @db.VarChar(255) |
| notas | String? | @db.Text |
| total_devuelto | Decimal | @db.Decimal(10, 2) |
| metodo_reembolso | String? | @db.VarChar(50) |
| procesado_por | String? | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([order_id])
- @@index([created_at(sort: Desc)])

**Relations:**
- orders (N:1, FK: order_id → orders.id, onDelete: SetNull)
- devolucion_items (1:N, backref)

---

### 22. devolucion_items (postgres: devolucion_items)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| devolucion_id | String | @db.Uuid |
| product_id | String? | @db.Uuid |
| product_name | String | @db.VarChar(255) |
| quantity | Int | |
| unit_price | Decimal | @db.Decimal(10, 2) |
| restock | Boolean | @default(true) |

**Indexes:**
- @@index([devolucion_id])

**Relations:**
- devoluciones (N:1, FK: devolucion_id → devoluciones.id, onDelete: Cascade)

---

### 23. audit_log (postgres: audit_log)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| user_email | String | @db.VarChar(255) |
| action | String | @db.VarChar(50) |
| entity | String | @db.VarChar(100) |
| entity_id | String? | @db.VarChar(255) |
| entity_name | String? | @db.VarChar(255) |
| changes | Json? | |
| ip_address | String? | @db.VarChar(50) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([entity, entity_id])
- @@index([user_email])
- @@index([created_at(sort: Desc)])

**Relations:** None

---

### 24. prescription_records (postgres: prescription_records)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| order_id | String? | @db.Uuid |
| product_id | String? | @db.Uuid |
| product_name | String | @db.VarChar(255) |
| quantity | Int | |
| prescription_number | String? | @db.VarChar(100) |
| patient_name | String | @db.VarChar(255) |
| patient_rut | String? | @db.VarChar(20) |
| doctor_name | String? | @db.VarChar(255) |
| medical_center | String? | @db.VarChar(255) |
| prescription_date | DateTime? | @db.Date |
| is_controlled | Boolean | @default(false) |
| dispensed_by | String? | @db.VarChar(255) |
| dispensed_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([dispensed_at])
- @@index([patient_rut])

**Relations:**
- orders (N:1, FK: order_id → orders.id, onDelete: SetNull)
- products (N:1, FK: product_id → products.id, onDelete: SetNull)

---

### 25. pharmacist_shifts (postgres: pharmacist_shifts)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| pharmacist_name | String | @db.VarChar(255) |
| pharmacist_rut | String | @db.VarChar(20) |
| shift_start | DateTime | @db.Timestamptz(6) |
| shift_end | DateTime? | @db.Timestamptz(6) |
| notes | String? | |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([shift_start])

**Relations:** None

---

### 26. purchase_payments (postgres: purchase_payments)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| purchase_order_id | String | @db.Uuid |
| amount | Decimal | @db.Decimal(10, 2) |
| payment_method | String | @db.VarChar(50) |
| paid_at | DateTime | @db.Timestamptz(6) |
| notes | String? | |
| created_by | String | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([purchase_order_id])
- @@index([paid_at(sort: Desc)])

**Relations:**
- purchase_orders (N:1, FK: purchase_order_id → purchase_orders.id, onDelete: Cascade)

---

### 27. gasto_categories (postgres: gasto_categories)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| name | String | @db.VarChar(100) |
| type | String | @default("variable") @db.VarChar(20) |
| sort_order | Int | @default(0) |

**Indexes:** None

**Relations:**
- gastos (1:N, backref)
- recurring_expenses (1:N, backref)

---

### 28. gastos (postgres: gastos)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| category_id | String | @db.Uuid |
| description | String | @db.VarChar(255) |
| amount | Decimal | @db.Decimal(10, 2) |
| expense_date | DateTime | @db.Date |
| paid_at | DateTime? | @db.Timestamptz(6) |
| payment_method | String? | @db.VarChar(50) |
| recurring_expense_id | String? | @db.Uuid |
| created_by | String | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |

**Indexes:**
- @@index([category_id])
- @@index([expense_date(sort: Desc)])
- @@index([paid_at])

**Relations:**
- gasto_categories (N:1, FK: category_id → gasto_categories.id)
- recurring_expenses (N:1, FK: recurring_expense_id → recurring_expenses.id, onDelete: SetNull)

---

### 29. recurring_expenses (postgres: recurring_expenses)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| category_id | String | @db.Uuid |
| description | String | @db.VarChar(255) |
| amount | Decimal | @db.Decimal(10, 2) |
| day_of_month | Int | |
| active | Boolean | @default(true) |
| created_by | String | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |

**Indexes:**
- @@index([active])

**Relations:**
- gasto_categories (N:1, FK: category_id → gasto_categories.id)
- gastos (1:N, backref)

---

### 30. internal_tasks (postgres: internal_tasks)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| title | String | @db.VarChar(255) |
| description | String? | @db.Text |
| assigned_to_uid | String? | @db.VarChar(255) |
| assigned_to_name | String? | @db.VarChar(255) |
| assigned_role | String? | @db.VarChar(20) |
| priority | String | @default("normal") @db.VarChar(10) |
| due_date | DateTime? | @db.Date |
| status | String | @default("open") @db.VarChar(15) |
| created_by_uid | String | @db.VarChar(255) |
| created_by_name | String? | @db.VarChar(255) |
| completed_at | DateTime? | @db.Timestamptz(6) |
| completed_by_uid | String? | @db.VarChar(255) |
| completed_by_name | String? | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |

**Indexes:**
- @@index([status, assigned_to_uid])
- @@index([assigned_role, status])
- @@index([created_at(sort: Desc)])

**Relations:** None

---

### 31. announcements (postgres: announcements)

| Field | Type | Attributes |
|-------|------|-----------|
| id | String | @id @default(dbgenerated("gen_random_uuid()")) @db.Uuid |
| title | String | @db.VarChar(255) |
| body | String | @db.Text |
| severity | String | @default("info") @db.VarChar(15) |
| visible_to | String | @default("all") @db.VarChar(20) |
| pinned | Boolean | @default(false) |
| expires_at | DateTime? | @db.Timestamptz(6) |
| created_by_uid | String | @db.VarChar(255) |
| created_by_name | String? | @db.VarChar(255) |
| created_at | DateTime | @default(now()) @db.Timestamptz(6) |
| updated_at | DateTime | @default(now()) @updatedAt @db.Timestamptz(6) |

**Indexes:**
- @@index([visible_to, pinned, created_at(sort: Desc)])
- @@index([expires_at])

**Relations:** None

---

## Notes

- **Enums:** None.
- **Custom types:** None.
- All UUID PKs use `@db.Uuid` (gen_random_uuid()).
- Timestamps `Timestamptz(6)` or `Date` for date-only fields.
- Decimal money standardized to `Decimal(10, 2)`.
- Indexing on FKs, status fields, and `created_at DESC` is common.
- Cascading deletes on most child relations; SetNull on optional product references.
- No enums: statuses are VarChar strings.

## Mapping cheatsheet (Postgres → SurrealDB)

| Postgres | SurrealDB |
|---|---|
| `Uuid` PK (`gen_random_uuid()`) | `id` autogenerated by `CREATE`; reference as `record<table>` |
| `VarChar(N)` | `string` (no length enforce; validate in domain) |
| `Text` | `string` |
| `Decimal(10,2)` | `decimal` |
| `Int` | `int` |
| `Boolean` | `bool` |
| `Json` | `object` |
| `Timestamptz(6)` | `datetime` (`time::now()` default) |
| `Date` | `datetime` (truncate to date in service) — or `string` ISO date if pure date storage |
| `@@index([a,b])` | `DEFINE INDEX ... ON TABLE x FIELDS a, b` |
| `@@unique([a,b])` | `DEFINE INDEX ... ON TABLE x FIELDS a, b UNIQUE` |
| FK `@relation(fields:[x],references:[id])` | `record<other>` + index on field |
| `onDelete: Cascade` | manual cascade in service OR `RELATE` graph + cleanup |
| `onDelete: SetNull` | manual nullify in service |

## Multi-tenant overlay (pharma-server-specific)

Every domain table gains `tenant: record<tenant>` (NOT NULL) + composite index `(tenant, <natural-key>)`. Exceptions:

- `tenant`, `user`, `session` — root tables (pre-existing).
- `barcode_catalog` — global Chile catalog (no `tenant`).
- `therapeutic_category_mapping` — global lookup (no `tenant`).

Cascade behaviors in Tu Farmacia are emulated at service layer (SurrealDB has no FK constraints).
