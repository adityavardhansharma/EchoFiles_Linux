package app.echoconnect.photos

import android.content.*
import android.provider.MediaStore
import android.graphics.Bitmap
import android.util.Size
import androidx.work.*
import app.echoconnect.*
import org.json.JSONObject
import org.json.JSONArray
import java.io.ByteArrayOutputStream
import java.util.concurrent.TimeUnit

object Photos : Plugin {
    fun fullAccess(ctx: Context): Boolean = if (android.os.Build.VERSION.SDK_INT >= 33)
        Work.allowed(ctx, android.Manifest.permission.READ_MEDIA_IMAGES) && Work.allowed(ctx, android.Manifest.permission.READ_MEDIA_VIDEO)
        else Work.allowed(ctx, android.Manifest.permission.READ_EXTERNAL_STORAGE)
    fun hasAccess(ctx: Context): Boolean = fullAccess(ctx) ||
        (android.os.Build.VERSION.SDK_INT >= 33 && (Work.allowed(ctx, android.Manifest.permission.READ_MEDIA_IMAGES) || Work.allowed(ctx, android.Manifest.permission.READ_MEDIA_VIDEO))) ||
        (android.os.Build.VERSION.SDK_INT >= 34 && Work.allowed(ctx, android.Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED))
    override val handles = setOf("echofiles.photos.request", "echofiles.photos.thumb.request", "echofiles.photos.read", "echofiles.backup.ack")
    private val acknowledgements = java.util.concurrent.ConcurrentHashMap<String, Boolean>()
    fun takeAck(id: String): Boolean? = acknowledgements.remove(id)
    private val pendingBackup = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
    fun expectAck(id: String) { pendingBackup.add(id) }
    fun forgetAck(id: String) { pendingBackup.remove(id); acknowledgements.remove(id) }
    override fun start(ctx: Context) {
        val constraints = Constraints.Builder().setRequiredNetworkType(NetworkType.UNMETERED).setRequiresCharging(true).build()
        WorkManager.getInstance(ctx).enqueueUniquePeriodicWork("camera-backup", ExistingPeriodicWorkPolicy.KEEP,
            PeriodicWorkRequestBuilder<BackupWorker>(15, TimeUnit.MINUTES).setConstraints(constraints).build())
    }
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (type == "echofiles.backup.ack") {
            val request = body.optString("requestId")
            if (pendingBackup.contains(request)) acknowledgements[request] = body.optBoolean("saved")
            return
        }
        if (!Prefs.on(ctx, "photos")) {
            Connect.send("echofiles.photos.error", JSONObject().put("requestId", body.optString("requestId")).put("id", body.optLong("id", -1)).put("message", "Enable Photos on your phone"), laptop)
            return
        }
        Work.run {
            try { if (type == "echofiles.photos.request") {
                val before = body.optLong("before", Long.MAX_VALUE)
                val beforeId = body.optLong("beforeId", Long.MAX_VALUE)
                val entries = JSONArray()
                ctx.contentResolver.query(MediaStore.Files.getContentUri("external"), arrayOf("_id", "_display_name", "date_added", "_size", "relative_path", "media_type"), "media_type IN (1,3) AND (date_added < ? OR (date_added = ? AND _id < ?))", arrayOf(before.toString(), before.toString(), beforeId.toString()), "date_added DESC, _id DESC")?.use { c ->
                    while (c.moveToNext() && entries.length() < 100) entries.put(JSONObject().put("id", c.getLong(0)).put("name", c.getString(1)).put("date", c.getLong(2)).put("size", c.getLong(3)).put("folder", c.getString(4)).put("video", c.getInt(5) == 3))
                }
                Connect.send("echofiles.photos", JSONObject().put("requestId", body.optString("requestId")).put("photos", entries).put("more", entries.length() == 100), laptop)
            } else {
                val uri = ContentUris.withAppendedId(MediaStore.Files.getContentUri("external"), body.getLong("id"))
                if (type.endsWith("thumb.request")) {
                    val bitmap = ctx.contentResolver.loadThumbnail(uri, Size(256, 256), null)
                    val out = ByteArrayOutputStream(); bitmap.compress(Bitmap.CompressFormat.JPEG, 80, out); bitmap.recycle()
                    Connect.sendBytes("echofiles.photos.thumb", JSONObject().put("id", body.getLong("id")).put("filename", "${body.getLong("id")}.jpg"), out.toByteArray())
                } else Connect.sendUri(uri, "echofiles.photos.file", body)
            } } catch (e: Exception) {
                Connect.send("echofiles.photos.error", JSONObject().put("requestId", body.optString("requestId")).put("id", body.optLong("id", -1)).put("message", e.message ?: "Allow photo access on the phone"), laptop)
            }
        }
    }
}
class BackupWorker(ctx: Context, params: WorkerParameters) : Worker(ctx, params) {
    override fun doWork(): Result {
        if (!Prefs.on(applicationContext, "backup", false) || !Prefs.on(applicationContext, "photos")) return Result.success()
        // A chronological backup cursor must never skip items hidden by partial access.
        if (!Photos.fullAccess(applicationContext)) return Result.retry()
        val laptop = Connect.state.value.laptop?.takeIf { it.lan }?.id ?: return Result.retry()
        val ctx = applicationContext
        val (dateCursor, idCursor) = Prefs.backupCursor(ctx, laptop)
        return try {
            ctx.contentResolver.query(MediaStore.Files.getContentUri("external"), arrayOf("_id", "date_added"), "media_type IN (1,3) AND (date_added > ? OR (date_added = ? AND _id > ?)) AND relative_path LIKE ?", arrayOf(dateCursor.toString(), dateCursor.toString(), idCursor.toString(), "DCIM/%"), "date_added ASC, _id ASC")?.use { c ->
                var count = 0
                while (c.moveToNext() && count++ < 50 && !isStopped) {
                    if (!Prefs.on(ctx, "backup", false) || Connect.laptopId != laptop) return Result.success()
                    val id = c.getLong(0); val date = c.getLong(1)
                    val request = java.util.UUID.randomUUID().toString()
                    Photos.expectAck(request)
                    try {
                        val t = Connect.sendUri(ContentUris.withAppendedId(MediaStore.Files.getContentUri("external"), id), "echofiles.backup", JSONObject().put("photoId", id).put("date", date).put("requestId", request)) ?: return Result.retry()
                        val deadline = android.os.SystemClock.elapsedRealtime() + 120_000
                        var saved = false
                        while (!isStopped && android.os.SystemClock.elapsedRealtime() < deadline) {
                            val ack = Photos.takeAck(request)
                            if (ack != null) { if (!ack) return Result.retry(); saved = true; break }
                            if (Connect.state.value.transfers.firstOrNull { it.id == t }?.error != null) return Result.retry()
                            Thread.sleep(300)
                        }
                        if (!saved || !Prefs.setBackupCursor(ctx, laptop, date, id)) return Result.retry()
                    } finally { Photos.forgetAck(request) }
                }
            } ?: return Result.retry()
            Result.success()
        } catch (_: Exception) { Result.retry() }
    }
}
