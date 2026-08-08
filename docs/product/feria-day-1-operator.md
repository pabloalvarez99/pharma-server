# Feria day-1 - qué ve el feriante

Estado: alineado con código en main (Google stubs + copy feria, 2026-08-08).
ADR: [0022](../adr/0022-feria-agent-first-identity-backup.md).

## Camino feliz (Android)

1. **Primer uso** - 3 pantallas: qué es, dónde se guarda (offline + llave), qué traer.
2. **Elegí rubro** - Feria/Calle primero (recomendado). Se guarda en el teléfono.
3. **Login** - card **Con tu cuenta de Google** (stub: copy feria, botón
   "Continuar con Google" explica que aún no está cableado) + dirección del
   computador + negocio + correo/clave (camino real hoy).
4. **Tarjeta de rescate** (solo si eligió feria) - 12 palabras / 8 bloques.
   "Ya la anoté en el cuaderno" o seguir (no recomendado).
5. **Home = Agente** - chips:
   - «Vendí 2 kg de tomates»
   - «Don Juan debe 5000»
   - «¿Cuánto vendí hoy?»
   - «Fiado 1 atado de cilantro a doña Ana»
   - «¿Quién me debe plata?»
6. **Pestañas** (copy feria): **Agente** | **Vender** | **Hoy**
   - Vender = buscar por nombre (sin escáner, sin reimprimir térmica).
   - Hoy = vendido + **quién te debe** (fiado). Sin caja FEFO.

## Qué no se muestra day-1 en feria

| Superficie | Flag pack | Day-1 feria |
|------------|-----------|-------------|
| Escáner cámara | `barcode` | oculto |
| Reimprimir boleta | `printer` | oculto |
| DTE / SII | `dte` | oculto (no hay UI Android densa aún) |
| Caja / vencimientos en Hoy | - | ocultos en resumen feria |

El código **sigue** en el binario para farmacia y para un futuro "modo formal".

## Server

- `business.vertical = feria` → `GET /api/v1/rubro-pack` con `agent_home=true`, etc.
- Si el tenant no tenía vertical, el primer login Android con preferencia local
  hace `PUT /api/v1/settings/business.vertical`.
- Assist: «2 kg de tomates» → qty 2, producto `tomates` (limpia kg/atado/granel).

## Vara del cuaderno

Si anotar una venta o ver una deuda tarda más que el lápiz, es un bug de producto.

## Google Sign-In (stubs)

| Capa | Qué hay | Qué falta |
|------|---------|-----------|
| Android core | `IdentidadGoogle` + `IdentidadGoogleNoCableada` | Credential Manager + client id en `local.properties` (gitignored) |
| Android UI | Card Google en login; copy feria distinto | Exchange real de `id_token` → sesión |
| Domain | `google_identity::{GoogleSignInRequest,Response}` path `/api/v1/auth/google` | Ruta API + verify JWKS |

**Regla 3:** no hay client id ni secret en el repo ni en el vault.

## Respaldo cifrado (estado en main)

- **Preparar** en cola offline: frase del cuaderno → PBKDF2 + AES-GCM → sobre local
  + intento de `POST /api/v1/user-backup` (ciphertext only).
- **Lab store:** `RUTBUSINESS_USER_BACKUP_MEMORY=1` en el server → `accepted: true`
  en RAM (no durable). Sin env = `accepted: false` hasta bucket.
- **Abrir respaldo**: frase + base64 → rehidrata `pending_sales` en la cola.
- **Sin** recuperación de clave por soporte. Bucket R2/S3 y Argon2id: pendientes.

## Fuera de scope aún

- Dibujo QR (ZXing) / PDF de la tarjeta de rescate
- Bucket real (ops del capitán)
- Google OAuth end-to-end (consola + secrets del capitán)
- Seed demo feria corrida en lab (código seed sí)
