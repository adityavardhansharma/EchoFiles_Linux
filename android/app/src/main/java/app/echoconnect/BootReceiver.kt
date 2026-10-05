package app.echoconnect

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Back after a restart or an update. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        if (intent.action !in setOf(Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED)) return
        if (Prefs.pairedOnce(ctx)) ConnectService.start(ctx)
    }
}
