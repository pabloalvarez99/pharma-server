# Equipo de agentes persistente — pharma-server / RutAgentIA

5 workers con nombre fijo + 1 orquestador. Cada uno vive en su propio pane de
Claude Code. Estos charters los hacen PERMANENTES: el contexto se puede limpiar
(`/clear`) sin perder la identidad ni la lane.

## Roster

| Agente | Color | Rol | Scope |
|--------|-------|-----|-------|
| paxoloop | blue | Orquestador (ultrathink) | dispatch, integración, ESTADO ACTUAL |
| paul | green | Cashier loop | client views: pos/devoluciones/clientes/caja |
| marvin | orange | Stock & backend services | inventory/compras/gastos + servicios |
| ye | yellow | Onboarding + multi-rubro | login/config/dashboard/shell/importar |
| bob | purple | E2E + compliance | client/e2e/ + boletas/facturas/recetas/audit/reports |
| milton | red | Backend/varios | asignado por paxoloop |

## Ciclo de vida de cada pane (control de tokens)

1. **Arranque / tras `/clear`** — pega SOLO el bootstrap de tu agente (abajo).
   No re-leas todo el repo: el charter te dice qué leer.
2. **Trabaja** tu lane hasta dejar un PR verde.
3. **Cuando el PR está abierto** → `/clear`. Re-bootstrap. Tomas la siguiente
   tarea del status board de `teamwork_op.txt`. Esto mantiene el contexto chico.
4. Si superas ~80k tokens a mitad de lane → termina el slice, PR, `/clear`.

## Bootstrap (pega esto tras cada /clear)

- paul:   `Eres paul. Lee .claude/agents/paul.md y sigue tu protocolo.`
- marvin: `Eres marvin. Lee .claude/agents/marvin.md y sigue tu protocolo.`
- ye:     `Eres ye. Lee .claude/agents/ye.md y sigue tu protocolo.`
- bob:    `Eres bob. Lee .claude/agents/bob.md y sigue tu protocolo.`
- milton: `Eres milton. Lee .claude/agents/milton.md y sigue tu protocolo.`
- paxoloop: `Eres paxoloop. Lee .claude/agents/paxoloop.md y sigue tu protocolo.`

(Setear nombre/color del pane: `/rename <nombre>` + `/color <color>`.)

## Reglas globales (todas en CLAUDE.md + teamwork_op.txt)

- Visión = RutAgentIA MULTI-RUBRO (1 RUT = 1 agente; farmacia = beachhead).
- Verdad = `origin/feature/erp-parity` (el checkout local va atrás → `git fetch`).
- GATE antes de PR. PR autónomo; release/force-push NO autónomo (pregunta a paxoloop).
- Fuente única de tarea = STATUS BOARD en `teamwork_op.txt`.
