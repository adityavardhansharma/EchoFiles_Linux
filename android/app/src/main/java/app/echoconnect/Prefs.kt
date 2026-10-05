package app.echoconnect

import android.content.Context
import org.json.JSONObject

/** Settings, saved the moment they change. Mirrors EchoFiles → Settings → Phone. */
object Prefs {
    private fun sp(ctx: Context) = ctx.getSharedPreferences("echoconnect", Context.MODE_PRIVATE)

    fun on(ctx: Context, feature: String, default: Boolean = true) = sp(ctx).getBoolean("f.$feature", default)
    fun set(ctx: Context, feature: String, on: Boolean) {
        sp(ctx).edit().putBoolean("f.$feature", on).apply()
        Work.main.post {
            app.echoconnect.calls.Calls.start(ctx)
            app.echoconnect.messages.Messages.start(ctx)
            app.echoconnect.signal.Signal.start(ctx)
            if (feature == "files" && !on) Connect.stopSftp()
            if (feature == "clipboard") { if (on) app.echoconnect.clipboard.ClipboardSync.resume(ctx) else app.echoconnect.clipboard.ClipboardSync.stopReader() }
        }
    }

    /** "auto" (developer-mode setup done) or "manual" (Tap to send). */
    fun clipMode(ctx: Context): String = sp(ctx).getString("clip.mode", "manual") ?: "manual"
    fun setClipMode(ctx: Context, m: String) = sp(ctx).edit().putString("clip.mode", m).apply()

    fun autoAccept(ctx: Context) = sp(ctx).getBoolean("accept.auto", true)
    fun setAutoAccept(ctx: Context, on: Boolean) = sp(ctx).edit().putBoolean("accept.auto", on).apply()

    /** "laptop" (match its Omarchy theme) or "phone" (Echo / Catppuccin Latte with dark mode). */
    fun themeMode(ctx: Context): String = sp(ctx).getString("theme.mode", "laptop") ?: "laptop"
    fun setThemeMode(ctx: Context, m: String) = sp(ctx).edit().putString("theme.mode", m).apply()

    fun theme(ctx: Context): Map<String, Long>? {
        val raw = sp(ctx).getString("theme.laptop", null) ?: return null
        return runCatching {
            val o = JSONObject(raw)
            o.keys().asSequence().associateWith { o.getLong(it) }
        }.getOrNull()
    }

    fun setTheme(ctx: Context, t: Map<String, Long>?) {
        sp(ctx).edit().apply { if (t == null) remove("theme.laptop") else putString("theme.laptop", JSONObject(t as Map<*, *>).toString()) }.apply()
    }

    fun mutedApps(ctx: Context): Set<String> = sp(ctx).getStringSet("notify.muted", emptySet()) ?: emptySet()
    fun setMutedApps(ctx: Context, s: Set<String>) = sp(ctx).edit().putStringSet("notify.muted", s).apply()

    fun pairedOnce(ctx: Context) = sp(ctx).getBoolean("paired.once", false)
    fun setPairedOnce(ctx: Context) = sp(ctx).edit().putBoolean("paired.once", true).apply()

    fun backupCursor(ctx: Context, laptop: String): Pair<Long, Long> = sp(ctx).getLong("backup.$laptop.date", 0L) to sp(ctx).getLong("backup.$laptop.id", 0L)
    fun setBackupCursor(ctx: Context, laptop: String, date: Long, id: Long) = sp(ctx).edit().putLong("backup.$laptop.date", date).putLong("backup.$laptop.id", id).commit()

    /** The laptop's Bluetooth address, from its pairing code. */
    fun laptopBluetooth(ctx: Context): String? = sp(ctx).getString("laptop.bt", null)
    fun setLaptopBluetooth(ctx: Context, mac: String?) = sp(ctx).edit().putString("laptop.bt", mac).apply()
}
