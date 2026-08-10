# ADR-0021: Cliente Android nativo en Jetpack Compose

- **Status**: Accepted
- **Date**: 2026-08-06
- **Deciders**: pabloalvarez99 (founder)
- **Tags**: producto, cliente, android, ux, cross-platform
- **Supersede**: [ADR-0015](./0015-universal-cross-platform-client.md) en lo que respecta a Android

## Contexto

[ADR-0015](./0015-universal-cross-platform-client.md) decidió un frontend único
(TS + Vite) compilado a cuatro superficies vía Tauri 2 y PWA, con la invariante
explícita *"nada de forks de UI por plataforma"*. Esa decisión se ejecutó: el
2026-07-25 se commiteó el scaffolding Android/iOS y el 2026-08-06 se cortó el
primer APK funcionando (18,3 MB, login real contra el server, POS con datos
reales).

Con la app en la mano en un teléfono aparecieron dos cosas que la decisión de
2026-06-14 no podía anticipar, porque no había app que mirar:

**1. La UI es escritorio metido a la fuerza.** El sidebar se come media pantalla,
el header pisa la barra de estado, la columna derecha queda en ~40 caracteres.
Auditoría posterior: **cero ocurrencias de `safe-area-inset` o `viewport-fit` en
todo el frontend**, y **cero virtualización** en las 37 vistas. Parte de esto es
layout mal hecho y se arregla sin cambiar de toolkit; parte no.

**2. El founder fijó un piso de hardware** (2026-08-06, ver `CLAUDE.md`): el
usuario real puede ser **una persona mayor con un celular viejo, lento y sin
espacio**. Es la máquina objetivo, no un caso borde.

El founder priorizó explícitamente **experiencia fluida e interactiva** por sobre
la simplicidad de mantener un solo frontend.

### Medición preliminar del WebView

Medido en emulador Android 34 x86_64 sobre un PC de escritorio, animaciones
apagadas (bisect, 3 repeticiones por punto):

| Umbral | Login pintado |
|---|---|
| 1.600 ms | 0 de 3 |
| 2.000 ms | 1 de 3 |
| 2.200 ms | 2 de 3 |
| 2.500 ms | **3 de 3** |

`am start -W` da TotalTime de 959 a 1.536 ms al primer frame, pero el usuario ve
**pantalla en blanco ~2,5 s** hasta que el login aparece. Eso es en hardware de
escritorio; en el aparato de referencia del piso de hardware el número es varias
veces peor.

> Cifras preliminares tomadas del carril de medición antes de su reporte formal.
> Sirven como línea base para verificar que la reescritura mejoró algo.

## Decision Drivers

- **Fluidez e interactividad** como prioridad declarada del founder.
- **Piso de hardware**: teléfonos viejos y lentos, usuarios mayores.
- **Accesibilidad del sistema**: la persona mayor ya subió el tamaño de letra en
  Ajustes de Android. La app debe obedecer esa preferencia sin trabajo extra.
- **Alcance de aparatos**: llegar a Android viejo, no solo a teléfonos recientes.
- **No tirar el core**: la lógica de negocio en Rust no se toca.
- **No cerrar la puerta a iOS**: sigue siendo objetivo de primera clase.

## Opciones consideradas

1. **Quedarse en WebView** (Tauri 2) y pulir móvil.
2. **Jetpack Compose** para Android.
3. **Compose Multiplatform** para Android + iOS.
4. **Flutter** para Android + iOS.

## Decisión

**Jetpack Compose para Android**, escrito con disciplina de Compose Multiplatform
desde la primera línea, hablando HTTP contra el server.

### Por qué no Flutter

Bajo el piso de hardware, Flutter pierde en lo que más importa:

| | Compose | Flutter |
|---|---|---|
| Motor de render | el del sistema, optimizado por el fabricante para ese chip | **trae el suyo**; en GPU viejas Impeller es lo menos maduro y el jank de compilación de shaders en gama baja fue su problema histórico |
| Peso extra en el APK | ~2-4 MB de runtime, y R8 lo achica | ~7-8 MB de motor antes de tu código |
| Escala de letra del sistema | **la respeta sola** | dibuja su propio texto; hay que programarlo, y el olvido es silencioso |
| TalkBack, alto contraste | widgets reales, accesibilidad del sistema gratis | capa propia |
| Lenguaje | Kotlin, ya presente en el proyecto | Dart, nuevo |

Lo que Flutter gana -render idéntico entre fabricantes, 120 Hz- no aplica: los
aparatos objetivo son de 60 Hz y el problema es potencia bruta, no consistencia.

### Por qué no quedarse en WebView

Aparte de los 2,5 s de pantalla en blanco: **la app Tauri es `minSdk 24`**
(Android 7.0, 2016), y Compose baja a **`minSdk 21`** (Android 5.0, 2014).
Contra la intuición, **cambiar a Compose amplía el alcance de aparatos**, no lo
achica. Justo los aparatos del piso de hardware.

### Transporte: HTTP, no JNI

La app Compose habla **HTTP/JSON** contra `pharma-server`, igual que la PWA.

Dos hechos lo hacen barato:

- **139 endpoints documentados en OpenAPI** (`utoipa`). El cliente Kotlin se
  **genera**, no se escribe.
- La **PWA web ya demostró** que los 73 comandos de Tauri funcionan por HTTP puro
  (shim `invoke`→`fetch`). No hay que portar nada a JNI.

Consecuencia: **se reescribe solo la presentación**. Dominio, API y reglas de
negocio quedan intactos en Rust.

Stack de red: **Ktor + kotlinx.serialization**, no Retrofit. Retrofit es solo
JVM/Android; Ktor es multiplataforma y es lo que permite portar a iOS sin
reescribir la capa de red.

### Alcance

- **Android**: Tauri desaparece. App Kotlin + Compose pura, módulo nuevo
  `client-android/`.
- **Web PWA y escritorio Windows**: **`client/` (TS) sobrevive intacto.** Lo
  shippeado no se tira.
- **iOS**: por Compose Multiplatform más adelante. Por eso la disciplina CMP
  desde el día uno: capa de UI sin imports `android.*`, `expect/actual` para lo
  de plataforma.

## Consecuencias

### Positivas

- Techo de fluidez que el WebView no alcanza: `LazyColumn` virtualiza gratis,
  scroll y overscroll del sistema, animaciones interrumpibles, manejo de IME,
  gesto atrás predictivo, hápticos finos.
- Accesibilidad del sistema gratis: escala de letra, TalkBack, contraste. Es la
  razón de más peso para el usuario mayor.
- `minSdk 21`: se recuperan Android 5 y 6.
- El core Rust no se toca. Cero riesgo sobre la lógica de negocio.
- Las capacidades del aparato (cámara, Bluetooth, impresora) se simplifican:
  CameraX y el Bluetooth estándar de Android en vez de escribir plugins de Tauri.

### Negativas

- **Dos bases de UI**: Compose para móvil, TS para web y escritorio. ADR-0015
  existía justamente para evitar esto. La disciplina CMP contiene el daño en
  dos y no en tres, al cubrir iOS con el mismo código.
- Reescritura de presentación: 37 vistas, 16.758 líneas de TS, más 5.504 de sus
  tests. El cliente generado y la ausencia de plumbing de DOM reducen bastante el
  equivalente en Compose, pero no a cero.
- Riesgo de divergencia entre móvil y web al agregar features. Mitigación: la API
  es la fuente única; toda regla de negocio vive en Rust, nunca en la UI.
- Durante la transición conviven las dos apps. El proyecto Tauri de Android se
  borra **solo cuando** Compose alcance paridad en las pantallas de uso diario.

### Neutras

- El **servidor embebido en el teléfono pasa a ser opcional** (`CLAUDE.md`, piso
  de hardware, regla 7): el `.so` pesa ~46 MB y SurrealKV necesita su RAM. La app
  debe funcionar entera en modo **cliente liviano** contra un server de la red o
  la nube; el modo **nodo completo** es para el aparato que lo aguante.

## Más información

- Supersede: [ADR-0015](./0015-universal-cross-platform-client.md)
- Piso de hardware: `CLAUDE.md`, bloque "PISO DE HARDWARE" (commit `0c33184`)
- Frontend que sobrevive: `client/README.md`
- Diseño de referencia del design system: `client/src/views/ui.ts`
