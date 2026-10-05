package app.echoconnect

import android.content.Context
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.provider.Settings
import android.util.Log
import app.echoconnect.core.Core
import app.echoconnect.core.Listener
import app.echoconnect.clipboard.ClipboardSync
import app.echoconnect.share.Share
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** A laptop running EchoFiles, connected now or paired earlier. */
data class Laptop(
    val id: String,
    val name: String,
    val paired: Boolean,
    val connected: Boolean,
    val lan: Boolean,
    val bluetooth: Boolean,
    val echofiles: Boolean,
    val ip: String = "",
    val battery: Int? = null,
    val charging: Boolean = false,
)

/** Something that crossed the clipboard. */
data class Clip(val text: String, val toLaptop: Boolean, val at: Long, val sensitive: Boolean, val bluetooth: Boolean, val image: Boolean = false)

data class Transfer(
    val id: Long,
    val name: String,
    val size: Long,
    val upload: Boolean,
    val done: Long = 0,
    val finished: Boolean = false,
    val error: String? = null,
    val at: Long = System.currentTimeMillis(),
    val uri: String? = null,
)

/** The laptop asks to pair (or we asked): both screens show [code]. */
data class PairPrompt(val laptop: String, val name: String, val code: String, val theyAsked: Boolean)

data class Call(val who: String, val number: String, val since: Long, val onLaptop: Boolean)

data class UiState(
    val started: Boolean = false,
    val error: String? = null,
    val laptops: List<Laptop> = emptyList(),
    val pair: PairPrompt? = null,
    val clips: List<Clip> = emptyList(),
    val transfers: List<Transfer> = emptyList(),
    val theme: Map<String, Long>? = null,
    val call: Call? = null,
    val log: List<String> = emptyList(),
) {
    /** The laptop the app works with: the connected, paired one. */
    val laptop: Laptop? get() = laptops.firstOrNull { it.paired && it.connected } ?: laptops.firstOrNull { it.paired }
}

/** One feature: the packets it answers, and what it does when a laptop is ready. */
interface Plugin {
    val handles: Set<String> get() = emptySet()
    fun start(ctx: Context) {}
    fun onReady(ctx: Context, laptop: String) {}
    fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {}
    /** A payload offered with packet [type]; return true if this plugin took it. */
    fun onIncoming(ctx: Context, laptop: String, transfer: Long, name: String, size: Long, type: String, body: JSONObject): Boolean = false
    fun onTransferDone(ctx: Context, transfer: Long, path: String?, error: String?) {}
}

/**
 * The connection to the laptop: the Rust core, what the screens show, and the features.
 * Started once by [ConnectService]; screens read [state].
 */
object Connect {
    private const val TAG = "EchoConnect"
    lateinit var app: Context
        private set
    @Volatile private var core: Core? = null
    private val lifecycle = java.util.concurrent.atomic.AtomicLong(0)
    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state
    private val plugins = mutableListOf<Plugin>()
    private val byType = HashMap<String, Plugin>()
    private val retries = java.util.concurrent.ConcurrentHashMap<Long, () -> Unit>()
    /** transfer id → plugin that took the payload. */
    // Never call back into Rust on a native callback thread: JNA may detach a nested
    // callback while ART still has Java frames on that thread. Preserve event order here.
    private val events = java.util.concurrent.Executors.newSingleThreadExecutor { task -> Thread(task, "connect-events") }
    private val owners = java.util.concurrent.ConcurrentHashMap<Long, Plugin>()

    fun init(ctx: Context) {
        app = ctx.applicationContext
        _state.update { it.copy(theme = Prefs.theme(app)) }
    }

    fun register(p: Plugin) {
        plugins += p
        p.handles.forEach { byType[it] = p }
    }

    val running: Boolean get() = core != null

    @Synchronized fun start() {
        if (core != null) return
        val generation = lifecycle.incrementAndGet()
        val dir = File(app.filesDir, "connect").apply { mkdirs() }
        try {
            core = Core.start(dir.absolutePath, deviceName(), Events(generation))
            _state.update { it.copy(started = true, error = null) }
            refreshTrusted()
            plugins.forEach { runCatching { it.start(app) }.onFailure { e -> Log.w(TAG, "start ${it.javaClass.simpleName}", e) } }
        } catch (e: Exception) {
            Log.e(TAG, "core", e)
            _state.update { it.copy(error = e.message ?: "Couldn't start") }
        }
    }

    @Synchronized fun stop() {
        lifecycle.incrementAndGet()
        val previous = core
        core = null
        previous?.shutdown()
        owners.clear()
        retries.clear()
        _state.update { s -> s.copy(started = false, pair = null, laptops = s.laptops.map { it.copy(connected = false, lan = false, bluetooth = false) }) }
    }

    private fun deviceName(): String {
        val n = runCatching { Settings.Global.getString(app.contentResolver, Settings.Global.DEVICE_NAME) }.getOrNull()
        return if (!n.isNullOrBlank()) n else "${Build.MANUFACTURER.replaceFirstChar { it.uppercase() }} ${Build.MODEL}"
    }

    // ------------------------------------------------------------------ actions

    fun expectQr(id: String, fingerprint: String) = core?.expectQr(id, fingerprint)

    fun announce() = core?.announce()

    fun connect(host: String, port: Int = 1716): Boolean {
        val parts = host.trim().split(":")
        val address = if (parts.size == 2) parts[0] else host.trim()
        val chosen = if (parts.size == 2) parts[1].toIntOrNull() ?: return false else port
        if (chosen !in 1..65535) return false
        return core?.connect(address, chosen.toUShort()) ?: false
    }

    fun pair(id: String) = core?.pair(id)

    fun acceptPair() {
        val p = _state.value.pair ?: return
        core?.acceptPairCode(p.laptop, p.code)
        _state.update { it.copy(pair = null) }
    }

    fun rejectPair() {
        val p = _state.value.pair ?: return
        core?.rejectPair(p.laptop)
        _state.update { it.copy(pair = null) }
    }

    fun forget(id: String) {
        core?.unpair(id)
        refreshTrusted()
    }

    val laptopId: String? get() = _state.value.laptops.firstOrNull { it.paired && it.connected }?.id

    /** Send a packet to the connected laptop. */
    fun send(type: String, body: JSONObject = JSONObject(), to: String? = laptopId): Boolean {
        val id = to ?: return false
        return core?.send(id, type, body.toString()) ?: false
    }

    fun sendClipboard(text: String, sensitive: Boolean = false): Boolean {
        if (!Prefs.on(app, "clipboard")) return false
        val id = laptopId ?: return false
        val ok = core?.sendClipboard(id, text, sensitive) ?: false
        if (ok) addClip(Clip(text, true, System.currentTimeMillis(), sensitive, _state.value.laptop?.lan == false))
        return ok
    }

    fun sendBytes(type: String, body: JSONObject, bytes: ByteArray): Long? {
        val id = laptopId ?: return null
        return core?.sendBytes(id, type, body.toString(), bytes)?.toLong()
    }

    /** Send a content URI (a photo, a document) as a share or any payload packet. */
    fun sendUri(uri: Uri, type: String = "kdeconnect.share.request", extra: JSONObject = JSONObject(), count: Int = 1, total: Long = 0): Long? {
        val id = laptopId ?: return null
        val cr = app.contentResolver
        var name = "file"
        var size = -1L
        runCatching { cr.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                val n = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                val z = c.getColumnIndex(OpenableColumns.SIZE)
                if (n >= 0) name = c.getString(n) ?: name
                if (z >= 0 && !c.isNull(z)) size = c.getLong(z)
            }
        } }
        if (name == "file") name = extra.optString("filename", "attachment-${uri.lastPathSegment ?: "file"}")
        val pfd = runCatching { cr.openFileDescriptor(uri, "r") }.getOrNull() ?: return null
        if (size < 0) size = pfd.statSize
        if (size < 0) { pfd.close(); return null }
        val body = JSONObject(extra.toString()).put("filename", name).put("lastModified", System.currentTimeMillis())
        if (type == "kdeconnect.share.request") body.put("numberOfFiles", count).put("totalPayloadSize", if (total > 0) total else size)
        // The core owns the descriptor from here and closes it on every path.
        val active = core ?: run { pfd.close(); return null }
        val transfer = active.sendFd(id, type, body.toString(), pfd.detachFd(), size.toULong())?.toLong()
        if (type == "kdeconnect.share.request" && transfer != null) {
            if (retries.size > 100) retries.clear()
            retries[transfer] = { sendUri(uri, type, extra, count, total) }
        }
        return transfer
    }

    fun sendFile(file: File, type: String, extra: JSONObject = JSONObject()): Long? {
        val id = laptopId ?: return null
        val body = JSONObject(extra.toString()).put("filename", file.name).put("lastModified", file.lastModified())
        return core?.sendPath(id, type, body.toString(), file.absolutePath)?.toLong()
    }

    fun acceptFile(transfer: Long, dir: File, owner: Plugin? = null) {
        dir.mkdirs()
        owner?.let { owners[transfer] = it }
        core?.acceptFile(transfer.toULong(), dir.absolutePath)
    }

    fun stopSftp() = core?.stopSftp()

    fun cancelTransfer(id: Long) { core?.cancelTransfer(id.toULong()) }
    fun canRetry(id: Long) = retries.containsKey(id)
    fun retryTransfer(id: Long) { retries.remove(id)?.let { Work.run(it) } }
    fun rejectFile(transfer: Long) = core?.rejectFile(transfer.toULong())

    fun progress(transfer: Long): Pair<Long, Long>? = core?.progress(transfer.toULong())?.takeIf { it.size == 2 }?.let { it[0].toLong() to it[1].toLong() }

    fun adoptBluetooth(fd: Int, initiator: Boolean) = core?.adoptFd(fd, initiator)

    fun startSftp(laptop: String, roots: List<String>, names: List<String>): Result<Int> =
        runCatching { core?.startSftp(laptop, roots, names)?.toInt() ?: error("not running") }

    // ------------------------------------------------------------------ state

    fun addClip(c: Clip) = _state.update { s ->
        val keep = if (c.sensitive) c.copy(text = "") else c
        s.copy(clips = (listOf(keep) + s.clips).filter { System.currentTimeMillis() - it.at < 24 * 3600_000 }.take(60))
    }

    fun clearClips() = _state.update { it.copy(clips = emptyList()) }

    fun setCall(c: Call?) = _state.update { it.copy(call = c) }

    fun setTheme(t: Map<String, Long>?) {
        Prefs.setTheme(app, t)
        _state.update { it.copy(theme = t) }
    }

    private fun upsertTransfer(id: Long, f: (Transfer?) -> Transfer?) = _state.update { s ->
        val cur = s.transfers.firstOrNull { it.id == id }
        val next = f(cur)
        val rest = s.transfers.filter { it.id != id }
        s.copy(transfers = (if (next != null) listOf(next) + rest else rest).take(100))
    }

    fun finishTransfer(id: Long, error: String? = null, uri: String? = null) = upsertTransfer(id) { it?.copy(finished = true, error = error, uri = uri) }
    fun publishingTransfer(id: Long) = upsertTransfer(id) { it?.copy(finished = false, error = null) }
    fun noteTransfer(t: Transfer) = upsertTransfer(t.id) { t }

    fun tickTransfers() = _state.update { s ->
        s.copy(transfers = s.transfers.map { t -> if (t.finished) t else progress(t.id)?.let { (d, _) -> t.copy(done = d) } ?: t })
    }

    private fun refreshTrusted() {
        val c = core ?: return
        val trusted = runCatching { JSONArray(c.trusted()) }.getOrDefault(JSONArray())
        _state.update { s ->
            val known = s.laptops.associateBy { it.id }.toMutableMap()
            for (i in 0 until trusted.length()) {
                val t = trusted.getJSONObject(i)
                val id = t.getString("id")
                if (id !in known) known[id] = Laptop(id, t.optString("name"), paired = true, connected = false, lan = false, bluetooth = false, echofiles = true)
                else known[id] = known[id]!!.copy(paired = true)
            }
            val ids = (0 until trusted.length()).map { trusted.getJSONObject(it).getString("id") }.toSet()
            s.copy(laptops = known.values.map { if (it.id !in ids) it.copy(paired = false) else it }.filter { it.paired || it.connected })
        }
    }

    private fun laptopFrom(o: JSONObject, connected: Boolean, prev: Laptop?) = Laptop(
        id = o.getString("id"),
        name = o.optString("name"),
        paired = o.optBoolean("paired"),
        connected = connected,
        lan = o.optBoolean("lan"),
        bluetooth = o.optBoolean("bluetooth"),
        echofiles = o.optBoolean("echoconnect"),
        ip = o.optString("ip"),
        battery = prev?.battery,
        charging = prev?.charging ?: false,
    )

    private fun log(line: String) {
        if (BuildConfig.DEBUG) Log.d(TAG, line)
        _state.update { it.copy(log = (it.log + line).takeLast(200)) }
    }

    /** Events from the core, on core threads. */
    private class Events(private val generation: Long) : Listener {
        override fun onLog(line: String) { if (generation == lifecycle.get()) log(line) }

        override fun onEvent(kind: String, json: String) {
            events.execute {
                if (generation != lifecycle.get()) return@execute
                val o = runCatching { JSONObject(json) }.getOrElse { JSONObject() }
                try { handle(kind, o) } catch (e: Exception) { Log.w(TAG, "event $kind", e) }
            }
        }

        private fun handle(kind: String, o: JSONObject) {
            when (kind) {
                "failed" -> _state.update { it.copy(error = o.optString("message")) }
                "device" -> {
                    val id = o.getString("id")
                    _state.update { s ->
                        val prev = s.laptops.firstOrNull { it.id == id }
                        val l = laptopFrom(o, true, prev)
                        s.copy(laptops = listOf(l) + s.laptops.filter { it.id != id })
                    }
                    ConnectService.refresh(app)
                }
                "paired" -> {
                    val id = o.getString("id")
                    _state.update { s ->
                        val prev = s.laptops.firstOrNull { it.id == id }
                        s.copy(laptops = listOf(laptopFrom(o, true, prev)) + s.laptops.filter { it.id != id }, pair = null)
                    }
                    Prefs.setPairedOnce(app)
                    ConnectService.refresh(app)
                }
                "ready" -> {
                    val id = o.getString("id")
                    plugins.forEach { p -> runCatching { p.onReady(app, id) }.onFailure { Log.w(TAG, "ready", it) } }
                }
                "gone" -> {
                    val id = o.getString("id")
                    _state.update { s -> s.copy(laptops = s.laptops.map { if (it.id == id) it.copy(connected = false, lan = false, bluetooth = false) else it }.filter { it.paired || it.connected }) }
                    ConnectService.refresh(app)
                }
                "unpaired" -> { refreshTrusted(); core?.stopSftp() }
                "pair_requested", "pair_code" -> {
                    val id = o.getString("id")
                    val name = _state.value.laptops.firstOrNull { it.id == id }?.name ?: "Laptop"
                    _state.update { it.copy(pair = PairPrompt(id, name, o.getString("code"), kind == "pair_requested")) }
                    if (kind == "pair_requested") Notify.pairRequest(app, name, o.getString("code"))
                }
                "pair_rejected" -> _state.update { it.copy(pair = null) }
                "battery" -> {
                    val id = o.getString("id")
                    _state.update { s -> s.copy(laptops = s.laptops.map { if (it.id == id) it.copy(battery = o.optInt("level"), charging = o.optBoolean("charging")) else it }) }
                }
                "clipboard" -> ClipboardSync.fromLaptop(app, o.optString("text"), o.optBoolean("sensitive"))
                "text" -> Share.textFromLaptop(app, o.optString("text"))
                "url" -> Share.urlFromLaptop(app, o.optString("url"))
                "incoming" -> {
                    val t = o.getLong("transfer")
                    val type = o.optString("kind")
                    val body = o.optJSONObject("body") ?: JSONObject()
                    val taken = plugins.any { it.onIncoming(app, o.getString("id"), t, o.optString("name"), o.optLong("size"), type, body) }
                    if (!taken) rejectFile(t)
                }
                "transfer_started" -> noteTransfer(Transfer(o.getLong("transfer"), o.optString("name"), o.optLong("size"), o.optBoolean("upload")))
                "transfer_done" -> {
                    val t = o.getLong("transfer")
                    val err = if (o.has("error")) o.getString("error") else null
                    val path = if (o.has("path")) o.getString("path") else null
                    upsertTransfer(t) { cur -> (cur ?: Transfer(t, "Transfer", 0, o.optBoolean("upload"))).let { it.copy(finished = true, error = err, done = if (err == null) it.size else it.done) } }
                    owners.remove(t)?.onTransferDone(app, t, path, err)
                }
                "packet" -> {
                    val type = o.getString("type")
                    val body = o.optJSONObject("body") ?: JSONObject()
                    val p = byType[type]
                    if (p != null) Work.run { p.onPacket(app, o.getString("id"), type, body) } else log("unhandled $type")
                }
            }
        }
    }
}
