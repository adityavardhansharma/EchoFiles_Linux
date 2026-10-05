package app.echoconnect.clipboard

import androidx.compose.runtime.*
import android.app.Activity
import android.content.*
import android.os.*
import android.provider.Settings
import android.service.quicksettings.TileService
import androidx.core.content.FileProvider
import app.echoconnect.*
import java.io.File
import org.json.JSONObject

object ClipboardSync : Plugin {
    override val handles = setOf("echofiles.clipboard.image", "echofiles.clipboard.image.inline")
    private var last = ""
    private var lastImage: String? = null
    @Volatile private var reader: java.lang.Process? = null
    var automaticActive by mutableStateOf(false)
        private set
    private var watching = false
    private var generation = 0L
    private var starting = false
    private val sensitiveImages = java.util.concurrent.ConcurrentHashMap<Long, Boolean>()
    fun ready(ctx: Context) = Work.allowed(ctx, "android.permission.READ_LOGS") && Settings.canDrawOverlays(ctx)
    fun fromLaptop(ctx: Context, text: String, sensitive: Boolean) {
        if (!Prefs.on(ctx, "clipboard")) return
        Work.main.post {
            last = text
            val clip = ClipData.newPlainText("From laptop", text)
            clip.description.extras = PersistableBundle().apply { putBoolean("android.content.extra.IS_SENSITIVE", sensitive) }
            ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(clip)
            Connect.addClip(Clip(text, false, System.currentTimeMillis(), sensitive, Connect.state.value.laptop?.lan == false))
        }
    }
    enum class Sent { Off, Empty, Unchanged, Sent, Sending, Failed }

    /** Sends the phone's clipboard. [force] resends text the laptop already has (an explicit tap). */
    fun send(ctx: Context, force: Boolean = false): Sent {
        if (!Prefs.on(ctx, "clipboard")) return Sent.Off
        val clip = ctx.getSystemService(ClipboardManager::class.java).primaryClip ?: return Sent.Empty
        if (clip.itemCount == 0) return Sent.Empty
        val sensitive = clip.description.extras?.getBoolean("android.content.extra.IS_SENSITIVE", false) == true
        val item = clip.getItemAt(0)
        if (clip.description.hasMimeType("image/*") && item.uri != null) {
            if (item.uri.toString() == lastImage && !force) return Sent.Unchanged
            Work.run {
                val bytes = ctx.contentResolver.openInputStream(item.uri)?.use { val out = java.io.ByteArrayOutputStream(); val buffer = ByteArray(8192); while (out.size() <= 16*1024*1024) { val n = it.read(buffer); if (n < 0) break; out.write(buffer, 0, n) }; out.toByteArray() } ?: return@run
                if (bytes.size > 16*1024*1024) return@run
                val png = normalize(bytes) ?: return@run
                val body = JSONObject().put("sensitive", sensitive).put("mime", "image/png").put("filename", "clipboard.png")
                val sent = if (png.size <= 48*1024) Connect.send("echofiles.clipboard.image.inline", body.put("data", android.util.Base64.encodeToString(png, android.util.Base64.NO_WRAP)))
                else Connect.sendBytes("echofiles.clipboard.image", body, png) != null
                if (sent) { lastImage = item.uri.toString(); Connect.addClip(Clip("", true, System.currentTimeMillis(), sensitive, Connect.state.value.laptop?.lan == false, true)) }
            }
            return Sent.Sending
        }
        val text = item.coerceToText(ctx)?.toString() ?: return Sent.Empty
        if (text.isBlank()) return Sent.Empty
        if (text == last && !force) return Sent.Unchanged
        if (!Connect.sendClipboard(text, sensitive)) return Sent.Failed
        last = text
        return Sent.Sent
    }

    /** Puts an earlier clip back on this phone's clipboard without sending it to the laptop again. */
    fun copyAgain(ctx: Context, text: String) {
        last = text
        ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("EchoConnect", text))
    }
    fun startWatching(ctx: Context) {
        if (watching) return
        watching = true
        ctx.getSystemService(ClipboardManager::class.java).addPrimaryClipChangedListener { send(ctx) }
        ctx.registerReceiver(object : BroadcastReceiver() {
            override fun onReceive(c: Context, i: Intent) {
                if (i.action == Intent.ACTION_SCREEN_OFF) stopReader() else resume(c)
            }
        }, IntentFilter().apply { addAction(Intent.ACTION_SCREEN_OFF); addAction(Intent.ACTION_SCREEN_ON) })
        resume(ctx)
    }
    @Synchronized fun resume(ctx: Context) {
        if (starting || reader != null || Prefs.clipMode(ctx) != "auto" || !ready(ctx) || Connect.laptopId == null || !Prefs.on(ctx, "clipboard")) return
        if (!ctx.getSystemService(PowerManager::class.java).isInteractive) return
        starting = true
        val token = ++generation
        Thread({
            val p = runCatching { ProcessBuilder("logcat", "-T", "1", "-v", "brief", "ClipboardService:I", "*:S").start() }.getOrElse { synchronized(this) { if (generation == token) starting = false }; return@Thread }
            synchronized(this) {
                if (generation != token) { p.destroy(); return@Thread }
                reader = p; starting = false
            }
            try {
                p.inputStream.bufferedReader().useLines { lines -> lines.forEach { line ->
                    if (line.contains(ctx.packageName) && line.contains("clipboard", true) && (line.contains("denying", true) || line.contains("denied", true))) {
                        automaticActive = true
                        Work.main.post { if (Connect.laptopId != null && Prefs.clipMode(ctx) == "auto") readWindow(ctx) }
                    }
                } }
            } finally { synchronized(this) { if (reader === p) { reader = null; automaticActive = false } }; p.destroy() }
        }, "clipboard-log").start()
    }
    @Synchronized fun stopReader() { generation++; starting = false; reader?.destroy(); reader = null; automaticActive = false }
    fun readWindow(ctx: Context) { ctx.startActivity(Intent(ctx, ClipboardReadActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "clipboard") || type != "echofiles.clipboard.image.inline") return
        val data = body.optString("data")
        if (data.length > 65536) return
        val bytes = android.util.Base64.decode(data, android.util.Base64.DEFAULT)
        val file = File(File(ctx.cacheDir, "clipboard").apply { mkdirs() }, "image-${System.currentTimeMillis()}.png")
        file.writeBytes(bytes)
        sensitiveImages[0] = body.optBoolean("sensitive")
        onTransferDone(ctx, 0, file.path, null)
    }
    override fun onReady(ctx: Context, laptop: String) { Work.main.post { resume(ctx) } }
    override fun onIncoming(ctx: Context, laptop: String, transfer: Long, name: String, size: Long, type: String, body: JSONObject): Boolean {
        if (type != "echofiles.clipboard.image") return false
        if (!Prefs.on(ctx, "clipboard") || size !in 1..16_777_216) { Connect.rejectFile(transfer); return true }
        sensitiveImages[transfer] = body.optBoolean("sensitive")
        Connect.acceptFile(transfer, File(ctx.cacheDir, "clipboard"), this)
        return true
    }
    private fun normalize(bytes: ByteArray): ByteArray? {
        val bounds = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
        android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0 || bounds.outWidth.toLong() * bounds.outHeight > 16_000_000) return null
        val bitmap = android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size) ?: return null
        return try { java.io.ByteArrayOutputStream().use { out -> bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, out); out.toByteArray().takeIf { it.size <= 16*1024*1024 } } } finally { bitmap.recycle() }
    }
    override fun onTransferDone(ctx: Context, transfer: Long, path: String?, error: String?) {
        val sensitive = sensitiveImages.remove(transfer) ?: false
        if (path == null || error != null) return
        Work.run {
            val source = File(path)
            val png = normalize(source.readBytes())
            if (png == null) { source.delete(); return@run }
            val directory = File(ctx.cacheDir, "clipboard").apply { mkdirs() }
            val target = File(directory, "clip-${java.util.UUID.randomUUID()}.png")
            target.writeBytes(png); source.delete()
            directory.listFiles()?.filter { it != target && System.currentTimeMillis() - it.lastModified() > 3600_000 }?.forEach { it.delete() }
            Work.main.post {
                val uri = FileProvider.getUriForFile(ctx, "${ctx.packageName}.files", target)
                lastImage = uri.toString()
                val clip = ClipData.newUri(ctx.contentResolver, "Image from laptop", uri)
                clip.description.extras = PersistableBundle().apply { putBoolean("android.content.extra.IS_SENSITIVE", sensitive) }
                ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(clip)
                Connect.addClip(Clip("", false, System.currentTimeMillis(), sensitive, Connect.state.value.laptop?.lan == false, true))
            }
        }
    }
}

class ClipboardReadActivity : Activity() {
    override fun onCreate(state: Bundle?) { super.onCreate(state); window.setBackgroundDrawableResource(android.R.color.transparent) }
    override fun onWindowFocusChanged(focus: Boolean) {
        super.onWindowFocusChanged(focus)
        if (focus) Work.main.postDelayed({ ClipboardSync.send(this); finish() }, 150)
    }
}
class ClipboardTile : TileService() {
    @android.annotation.SuppressLint("StartActivityAndCollapseDeprecated") // PendingIntent overload exists only from API 34; older phones need Intent.
    override fun onClick() {
        super.onClick()
        val intent = Intent(this, ClipboardReadActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        if (Build.VERSION.SDK_INT >= 34) startActivityAndCollapse(android.app.PendingIntent.getActivity(this, 40, intent, android.app.PendingIntent.FLAG_IMMUTABLE))
        else @Suppress("DEPRECATION") startActivityAndCollapse(intent)
    }
}
