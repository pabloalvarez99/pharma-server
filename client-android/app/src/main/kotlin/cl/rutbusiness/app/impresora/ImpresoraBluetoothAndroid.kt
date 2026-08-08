package cl.rutbusiness.app.impresora

import android.Manifest
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothClass
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothSocket
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat
import cl.rutbusiness.app.ui.impresora.AnchoDePapel
import cl.rutbusiness.app.ui.impresora.EnlaceDeImpresora
import cl.rutbusiness.app.ui.impresora.EscPos
import cl.rutbusiness.app.ui.impresora.EstadoDePapel
import cl.rutbusiness.app.ui.impresora.FallaDeImpresion
import cl.rutbusiness.app.ui.impresora.ImpresoraConocida
import cl.rutbusiness.app.ui.impresora.ImpresoraElegida
import cl.rutbusiness.app.ui.impresora.Intento
import java.io.IOException
import java.util.TimeZone
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext

/**
 * El único archivo del módulo que sabe que existe Android.
 *
 * Todo lo de arriba —el armado de la boleta, la pantalla, el ViewModel— habla
 * con [EnlaceDeImpresora] y no con esto. Es la misma frontera que
 * `AlmacenamientoPlataforma` en `:core`: acá adentro hay `android.bluetooth`,
 * afuera no hay ni un import.
 *
 * **Permisos, los dos caminos.** Android 12 (API 31) partió el viejo permiso
 * `BLUETOOTH` en `BLUETOOTH_CONNECT` y `BLUETOOTH_SCAN`, y cambió el primero de
 * permiso de instalación a permiso que hay que pedir en el momento. El aparato
 * objetivo es viejo y puede estar de cualquiera de los dos lados:
 *
 * - hasta API 30, `BLUETOOTH` se concede al instalar y [permisosQuePedir] va
 *   vacío: no hay nada que preguntarle a la dueña;
 * - desde API 31, hay que pedir `BLUETOOTH_CONNECT` en pantalla.
 *
 * **`BLUETOOTH_SCAN` no se pide, y es a propósito.** Es el permiso de
 * *descubrir* aparatos, y esto nunca descubre: sólo lee las que ya están
 * emparejadas. Pedir un permiso que no se usa es peor que no pedirlo — la dueña
 * ve "buscar dispositivos cercanos" en una app de boletas, y con razón
 * desconfía. Emparejar se hace una vez desde los ajustes del sistema, que
 * además es donde ya sabe hacerlo porque es donde emparejó sus audífonos.
 * De paso resuelve el piso de hardware: sin escaneo no hay radio despierta
 * comiéndose la batería.
 */
class ImpresoraBluetoothAndroid(context: Context) : EnlaceDeImpresora {

    private val app = context.applicationContext

    private val adaptador: BluetoothAdapter?
        get() = (app.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager)?.adapter

    override val permisosQuePedir: List<String> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            listOf(Manifest.permission.BLUETOOTH_CONNECT)
        } else {
            // Hasta Android 11 `BLUETOOTH` se concede al instalar. No hay
            // diálogo que mostrar y pedirlo en runtime no haría nada.
            emptyList()
        }

    override fun faltaPermiso(): Boolean = permisosQuePedir.any {
        ContextCompat.checkSelfPermission(app, it) != PackageManager.PERMISSION_GRANTED
    }

    override fun hayBluetooth(): Boolean = adaptador != null

    override fun bluetoothEncendido(): Boolean = adaptador?.isEnabled == true

    override fun desfaseHorarioMinutos(): Int =
        TimeZone.getDefault().getOffset(System.currentTimeMillis()) / 60_000

    override fun emparejadas(): Intento<List<ImpresoraConocida>> {
        val adaptador = adaptador ?: return Intento.Fallo(FallaDeImpresion.SinBluetooth())
        if (faltaPermiso()) return Intento.Fallo(FallaDeImpresion.FaltaPermiso())
        if (!adaptador.isEnabled) return Intento.Fallo(FallaDeImpresion.BluetoothApagado())

        // `getBondedDevices` tira SecurityException si el permiso se revocó
        // entre el chequeo de arriba y esta línea. Pasa de verdad: Android
        // puede revocar permisos de una app que no se usa hace meses.
        val emparejadas = try {
            adaptador.bondedDevices.orEmpty()
        } catch (e: SecurityException) {
            return Intento.Fallo(FallaDeImpresion.FaltaPermiso())
        }

        if (emparejadas.isEmpty()) {
            return Intento.Fallo(FallaDeImpresion.NingunaEmparejada())
        }

        // Las que parecen impresoras primero, pero **sin esconder el resto**:
        // más de un clon barato se declara con clase "sin categoría", y
        // filtrar por clase escondería justamente la impresora que la dueña
        // compró. Ordenar ayuda; ocultar rompe.
        return Intento.Ok(
            emparejadas
                .sortedWith(
                    compareByDescending<BluetoothDevice> { pareceImpresora(it) }
                        .thenBy { nombreDe(it).lowercase() },
                )
                .map { ImpresoraConocida(direccion = it.address, nombre = nombreDe(it)) },
        )
    }

    override suspend fun imprimir(
        impresora: ImpresoraElegida,
        bytes: ByteArray,
    ): Intento<Unit> = withContext(Dispatchers.IO) {
        val adaptador = adaptador ?: return@withContext Intento.Fallo(FallaDeImpresion.SinBluetooth())
        if (faltaPermiso()) return@withContext Intento.Fallo(FallaDeImpresion.FaltaPermiso())
        if (!adaptador.isEnabled) {
            return@withContext Intento.Fallo(FallaDeImpresion.BluetoothApagado())
        }

        val aparato = try {
            adaptador.bondedDevices.orEmpty().firstOrNull { it.address == impresora.direccion }
        } catch (e: SecurityException) {
            return@withContext Intento.Fallo(FallaDeImpresion.FaltaPermiso())
        } ?: return@withContext Intento.Fallo(
            FallaDeImpresion.YaNoEstaEmparejada(impresora.nombre),
        )

        val socket = try {
            abrir(aparato)
        } catch (e: SecurityException) {
            return@withContext Intento.Fallo(FallaDeImpresion.FaltaPermiso())
        } catch (e: IOException) {
            // Apagada y fuera de alcance dan la misma IOException. No se
            // inventa cuál de las dos fue.
            return@withContext Intento.Fallo(
                FallaDeImpresion.NoContesta(impresora.nombre, e.toString()),
            )
        }

        try {
            if (papel(socket) == EstadoDePapel.NoHay) {
                return@withContext Intento.Fallo(FallaDeImpresion.SinPapel(impresora.nombre))
            }

            socket.outputStream.write(bytes)
            socket.outputStream.flush()

            // Cerrar apenas termina el `flush` corta la boleta a la mitad en
            // varios clones: el `flush` vacía el buffer del teléfono, no el de
            // la impresora, que sigue empujando papel. Esta espera es lo que
            // separa una boleta entera de media boleta.
            delay(ESPERA_DE_VACIADO_MS)
            Intento.Ok(Unit)
        } catch (e: IOException) {
            Intento.Fallo(FallaDeImpresion.SeCortoAMitad(impresora.nombre, e.toString()))
        } catch (e: SecurityException) {
            Intento.Fallo(FallaDeImpresion.FaltaPermiso())
        } finally {
            // Siempre se cierra. Dejar el socket abierto mantiene el enlace
            // Bluetooth vivo entre venta y venta, que es batería regalada y
            // además deja la impresora tomada para cualquier otro teléfono.
            runCatching { socket.close() }
        }
    }

    /**
     * Abre el canal serie con la impresora.
     *
     * Dos caminos, y el segundo no es paranoia: el descubrimiento de servicios
     * SDP falla en buena parte de los clones chinos de 58 mm y en Androids
     * viejos, y ahí `createRfcommSocketToServiceRecord` tira IOException
     * aunque la impresora esté encendida al lado. El camino de reserva pide el
     * canal 1 por reflexión, que es donde esas impresoras publican el puerto
     * serie. Es el aparato viejo del encargo, no un caso de borde.
     */
    @Throws(IOException::class, SecurityException::class)
    private fun abrir(aparato: BluetoothDevice): BluetoothSocket {
        val estandar = aparato.createRfcommSocketToServiceRecord(SPP)
        try {
            estandar.connect()
            return estandar
        } catch (e: IOException) {
            runCatching { estandar.close() }
            val reserva = try {
                BluetoothDevice::class.java
                    .getMethod("createRfcommSocket", Int::class.javaPrimitiveType)
                    .invoke(aparato, 1) as BluetoothSocket
            } catch (reflexion: Exception) {
                // La reflexión no está disponible: gana el error original, que
                // es el que describe lo que le pasa a la impresora.
                throw e
            }
            try {
                reserva.connect()
                return reserva
            } catch (segundo: IOException) {
                runCatching { reserva.close() }
                throw segundo
            }
        }
    }

    /**
     * Le pregunta a la impresora si le queda papel, sin bloquear.
     *
     * `DLE EOT 4` se procesa en tiempo real, fuera de la cola de impresión, así
     * que se puede preguntar antes de mandar la boleta. La mayoría de los
     * clones baratos **no contesta**, y eso no es una falla: se imprime igual
     * y si no hay papel el cajero lo ve. Sólo cuando contesta que no hay se
     * corta antes, que es lo que evita mandar una boleta al vacío.
     *
     * Se sondea `available()` en vez de bloquear en `read()`: un `read()` sin
     * respuesta se queda colgado hasta que alguien cierre el socket, y eso
     * serían dos segundos de pantalla congelada en cada impresión de las
     * impresoras que no hablan.
     */
    private suspend fun papel(socket: BluetoothSocket): EstadoDePapel = try {
        socket.outputStream.write(EscPos.CONSULTA_DE_PAPEL)
        socket.outputStream.flush()

        var restante = ESPERA_DE_ESTADO_MS
        var respuesta: Int? = null
        while (restante > 0 && respuesta == null) {
            if (socket.inputStream.available() > 0) {
                respuesta = socket.inputStream.read()
            } else {
                delay(SONDEO_MS)
                restante -= SONDEO_MS
            }
        }

        when {
            respuesta == null || respuesta < 0 -> EstadoDePapel.NoContesta
            respuesta and EscPos.SIN_PAPEL != 0 -> EstadoDePapel.NoHay
            else -> EstadoDePapel.Hay
        }
    } catch (e: IOException) {
        // Que falle la consulta no puede impedir imprimir: la boleta es lo
        // importante y el estado del papel es un lujo.
        EstadoDePapel.NoContesta
    }

    private fun nombreDe(aparato: BluetoothDevice): String = try {
        aparato.name?.takeIf { it.isNotBlank() } ?: aparato.address
    } catch (e: SecurityException) {
        aparato.address
    }

    private fun pareceImpresora(aparato: BluetoothDevice): Boolean = try {
        val clase = aparato.bluetoothClass
        clase?.majorDeviceClass == BluetoothClass.Device.Major.IMAGING
    } catch (e: SecurityException) {
        false
    }

    private companion object {
        /** El UUID del perfil de puerto serie. Toda impresora ESC/POS lo usa. */
        val SPP: UUID = UUID.fromString("00001101-0000-1000-8000-00805F9B34FB")

        const val ESPERA_DE_VACIADO_MS = 400L
        const val ESPERA_DE_ESTADO_MS = 400L
        const val SONDEO_MS = 40L
    }
}
