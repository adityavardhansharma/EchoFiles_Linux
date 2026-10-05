package app.echoconnect

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import app.echoconnect.ui.MainActivity

/** EchoConnect's own notifications: the quiet Connected one, pairing, files, ringing. */
object Notify {
    const val SERVICE = "connected"
    const val EVENTS = "events"
    const val RING = "ring-alert"
    const val ID_SERVICE = 1
    const val ID_PAIR = 2
    const val ID_RING = 3
    const val ID_TEXT = 4

    fun channels(ctx: Context) {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(SERVICE, "Connected to laptop", NotificationManager.IMPORTANCE_MIN).apply {
            description = "The small notification while EchoConnect stays connected."
            setShowBadge(false)
        })
        nm.createNotificationChannel(NotificationChannel(EVENTS, "Pairing and files", NotificationManager.IMPORTANCE_DEFAULT))
        nm.createNotificationChannel(NotificationChannel(RING, "Ringing", NotificationManager.IMPORTANCE_HIGH).apply {
            setSound(android.media.RingtoneManager.getDefaultUri(android.media.RingtoneManager.TYPE_ALARM), android.media.AudioAttributes.Builder().setUsage(android.media.AudioAttributes.USAGE_ALARM).build())
            if (nm.isNotificationPolicyAccessGranted) setBypassDnd(true)
            enableVibration(true)
            vibrationPattern = longArrayOf(0, 700, 500)
            description = "Full screen when the laptop rings this phone. Allow override Do Not Disturb in this channel's settings."
        })
    }

    fun openApp(ctx: Context): PendingIntent =
        PendingIntent.getActivity(ctx, 0, Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK), PendingIntent.FLAG_IMMUTABLE)

    fun action(ctx: Context, what: String, code: Int): PendingIntent =
        PendingIntent.getBroadcast(ctx, code, Intent(ctx, NotificationActions::class.java).setAction(what), PendingIntent.FLAG_IMMUTABLE)

    fun pairRequest(ctx: Context, name: String, code: String) {
        val n = Notification.Builder(ctx, EVENTS)
            .setSmallIcon(R.drawable.ic_laptop)
            .setContentTitle("$name wants to pair")
            .setContentText("Check the laptop shows ${code.chunked(2).joinToString(" ")}")
            .setContentIntent(openApp(ctx))
            .setAutoCancel(true)
            .build()
        ctx.getSystemService(NotificationManager::class.java).notify(ID_PAIR, n)
    }

    fun info(ctx: Context, id: Int, title: String, text: String, tap: PendingIntent? = null) {
        val n = Notification.Builder(ctx, EVENTS)
            .setSmallIcon(R.drawable.ic_laptop)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(Notification.BigTextStyle().bigText(text))
            .setContentIntent(tap ?: openApp(ctx))
            .setAutoCancel(true)
            .build()
        ctx.getSystemService(NotificationManager::class.java).notify(id, n)
    }
}
