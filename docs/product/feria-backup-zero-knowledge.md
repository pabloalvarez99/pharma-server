# Respaldo cifrado con llave del usuario (feria / ADR-0022)

Estado: **diseño + stubs de código** (2026-08-08). No hay upload real ni
cifrado en producción todavía.

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

## Flujo backup (futuro)

| Paso | Quién | Qué |
|------|-------|-----|
| Empaquetar snapshot offline | Cliente | Ventas, fiado, catálogo local |
| KDF | Cliente | Argon2id(phrase) → key |
| Cifrar | Cliente | AES-GCM, `format_version=1` |
| Subir | Cliente → API | Solo `EncryptedBackupMeta` + bytes |
| Guardar | Server | Blob opaco en bucket, metadatos |

## Flujo restore (futuro)

1. Login Google / identidad del tenant.
2. Usuario ingresa frase o bloques de la tarjeta.
3. Cliente descarga blob, descifra, rehidrata Surreal/local store.
4. Fallo de frase = error claro, sin "¿olvidaste tu clave?".

## Código ancla

| Pieza | Path |
|-------|------|
| ADR | `docs/adr/0022-feria-agent-first-identity-backup.md` |
| Domain shapes | `crates/domain/src/user_backup.rs` |
| Android clave UI | `client-android/core/.../backup/ClaveDelNegocio.kt` |
| Pantalla rescate | `client-android/app/.../entrada/TarjetaRescate.kt` |
| Admin backup legacy | `BackupApi` / `POST /api/v1/admin/backup` = **otro** (dump servidor, admin+) |

El backup admin del ERP (tar.gz del data dir) **no** es el de feria. No
mezclar: uno es ops del operador formal; el otro es continuidad del
feriante con llave propia.

## No hacer

- Guardar la frase en el server "por un rato".
- Guardar la frase en el vault / Notion / logs (Regla 3).
- Ofrecer "reset de clave" sin material de recuperación.
- Subir plaintext "y ciframos en el server".
