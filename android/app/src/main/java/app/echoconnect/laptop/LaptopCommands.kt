package app.echoconnect.laptop

import android.app.NotificationManager
import android.content.Context
import app.echoconnect.*
import org.json.JSONObject

object LaptopCommands : Plugin {
    override val handles = setOf("echofiles.theme", "echofiles.settings", "echofiles.dnd")
    override fun onReady(ctx: Context, laptop: String) {
        val values = JSONObject()
        listOf("clipboard", "notifications", "sms", "files", "photos", "battery", "ring", "contacts", "calls", "media", "signal", "capture").forEach { values.put(it, Prefs.on(ctx, it)) }
        listOf("backup", "dnd", "autolock").forEach { values.put(it, Prefs.on(ctx, it, false)) }
        Connect.send("echofiles.settings", values, laptop)
    }
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        when (type) {
            "echofiles.theme" -> {
                val colors = body.optJSONObject("tokens") ?: body
                val theme = colors.keys().asSequence().mapNotNull { key ->
                    val value = colors.opt(key)
                    val color = when (value) { is Number -> value.toLong(); is String -> runCatching { android.graphics.Color.parseColor(value).toLong() and 0xffffffffL }.getOrNull(); else -> null }
                    color?.let { key to it }
                }.toMap()
                if (theme.keys.containsAll(listOf("bg", "ink", "accent"))) Connect.setTheme(theme)
            }
            "echofiles.settings" -> listOf("clipboard", "notifications", "sms", "files", "photos", "battery", "ring", "contacts", "calls", "media", "signal", "capture", "backup", "dnd", "autolock").forEach { if (body.has(it)) { Prefs.set(ctx, it, body.getBoolean(it)); if (it == "files" && !body.getBoolean(it)) Connect.stopSftp(); if (it == "clipboard" && !body.getBoolean(it)) app.echoconnect.clipboard.ClipboardSync.stopReader() } }
            "echofiles.dnd" -> {
                if (!Prefs.on(ctx, "dnd", false)) return
                val nm = ctx.getSystemService(NotificationManager::class.java)
                if (nm.isNotificationPolicyAccessGranted) nm.setInterruptionFilter(if (body.optBoolean("enabled")) NotificationManager.INTERRUPTION_FILTER_PRIORITY else NotificationManager.INTERRUPTION_FILTER_ALL)
            }
        }
    }
}
