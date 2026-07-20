# Publicar mi web en 15 minutos (Free Web)

Runbook del operador para encender el storefront integrado (ADR-0020).
Cada paso es un comando real verificado contra un server local sembrado.
Prepara una vez en tu PowerShell (ajusta origen y credenciales):

```powershell
$O = "http://127.0.0.1:8080"   # origen de tu RutBusiness Server
$login = Invoke-RestMethod "$O/api/v1/login" -Method Post -ContentType "application/json" -Body '{"tenant":"demo","email":"admin@demo.cl","password":"***"}'
$h = @{Authorization="Bearer $($login.token)"}
```

## 1. Marcar productos visibles en la web

Opt-in por SKU (`online_visible`). Solo salen productos activos, venta directa.
Opcional: título/precio/descripción/orden solo-web (`online_title`, `online_price`,
`online_description`, `online_sort`) sin tocar los campos del POS.

```powershell
Invoke-RestMethod "$O/api/v1/products/product:XXXX" -Method Patch -Headers $h -ContentType "application/json" -Body '{"online_visible":true}'
Invoke-RestMethod "$O/api/v1/products/product:XXXX" -Method Patch -Headers $h -ContentType "application/json" -Body '{"online_title":"Oferta web","online_price":"1490"}'
```

Los ids salen de `Invoke-RestMethod "$O/api/v1/products" -Headers $h`.

## 2. Datos de la tienda

```powershell
Invoke-RestMethod "$O/api/v1/settings/web.store_name" -Method Put -Headers $h -ContentType "application/json" -Body '{"value":"Farmacia Demo"}'
```

Misma forma para: `web.whatsapp_e164` (`+56912345678`), `web.hours_label`
(`Lun-Sab 9:00-20:00`), `web.address_line`, `web.pickup_instructions`.

## 3. Credencial del storefront (guardar UNA vez)

```powershell
$cred = Invoke-RestMethod "$O/api/v1/admin/web/keys" -Method Post -Headers $h -ContentType "application/json" -Body '{"name":"storefront"}'
$cred | ConvertTo-Json   # key (rb_live_…) + hmac_secret (whsec_…) — NO se pueden recuperar después
```

Perdiste el secreto → `POST /api/v1/admin/web/keys/{id}/rotate` (la clave vieja muere).

## 4. Publicar

```powershell
Invoke-RestMethod "$O/api/v1/settings/web.published" -Method Put -Headers $h -ContentType "application/json" -Body '{"value":"true"}'
```

## 5. Probar el seam completo

```powershell
$env:ERP_ORIGIN = $O; $env:RB_SLUG = "demo"
node scripts/web-sync/pull-catalog.mjs          # catálogo público + catalog.json
$env:RB_API_KEY = $cred.key; $env:RB_HMAC_SECRET = $cred.hmac_secret
node scripts/web-sync/push-order.mjs --product product:XXXX --qty 2 --replay
```

`--replay` reenvía la misma Idempotency-Key y debe devolver el MISMO `order_id`.

## 6. Atender pedidos web

```powershell
Invoke-RestMethod "$O/api/v1/orders?channel=web" -Headers $h                    # llegan status=reserved + pickup_code RET-XXXX
Invoke-RestMethod "$O/api/v1/admin/orders/order:XXXX/transition" -Method Post -Headers $h -ContentType "application/json" -Body '{"to":"preparing"}'
```

Ciclo: `reserved` → `preparing` → `ready_for_pickup` → cliente retira con su
código → `completed` (descuenta stock). `{"to":"cancelled"}` libera la reserva
sin tocar stock físico. El pedido expira solo si nadie lo atiende (`expires_at`).

## 7. Apagar la web

```powershell
Invoke-RestMethod "$O/api/v1/settings/web.published" -Method Put -Headers $h -ContentType "application/json" -Body '{"value":"false"}'
```

Todo `/api/v1/public/{slug}/…` vuelve a 404 al instante. Tus datos y pedidos quedan intactos.
