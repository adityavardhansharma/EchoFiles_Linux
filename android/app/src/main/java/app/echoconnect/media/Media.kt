package app.echoconnect.media

import android.content.*
import android.media.session.*
import app.echoconnect.*
import app.echoconnect.notifications.NotificationListener
import org.json.JSONObject
import org.json.JSONArray

object Media : Plugin {
    override val handles = setOf("kdeconnect.mpris.request")
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "media")) return
        val sessions = runCatching { ctx.getSystemService(MediaSessionManager::class.java).getActiveSessions(ComponentName(ctx, NotificationListener::class.java)) }.getOrDefault(emptyList())
        if (body.optBoolean("requestPlayerList")) Connect.send("kdeconnect.mpris", JSONObject().put("playerList", JSONArray(sessions.map { it.packageName })), laptop)
        val player = sessions.firstOrNull { it.packageName == body.optString("player") } ?: return
        when (body.optString("action")) {
            "Play" -> player.transportControls.play(); "Pause" -> player.transportControls.pause()
            "PlayPause" -> if (player.playbackState?.state == PlaybackState.STATE_PLAYING) player.transportControls.pause() else player.transportControls.play()
            "Next" -> player.transportControls.skipToNext(); "Previous" -> player.transportControls.skipToPrevious(); "Stop" -> player.transportControls.stop()
        }
        val m = player.metadata
        Connect.send("kdeconnect.mpris", JSONObject().put("player", player.packageName).put("isPlaying", player.playbackState?.state == PlaybackState.STATE_PLAYING).put("title", m?.getString("android.media.metadata.TITLE").orEmpty()).put("artist", m?.getString("android.media.metadata.ARTIST").orEmpty()).put("pos", player.playbackState?.position ?: 0), laptop)
    }
}
