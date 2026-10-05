package app.echoconnect.messages

import android.Manifest
import android.content.*
import android.database.ContentObserver
import android.net.Uri
import android.provider.Telephony
import android.telephony.SmsManager
import app.echoconnect.*
import org.json.JSONObject
import org.json.JSONArray

object Messages : Plugin {
    override val handles = setOf("kdeconnect.sms.request", "kdeconnect.sms.request_conversations", "kdeconnect.sms.request_conversation", "kdeconnect.sms.request_attachment")
    private var observer: ContentObserver? = null
    @Synchronized override fun start(ctx: Context) {
        if (!Prefs.on(ctx, "sms") || !Work.allowed(ctx, Manifest.permission.READ_SMS)) {
            observer?.let { ctx.contentResolver.unregisterContentObserver(it) }; observer = null; return
        }
        if (observer != null) return
        observer = object : ContentObserver(Work.main) {
            override fun onChange(self: Boolean) { Connect.laptopId?.let { onReady(ctx, it) } }
        }.also { ctx.contentResolver.registerContentObserver(Uri.parse("content://mms-sms"), true, it) }
    }
    override fun onReady(ctx: Context, laptop: String) { if (Prefs.on(ctx, "sms") && Work.allowed(ctx, Manifest.permission.READ_SMS)) Work.run { list(ctx, laptop, null, 100) } }
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "sms")) return
        Work.run {
            when (type) {
                "kdeconnect.sms.request_conversations" -> if (Work.allowed(ctx, Manifest.permission.READ_SMS)) list(ctx, laptop, null, 100)
                "kdeconnect.sms.request_conversation" -> if (Work.allowed(ctx, Manifest.permission.READ_SMS)) list(ctx, laptop, body.optLong("threadID"), body.optInt("numberOfRequest", body.optInt("numberToRequest", 100)).coerceIn(1, 500))
                "kdeconnect.sms.request" -> if (Work.allowed(ctx, Manifest.permission.SEND_SMS)) {
                    val recipients = body.optJSONArray("addresses") ?: JSONArray().put(JSONObject().put("address", body.optString("phoneNumber")))
                    val text = body.optString("messageBody").take(10000)
                    if (text.isNotEmpty()) for (i in 0 until recipients.length().coerceAtMost(20)) {
                        val number = recipients.getJSONObject(i).optString("address")
                        if (number.isNotBlank()) {
                            val manager = ctx.getSystemService(SmsManager::class.java)
                            manager.sendMultipartTextMessage(number, null, manager.divideMessage(text), null, null)
                        }
                    }
                }
                "kdeconnect.sms.request_attachment" -> if (Work.allowed(ctx, Manifest.permission.READ_SMS)) {
                    val id = body.optString("partID", body.optString("uniqueIdentifier"))
                    require(id.matches(Regex("[0-9]+")))
                    Connect.sendUri(Uri.parse("content://mms/part/$id"), "kdeconnect.sms.attachment_file", body)
                }
            }
        }
    }
    private fun list(ctx: Context, laptop: String, thread: Long?, limit: Int) {
        val messages = JSONArray(); val seen = HashSet<Long>()
        val cols = arrayOf("_id", "thread_id", "address", "body", "date", "type", "read")
        ctx.contentResolver.query(Telephony.Sms.CONTENT_URI, cols, if (thread != null) "thread_id=?" else null, thread?.let { arrayOf(it.toString()) }, "date DESC")?.use { c ->
            while (c.moveToNext() && messages.length() < limit) {
                val tid = c.getLong(1)
                if (thread == null && !seen.add(tid)) continue
                messages.put(JSONObject().put("_id", c.getLong(0)).put("thread_id", tid).put("addresses", JSONArray().put(JSONObject().put("address", c.getString(2)))).put("body", c.getString(3)).put("date", c.getLong(4)).put("type", c.getInt(5)).put("read", c.getInt(6)).put("event", 1))
            }
        }
        // MMS uses seconds for dates and keeps media in the part provider.
        ctx.contentResolver.query(Uri.parse("content://mms"), arrayOf("_id", "thread_id", "date", "msg_box", "read"), if (thread != null) "thread_id=?" else null, thread?.let { arrayOf(it.toString()) }, "date DESC")?.use { c ->
            var count = 0
            while (c.moveToNext() && count++ < limit) {
                val id = c.getLong(0); val tid = c.getLong(1); val parts = JSONArray(); val text = StringBuilder(); val addresses = JSONArray()
                ctx.contentResolver.query(Uri.parse("content://mms/$id/addr"), arrayOf("address", "type"), null, null, null)?.use { a -> while (a.moveToNext()) { val address = a.getString(0); if (address != "insert-address-token") addresses.put(JSONObject().put("address", address)) } }
                ctx.contentResolver.query(Uri.parse("content://mms/part"), arrayOf("_id", "ct", "text", "name"), "mid=?", arrayOf(id.toString()), null)?.use { p ->
                    while (p.moveToNext()) {
                        val mime = p.getString(1).orEmpty()
                        if (mime == "text/plain") text.append(p.getString(2).orEmpty())
                        else if (mime != "application/smil") parts.put(JSONObject().put("partID", p.getString(0)).put("uniqueIdentifier", p.getString(0)).put("mimeType", mime).put("filename", p.getString(3) ?: "attachment-${p.getLong(0)}"))
                    }
                }
                messages.put(JSONObject().put("_id", -id).put("thread_id", tid).put("addresses", addresses).put("body", text.toString()).put("date", c.getLong(2)*1000).put("type", if (c.getInt(3) == 2) 2 else 1).put("read", c.getInt(4)).put("event", 1).put("attachments", parts))
            }
        }
        Connect.send("kdeconnect.sms.messages", JSONObject().put("version", 2).put("messages", messages), laptop)
    }
}
class SmsReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        if (intent.action != android.provider.Telephony.Sms.Intents.SMS_RECEIVED_ACTION) return
        Connect.laptopId?.let { Messages.onReady(ctx, it) }
    }
}
