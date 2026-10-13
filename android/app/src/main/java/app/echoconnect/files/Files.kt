package app.echoconnect.files

import android.content.Context
import android.os.Build
import android.os.Environment
import app.echoconnect.*
import org.json.JSONObject
import org.json.JSONArray
import java.io.File

object Files : Plugin {
    override val handles = setOf("kdeconnect.sftp.request", "echofiles.files.list", "echofiles.files.read", "echofiles.files.write")
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
            Connect.send(type, JSONObject().put("requestId", body.optString("requestId")).put("error", "Enable Files and All files access on your phone"), laptop)
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
            } } catch (error: Exception) {
                Connect.send(type, JSONObject().put("requestId", body.optString("requestId")).put("error", error.message ?: "Could not read shared storage"), laptop)
            }
        }
    }
    override fun onIncoming(ctx: Context, laptop: String, transfer: Long, name: String, size: Long, type: String, body: JSONObject): Boolean {
        if (type != "echofiles.files.write") return false
        if (!Prefs.on(ctx, "files") || !allowed()) Connect.rejectFile(transfer)
        else runCatching { Connect.acceptFile(transfer, resolve(body.optString("directory"))) }.onFailure { Connect.rejectFile(transfer) }
        return true
    }
}
