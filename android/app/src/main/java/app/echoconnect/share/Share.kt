package app.echoconnect.share

import android.app.PendingIntent
import android.content.*
import android.net.Uri
import android.os.Environment
import android.provider.MediaStore
import app.echoconnect.*
import org.json.JSONObject
import java.io.File

object Share : Plugin {
    private val pending = java.util.concurrent.ConcurrentHashMap<Long, String>()
    override fun onIncoming(ctx: Context, laptop: String, transfer: Long, name: String, size: Long, type: String, body: JSONObject): Boolean {
        if (type != "kdeconnect.share.request") return false
        if (!Prefs.on(ctx, "files")) { Connect.rejectFile(transfer); return true }
        if (!Prefs.autoAccept(ctx)) {
            pending[transfer] = name
            Notify.info(ctx, transfer.toInt(), "Receive $name?", "Open EchoConnect to accept or decline this file.")
            Connect.noteTransfer(Transfer(transfer, name, size, false, error = "Waiting for your approval"))
        } else receive(ctx, transfer)
        return true
    }
    fun waiting() = pending.toMap()
    fun receive(ctx: Context, id: Long) { pending.remove(id); Connect.acceptFile(id, File(ctx.cacheDir, "received"), this) }
    fun decline(id: Long) { pending.remove(id); Connect.rejectFile(id); Connect.finishTransfer(id, "Declined") }
    override fun onTransferDone(ctx: Context, transfer: Long, path: String?, error: String?) {
        if (path == null || error != null) return
        Connect.publishingTransfer(transfer)
        Work.run {
            val file = File(path)
            val values = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, file.name)
                put(MediaStore.Downloads.RELATIVE_PATH, "Download/EchoConnect")
                put(MediaStore.Downloads.IS_PENDING, 1)
            }
            var uri: Uri? = null
            try {
                uri = ctx.contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values) ?: error("Couldn't create Downloads entry")
                ctx.contentResolver.openOutputStream(uri)!!.use { out -> file.inputStream().use { it.copyTo(out) } }
                ctx.contentResolver.update(uri, ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) }, null, null)
                file.delete()
                Connect.finishTransfer(transfer, uri = uri.toString())
                val open = PendingIntent.getActivity(ctx, transfer.toInt(), Intent(Intent.ACTION_VIEW).setDataAndType(uri, ctx.contentResolver.getType(uri) ?: "*/*").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION), PendingIntent.FLAG_IMMUTABLE)
                Notify.info(ctx, transfer.toInt(), "Received ${file.name}", "Saved to Downloads → EchoConnect", open)
            } catch (e: Exception) { uri?.let { ctx.contentResolver.delete(it, null, null) }; Connect.finishTransfer(transfer, e.message ?: "Could not save file") }
        }
    }
    fun textFromLaptop(ctx: Context, text: String) = app.echoconnect.clipboard.ClipboardSync.fromLaptop(ctx, text, false)
    fun urlFromLaptop(ctx: Context, url: String) {
        val uri = Uri.parse(url)
        if (uri.scheme !in listOf("https", "http")) return
        val tap = PendingIntent.getActivity(ctx, 44, Intent(Intent.ACTION_VIEW, uri), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        Notify.info(ctx, Notify.ID_TEXT, "Open link from laptop", url, tap)
    }
}
