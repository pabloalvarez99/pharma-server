---
title: pharma-server — Documentación de producto
status: Activo
owners: pabloalvarez99
last_review: 2026-05-20
---

# Documentación de producto

Contratos de dominio, modelos de datos, paridad con sistemas externos. **Audiencia**: clientes,
partners técnicos, integradores. Todo lo que aquí vive debería poderse compartir externamente
con mínima edición.

## Contenido

| Documento | Propósito |
|---|---|
| [`parity-prisma-models.md`](./parity-prisma-models.md) | Mapeo modelos Prisma (Tu Farmacia legado Next.js) ↔ tablas SurrealDB pharma-server. Garantiza paridad de dominio. |
| [`erp-parity-prompt.md`](./erp-parity-prompt.md) | Prompt template usado para auditorías de paridad ERP. |

## Convenciones

- **Versionado de contratos**: cualquier cambio breaking en payloads `/api/v1/*` requiere bump a `/api/v2/*`. No editar respuestas v1 después de release.
- **Nombres en inglés** para columnas, IDs, enums internos. **Mensajes user-facing en español**.
- **Multi-tenant obligatorio**: toda tabla de dominio tiene `tenant: record<tenant>` + índice compuesto liderado por tenant.

## Próximos documentos planeados

- `feature-keys-catalog.md` — catálogo canónico de claves `module.feature` (entitlements freemium).
- `api-stability-matrix.md` — qué endpoints están estables, beta, deprecados.
- `data-export-formats.md` — esquemas CSV/JSON de export (compromiso "sin lock-in").
