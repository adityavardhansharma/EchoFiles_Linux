package app.echoconnect.notifications

import android.app.Notification
import android.app.RemoteInput
import android.content.*
import android.service.notification.*
import app.echoconnect.*
import org.json.JSONObject
import org.json.JSONArray

object Notifications : Plugin {
    @Volatile var listener: NotificationListener? = null
    override val handles = setOf("kdeconnect.notification.request", "kdeconnect.notification.reply", "kdeconnect.notification.action")
    override fun onReady(ctx: Context, laptop: String) { if (!Prefs.on(ctx, "notifications")) return; listener?.activeNotifications?.forEach { publish(ctx, it, true) } }
    fun publish(ctx: Context, n: StatusBarNotification, silent: Boolean = false) {
        if (!Prefs.on(ctx, "notifications") || n.packageName == ctx.packageName || n.packageName in Prefs.mutedApps(ctx)) return
        val notification = n.notification
        if (notification.flags and Notification.FLAG_GROUP_SUMMARY != 0) return
        val extras = notification.extras
        val title = extras.getCharSequence(Notification.EXTRA_TITLE)?.toString().orEmpty()
        val text = (extras.getCharSequence(Notification.EXTRA_BIG_TEXT) ?: extras.getCharSequence(Notification.EXTRA_TEXT))?.toString().orEmpty()
        val app = runCatching { ctx.packageManager.getApplicationLabel(ctx.packageManager.getApplicationInfo(n.packageName, 0)).toString() }.getOrDefault(n.packageName)
        val actions = JSONArray()
        notification.actions.orEmpty().forEach { if (it.remoteInputs.isNullOrEmpty()) actions.put(it.title.toString()) }
        val b = JSONObject().put("id", n.key).put("appName", app).put("title", title).put("text", text).put("ticker", "$title: $text").put("time", n.postTime.toString()).put("isClearable", n.isClearable).put("silent", silent).put("actions", actions)
        if (notification.actions.orEmpty().any { !it.remoteInputs.isNullOrEmpty() }) b.put("requestReplyId", n.key)
        Connect.send("kdeconnect.notification", b)
    }
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "notifications")) return
        val l = listener ?: return
        when (type) {
            "kdeconnect.notification.request" -> {
                if (body.optBoolean("request")) onReady(ctx, laptop)
                if (body.has("cancel")) l.cancelNotification(body.getString("cancel"))
            }
            "kdeconnect.notification.reply" -> {
                val n = l.activeNotifications.firstOrNull { it.key == body.optString("requestReplyId") } ?: return
                val a = n.notification.actions.orEmpty().firstOrNull { !it.remoteInputs.isNullOrEmpty() } ?: return
                val reply = Intent()
                val bundle = android.os.Bundle()
                a.remoteInputs.forEach { bundle.putCharSequence(it.resultKey, body.optString("message")) }
                RemoteInput.addResultsToIntent(a.remoteInputs, reply, bundle)
                a.actionIntent.send(ctx, 0, reply)
            }
            "kdeconnect.notification.action" -> {
                val n = l.activeNotifications.firstOrNull { it.key == body.optString("key") } ?: return
                n.notification.actions.orEmpty().firstOrNull { it.title.toString() == body.optString("action") }?.actionIntent?.send()
            }
        }
    }
}
class NotificationListener : NotificationListenerService() {
    override fun onListenerConnected() { Notifications.listener = this; Connect.laptopId?.let { Notifications.onReady(this, it) } }
    override fun onListenerDisconnected() { Notifications.listener = null }
    override fun onNotificationPosted(sbn: StatusBarNotification) { Notifications.publish(this, sbn) }
    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        if (Prefs.on(this, "notifications")) Connect.send("kdeconnect.notification", JSONObject().put("id", sbn.key).put("isCancel", true))
    }
}
