-- pharma-server 0052: la cuenta de Google ligada a un usuario (ADR-0022).
--
-- Acá NO hay ningún secreto: ni client id, ni client secret, ni el `id_token`.
-- El `id_token` es de un solo uso, se verifica contra las llaves públicas de
-- Google y se tira. Lo único que sobrevive a la verificación es el `sub`, que
-- es un identificador opaco y público dentro de nuestra cuenta de Google.
--
-- ## Por qué `sub` y no el email
--
-- Es la decisión central de esta tabla y la razón de que exista, en vez de
-- resolver todo con un `WHERE email = $email` contra `user`.
--
-- Un email cambia de dueño. Alguien deja la empresa y a los seis meses el
-- dominio reasigna `ventas@` a quien entró después; una casilla de Gmail
-- abandonada vuelve al pozo. Si la sesión se resolviera por email, el día que
-- eso pasa la persona nueva entra al negocio de la anterior — con las ventas,
-- las deudas y la caja adentro. No hay aviso, no hay error: entra bien.
--
-- El `sub` de Google no se reasigna nunca. Es el mismo string para esa persona
-- aunque cambie de correo, de nombre y de teléfono. Por eso la sesión se busca
-- por `sub` y `email_al_ligar` queda sólo como rastro de auditoría: sirve para
-- que un humano entienda una fila, nunca para encontrarla.
--
-- ## Las dos formas de ligar, y por qué el email igual aparece
--
-- El vínculo tiene que nacer alguna vez. La primera vez que un `sub` llega no
-- hay fila, y hay que decidir a qué usuario corresponde. Se hace con el email
-- verificado de Google contra el email del `user` del negocio — es el único
-- dato compartido entre las dos cuentas.
--
-- Ese primer cruce es la parte delicada, y por eso el server lo exige completo:
-- `email_verified` en true, negocio explícito en el pedido, y usuario activo.
-- Después de esa vez, el email no vuelve a decidir nada.

DEFINE TABLE google_identity SCHEMAFULL;

-- El `sub` del `id_token` verificado. Identidad estable.
DEFINE FIELD sub ON google_identity TYPE string
    ASSERT $value != NONE AND string::len($value) > 0;

DEFINE FIELD user   ON google_identity TYPE record<user>;
DEFINE FIELD tenant ON google_identity TYPE record<tenant>;

-- Rastro, no llave. Qué correo traía Google el día que se armó el vínculo.
-- Que después cambie no rompe nada: nadie lo consulta para entrar.
DEFINE FIELD email_al_ligar ON google_identity TYPE option<string>;

DEFINE FIELD linked_at ON google_identity TYPE datetime DEFAULT time::now();
DEFINE FIELD last_login_at ON google_identity TYPE option<datetime>;

-- Consulta caliente y única: dado un `sub`, en qué negocio y como quién entra.
--
-- No es UNIQUE sobre `sub` solo, a propósito: la misma persona puede tener un
-- puesto en la feria y ayudar en el almacén de un hermano. Un `sub` con dos
-- filas es el caso legítimo de "pertenece a dos negocios", y el server lo
-- resuelve pidiendo que elija — no rechazándolo.
DEFINE INDEX gi_sub ON google_identity FIELDS sub;

-- Un `sub` no puede estar ligado dos veces dentro del mismo negocio: serían dos
-- usuarios distintos para la misma persona y el login quedaría no determinista.
DEFINE INDEX gi_sub_tenant ON google_identity FIELDS sub, tenant UNIQUE;

-- Y un usuario tiene a lo sumo una cuenta de Google. Sin esto, dos cuentas de
-- Google ligadas al mismo `user` significan que quien controle cualquiera de
-- las dos entra: la superficie de robo de cuenta se duplica sin que nadie lo
-- haya decidido.
DEFINE INDEX gi_user ON google_identity FIELDS user UNIQUE;
