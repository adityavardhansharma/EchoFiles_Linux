package app.echoconnect.ring

import android.app.*
import android.content.*
import android.media.*
import android.os.*
import app.echoconnect.*
import app.echoconnect.ui.RingActivity
import org.json.JSONObject

object Ring : Plugin {
    override val handles = setOf("kdeconnect.findmyphone.request")
    private var ringing = false
    private val timeout = Runnable { stop(Connect.app) }
    private var volume: Int? = null
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "ring")) return
        Work.main.post { if (ringing) stop(ctx) else startRing(ctx) }
    }
    private fun startRing(ctx: Context) {
        val audio = ctx.getSystemService(AudioManager::class.java)
        volume = audio.getStreamVolume(AudioManager.STREAM_ALARM)
        audio.setStreamVolume(AudioManager.STREAM_ALARM, audio.getStreamMaxVolume(AudioManager.STREAM_ALARM), 0)
        ringing = true
        Notify.channels(ctx)
        val intent = PendingIntent.getActivity(ctx, 30, Intent(ctx, RingActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK), PendingIntent.FLAG_IMMUTABLE)
        ctx.getSystemService(NotificationManager::class.java).notify(Notify.ID_RING, Notification.Builder(ctx, Notify.RING)
            .setSmallIcon(R.drawable.ic_phone_ring).setContentTitle("Your laptop is ringing this phone")
            .setOngoing(true).setCategory(Notification.CATEGORY_ALARM).setContentIntent(intent).setFullScreenIntent(intent, true)
            .addAction(Notification.Action.Builder(null, "Stop ringing", Notify.action(ctx, NotificationActions.STOP_RING, 31)).build()).build().apply { flags = flags or Notification.FLAG_INSISTENT })
        Work.main.removeCallbacks(timeout)
        Work.main.postDelayed(timeout, 120_000)
        torch = ctx.getSystemService(android.hardware.camera2.CameraManager::class.java).let { cm ->
            runCatching { cm.cameraIdList.firstOrNull { cm.getCameraCharacteristics(it).get(android.hardware.camera2.CameraCharacteristics.FLASH_INFO_AVAILABLE) == true } }.getOrNull()?.let { cm to it }
        }
        Work.main.removeCallbacks(blink); Work.main.post(blink)
    }

    /** The flashlight blinks while ringing, so the phone is easy to spot in the dark. */
    private var torch: Pair<android.hardware.camera2.CameraManager, String>? = null
    private var lit = false
    private val blink: Runnable = object : Runnable {
        override fun run() {
            val (cm, id) = torch ?: return
            if (!ringing) { if (lit) runCatching { cm.setTorchMode(id, false) }; lit = false; return }
            lit = !lit
            runCatching { cm.setTorchMode(id, lit) }
            Work.main.postDelayed(this, 500)
        }
    }
    fun stop(ctx: Context) {
        ringing = false
        Work.main.removeCallbacks(timeout)
        Work.main.removeCallbacks(blink); torch?.let { (cm, id) -> runCatching { cm.setTorchMode(id, false) } }; lit = false
        ctx.getSystemService(Vibrator::class.java).cancel()
        volume?.let { ctx.getSystemService(AudioManager::class.java).setStreamVolume(AudioManager.STREAM_ALARM, it, 0) }; volume = null
        ctx.getSystemService(NotificationManager::class.java).cancel(Notify.ID_RING)
    }
}
