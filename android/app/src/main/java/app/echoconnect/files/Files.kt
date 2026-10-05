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
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "files") || !allowed()) {
            Connect.send(type, JSONObject().put("requestId", body.optString("requestId")).put("error", "Enable Files and All files access on your phone"), laptop)
            return
        }
        Work.run {
            try { when (type) {
                "kdeconnect.sftp.request" -> Connect.startSftp(laptop, listOf(root().path), listOf("Internal storage"))
                "echofiles.files.list" -> {
                    val files = resolve(body.optString("path")).listFiles().orEmpty().filter { runCatching { resolve(it.relativeTo(root()).path) == it.absoluteFile }.getOrDefault(false) }.sortedBy { it.name }
                    val offset = body.optInt("offset").coerceAtLeast(0)
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
