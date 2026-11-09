package app.echoconnect.scribe

import android.Manifest
import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.media.audiofx.NoiseSuppressor
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.util.Base64
import android.util.Log
import androidx.activity.ComponentActivity
import app.echoconnect.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import org.json.JSONObject
import kotlin.math.log10
import kotlin.math.sqrt

/**
 * Dictation on the laptop with this phone's microphone, through EchoScribe.
 *
 * Packets ride the EchoFiles link (`echofiles.scribe` for control, `echofiles.scribe.audio` for
 * 200 ms chunks of 16 kHz mono PCM, base64); EchoFiles hands them to EchoScribe on the laptop,
 * which types the result wherever the laptop's cursor is. Two ways in:
 * - the phone starts it (the Dictate screen, the Quick Settings tile): [start] / [stop];
 * - the laptop asks for it (Phone picked as EchoScribe's microphone, Right Ctrl pressed): a
 *   `capture` packet starts the mic, `release` stops it.
 */
object Scribe : Plugin {
    const val CONTROL = "echofiles.scribe"
    const val AUDIO = "echofiles.scribe.audio"
    private const val TAG = "EchoConnect"
    private const val RATE = 16_000
    /** 200 ms per packet: few enough packets, short enough that the laptop's meter moves. */
    private const val CHUNK = RATE / 5

    /** What EchoScribe on the laptop says, plus this phone's side. */
    data class State(
        /** EchoScribe answered (it's running and the link works). */
        val available: Boolean = false,
        val asked: Boolean = false,
        /** idle, listening, paused, transcribing, cleaning, working, done, error, notice, busy. */
        val phase: String = "idle",
        val text: String = "",
        val live: String = "",
        val error: String = "",
        val detail: String = "",
        val app: String = "",
        val persona: String = "",
        val hinglish: Boolean = false,
        val code: String = "auto",
        /** This phone's mic is open. */
        val recording: Boolean = false,
        /** The laptop asked for the mic rather than the phone starting. */
        val laptopAsked: Boolean = false,
        val mode: String = "dictate",
        val level: Float = 0f,
        val history: List<String> = emptyList(),
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state

    override val handles = setOf(CONTROL)

    @Volatile private var session = 0L
    @Volatile private var recorder: Thread? = null
    @Volatile private var running = false

    override fun onReady(ctx: Context, laptop: String) {
        if (!Prefs.on(ctx, "dictation")) return
        _state.update { it.copy(asked = true) }
        Connect.send(CONTROL, JSONObject().put("op", "hello"), laptop)
    }

    /** Ask EchoScribe for its state (when the Dictate screen opens). */
    fun hello() {
        _state.update { it.copy(asked = true) }
        Connect.send(CONTROL, JSONObject().put("op", "hello"))
    }

    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        when (body.optString("op")) {
            "state" -> {
                val phase = body.optString("phase", "idle")
                _state.update { s ->
                    s.copy(
                        available = body.optBoolean("available", true),
                        phase = phase,
                        text = if (phase == "done") body.optString("text") else s.text,
                        error = body.optString("error"),
                        detail = body.optString("detail"),
                        app = body.optString("app", s.app),
                        persona = body.optString("persona", s.persona),
                        hinglish = body.optBoolean("hinglish", s.hinglish),
                        code = body.optString("code", s.code),
                        live = if (phase == "listening" || phase == "paused") s.live else "",
                        history = if (phase == "done" && body.optString("text").isNotBlank()) (listOf(body.optString("text")) + s.history).take(20) else s.history,
                    )
                }
                // The laptop finished or gave up: a phone-started mic has nothing more to do.
                if (phase in setOf("done", "error") && running && !_state.value.laptopAsked) stopMic()
            }
            "live" -> _state.update { it.copy(live = body.optString("text")) }
            "capture" -> {
                if (!Prefs.on(ctx, "dictation")) return
                val id = body.optLong("session")
                Work.main.post { startFromLaptop(ctx, id) }
            }
            "release" -> {
                if (body.optLong("session") == session || session == 0L) stopMic()
                ctx.getSystemService(NotificationManager::class.java).cancel(ID_ASK)
            }
        }
    }

    // ------------------------------------------------------------------ actions

    fun canRecord(ctx: Context) = Work.allowed(ctx, Manifest.permission.RECORD_AUDIO)

    /** Start a dictation from the phone. [mode]: dictate, edit (change what's selected on the laptop), note. */
    fun start(ctx: Context, mode: String = "dictate"): Boolean {
        if (running || !canRecord(ctx) || Connect.laptopId == null) return false
        session = System.currentTimeMillis()
        _state.update { it.copy(mode = mode, laptopAsked = false, text = "", live = "", error = "") }
        if (!Connect.send(CONTROL, JSONObject().put("op", "start").put("session", session).put("mode", mode))) return false
        return openMic(ctx)
    }

    /** Send what was said. */
    fun stop() {
        if (!running) return
        val id = session
        stopMic()
        Connect.send(CONTROL, JSONObject().put("op", "stop").put("session", id))
    }

    fun cancel() {
        stopMic()
        Connect.send(CONTROL, JSONObject().put("op", "cancel").put("session", session))
    }

    fun setMode(m: String) = _state.update { it.copy(mode = m) }

    fun command(op: String, extra: JSONObject.() -> Unit = {}) = Connect.send(CONTROL, JSONObject().put("op", op).apply(extra))

    private fun startFromLaptop(ctx: Context, id: Long) {
        if (running) stopMic()
        session = id
        _state.update { it.copy(laptopAsked = true, mode = "dictate", text = "", live = "") }
        if (!canRecord(ctx) || !openMic(ctx)) {
            // Android won't open the mic from the background: ask on the phone.
            askToStart(ctx)
        }
    }

    // ------------------------------------------------------------------ microphone

    private fun openMic(ctx: Context): Boolean {
        // A microphone-type foreground service keeps capture alive with the screen off; Android
        // only lets us start one while the app (or its tile, or a notification tap) is in front.
        val started = runCatching {
            ctx.startForegroundService(Intent(ctx, MicService::class.java))
        }.isSuccess
        if (!started) return false
        running = true
        _state.update { it.copy(recording = true) }
        recorder = Thread({ capture() }, "echoconnect-mic").apply { start() }
        return true
    }

    @SuppressLint("MissingPermission") // canRecord() is checked before every openMic().
    private fun capture() {
        val min = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val rec = runCatching {
            AudioRecord(MediaRecorder.AudioSource.VOICE_RECOGNITION, RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, maxOf(min, CHUNK * 4))
        }.getOrNull()
        if (rec == null || rec.state != AudioRecord.STATE_INITIALIZED) {
            Log.w(TAG, "mic unavailable")
            rec?.release()
            _state.update { it.copy(recording = false, error = "The microphone is busy or blocked") }
            running = false
            Connect.app.stopService(Intent(Connect.app, MicService::class.java))
            return
        }
        val ns = if (NoiseSuppressor.isAvailable()) runCatching { NoiseSuppressor.create(rec.audioSessionId)?.apply { enabled = true } }.getOrNull() else null
        val buf = ShortArray(CHUNK)
        val bytes = ByteArray(CHUNK * 2)
        var seq = 0
        val id = session
        rec.startRecording()
        try {
            while (running && session == id) {
                var got = 0
                while (got < CHUNK && running) {
                    val n = rec.read(buf, got, CHUNK - got)
                    if (n <= 0) break
                    got += n
                }
                if (got <= 0) break
                var sum = 0.0
                for (i in 0 until got) {
                    val v = buf[i].toInt()
                    bytes[i * 2] = (v and 0xff).toByte()
                    bytes[i * 2 + 1] = ((v shr 8) and 0xff).toByte()
                    sum += (v / 32768.0) * (v / 32768.0)
                }
                val db = 20 * log10(sqrt(sum / got).coerceAtLeast(1e-6))
                _state.update { it.copy(level = ((db + 55) / 50).toFloat().coerceIn(0f, 1f)) }
                val pcm = Base64.encodeToString(bytes, 0, got * 2, Base64.NO_WRAP)
                Connect.send(AUDIO, JSONObject().put("op", "audio").put("session", id).put("seq", seq++).put("pcm", pcm))
            }
        } finally {
            runCatching { rec.stop() }
            ns?.release()
            rec.release()
            _state.update { it.copy(recording = false, level = 0f) }
        }
    }

    /** Closes the mic; waits briefly so the last chunk is sent before any `stop`. */
    private fun stopMic() {
        running = false
        val t = recorder
        recorder = null
        if (t != null && t != Thread.currentThread()) runCatching { t.join(400) }
        Connect.app.stopService(Intent(Connect.app, MicService::class.java))
        _state.update { it.copy(recording = false, level = 0f) }
    }

    // ------------------------------------------------------------------ asking from the background

    const val ID_ASK = 7
    const val EXTRA_SESSION = "session"

    private fun askToStart(ctx: Context) {
        Notify.channels(ctx)
        val tap = PendingIntent.getActivity(ctx, 70, Intent(ctx, MicActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK).putExtra(EXTRA_SESSION, session), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val laptop = Connect.state.value.laptop?.name ?: "Your laptop"
        val n = Notification.Builder(ctx, Notify.EVENTS)
            .setSmallIcon(R.drawable.ic_mic)
            .setContentTitle("$laptop wants this phone's microphone")
            .setContentText(if (canRecord(ctx)) "Tap to start dictating" else "Tap to allow the microphone")
            .setContentIntent(tap)
            .setAutoCancel(true)
            .setTimeoutAfter(30_000)
            .build()
        ctx.getSystemService(NotificationManager::class.java).notify(ID_ASK, n)
    }

    /** From the notification or the tile: in front for a moment, which lets the mic start. */
    internal fun startFromActivity(ctx: Context, laptopSession: Long) {
        if (laptopSession != 0L) {
            session = laptopSession
            _state.update { it.copy(laptopAsked = true) }
            openMic(ctx)
        } else if (running) stop() else start(ctx)
    }
}

/** Keeps the microphone open with the screen off (Android requires a visible notification). */
class MicService : Service() {
    private var wake: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val laptop = Connect.state.value.laptop?.name ?: "your laptop"
        val stop = PendingIntent.getActivity(this, 71, Intent(this, MicActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK), PendingIntent.FLAG_IMMUTABLE)
        val n = Notification.Builder(this, Notify.SERVICE)
            .setSmallIcon(R.drawable.ic_mic)
            .setContentTitle("Dictating on $laptop")
            .setContentText("This phone's microphone is on · tap to send")
            .setContentIntent(stop)
            .setOngoing(true)
            .build()
        if (Build.VERSION.SDK_INT >= 30) startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE) else startForeground(ID, n)
        wake = getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "echoconnect:dictate").apply { acquire(15 * 60_000L) }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_NOT_STICKY

    override fun onDestroy() {
        wake?.takeIf { it.isHeld }?.release()
        super.onDestroy()
    }

    companion object { const val ID = 6 }
}

/** Invisible: comes to the front so Android allows the mic, then toggles dictation and closes. */
class MicActivity : ComponentActivity() {
    private val permission = registerForActivityResult(androidx.activity.result.contract.ActivityResultContracts.RequestPermission()) { ok ->
        if (ok) go() else finish()
    }

    override fun onCreate(saved: android.os.Bundle?) {
        super.onCreate(saved)
        if (Scribe.canRecord(this)) go() else permission.launch(Manifest.permission.RECORD_AUDIO)
    }

    private fun go() {
        getSystemService(NotificationManager::class.java).cancel(Scribe.ID_ASK)
        Scribe.startFromActivity(this, intent.getLongExtra(Scribe.EXTRA_SESSION, 0L))
        Work.main.postDelayed({ finish() }, 120)
    }
}

/** Quick Settings: tap to start dictating on the laptop, tap again to send. */
class DictateTile : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        qsTile?.apply {
            state = when {
                Scribe.state.value.recording -> Tile.STATE_ACTIVE
                Connect.laptopId == null -> Tile.STATE_UNAVAILABLE
                else -> Tile.STATE_INACTIVE
            }
            updateTile()
        }
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    override fun onClick() {
        super.onClick()
        val intent = Intent(this, MicActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        if (Build.VERSION.SDK_INT >= 34) startActivityAndCollapse(PendingIntent.getActivity(this, 72, intent, PendingIntent.FLAG_IMMUTABLE))
        else @Suppress("DEPRECATION") startActivityAndCollapse(intent)
    }
}
