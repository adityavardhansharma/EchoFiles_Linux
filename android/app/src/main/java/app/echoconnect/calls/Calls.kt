package app.echoconnect.calls

import android.Manifest
import android.content.*
import android.media.AudioManager
import android.telephony.*
import android.telecom.TelecomManager
import app.echoconnect.*
import app.echoconnect.contacts.Contacts
import org.json.JSONObject

object Calls : Plugin {
    override val handles = setOf("kdeconnect.telephony.request_mute", "echofiles.call")
    private var observer: PhoneStateListener? = null
    @Suppress("DEPRECATION")
    override fun start(ctx: Context) {
        val manager = ctx.getSystemService(TelephonyManager::class.java)
        if (!Prefs.on(ctx, "calls") || !Work.allowed(ctx, Manifest.permission.READ_PHONE_STATE)) {
            observer?.let { manager.listen(it, PhoneStateListener.LISTEN_NONE) }; observer = null; Connect.setCall(null); return
        }
        if (observer != null) return
        observer = object : PhoneStateListener() {
            override fun onCallStateChanged(state: Int, number: String?) {
                if (!Prefs.on(ctx, "calls")) return
                val n = number.orEmpty(); val who = Contacts.name(ctx, n)
                val event = when (state) { TelephonyManager.CALL_STATE_RINGING -> "ringing"; TelephonyManager.CALL_STATE_OFFHOOK -> "talking"; else -> "talking" }
                Connect.send("kdeconnect.telephony", JSONObject().put("event", event).put("phoneNumber", n).put("contactName", who).put("isCancel", state == TelephonyManager.CALL_STATE_IDLE))
                Connect.setCall(if (state == TelephonyManager.CALL_STATE_IDLE) null else Call(who, n, System.currentTimeMillis(), false))
            }
        }.also { manager.listen(it, PhoneStateListener.LISTEN_CALL_STATE) }
    }
    override fun onReady(ctx: Context, laptop: String) { Work.main.post { start(ctx) } }
    @Suppress("DEPRECATION")
    @android.annotation.SuppressLint("MissingPermission") // Each operation below checks its runtime permission.
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "calls")) return
        Work.main.post {
            if (body.has("onLaptop")) Connect.state.value.call?.let { Connect.setCall(it.copy(onLaptop = body.optBoolean("onLaptop"))) }
            val telecom = ctx.getSystemService(TelecomManager::class.java)
            when (if (type.endsWith("request_mute")) "mute" else body.optString("action")) {
                "mute" -> { val audio = ctx.getSystemService(AudioManager::class.java); runCatching { audio.adjustStreamVolume(AudioManager.STREAM_RING, AudioManager.ADJUST_MUTE, 0) } }
                "answer" -> if (Work.allowed(ctx, Manifest.permission.ANSWER_PHONE_CALLS)) telecom.acceptRingingCall()
                "hangup" -> if (Work.allowed(ctx, Manifest.permission.ANSWER_PHONE_CALLS)) telecom.endCall()
                "phone" -> usePhone(ctx)
            }
        }
    }
    @Suppress("DEPRECATION") fun usePhone(ctx: Context) {
        Connect.send("echofiles.call", JSONObject().put("action", "phone"))
        Connect.state.value.call?.let { Connect.setCall(it.copy(onLaptop = false)) }
    }
}
