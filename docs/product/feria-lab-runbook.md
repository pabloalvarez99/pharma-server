# Feria lab runbook (sin secretos)

Cómo probar day-1 feria en un nodo local **sin** bucket R2 ni Google Cloud
console. Para el capitán o un agente en la máquina de lab.

## 1. Server

```powershell
cd C:\dev\firstmate-home\projects\rutbusiness\pharma-server
$env:RUTBUSINESS_USER_BACKUP_MEMORY = "1"
# arrancar el binario del server como de costumbre (cargo run / scripts lab)
```

- Con `RUTBUSINESS_USER_BACKUP_MEMORY=1`, `POST /api/v1/user-backup` puede
  devolver `accepted: true` y guardar el sobre **en RAM del proceso**.
- Sin la env: `accepted: false` (contrato honesto; no hay bucket).
- `GET /api/v1/user-backup` lista metas con `backup_id`.
- `GET /api/v1/user-backup/{id}` baja ciphertext.
- `POST /api/v1/auth/google` → **501** hasta OAuth ops.

## 2. Seed vertical feria

Usar el seed del vertical `feria` (verdura/fruta, ≥3 suppliers, sin clínica).
Ver `domain::seed::SeedVertical::Feria`.

## 3. Android

1. Login correo/clave (Google UI existe; OAuth no hasta client id en
   `local.properties` gitignored).
2. Elegir rubro **Feria/Calle** en primer uso.
3. Anotar tarjeta de rescate (CSPRNG; huella visual no es QR escaneable).
4. Cobrar offline → franja → **Preparar respaldo** con la llave del cuaderno.
5. Con lab memory: mensaje de subida `accepted` + id.
6. **Traer de la nube** + **Abrir respaldo** rehidrata `pending_sales` a la cola.

## 4. Qué no hacer

- No pegar `ADMIN_KEY`, OAuth client secrets ni JWT en el vault (Regla 3).
- No force-push.
- No tocar worktrees `compose-*` dirty.

## Anclas

- ADR-0022, `docs/product/feria-backup-zero-knowledge.md`
- `feria-day-1-operator.md`
