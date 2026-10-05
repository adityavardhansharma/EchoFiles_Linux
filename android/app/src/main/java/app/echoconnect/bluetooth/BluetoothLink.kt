package app.echoconnect.bluetooth

import android.Manifest
import android.bluetooth.*
import android.content.Context
import android.os.Build
import android.os.ParcelFileDescriptor
import app.echoconnect.*
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import kotlin.concurrent.thread

/** RFCOMM bytes are bridged to an owned socket descriptor; Rust authenticates with TLS. */
object BluetoothLink {
    val UUID_SERVICE: UUID = UUID.fromString("d5f58a22-99f5-4c68-9e21-e1b168395832")
    @Volatile private var running = false
    @Volatile private var generation = 0L
    private var server: BluetoothServerSocket? = null
    private val sockets = ConcurrentHashMap.newKeySet<BluetoothSocket>()
    private fun permitted(ctx: Context) = Build.VERSION.SDK_INT < 31 || Work.allowed(ctx, Manifest.permission.BLUETOOTH_CONNECT)
    @android.annotation.SuppressLint("MissingPermission") // permitted() checks BLUETOOTH_CONNECT before any adapter access.
    @Synchronized fun start(ctx: Context) {
        if (running || !permitted(ctx)) return
        val adapter = ctx.getSystemService(BluetoothManager::class.java)?.adapter ?: return
        if (!adapter.isEnabled) return
        running = true
        val token = ++generation
        thread(name = "connect-bt-listen", isDaemon = true) {
            var listener: BluetoothServerSocket? = null
            try {
                listener = adapter.listenUsingRfcommWithServiceRecord("EchoConnect", UUID_SERVICE)
                synchronized(this) { if (generation != token) { listener.close(); return@thread }; server = listener }
                while (running && generation == token) listener.accept()?.let { bridge(it, false) }
            } catch (_: Exception) { } finally { runCatching { listener?.close() }; synchronized(this) { if (generation == token) running = false } }
        }
        thread(name = "connect-bt-reconnect", isDaemon = true) {
            while (running && generation == token) {
                val mac = Prefs.laptopBluetooth(ctx)
                if (mac != null && sockets.isEmpty()) runCatching {
                    val socket = adapter.getRemoteDevice(mac).createRfcommSocketToServiceRecord(UUID_SERVICE)
                    sockets.add(socket)
                    try { socket.connect(); bridge(socket, true) } catch (e: Exception) { sockets.remove(socket); socket.close() }
                }
                Thread.sleep(15000)
            }
        }
    }
    private fun bridge(socket: BluetoothSocket, initiator: Boolean) {
        sockets.add(socket)
        val pair = ParcelFileDescriptor.createSocketPair()
        Connect.adoptBluetooth(pair[0].detachFd(), initiator)
        val input = ParcelFileDescriptor.AutoCloseInputStream(pair[1])
        val output = ParcelFileDescriptor.AutoCloseOutputStream(ParcelFileDescriptor.dup(pair[1].fileDescriptor))
        fun close() { sockets.remove(socket); runCatching { socket.close() }; runCatching { input.close() }; runCatching { output.close() } }
        thread(isDaemon = true) { try { socket.inputStream.copyTo(output) } catch (_: Exception) {} finally { close() } }
        thread(isDaemon = true) { try { input.copyTo(socket.outputStream) } catch (_: Exception) {} finally { close() } }
    }
    @Synchronized fun stop() { generation++; running = false; runCatching { server?.close() }; sockets.forEach { runCatching { it.close() } }; sockets.clear() }
}
