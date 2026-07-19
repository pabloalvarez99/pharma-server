# ADR-0015: Cliente universal cross-platform (Tauri 2 + PWA) sobre API-first

- **Status**: Accepted
- **Date**: 2026-06-14
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, cliente, cross-platform, arquitectura

## Context and Problem Statement

El cliente operador de pharma-server hoy es **Tauri 2** desktop (Windows MSI; el
frontend es TS vanilla + Vite, 18 vistas en `client/src/views/`). El fundador quiere
que el producto se use **nativamente en Android, iOS, web y de forma universal**.

Dos hechos habilitan esto casi gratis:
1. El cliente YA es **Tauri 2** (`@tauri-apps/api ^2`, `tauri = "2"`, `bundle.targets:
   all`). Tauri 2 compila el MISMO frontend a **desktop + Android + iOS**.
2. La UI está **desacoplada de la API** ([CLAUDE.md](../../CLAUDE.md): "API HTTP/JSON
   estable y versionada `/api/v1`; el frontend es cliente separado"). Cualquier número
   de clientes puede existir sin tocar el server.

El problema no es sólo "qué framework" — es la **topología**: en desktop el server
corre en la misma máquina (localhost, offline). En móvil/web el dispositivo NO es el
server: es un terminal en red que apunta a un pharma-server en la LAN del local
(tablet-caja), o por internet (Cloudflare Tunnel / cloud companion). Hay que decidir
forma del cliente Y dónde vive el dato sin romper offline-first.

## Decision Drivers

- **Reuso máximo**: no rebotear el frontend ya construido (18 vistas, `api.ts`,
  `format.ts`, tests). Churn bajo.
- **Offline-first del CORE intacto** ([ADR-0005](./0005-core-gratis-no-locked-in.md)):
  el server on-prem sigue siendo la verdad; el cliente móvil/web no puede degradar eso.
- **Valor inmediato barato**: una tablet Android en el WiFi del local como **caja
  móvil** contra el PC mostrador = feature enorme, casi sin código nuevo.
- **Costo $0 primero** (ethos freemium): PWA no necesita app store; las stores
  (Play $25, Apple $99/año) se pagan cuando hay demanda.
- **No confundir clientes**: app del **operador** (POS/admin) ≠ **storefront del
  cliente final** (DSS web, [ADR-0014](./0014-dss-storefront-integration.md)). Ambos
  consumen la misma API; son apps distintas.

## Considered Options

1. **Tauri 2 universal (desktop+Android+iOS) + build PWA/web del MISMO frontend.**
2. **Capacitor/Ionic** (reescribir el shell; reusar vistas TS).
3. **Flutter / React Native** (reescritura total de UI).
4. **Solo PWA** (sin app nativa; APIs nativas limitadas: scanner/impresora/offline).

## Decision Outcome

**Elegida: Opción 1 — Tauri 2 como shell universal + PWA del mismo frontend.** Un
frontend TS, múltiples targets:
- **Desktop** (Windows/macOS/Linux) — ya existe (MSI).
- **Android / iOS** — `tauri android`/`tauri ios` sobre el mismo `client/src`.
- **Web / PWA** — `vite build` del mismo frontend, instalable, sin app store.

Las opciones 2/3 botan la inversión actual; la 4 sola no da scanner/impresora/offline
nativos que un POS necesita. Tauri 2 da las tres superficies desde un código y deja la
puerta abierta a plugins nativos.

### Topología (clave)

El cliente apunta a una **URL de server configurable** (nuevo en onboarding/config):
`localhost` (desktop co-instalado) · `IP LAN` (tablet en el WiFi del local) ·
`tunnel`/`cloud` (acceso remoto, dueño desde casa). 

- **Server = verdad offline-first**, corre en el PC mostrador o nodo del local.
- **Móvil/web = terminales en red.** MVP: requieren alcanzar el server (LAN primero).
  El "cliente offline con cola local + sync" es más duro (conflictos) → **Fase posterior**,
  no MVP. No prometer offline-en-móvil todavía.

```
                 pharma-server (on-prem, offline-first, /api/v1)   ← VERDAD
                    ▲           ▲              ▲
        localhost   │   LAN/WiFi│       tunnel/cloud
   ┌────────────────┴───┐  ┌────┴─────┐  ┌────┴───────────────┐
   │ Tauri desktop (MSI)│  │Tauri móvil│  │ PWA/web (universal) │   ← app OPERADOR
   └────────────────────┘  │Android/iOS│  └─────────────────────┘
                           └──────────┘
   (cliente final usa el STOREFRONT DSS, ADR-0014 — app distinta)
```

### Plan de ejecución (orden self-funded)

**P0 — URL de server configurable (desbloquea caja móvil HOY, casi sin código):**
`api.ts` toma base URL de config (no hardcode localhost); vista de Configuración
permite setear IP LAN del server; persistir. → una tablet Android (build P1) o
incluso el desktop apuntando a otra máquina ya sirve de terminal. Lane: **ye**
(config/onboarding) + **bob** (api.ts base-url, append-only).

**P1 — Android (mercado CL = mayoría Android):** `tauri android init` + build;
probar una tablet como caja en el WiFi del local. Ajustes responsive de las vistas
POS para touch. Lane: cross-platform (nueva, asignar a un worker libre).

**P2 — PWA/web:** manifest + service worker básico (shell cacheable, datos siempre
en red por ahora); deploy estático (Vercel/CF, como DSS). Universal sin store, $0.
Sirve dashboard del dueño vía tunnel.

**P3 — iOS:** `tauri ios` (requiere Apple Developer $99/año → cuando haya demanda).

**P4 — Plugins nativos + offline-cliente:** scanner por cámara, impresora BT/USB,
cajón de dinero (plugins Tauri por plataforma); luego cola offline + sync en cliente
(Fase 15, junto al agente del RUT en móvil).

### Invariantes
- **API-first**: todo cliente habla `/api/v1`; cero lógica de negocio en el cliente
  que no esté respaldada por el server. Multi-tenant por JWT igual en todas las apps.
- **Offline-first del server** no se toca; el cliente móvil/web es terminal en red
  (MVP). El offline-cliente es aditivo y posterior, nunca un requisito del core.
- **Un frontend**: Android/iOS/web/desktop salen del MISMO `client/src` (Tauri 2 +
  PWA). Nada de forks de UI por plataforma.
- **Freemium**: terminales adicionales / acceso remoto pueden ser palancas de tier
  (seats, sync online) sin quitar nada al Free.

## Consequences

### Positivas
- Caja móvil (tablet) y dashboard web del dueño con reuso casi total → ROI inmediato.
- Una sola base de UI → mantener N plataformas cuesta ~1.
- Encaja con RutAgentIA: el cliente es sólo "una ventana"; el valor está en el
  server/agente. "Universal" es la recompensa del diseño API-first.

### Negativas / riesgos
- Móvil/web sin server alcanzable = sin función (MVP). Mitigación: doc de topología
  (LAN/tunnel) + URL configurable + mensaje claro de "sin conexión al servidor".
- Stores cuestan (iOS $99/año). Mitigación: PWA primero ($0); stores cuando haya pull.
- Tentación de offline-cliente prematuro (complejo). Mitigación: P4/Fase 15 explícito.

### Neutras
- DSS (ADR-0014) cubre el frontend del CLIENTE FINAL (storefront); este ADR cubre el
  frontend del OPERADOR. Complementarios, no solapados.

## More Information
- Cliente actual: `client/` (Tauri 2). [ADR-0014](./0014-dss-storefront-integration.md)
  (storefront cliente final), [ADR-0012](./0012-web-onprem-interop.md) (seam HTTP),
  [ADR-0005](./0005-core-gratis-no-locked-in.md) (offline-first/freemium).
- Visión: [`docs/strategy/rutagentia-vision.md`](../strategy/rutagentia-vision.md).
- Tauri 2 mobile: `tauri android`/`tauri ios`. PWA: vite manifest + SW.
