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
    private const val LIMIT = 120_000L
    private var deadline = 0L
    private var wake: PowerManager.WakeLock? = null
    private var since = 0L

    /** Android silences the alarm itself when a volume key is pressed outside RingActivity (unlocked phone,
     *  heads-up only), and never tells us — so any volume key press while ringing stops everything. */
    private val keys = object : BroadcastReceiver() {
        override fun onReceive(c: Context, i: Intent) {
            // Our own jump to full alarm volume broadcasts too; ignore the first moment.
            if (i.action == VOLUME_CHANGED && SystemClock.elapsedRealtime() - since < 1_000) return
            if (ringing) stop(c)
        }
    }
    private const val VOLUME_CHANGED = "android.media.VOLUME_CHANGED_ACTION"
    private var listening = false
    private fun listen(ctx: Context) {
        if (listening) return
        since = SystemClock.elapsedRealtime()
        val filter = IntentFilter(VOLUME_CHANGED)
        runCatching {
            if (Build.VERSION.SDK_INT >= 33) Connect.app.registerReceiver(keys, filter, Context.RECEIVER_NOT_EXPORTED) else Connect.app.registerReceiver(keys, filter)
            listening = true
        }
    }
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
        Work.main.postDelayed(timeout, LIMIT)
        deadline = SystemClock.elapsedRealtime() + LIMIT
        // Without a wake lock the CPU sleeps between blinks: the torch can freeze lit and the timeout never fires.
        wake = ctx.getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "echoconnect:ring").apply { acquire(LIMIT + 5_000) }
        listen(ctx)
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
            if (ringing && SystemClock.elapsedRealtime() > deadline) { stop(Connect.app); return }
            if (!ringing) { if (lit) runCatching { cm.setTorchMode(id, false) }; lit = false; return }
            lit = !lit
            runCatching { cm.setTorchMode(id, lit) }
            Work.main.postDelayed(this, 500)
        }
    }
    fun stop(ctx: Context) {
        ringing = false
        Work.main.removeCallbacks(timeout)
        if (listening) { runCatching { Connect.app.unregisterReceiver(keys) }; listening = false }
        wake?.let { if (it.isHeld) runCatching { it.release() } }; wake = null
        Work.main.removeCallbacks(blink); torch?.let { (cm, id) -> runCatching { cm.setTorchMode(id, false) } }; lit = false
        ctx.getSystemService(Vibrator::class.java).cancel()
        volume?.let { ctx.getSystemService(AudioManager::class.java).setStreamVolume(AudioManager.STREAM_ALARM, it, 0) }; volume = null
        ctx.getSystemService(NotificationManager::class.java).cancel(Notify.ID_RING)
    }
}
