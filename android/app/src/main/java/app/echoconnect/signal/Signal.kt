package app.echoconnect.signal

import android.Manifest
import android.content.Context
import android.telephony.*
import app.echoconnect.*
import org.json.JSONObject

object Signal : Plugin {
    private var observer: PhoneStateListener? = null
    private var strength = 0
    @Suppress("DEPRECATION")
    override fun start(ctx: Context) {
        val manager = ctx.getSystemService(TelephonyManager::class.java)
        if (!Prefs.on(ctx, "signal") || !Work.allowed(ctx, Manifest.permission.READ_PHONE_STATE)) {
            observer?.let { manager.listen(it, PhoneStateListener.LISTEN_NONE) }; observer = null; return
        }
        if (observer != null) return
        observer = object : PhoneStateListener() {
            override fun onSignalStrengthsChanged(signal: SignalStrength) { strength = signal.level; report(ctx) }
            override fun onServiceStateChanged(service: ServiceState) { report(ctx) }
        }.also { manager.listen(it, PhoneStateListener.LISTEN_SIGNAL_STRENGTHS or PhoneStateListener.LISTEN_SERVICE_STATE) }
    }
    override fun onReady(ctx: Context, laptop: String) { if (Prefs.on(ctx, "signal")) report(ctx) }
    private fun report(ctx: Context) {
        if (!Prefs.on(ctx, "signal")) return
        val manager = ctx.getSystemService(TelephonyManager::class.java)
        Connect.send("kdeconnect.connectivity_report", JSONObject().put("signalStrengths", JSONObject().put("0", JSONObject().put("networkType", manager.networkOperatorName.ifBlank { "Mobile" }).put("signalStrength", strength))))
    }
}
