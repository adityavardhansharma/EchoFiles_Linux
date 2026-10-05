package app.echoconnect

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import app.echoconnect.calls.Calls
import app.echoconnect.ring.Ring
import org.json.JSONObject

/** Buttons on EchoConnect's notifications that don't need a screen. */
class NotificationActions : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        when (intent.action) {
            LOCK -> Connect.send("echofiles.lock", JSONObject())
            STOP_RING -> Ring.stop(ctx)
            USE_PHONE -> Calls.usePhone(ctx)
        }
    }

    companion object {
        const val LOCK = "app.echoconnect.LOCK"
        const val STOP_RING = "app.echoconnect.STOP_RING"
        const val USE_PHONE = "app.echoconnect.USE_PHONE"
    }
}
