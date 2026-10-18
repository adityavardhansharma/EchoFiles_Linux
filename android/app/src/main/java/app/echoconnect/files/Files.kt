package app.echoconnect.files

import android.content.ContentUris
import android.content.Context
import android.graphics.Bitmap
import android.media.ThumbnailUtils
import android.os.Build
import android.provider.MediaStore
import android.util.Size
import android.os.Environment
import app.echoconnect.*
import org.json.JSONObject
import org.json.JSONArray
import java.io.ByteArrayOutputStream
import java.io.File

object Files : Plugin {
    override val handles = setOf("kdeconnect.sftp.request", "echofiles.files.list", "echofiles.files.read", "echofiles.files.write", "echofiles.files.thumb")
    fun allowed() = Build.VERSION.SDK_INT < 30 || Environment.isExternalStorageManager()
    @Suppress("DEPRECATION") private fun root() = Environment.getExternalStorageDirectory().canonicalFile
    private fun resolve(path: String): File {
        val base = root(); val target = File(base, path.removePrefix("/")).canonicalFile
        require(target == base || target.path.startsWith(base.path + "/")) { "Outside shared storage" }
        return target
    }
    private class Snapshot(val dir: File, val stamp: Long, val made: Long, val files: List<File>)
    @Volatile private var snapshot: Snapshot? = null

    /** The sorted entries of [dir]. Later pages reuse the first page's snapshot while the
     *  folder is unchanged, rather than listing, checking and sorting it again per page. */
    private fun listing(dir: File, fresh: Boolean): List<File> {
        val stamp = dir.lastModified()
        if (!fresh) snapshot?.takeIf { it.dir == dir && it.stamp == stamp && System.currentTimeMillis() - it.made < 60_000 }?.let { return it.files }
        val base = root()
        // Only a symlink can lead outside shared storage; plain entries of a resolved folder
        // are already inside it.
        val files = dir.listFiles().orEmpty().filter { f ->
            !java.nio.file.Files.isSymbolicLink(f.toPath()) || runCatching { resolve(f.relativeTo(base).path) == f.absoluteFile }.getOrDefault(false)
        }.sortedBy { it.name }
        snapshot = Snapshot(dir, stamp, System.currentTimeMillis(), files)
        return files
    }

    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "files") || !allowed()) {
            fail(laptop, type, body, "Turn on Files in EchoConnect and allow All files access on the phone.")
            return
        }
        Work.run {
            try { when (type) {
                "kdeconnect.sftp.request" -> Connect.startSftp(laptop, listOf(root().path), listOf("Internal storage"))
                "echofiles.files.list" -> {
                    val offset = body.optInt("offset").coerceAtLeast(0)
                    val files = listing(resolve(body.optString("path")), offset == 0)
                    val entries = JSONArray()
                    files.drop(offset).take(500).forEach { entries.put(JSONObject().put("name", it.name).put("path", it.relativeTo(root()).path).put("directory", it.isDirectory).put("size", it.length()).put("modified", it.lastModified())) }
                    Connect.send("echofiles.files.list", JSONObject().put("requestId", body.optString("requestId")).put("path", body.optString("path")).put("entries", entries).put("next", if (offset + 500 < files.size) offset + 500 else -1), laptop)
                }
                "echofiles.files.read" -> Connect.sendFile(resolve(body.getString("path")), "echofiles.files.read", body)
                "echofiles.files.thumb" -> {
                    val file = resolve(body.getString("path"))
                    val bitmap = thumbnail(ctx, file)
                    val out = ByteArrayOutputStream(); bitmap.compress(Bitmap.CompressFormat.JPEG, 80, out); bitmap.recycle()
                    Connect.sendBytes("echofiles.files.thumb", JSONObject().put("requestId", body.optString("requestId")).put("path", body.getString("path")).put("filename", "thumb.jpg"), out.toByteArray())
                }
            } } catch (error: Exception) {
                fail(laptop, type, body, error.message ?: "Could not read shared storage")
            }
        }
    }

    private val videoExtensions = setOf("mp4", "mov", "3gp", "mkv", "webm")

    /** A small picture of [file] for EchoFiles' grid: Android's own cached thumbnail when
     *  the media library knows the file (no decoding at all), else one made from the file. */
    private fun thumbnail(ctx: Context, file: File): Bitmap {
        val size = Size(256, 256)
        val id = runCatching {
            ctx.contentResolver.query(MediaStore.Files.getContentUri("external"), arrayOf("_id"), "_data = ?", arrayOf(file.path), null)
                ?.use { c -> if (c.moveToFirst()) c.getLong(0) else null }
        }.getOrNull()
        if (id != null) {
            runCatching { ctx.contentResolver.loadThumbnail(ContentUris.withAppendedId(MediaStore.Files.getContentUri("external"), id), size, null) }
                .getOrNull()?.let { return it }
        }
        return if (file.extension.lowercase() in videoExtensions) ThumbnailUtils.createVideoThumbnail(file, size, null)
        else ThumbnailUtils.createImageThumbnail(file, size, null)
    }

    /** Say why a request failed, in the reply EchoFiles waits for: the file server's answer
     *  (`kdeconnect.sftp` with `errorMessage`), or the request's own type with `error`. */
    private fun fail(laptop: String, type: String, body: JSONObject, message: String) {
        if (type == "kdeconnect.sftp.request") {
            Connect.send("kdeconnect.sftp", JSONObject().put("errorMessage", message), laptop)
        } else {
            Connect.send(type, JSONObject().put("requestId", body.optString("requestId")).put("error", message), laptop)
        }
    }
    override fun onIncoming(ctx: Context, laptop: String, transfer: Long, name: String, size: Long, type: String, body: JSONObject): Boolean {
        if (type != "echofiles.files.write") return false
        if (!Prefs.on(ctx, "files") || !allowed()) Connect.rejectFile(transfer)
        else runCatching { Connect.acceptFile(transfer, resolve(body.optString("directory"))) }.onFailure { Connect.rejectFile(transfer) }
        return true
    }
}
