# Respaldo cifrado con llave del usuario (feria / ADR-0022)

Estado: **contrato v1 + cifrado cliente + upload/restore cableados** (2026-08-08).
PBKDF2-HMAC-SHA256 + AES-256-GCM reales en Android; `POST /api/v1/user-backup`
sube ciphertext (server `accepted:false` sin bucket). Restore local por frase
+ sobre base64. Bucket y Argon2id siguen pendientes.

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
| Empaquetar snapshot offline | Cliente | `SnapshotBackupV1` JSON (`snapshot_version=1`): ventas en cola + secciones |
| KDF | Cliente | **PBKDF2-HMAC-SHA256** 210k iter → 32 B (hoy). Argon2id = objetivo futuro |
| Cifrar | Cliente | **AES-256-GCM real** (`CifrarSobre.kt` + `CryptoPlataforma` Android) |
| Sobre wire | Cliente | `RB1\n` + header JSON + ciphertext\|\|tag |
| Subir | Cliente → `POST /api/v1/user-backup` | `UserBackupApi.subirSobre` (meta + base64) |
| Validar | Server | sha256 + size + format_version (puro) |
| Guardar | Server | Bucket prod pendiente. **Lab:** `RUTBUSINESS_USER_BACKUP_MEMORY=1` guarda en RAM y `accepted: true` |
| Listar / bajar | `GET /api/v1/user-backup` y `GET .../{id}` | Metas + ciphertext; sin frase |
| Restore local | Cliente | `restaurarDesdeSobre` + rehidrata cola; UI "Abrir un respaldo" |

Parámetros en `domain::user_backup`, `SobreCifrado.kt`, `CifrarSobre.kt`, `SnapshotBackup.kt`.

**Snapshot plaintext (antes de AEAD):** `pending_sales` (cola offline) es la
sección day-1. Fiado/catálogo entran como `sections` opacas en v1.1.

## Flujo restore

**Hoy (local):** en la cola offline → "Abrir un respaldo" con llave + base64
del sobre (prefill del último preparado). `restaurarDesdeSobre` re-deriva
con salt del header y abre el snapshot. **Aún no** rehidrata la cola de ventas.

**Siguiente:**

1. Login Google / identidad del tenant.
2. Descargar blob listado (`GET /api/v1/user-backup`).
3. Rehidratar `pending_sales` en `ColaDeVentas`.
4. Fallo de frase = error claro, sin "¿olvidaste tu clave?".

## Código ancla

| Pieza | Path |
|-------|------|
| ADR | `docs/adr/0022-feria-agent-first-identity-backup.md` |
| Domain shapes + validate | `crates/domain/src/user_backup.rs` |
| API stub | `crates/api/src/v1/user_backup.rs` → `POST/GET /api/v1/user-backup` |
| Android clave UI | `client-android/core/.../backup/ClaveDelNegocio.kt` |
| Android sobre v1 | `client-android/core/.../backup/SobreCifrado.kt` |
| Android AES-GCM + PBKDF2 | `CifrarSobre.kt` + `CryptoPlataforma` + `derivarClaveDeMaterial` |
| Android snapshot v1 | `client-android/core/.../backup/SnapshotBackup.kt` |
| Preparar / restaurar | `PrepararRespaldo.kt`, `RestaurarRespaldo.kt` |
| Cliente upload | `UserBackupApi.kt` + wire en `ContenedorDeDestinos` / `PantallaDeCola` |
| Material recovery parse | `client-android/core/.../backup/MaterialRecuperacion.kt` |
| Pantalla rescate + payload QR + texto página | `client-android/app/.../entrada/TarjetaRescate.kt` |
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
