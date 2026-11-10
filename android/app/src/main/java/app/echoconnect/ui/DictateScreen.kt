package app.echoconnect.ui

import android.Manifest
import androidx.compose.animation.core.*
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.echoconnect.*
import app.echoconnect.clipboard.ClipboardSync
import app.echoconnect.scribe.Scribe
import kotlin.math.abs
import kotlin.math.sin

private val MODES = listOf("dictate", "edit", "note")

/** The laptop's word for what it's doing, and the light for it. */
private fun phase(p: String, recording: Boolean): Pair<String, LedState> = when {
    recording && p == "paused" -> "Paused" to LedState.Warn
    recording -> "On air" to LedState.Signal
    p == "transcribing" -> "Hearing it" to LedState.Warn
    p == "cleaning" -> "Polishing" to LedState.Warn
    p == "working" -> "Working" to LedState.Warn
    p == "done" -> "Typed" to LedState.Live
    p == "error" -> "Didn't land" to LedState.Bad
    else -> "Standing by" to LedState.Off
}

/**
 * Dictation on the laptop, in the Relay language: the carbon half is the laptop (where the
 * words land, live, in its type), the seam is your voice crossing over, and the paper half is
 * this phone — one big signal key to hold and talk, the mode, the switches, and remote keys.
 */
@Composable fun DictateScreen(s: UiState, host: Host, back: () -> Unit) {
    val ctx = host.context
    val r = relay()
    val view = LocalView.current
    host.revision
    val d by Scribe.state.collectAsStateWithLifecycle()
    val l = s.laptop
    val connected = l?.connected == true
    LaunchedEffect(connected) { if (connected) Scribe.hello() }
    // The screen stays on while this phone is the microphone.
    DisposableEffect(d.recording) {
        view.keepScreenOn = d.recording
        onDispose { view.keepScreenOn = false }
    }
    // The last few seconds of your voice, for the seam.
    val levels = remember { mutableStateListOf<Float>().apply { repeat(48) { add(0f) } } }
    LaunchedEffect(d.level, d.recording) { if (d.recording) { levels.removeAt(0); levels.add(d.level) } }

    var since by remember { mutableLongStateOf(0L) }
    var wpm by remember { mutableIntStateOf(0) }
    var locked by remember { mutableStateOf(false) }
    var last by remember { mutableStateOf(d.phase) }
    LaunchedEffect(d.phase) {
        if (d.phase == "done" && last != "done") {
            Haptics.confirm(view)
            val secs = ((System.currentTimeMillis() - since) / 1000.0).coerceAtLeast(1.0)
            if (since > 0) wpm = (d.text.split(Regex("\\s+")).count { it.isNotBlank() } * 60 / secs).toInt()
        }
        if (d.phase == "error" && last != "error") Haptics.reject(view)
        if (d.phase !in setOf("listening", "paused")) locked = false
        last = d.phase
    }
    val ready = connected && d.available && Scribe.canRecord(ctx) && Prefs.on(ctx, "dictation")
    val working = d.phase in setOf("transcribing", "cleaning", "working")
    val (status, led) = phase(d.phase, d.recording)

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        // ------------------------------------------------------------ carbon: the laptop
        Head(back) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Led(led, blink = d.recording && d.phase != "paused")
                Lbl(
                    when {
                        !connected -> "No signal"
                        !d.available -> "EchoScribe · not running"
                        d.app.isNotBlank() -> "$status · ${d.app}${if (d.persona.isNotBlank()) " · ${d.persona}" else ""}"
                        else -> "$status · EchoScribe"
                    },
                    ink(2), Modifier.weight(1f),
                )
                if (d.laptopAsked && d.recording) Lbl("${l?.name ?: "Laptop"} asked", r.sig)
            }
            Title(l?.name ?: "No laptop", if (d.recording) "is listening." else "types here.", size = 32)
            // The words, as they land.
            val shown = when {
                d.recording || d.phase == "listening" || d.phase == "paused" -> d.live
                d.phase == "done" -> d.text
                d.phase == "error" -> d.error
                working -> d.live.ifBlank { d.detail }
                else -> ""
            }
            Box(Modifier.fillMaxWidth().heightIn(min = 96.dp).background(r.carbon2, RoundedCornerShape(16.dp)).padding(14.dp)) {
                if (shown.isBlank()) Ser(if (ready) "Say something…" else "—", 26, ink(3))
                else T(shown.takeLast(420), if (shown.length > 160) 16 else 20, 500, if (d.phase == "error") r.badInk else ink(1), line = if (shown.length > 160) 22 else 27)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Readout("Pace", if (wpm > 0) "$wpm" else "--", Modifier.weight(1f), if (wpm > 0) r.live else null)
                Readout("Words", d.text.split(Regex("\\s+")).count { it.isNotBlank() }.takeIf { d.text.isNotBlank() }?.toString() ?: "--", Modifier.weight(1f))
                Readout("Sent", "${d.history.size}", Modifier.weight(1f))
            }
        }
        // ------------------------------------------------------------ the seam: your voice
        VoiceSeam(levels, d.recording && d.phase != "paused", working)
        // ------------------------------------------------------------ paper: this phone
        PaperBody {
            when {
                l == null -> Banner("No laptop yet", "Dictation types on a laptop running EchoFiles and EchoScribe.") { Key("Pair", tone = Tone.Carbon, height = 38.dp, size = 13, radius = 11.dp) { host.go("pair") } }
                !connected -> Banner("Out of range", "Open ${l.name} on the same Wi‑Fi, or keep it in Bluetooth range.")
                !Prefs.on(ctx, "dictation") -> Banner("Dictation is off", "Turn it on to use this phone's mic with EchoScribe.") {
                    Key("Turn on", tone = Tone.Carbon, height = 38.dp, size = 13, radius = 11.dp) { Prefs.set(ctx, "dictation", true); host.refresh(); Scribe.hello() }
                }
                !Scribe.canRecord(ctx) -> Banner("Microphone needed", "EchoConnect only listens while you hold the key, or when the laptop asks and you allow it.") {
                    Key("Allow", tone = Tone.Carbon, height = 38.dp, size = 13, radius = 11.dp) { host.request(Manifest.permission.RECORD_AUDIO) }
                }
                !d.available && d.asked -> Banner("EchoScribe isn't answering", d.error.ifBlank { "Start EchoScribe on ${l.name}; EchoFiles connects them on its own." }, LedState.Off) {
                    Key("Retry", tone = Tone.Carbon, height = 38.dp, size = 13, radius = 11.dp) { Scribe.hello() }
                }
            }
            Lbl("This phone")
            TalkKey(
                recording = d.recording,
                locked = locked,
                enabled = ready || d.recording,
                onPress = {
                    if (!Scribe.state.value.recording) {
                        since = System.currentTimeMillis()
                        if (!Scribe.start(ctx, d.mode)) host.snack(Snack("No link to ${l?.name ?: "the laptop"}", "danger"))
                    }
                },
                // Read the scribe's state directly: `d` only catches up on the next frame.
                onRelease = { held ->
                    val rec = Scribe.state.value.recording
                    when {
                        held < 300 && !locked && rec -> { locked = true; Haptics.tick(view) }
                        locked && held < 300 && rec && System.currentTimeMillis() - since > 600 -> { locked = false; Scribe.stop() }
                        !locked -> Scribe.stop()
                    }
                },
            )
            if (d.recording) Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Key("Scrap it", Modifier.weight(1f), Tone.Cut, icon = "close", height = 46.dp, size = 14) { locked = false; Scribe.cancel() }
                Key(if (d.phase == "paused") "Resume" else "Pause", Modifier.weight(1f), icon = if (d.phase == "paused") "play" else "pause", height = 46.dp, size = 14) { Scribe.command("pause") }
                if (locked) Key("Send", Modifier.weight(1f), Tone.Signal, icon = "send", height = 46.dp, size = 14) { locked = false; Scribe.stop() }
            }
            ThreeWay(listOf("Dictate", "Edit selection", "Note"), MODES.indexOf(d.mode).coerceAtLeast(0), "Mode") { Scribe.setMode(MODES[it]) }

            PlateGroup("On the laptop") {
                PlateRow("Hinglish", "Indic model, Latin script, every app") { Slide(d.hinglish, "Hinglish", enabled = d.available) { Scribe.command("hinglish") } }
                PlateRow("Code mode", when (d.code) { "on" -> "Everywhere: camelCase, symbols, repo names"; "off" -> "Off"; else -> "In editors and terminals" }) {
                    Key(when (d.code) { "on" -> "All"; "off" -> "Off"; else -> "Auto" }, height = 38.dp, size = 13, radius = 11.dp, enabled = d.available) { Scribe.command("code") }
                }
                PlateRow("Voice", d.persona.ifBlank { "Follows the app" }) {
                    Key("Next", trailingIcon = "chevron-right", height = 38.dp, size = 13, radius = 11.dp, enabled = d.available) { Scribe.command("persona") }
                }
            }

            Lbl("Keys")
            val keys = listOf(
                Triple("Enter", "send") { Scribe.command("key") { put("key", "Return") } },
                Triple("Tab", "arrow-right") { Scribe.command("key") { put("key", "Tab") } },
                Triple("Undo", "undo") { Scribe.command("undo") },
                Triple("Raw words", "redo") { Scribe.command("undo_cleanup") },
                Triple("Paste last", "paste") { Scribe.command("paste_last") },
                Triple("Esc", "close") { Scribe.command("key") { put("key", "Escape") } },
            )
            keys.chunked(3).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    row.forEach { (label, icon, run) -> Key(label, Modifier.weight(1f), icon = icon, height = 48.dp, size = 13, enabled = d.available) { run() } }
                }
            }

            if (d.history.isNotEmpty()) PlateGroup("Sent", right = {
                Key("Copy all", height = 32.dp, size = 12, radius = 10.dp) { ClipboardSync.copyAgain(ctx, d.history.reversed().joinToString("\n\n")); host.snack(Snack("Copied", "success")) }
            }) {
                d.history.take(6).forEach { t ->
                    PlateRow(t.take(140)) { Key("Copy", height = 34.dp, size = 12, radius = 10.dp) { ClipboardSync.copyAgain(ctx, t); host.snack(Snack("Copied", "success")) } }
                }
            }
            Note("Hands off", "Add the “Dictate on laptop” tile to Quick Settings, or pick Phone as EchoScribe's microphone and press Right Ctrl on the laptop.")
            Spacer(Modifier.windowInsetsPadding(WindowInsets.navigationBars.only(WindowInsetsSides.Bottom)).height(24.dp))
        }
    }
}

/**
 * The seam as your voice: bars that are the last few seconds of the phone's mic, in signal,
 * crossing to the laptop. Flat when quiet; a slow sweep while the laptop works.
 */
@Composable private fun VoiceSeam(levels: List<Float>, live: Boolean, working: Boolean, height: Dp = 88.dp) {
    val r = relay()
    val t = rememberInfiniteTransition(label = "seam")
    val sweep by t.animateFloat(0f, 1f, infiniteRepeatable(tween(1200, easing = LinearEasing)), label = "sweep")
    Carbon(Modifier.fillMaxWidth().height(height)) {
        Canvas(Modifier.fillMaxSize().clipToBounds().semantics { contentDescription = if (live) "Your voice, going to the laptop" else "Link" }) {
            drawRect(r.carbon3, size = Size(size.width, 1.dp.toPx()))
            val mid = size.height / 2
            val step = 7.dp.toPx(); val w = 4.dp.toPx()
            val n = ((size.width - 32.dp.toPx()) / step).toInt().coerceAtLeast(1)
            val x0 = (size.width - n * step) / 2
            for (i in 0 until n) {
                val lv = levels.getOrElse(levels.size - n + i) { 0f }
                val h = when {
                    live -> (4.dp.toPx() + lv * size.height * 0.82f)
                    working -> 4.dp.toPx() + size.height * 0.3f * abs(sin((i / n.toFloat() - sweep) * 6.28f)).let { if (abs(i / n.toFloat() - sweep) < 0.18f) it else 0f }
                    else -> 2.dp.toPx()
                }
                val c = when {
                    live -> lerp(r.sig, Color.White, (lv * 0.6f).coerceIn(0f, 0.6f))
                    working -> r.warn
                    else -> r.carbon3
                }
                drawRoundRect(c, Offset(x0 + i * step, mid - h / 2), Size(w, h), CornerRadius(w / 2))
            }
        }
    }
}

/**
 * The talk key: Relay's keycap, tall. Hold to talk, let go to send; a quick tap is hands-free.
 * It sinks the whole lip while held and glows signal while the mic is open.
 */
@Composable private fun TalkKey(recording: Boolean, locked: Boolean, enabled: Boolean, onPress: () -> Unit, onRelease: (Long) -> Unit) {
    val r = relay()
    val view = LocalView.current
    var held by remember { mutableStateOf(false) }
    val down by animateDpAsState(if (held || recording) 4.dp else 0.dp, tween(60, easing = LinearEasing), label = "talk")
    val pulse by rememberInfiniteTransition(label = "talk-pulse").animateFloat(0.25f, 0.6f, infiniteRepeatable(tween(900, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "p")
    val face = if (recording) r.sig else r.cap
    val lip = if (recording) lerp(r.sig, Color.Black, 0.45f) else r.edge
    val fg = if (recording) r.onSig else r.ink
    val pressNow by rememberUpdatedState(onPress)
    val releaseNow by rememberUpdatedState(onRelease)
    Box(
        Modifier.fillMaxWidth().padding(bottom = 4.dp).height(132.dp).alpha(if (enabled) 1f else 0.4f)
            .semantics {
                contentDescription = if (recording) "Recording. Double tap to send." else "Talk. Double tap to start, again to send."
                if (enabled) onClick(if (recording) "Send" else "Start talking") { pressNow(); releaseNow(0); true }
            }
            .pointerInput(enabled) {
                if (!enabled) return@pointerInput
                detectTapGestures(onPress = {
                    val t0 = System.currentTimeMillis()
                    held = true; Haptics.key(view); pressNow()
                    tryAwaitRelease()
                    held = false; releaseNow(System.currentTimeMillis() - t0)
                })
            }
            .drawBehind {
                if (recording) drawRoundRect(r.sig.copy(alpha = pulse * 0.35f), Offset(-6.dp.toPx(), -6.dp.toPx() + down.toPx()), Size(size.width + 12.dp.toPx(), size.height + 12.dp.toPx()), CornerRadius(28.dp.toPx()))
                drawRoundRect(lip, Offset(0f, 4.dp.toPx()), size, CornerRadius(22.dp.toPx()))
            }
            .offset(y = down)
            .background(face, RoundedCornerShape(22.dp))
            .padding(horizontal = 20.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Glyph("mic", color = fg, size = 34, stroke = 2.2f)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                T(when { recording && locked -> "Hands-free"; recording -> "Talking…"; else -> "Hold to talk" }, 24, 700, fg)
                T(when { recording && locked -> "Tap to send"; recording -> "Let go to send"; else -> "Tap once for hands-free" }, 14, color = fg.copy(alpha = 0.75f))
            }
            if (recording) Lbl("Live", fg)
        }
    }
}
