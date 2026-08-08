# Respaldo cifrado con llave del usuario (feria / ADR-0022)

Estado: **contrato v1 + API stub** (2026-08-08). Validación de upload y
payload QR listos; bucket y Argon2id/AES-GCM en cliente aún no cableados.

## Objetivo

El feriante pierde el teléfono y no pierde el historial **si** tiene:

1. Su cuenta (Google: UI + contrato stub; OAuth real pendiente; hoy email/clave).
2. La **llave del negocio** escrita en el cuaderno (tarjeta de rescate).

RutBusiness guarda **solo ciphertext** en bucket propio. Sin la llave del
usuario el blob es basura. **No hay recuperación de clave por soporte.**

## Flujo day-1

1. Alta / primer login genera material de recuperación en el cliente
   (CSPRNG → 12 palabras es-CL **o** 8 bloques de 4 caracteres).
2. Pantalla a pantalla completa: frase + aviso "escribila en el cuaderno".
3. Stub UI: `TarjetaRescate` (Android) + `domain::user_backup` (shapes).
4. Más adelante: PDF de una página con QR + palabras.

## Flujo backup (v1)

| Paso | Quién | Qué |
|------|-------|-----|
| Empaquetar snapshot offline | Cliente | Ventas, fiado, catálogo local |
| KDF | Cliente | Argon2id: m=65536 KiB, t=3, p=1 → 32 B |
| Cifrar | Cliente | AES-256-GCM, `format_version=1` |
| Subir | Cliente → `POST /api/v1/user-backup` | `UploadEncryptedBackupRequest` (meta + base64) |
| Validar | Server | sha256 + size + format_version (puro) |
| Guardar | Server | Blob opaco en bucket (**stub:** `accepted: false` hasta bucket) |

Parámetros congelados en `domain::user_backup` y `SobreCifrado.kt`.

## Flujo restore (futuro)

1. Login Google / identidad del tenant.
2. Usuario ingresa frase o bloques de la tarjeta.
3. Cliente descarga blob, descifra, rehidrata Surreal/local store.
4. Fallo de frase = error claro, sin "¿olvidaste tu clave?".

## Código ancla

| Pieza | Path |
|-------|------|
| ADR | `docs/adr/0022-feria-agent-first-identity-backup.md` |
| Domain shapes + validate | `crates/domain/src/user_backup.rs` |
| API stub | `crates/api/src/v1/user_backup.rs` → `POST/GET /api/v1/user-backup` |
| Android clave UI | `client-android/core/.../backup/ClaveDelNegocio.kt` |
| Android sobre v1 | `client-android/core/.../backup/SobreCifrado.kt` |
| Pantalla rescate + payload QR | `client-android/app/.../entrada/TarjetaRescate.kt` |
| QR payload | `rutbusiness-rescue:v1:<tenant>:<8 bloques>` |
| Admin backup legacy | `POST /api/v1/admin/backup` = **otro** (dump servidor, admin+) |

El backup admin del ERP (tar.gz del data dir) **no** es el de feria. No
mezclar: uno es ops del operador formal; el otro es continuidad del
feriante con llave propia.

## No hacer

- Guardar la frase en el server "por un rato".
- Guardar la frase en el vault / Notion / logs (Regla 3).
- Ofrecer "reset de clave" sin material de recuperación.
- Subir plaintext "y ciframos en el server".
