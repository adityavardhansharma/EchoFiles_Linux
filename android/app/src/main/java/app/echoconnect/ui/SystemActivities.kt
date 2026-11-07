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
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.echoconnect.*
import app.echoconnect.ring.Ring
import org.json.JSONObject

/**
 * Full screen while the laptop rings this phone: the screen swaps signal and black every 1.2 s
 * and rows of HERE scroll in opposite directions. One big key; any volume key stops it too.
 */
class RingActivity : ComponentActivity() {
    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved); enableEdgeToEdge(); setShowWhenLocked(true); setTurnScreenOn(true)
        setContent { AppTheme {
            val r = relay()
            val laptop = Connect.state.collectAsStateWithLifecycle().value.laptop?.name ?: "your laptop"
            val t = rememberInfiniteTransition(label = "ring")
            val invert by t.animateFloat(0f, 1f, infiniteRepeatable(tween(2400, easing = LinearEasing)), label = "inv")
            val run by t.animateFloat(0f, 1f, infiniteRepeatable(tween(6000, easing = LinearEasing)), label = "run")
            val dark = invert >= 0.5f
            val bg = if (dark) Color.Black else r.sig
            val fg = if (dark) r.sig else Color.Black
            Column(Modifier.fillMaxSize().background(bg).safeDrawingPadding()) {
                Row(Modifier.fillMaxWidth().padding(start = 18.dp, end = 18.dp, top = 16.dp)) { Lbl("Ringing · full volume", fg, Modifier.weight(1f)); Lbl("From $laptop", fg) }
                SrOnly("Ringing. $laptop is looking for this phone.", Modifier.semantics { heading() })
                Column(Modifier.weight(1f).fillMaxWidth().clipToBounds(), verticalArrangement = Arrangement.Center) {
                    repeat(4) { row ->
                        val words = if (row % 2 == 0) "HERE  HERE  HERE  HERE  HERE  HERE" else "I'M HERE  I'M HERE  I'M HERE  I'M HERE"
                        val shift = (if (row % 2 == 0) -run else run - 1f) * 600f
                        Box(Modifier.fillMaxWidth().height(108.dp)) {
                            Box(Modifier.wrapContentWidth(unbounded = true, align = Alignment.Start).offset(x = shift.dp)) { Dm(words, 110, fg) }
                        }
                    }
                }
                Column(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Paper { Key("Found it", Modifier.fillMaxWidth(), height = 80.dp, size = 22, radius = 24.dp) { stop() } }
                    Lbl("Any volume key stops it · the flashlight blinks along", fg, Modifier.fillMaxWidth(), align = TextAlign.Center, maxLines = 2)
                }
            }
        } }
    }
    private fun stop() { Ring.stop(this); finish() }
    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (keyCode == KeyEvent.KEYCODE_VOLUME_UP || keyCode == KeyEvent.KEYCODE_VOLUME_DOWN) { stop(); return true }
        return super.onKeyDown(keyCode, event)
    }
}

/** A screen-reader-only line. */
@Composable private fun SrOnly(text: String, modifier: Modifier) = Box(modifier.size(1.dp).semantics { contentDescription = text })

/** "Send to laptop" in the text-selection menu. */
class ProcessTextActivity : Activity() {
    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved)
        val text = intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString()
        val ok = !text.isNullOrBlank() && Connect.sendClipboard(text)
        android.widget.Toast.makeText(this, if (ok) "Sent up to ${Connect.state.value.laptop?.name ?: "laptop"}" else "No link to the laptop", android.widget.Toast.LENGTH_SHORT).show()
        finish()
    }
}

/** Share → EchoConnect from any app: a paper sheet (it's leaving this phone) under a carbon destination plate. */
class ShareActivity : ComponentActivity() {
    private class Item(val uri: Uri, val name: String, val size: Long, val image: Boolean)

    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved); enableEdgeToEdge()
        @Suppress("DEPRECATION") val uris: List<Uri> = if (intent.action == Intent.ACTION_SEND_MULTIPLE) intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM).orEmpty() else listOfNotNull(intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM))
        val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()?.takeIf { uris.isEmpty() && it.isNotBlank() }
        val isLink = text != null && (text.startsWith("https://") || text.startsWith("http://")) && !text.trim().contains(' ')
        setContent { AppTheme {
            val r = relay()
            val s by Connect.state.collectAsStateWithLifecycle()
            var items by remember { mutableStateOf<List<Item>>(emptyList()) }
            var thumbs by remember { mutableStateOf<List<Bitmap>>(emptyList()) }
            LaunchedEffect(Unit) {
                items = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { uris.map { describe(it) } }
                items.filter { it.image }.take(3).forEach { item ->
                    kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { runCatching { contentResolver.loadThumbnail(item.uri, android.util.Size(320, 320), null) }.getOrNull() }?.let { thumbs = thumbs + it }
                }
            }
            val laptop = s.laptop
            val ready = laptop?.lan == true && (text != null || Prefs.on(this, "files"))
            val images = items.isNotEmpty() && items.all { it.image }
            val label = when {
                text != null -> if (isLink) "Open there" else "Send text up"
                uris.size == 1 -> if (images) "Send photo up" else "Send file up"
                images -> "Send ${uris.size} up"
                else -> "Send ${uris.size} up"
            }
            Sheet({ finish() }, carbon = false) {
                Carbon(Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp))) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) { Lbl("Up to"); T(laptop?.name ?: "No laptop", 20, 600, maxLines = 1) }
                            Led(if (laptop?.lan == true) LedState.Live else LedState.Off); Spacer(Modifier.width(8.dp)); Lbl(if (laptop?.lan == true) "Wi‑Fi" else "No Wi‑Fi", ink(2))
                        }
                        MonoText(if (text != null) (if (isLink) "→ the laptop's browser" else "→ the laptop's clipboard") else "→ ~/Downloads/Phone", 12, ink(2))
                    }
                }
                when {
                    laptop == null -> Banner("No laptop paired", "Open EchoConnect and do the handshake with EchoFiles first.")
                    !laptop.lan -> Banner("${laptop.name} isn't on this Wi‑Fi", "Files and links cross over Wi‑Fi. Open the laptop on the same network and try again.")
                    text == null && !Prefs.on(this@ShareActivity, "files") -> Banner("File sharing is off", "Turn on Files on the EchoConnect Panel.")
                }
                if (text != null) Slip(false) { Lbl(if (isLink) "Link" else "Text"); T(text.take(240), 16, maxLines = 5) }
                else if (thumbs.isNotEmpty()) Box(Modifier.fillMaxWidth().height(190.dp)) {
                    thumbs.forEachIndexed { i, b ->
                        Box(Modifier.offset(x = (40 + i * 52).dp, y = (24 - i * 4).dp).rotate(listOf(-10f, -3f, 6f)[i]).size(130.dp, 158.dp).background(r.cap, RoundedCornerShape(10.dp)).padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 26.dp)) {
                            Image(b.asImageBitmap(), null, Modifier.fillMaxSize().clip(RoundedCornerShape(4.dp)), contentScale = ContentScale.Crop)
                        }
                    }
                    if (uris.size > 1) Dm("×${uris.size}", 60, r.sigInk, Modifier.align(Alignment.CenterEnd).padding(end = 12.dp))
                } else items.firstOrNull()?.let { first ->
                    Ticket(false, body = {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) { Plate(first.name); T(first.name, 15, 600, maxLines = 1) }
                        if (items.size > 1) Lbl("and ${items.size - 1} more")
                    }, stub = { Dm(bytes(items.sumOf { it.size }).substringBefore(' '), 22); Lbl(bytes(items.sumOf { it.size }).substringAfter(' ')) })
                }
                if (text == null && items.isNotEmpty()) Row { Lbl("${items.size} ${if (images) "photos" else "files"}".replace("1 photos", "1 photo").replace("1 files", "1 file"), modifier = Modifier.weight(1f)); Lbl(bytes(items.sumOf { it.size })) }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key("Cancel", Modifier.weight(1f), height = 58.dp) { finish() }
                    Key(label, Modifier.weight(2f), Tone.Signal, trailingIcon = "arrow-up", height = 58.dp, size = 16, enabled = ready) { send(uris, text, isLink); finish() }
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
        android.widget.Toast.makeText(app, if (text != null) (if (isLink) "Opening on the laptop" else "Sent to the laptop's clipboard") else "Sending up to the laptop", android.widget.Toast.LENGTH_SHORT).show()
    }
}
