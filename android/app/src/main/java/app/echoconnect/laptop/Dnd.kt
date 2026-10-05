package app.echoconnect.laptop

import android.app.NotificationManager
import android.content.*
import app.echoconnect.*
import org.json.JSONObject

object Dnd : Plugin {
    override fun start(ctx: Context) {
        ctx.registerReceiver(object : BroadcastReceiver() {
            override fun onReceive(c: Context, i: Intent) {
                if (Prefs.on(c, "dnd", false)) report(c)
            }
        }, IntentFilter(NotificationManager.ACTION_INTERRUPTION_FILTER_CHANGED))
    }
    override fun onReady(ctx: Context, laptop: String) { if (Prefs.on(ctx, "dnd", false)) report(ctx) }
    private fun report(ctx: Context) {
        val filter = ctx.getSystemService(NotificationManager::class.java).currentInterruptionFilter
        Connect.send("echofiles.dnd", JSONObject().put("enabled", filter != NotificationManager.INTERRUPTION_FILTER_ALL && filter != NotificationManager.INTERRUPTION_FILTER_UNKNOWN))
    }
}
