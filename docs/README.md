---
title: pharma-server — Documentación
status: Activo
owners: pabloalvarez99
last_review: 2026-05-20
---

# pharma-server — Documentación

Índice maestro de toda la documentación del proyecto. Tres pistas separadas a propósito:

| Pista | Audiencia | Contenido | Ruta |
|---|---|---|---|
| **product/** | Clientes, partners técnicos, integradores | Modelos de dominio, contratos públicos, parity con sistemas externos | [`product/`](./product/README.md) |
| **strategy/** | Fundador, board, design partners | Modelo de negocio, arquitectura objetivo, decisiones lockeadas | [`strategy/`](./strategy/README.md) |
| **adr/** | Equipo de ingeniería | Architecture Decision Records (MADR) — una decisión por archivo | [`adr/`](./adr/README.md) |

## Reglas de la documentación

1. **Decisiones van en ADRs**, no en wikis ni en commit messages. Si una decisión amerita discusión, amerita ADR.
2. **`strategy/` es lockeado** — cualquier cambio requiere ADR nuevo o `Status: Superseded by ADR-NNNN`.
3. **`product/` es público-eventual** — escribir como si lo fuera a leer un partner. Sin chistes internos.
4. **Diagramas en Mermaid** — embebidos en el `.md`, no PNG. Versionables, diffables.
5. **Última revisión** en frontmatter (`last_review`). Si pasa >180 días sin revisión, marcar `status: Stale`.

## Estado actual (2026-05-20)

- **Versión release**: `v0.1.23` (MSI publicado).
- **Pivote estratégico activo**: ERP licencia única → **MSI freemium** (ver [`strategy/freemium-master-plan.md`](./strategy/freemium-master-plan.md)).
- **Próximo hito de código**: `crates/license` (Fase 10 — ver `bitacora.md` BACKLOG).

## Navegación rápida

- ¿Por qué existe el producto? → [`strategy/freemium-master-plan.md`](./strategy/freemium-master-plan.md) §1-2
- ¿Cómo funciona el licenciamiento? → [`strategy/license-architecture.md`](./strategy/license-architecture.md)
- ¿Cómo cobramos? → [`strategy/payments-cl.md`](./strategy/payments-cl.md)
- ¿Cómo escala? → [`strategy/scaling-architecture.md`](./strategy/scaling-architecture.md)
- ¿Qué se decidió y por qué? → [`adr/`](./adr/README.md)
- ¿Modelos de dominio? → [`product/parity-prisma-models.md`](./product/parity-prisma-models.md)
