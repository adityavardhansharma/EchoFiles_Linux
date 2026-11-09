package app.echoconnect.ui

import android.Manifest
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.*
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.echoconnect.*
import app.echoconnect.clipboard.ClipboardSync
import app.echoconnect.scribe.Scribe
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin
import kotlin.random.Random

/** Words a phase is called on screen, and the tone of its pill. */
private fun phaseLabel(p: String): Pair<String, String?> = when (p) {
    "listening" -> "Listening" to "danger"
    "paused" -> "Paused" to "warning"
    "transcribing" -> "Hearing it" to "info"
    "cleaning" -> "Polishing" to "info"
    "working" -> "Working" to "info"
    "done" -> "Typed" to "success"
    "error" -> "Didn't land" to "danger"
    "busy" -> "Busy" to "warning"
    else -> "Ready" to null
}

/**
 * Dictate on the laptop with this phone: hold the mic and talk, let go to send; a quick tap
 * locks it hands-free (tap again to send). Words stream back while you speak, and the text
 * lands wherever the laptop's cursor is.
 */
@Composable fun DictateScreen(s: UiState, host: Host, back: () -> Unit) {
    val ctx = host.context
    host.revision
    val d by Scribe.state.collectAsStateWithLifecycle()
    val l = s.laptop
    val haptic = LocalHapticFeedback.current
    LaunchedEffect(l?.connected) { if (l?.connected == true) Scribe.hello() }
    // The screen stays on while this phone is the microphone.
    val view = LocalView.current
    DisposableEffect(d.recording) {
        view.keepScreenOn = d.recording
        onDispose { view.keepScreenOn = false }
    }

    // Session stats: when this dictation started, and the last one's pace.
    var since by remember { mutableLongStateOf(0L) }
    var wpm by remember { mutableIntStateOf(0) }
    var sparks by remember { mutableIntStateOf(0) }
    var locked by remember { mutableStateOf(false) }
    var lastPhase by remember { mutableStateOf(d.phase) }
    LaunchedEffect(d.phase) {
        if (d.phase == "done" && lastPhase != "done") {
            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            sparks++
            val secs = ((System.currentTimeMillis() - since) / 1000.0).coerceAtLeast(1.0)
            if (since > 0) wpm = (d.text.split(Regex("\\s+")).count { it.isNotBlank() } * 60 / secs).toInt()
        }
        if (d.phase == "error" && lastPhase != "error") haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        if (d.phase !in setOf("listening", "paused")) locked = false
        lastPhase = d.phase
    }

    val connected = l?.connected == true
    val ready = connected && d.available && Scribe.canRecord(ctx) && Prefs.on(ctx, "dictation")
    val (pill, tone) = phaseLabel(if (d.recording && d.phase == "idle") "listening" else d.phase)

    Column {
        AppBar("Dictate", back, sub = when {
            !connected -> "Laptop not nearby"
            !d.available -> "Waiting for EchoScribe on ${l?.name}"
            d.app.isNotBlank() -> "Into ${d.app}${if (d.persona.isNotBlank()) " · ${d.persona}" else ""}"
            else -> "Types where ${l?.name}'s cursor is"
        })
        ScreenBody {
            when {
                l == null -> Banner("Pair a laptop first.", "Dictation types on a laptop running EchoFiles and EchoScribe.", "warning") { Button("Pair", size = BtnSize.Sm) { host.go("pair") } }
                !connected -> Banner("Not nearby.", "Open ${l.name} on the same Wi-Fi, or keep it in Bluetooth range.", "info")
                !Prefs.on(ctx, "dictation") -> Banner("Dictation is off.", "Turn it on to use this phone's mic with EchoScribe.", "warning") { Button("Turn on", size = BtnSize.Sm) { Prefs.set(ctx, "dictation", true); host.refresh(); Scribe.hello() } }
                !Scribe.canRecord(ctx) -> Banner("Microphone needed.", "EchoConnect only listens while you hold the mic, or when the laptop asks and you allow it.", "warning") { Button("Allow", size = BtnSize.Sm) { host.request(Manifest.permission.RECORD_AUDIO) } }
                !d.available && d.asked -> Banner("EchoScribe isn't answering.", d.error.ifBlank { "Start EchoScribe on ${l.name}; EchoFiles connects them on its own." }, "info") { Button("Retry", size = BtnSize.Sm) { Scribe.hello() } }
            }

            // ---------------------------------------------------------------- stage
            Column(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(4.dp)).background(token("bg-sunken")).border(1.dp, token("line"), RoundedCornerShape(4.dp)).padding(vertical = 22.dp, horizontal = 16.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    StatePill(pill, tone, small = true)
                    if (d.laptopAsked && d.recording) StatePill("${l?.name ?: "Laptop"} asked", "info", "laptop", small = true)
                    if (d.hinglish) StatePill("Hinglish", "warning", small = true)
                    if (d.code == "on") StatePill("Code", "info", "code", small = true)
                }
                Box(contentAlignment = Alignment.Center) {
                    Orb(
                        level = d.level,
                        live = d.recording,
                        busy = d.phase in setOf("transcribing", "cleaning", "working"),
                        enabled = ready || d.recording,
                        sparks = sparks,
                        onPress = {
                            if (!Scribe.state.value.recording) {
                                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                since = System.currentTimeMillis()
                                if (!Scribe.start(ctx, d.mode)) host.snack(Snack("Couldn't reach ${l?.name ?: "the laptop"}", "danger", "error"))
                            }
                        },
                        // Read the scribe's state directly: `d` only catches up on the next frame.
                        onRelease = { heldMs ->
                            when {
                                // A quick tap starts hands-free; the next tap sends.
                                heldMs < 300 && !locked && Scribe.state.value.recording -> { locked = true; haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove) }
                                locked && heldMs < 300 && Scribe.state.value.recording && System.currentTimeMillis() - since > 600 -> { locked = false; Scribe.stop() }
                                !locked -> Scribe.stop()
                            }
                        },
                    )
                }
                if (d.recording) Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button("Cancel", Variant.Ghost, BtnSize.Sm, icon = "close") { locked = false; Scribe.cancel(); haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove) }
                    if (locked) Button("Send", Variant.Primary, BtnSize.Sm, icon = "send") { locked = false; Scribe.stop() }
                }
                Txt(
                    when {
                        d.recording && locked -> "Hands-free · tap to send"
                        d.recording -> "Let go to send"
                        ready -> "Hold to talk · tap for hands-free"
                        else -> " "
                    },
                    muted = true, size = 13, line = 18,
                )

                // Live words, then the result.
                val shown = when {
                    d.recording || d.phase == "listening" || d.phase == "paused" -> d.live
                    d.phase == "done" -> d.text
                    d.phase == "error" -> d.error
                    d.phase in setOf("transcribing", "cleaning", "working") -> d.live.ifBlank { d.detail }
                    else -> ""
                }
                AnimatedContent(targetState = shown, transitionSpec = { fadeIn(tween(160)) togetherWith fadeOut(tween(120)) }, label = "words") { t ->
                    Text(
                        t.ifBlank { if (ready) "Your words show up here as you speak." else "" },
                        color = when {
                            t.isBlank() -> token("ink-faint")
                            d.phase == "error" -> token("danger-ink")
                            d.phase == "done" -> token("ink-strong")
                            else -> token("ink")
                        },
                        fontSize = if (t.length > 160) 15.sp else 18.sp,
                        lineHeight = if (t.length > 160) 22.sp else 26.sp,
                        textAlign = TextAlign.Center,
                        maxLines = 8,
                        modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
                    )
                }
                if (d.phase == "done" && wpm > 0) Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    Stat("$wpm", "wpm")
                    Stat("${d.text.split(Regex("\\s+")).count { it.isNotBlank() }}", "words")
                    Stat("${d.history.size}", "this session")
                }
            }

            // ---------------------------------------------------------------- mode and switches
            Segmented(listOf("dictate" to "Dictate", "edit" to "Edit selection", "note" to "Note"), d.mode, "Mode") { Scribe.setMode(it) }
            ListGroup("Switches", foot = "These change EchoScribe on the laptop too.") {
                ListRow("Hinglish", "Indic model, Latin script, whatever the app", icon = "message", trailing = { Switch(d.hinglish, "Hinglish", enabled = d.available) { Scribe.command("hinglish") } })
                ListRow(
                    "Code mode",
                    when (d.code) { "on" -> "Everywhere: camelCase, symbols, repo names"; "off" -> "Off"; else -> "In editors and terminals" },
                    icon = "code",
                    trailing = { Button(when (d.code) { "on" -> "Everywhere"; "off" -> "Off"; else -> "Auto" }, size = BtnSize.Sm, enabled = d.available) { Scribe.command("code") } },
                )
                ListRow("Personality", d.persona.ifBlank { "Follows the app" }, icon = "user", trailing = { Button("Next", size = BtnSize.Sm, enabled = d.available) { Scribe.command("persona") } })
            }

            // ---------------------------------------------------------------- remote keys
            val keys = listOf(
                KeyAction("send", "Enter", true) { Scribe.command("key") { put("key", "Return") } },
                KeyAction("arrow-right", "Tab", true) { Scribe.command("key") { put("key", "Tab") } },
                KeyAction("pause", if (d.phase == "paused") "Resume" else "Pause", d.recording) { Scribe.command("pause") },
                KeyAction("undo", "Undo", true) { Scribe.command("undo") },
                KeyAction("redo", "Raw words", true) { Scribe.command("undo_cleanup") },
                KeyAction("paste", "Paste last", true) { Scribe.command("paste_last") },
            )
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                keys.chunked(3).forEach { row ->
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        row.forEach { k ->
                            Column(
                                Modifier.weight(1f).disabled(!(k.enabled && d.available)).clip(RoundedCornerShape(4.dp)).background(token("bg-raised")).border(1.dp, token("line"), RoundedCornerShape(4.dp))
                                    .tap(k.enabled && d.available, label = k.label, radius = 4.dp) { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); k.run() }.padding(vertical = 12.dp),
                                horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp),
                            ) {
                                Glyph(k.icon, color = token("world-linux"), size = 18)
                                Txt(k.label, size = 12, line = 16, bold = true, color = token("ink-strong"))
                            }
                        }
                    }
                }
            }

            // ---------------------------------------------------------------- what landed
            if (d.history.isNotEmpty()) ListGroup("Just now", "Copy all", { ClipboardSync.copyAgain(ctx, d.history.reversed().joinToString("\n\n")); host.snack(Snack("Copied", icon = "check")) }) {
                d.history.take(6).forEach { t ->
                    ListRow(t.take(140), null, icon = "check", trailing = { IconButton("copy", "Copy") { ClipboardSync.copyAgain(ctx, t); host.snack(Snack("Copied", icon = "check")) } })
                }
            }
            Txt("Tip: add the “Dictate on laptop” tile to Quick Settings, or pick Phone as EchoScribe's microphone and press Right Ctrl on the laptop.", muted = true, size = 12, line = 17)
        }
    }
}

private class KeyAction(val icon: String, val label: String, val enabled: Boolean, val run: () -> Unit)

@Composable private fun Stat(value: String, label: String) {
    Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(value, color = token("ink-strong"), fontFamily = Mono, fontWeight = FontWeight.Bold, fontSize = 18.sp)
        Txt(label, muted = true, size = 12, line = 18)
    }
}

/**
 * The mic: concentric squares (EchoConnect's ring motif) that breathe with your voice while
 * recording, orbit while the laptop works, and throw a burst of sparks when the text lands.
 */
@Composable private fun Orb(level: Float, live: Boolean, busy: Boolean, enabled: Boolean, sparks: Int, onPress: () -> Unit, onRelease: (Long) -> Unit) {
    val ember = token("brand-ember")
    val accent = token("accent")
    val line = token("line-strong")
    val raised = token("bg-raised")
    val voice by animateFloatAsState(if (live) level else 0f, spring(dampingRatio = 0.55f, stiffness = 380f), label = "voice")
    val press by animateFloatAsState(if (live) 0.94f else 1f, spring(dampingRatio = 0.5f, stiffness = 500f), label = "press")
    val t = rememberInfiniteTransition(label = "orb")
    val breathe by t.animateFloat(0f, 1f, infiniteRepeatable(tween(2200, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "breathe")
    val spin by t.animateFloat(0f, 360f, infiniteRepeatable(tween(1400, easing = LinearEasing)), label = "spin")
    // Sparks: a short burst each time `sparks` ticks.
    val burst = remember { Animatable(1f) }
    val seeds = remember(sparks) { List(18) { Random.nextFloat() to Random.nextFloat() } }
    LaunchedEffect(sparks) { if (sparks > 0) { burst.snapTo(0f); burst.animateTo(1f, tween(900, easing = FastOutSlowInEasing)) } }
    val tint = if (live) ember else if (busy) accent else line
    // pointerInput captures its block once; read the newest callbacks through these.
    val pressNow by rememberUpdatedState(onPress)
    val releaseNow by rememberUpdatedState(onRelease)

    Box(
        Modifier.size(232.dp)
            .semantics {
                contentDescription = if (live) "Recording. Double tap to send." else "Dictate on the laptop. Double tap to start, again to send."
                // TalkBack can't hold: a double tap toggles (as a quick tap does, hands-free).
                if (enabled) onClick(if (live) "Send" else "Start dictating") { pressNow(); releaseNow(0); true }
            }
            .pointerInput(enabled) {
                if (!enabled) return@pointerInput
                detectTapGestures(onPress = {
                    val down = System.currentTimeMillis()
                    pressNow()
                    tryAwaitRelease()
                    releaseNow(System.currentTimeMillis() - down)
                })
            },
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val c = Offset(size.width / 2, size.height / 2)
            fun square(half: Float, color: Color, width: Float, rotate: Float = 0f) {
                val tl = Offset(c.x - half, c.y - half)
                if (rotate == 0f) drawRoundRect(color, tl, Size(half * 2, half * 2), CornerRadius(4.dp.toPx()), style = Stroke(width))
                else rotate(rotate, c) { drawRoundRect(color, tl, Size(half * 2, half * 2), CornerRadius(4.dp.toPx()), style = Stroke(width)) }
            }
            val base = size.minDimension * 0.24f
            // Three rings: the outer ones ride the voice, the idle ones breathe.
            for (i in 0..2) {
                val reach = base + i * size.minDimension * 0.085f + (if (live) voice * size.minDimension * 0.07f * (i + 1) else breathe * 4.dp.toPx() * (i + 1))
                val a = if (live) (0.75f - i * 0.2f) * (0.4f + voice) else if (busy) 0.5f - i * 0.12f else 0.22f - i * 0.06f
                square(reach, tint.copy(alpha = a.coerceIn(0.05f, 1f)), (2.5f - i * 0.6f).dp.toPx(), if (busy) spin * (if (i % 2 == 0) 1 else -1) * (0.25f + i * 0.1f) else 0f)
            }
            // Sparks when text lands.
            if (burst.value < 1f) {
                seeds.forEachIndexed { k, (r, s) ->
                    val angle = (k / seeds.size.toFloat()) * 2 * PI + r * 0.6
                    val dist = base + burst.value * size.minDimension * (0.25f + s * 0.2f)
                    val p = Offset(c.x + (cos(angle) * dist).toFloat(), c.y + (sin(angle) * dist).toFloat())
                    val sz = (3f + s * 4f).dp.toPx() * (1f - burst.value)
                    drawRect(if (k % 3 == 0) accent else ember, Offset(p.x - sz / 2, p.y - sz / 2), Size(sz, sz), alpha = 1f - burst.value)
                }
            }
        }
        Box(
            Modifier.scale(press).size(108.dp).clip(RoundedCornerShape(4.dp)).background(if (live) ember else raised).border(1.dp, if (live) ember else line, RoundedCornerShape(4.dp)),
            contentAlignment = Alignment.Center,
        ) {
            Glyph("mic", color = if (live) Color.Black else token("ink-strong"), size = 40, stroke = 2.2f)
        }
    }
}

/** Home's way in: a slim card that says who's listening. */
@Composable fun DictateCard(l: Laptop?, host: Host) {
    val d by Scribe.state.collectAsStateWithLifecycle()
    val ok = l?.connected == true && d.available
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(4.dp)).background(token("bg-raised")).border(1.dp, if (d.recording) token("brand-ember") else token("line"), RoundedCornerShape(4.dp))
            .tap(label = "Dictate on laptop", radius = 4.dp) { host.go("dictate") }.padding(12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.size(40.dp).background(if (d.recording) token("brand-ember") else token("state-hover"), RoundedCornerShape(4.dp)), contentAlignment = Alignment.Center) {
            Glyph("mic", color = if (d.recording) Color.Black else token("world-linux"))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Txt("Dictate on ${l?.name ?: "laptop"}", size = 14, line = 19, bold = true, color = token("ink-strong"))
            Txt(
                when {
                    d.recording -> "Listening…"
                    ok -> "Talk here, it types there · EchoScribe"
                    l?.connected == true -> "Start EchoScribe on the laptop"
                    else -> "Laptop not nearby"
                },
                muted = true, size = 12, line = 16,
            )
        }
        if (ok) StatePill("Ready", "success", small = true)
        Glyph("chevron-right")
    }
}
