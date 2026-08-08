# RutAgent — Del cuaderno al ERP, hablando

> Ultra-plan de visión y producto. Fijado por el founder (Coquimbo, Chile), 2026-07-26.
> Enmarca TODA decisión de producto, diseño y priorización. No caduca.
> Bloque resumen en `CLAUDE.md` (META GENERAL). Vectores en `business-depth-master-plan.md`.

---

## 1. El norte, en una frase

**Que cualquier chileno con un cuaderno y un celular pueda, en cinco minutos y sin pagar
nada, tener el mismo ERP que una gran empresa — hablándole a un agente en vez de aprender
un sistema.**

De la libreta de fiados de la señora del almacén al negocio formal, con boleta, con IVA al
día, con datos que le dicen qué comprar y a quién cobrar. Sin manual, sin contador que no
puede pagar, sin instalar nada. Gratis de verdad.

---

## 2. Para quién — el retrato real

No "PYMEs". Personas concretas, las que hoy NO usan ningún software:

- **La señora Rosa, almacén de barrio.** Fía en un cuaderno. Sabe el stock "de memoria".
  Cuenta la plata a mano. Le da miedo el SII. Tiene WhatsApp y un celular de gama media.
- **Don Luis, feriante (feria libre de La Serena/Coquimbo).** Vende sin conexión, cobra
  efectivo y transferencia, pierde plata en mermas de verdura, no sabe su margen.
- **Javiera, repostería casera que vende por Instagram/WhatsApp.** Toma pedidos por chat,
  no emite boleta, quiere formalizarse pero no sabe por dónde.
- **El emprendedor que recién parte** y no puede pagar Bsale/Nubox/Defontana ni un contador.

Denominador común: **smartphone + WhatsApp + cuaderno + miedo a la complejidad.** Ese es el
usuario. Todo lo que construyamos se mide contra: *"¿la señora Rosa, en su celular, sin
manual, le pide esto al agente y funciona como en un ERP caro?"*

---

## 3. El insight central: el cuaderno es el enemigo Y el maestro

El competidor real no es Bsale ni Defontana. **Es el cuaderno.** Y el cuaderno gana en lo
que importa: es instantáneo, no se cae, habla el idioma del dueño, no cobra, no juzga, no
pide capacitación. Cualquier ERP que exija *aprender un ERP* ya perdió contra el cuaderno.

Entonces no reemplazamos el cuaderno con un sistema. **Lo reemplazamos con un cuaderno que
tiene superpoderes** y habla chileno. El agente ES el cuaderno: le dictas y él anota, suma,
recuerda, cobra y formaliza por detrás. El ERP es la tinta invisible.

---

## 4. Los cinco principios (no-negociables)

1. **Agente-first, no menú-first.** La puerta de entrada es una frase, no un menú de 15
   secciones. *"fié 5 lucas a la señora Rosa"*, *"¿cuánto vendí hoy?"*, *"¿me queda pan?"*,
   *"¿quién me debe?"*. El agente cierra el LOOP COMPLETO del negocio de barrio. Los menús
   son el respaldo para el que quiere, nunca el requisito.

2. **Cero fricción de entrada — la adopción ES el producto.** Web, sin instalar, en el
   navegador del celular. Alta con RUT en 30 segundos y ya tienes tienda, pre-cargada según
   tu rubro. Funciona con internet malo (PWA, tolerante a caídas). **Nunca una tarjeta de
   crédito.** Si cuesta entrar, no existe.

3. **El puente desde lo analógico: importar el cuaderno con una foto.** El acto de adopción
   killer. Le sacas una foto a la hoja del cuaderno — fiados, productos, precios — y el
   agente lo lee y lo carga. El dueño cruza de la libreta al ERP sin tipear una fila. Ese
   es el momento en que se enamora.

4. **Formalización como regalo, no como amenaza.** El SII asusta; nosotros lo hacemos fácil
   y gratis. Boleta electrónica incorporada. IVA / F29 calculado solo. Inicio de actividades
   guiado. **El agente es el contador que no podían pagar.** Cada negocio informal que
   formalizamos entra al sistema financiero (crédito, crecimiento). Ese es el impacto social,
   no un feature.

5. **Profundidad que se gana la confianza — cero módulos a medias.** Un cuaderno nunca se
   cae; nosotros tampoco. Ledgers inmutables (el fiado ya lo es), append-only, plata real
   sin bugs. Regla #1 GATE + DoD: cada capacidad se construye COMPLETA de punta a punta
   antes de pasar a la siguiente. Vara de calidad = las grandes (Defontana/Bsale/Nubox);
   precio = cero.

---

## 5. La escalera de adopción (cómo entra un negocio en 5 minutos)

1. Alguien del barrio le muestra el celular: *"mira, es gratis, le hablái no más"*.
2. Abre el link (web, sin instalar). RUT + rubro → tienda lista, sembrada.
3. Foto del cuaderno → sus fiados y productos adentro. **Ya ganamos.**
4. Primera venta hablándole al agente. Primer *"¿cuánto vendí hoy?"* respondido.
5. A la semana: *"la señora Rosa te debe hace 20 días"* — el agente cobra por él.
6. Al mes: *"tu IVA da $X, ¿lo declaramos?"* — se formaliza sin darse cuenta.
7. Le muestra el celular al almacén de al lado. El loop se repite. **Boca a boca = fiado:
   corre por confianza de barrio.**

---

## 6. Los vectores de profundidad que faltan (mapeados a la visión)

Ya construido (base sólida): V1 fiado/cuenta corriente · V2 multi-sucursal · V2.1 FEFO por
sucursal · V3 libro de compras + IVA/F29 · agente que vende, fía, cobra, repone y declara ·
cimientos SII (DTE, CAF, TED) · cliente Tauri web+desktop+móvil-scaffold.

Lo que hace REALIDAD esta visión (candidatos V4–V6, priorizar por impacto de adopción):

- **Boleta electrónica SII gratis, de punta a punta.** El moat y el bien público: empuja
  informal → formal. Emisión real desde el agente (*"hazme la boleta"*), sobre los cimientos
  DTE/CAF que ya existen. **El vector de mayor impacto social.**
- **Importar-cuaderno-por-foto (OCR + extracción del agente).** El killer de onboarding del
  §4.3. Puente analógico→digital. **El vector de mayor impacto de adopción.**
- **Pagos como paga Chile: transferencia + apps.** Registrar *"me transfirió"*, conciliar,
  MACH/MercadoPago/Sumup. El efectivo ya no es el único; la transferencia sí es el default.
- **Puente WhatsApp.** El dueño ya vive en WhatsApp. Catálogo, pedidos, *"tu cuenta: debes
  $X"*, recordatorio de fiado. El comercio social es CÓMO vende Chile.
- **Mermas y vencimientos por local** (venc en curso). Crítico para verdulería/carnicería/
  feria — donde se pierde la plata de verdad.
- **Alfabetización financiera del agente.** Que enseñe: *"tu margen bajó"*, *"estás dando
  mucho fiado"*, *"te conviene comprar X"*. El ERP como coach, no solo registro.
- **Modo feria / offline real.** Vender sin señal, sincronizar después. Coquimbo tiene
  conectividad de barrio y de feria.

Cada uno se construye COMPLETO (§4.5), en su propia lane, con GATE + smoke en vivo.

---

## 7. El motor de impacto — Coquimbo primero

El impacto no se declara, se siembra en una ciudad y se deja crecer:

- **Coquimbo / La Serena** como primer terreno: ferias libres, caletas, almacenes de barrio,
  bazares, emprendimiento turístico. Densidad de exactamente el usuario del §2.
- **Piloto con negocios reales** — no usuarios de demo, almacenes y feriantes de verdad.
- **Boca a boca de barrio** como canal de crecimiento (§5.7): un negocio que adopta le
  cuenta al siguiente. Nano-influencia, confianza local.
- **Aliados locales**: municipalidad, Sercotec, cámara de comercio, Prodemu, juntas de
  vecinos. Ellos ya quieren formalizar el comercio informal; nosotros somos la herramienta
  gratis que lo hace posible.
- Meta de impacto: que en Coquimbo empezar un negocio formal deje de necesitar plata,
  contador ni saber de sistemas — solo un RUT, un celular y una frase. Y desde ahí, Chile.

---

## 8. Cómo medimos "profesional" (la vara)

Profesional NO es corporativo ni intimidante. Es:

- **Confiable con la plata** — nunca perder un dato del cuaderno; ledgers inmutables.
- **Instantáneo** — una venta en 2 toques o 1 frase; POS <50ms p99, resto <100ms.
- **Chileno hasta el hueso** — lucas, transferencia, boleta, fiado, mermas, es-CL humano,
  cero jerga técnica.
- **Cálido, no frío** — se siente como el cuaderno de confianza, no como una planilla de
  contador.
- **Cero dead-ends, cero crash con input basura, estados empty/error que enseñan** (las 8
  varas de UX intuitiva del CLAUDE.md siguen vigentes).
- **Respetuoso con los datos** — Ley 19.628, privacidad por defecto.

Regla de oro: *si la señora Rosa no lo entiende sola en su celular, no está terminado.*

---

## 9. Qué NO somos

- No somos software para contadores. Somos para el DUEÑO.
- No somos un menú de 15 módulos que hay que aprender.
- No somos freemium-trampa: el core es gratis para siempre, de verdad.
- No somos genéricos-globales: somos chilenos, del barrio, de la feria.
- No dejamos módulos a medias para verse más completos.

---

*Este documento es el norte. Cuando una decisión de producto o técnica esté en duda, se
resuelve preguntando: "¿esto acerca a la señora Rosa, en su celular, sin manual, a tener el
ERP de una gran empresa gratis?" Si no, no va.*
