package app.echoconnect.ui

import android.app.Activity
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.core.*
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.echoconnect.*
import app.echoconnect.ring.Ring
import org.json.JSONObject

/** Full screen while the laptop rings this phone: big Stop ringing, two flat outlines pulsing. */
class RingActivity : ComponentActivity() {
    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved); enableEdgeToEdge(); setShowWhenLocked(true); setTurnScreenOn(true)
        setContent { AppTheme {
            val laptop = Connect.state.collectAsStateWithLifecycle().value.laptop?.name ?: "Your laptop"
            val pulse = rememberInfiniteTransition(label = "pulse")
            val outer by pulse.animateFloat(0.05f, 0.25f, infiniteRepeatable(tween(1400, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "outer")
            val inner by pulse.animateFloat(0.5f, 0.15f, infiniteRepeatable(tween(1400, 350, FastOutSlowInEasing), RepeatMode.Reverse), label = "inner")
            Column(Modifier.fillMaxSize().background(token("bg-deep")).safeDrawingPadding().padding(start = 24.dp, end = 24.dp, top = 32.dp, bottom = 24.dp),
                horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.padding(top = 24.dp, bottom = 12.dp).size(168.dp), contentAlignment = Alignment.Center) {
                    Box(Modifier.fillMaxSize().alpha(outer).border(2.dp, token("accent"), RoundedCornerShape(4.dp)))
                    Box(Modifier.padding(22.dp).fillMaxSize().alpha(inner).border(2.dp, token("accent"), RoundedCornerShape(4.dp)))
                    Box(Modifier.size(88.dp).background(token("accent"), RoundedCornerShape(4.dp)), contentAlignment = Alignment.Center) { Glyph("phone-ring", "Ringing", token("on-accent"), 40) }
                }
                Display("Ringing", align = TextAlign.Center)
                Lede("$laptop is looking for this phone. Full volume, even on silent; the flashlight blinks too.", Modifier.widthIn(max = 280.dp), TextAlign.Center)
                Spacer(Modifier.weight(1f))
                Button("Stop ringing", Variant.Primary, BtnSize.Lg, "close", block = true) { stop() }
                Txt("Any volume key stops it too.", muted = true, size = 12, line = 16)
            }
        } }
    }
    private fun stop() { Ring.stop(this); finish() }
    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (keyCode == KeyEvent.KEYCODE_VOLUME_UP || keyCode == KeyEvent.KEYCODE_VOLUME_DOWN) { stop(); return true }
        return super.onKeyDown(keyCode, event)
    }
}

/** "Send to laptop" in the text-selection menu. */
class ProcessTextActivity : Activity() {
    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved)
        val text = intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString()
        val ok = !text.isNullOrBlank() && Connect.sendClipboard(text)
        android.widget.Toast.makeText(this, if (ok) "Sent to ${Connect.state.value.laptop?.name ?: "laptop"}" else "Laptop not connected", android.widget.Toast.LENGTH_SHORT).show()
        finish()
    }
}

/** Share → EchoConnect from any app: what goes, where it lands, Send. */
class ShareActivity : ComponentActivity() {
    private class Item(val uri: Uri, val name: String, val size: Long, val image: Boolean)

    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved); enableEdgeToEdge()
        @Suppress("DEPRECATION") val uris: List<Uri> = if (intent.action == Intent.ACTION_SEND_MULTIPLE) intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM).orEmpty() else listOfNotNull(intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM))
        val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()?.takeIf { uris.isEmpty() && it.isNotBlank() }
        val isLink = text != null && (text.startsWith("https://") || text.startsWith("http://")) && !text.trim().contains(' ')
        setContent { AppTheme {
            val s by Connect.state.collectAsStateWithLifecycle()
            var items by remember { mutableStateOf<List<Item>>(emptyList()) }
            var thumbs by remember { mutableStateOf<Map<Uri, Bitmap>>(emptyMap()) }
            LaunchedEffect(Unit) {
                items = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { uris.map { describe(it) } }
                items.filter { it.image }.take(4).forEach { item ->
                    kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { contentResolver.loadThumbnail(item.uri, android.util.Size(240, 240), null) }.getOrNull() }?.let { thumbs = thumbs + (item.uri to it) }
                }
            }
            val laptop = s.laptop
            val ready = laptop?.lan == true && (text != null || Prefs.on(this, "files"))
            val images = items.isNotEmpty() && items.all { it.image }
            val label = when {
                text != null -> if (isLink) "Open on laptop" else "Send text"
                uris.size == 1 -> if (images) "Send photo" else "Send file"
                images -> "Send ${uris.size} photos"
                else -> "Send ${uris.size} files"
            }
            BottomSheet("Send to ${laptop?.name ?: "laptop"}", "laptop", { finish() }, footer = {
                Button("Cancel", Variant.Ghost) { finish() }
                Button(label, Variant.Primary, icon = "send", enabled = ready) { send(uris, text, isLink); finish() }
            }) {
                if (thumbs.isNotEmpty()) Row(Modifier.padding(horizontal = 10.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    thumbs.values.forEach { Image(it.asImageBitmap(), null, Modifier.size(72.dp).clip(RoundedCornerShape(6.dp)), contentScale = ContentScale.Crop) }
                }
                when {
                    laptop == null -> Banner("No laptop paired.", "Open EchoConnect and pair with EchoFiles first.", "warning")
                    !laptop.lan -> Banner("${laptop.name} is not on this Wi-Fi.", "Files and links cross over Wi-Fi. Open the laptop on the same network and try again.", "warning")
                    text == null && !Prefs.on(this@ShareActivity, "files") -> Banner("File sharing is off.", "Turn on Files in EchoConnect → Settings.", "warning")
                }
                if (text != null) ListRow(if (isLink) "Opens in the laptop's browser" else "Arrives on the laptop's clipboard", text.take(200), icon = if (isLink) "external" else "clipboard")
                else {
                    if (items.size == 1 && thumbs.isEmpty()) ListRow(items[0].name, bytes(items[0].size), lead = { FileIcon(items[0].name) })
                    val total = items.sumOf { it.size }
                    ListRow("Lands in ~/Downloads/Phone", (if (items.isEmpty()) "" else "${bytes(total)} · ") + "over Wi-Fi", icon = "folder")
                }
            }
        } }
    }

    private fun describe(uri: Uri): Item {
        var name = uri.lastPathSegment ?: "file"; var size = 0L
        runCatching { contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { c -> if (c.moveToFirst()) { c.getString(0)?.let { name = it }; if (!c.isNull(1)) size = c.getLong(1) } } }
        return Item(uri, name, size, contentResolver.getType(uri)?.startsWith("image/") == true)
    }

    private fun send(uris: List<Uri>, text: String?, isLink: Boolean) {
        val app = applicationContext
        Work.run {
            if (text != null) {
                Connect.send("kdeconnect.share.request", if (isLink) JSONObject().put("url", text.trim()) else JSONObject().put("text", text))
            }
            uris.forEach { Connect.sendUri(it, count = uris.size) }
        }
        android.widget.Toast.makeText(app, if (text != null) (if (isLink) "Opening on laptop" else "Sent to the laptop's clipboard") else "Sending to laptop", android.widget.Toast.LENGTH_SHORT).show()
    }
}
