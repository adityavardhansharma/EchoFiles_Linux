package app.echoconnect.battery

import android.content.*
import android.os.BatteryManager
import app.echoconnect.*
import org.json.JSONObject

object Battery : Plugin {
    override val handles = setOf("kdeconnect.battery.request")
    override fun start(ctx: Context) {
        ctx.registerReceiver(object : BroadcastReceiver() {
            override fun onReceive(c: Context, i: Intent) { Connect.laptopId?.let { report(c, it) } }
        }, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
    }
    override fun onReady(ctx: Context, laptop: String) = report(ctx, laptop)
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) = report(ctx, laptop)
    private fun report(ctx: Context, id: String) {
        if (!Prefs.on(ctx, "battery")) return
        val i = ctx.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED)) ?: return
        val level = i.getIntExtra(BatteryManager.EXTRA_LEVEL, 0) * 100 / i.getIntExtra(BatteryManager.EXTRA_SCALE, 100).coerceAtLeast(1)
        Connect.send("kdeconnect.battery", JSONObject().put("currentCharge", level).put("isCharging", i.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0).put("thresholdEvent", if (level <= 15) 1 else 0), id)
    }
}
