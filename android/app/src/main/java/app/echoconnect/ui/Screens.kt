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
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.material3.Text
import app.echoconnect.*
import app.echoconnect.clipboard.ClipboardSync
import app.echoconnect.share.Share

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
    Feature("notifications", "bell", "Notifications", "Shown on the laptop; reply from there"),
    Feature("sms", "message", "Messages", "Read and send texts from the laptop"),
    Feature("contacts", "users", "Contact names", "Names instead of numbers in texts and calls"),
    Feature("calls", "call", "Calls on the laptop", "Answer on laptop connects Bluetooth audio"),
    Feature("files", "folder", "Files", "Send files both ways; browse the phone from EchoFiles"),
    Feature("photos", "image", "Photos", "Your camera roll in EchoFiles"),
    Feature("clipboard", "clipboard", "Clipboard", "Copy on one, paste on the other"),
    Feature("battery", "battery", "Battery", "This phone's charge on the laptop"),
    Feature("ring", "phone-ring", "Ring this phone", "Find it from the laptop, even on silent"),
    Feature("capture", "camera", "Camera and scanning", "Take a photo or scan into a laptop folder"),
    Feature("media", "music", "Media controls", "Play, pause and skip from the laptop"),
    Feature("signal", "network", "Mobile signal", "Signal and network type on the laptop"),
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
    else -> Unit
}

class Perm(val icon: String, val title: String, val why: String, val granted: Boolean, val action: String = "Allow", val optional: Boolean = false, val request: (Host) -> Unit)

fun batteryUnrestricted(ctx: Context) = ctx.getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(ctx.packageName)

/** Every permission EchoConnect can use, named as Android names it, with what it unlocks. */
fun permissions(ctx: Context): List<Perm> {
    val photosFull = app.echoconnect.photos.Photos.fullAccess(ctx)
    return listOfNotNull(
        Perm("bell", "Notification access", "Shows your notifications on the laptop", featureReady(ctx, "notifications"), "Open setting") { allowFeature(it, "notifications") },
        Perm("message", "SMS", "Read and send texts from the laptop", featureReady(ctx, "sms")) { allowFeature(it, "sms") },
        Perm("users", "Contacts", "Names instead of numbers in texts and calls", featureReady(ctx, "contacts")) { allowFeature(it, "contacts") },
        Perm("call", "Phone", "Caller name on the laptop; answer from there", granted(ctx, Manifest.permission.READ_PHONE_STATE, Manifest.permission.ANSWER_PHONE_CALLS)) { allowFeature(it, "calls") },
        if (Build.VERSION.SDK_INT >= 31) Perm("bluetooth", "Nearby devices", "Bluetooth for calls and clipboard", granted(ctx, Manifest.permission.BLUETOOTH_CONNECT)) { it.request(Manifest.permission.BLUETOOTH_CONNECT, Manifest.permission.BLUETOOTH_SCAN) } else null,
        Perm("folder", "All files access", "Browse the phone from EchoFiles", featureReady(ctx, "files"), "Open setting") { allowFeature(it, "files") },
        Perm("image", "Photos and videos", if (photosFull || !featureReady(ctx, "photos")) "Your camera roll in EchoFiles" else "Only allowed photos and videos", photosFull, if (featureReady(ctx, "photos")) "Allow all" else "Allow") { it.requestPhotos() },
        Perm("camera", "Camera", "Scan the laptop's code; take photos for it", featureReady(ctx, "capture"), optional = true) { allowFeature(it, "capture") },
        Perm("clipboard", "Display over other apps", "Lets automatic clipboard read what you copy", Settings.canDrawOverlays(ctx), "Open setting", true) { it.settings(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, true) },
        Perm("moon", "Do Not Disturb access", "Ring through silent; match the laptop's Do Not Disturb", featureReady(ctx, "dnd"), "Open setting", true) { allowFeature(it, "dnd") },
        if (Build.VERSION.SDK_INT >= 34) Perm("phone-ring", "Full-screen notifications", "Shows Ringing over the lock screen", ctx.getSystemService(NotificationManager::class.java).canUseFullScreenIntent(), "Open setting", true) { it.settings(Settings.ACTION_MANAGE_APP_USE_FULL_SCREEN_INTENT, true) } else null,
        Perm("battery", "Battery: Unrestricted", "Stays connected with the screen off", batteryUnrestricted(ctx), "Open setting") { requestUnrestricted(it) },
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
        ClipboardSync.Sent.Sent -> host.snack(Snack("Sent to $name", "success", "check"))
        ClipboardSync.Sent.Sending -> host.snack(Snack("Sending the image to $name", icon = "upload"))
        ClipboardSync.Sent.Empty -> host.snack(Snack("The clipboard is empty. Copy something first.", icon = "clipboard"))
        ClipboardSync.Sent.Off -> host.snack(Snack("Clipboard sharing is off", action = "Turn on") { Prefs.set(host.context, "clipboard", true); host.refresh() })
        ClipboardSync.Sent.Unchanged -> host.snack(Snack("$name already has it", icon = "check"))
        ClipboardSync.Sent.Failed -> host.snack(Snack("Couldn't reach $name", "danger", "error", "Retry") { sendClipboard(host) })
    }
}

// ------------------------------------------------------------------ frame

/** The scrolling body under an app bar: 16dp gutter, 16dp between blocks. */
@Composable fun ScreenBody(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(16.dp), content = content)
}

// ------------------------------------------------------------------ Home

class ActionItem(val icon: String, val label: String, val sub: String, val enabled: Boolean, val onClick: () -> Unit)

/** Four quick actions, two by two; tiles act on the laptop, so their glyphs are `world-linux`. */
@Composable fun ActionGrid(items: List<ActionItem>) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        items.chunked(2).forEach { row ->
            Row(Modifier.height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { a ->
                    Column(Modifier.weight(1f).fillMaxHeight().disabled(!a.enabled).clip(RoundedCornerShape(4.dp)).background(token("bg-raised")).border(1.dp, token("line"), RoundedCornerShape(4.dp))
                        .tap(a.enabled, label = a.label, radius = 4.dp, onClick = a.onClick).padding(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Box(Modifier.padding(bottom = 6.dp).size(36.dp).background(token("state-hover"), RoundedCornerShape(4.dp)), contentAlignment = Alignment.Center) { Glyph(a.icon, color = token("world-linux")) }
                        Txt(a.label, size = 14, line = 19, bold = true, color = token("ink-strong"))
                        Txt(a.sub, muted = true, size = 12, line = 16)
                    }
                }
            }
        }
    }
}

/** The top of Home: the laptop drawing, its state, battery, name and how it's reached. */
@Composable fun LaptopCard(l: Laptop?, onPair: () -> Unit) {
    val away = l?.connected != true
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(4.dp)).background(token("bg-raised")).border(1.dp, token("line"), RoundedCornerShape(4.dp))) {
        Box(Modifier.fillMaxWidth().background(token("bg-sunken")).hairline(bottom = true).padding(start = 12.dp, end = 12.dp, top = 16.dp, bottom = 12.dp), contentAlignment = Alignment.Center) {
            LaptopDevice(if (away) LaptopLook.Away else LaptopLook.Connected, 236.dp, l?.name ?: "Your laptop")
        }
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                when { l == null -> StatePill("Not paired", "warning"); away -> StatePill("Not nearby"); else -> StatePill("Connected", "success") }
                Spacer(Modifier.weight(1f))
                if (!away && l?.battery != null) BatteryMeter(l.battery, l.charging)
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                WorldMark()
                Text(l?.name ?: "Your laptop", color = token(if (away) "ink-muted" else "ink-strong"), fontFamily = Mono, fontSize = 20.sp, lineHeight = 26.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.2).sp)
            }
            when {
                l == null -> {
                    Txt("Pair with EchoFiles to share files, clipboard, texts, notifications and calls.", muted = true, size = 13, line = 19)
                    Button("Pair laptop", Variant.Primary, icon = "qr", block = true, onClick = onPair)
                }
                away -> Txt("Open the laptop on the same Wi-Fi. Calls and clipboard also work over Bluetooth when it's in range.", muted = true, size = 13, line = 19)
                else -> LinkPills(l.lan, l.bluetooth)
            }
        }
    }
}

@Composable fun HomeScreen(s: UiState, host: Host, openLink: () -> Unit) {
    val ctx = host.context
    host.revision
    val l = s.laptop
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(s.call != null) { while (s.call != null) { now = System.currentTimeMillis(); kotlinx.coroutines.delay(1000) } }
    Column {
        AppBar("EchoConnect", actions = listOf(BarAction("qr", "Pair another laptop") { host.go("pair") }))
        ScreenBody {
            s.error?.let { Banner("Needs attention.", it, "danger") { Button("Reconnect", size = BtnSize.Sm) { Connect.start(); Connect.announce() } } }
            LaptopCard(l) { host.go("pair") }
            s.call?.let { c ->
                val secs = ((now - c.since) / 1000).coerceAtLeast(0)
                CallBar(c.who.ifBlank { c.number.ifBlank { "Call" } }, "%02d:%02d".format(secs / 60, secs % 60), c.onLaptop) { app.echoconnect.calls.Calls.usePhone(ctx) }
            }
            val lan = l?.lan == true
            ActionGrid(listOf(
                ActionItem("upload", "Send files", "Photos, PDFs, anything", lan && Prefs.on(ctx, "files")) { host.pickFiles() },
                ActionItem("clipboard", "Send clipboard", "What you copied last", l?.connected == true && Prefs.on(ctx, "clipboard")) { sendClipboard(host) },
                ActionItem("external", "Open on laptop", "A link, in its browser", lan, openLink),
                ActionItem("lock", "Lock laptop", "Locks the screen now", lan) {
                    if (Connect.send("echofiles.lock")) host.snack(Snack("Locked ${l?.name}", icon = "lock")) else host.snack(Snack("Couldn't reach ${l?.name}", "danger", "error"))
                },
            ))
            ListGroup("Clipboard", "See all", { host.go("clipboard") }) {
                if (s.clips.isEmpty()) ListRow("Nothing copied yet", "Your next copy appears here", icon = "clipboard")
                s.clips.take(2).forEach { c -> ClipItem(c, copyAction(host, c)) }
            }
            ListGroup("Transfers", "See all", { host.go("transfers") }) {
                if (s.transfers.isEmpty()) ListRow("Nothing sent yet", "Files you send and receive appear here", icon = "swap")
                s.transfers.take(2).forEach { TransferRow(it, host) }
            }
        }
    }
}

fun copyAction(host: Host, c: Clip): (() -> Unit)? =
    if (c.sensitive || c.image || c.text.isEmpty()) null else ({ ClipboardSync.copyAgain(host.context, c.text); host.snack(Snack("Copied", icon = "check")) })

/** Send a web address to the laptop's browser. */
@Composable fun OpenLinkSheet(s: UiState, host: Host, onDone: () -> Unit) {
    val ctx = host.context
    var link by rememberSaveable {
        val clip = runCatching { ctx.getSystemService(ClipboardManager::class.java).primaryClip?.getItemAt(0)?.coerceToText(ctx)?.toString()?.trim() }.getOrNull().orEmpty()
        mutableStateOf(if (clip.startsWith("https://") || clip.startsWith("http://")) clip else "")
    }
    val valid = Uri.parse(link.trim()).let { it.scheme in listOf("http", "https") && !it.host.isNullOrBlank() }
    val name = s.laptop?.name ?: "laptop"
    BottomSheet("Open on $name", "external", onDone, footer = {
        Button("Cancel", Variant.Ghost, onClick = onDone)
        Button("Open on laptop", Variant.Primary, icon = "external", enabled = valid && s.laptop?.lan == true) {
            if (Connect.send("kdeconnect.share.request", org.json.JSONObject().put("url", link.trim()))) { host.snack(Snack("Opened on $name", "success", "check")); onDone() }
            else host.snack(Snack("Couldn't reach $name", "danger", "error"))
        }
    }) {
        Column(Modifier.padding(horizontal = 10.dp, vertical = 4.dp)) {
            TextField(link, "Web address", "https://", hint = "Opens in the laptop's browser.", error = if (link.isNotBlank() && !valid) "Start with https:// or http://" else null, icon = "link",
                keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri)) { link = it }
        }
    }
}

// ------------------------------------------------------------------ Clipboard

@Composable fun ClipboardScreen(s: UiState, host: Host) {
    val ctx = host.context
    host.revision
    var filter by rememberSaveable { mutableStateOf("all") }
    val on = Prefs.on(ctx, "clipboard")
    val auto = Prefs.clipMode(ctx) == "auto"
    val connected = s.laptop?.connected == true
    val mode = if (!auto) ClipMode.Manual else if (ClipboardSync.automaticActive) ClipMode.Auto else ClipMode.Paused
    Column {
        AppBar("Clipboard", actions = if (s.clips.isEmpty()) emptyList() else listOf(BarAction("trash", "Clear history") { Connect.clearClips(); host.snack(Snack("History cleared", icon = "check")) }))
        ScreenBody {
            if (!on) Banner("Clipboard is off.", "Nothing you copy crosses to the laptop, and the laptop's copies stay there.", "warning") {
                Button("Turn on clipboard", size = BtnSize.Sm) { Prefs.set(ctx, "clipboard", true); host.refresh() }
            }
            val ready = ClipboardSync.ready(ctx)
            ClipModeCard(mode,
                when (mode) {
                    ClipMode.Auto -> "Copy anything on the phone and it's on the laptop."
                    ClipMode.Manual -> "Send with Send to laptop in the text menu, the quick-settings tile, Share, or the button below."
                    ClipMode.Paused -> when {
                        !connected -> "Paused while the laptop is away. It picks up again when they reconnect."
                        !ready -> "The phone restarted or a permission changed. Allow log access once to switch automatic sending back on."
                        else -> "Paused while the screen was off. Resume to start again."
                    }
                },
                when { mode == ClipMode.Manual -> "Make it automatic"; mode == ClipMode.Paused && connected -> "Resume automatic"; else -> null }) {
                if (mode == ClipMode.Manual || !ready) host.go("clip-setup") else { ClipboardSync.resume(ctx); host.refresh() }
            }
            Button("Send clipboard now", if (mode == ClipMode.Manual) Variant.Primary else Variant.Default, BtnSize.Lg, "send", block = true, enabled = connected && on) { sendClipboard(host) }
            Segmented(listOf("all" to "All", "to" to "To laptop", "from" to "From laptop"), filter, "Show") { filter = it }
            val clips = s.clips.filter { System.currentTimeMillis() - it.at < 86_400_000 && (filter == "all" || it.toLaptop == (filter == "to")) }
            ListGroup("Last 24 hours", foot = "History stays on this phone for 24 hours. Passwords from password managers are hidden and never kept.") {
                if (clips.isEmpty()) EmptyState(EmptyArt.Clipboard, if (s.clips.isEmpty()) "Nothing copied yet" else "Nothing this way yet", "Copy on either device and it appears here.")
                clips.forEach { c -> ClipItem(c, copyAction(host, c)) }
            }
        }
    }
}

// ------------------------------------------------------------------ Transfers

@Composable fun TransferRow(t: Transfer, host: Host) {
    val ctx = host.context
    val waiting = Share.waiting().containsKey(t.id)
    val moving = !t.finished && t.error == null
    val quiet = t.error in setOf("Cancelled", "Declined")
    val failed = t.error != null && !waiting
    val dir = if (t.upload) "to laptop" else "from laptop"
    val sub = when {
        waiting -> "${bytes(t.size)} · $dir · waiting for you"
        failed && quiet -> "${t.error} · ${bytes(t.size)} · $dir"
        failed -> "Stopped · ${t.error}"
        moving -> {
            val secs = ((System.currentTimeMillis() - t.at) / 1000).coerceAtLeast(1)
            val rate = t.done / secs
            val left = if (rate > 0 && t.size > t.done) " · ${(t.size - t.done) / rate + 1} s left" else ""
            "${bytes(t.done)} of ${bytes(t.size)} · ${bytes(rate)}/s$left"
        }
        else -> "${bytes(t.size)} · $dir · ${relativeTime(t.at)}"
    }
    ListRow(t.name, sub, lead = { FileIcon(t.name) }, subColor = if (failed && !quiet) token("danger-ink") else null,
        below = when {
            moving && !waiting -> ({ ProgressBar(if (t.size > 0) t.done.toFloat() / t.size else null) })
            waiting -> ({
                Row(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Button("Receive", Variant.Primary, BtnSize.Sm) { Share.receive(ctx, t.id); host.refresh() }
                    Button("Decline", size = BtnSize.Sm) { Share.decline(t.id); host.refresh() }
                }
            })
            else -> null
        },
        trailing = when {
            moving && !waiting -> ({ IconButton("close", "Stop", 18) { Connect.cancelTransfer(t.id) } })
            failed && !quiet && Connect.canRetry(t.id) -> ({ Button("Retry", size = BtnSize.Sm) { Connect.retryTransfer(t.id) } })
            t.finished && t.error == null && t.uri != null -> ({ IconButton("external", "Open", 18) { openUri(ctx, t.uri, host) } })
            t.finished && t.error == null -> ({ Glyph("check", "Done", token("success-ink"), 18) })
            else -> null
        })
}

fun openUri(ctx: Context, uri: String, host: Host) {
    val u = Uri.parse(uri)
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW).setDataAndType(u, ctx.contentResolver.getType(u) ?: "*/*").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)) }
        .onFailure { host.snack(Snack("No app on this phone opens this file", icon = "info")) }
}

fun dayLabel(at: Long): String = when {
    android.text.format.DateUtils.isToday(at) -> "Today"
    android.text.format.DateUtils.isToday(at + 86_400_000) -> "Yesterday"
    else -> java.text.SimpleDateFormat("EEEE d MMM", java.util.Locale.getDefault()).format(java.util.Date(at))
}

@Composable fun TransfersScreen(s: UiState, host: Host) {
    host.revision
    val foot = "Files from the laptop save to Download/EchoConnect. Files you send land in ~/Downloads/Phone on the laptop, unless EchoFiles is set to another folder."
    val now = s.transfers.filter { (!it.finished && it.error == null) || Share.waiting().containsKey(it.id) }
    val done = s.transfers - now.toSet()
    Column {
        AppBar("Transfers", actions = listOf(BarAction("upload", "Send files", s.laptop?.lan == true && Prefs.on(host.context, "files")) { host.pickFiles() }))
        ScreenBody {
            if (s.transfers.isEmpty()) ListGroup(foot = foot) { EmptyState(EmptyArt.Transfers, "Nothing sent yet", "Send from Home, Share → EchoConnect, or EchoFiles on the laptop.") }
            if (now.isNotEmpty()) ListGroup("Now", foot = if (done.isEmpty()) foot else null) { now.forEach { TransferRow(it, host) } }
            val days = done.groupBy { dayLabel(it.at) }.toList()
            days.forEachIndexed { i, (day, items) -> ListGroup(day, foot = if (i == days.lastIndex) foot else null) { items.forEach { TransferRow(it, host) } } }
        }
    }
}

// ------------------------------------------------------------------ Settings

@Composable fun SettingsScreen(s: UiState, host: Host, forget: () -> Unit) {
    val ctx = host.context
    host.revision
    val l = s.laptop
    Column {
        AppBar("Settings")
        ScreenBody {
            ListGroup("Laptop") {
                if (l != null) {
                    ListRow(l.name, if (l.connected) "Connected" + if (l.ip.isNotBlank()) " · ${l.ip}" else "" else "Not nearby", lead = { LaptopDevice(if (l.connected) LaptopLook.Connected else LaptopLook.Away, 52.dp, l.name) })
                    ListRow("Wi-Fi", "Files, photos, texts, notifications", icon = "wifi", trailing = { if (l.lan) StatePill("On", "success") else StatePill("Off") })
                    ListRow("Bluetooth", "Calls and clipboard, even off Wi-Fi", icon = "bluetooth", trailing = { if (l.bluetooth) StatePill("On", "success") else StatePill("Off") })
                }
                ListRow(if (l == null) "Pair laptop" else "Pair another laptop", "Scan the code in EchoFiles → Connect phone", icon = "qr", chevron = true) { host.go("pair") }
                if (l != null) ListRow("Forget this laptop", "Pair again with its code to undo", icon = "unlink", chevron = true, onClick = forget)
            }
            ListGroup("Shared with the laptop", foot = "Each change saves at once and shows in EchoFiles → Settings → Phone. A feature you turn off stops on both.") {
                FEATURES.forEach { f ->
                    val on = Prefs.on(ctx, f.key, f.default)
                    val ready = featureReady(ctx, f.key)
                    val sub = when {
                        !on -> f.what
                        !ready -> "Needs permission · tap to allow"
                        f.key == "photos" && !app.echoconnect.photos.Photos.fullAccess(ctx) -> "Only allowed photos and videos · tap to change"
                        f.key == "clipboard" -> if (Prefs.clipMode(ctx) == "auto") "Automatic" + if (ClipboardSync.automaticActive) "" else " · paused" else "Tap to send"
                        else -> f.what
                    }
                    ListRow(f.title, sub, icon = f.icon, ok = on && ready, subColor = if (on && !ready) token("warning-ink") else null,
                        onClick = when {
                            on && !ready -> ({ allowFeature(host, f.key) })
                            on && f.key == "photos" -> ({ host.requestPhotos() })
                            f.key == "clipboard" -> ({ host.go("clipboard") })
                            f.key == "notifications" && on -> ({ host.go("notify-apps") })
                            else -> null
                        },
                        trailing = { Switch(on, f.title) { v ->
                            Prefs.set(ctx, f.key, v)
                            Connect.send("echofiles.settings", org.json.JSONObject().put(f.key, v))
                            if (v && !featureReady(ctx, f.key)) allowFeature(host, f.key)
                            host.refresh()
                        } })
                }
            }
            ListGroup("Receiving") {
                ListRow("Save files to", "Download/EchoConnect", icon = "download")
                ListRow("Accept files automatically", if (Prefs.autoAccept(ctx)) "From your paired laptop" else "Off asks before each file", icon = "check",
                    trailing = { Switch(Prefs.autoAccept(ctx), "Accept files automatically") { Prefs.setAutoAccept(ctx, it); host.refresh() } })
                val backup = Prefs.on(ctx, "backup", false)
                ListRow("Camera-roll backup", if (backup && !featureReady(ctx, "backup")) "Paused · allow all photos and videos" else "To ~/Pictures/Phone/Backup on Wi-Fi while charging", icon = "cloud-upload",
                    subColor = if (backup && !featureReady(ctx, "backup")) token("warning-ink") else null, onClick = if (backup && !featureReady(ctx, "backup")) ({ host.requestPhotos() }) else null,
                    trailing = { Switch(backup, "Camera-roll backup") { v -> Prefs.set(ctx, "backup", v); Connect.send("echofiles.settings", org.json.JSONObject().put("backup", v)); if (v && !featureReady(ctx, "backup")) host.requestPhotos(); host.refresh() } })
            }
            ListGroup("With the laptop") {
                val dnd = Prefs.on(ctx, "dnd", false)
                ListRow("Do Not Disturb sync", if (dnd && !featureReady(ctx, "dnd")) "Needs Do Not Disturb access · tap to allow" else "Quiet on one, quiet on both", icon = "moon",
                    subColor = if (dnd && !featureReady(ctx, "dnd")) token("warning-ink") else null, onClick = if (dnd && !featureReady(ctx, "dnd")) ({ allowFeature(host, "dnd") }) else null,
                    trailing = { Switch(dnd, "Do Not Disturb sync") { v -> Prefs.set(ctx, "dnd", v); Connect.send("echofiles.settings", org.json.JSONObject().put("dnd", v)); if (v && !featureReady(ctx, "dnd")) allowFeature(host, "dnd"); host.refresh() } })
                val lock = Prefs.on(ctx, "autolock", false)
                ListRow("Lock laptop when away", "Locks it when this phone disconnects", icon = "lock",
                    trailing = { Switch(lock, "Lock laptop when away") { v -> Prefs.set(ctx, "autolock", v); Connect.send("echofiles.settings", org.json.JSONObject().put("autolock", v)); host.refresh() } })
            }
            ListGroup("Look") {
                val mode = if (Prefs.themeMode(ctx) == "laptop") "laptop" else "phone"
                ListRow("Theme", if (mode == "phone") "Echo in dark mode, Catppuccin Latte in light" else if (s.theme == null) "Uses the laptop's Omarchy theme once it connects" else "Uses the laptop's Omarchy theme", icon = "sliders",
                    below = { Box(Modifier.padding(top = 6.dp)) { Segmented(listOf("laptop" to "Match laptop", "phone" to "Phone"), mode, "Theme") { Prefs.setThemeMode(ctx, it); host.refresh() } } })
            }
            ListGroup("This phone") {
                val perms = permissions(ctx)
                ListRow("Permissions", "${perms.count { it.granted }} of ${perms.size} allowed", icon = "shield", chevron = true) { host.go("permissions") }
                val unrestricted = batteryUnrestricted(ctx)
                ListRow("Stay connected", if (samsung) "Battery: Unrestricted, and Never sleeping apps" else "Battery: Unrestricted", icon = "battery", ok = unrestricted,
                    trailing = { if (unrestricted) StatePill("Set", "success") else Button("Allow", Variant.Primary, BtnSize.Sm) { requestUnrestricted(host) } })
                ListRow("Automatic clipboard", "One-time setup with EchoFiles", icon = "bolt", chevron = true) { host.go("clip-setup") }
            }
            ListGroup("About", foot = "EchoConnect and EchoFiles are open source (GPL-3.0-or-later). Read every line at $SOURCE.") {
                ListRow("EchoConnect ${BuildConfig.VERSION_NAME}", "Android ${Build.VERSION.RELEASE} · ${Build.MODEL}", icon = "info")
                ListRow("Source code", SOURCE, icon = "code", trailing = { Glyph("external", size = 18) }) { host.open("https://$SOURCE") }
            }
        }
    }
}

@Composable fun NotificationAppsScreen(host: Host, back: () -> Unit) {
    val ctx = host.context
    host.revision
    val pm = ctx.packageManager
    val apps = remember(host.revision) {
        app.echoconnect.notifications.Notifications.listener?.activeNotifications.orEmpty().map { it.packageName }.distinct().filter { it != ctx.packageName }
            .map { pkg -> pkg to runCatching { pm.getApplicationLabel(pm.getApplicationInfo(pkg, 0)).toString() }.getOrDefault(pkg) }.sortedBy { it.second.lowercase() }
    }
    val muted = Prefs.mutedApps(ctx)
    Column {
        AppBar("Notification apps", back)
        ScreenBody {
            Lede("Turn off apps whose notifications should stay on this phone. Apps appear here while they have a notification showing.")
            if (!featureReady(ctx, "notifications")) Banner("Notification access is off.", "Allow it so the laptop can show and answer your notifications.", "warning") {
                Button("Open setting", size = BtnSize.Sm) { allowFeature(host, "notifications") }
            }
            ListGroup("Shown on the laptop") {
                if (apps.isEmpty()) ListRow("No notifications right now", "Apps appear here when they have one", icon = "bell")
                apps.forEach { (pkg, name) ->
                    ListRow(name, if (pkg in muted) "Stays on this phone" else "Shown on the laptop", icon = if (pkg in muted) "bell-off" else "bell",
                        trailing = { Switch(pkg !in muted, name) { on -> Prefs.setMutedApps(ctx, muted.toMutableSet().apply { if (on) remove(pkg) else add(pkg) }); host.refresh() } })
                }
            }
        }
    }
}

// ------------------------------------------------------------------ Permissions

@Composable fun PermissionsScreen(host: Host, back: () -> Unit) {
    val ctx = host.context
    host.revision
    val perms = permissions(ctx)
    Column {
        AppBar("Permissions", back)
        ScreenBody {
            Lede("Each permission unlocks one feature. Turn a feature off in Settings and EchoConnect stops using its permission.")
            ListGroup("Allowed · ${perms.count { it.granted }} of ${perms.size}") { perms.forEach { p -> PermissionRow(p.icon, p.title, p.why, p.granted, p.action, p.optional) { p.request(host) } } }
            if (samsung) Banner("On Samsung.", "Settings → Battery → Background usage limits → Never sleeping apps → add EchoConnect. Otherwise One UI may disconnect it overnight.", icon = "battery")
            else Banner("Battery.", "Settings → Apps → EchoConnect → Battery → Unrestricted keeps the connection while the screen is off.", icon = "battery")
        }
    }
}

// ------------------------------------------------------------------ Automatic clipboard setup

@Composable fun ClipboardSetupScreen(s: UiState, host: Host, back: () -> Unit) {
    val ctx = host.context
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
    Column {
        AppBar("Automatic clipboard", back)
        ScreenBody {
            Lede("About 2 minutes, once. Afterwards you copy on the phone and paste on the laptop — no taps. EchoConnect checks each step itself.")
            StepList(listOf(
                Step("Turn on Developer options", "On") {
                    Lede(strong(if (samsung) "Settings → About phone → Software information → tap **Build number** 7 times. Enter your PIN when asked." else "Settings → About phone → tap **Build number** 7 times. Enter your PIN when asked."))
                    Button(if (samsung) "Open Software information" else "Open About phone", icon = "external", block = true) { host.settings(Settings.ACTION_DEVICE_INFO_SETTINGS) }
                },
                Step("Turn on Wireless debugging", "On") {
                    Lede(strong("Settings → Developer options → **Wireless debugging** → Allow on this Wi-Fi. Then tap **Pair device with QR code**."))
                    Button("Open Developer options", icon = "external", block = true) { host.settings(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS) }
                },
                Step("Scan the code on the laptop", "Permission granted") {
                    Lede(strong("On the laptop: EchoFiles → Settings → Phone → **Make clipboard automatic**. Scan its code from the Wireless debugging screen."))
                    Wait("Waiting for $laptop…")
                },
                Step("Allow Display over other apps", "Allowed") {
                    Lede("A normal setting. It lets EchoConnect open an invisible window for a moment to read what you copied.")
                    Button("Open the setting", Variant.Primary, icon = "external", block = true) { host.settings(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, true) }
                },
                Step("Turn Developer options off", "Off · banking apps work normally") {
                    Lede("The permission stays. Banking and payment apps only check whether Developer options are on now.")
                    Button("Open Developer options", Variant.Primary, icon = "external", block = true) { host.settings(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS) }
                    Wait("EchoConnect checks again on its own.") { StatePill("Still on", "warning") }
                },
            ), current)
            if (current >= 5) {
                if (auto) Banner("Automatic is on.", "Copy anything on the phone and it's on the laptop.", "success", "check")
                else Button("Turn on automatic clipboard", Variant.Primary, BtnSize.Lg, "bolt", block = true) {
                    Prefs.setClipMode(ctx, "auto"); Prefs.set(ctx, "clipboard", true); ClipboardSync.stopReader(); ClipboardSync.resume(ctx)
                    host.snack(Snack("Automatic clipboard is on", "success", "check")); host.go("clipboard")
                }
            }
            Banner("What EchoConnect reads.", "Only the one line Android writes when a clipboard read is blocked. It never stores or sends anything else from the log. EchoConnect is open source — check the code at $SOURCE.", icon = "shield")
            Banner("After a restart.", "Android asks once: Allow EchoConnect to access all device logs? Tap Allow one-time access and it works until the next restart.", icon = "refresh")
            if (auto) Button("Switch to Tap to send", Variant.Ghost, block = true) { Prefs.setClipMode(ctx, "manual"); ClipboardSync.stopReader(); host.go("clipboard") }
            else Button("Use Tap to send instead", Variant.Ghost, block = true) { Prefs.setClipMode(ctx, "manual"); host.go("clipboard") }
        }
    }
}

// ------------------------------------------------------------------ First run and pairing

val PAIR_STEPS = listOf("Welcome", "Scan", "Check code", "Permissions", "Clipboard")

/** Welcome → scan the laptop's code → check the code → permissions → clipboard choice → connected. */
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
    val goBack = { if (step == 2) Connect.rejectPair(); if (!firstRun && step == 1) done(false) else step-- }
    androidx.activity.compose.BackHandler(step in 1..4) { goBack() }
    val name = paired?.name ?: s.pair?.name ?: "your laptop"
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 16.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (step in 1..4) IconButton("arrow-left", "Back") { goBack() }
            else Spacer(Modifier.size(48.dp))
            Stepper(step, 5, PAIR_STEPS.getOrElse(step) { "Done" }, Modifier.weight(1f))
        }
        ScreenBody(Modifier.weight(1f)) {
            when (step) {
                0 -> {
                    PairHero(connected = false, modifier = Modifier.padding(top = 16.dp, bottom = 8.dp))
                    Display("Your phone, on your laptop")
                    Lede("Files, photos, clipboard, texts, notifications and calls between this phone and EchoFiles — over your Wi-Fi, with Bluetooth for calls.")
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) { listOf("Nothing leaves your network", "Encrypted, paired once", "Open source").forEach { Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { Glyph("check", color = token("success-ink"), size = 16); Txt(it, size = 14) } } }
                }
                1 -> {
                    Lede(strong("On the laptop, open EchoFiles and click **Connect phone** in the sidebar. A code appears. Keep both on the same Wi-Fi."))
                    val found = s.laptops.filter { !it.paired && it.connected }
                    ListGroup("On this Wi-Fi") {
                        if (found.isEmpty()) ListRow("Looking for laptops…", "EchoFiles appears here when it's open", lead = { Box(Modifier.size(36.dp), contentAlignment = Alignment.Center) { Spinner(18) } },
                            trailing = { Button("Search again", Variant.Ghost, BtnSize.Sm) { Connect.announce() } })
                        found.forEach { lap -> ListRow(lap.name, if (lap.ip.isNotBlank()) lap.ip else "EchoFiles", icon = "laptop", trailing = { Button("Pair", Variant.Primary, BtnSize.Sm) { Connect.pair(lap.id) } }) }
                    }
                    if (typing) {
                        TextField(address, "Laptop address", "192.168.1.20", hint = "Shown in EchoFiles → Connect phone.", icon = "network", keyboard = KeyboardOptions(keyboardType = KeyboardType.Uri)) { address = it }
                        Button("Connect to address", enabled = address.isNotBlank(), block = true) {
                            val (host_, port) = address.trim().split(":").let { it[0] to (it.getOrNull(1)?.toIntOrNull() ?: 1716) }
                            Work.run { if (!Connect.connect(host_, port)) Work.main.post { host.snack(Snack("Couldn't reach $host_. Check the address and that both are on the same Wi-Fi.", "danger", "error")) } }
                        }
                    }
                }
                2 -> {
                    val p = s.pair
                    if (p != null) {
                        Lede("Check ${p.name} shows the same code. Pairing is approved on both at once.")
                        PairCode(p.code)
                        if (!p.theyAsked) Wait("Accept on ${p.name}…")
                    } else if (paired != null) { Wait("Paired with ${paired.name}") { Glyph("check", color = token("success-ink"), size = 16) } }
                    else Wait("Waiting for the laptop…")
                }
                3 -> {
                    Lede("Allow what you want on the laptop. Skip any — you can turn it on later in Settings.")
                    ListGroup { permissions(ctx).take(7).forEach { p -> PermissionRow(p.icon, p.title, p.why, p.granted, p.action, p.optional) { p.request(host) } } }
                }
                4 -> {
                    Lede("How should what you copy on the phone reach the laptop?")
                    ChoiceCard("bolt", "Automatic", "Copy on the phone, paste on the laptop. One-time setup with the laptop, about 2 minutes; Developer options go back off after.", autoClip, { StatePill("Recommended", "accent") }) { autoClip = true }
                    ChoiceCard("send", "Tap to send", "Use Send to laptop in the text menu, the quick-settings tile, or Share. No setup.", !autoClip) { autoClip = false }
                    Note("Laptop → phone is automatic either way.")
                }
                else -> {
                    PairHero(connected = true, modifier = Modifier.padding(top = 16.dp, bottom = 8.dp))
                    Display("Connected")
                    Lede("$name can now see this phone. EchoConnect keeps a small notification while it's connected.")
                }
            }
        }
        Column(Modifier.fillMaxWidth().hairline().padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            when (step) {
                0 -> Button("Scan the laptop's code", Variant.Primary, BtnSize.Lg, "scan", block = true) { step = 1; host.scan() }
                1 -> {
                    Button("Scan the laptop's code", Variant.Primary, BtnSize.Lg, "scan", block = true) { host.scan() }
                    if (!typing) Button("Type the laptop's address instead", Variant.Ghost, block = true) { typing = true }
                }
                2 -> {
                    val p = s.pair
                    if (p?.theyAsked == true) Button("Codes match", Variant.Primary, BtnSize.Lg, block = true) { Connect.acceptPair() }
                    if (p != null) Button(if (p.theyAsked) "They don't match" else "Cancel pairing", Variant.Ghost, block = true) { Connect.rejectPair(); step = 1 }
                    else if (paired != null) Button("Continue", Variant.Primary, BtnSize.Lg, block = true) { step = 3 }
                    else Button("Back to scanning", Variant.Ghost, block = true) { step = 1 }
                }
                3 -> Button("Continue", Variant.Primary, BtnSize.Lg, block = true) { step = 4 }
                4 -> Button(if (autoClip) "Set up Automatic" else "Use Tap to send", Variant.Primary, BtnSize.Lg, block = true) {
                    if (autoClip) { Prefs.setPairedOnce(ctx); done(true) } else { Prefs.setClipMode(ctx, "manual"); step = 5 }
                }
                else -> Button("Done", Variant.Primary, BtnSize.Lg, block = true) { Prefs.setPairedOnce(ctx); done(false) }
            }
        }
    }
}

/** Shown when a laptop asks to pair outside the first-run flow. */
@Composable fun PairDialog(p: PairPrompt) {
    EchoDialog("Pair with ${p.name}?", { Connect.rejectPair() }, body = if (p.theyAsked) "Check both screens show this code." else "Accept on ${p.name}; both screens show this code.", content = { PairCode(p.code) }) {
        Button(if (p.theyAsked) "They don't match" else "Cancel pairing", Variant.Ghost) { Connect.rejectPair() }
        if (p.theyAsked) Button("Codes match", Variant.Primary) { Connect.acceptPair() }
    }
}

