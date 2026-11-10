package app.echoconnect.ui

import android.Manifest
import android.app.NotificationManager
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.compose.animation.core.*
import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.*
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.echoconnect.*
import app.echoconnect.clipboard.ClipboardSync
import app.echoconnect.share.Share
import kotlin.math.PI
import kotlin.math.sin

/** What screens need from the activity: permission prompts, Settings detours, pickers and results. */
interface Host {
    val context: Context
    /** Bumped when permissions or settings may have changed; reading it recomposes the caller. */
    val revision: Int
    fun refresh()
    fun request(vararg permissions: String)
    fun settings(action: String, packageUri: Boolean = false)
    fun requestPhotos()
    fun pickFiles()
    fun scan()
    fun snack(s: Snack)
    fun go(page: String)
    fun open(url: String)
}

const val SOURCE = "github.com/adityavardhansharma/EchoFiles_Linux"
val TABS = listOf("home", "clipboard", "transfers", "settings")

// ------------------------------------------------------------------ features and permissions

class Feature(val key: String, val icon: String, val title: String, val what: String, val default: Boolean = true)

val FEATURES = listOf(
    Feature("notifications", "bell", "Notifications", "Shown + answered"),
    Feature("sms", "message", "Messages", "Read and reply"),
    Feature("contacts", "users", "Contact names", "In texts, calls"),
    Feature("calls", "call", "Calls", "Answer on laptop"),
    Feature("files", "folder", "Files", "Both ways"),
    Feature("photos", "image", "Photos", "Camera roll"),
    Feature("clipboard", "clipboard", "Clipboard", "Copy, paste across"),
    Feature("battery", "battery", "Battery", "Level on laptop"),
    Feature("ring", "phone-ring", "Ring phone", "Even on silent"),
    Feature("capture", "camera", "Camera", "Shoot + scan"),
    Feature("media", "music", "Media keys", "Play, pause, skip"),
    Feature("signal", "network", "Mobile signal", "Bars on laptop"),
    Feature("dictation", "mic", "Dictation", "Talk, it types"),
)

fun granted(ctx: Context, vararg p: String) = p.all { Work.allowed(ctx, it) }

/** Whether Android lets the feature work now. */
fun featureReady(ctx: Context, key: String): Boolean = when (key) {
    "notifications", "media" -> app.echoconnect.notifications.Notifications.listener != null
    "sms" -> granted(ctx, Manifest.permission.READ_SMS, Manifest.permission.SEND_SMS)
    "contacts" -> granted(ctx, Manifest.permission.READ_CONTACTS)
    "calls", "signal" -> granted(ctx, Manifest.permission.READ_PHONE_STATE)
    "files" -> app.echoconnect.files.Files.allowed()
    "photos" -> app.echoconnect.photos.Photos.hasAccess(ctx)
    "backup" -> app.echoconnect.photos.Photos.fullAccess(ctx)
    "dnd" -> ctx.getSystemService(NotificationManager::class.java).isNotificationPolicyAccessGranted
    "capture" -> granted(ctx, Manifest.permission.CAMERA)
    "dictation" -> granted(ctx, Manifest.permission.RECORD_AUDIO)
    else -> true
}

/** Asks for what [key] needs, in context. */
fun allowFeature(host: Host, key: String) = when (key) {
    "notifications", "media" -> host.settings(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)
    "sms" -> host.request(Manifest.permission.READ_SMS, Manifest.permission.SEND_SMS, Manifest.permission.RECEIVE_SMS, Manifest.permission.RECEIVE_MMS)
    "contacts" -> host.request(Manifest.permission.READ_CONTACTS)
    "calls", "signal" -> host.request(*listOfNotNull(Manifest.permission.READ_PHONE_STATE, Manifest.permission.READ_CALL_LOG, Manifest.permission.ANSWER_PHONE_CALLS, if (Build.VERSION.SDK_INT >= 31) Manifest.permission.BLUETOOTH_CONNECT else null).toTypedArray())
    "files" -> if (Build.VERSION.SDK_INT >= 30) host.settings(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, true) else host.request(Manifest.permission.READ_EXTERNAL_STORAGE)
    "photos", "backup" -> host.requestPhotos()
    "dnd" -> host.settings(Settings.ACTION_NOTIFICATION_POLICY_ACCESS_SETTINGS)
    "capture" -> host.request(Manifest.permission.CAMERA)
    "dictation" -> host.request(Manifest.permission.RECORD_AUDIO)
    else -> Unit
}

class Perm(val icon: String, val title: String, val why: String, val granted: Boolean, val action: String = "Allow", val optional: Boolean = false, val request: (Host) -> Unit)

fun batteryUnrestricted(ctx: Context) = ctx.getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(ctx.packageName)

/** Every permission EchoConnect can use, named as Android names it, with what it unlocks. */
fun permissions(ctx: Context): List<Perm> {
    val photosFull = app.echoconnect.photos.Photos.fullAccess(ctx)
    return listOfNotNull(
        Perm("bell", "Notification access", "Shows your notifications on the laptop", featureReady(ctx, "notifications"), "Open ↗") { allowFeature(it, "notifications") },
        Perm("message", "SMS", "Read and send texts from the laptop", featureReady(ctx, "sms")) { allowFeature(it, "sms") },
        Perm("users", "Contacts", "Names instead of numbers in texts and calls", featureReady(ctx, "contacts")) { allowFeature(it, "contacts") },
        Perm("call", "Phone", "Caller name on the laptop; answer from there", granted(ctx, Manifest.permission.READ_PHONE_STATE, Manifest.permission.ANSWER_PHONE_CALLS)) { allowFeature(it, "calls") },
        if (Build.VERSION.SDK_INT >= 31) Perm("bluetooth", "Nearby devices", "Bluetooth for calls and clipboard", granted(ctx, Manifest.permission.BLUETOOTH_CONNECT)) { it.request(Manifest.permission.BLUETOOTH_CONNECT, Manifest.permission.BLUETOOTH_SCAN) } else null,
        Perm("folder", "All files access", "Browse the phone from EchoFiles", featureReady(ctx, "files"), "Open ↗") { allowFeature(it, "files") },
        Perm("image", "Photos and videos", if (photosFull || !featureReady(ctx, "photos")) "Your camera roll in EchoFiles" else "Only allowed photos and videos", photosFull, if (featureReady(ctx, "photos")) "Allow all" else "Allow") { it.requestPhotos() },
        Perm("camera", "Camera", "Scan the laptop's code; take photos for it", featureReady(ctx, "capture"), optional = true) { allowFeature(it, "capture") },
        Perm("mic", "Microphone", "Dictate on the laptop; only while you hold the key or allow the laptop", featureReady(ctx, "dictation"), optional = true) { allowFeature(it, "dictation") },
        Perm("clipboard", "Display over other apps", "Lets automatic clipboard read what you copy", Settings.canDrawOverlays(ctx), "Open ↗", true) { it.settings(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, true) },
        Perm("moon", "Do Not Disturb access", "Ring through silent; match the laptop's Do Not Disturb", featureReady(ctx, "dnd"), "Open ↗", true) { allowFeature(it, "dnd") },
        if (Build.VERSION.SDK_INT >= 34) Perm("phone-ring", "Full-screen alerts", "Ringing shows over the lock screen", ctx.getSystemService(NotificationManager::class.java).canUseFullScreenIntent(), "Open ↗", true) { it.settings(Settings.ACTION_MANAGE_APP_USE_FULL_SCREEN_INTENT, true) } else null,
        Perm("battery", "Battery: Unrestricted", "Stays connected with the screen off", batteryUnrestricted(ctx), "Open ↗") { requestUnrestricted(it) },
        if (Build.VERSION.SDK_INT >= 33) Perm("bell", "Notifications", "The small Connected notification, rings and received files", granted(ctx, Manifest.permission.POST_NOTIFICATIONS)) { it.request(Manifest.permission.POST_NOTIFICATIONS) } else null,
    )
}

@android.annotation.SuppressLint("BatteryLife")
fun requestUnrestricted(host: Host) = host.settings(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, true)

val samsung = Build.MANUFACTURER.equals("samsung", ignoreCase = true)

/** Sends the phone's clipboard and says what happened. */
fun sendClipboard(host: Host) {
    val name = Connect.state.value.laptop?.name ?: "the laptop"
    when (ClipboardSync.send(host.context, force = true)) {
        ClipboardSync.Sent.Sent -> host.snack(Snack("Sent up to $name", "success"))
        ClipboardSync.Sent.Sending -> host.snack(Snack("Sending the image to $name"))
        ClipboardSync.Sent.Empty -> host.snack(Snack("The clipboard is empty. Copy something first.", "warning"))
        ClipboardSync.Sent.Off -> host.snack(Snack("Clipboard is switched off", "warning", action = "Turn on") { Prefs.set(host.context, "clipboard", true); host.refresh() })
        ClipboardSync.Sent.Unchanged -> host.snack(Snack("$name already has it", "success"))
        ClipboardSync.Sent.Failed -> host.snack(Snack("No link to $name", "danger", action = "Retry") { sendClipboard(host) })
    }
}

fun copyAction(host: Host, c: Clip): (() -> Unit)? =
    if (c.sensitive || c.image || c.text.isEmpty()) null else ({ ClipboardSync.copyAgain(host.context, c.text); host.snack(Snack("Copied", "success")) })

fun openUri(ctx: Context, uri: String, host: Host) {
    val u = Uri.parse(uri)
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW).setDataAndType(u, ctx.contentResolver.getType(u) ?: "*/*").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)) }
        .onFailure { host.snack(Snack("No app on this phone opens this file", "warning")) }
}

fun dayLabel(at: Long): String = when {
    android.text.format.DateUtils.isToday(at) -> "Today"
    android.text.format.DateUtils.isToday(at + 86_400_000) -> "Yesterday"
    else -> java.text.SimpleDateFormat("EEEE d MMM", java.util.Locale.getDefault()).format(java.util.Date(at))
}

// ------------------------------------------------------------------ shared pieces

/** A tab's scrolling body, with room left under it for the dial. */
@Composable fun TabBody(content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        content()
        Spacer(Modifier.windowInsetsPadding(WindowInsets.navigationBars.only(WindowInsetsSides.Bottom)).height(112.dp))
    }
}

/** The paper half of a screen: 16dp gutter, 10dp between blocks. */
@Composable fun PaperBody(top: Dp = 18.dp, content: @Composable ColumnScope.() -> Unit) =
    Column(Modifier.fillMaxWidth().sideSafe().padding(start = 16.dp, end = 16.dp, top = top), verticalArrangement = Arrangement.spacedBy(10.dp), content = content)

private fun moving(t: Transfer) = !t.finished && t.error == null && !Share.waiting().containsKey(t.id)

private fun rate(t: Transfer): Long = t.done / ((System.currentTimeMillis() - t.at) / 1000).coerceAtLeast(1)

/** A file as a ticket. Carbon if it came from the laptop, paper if it left this phone. */
@Composable fun TransferTicket(t: Transfer, host: Host) {
    val ctx = host.context
    val r = relay()
    val waiting = Share.waiting().containsKey(t.id)
    val moving = !t.finished && t.error == null && !waiting
    val quiet = t.error in setOf("Cancelled", "Declined")
    val failed = t.error != null && !waiting
    val carbon = !t.upload
    val dir = if (t.upload) "↑ to laptop" else "↓ from laptop"
    val fraction = if (t.size > 0) t.done.toFloat() / t.size else null
    Ticket(carbon, faded = quiet, description = "${t.name}, " + when { waiting -> "waiting for you"; moving -> "${((fraction ?: 0f) * 100).toInt()} percent"; failed -> t.error ?: "stopped"; else -> "done" }, body = {
        if (waiting) Lbl("● Wants to send", r.sig)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Plate(t.name); T(t.name, 15, 600, maxLines = 1, strike = quiet)
        }
        when {
            moving -> {
                DotBar(fraction)
                val left = rate(t).let { if (it > 0 && t.size > t.done) " · ${(t.size - t.done) / it + 1}s left" else "" }
                Lbl("${bytes(t.done)} / ${bytes(t.size)} · ${bytes(rate(t))}/s$left", ink(3))
            }
            waiting -> {
                Lbl("${bytes(t.size)} · $dir", ink(3))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key("Decline", Modifier.weight(1f), if (carbon) Tone.Carbon else Tone.Paper, height = 42.dp, size = 14) { Share.decline(t.id); host.refresh() }
                    Key("Receive ↓", Modifier.weight(1.4f), Tone.Signal, height = 42.dp, size = 14) { Share.receive(ctx, t.id); host.refresh() }
                }
            }
            failed && !quiet -> Lbl("Stopped · ${t.error}", if (carbon) r.bad else r.badInk, maxLines = 2)
            failed -> Lbl("${t.error} · ${bytes(t.size)}", ink(3))
            else -> Lbl("${bytes(t.size)} · $dir · ${relativeTime(t.at)}", ink(3))
        }
    }, stub = {
        when {
            waiting -> { Dm(bytes(t.size).substringBefore(' '), 24); Lbl(bytes(t.size).substringAfter(' '), ink(3)) }
            moving -> {
                Dm("${((fraction ?: 0f) * 100).toInt().coerceIn(0, 99)}", 28, if (carbon) r.sig else r.sigInk)
                Key("Stop", tone = if (carbon) Tone.Carbon else Tone.Paper, height = 32.dp, size = 12, radius = 10.dp) { Connect.cancelTransfer(t.id) }
            }
            failed && !quiet && Connect.canRetry(t.id) -> Key("Retry", tone = if (carbon) Tone.Carbon else Tone.Paper, height = 34.dp, size = 13, radius = 10.dp) { Connect.retryTransfer(t.id) }
            failed -> Lbl("—", ink(3))
            t.uri != null -> Key("Open", tone = if (carbon) Tone.Carbon else Tone.Paper, height = 34.dp, size = 13, radius = 10.dp) { openUri(ctx, t.uri, host) }
            else -> Lbl("✓ Done", if (carbon) r.live else r.liveInk)
        }
    })
}

/** A clipboard item as a slip. Carbon if it came from the laptop. */
@Composable fun ClipSlip(c: Clip, number: Int, onCopy: (() -> Unit)?) {
    val r = relay()
    val view = LocalView.current
    var copied by remember(c.at) { mutableStateOf(false) }
    LaunchedEffect(copied) { if (copied) { kotlinx.coroutines.delay(900); copied = false } }
    Slip(carbon = !c.toLaptop) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Lbl("#%03d · %s · %s%s".format(number, if (c.toLaptop) "↑ to laptop" else "↓ from laptop", relativeTime(c.at), if (c.bluetooth) " · BT" else ""), ink(3), Modifier.weight(1f))
            if (onCopy != null) {
                val face = if (c.toLaptop) r.paper2 else r.capd
                Box(Modifier.size(40.dp).clip(RoundedCornerShape(11.dp)).background(if (copied) r.live else face)
                    .clickable(onClickLabel = "Copy again", role = androidx.compose.ui.semantics.Role.Button) { Haptics.confirm(view); copied = true; onCopy() }
                    .semantics { contentDescription = "Copy again" }, contentAlignment = Alignment.Center) {
                    Glyph(if (copied) "check" else "copy", color = if (copied) Color(0xFF04170C) else ink(1), size = 16)
                }
            }
        }
        when {
            c.sensitive -> {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) { listOf(64, 110, 40).forEach { Box(Modifier.size(it.dp, 14.dp).background(ink(1), RoundedCornerShape(2.dp))) } }
                Lbl("Password manager · hidden · never kept", ink(3))
            }
            c.image -> Box(Modifier.fillMaxWidth().height(96.dp).clip(RoundedCornerShape(10.dp)).background(if (c.toLaptop) r.paper2 else r.carbon2)
                .drawBehind { val s = 8.dp.toPx(); var y = s / 2; while (y < size.height) { var x = s / 2; while (x < size.width) { drawCircle(r.ink3.copy(alpha = 0.5f), 1.4.dp.toPx(), Offset(x, y)); x += s }; y += s } }
                .padding(8.dp), contentAlignment = Alignment.BottomStart) { Lbl("Image", ink(2)) }
            else -> T(c.text, 17, line = 24, maxLines = 4)
        }
    }
}

// ------------------------------------------------------------------ Home

@Composable fun HomeScreen(s: UiState, host: Host, openStations: () -> Unit, openLink: () -> Unit) {
    val ctx = host.context
    val r = relay()
    host.revision
    val l = s.laptop
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(s.call != null) { while (s.call != null) { now = System.currentTimeMillis(); kotlinx.coroutines.delay(1000) } }
    val lan = l?.lan == true
    val connected = l?.connected == true
    val inFlight = s.transfers.filter { moving(it) }
    TabBody {
        val call = s.call
        if (call != null) {
            Head {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Led(LedState.Live, blink = true); Lbl(if (call.onLaptop) "Call · on ${l?.name ?: "the laptop"}" else "Call · on this phone", r.live, Modifier.weight(1f))
                }
                Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Ser(call.who.ifBlank { call.number.ifBlank { "Call" } }, 72)
                    if (call.who.isNotBlank() && call.number.isNotBlank()) Lbl(call.number, ink(3), size = 12)
                }
                Row(verticalAlignment = Alignment.Bottom) {
                    Dm(clock(((now - call.since) / 1000).coerceAtLeast(0)), 52, r.live, glow = true)
                    Spacer(Modifier.weight(1f)); Lbl(if (call.onLaptop) "Audio via Bluetooth" else "", ink(3))
                }
                if (l != null) Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key("Lock screen", Modifier.weight(1f), Tone.Carbon, icon = "lock", height = 46.dp, size = 14, enabled = lan) {
                        if (Connect.send("echofiles.lock")) host.snack(Snack("Locked ${l.name}", "success")) else host.snack(Snack("No link to ${l.name}", "danger"))
                    }
                    Key("Open a link", Modifier.weight(1f), Tone.Carbon, icon = "external", height = 46.dp, size = 14, enabled = lan, onClick = openLink)
                }
            }
            Seam(SeamMode.Voice, height = 96.dp, description = "Call in progress")
            PaperBody {
                Lbl("This phone")
                if (call.onLaptop) Key("Take it on the phone", Modifier.fillMaxWidth(), Tone.Live, icon = "phone", trailingIcon = "arrow-down", height = 72.dp, size = 18, start = true) { app.echoconnect.calls.Calls.usePhone(ctx) }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key("Send files", Modifier.weight(1f), icon = "upload", enabled = lan && Prefs.on(ctx, "files"), size = 14) { host.pickFiles() }
                    Key("Clipboard", Modifier.weight(1f), icon = "clipboard", enabled = connected && Prefs.on(ctx, "clipboard"), size = 14) { sendClipboard(host) }
                }
            }
        } else {
        Head {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Led(if (connected) LedState.Live else LedState.Off)
                Lbl(when {
                    l == null -> "Not paired"
                    !connected -> "No signal · looking"
                    l.lan && l.bluetooth -> "Linked · Wi‑Fi + BT"
                    l.lan -> "Linked · Wi‑Fi"
                    else -> "Linked · Bluetooth"
                }, ink(2), Modifier.weight(1f))
                val count = s.laptops.count { it.paired }
                if (count == 0) Key("Pair", tone = Tone.Carbon, icon = "qr", height = 34.dp, size = 12, radius = 999.dp) { host.go("pair") }
                else Key(if (count == 1) "1 laptop" else "$count laptops", tone = Tone.Carbon, trailingIcon = "chevron-down", height = 34.dp, size = 12, radius = 999.dp, onClick = openStations)
            }
            Title(l?.name ?: "No laptop yet", size = 36, modifier = Modifier.alpha(if (connected || l == null) 1f else 0.6f))
            if (l == null) {
                T("Pair with EchoFiles to share files, clipboard, texts, notifications and calls.", 14, color = ink(2))
                Key("Pair a laptop", Modifier.fillMaxWidth(), Tone.Signal, icon = "qr", height = 56.dp) { host.go("pair") }
            } else {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Readout("Battery", if (connected && l.battery != null) "${l.battery}%" else "--", Modifier.weight(1f), if (l.charging && connected) r.live else null)
                    Readout("Wi‑Fi", if (connected && l.lan) "ON" else "OFF", Modifier.weight(1f), if (connected && l.lan) r.live else ink(3))
                    Readout("Bluetooth", if (connected && l.bluetooth) "ON" else "OFF", Modifier.weight(1f), if (connected && l.bluetooth) r.live else ink(3))
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key("Lock screen", Modifier.weight(1f), Tone.Carbon, icon = "lock", height = 50.dp, size = 14, enabled = lan) {
                        if (Connect.send("echofiles.lock")) host.snack(Snack("Locked ${l.name}", "success")) else host.snack(Snack("No link to ${l.name}", "danger"))
                    }
                    Key("Open a link", Modifier.weight(1f), Tone.Carbon, icon = "external", height = 50.dp, size = 14, enabled = lan, onClick = openLink)
                }
            }
            s.error?.let { e ->
                Row(Modifier.fillMaxWidth().background(r.bad.copy(alpha = 0.16f), RoundedCornerShape(14.dp)).padding(start = 12.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Led(LedState.Bad); T(e, 13, color = ink(1), modifier = Modifier.weight(1f), maxLines = 2)
                    Key("Reconnect", tone = Tone.Carbon, height = 38.dp, size = 13, radius = 11.dp) { Connect.start(); Connect.announce() }
                }
            }
        }
        val first = inFlight.firstOrNull()
        when {
            first != null -> Seam(SeamMode.Moving, left = if (first.upload) "↑ TX" else "↓ RX", right = "${bytes(rate(first)).replace(" ", "")}/S", description = "Sending ${first.name}")
            connected -> Seam(SeamMode.Live, left = "Link", right = l?.battery?.let { "$it%" }, description = "Linked")
            l != null -> Seam(SeamMode.Flat, left = "● Searching", right = "FLAT", rightColor = r.chalk3, description = "No link. Searching for the laptop.")
            else -> Seam(SeamMode.Flat, description = "Not paired")
        }
        PaperBody {
            if (l != null && !connected) Column(Modifier.fillMaxWidth().background(r.paper2, RoundedCornerShape(18.dp)).padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Row(verticalAlignment = Alignment.Bottom) { Ser("Out of range.", 26); T(" Open the laptop on the same Wi‑Fi.", 18) }
                T("Calls and clipboard come back over Bluetooth as soon as it's close.", 14, color = ink(2))
                Key("Search again", Modifier.fillMaxWidth(), icon = "refresh", height = 46.dp, size = 14) { Connect.start(); Connect.announce() }
            }
            Lbl("This phone")
            Key("Send files up", Modifier.fillMaxWidth(), Tone.Signal, icon = "arrow-up", height = 74.dp, size = 19, start = true, note = if (lan) "Any file" else "Needs Wi‑Fi",
                enabled = lan && Prefs.on(ctx, "files")) { host.pickFiles() }
            val lastUp = s.clips.firstOrNull { it.toLaptop }
            Key("Send clipboard", Modifier.fillMaxWidth(), icon = "clipboard", height = 56.dp, start = true, note = lastUp?.let { "Last · ${relativeTime(it.at)}" },
                enabled = connected && Prefs.on(ctx, "clipboard")) { sendClipboard(host) }
            if (l != null && Prefs.on(ctx, "dictation")) {
                val d by app.echoconnect.scribe.Scribe.state.collectAsStateWithLifecycle()
                Key(if (d.recording) "Dictating…" else "Dictate on ${l.name}", Modifier.fillMaxWidth(), if (d.recording) Tone.Signal else Tone.Paper, icon = "mic", height = 56.dp, start = true,
                    note = when { d.recording -> "Live"; connected && d.available -> "EchoScribe"; connected -> "Start EchoScribe"; else -> "Out of range" }, trailingIcon = "chevron-right") { host.go("dictate") }
            }

            val tickets = (inFlight + s.transfers.filter { Share.waiting().containsKey(it.id) } + s.transfers).distinctBy { it.id }.take(2)
            if (tickets.isNotEmpty()) {
                Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Lbl(if (inFlight.isNotEmpty()) "In transit" else "Files", modifier = Modifier.weight(1f))
                    Box(Modifier.heightIn(min = 32.dp).clickable(role = androidx.compose.ui.semantics.Role.Button) { host.go("transfers") }.padding(horizontal = 4.dp), contentAlignment = Alignment.Center) { Lbl("See all ›", ink(2)) }
                }
                tickets.forEach { TransferTicket(it, host) }
            }
            s.clips.firstOrNull()?.let { c ->
                Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Lbl("Clip", modifier = Modifier.weight(1f))
                    Box(Modifier.heightIn(min = 32.dp).clickable(role = androidx.compose.ui.semantics.Role.Button) { host.go("clipboard") }.padding(horizontal = 4.dp), contentAlignment = Alignment.Center) { Lbl("See all ›", ink(2)) }
                }
                ClipSlip(c, s.clips.size, copyAction(host, c))
            }
        }
        }
    }
}

/** A live number in a carbon cell: label over a dot-matrix value. */
@Composable fun Readout(label: String, value: String, modifier: Modifier = Modifier, color: Color? = null) {
    Column(modifier.background(relay().carbon2, RoundedCornerShape(14.dp)).padding(horizontal = 12.dp, vertical = 10.dp).semantics(mergeDescendants = true) { contentDescription = "$label $value" },
        verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Lbl(label, ink(3)); Dm(value, 26, color ?: ink(1))
    }
}

/** Send a web address to the laptop's browser. */
@Composable fun OpenLinkSheet(s: UiState, host: Host, onDone: () -> Unit) {
    val ctx = host.context
    var fromClip by rememberSaveable { mutableStateOf(false) }
    var link by rememberSaveable {
        val clip = runCatching { ctx.getSystemService(ClipboardManager::class.java).primaryClip?.getItemAt(0)?.coerceToText(ctx)?.toString()?.trim() }.getOrNull().orEmpty()
        val ok = clip.startsWith("https://") || clip.startsWith("http://")
        fromClip = ok
        mutableStateOf(if (ok) clip else "")
    }
    val valid = Uri.parse(link.trim()).let { it.scheme in listOf("http", "https") && !it.host.isNullOrBlank() }
    val name = s.laptop?.name ?: "laptop"
    Sheet(onDone, footer = {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Key("Cancel", Modifier.weight(1f), Tone.Carbon, height = 56.dp, onClick = onDone)
            Key("Open there", Modifier.weight(2f), Tone.Signal, trailingIcon = "arrow-up", height = 56.dp, enabled = valid && s.laptop?.lan == true) {
                if (Connect.send("kdeconnect.share.request", org.json.JSONObject().put("url", link.trim()))) { host.snack(Snack("Opened on $name", "success")); onDone() }
                else host.snack(Snack("No link to $name", "danger"))
            }
        }
    }) {
        Row(Modifier.padding(horizontal = 4.dp), verticalAlignment = Alignment.Bottom) {
            Title("Open on the laptop", size = 24, modifier = Modifier.weight(1f)); Lbl("Its browser")
        }
        Field(link, "Address", "https://", error = if (link.isNotBlank() && !valid) "Start with https:// or http://" else null, tag = if (fromClip) "From clipboard" else null,
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri)) { link = it; fromClip = false }
    }
}

// ------------------------------------------------------------------ Clip

@Composable fun ClipboardScreen(s: UiState, host: Host) {
    val ctx = host.context
    val r = relay()
    host.revision
    var filter by rememberSaveable { mutableIntStateOf(0) }
    val on = Prefs.on(ctx, "clipboard")
    val auto = Prefs.clipMode(ctx) == "auto"
    val connected = s.laptop?.connected == true
    val ready = ClipboardSync.ready(ctx)
    val active = auto && ClipboardSync.automaticActive
    TabBody {
        Head {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Lbl("Clipboard · last 24 h", modifier = Modifier.weight(1f))
                if (s.clips.isNotEmpty()) Key("Clear", tone = Tone.Carbon, height = 34.dp, size = 12, radius = 999.dp) {
                    Connect.clearClips(); host.snack(Snack("History cleared", "success"))
                }
            }
            Row(verticalAlignment = Alignment.Bottom) {
                Title("Clip", size = 36, modifier = Modifier.weight(1f))
                Row(Modifier.background(r.carbon2, RoundedCornerShape(14.dp)).padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Led(when { !on -> LedState.Off; active -> LedState.Live; auto -> LedState.Warn; else -> LedState.Signal }, blink = false)
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Dm(when { !on -> "OFF"; !auto -> "TAP"; active -> "AUTO"; else -> "PAUSED" }, 22)
                        Lbl(when { !on -> "Not crossing"; !auto -> "Tap to send"; active -> "Copy here, paste there"; else -> "Automatic, paused" }, ink(3), size = 10)
                    }
                }
            }
            T(when {
                !on -> "Nothing you copy crosses to the laptop, and the laptop's copies stay there."
                !auto -> "Send with Send to laptop in the text menu, the quick-settings tile, Share, or the key below."
                active -> "Copy anything on the phone and it's on the laptop."
                !connected -> "Paused while the laptop is away. It picks up again when they reconnect."
                !ready -> "The phone restarted or a permission changed. Allow log access once to switch automatic sending back on."
                else -> "Paused while the screen was off. Resume to start again."
            }, 14, color = ink(2))
            when {
                !on -> Key("Turn on clipboard", Modifier.fillMaxWidth(), Tone.Carbon, height = 48.dp, size = 14) { Prefs.set(ctx, "clipboard", true); host.refresh() }
                !auto -> Key("Make it automatic", Modifier.fillMaxWidth(), Tone.Carbon, icon = "bolt", height = 48.dp, size = 14) { host.go("clip-setup") }
                !active && connected -> Key("Resume automatic", Modifier.fillMaxWidth(), Tone.Carbon, icon = "refresh", height = 48.dp, size = 14) {
                    if (!ready) host.go("clip-setup") else { ClipboardSync.resume(ctx); host.refresh() }
                }
            }
        }
        Carbon(Modifier.fillMaxWidth().height(22.dp)) {
            Box(Modifier.padding(horizontal = 24.dp, vertical = 6.dp).fillMaxSize().background(Color.Black, RoundedCornerShape(6.dp)))
        }
        PaperBody(16.dp) {
            Key("Send what I copied", Modifier.fillMaxWidth(), Tone.Signal, icon = "arrow-up", height = 58.dp, size = 16, enabled = connected && on) { sendClipboard(host) }
            ThreeWay(listOf("All", "↑ Sent", "↓ Got"), filter, "Show") { filter = it }
            val clips = s.clips.filter { System.currentTimeMillis() - it.at < 86_400_000 && (filter == 0 || it.toLaptop == (filter == 1)) }
            if (clips.isEmpty()) Empty(if (s.clips.isEmpty()) "Nothing copied yet" else "Nothing this way yet", "Copy on either device and it shows up here.")
            clips.forEach { c -> ClipSlip(c, s.clips.size - s.clips.indexOf(c), copyAction(host, c)) }
            Lbl("History stays on this phone for 24 h · passwords are never kept", ink(3), Modifier.fillMaxWidth().padding(top = 6.dp), maxLines = 3, align = TextAlign.Center)
        }
    }
}

/** A recessed three-position slide: the thumb springs between options. */
@Composable fun ThreeWay(options: List<String>, value: Int, label: String, onChange: (Int) -> Unit) {
    val r = relay()
    val view = LocalView.current
    BoxWithConstraints(Modifier.fillMaxWidth().height(48.dp).background(r.paper3, RoundedCornerShape(14.dp)).padding(4.dp).semantics { contentDescription = label }) {
        val w = maxWidth / options.size
        val x by animateDpAsState(w * value, spring(dampingRatio = 0.6f, stiffness = 600f), label = "thumb")
        Box(Modifier.offset(x = x).width(w).fillMaxHeight().padding(bottom = 2.dp).drawBehind { drawRoundRect(r.edge, Offset(0f, 2.dp.toPx()), size, CornerRadius(10.dp.toPx())) }.background(r.cap, RoundedCornerShape(10.dp)))
        Row(Modifier.fillMaxSize()) {
            options.forEachIndexed { i, o ->
                Box(Modifier.weight(1f).fillMaxHeight().clip(RoundedCornerShape(10.dp)).clickable(role = androidx.compose.ui.semantics.Role.RadioButton) { if (i != value) { Haptics.tick(view); onChange(i) } }
                    .semantics { selected = i == value }, contentAlignment = Alignment.Center) { Lbl(o, if (i == value) r.ink else r.ink2) }
            }
        }
    }
}

/** Nothing here yet: a serif line and a hint. */
@Composable fun Empty(title: String, body: String) {
    Column(Modifier.fillMaxWidth().background(relay().paper2, RoundedCornerShape(20.dp)).padding(vertical = 28.dp, horizontal = 20.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Ser(title, 26); T(body, 14, color = ink(2), align = TextAlign.Center)
    }
}

// ------------------------------------------------------------------ Files

@Composable fun TransfersScreen(s: UiState, host: Host) {
    host.revision
    val waiting = s.transfers.filter { Share.waiting().containsKey(it.id) }
    val now = s.transfers.filter { moving(it) }
    val done = s.transfers - now.toSet() - waiting.toSet()
    TabBody {
        Head {
            Row(verticalAlignment = Alignment.Bottom) {
                Title("Files", size = 36, modifier = Modifier.weight(1f))
                val today = s.transfers.filter { it.finished && it.error == null && android.text.format.DateUtils.isToday(it.at) }
                Counter("${today.count { it.upload }}", "↑ Today")
                Spacer(Modifier.width(14.dp))
                Counter("${today.count { !it.upload }}", "↓ Today")
            }
            Row {
                Spacer(Modifier.weight(1f))
                Key("Send files up", tone = Tone.Signal, icon = "arrow-up", height = 40.dp, size = 14, radius = 999.dp, enabled = s.laptop?.lan == true && Prefs.on(host.context, "files")) { host.pickFiles() }
            }
            Column(Modifier.fillMaxWidth().background(relay().carbon2, RoundedCornerShape(14.dp)).padding(horizontal = 12.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row { Lbl("↓ Lands here in", modifier = Modifier.weight(1f)); MonoText("Download/EchoConnect", 12) }
                Row { Lbl("↑ Lands there in", modifier = Modifier.weight(1f)); MonoText("~/Downloads/Phone", 12) }
            }
            waiting.forEach { TransferTicket(it, host) }
        }
        if (now.isNotEmpty()) Seam(SeamMode.Moving, height = 40.dp, description = "Moving files") else Seam(if (s.laptop?.connected == true) SeamMode.Live else SeamMode.Flat, height = 40.dp, description = "Link")
        PaperBody(16.dp) {
            if (s.transfers.isEmpty()) Empty("Nothing sent yet", "Send from Home, Share › EchoConnect, or EchoFiles on the laptop.")
            if (now.isNotEmpty()) { Lbl("Moving now"); now.forEach { TransferTicket(it, host) } }
            done.groupBy { dayLabel(it.at) }.forEach { (day, items) ->
                Lbl(day, modifier = Modifier.padding(top = 6.dp)); items.forEach { TransferTicket(it, host) }
            }
            Lbl("Unless EchoFiles is set to another folder", ink(3), Modifier.fillMaxWidth().padding(top = 6.dp), align = TextAlign.Center)
        }
    }
}

// ------------------------------------------------------------------ Panel

@Composable fun SettingsScreen(s: UiState, host: Host, forget: () -> Unit) {
    val ctx = host.context
    val r = relay()
    host.revision
    val l = s.laptop
    TabBody {
        Head {
            Title("Panel", size = 36)
            Column(Modifier.fillMaxWidth().background(r.carbon2, RoundedCornerShape(16.dp)).padding(14.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (l != null) {
                    Row(verticalAlignment = Alignment.Bottom) {
                        T(l.name, 20, 600, modifier = Modifier.weight(1f), maxLines = 1)
                        Lbl(if (l.connected) l.ip.ifBlank { "Linked" } else "Not nearby", ink(3))
                    }
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { Led(if (l.connected && l.lan) LedState.Live else LedState.Off); Lbl("Wi‑Fi · files, texts, alerts", ink(2)) }
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { Led(if (l.connected && l.bluetooth) LedState.Live else LedState.Off); Lbl("BT · calls, clipboard, off Wi‑Fi too", ink(2)) }
                } else T("No laptop paired yet.", 15, color = ink(2))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key(if (l == null) "Pair a laptop" else "Pair another", Modifier.weight(1f), if (l == null) Tone.Signal else Tone.Carbon, icon = "qr", height = 44.dp, size = 14) { host.go("pair") }
                    if (l != null) Key("Cut the link", Modifier.weight(1f), Tone.Carbon, icon = "unlink", height = 44.dp, size = 14, onClick = forget)
                }
            }
        }
        SigBar()
        Column(Modifier.fillMaxWidth().sideSafe().padding(start = 12.dp, end = 12.dp, top = 18.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            val onCount = FEATURES.count { Prefs.on(ctx, it.key, it.default) }
            PlateGroup("Channels · shared with the laptop", right = { Dm("$onCount/${FEATURES.size}", 16, r.sigInk) }) {
                FEATURES.chunked(2).forEachIndexed { row, pair ->
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.height(IntrinsicSize.Min)) {
                        pair.forEachIndexed { col, f -> ChannelModule(row * 2 + col + 1, f, host, Modifier.weight(1f).fillMaxHeight()) }
                    }
                }
            }
            T("Changes save at once and show in EchoFiles › Settings › Phone. Switch one off and it stops on both.", 13, color = ink(2), modifier = Modifier.padding(horizontal = 6.dp))
            PlateRow("Notification mixer", "Choose apps that stay on this phone · ${Prefs.mutedApps(ctx).size} muted", onClick = { host.go("notify-apps") }) { Glyph("chevron-right", color = ink(3)) }

            PlateGroup("Receiving") {
                PlateRow("Save to", "Download/EchoConnect")
                PlateRow("Accept automatically", if (Prefs.autoAccept(ctx)) "From your paired laptop" else "Asks before each file") {
                    Slide(Prefs.autoAccept(ctx), "Accept files automatically") { Prefs.setAutoAccept(ctx, it); host.refresh() }
                }
                val backup = Prefs.on(ctx, "backup", false)
                val paused = backup && !featureReady(ctx, "backup")
                PlateRow("Camera‑roll backup", if (paused) "Paused · allow all photos and videos" else "To ~/Pictures/Phone/Backup on Wi‑Fi while charging",
                    subColor = if (paused) r.warnInk else null, onClick = if (paused) ({ host.requestPhotos() }) else null) {
                    Slide(backup, "Camera-roll backup", needs = paused) { v -> Prefs.set(ctx, "backup", v); Connect.send("echofiles.settings", org.json.JSONObject().put("backup", v)); if (v && !featureReady(ctx, "backup")) host.requestPhotos(); host.refresh() }
                }
            }

            PlateGroup("With the laptop") {
                val dnd = Prefs.on(ctx, "dnd", false)
                val dndNeeds = dnd && !featureReady(ctx, "dnd")
                PlateRow("Do Not Disturb sync", if (dndNeeds) "Needs Do Not Disturb access · tap to allow" else "Quiet on one, quiet on both", subColor = if (dndNeeds) r.warnInk else null,
                    onClick = if (dndNeeds) ({ allowFeature(host, "dnd") }) else null) {
                    Slide(dnd, "Do Not Disturb sync", needs = dndNeeds) { v -> Prefs.set(ctx, "dnd", v); Connect.send("echofiles.settings", org.json.JSONObject().put("dnd", v)); if (v && !featureReady(ctx, "dnd")) allowFeature(host, "dnd"); host.refresh() }
                }
                val lock = Prefs.on(ctx, "autolock", false)
                PlateRow("Lock it when I leave", "Locks the laptop when this phone disconnects") {
                    Slide(lock, "Lock laptop when away") { v -> Prefs.set(ctx, "autolock", v); Connect.send("echofiles.settings", org.json.JSONObject().put("autolock", v)); host.refresh() }
                }
            }

            PlateGroup("Look") {
                val mode = if (Prefs.themeMode(ctx) == "laptop") 0 else 1
                Column(Modifier.fillMaxWidth().background(r.cap, RoundedCornerShape(14.dp)).padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    ThreeWay(listOf("Match laptop", "Relay only"), mode, "Theme") { Prefs.setThemeMode(ctx, if (it == 0) "laptop" else "phone"); host.refresh() }
                    T(when {
                        mode == 1 -> "Carbon and paper, with the signal in vermilion."
                        s.theme == null -> "The top half takes the laptop's Omarchy theme once it connects."
                        else -> "The top half wears the laptop's Omarchy theme."
                    }, 13, color = ink(2))
                }
            }

            PlateGroup("This phone") {
                val perms = permissions(ctx)
                PlateRow("Permissions", "What Android allows", onClick = { host.go("permissions") }) { Dm("${perms.count { it.granted }}/${perms.size}", 20); Glyph("chevron-right", color = ink(3)) }
                val unrestricted = batteryUnrestricted(ctx)
                PlateRow("Stay connected", if (samsung) "Battery: Unrestricted, and Never sleeping apps" else "Battery: Unrestricted") {
                    if (unrestricted) { Led(LedState.Live); Lbl("Set", r.liveInk) } else Key("Allow", tone = Tone.Signal, height = 36.dp, size = 13, radius = 11.dp) { requestUnrestricted(host) }
                }
                PlateRow("Automatic clipboard", "One-time setup with EchoFiles", onClick = { host.go("clip-setup") }) { Glyph("chevron-right", color = ink(3)) }
            }

            Column(Modifier.fillMaxWidth().drawBehind {
                drawRoundRect(r.paper3, cornerRadius = CornerRadius(16.dp.toPx()), style = Stroke(1.5.dp.toPx(), pathEffect = androidx.compose.ui.graphics.PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 5.dp.toPx()))))
            }.clickable(role = androidx.compose.ui.semantics.Role.Button) { host.open("https://$SOURCE") }.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row { Lbl("EchoConnect ${BuildConfig.VERSION_NAME}", ink(1), Modifier.weight(1f)); Lbl("MIT", ink(3)) }
                Lbl("Android ${Build.VERSION.RELEASE} · ${Build.MODEL}", ink(3))
                MonoText("$SOURCE ↗", 12, ink(2))
                Ser("Open source. Read every line.", 18, ink(2))
            }
        }
    }
}

/** One channel on the panel: number, LED, name, state, and its slide switch. */
@Composable fun ChannelModule(n: Int, f: Feature, host: Host, modifier: Modifier) {
    val ctx = host.context
    val r = relay()
    val on = Prefs.on(ctx, f.key, f.default)
    val ready = featureReady(ctx, f.key)
    val needs = on && !ready
    val sub = when {
        !on -> "Off"
        !ready -> "Needs access · tap"
        f.key == "photos" && !app.echoconnect.photos.Photos.fullAccess(ctx) -> "Only allowed items · tap"
        f.key == "clipboard" -> if (Prefs.clipMode(ctx) == "auto") "Automatic" + if (ClipboardSync.automaticActive) "" else " · paused" else "Tap to send"
        else -> f.what
    }
    val toggle: (Boolean) -> Unit = { v ->
        Prefs.set(ctx, f.key, v)
        Connect.send("echofiles.settings", org.json.JSONObject().put(f.key, v))
        if (v && !featureReady(ctx, f.key)) allowFeature(host, f.key)
        host.refresh()
    }
    val open: (() -> Unit)? = when {
        needs -> ({ allowFeature(host, f.key) })
        on && f.key == "photos" -> ({ host.requestPhotos() })
        f.key == "clipboard" -> ({ host.go("clipboard") })
        f.key == "notifications" && on -> ({ host.go("notify-apps") })
        else -> null
    }
    Column(modifier.heightIn(min = 124.dp).clip(RoundedCornerShape(16.dp)).background(r.cap)
        .then(if (needs) Modifier.border(2.dp, r.warn, RoundedCornerShape(16.dp)) else Modifier)
        .then(if (open != null) Modifier.clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = open) else Modifier)
        .padding(12.dp), verticalArrangement = Arrangement.SpaceBetween) {
        Row(verticalAlignment = Alignment.CenterVertically) { Lbl("CH %02d".format(n), modifier = Modifier.weight(1f)); Led(when { needs -> LedState.Warn; on -> LedState.Signal; else -> LedState.Off }) }
        Column(Modifier.padding(vertical = 6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(f.title, 15, 600, maxLines = 1); Lbl(sub, if (needs) r.warnInk else r.ink3, size = 10, maxLines = 2)
        }
        Box(Modifier.offset(x = (-2).dp)) { Slide(on, f.title, needs = needs, onChange = toggle) }
    }
}

// ------------------------------------------------------------------ the mixer

@Composable fun NotificationAppsScreen(host: Host, back: () -> Unit) {
    val ctx = host.context
    val r = relay()
    host.revision
    val pm = ctx.packageManager
    // Meters follow new alerts as they arrive, and a minute tick lets old ones age out of the hour.
    val posted by app.echoconnect.notifications.Notifications.version.collectAsStateWithLifecycle()
    var minute by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) { while (true) { kotlinx.coroutines.delay(60_000); minute++ } }
    val apps = remember(host.revision, posted, minute) {
        (app.echoconnect.notifications.Notifications.listener?.activeNotifications.orEmpty().map { it.packageName } + app.echoconnect.notifications.Notifications.recentApps()).distinct().filter { it != ctx.packageName }
            .map { pkg -> pkg to runCatching { pm.getApplicationLabel(pm.getApplicationInfo(pkg, 0)).toString() }.getOrDefault(pkg) }.sortedBy { it.second.lowercase() }
    }
    val muted = Prefs.mutedApps(ctx)
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Head(back) {
            Row(verticalAlignment = Alignment.Bottom) {
                Title("The", "mixer.", size = 32, modifier = Modifier.weight(1f))
                Column(horizontalAlignment = Alignment.End) { Dm("${apps.count { it.first in muted }} M", 30, r.sig); Lbl("muted") }
            }
            T("Every app with a notification gets a channel; its meter shows the last hour. Press M to keep it on this phone.", 14, color = ink(2))
        }
        SigBar()
        PaperBody(18.dp) {
            if (!featureReady(ctx, "notifications")) Banner("Notification access is off", "Allow it so the laptop can show and answer your notifications.") {
                Key("Open setting ↗", height = 40.dp, size = 13) { allowFeature(host, "notifications") }
            }
            if (apps.isEmpty()) Empty("No notifications right now", "Apps show up here while they have one.")
            else Carbon(Modifier.fillMaxWidth().clip(RoundedCornerShape(22.dp))) {
                Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp, vertical = 10.dp), horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                    apps.forEach { (pkg, name) ->
                        val off = pkg in muted
                        Strip(name, off, app.echoconnect.notifications.Notifications.recent(pkg)) { m -> Prefs.setMutedApps(ctx, muted.toMutableSet().apply { if (m) add(pkg) else remove(pkg) }); host.refresh() }
                    }
                }
            }
            if (apps.isNotEmpty()) Lbl("Meters · alerts in the last hour · lit M stays on this phone", ink(3), Modifier.fillMaxWidth(), maxLines = 2, align = TextAlign.Center)
        }
        Spacer(Modifier.height(32.dp).windowInsetsPadding(WindowInsets.navigationBars))
    }
}

/** One app as a channel strip: its letter, its name up the strip, and a mute key. */
@Composable private fun Strip(name: String, muted: Boolean, recent: Int, onMute: (Boolean) -> Unit) {
    val r = relay()
    val view = LocalView.current
    val hue = (name.hashCode() and 0xFFFFFF).let { listOf(0xFFFF9A8A, 0xFFE8B56A, 0xFFB8BFCC, 0xFF8DB1FF, 0xFFE9A0DD, 0xFF8DC4FF, 0xFF7EE3A8)[it % 7] }
    val down by animateDpAsState(if (muted) 3.dp else 0.dp, tween(60), label = "m")
    Column(Modifier.width(48.dp).background(r.carbon2, RoundedCornerShape(14.dp)).padding(vertical = 10.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(Modifier.size(32.dp).alpha(if (muted) 0.35f else 1f).background(Color(hue), RoundedCornerShape(10.dp)), contentAlignment = Alignment.Center) { T(name.take(1).uppercase(), 14, 700, Color(0xFF121211)) }
        Column(Modifier.alpha(if (muted) 0.35f else 1f).semantics { contentDescription = "$recent in the last hour" }, verticalArrangement = Arrangement.spacedBy(3.dp)) {
            for (i in 9 downTo 0) Box(Modifier.size(14.dp, 6.dp).background(when { i >= recent.coerceAtMost(10) -> r.carbon3; i >= 7 -> r.warn; else -> r.live }, RoundedCornerShape(2.dp)))
        }
        Box(Modifier.height(96.dp).width(20.dp), contentAlignment = Alignment.Center) {
            Lbl(name, r.chalk2, Modifier.requiredWidth(96.dp).rotate(-90f), maxLines = 1, align = TextAlign.Start)
        }
        Box(Modifier.padding(bottom = 3.dp).size(38.dp).offset(y = down)
            .drawBehind { drawRoundRect(if (muted) lerp(r.sig, Color.Black, 0.5f) else Color.Black, Offset(0f, (3.dp - down).toPx()), size, CornerRadius(10.dp.toPx())) }
            .background(if (muted) r.sig else r.capd, RoundedCornerShape(10.dp))
            .clickable(role = androidx.compose.ui.semantics.Role.Switch) { Haptics.key(view); onMute(!muted) }
            .semantics { contentDescription = "Keep $name on this phone"; stateDescription = if (muted) "Muted on the laptop" else "Shown on the laptop" },
            contentAlignment = Alignment.Center) { T("M", 14, 700, if (muted) r.onSig else r.chalk3) }
    }
}

// ------------------------------------------------------------------ Permissions

@Composable fun PermissionsScreen(host: Host, back: () -> Unit) {
    val ctx = host.context
    val r = relay()
    host.revision
    val perms = permissions(ctx)
    val allowed = perms.count { it.granted }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Head(back) {
            Row(verticalAlignment = Alignment.Bottom) { Title("Permissions", size = 32, modifier = Modifier.weight(1f)); Dm("$allowed/${perms.size}", 38, r.live) }
            Row(Modifier.fillMaxWidth().semantics { contentDescription = "$allowed of ${perms.size} allowed" }, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                perms.forEach { p -> Box(Modifier.weight(1f).height(12.dp).background(if (p.granted) r.live else r.carbon3, RoundedCornerShape(3.dp))) }
            }
            T("Each one unlocks one channel. Switch a channel off on the Panel and EchoConnect stops using its permission.", 14, color = ink(2))
        }
        SigBar()
        Column(Modifier.fillMaxWidth().sideSafe().padding(start = 12.dp, end = 12.dp, top = 18.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val missing = perms.filter { !it.granted }
            if (missing.isNotEmpty()) {
                Lbl("● Waiting on you · ${missing.size}", r.warnInk, Modifier.padding(horizontal = 4.dp))
                missing.forEach { p ->
                    Row(Modifier.fillMaxWidth().heightIn(min = 64.dp).background(r.cap, RoundedCornerShape(14.dp)).padding(start = 14.dp, end = 10.dp, top = 10.dp, bottom = 10.dp),
                        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Led(if (p.optional) LedState.Off else LedState.Warn)
                        Column(Modifier.weight(1f)) {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) { T(p.title, 15, 600, maxLines = 1, modifier = Modifier.weight(1f, false)); if (p.optional) Lbl("Optional", r.ink3, size = 10) }
                            T(p.why, 13, color = ink(2))
                        }
                        Key(p.action, tone = if (p.optional || p.action.endsWith("↗")) Tone.Paper else Tone.Signal, height = 38.dp, size = 13, radius = 12.dp) { p.request(host) }
                    }
                }
            }
            val ok = perms.filter { it.granted }
            if (ok.isNotEmpty()) {
                Lbl("Allowed · ${ok.size}", modifier = Modifier.padding(start = 4.dp, end = 4.dp, top = 12.dp))
                ok.chunked(2).forEach { pair ->
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        pair.forEach { p -> Row(Modifier.weight(1f).background(r.paper2, RoundedCornerShape(12.dp)).padding(horizontal = 12.dp, vertical = 11.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { Led(LedState.Live); Lbl(p.title, r.ink) } }
                        if (pair.size == 1) Spacer(Modifier.weight(1f))
                    }
                }
            }
            Spacer(Modifier.height(6.dp))
            if (samsung) Note("On Samsung", "Settings › Battery › Background usage limits › Never sleeping apps › add EchoConnect. Otherwise One UI may cut the link overnight.")
            else Note("Battery", "Settings › Apps › EchoConnect › Battery › Unrestricted keeps the link while the screen is off.")
        }
        Spacer(Modifier.height(32.dp).windowInsetsPadding(WindowInsets.navigationBars))
    }
}

// ------------------------------------------------------------------ Automatic clipboard setup

@OptIn(ExperimentalLayoutApi::class)
@Composable fun ClipboardSetupScreen(s: UiState, host: Host, back: () -> Unit) {
    val ctx = host.context
    val r = relay()
    host.revision
    LaunchedEffect(Unit) { while (true) { kotlinx.coroutines.delay(1500); host.refresh() } }
    val dev = Settings.Global.getInt(ctx.contentResolver, Settings.Global.DEVELOPMENT_SETTINGS_ENABLED, 0) != 0
    val adbWifi = Settings.Global.getInt(ctx.contentResolver, "adb_wifi_enabled", 0) != 0
    val logs = granted(ctx, "android.permission.READ_LOGS")
    val overlay = Settings.canDrawOverlays(ctx)
    val current = when {
        !logs && !dev -> 0
        !logs && !adbWifi -> 1
        !logs -> 2
        !overlay -> 3
        dev -> 4
        else -> 5
    }
    val laptop = s.laptop?.name ?: "your laptop"
    val auto = Prefs.clipMode(ctx) == "auto"
    class St(val title: String, val done: String, val sub: String, val body: @Composable ColumnScope.() -> Unit)
    val steps = listOf(
        St("Turn on Developer options", "On", "Tap Build number 7 times") {
            T("Tap Build number 7 times, then enter your PIN.", 14, color = ink(2))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Kc("Settings"); Kc("About phone"); if (samsung) Kc("Software information"); Kc("Build number ×7", hot = true)
            }
            Key(if (samsung) "Open Software information ↗" else "Open About phone ↗", Modifier.fillMaxWidth(), Tone.Carbon, height = 46.dp, size = 14) { host.settings(Settings.ACTION_DEVICE_INFO_SETTINGS) }
        },
        St("Turn on Wireless debugging", "On", "Allow on this Wi‑Fi") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Kc("Developer options"); Kc("Wireless debugging", hot = true); Kc("Allow on this Wi‑Fi")
            }
            Key("Open Developer options ↗", Modifier.fillMaxWidth(), Tone.Carbon, height = 46.dp, size = 14) { host.settings(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS) }
        },
        St("Scan the laptop's code from Wireless debugging", "Permission granted", "From the laptop's EchoFiles") {
            Lbl("On the laptop", ink(3))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) { Kc("EchoFiles"); Kc("Settings"); Kc("Phone"); Kc("Make clipboard automatic", hot = true) }
            Lbl("On this phone", ink(3))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) { Kc("Wireless debugging"); Kc("Pair device with QR code", hot = true) }
            Row(Modifier.fillMaxWidth().background(Color.Black, RoundedCornerShape(12.dp)).padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                DotLoader(); Lbl("Waiting for $laptop", OnBlack2)
            }
        },
        St("Allow Display over other apps", "Allowed", "Opens a hidden window for a moment") {
            T("A normal setting. It lets EchoConnect open an invisible window for a moment to read what you copied.", 14, color = ink(2))
            Key("Open the setting ↗", Modifier.fillMaxWidth(), Tone.Signal, height = 46.dp, size = 14) { host.settings(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, true) }
        },
        St("Turn Developer options off", "Off · banking apps work normally", "The permission stays") {
            T("The permission stays. Banking and payment apps only check whether Developer options are on now.", 14, color = ink(2))
            Key("Open Developer options ↗", Modifier.fillMaxWidth(), Tone.Signal, height = 46.dp, size = 14) { host.settings(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS) }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { Led(LedState.Warn); Lbl("Still on · checking by itself", r.warn) }
        },
    )
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Head(back) {
            Row(verticalAlignment = Alignment.Bottom) {
                Title("Make clipboard", "automatic.", size = 30, modifier = Modifier.weight(1f))
                Column(horizontalAlignment = Alignment.End) { Dm("${current.coerceAtMost(5)}/5", 34, r.sig); Lbl("≈ 2 min, once") }
            }
            T("EchoConnect watches each step and ticks it off by itself.", 14, color = ink(2))
        }
        SigBar()
        Column(Modifier.fillMaxWidth().sideSafe().padding(start = 16.dp, end = 16.dp, top = 22.dp), verticalArrangement = Arrangement.spacedBy(18.dp)) {
            steps.forEachIndexed { i, st ->
                val state = if (i < current) 0 else if (i == current) 1 else 2
                Row(Modifier.alpha(if (state == 2) 0.55f else 1f), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Dm("%02d".format(i + 1), 30, when (state) { 0 -> r.liveInk; 1 -> r.sigInk; else -> r.ink3 }, Modifier.width(44.dp))
                    when (state) {
                        0 -> Column(Modifier.weight(1f).padding(top = 4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            T(st.title, 15, 600, r.ink2, strike = true)
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { Led(LedState.Live); Lbl(st.done, r.liveInk) }
                        }
                        1 -> Carbon(Modifier.weight(1f).clip(RoundedCornerShape(20.dp))) {
                            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { Led(LedState.Signal, blink = true); Lbl("Now", r.sig) }
                                T(st.title, 18, 600)
                                st.body(this)
                            }
                        }
                        else -> Column(Modifier.weight(1f).padding(top = 4.dp)) { T(st.title, 15, 600); T(st.sub, 13, color = ink(2)) }
                    }
                }
            }
            if (current >= 5) {
                if (auto) Banner("Automatic is on", "Copy anything on the phone and it's on the laptop.", LedState.Live)
                else Key("Turn on automatic clipboard", Modifier.fillMaxWidth(), Tone.Signal, icon = "bolt", height = 60.dp, size = 16) {
                    Prefs.setClipMode(ctx, "auto"); Prefs.set(ctx, "clipboard", true); ClipboardSync.stopReader(); ClipboardSync.resume(ctx)
                    host.snack(Snack("Automatic clipboard is on", "success")); host.go("clipboard")
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.height(IntrinsicSize.Min)) {
                Note("What it reads", "Only the one log line Android writes when a clipboard read is blocked. Nothing else is kept or sent.", Modifier.weight(1f).fillMaxHeight())
                Note("After a restart", "Android asks once. Tap “Allow one-time access” and it runs until the next restart.", Modifier.weight(1f).fillMaxHeight())
            }
            if (auto) Key("Switch to Tap to send", Modifier.fillMaxWidth(), height = 50.dp, size = 14) { Prefs.setClipMode(ctx, "manual"); ClipboardSync.stopReader(); host.go("clipboard") }
            else Key("Use Tap to send instead", Modifier.fillMaxWidth(), height = 50.dp, size = 14) { Prefs.setClipMode(ctx, "manual"); host.go("clipboard") }
        }
        Spacer(Modifier.height(32.dp).windowInsetsPadding(WindowInsets.navigationBars))
    }
}

// ------------------------------------------------------------------ the handshake (first run and pairing)

/** Welcome → tune in → match the code → switchboard → clipboard lever → linked. */
@Composable fun PairingScreen(s: UiState, host: Host, firstRun: Boolean, done: (setupClipboard: Boolean) -> Unit) {
    val ctx = host.context
    host.revision
    var step by rememberSaveable { mutableIntStateOf(if (firstRun) 0 else 1) }
    var typing by rememberSaveable { mutableStateOf(false) }
    var address by rememberSaveable { mutableStateOf("") }
    var autoClip by rememberSaveable { mutableStateOf(true) }
    val before = rememberSaveable { ArrayList(s.laptops.filter { it.paired }.map { it.id }) }
    val paired = s.laptops.firstOrNull { it.paired && it.id !in before }
    LaunchedEffect(s.pair != null) { if (s.pair != null && step in 0..2) step = 2 }
    LaunchedEffect(paired?.id) { if (paired != null && step in 1..2) step = 3 }
    LaunchedEffect(step) { if (step == 1) Connect.announce() }
    val goBack: () -> Unit = { if (step == 2) Connect.rejectPair(); if (!firstRun && step == 1) done(false) else step-- }
    androidx.activity.compose.BackHandler(step in 1..4) { goBack() }
    val name = paired?.name ?: s.pair?.name ?: "your laptop"
    when (step) {
        0 -> Welcome { step = 1 }
        1 -> TuneIn(s, host, typing, address, { address = it }, { typing = true }, goBack)
        2 -> MatchCode(s, paired, goBack, onContinue = { step = 3 }, onRestart = { step = 1 })
        3 -> Switchboard(host, goBack) { step = 4 }
        4 -> Lever(autoClip, { autoClip = it }, goBack) {
            if (autoClip) { Prefs.setPairedOnce(ctx); done(true) } else { Prefs.setClipMode(ctx, "manual"); step = 5 }
        }
        else -> Linked(name) { Prefs.setPairedOnce(ctx); done(false) }
    }
}

/** The step counter: five segments, the done ones lit. */
@Composable private fun Steps(n: Int) {
    val r = relay()
    Row(Modifier.semantics(mergeDescendants = true) { contentDescription = "Step $n of 5" }, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) { repeat(5) { i -> Box(Modifier.size(18.dp, 6.dp).background(if (i < n) r.sig else r.carbon3, RoundedCornerShape(2.dp))) } }
        Lbl("$n/5")
    }
}

/** A carbon header with Back and the step counter. */
@Composable private fun FlowHead(n: Int, back: () -> Unit, content: @Composable ColumnScope.() -> Unit) = Head(back, trailing = { Steps(n) }, content = content)

/** The bottom bar of a flow screen: keys sit above the navigation bar. */
@Composable private fun FlowFoot(content: @Composable ColumnScope.() -> Unit) =
    Column(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.navigationBars).sideSafe().padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 22.dp), verticalArrangement = Arrangement.spacedBy(8.dp), content = content)

@Composable private fun Welcome(onStart: () -> Unit) {
    val r = relay()
    val words = listOf("FILES", "CLIPBOARD", "TEXTS", "CALLS", "PHOTOS", "ALERTS")
    val t = rememberInfiniteTransition(label = "board")
    val k by t.animateFloat(0f, words.size.toFloat(), infiniteRepeatable(tween(1500 * words.size, easing = LinearEasing)), label = "k")
    val idx = k.toInt() % words.size
    val within = k - k.toInt()
    val roll = if (within > 0.85f) (within - 0.85f) / 0.15f else 0f
    Column(Modifier.fillMaxSize()) {
        Head {
            Row { Lbl("EchoConnect", modifier = Modifier.weight(1f)); Lbl("Relay") }
            Column(Modifier.fillMaxWidth().background(Color.Black, RoundedCornerShape(18.dp)).padding(horizontal = 16.dp, vertical = 18.dp).semantics(mergeDescendants = true) { contentDescription = "Files, clipboard, texts, calls, photos and alerts cross between them" },
                verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Lbl("Now crossing ↑↓", OnBlack3)
                Box(Modifier.fillMaxWidth().height(64.dp).clipToBounds()) {
                    Dm(words[idx], 52, r.sig, Modifier.align(Alignment.CenterStart).offset(y = (-64 * roll).dp), glow = true)
                    Dm(words[(idx + 1) % words.size], 52, r.sig, Modifier.align(Alignment.CenterStart).offset(y = (64 * (1 - roll)).dp), glow = true)
                }
                Row { Lbl("From · this phone", OnBlack3, Modifier.weight(1f)); Lbl("To · your laptop", OnBlack3) }
            }
        }
        Seam(SeamMode.Live, height = 56.dp, description = "")
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).sideSafe().padding(start = 22.dp, end = 22.dp, top = 26.dp), verticalArrangement = Arrangement.spacedBy(18.dp)) {
            Title("Your phone and your laptop,", "on one line.", size = 38)
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                listOf("Stays on your own network", "Encrypted · paired once", "Open source, every line").forEach {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) { Led(LedState.Live); Lbl(it, ink(2)) }
                }
            }
        }
        FlowFoot {
            Key("Start the handshake", Modifier.fillMaxWidth(), Tone.Signal, trailingIcon = "arrow-right", height = 66.dp, size = 18, start = true, onClick = onStart)
            Lbl("About one minute", modifier = Modifier.fillMaxWidth(), align = TextAlign.Center)
        }
    }
}

@Composable private fun TuneIn(s: UiState, host: Host, typing: Boolean, address: String, onAddress: (String) -> Unit, onType: () -> Unit, back: () -> Unit) {
    val r = relay()
    val view = LocalView.current
    val found = s.laptops.filter { !it.paired && it.connected }
    LaunchedEffect(found.size) { if (found.isNotEmpty()) Haptics.tick(view) }
    val needle by rememberInfiniteTransition(label = "tuner").animateFloat(0f, 1f, infiniteRepeatable(tween(3400, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "needle")
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            FlowHead(2, back) {
                Title("Tune in to", "your laptop.", size = 32)
                Carbon(Modifier.fillMaxWidth().height(150.dp).clip(RoundedCornerShape(18.dp)), Color.Black) {
                    Canvas(Modifier.fillMaxSize().semantics { contentDescription = if (found.isEmpty()) "Scanning this Wi-Fi" else "Found ${found.joinToString { it.name }}" }) {
                        val pad = 12.dp.toPx(); val bottom = size.height - 40.dp.toPx(); val w = size.width - 2 * pad
                        var x = pad; var i = 0
                        while (x <= size.width - pad) { val major = i % 5 == 0; drawRect(if (major) OnBlack2 else OnBlack3.copy(alpha = 0.5f), Offset(x, bottom - if (major) 34.dp.toPx() else 14.dp.toPx()), Size(if (major) 2.dp.toPx() else 1.dp.toPx(), if (major) 34.dp.toPx() else 14.dp.toPx())); x += 8.dp.toPx(); i++ }
                        found.forEachIndexed { j, _ ->
                            val sx = pad + w * (0.3f + 0.4f * ((j * 0.37f + 0.25f) % 1f))
                            drawCircle(r.live.copy(alpha = 0.3f), 10.dp.toPx(), Offset(sx, 40.dp.toPx())); drawCircle(r.live, 6.dp.toPx(), Offset(sx, 40.dp.toPx()))
                        }
                        val nx = pad + w * needle
                        drawRect(r.sig.copy(alpha = 0.3f), Offset(nx - 3.dp.toPx(), 14.dp.toPx()), Size(8.dp.toPx(), bottom - 14.dp.toPx()))
                        drawRect(r.sig, Offset(nx, 14.dp.toPx()), Size(2.dp.toPx(), bottom - 14.dp.toPx()))
                    }
                    Row(Modifier.fillMaxWidth().padding(14.dp)) { Lbl("Scanning this Wi‑Fi", OnBlack3, Modifier.weight(1f)); Dm(if (found.isEmpty()) "…" else "${found.size} FOUND", 18, r.live) }
                    Row(Modifier.fillMaxSize().padding(14.dp), verticalAlignment = Alignment.Bottom) { Lbl("LAN", OnBlack3, Modifier.weight(1f)); Lbl("EchoFiles · Connect phone", OnBlack3) }
                }
            }
            SigBar()
            PaperBody(20.dp) {
                T(strong("On the laptop, open EchoFiles and press **Connect phone** in the sidebar. Same Wi‑Fi on both."), 15, color = ink(2))
                if (found.isEmpty()) Row(Modifier.fillMaxWidth().background(r.paper2, RoundedCornerShape(16.dp)).padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    DotLoader(); T("Looking for laptops…", 14, color = ink(2), modifier = Modifier.weight(1f))
                    Key("Again", height = 36.dp, size = 13, radius = 11.dp) { Connect.announce() }
                }
                found.forEach { lap ->
                    Carbon(Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp))) {
                        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) { T(lap.name, 20, 600); Lbl(if (lap.ip.isNotBlank()) lap.ip else "EchoFiles") }
                                Led(LedState.Live, 10.dp)
                            }
                            Key("Tune in", Modifier.fillMaxWidth(), Tone.Signal, trailingIcon = "arrow-right", height = 52.dp) { Connect.pair(lap.id) }
                        }
                    }
                }
                if (typing) {
                    Carbon(Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp))) {
                        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            Field(address, "Laptop address", "192.168.1.20", keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri), onChange = onAddress)
                            Key("Connect to address", Modifier.fillMaxWidth(), Tone.Carbon, height = 48.dp, size = 14, enabled = address.isNotBlank()) {
                                val (h, port) = address.trim().split(":").let { it[0] to (it.getOrNull(1)?.toIntOrNull() ?: 1716) }
                                Work.run { if (!Connect.connect(h, port)) Work.main.post { host.snack(Snack("Couldn't reach $h. Check the address and that both are on the same Wi‑Fi.", "danger")) } }
                            }
                        }
                    }
                }
            }
        }
        FlowFoot {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Key("Read its code", Modifier.weight(1f), icon = "scan", height = 52.dp, size = 14) { host.scan() }
                Key("Type address", Modifier.weight(1f), icon = "keyboard", height = 52.dp, size = 14, enabled = !typing, onClick = onType)
            }
        }
    }
}

/** Two LED panels, the laptop's over the phone's, with = on the seam between them. */
@Composable private fun MatchCode(s: UiState, paired: Laptop?, back: () -> Unit, onContinue: () -> Unit, onRestart: () -> Unit) {
    val r = relay()
    val p = s.pair
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            FlowHead(3, back) {
                if (p != null) CodePanel("On ${p.name}", p.code, carbon = true)
                else Column(Modifier.fillMaxWidth().background(Color.Black, RoundedCornerShape(20.dp)).padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (paired != null) { Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { Led(LedState.Live); Lbl("Paired with ${paired.name}", r.live) } }
                    else { DotLoader(); Lbl("Waiting for the laptop", OnBlack3) }
                }
            }
            Carbon(Modifier.fillMaxWidth().height(56.dp)) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Box(Modifier.fillMaxWidth().height(2.dp).background(r.sig.copy(alpha = 0.5f)))
                    Box(Modifier.background(r.carbon).padding(horizontal = 14.dp)) { Dm("=", 40, r.sig) }
                }
            }
            PaperBody(22.dp) {
                if (p != null) {
                    CodePanel("On this phone", p.code, carbon = false)
                    Row(verticalAlignment = Alignment.Bottom) { T("Same six digits? ", 22, 500); Ser("Then they trust each other.", 24, ink(2)) }
                    if (!p.theyAsked) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) { DotLoader(); Lbl("Accept on ${p.name}", ink(2)) }
                }
            }
        }
        FlowFoot {
            when {
                p != null -> {
                    if (p.theyAsked) Key("Codes match", Modifier.fillMaxWidth(), Tone.Signal, icon = "check", height = 64.dp, size = 18) { Connect.acceptPair() }
                    Key(if (p.theyAsked) "They differ — stop" else "Cancel pairing", Modifier.fillMaxWidth(), height = 50.dp) { Connect.rejectPair(); onRestart() }
                }
                paired != null -> Key("Continue", Modifier.fillMaxWidth(), Tone.Signal, trailingIcon = "arrow-right", height = 62.dp, size = 17, onClick = onContinue)
                else -> Key("Back to tuning", Modifier.fillMaxWidth(), height = 50.dp, onClick = onRestart)
            }
        }
    }
}

/** A pairing code as an LED panel: glowing on carbon, printed on paper. It flickers on. */
@Composable fun CodePanel(label: String, code: String, carbon: Boolean) {
    val r = relay()
    val on = remember(code) { Animatable(0f) }
    LaunchedEffect(code) { on.animateTo(1f, tween(900, delayMillis = if (carbon) 0 else 250, easing = { f -> when { f < 0.4f -> 0f; f < 0.6f -> 0.6f; f < 0.75f -> 0.2f; else -> 1f } })) }
    val pairs = code.chunked(2)
    Column(Modifier.fillMaxWidth().background(if (carbon) Color.Black else if (r.night) Color(0xFF2E2C27) else Color(0xFFD9D3C5), RoundedCornerShape(20.dp)).padding(horizontal = 16.dp, vertical = 18.dp)
        .semantics(mergeDescendants = true) { contentDescription = "$label: " + pairs.joinToString(" ") }, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Lbl(label, if (carbon) OnBlack3 else r.ink3)
        Row(Modifier.fillMaxWidth().alpha(on.value), horizontalArrangement = Arrangement.SpaceBetween) {
            pairs.forEach { Dm(it, if (pairs.size > 3) 50 else 64, if (carbon) r.sig else r.ink, glow = carbon) }
        }
    }
}

/** Seven flip switches, one per permission, lit as Android allows each. */
@Composable private fun Switchboard(host: Host, back: () -> Unit, onContinue: () -> Unit) {
    val ctx = host.context
    val r = relay()
    val perms = permissions(ctx).take(7)
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            FlowHead(4, back) {
                Row(verticalAlignment = Alignment.Bottom) {
                    Title("The", "switchboard.", size = 32, modifier = Modifier.weight(1f))
                    Column(horizontalAlignment = Alignment.End) { Dm("${perms.count { it.granted }}/${perms.size}", 34, r.sig); Lbl("patched") }
                }
                T("Flip up what the laptop may see. Skip any; they're all on the Panel later.", 14, color = ink(2))
            }
            SigBar()
            Column(Modifier.fillMaxWidth().sideSafe().padding(start = 12.dp, end = 12.dp, top = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                perms.forEach { p -> Flip(p) { if (!p.granted) p.request(host) } }
            }
        }
        FlowFoot { Key("Continue", Modifier.fillMaxWidth(), Tone.Signal, trailingIcon = "arrow-right", height = 62.dp, size = 17, start = true, onClick = onContinue) }
    }
}

@Composable private fun Flip(p: Perm, onClick: () -> Unit) {
    val r = relay()
    val view = LocalView.current
    val y by animateDpAsState(if (p.granted) 3.dp else 27.dp, spring(dampingRatio = 0.5f, stiffness = 600f), label = "flip")
    Row(Modifier.fillMaxWidth().heightIn(min = 64.dp).drawBehind { drawRoundRect(r.edge, Offset(0f, 2.dp.toPx()), size, CornerRadius(14.dp.toPx())) }
        .background(r.cap, RoundedCornerShape(14.dp)).clickable(role = androidx.compose.ui.semantics.Role.Switch) { Haptics.tick(view); onClick() }
        .semantics(mergeDescendants = true) { stateDescription = if (p.granted) "Allowed" else "Not allowed" }
        .padding(start = 14.dp, end = 10.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Led(if (p.granted) LedState.Live else LedState.Off)
        Column(Modifier.weight(1f)) { T(p.title, 15, 600, maxLines = 1); T(p.why, 12, color = ink(2), line = 16, maxLines = 2) }
        if (p.action.endsWith("↗") && !p.granted) Lbl("↗ Android", r.ink3, size = 10)
        Box(Modifier.size(34.dp, 52.dp).background(if (p.granted) r.sig else r.paper3, RoundedCornerShape(9.dp))) {
            Box(Modifier.offset(x = 3.dp, y = y).size(28.dp, 22.dp).drawBehind { drawRoundRect(r.edge, Offset(0f, 2.dp.toPx()), size, CornerRadius(6.dp.toPx())) }.background(r.cap, RoundedCornerShape(6.dp)))
        }
    }
}

/** A heavy two-position lever: AUTO or TAP. */
@Composable private fun Lever(auto: Boolean, onChange: (Boolean) -> Unit, back: () -> Unit, onContinue: () -> Unit) {
    val r = relay()
    val view = LocalView.current
    val top by animateDpAsState(if (auto) 14.dp else 222.dp, spring(dampingRatio = 0.55f, stiffness = 300f), label = "lever")
    val pick: (Boolean) -> Unit = { if (it != auto) { Haptics.heavy(view); onChange(it) } }
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            FlowHead(5, back) {
                Title("How do copies", "cross over?", size = 32)
                Lbl("Laptop → phone is always automatic")
            }
            SigBar()
            Row(Modifier.fillMaxWidth().sideSafe().padding(start = 16.dp, end = 16.dp, top = 22.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Box(Modifier.size(104.dp, 356.dp).background(Color(0xFF121211), RoundedCornerShape(30.dp))
                    .toggleableLever(auto, pick).semantics { contentDescription = "Automatic clipboard"; stateDescription = if (auto) "Automatic" else "Tap to send" }) {
                    Box(Modifier.offset(x = 46.dp, y = 30.dp).size(12.dp, 296.dp).background(Color.Black, RoundedCornerShape(6.dp)))
                    Column(Modifier.offset(x = 10.dp, y = top).size(84.dp, 120.dp).drawBehind { drawRoundRect(r.edge, Offset(0f, 4.dp.toPx()), size, CornerRadius(24.dp.toPx())) }
                        .background(r.cap, RoundedCornerShape(24.dp)), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically)) {
                        repeat(3) { Box(Modifier.size(44.dp, 3.dp).background(r.paper3, RoundedCornerShape(2.dp))) }
                        Box(Modifier.padding(top = 6.dp)) { Led(if (auto) LedState.Signal else LedState.Off, 10.dp) }
                    }
                }
                Column(Modifier.weight(1f).height(356.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    LeverOption("AUTO", "Copy on the phone, paste on the laptop. No taps.", "2-min setup, once", auto, Modifier.weight(1f)) { pick(true) }
                    LeverOption("TAP", "Send to laptop from the text menu, the tile, or Share.", "No setup", !auto, Modifier.weight(1f)) { pick(false) }
                }
            }
        }
        FlowFoot { Key(if (auto) "Set up Automatic" else "Use Tap", Modifier.fillMaxWidth(), Tone.Signal, trailingIcon = "arrow-right", height = 62.dp, size = 17, start = true, onClick = onContinue) }
    }
}

private fun Modifier.toggleableLever(auto: Boolean, onChange: (Boolean) -> Unit) = this.clickable(role = androidx.compose.ui.semantics.Role.Switch) { onChange(!auto) }

@Composable private fun LeverOption(title: String, body: String, note: String, on: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val r = relay()
    Column(modifier.fillMaxWidth().alpha(if (on) 1f else 0.5f).clip(RoundedCornerShape(18.dp)).background(if (on) r.cap else Color.Transparent)
        .clickable(role = androidx.compose.ui.semantics.Role.RadioButton, onClick = onClick).semantics { selected = on }.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Dm(title, 30); T(body, 14, color = ink(2)); Spacer(Modifier.weight(1f)); Lbl(note)
    }
}

/** LINKED flickers on, the seam draws in, then each open channel lights in turn. */
@Composable private fun Linked(name: String, onDone: () -> Unit) {
    val ctx = LocalView.current.context
    val r = relay()
    val view = LocalView.current
    val draw = remember { Animatable(0f) }
    LaunchedEffect(Unit) { kotlinx.coroutines.delay(900); draw.animateTo(1f, tween(900, easing = CubicBezierEasing(0.6f, 0f, 0.2f, 1f))); Haptics.confirm(view) }
    val channels = FEATURES.filter { Prefs.on(ctx, it.key, it.default) && featureReady(ctx, it.key) }.map { it.title }
    Column(Modifier.fillMaxSize()) {
        Head {
            Lbl("Handshake complete · $name", modifier = Modifier.padding(top = 30.dp))
            CodeFlicker("LINKED")
            T("A small notification stays while they're linked. The line below is the link; it lives on Home.", 15, color = ink(2))
        }
        Box(Modifier.fillMaxWidth().height(72.dp).drawWithContent { clipRect(right = size.width * draw.value) { this@drawWithContent.drawContent() } }) {
            Seam(SeamMode.Live, height = 72.dp, description = "Linked")
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).sideSafe().padding(start = 16.dp, end = 16.dp, top = 20.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Lbl("Channels open", modifier = Modifier.padding(bottom = 4.dp))
            channels.chunked(2).forEach { pair ->
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    pair.forEach { c -> Row(Modifier.weight(1f).background(r.cap, RoundedCornerShape(14.dp)).padding(horizontal = 14.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) { Led(if (draw.value >= 1f) LedState.Live else LedState.Off); Lbl(c, r.ink) } }
                    if (pair.size == 1) Spacer(Modifier.weight(1f))
                }
            }
        }
        FlowFoot { Key("Go to Home", Modifier.fillMaxWidth(), Tone.Signal, trailingIcon = "arrow-right", height = 64.dp, size = 18, start = true, onClick = onDone) }
    }
}

@Composable private fun CodeFlicker(text: String) {
    val r = relay()
    val on = remember { Animatable(0f) }
    LaunchedEffect(Unit) { kotlinx.coroutines.delay(200); on.animateTo(1f, tween(700, easing = { f -> when { f < 0.3f -> 0f; f < 0.5f -> 0.5f; f < 0.7f -> 0.1f; else -> 1f } })) }
    Dm(text, 88, r.sig, Modifier.alpha(on.value).semantics { heading() }, glow = true)
}

/** Shown when a laptop asks to pair outside the first-run flow: it drops from the laptop's side. */
@Composable fun PairDialog(p: PairPrompt) {
    val r = relay()
    Sheet({ Connect.rejectPair() }, fromTop = true, footer = {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Key(if (p.theyAsked) "They differ" else "Cancel pairing", Modifier.weight(1f), Tone.Carbon, height = 56.dp) { Connect.rejectPair() }
            if (p.theyAsked) Key("Codes match", Modifier.weight(1.6f), Tone.Signal, height = 56.dp) { Connect.acceptPair() }
        }
    }) {
        Lbl("● Incoming handshake", r.sig)
        Title(p.name, if (p.theyAsked) "wants in." else "is checking.", size = 28)
        CodePanel(if (p.theyAsked) "Its screen should show" else "Accept on ${p.name}; both show", p.code, carbon = true)
    }
}

/** Forget, framed as what it is: cutting the link. Hold to cut. */
@Composable fun ForgetSheet(l: Laptop, onKeep: () -> Unit, onCut: () -> Unit) {
    val r = relay()
    Sheet(onKeep, carbon = false, footer = {
        HoldKey("Hold to cut", onDone = onCut)
        Key("Keep it", Modifier.fillMaxWidth(), height = 52.dp, onClick = onKeep)
        Lbl("Hold 1 s · it buzzes harder as it fills", modifier = Modifier.fillMaxWidth(), align = TextAlign.Center)
    }) {
        Carbon(Modifier.fillMaxWidth().height(70.dp).clip(RoundedCornerShape(18.dp))) {
            Canvas(Modifier.fillMaxSize().clearAndSetSemantics { }) {
                val mid = size.height / 2; val wl = 20.dp.toPx(); val cut = size.width * 0.5f
                val left = Path(); var x = 12.dp.toPx(); left.moveTo(x, mid)
                while (x < cut - 10.dp.toPx()) { left.lineTo(x, mid - 7.dp.toPx() * sin(2 * PI * x / wl).toFloat()); x += 3f }
                drawPath(left, r.sig, style = Stroke(2.5.dp.toPx()))
                val right = Path(); x = cut + 12.dp.toPx(); right.moveTo(x, mid + 6.dp.toPx())
                while (x < size.width - 12.dp.toPx()) { right.lineTo(x, mid + 6.dp.toPx() - 5.dp.toPx() * sin(2 * PI * x / wl).toFloat()); x += 3f }
                drawPath(right, r.chalk.copy(alpha = 0.3f), style = Stroke(2.5.dp.toPx()))
                rotate(-12f, Offset(cut, mid)) { drawRect(r.bad, Offset(cut - 1.5.dp.toPx(), mid - 27.dp.toPx()), Size(3.dp.toPx(), 54.dp.toPx())) }
            }
        }
        Title("Cut the link to", "${l.name}?", size = 28)
        T("It stops seeing this phone and forgets its saved file names. To undo, do the handshake again.", 15, color = ink(2))
    }
}

/** A dot-matrix count with its engraved label, right-aligned in a header. */
@Composable fun Counter(value: String, label: String) {
    Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.semantics(mergeDescendants = true) { contentDescription = "$value $label" }) {
        Dm(value, 26); Lbl(label)
    }
}

/** Every paired laptop as a station; pick the one EchoConnect works with, or tune in to another. */
@Composable fun StationsSheet(s: UiState, host: Host, onDone: () -> Unit) {
    val r = relay()
    val view = LocalView.current
    val active = s.laptop?.id
    val paired = s.laptops.filter { it.paired }
    Sheet(onDone, footer = {
        Key("Tune in to another laptop", Modifier.fillMaxWidth(), Tone.Carbon, icon = "plus", height = 52.dp) { onDone(); host.go("pair") }
    }) {
        Row(Modifier.padding(horizontal = 4.dp), verticalAlignment = Alignment.Bottom) {
            Title("Stations", size = 26, modifier = Modifier.weight(1f)); Lbl("${paired.size} paired")
        }
        paired.forEach { l ->
            val on = l.id == active
            Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(r.carbon2)
                .then(if (on) Modifier.border(2.dp, r.sig, RoundedCornerShape(18.dp)) else Modifier)
                .clickable(role = androidx.compose.ui.semantics.Role.RadioButton) {
                    when {
                        on -> onDone()
                        l.connected -> { Haptics.tick(view); Connect.choose(l.id); host.snack(Snack("Working with ${l.name}", "success")); onDone() }
                        else -> host.snack(Snack("${l.name} isn't nearby", "warning"))
                    }
                }
                .semantics { selected = on }.padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Led(if (l.connected) LedState.Live else LedState.Off, 10.dp)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    T(l.name, 18, 600, if (l.connected) r.chalk else r.chalk2, maxLines = 1)
                    Lbl(when {
                        !l.connected -> "Not nearby"
                        l.lan && l.bluetooth -> "Wi‑Fi + BT" + if (l.ip.isNotBlank()) " · ${l.ip}" else ""
                        l.lan -> "Wi‑Fi" + if (l.ip.isNotBlank()) " · ${l.ip}" else ""
                        else -> "Bluetooth"
                    })
                }
                Dm(if (on && l.connected) "LIVE" else if (l.connected) "READY" else "AWAY", 20, if (on && l.connected) r.sig else r.chalk3)
            }
        }
    }
}
