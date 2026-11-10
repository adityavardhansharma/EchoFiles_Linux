package app.echoconnect.ui

import android.app.Activity
import android.content.ContextWrapper
import android.os.Build
import android.view.HapticFeedbackConstants
import android.view.View
import androidx.activity.compose.BackHandler
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.*
import androidx.compose.foundation.*
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.*
import androidx.compose.ui.*
import androidx.compose.ui.draw.*
import androidx.compose.ui.geometry.*
import androidx.compose.ui.graphics.*
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.*
import androidx.compose.ui.text.font.*
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.*
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.echoconnect.Connect
import app.echoconnect.Prefs
import app.echoconnect.R
import kotlinx.coroutines.launch
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

// ------------------------------------------------------------------ Relay: two materials and a signal

/**
 * Relay's palette. Carbon is the laptop's half (it takes the laptop's Omarchy theme under
 * Match laptop); paper is this phone's half (day or night with the system). Signal is the one
 * loud colour, kept for things that cross between them.
 */
@Immutable data class Relay(
    val paper: Color, val paper2: Color, val paper3: Color, val cap: Color, val edge: Color,
    val ink: Color, val ink2: Color, val ink3: Color,
    val carbon: Color, val carbon2: Color, val carbon3: Color, val capd: Color, val edged: Color,
    val chalk: Color, val chalk2: Color, val chalk3: Color,
    val sig: Color, val onSig: Color, val sigInk: Color,
    val live: Color, val liveInk: Color, val warn: Color, val warnInk: Color, val bad: Color, val badInk: Color,
    val night: Boolean,
)

private val SIGNAL = Color(0xFFFF4F1A)
private val DAY_PAPER = listOf(0xFFECE8E1, 0xFFE2DDD3, 0xFFD2CBBE, 0xFFF9F7F2, 0xFFC4BBAB, 0xFF161512, 0xFF4E4B44, 0xFF67635B)
private val NIGHT_PAPER = listOf(0xFF191816, 0xFF22211E, 0xFF34322E, 0xFF2C2B28, 0xFF0A0A09, 0xFFF2EFE8, 0xFFC2BEB4, 0xFF9C978E)

fun relayOf(night: Boolean, laptop: Map<String, Long>?): Relay {
    val p = (if (night) NIGHT_PAPER else DAY_PAPER).map { Color(it) }
    fun t(k: String, d: Long) = Color(laptop?.get(k) ?: d)
    val sig = if (laptop != null) t("accent", 0xFFFF4F1A) else SIGNAL
    val onSig = if (laptop != null) t("on-accent", 0xFF000000) else Color(0xFF160700)
    return Relay(
        paper = p[0], paper2 = p[1], paper3 = p[2], cap = p[3], edge = p[4], ink = p[5], ink2 = p[6], ink3 = p[7],
        carbon = t("bg", 0xFF121211), carbon2 = t("bg-raised", 0xFF1E1E1C), carbon3 = t("line", 0xFF2D2D2A),
        capd = if (laptop != null) t("bg-raised", 0xFF2B2B28) else Color(0xFF2B2B28), edged = t("bg-deep", 0xFF000000),
        chalk = t("ink-strong", 0xFFF2EFE8), chalk2 = t("ink", 0xFFBEBAB0), chalk3 = t("ink-muted", 0xFF97938B),
        sig = sig, onSig = onSig, sigInk = if (night) sig else lerp(sig, Color.Black, 0.28f),
        live = if (laptop != null) t("success", 0xFF2BD67B) else Color(0xFF2BD67B), liveInk = if (night) Color(0xFF2BD67B) else Color(0xFF0B7A3F),
        warn = Color(0xFFF0A81C), warnInk = if (night) Color(0xFFF0A81C) else Color(0xFF8A5A00),
        bad = Color(0xFFF0444B), badInk = if (night) Color(0xFFFF6B70) else Color(0xFFB3191F),
        night = night,
    )
}

val LocalRelay = staticCompositionLocalOf { relayOf(false, null) }
/** True inside a carbon (laptop-side) surface: text and LEDs pick their carbon shades. */
val LocalOnCarbon = compositionLocalOf { false }
@Composable fun relay(): Relay = LocalRelay.current

/** Text colour by emphasis (1 strong, 2 secondary, 3 caption) for the surface we're on. */
@Composable fun ink(level: Int = 1): Color {
    val r = relay(); val c = LocalOnCarbon.current
    return when (level) { 1 -> if (c) r.chalk else r.ink; 2 -> if (c) r.chalk2 else r.ink2; else -> if (c) r.chalk3 else r.ink3 }
}

/**
 * Every EchoConnect window: paper follows the system's dark mode; carbon is Relay's own, or the
 * laptop's Omarchy theme (sent over the connection) under Match laptop. Tints the system bars.
 */
@Composable fun AppTheme(content: @Composable () -> Unit) {
    val ctx = LocalContext.current
    val state by Connect.state.collectAsStateWithLifecycle()
    val night = isSystemInDarkTheme()
    var revision by remember { mutableIntStateOf(0) }
    DisposableEffect(Unit) {
        val prefs = ctx.getSharedPreferences("echoconnect", android.content.Context.MODE_PRIVATE)
        val listener = android.content.SharedPreferences.OnSharedPreferenceChangeListener { _, key -> if (key?.startsWith("theme.") == true) revision++ }
        prefs.registerOnSharedPreferenceChangeListener(listener)
        onDispose { prefs.unregisterOnSharedPreferenceChangeListener(listener) }
    }
    val r = remember(revision, state.theme, night) {
        val choice = Prefs.themeMode(ctx)
        val laptop = when {
            choice == "laptop" && state.theme != null -> THEMES.getValue("echo") + state.theme!!
            choice in THEMES -> THEMES.getValue(choice)
            else -> null
        }
        relayOf(night, laptop)
    }
    val view = LocalView.current
    if (!view.isInEditMode) SideEffect {
        var c = view.context
        while (c is ContextWrapper && c !is Activity) c = c.baseContext
        (c as? Activity)?.window?.let { window ->
            androidx.core.view.WindowCompat.getInsetsController(window, window.decorView).apply {
                isAppearanceLightStatusBars = r.carbon.luminance() > 0.5f
                isAppearanceLightNavigationBars = !r.night
            }
            window.isNavigationBarContrastEnforced = false
        }
    }
    val scheme = if (r.night) darkColorScheme(primary = r.sig, onPrimary = r.onSig, background = r.paper, onBackground = r.ink, surface = r.cap, onSurface = r.ink)
        else lightColorScheme(primary = r.sig, onPrimary = r.onSig, background = r.paper, onBackground = r.ink, surface = r.cap, onSurface = r.ink)
    CompositionLocalProvider(LocalRelay provides r, LocalContentColor provides r.ink) {
        MaterialTheme(colorScheme = scheme, content = content)
    }
}

// ------------------------------------------------------------------ type: four voices

@OptIn(ExperimentalTextApi::class)
private fun sans(w: Int) = Font(R.font.instrument_sans, FontWeight(w), variationSettings = FontVariation.Settings(FontVariation.weight(w)))
/** Names and actions. */
val Sans = FontFamily(sans(400), sans(500), sans(600), sans(700))
/** The human aside, always italic and always short. */
val Serif = FontFamily(Font(R.font.instrument_serif_italic, FontWeight.Normal, FontStyle.Italic))
/** Engraved labels: Omarchy's font. */
val Mono = FontFamily(Font(R.font.jetbrains_mono), Font(R.font.jetbrains_mono_bold, FontWeight.Bold))
/** Live readouts only. */
@OptIn(ExperimentalTextApi::class)
val Dot = FontFamily(Font(R.font.doto, FontWeight.Black, variationSettings = FontVariation.Settings(FontVariation.weight(900))))

@Composable fun T(
    text: String, size: Int = 15, weight: Int = 400, color: Color = LocalContentColor.current, modifier: Modifier = Modifier,
    line: Int = size + size / 3 + 1, maxLines: Int = Int.MAX_VALUE, align: TextAlign? = null, tracking: Float = 0f, strike: Boolean = false,
) = Text(text, modifier, color = color, fontFamily = Sans, fontWeight = FontWeight(weight), fontSize = size.sp, lineHeight = line.sp, letterSpacing = (tracking * size).sp,
    textAlign = align, maxLines = maxLines, overflow = TextOverflow.Ellipsis, textDecoration = if (strike) TextDecoration.LineThrough else null)

@Composable fun T(text: AnnotatedString, size: Int = 15, color: Color = LocalContentColor.current, modifier: Modifier = Modifier) =
    Text(text, modifier, color = color, fontFamily = Sans, fontSize = size.sp, lineHeight = (size + size / 3 + 1).sp)

@Composable fun Lbl(text: String, color: Color = ink(3), modifier: Modifier = Modifier, size: Int = 11, maxLines: Int = 1, align: TextAlign? = null) =
    Text(text.uppercase(), modifier, color = color, fontFamily = Mono, fontWeight = FontWeight.Bold, fontSize = size.sp, lineHeight = (size + 4).sp,
        letterSpacing = (size * 0.08f).sp, maxLines = maxLines, overflow = TextOverflow.Ellipsis, textAlign = align)

@Composable fun MonoText(text: String, size: Int = 13, color: Color = LocalContentColor.current, modifier: Modifier = Modifier, maxLines: Int = Int.MAX_VALUE) =
    Text(text, modifier, color = color, fontFamily = Mono, fontSize = size.sp, lineHeight = (size + 7).sp, maxLines = maxLines, overflow = TextOverflow.Ellipsis)

@Composable fun Dm(text: String, size: Int, color: Color = LocalContentColor.current, modifier: Modifier = Modifier, glow: Boolean = false) =
    Text(text, modifier, color = color, fontFamily = Dot, fontWeight = FontWeight.Black, fontSize = size.sp, lineHeight = size.sp, letterSpacing = (size * 0.03f).sp, maxLines = 1,
        style = if (glow) TextStyle(shadow = Shadow(color.copy(alpha = 0.55f), blurRadius = size * 0.4f)) else TextStyle.Default)

@Composable fun Ser(text: String, size: Int, color: Color = LocalContentColor.current, modifier: Modifier = Modifier) =
    Text(text, modifier, color = color, fontFamily = Serif, fontStyle = FontStyle.Italic, fontSize = size.sp, lineHeight = (size * 1.05f).sp)

/** A screen title: Sans, with an optional serif aside after it. */
@Composable fun Title(main: String, aside: String? = null, size: Int = 34, modifier: Modifier = Modifier) {
    val color = LocalContentColor.current
    Text(buildAnnotatedString {
        append(main)
        if (aside != null) { append(" "); withStyle(SpanStyle(fontFamily = Serif, fontStyle = FontStyle.Italic, fontWeight = FontWeight.Normal, fontSize = (size * 1.15f).sp, letterSpacing = 0.sp)) { append(aside) } }
    }, modifier.semantics { heading() }, color = color, fontFamily = Sans, fontWeight = FontWeight.SemiBold, fontSize = size.sp, lineHeight = (size * 1.08f).sp, letterSpacing = (-0.035f * size).sp)
}

/** Plain text with **strong** spans. */
@Composable fun strong(text: String): AnnotatedString {
    val c = ink(1)
    return buildAnnotatedString {
        text.split("**").forEachIndexed { i, part -> if (i % 2 == 1) withStyle(SpanStyle(color = c, fontWeight = FontWeight.SemiBold)) { append(part) } else append(part) }
    }
}

// ------------------------------------------------------------------ glyphs

@Composable fun Glyph(name: String, description: String? = null, color: Color = LocalContentColor.current, size: Int = 20, stroke: Float = 1.9f) {
    val paths = remember(name) { GLYPHS[name].orEmpty().map { PathParser().parsePathString(it).toPath() } }
    Canvas(Modifier.size(size.dp).then(if (description != null) Modifier.semantics { contentDescription = description } else Modifier.clearAndSetSemantics { })) {
        val s = this.size.width / 24f
        withTransform({ scale(s, s, Offset.Zero) }) { paths.forEach { drawPath(it, color, style = Stroke(stroke, cap = StrokeCap.Round, join = StrokeJoin.Round)) } }
    }
}

/** A file's format as an engraved plate: PDF, ZIP, PNG… tinted by kind. */
@Composable fun Plate(fileName: String) {
    val r = relay()
    val ext = fileName.substringAfterLast('.', "").uppercase().take(4).ifBlank { "FILE" }
    val color = when (iconFor(fileName)) {
        "file-pdf" -> if (LocalOnCarbon.current) r.bad else r.badInk
        "file-archive" -> if (LocalOnCarbon.current) r.warn else r.warnInk
        "file-image" -> if (LocalOnCarbon.current) r.sig else r.sigInk
        "file-sheet" -> if (LocalOnCarbon.current) r.live else r.liveInk
        else -> ink(2)
    }
    Box(Modifier.border(1.5.dp, color, RoundedCornerShape(6.dp)).padding(horizontal = 5.dp, vertical = 1.dp)) {
        Text(ext, color = color, fontFamily = Mono, fontWeight = FontWeight.Bold, fontSize = 10.sp, lineHeight = 14.sp, letterSpacing = 0.6.sp)
    }
}

private val EXT = mapOf(
    "jpg" to "file-image", "jpeg" to "file-image", "png" to "file-image", "webp" to "file-image", "heic" to "file-image", "gif" to "file-image",
    "mp4" to "file-video", "mkv" to "file-video", "mov" to "file-video", "webm" to "file-video", "mp3" to "file-audio", "m4a" to "file-audio", "opus" to "file-audio", "ogg" to "file-audio", "flac" to "file-audio",
    "pdf" to "file-pdf", "doc" to "file-doc", "docx" to "file-doc", "odt" to "file-doc", "txt" to "file-lines", "md" to "file-text",
    "xls" to "file-sheet", "xlsx" to "file-sheet", "csv" to "file-sheet", "ods" to "file-sheet", "ppt" to "file-slides", "pptx" to "file-slides", "odp" to "file-slides",
    "zip" to "file-archive", "7z" to "file-archive", "tar" to "file-archive", "gz" to "file-archive", "rar" to "file-archive", "apk" to "file-package",
    "rs" to "file-code", "py" to "file-code", "kt" to "file-code", "js" to "file-code", "ts" to "file-code", "c" to "file-code", "sh" to "file-script")
fun iconFor(name: String) = EXT[name.substringAfterLast('.', "").lowercase()] ?: "file"

// ------------------------------------------------------------------ haptics

/** Every mechanical part answers in the hand. */
object Haptics {
    fun key(v: View) = v.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
    fun tick(v: View) = v.performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK)
    fun confirm(v: View) = v.performHapticFeedback(if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.CONFIRM else HapticFeedbackConstants.VIRTUAL_KEY)
    fun reject(v: View) = v.performHapticFeedback(if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.REJECT else HapticFeedbackConstants.LONG_PRESS)
    fun heavy(v: View) = v.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
}

// ------------------------------------------------------------------ surfaces

/** Keeps content clear of a display cutout on the side; backgrounds still run edge to edge. */
@Composable fun Modifier.sideSafe(): Modifier = windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))

/** Text on the fixed-black LED panels: light whatever the laptop's theme, so light themes stay readable. */
val OnBlack = Color(0xFFF2EFE8)
val OnBlack2 = Color(0xFFBEBAB0)
val OnBlack3 = Color(0xFF97938B)

/** The laptop's half: carbon, with chalk text. */
@Composable fun Carbon(modifier: Modifier = Modifier, color: Color? = null, content: @Composable () -> Unit) {
    val r = relay()
    CompositionLocalProvider(LocalOnCarbon provides true, LocalContentColor provides r.chalk) {
        Box(modifier.background(color ?: r.carbon)) { content() }
    }
}

/** Back to paper inside a carbon surface. */
@Composable fun Paper(modifier: Modifier = Modifier, color: Color? = null, content: @Composable () -> Unit) {
    val r = relay()
    CompositionLocalProvider(LocalOnCarbon provides false, LocalContentColor provides r.ink) {
        Box(modifier.then(if (color != null) Modifier.background(color) else Modifier)) { content() }
    }
}

/** The carbon header every screen opens with: status-bar inset, optional Back, then content. */
@Composable fun Head(back: (() -> Unit)? = null, trailing: (@Composable RowScope.() -> Unit)? = null, content: @Composable ColumnScope.() -> Unit) {
    Carbon(Modifier.fillMaxWidth()) {
        Column(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.statusBars).sideSafe().padding(start = 16.dp, end = 16.dp, top = if (back != null || trailing != null) 4.dp else 20.dp, bottom = 20.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp)) {
            if (back != null || trailing != null) Row(Modifier.fillMaxWidth().height(48.dp), verticalAlignment = Alignment.CenterVertically) {
                if (back != null) Box(Modifier.offset(x = (-12).dp).size(48.dp).clickable(onClickLabel = "Back", role = Role.Button, onClick = back).semantics { contentDescription = "Back" }, contentAlignment = Alignment.Center) {
                    Glyph("arrow-left", size = 22)
                }
                Spacer(Modifier.weight(1f))
                trailing?.invoke(this)
            }
            content()
        }
    }
}

/** The 8dp signal stripe under a header: the edge where the laptop ends. */
@Composable fun SigBar() = Box(Modifier.fillMaxWidth().height(8.dp).background(relay().sig))

/** A recessed paper plate that groups rows. */
@Composable fun PlateGroup(title: String? = null, right: (@Composable () -> Unit)? = null, content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().background(relay().paper2, RoundedCornerShape(20.dp)).padding(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (title != null) Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, top = 8.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Lbl(title, ink(2), Modifier.weight(1f).semantics { heading() }); right?.invoke()
        }
        content()
    }
}

/** One row on a plate: a raised key-coloured strip. */
@Composable fun PlateRow(title: String, sub: String? = null, subColor: Color? = null, onClick: (() -> Unit)? = null, trailing: (@Composable RowScope.() -> Unit)? = null) {
    val r = relay()
    Row(Modifier.fillMaxWidth().heightIn(min = 60.dp).clip(RoundedCornerShape(14.dp)).background(r.cap).then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
        .padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            T(title, 15, 600, maxLines = 1)
            if (sub != null) T(sub, 13, color = subColor ?: ink(2), maxLines = 2)
        }
        trailing?.invoke(this)
    }
}

/** A dashed box for side notes. */
@Composable fun Note(title: String, body: String, modifier: Modifier = Modifier) {
    val r = relay()
    Column(modifier.fillMaxWidth().drawBehind {
        drawRoundRect(r.paper3, cornerRadius = CornerRadius(16.dp.toPx()), style = Stroke(1.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 5.dp.toPx()))))
    }.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Lbl(title, ink(1)); T(body, 13, color = ink(2))
    }
}

// ------------------------------------------------------------------ keys

enum class Tone { Paper, Carbon, Signal, Live, Cut }

/**
 * A keycap: a face over a 3dp lip. It travels the whole lip in 60 ms when pressed, and answers
 * with a key haptic. Busy keys keep their size and show three blinking dots.
 */
@Composable fun Key(
    label: String, modifier: Modifier = Modifier, tone: Tone = Tone.Paper, icon: String? = null, trailingIcon: String? = null, height: Dp = 52.dp,
    enabled: Boolean = true, busy: Boolean = false, size: Int = 15, radius: Dp = 16.dp, note: String? = null, start: Boolean = false, onClick: () -> Unit,
) {
    val r = relay()
    val (face, lip, fg) = when (tone) {
        Tone.Paper -> Triple(r.cap, r.edge, r.ink)
        Tone.Carbon -> Triple(r.capd, r.edged, r.chalk)
        Tone.Signal -> Triple(r.sig, lerp(r.sig, Color.Black, 0.45f), r.onSig)
        Tone.Live -> Triple(r.live, lerp(r.live, Color.Black, 0.4f), Color(0xFF04170C))
        Tone.Cut -> Triple(Color(0xFF3A1012), Color(0xFF1C0607), Color(0xFFFFD9DA))
    }
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val down by animateDpAsState(if (!enabled || pressed) 3.dp else 0.dp, tween(60, easing = LinearEasing), label = "key")
    val view = LocalView.current
    val shape = RoundedCornerShape(radius)
    Row(modifier.padding(bottom = 3.dp).height(height).alpha(if (enabled) 1f else 0.4f)
        .clickable(source, null, enabled && !busy, label, Role.Button) { Haptics.key(view); onClick() }
        .offset(y = down)
        .drawBehind { drawRoundRect(lip, Offset(0f, (3.dp - down).toPx()), this.size, CornerRadius(radius.toPx())) }
        .background(face, shape).padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = if (start) Arrangement.spacedBy(12.dp) else Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally)) {
        if (busy) DotLoader(fg) else {
            if (icon != null) Glyph(icon, color = fg, size = if (height > 56.dp) 22 else 18, stroke = 2.1f)
            Text(label, color = fg, fontFamily = Sans, fontWeight = FontWeight.SemiBold, fontSize = size.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
                modifier = if (start) Modifier.weight(1f) else Modifier)
            if (note != null) Lbl(note, fg.copy(alpha = 0.7f))
            if (trailingIcon != null) Glyph(trailingIcon, color = fg, size = 18, stroke = 2.1f)
        }
    }
}

/** A small engraved keycap, for paths written as the keys you'd press. */
@Composable fun Kc(text: String, hot: Boolean = false) {
    val r = relay()
    Box(Modifier.drawBehind { drawRoundRect(if (hot) lerp(r.sig, Color.Black, 0.45f) else r.edge, Offset(0f, 2.dp.toPx()), size, CornerRadius(7.dp.toPx())) }
        .background(if (hot) r.sig else r.cap, RoundedCornerShape(7.dp)).padding(horizontal = 8.dp, vertical = 5.dp)) {
        Text(text, color = if (hot) r.onSig else r.ink, fontFamily = Mono, fontWeight = FontWeight.Bold, fontSize = 11.sp, lineHeight = 15.sp)
    }
}

/** Three square dots blinking in turn: something is on its way. */
@Composable fun DotLoader(color: Color = relay().sig) {
    val t = rememberInfiniteTransition(label = "loader")
    val p by t.animateFloat(0f, 3f, infiniteRepeatable(tween(1200, easing = LinearEasing)), label = "p")
    Row(Modifier.semantics { contentDescription = "Working" }, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
        repeat(3) { i -> Box(Modifier.size(7.dp).alpha(if (p.toInt() == i) 1f else 0.25f).background(color, RoundedCornerShape(2.dp))) }
    }
}

// ------------------------------------------------------------------ switches and lights

/**
 * The slide switch: a square-ish thumb that springs across and overshoots before it settles,
 * with a tick. "Needs" glows amber: on, but Android hasn't allowed it yet.
 */
@Composable fun Slide(checked: Boolean, label: String, needs: Boolean = false, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    val r = relay()
    val view = LocalView.current
    val x by animateDpAsState(if (checked) 27.dp else 3.dp, spring(dampingRatio = 0.55f, stiffness = 700f), label = "thumb")
    val track by animateColorAsState(when { needs && checked -> r.warn; checked -> r.sig; LocalOnCarbon.current -> r.carbon3; else -> r.paper3 }, tween(180), label = "track")
    Box(Modifier.size(60.dp, 48.dp).alpha(if (enabled) 1f else 0.4f)
        .toggleable(checked, remember { MutableInteractionSource() }, null, enabled, Role.Switch) { Haptics.tick(view); onChange(it) }
        .semantics { contentDescription = label }, contentAlignment = Alignment.Center) {
        Box(Modifier.size(56.dp, 30.dp).background(track, RoundedCornerShape(9.dp))) {
            Box(Modifier.offset(x = x, y = 3.dp).size(26.dp, 24.dp).drawBehind { drawRoundRect(r.edge, Offset(0f, 2.dp.toPx()), size, CornerRadius(7.dp.toPx())) }.background(r.cap, RoundedCornerShape(7.dp)))
        }
    }
}

enum class LedState { Off, Live, Signal, Warn, Bad }

/** A status light: lit ones glow; Warn blinks once a second. */
@Composable fun Led(state: LedState, size: Dp = 8.dp, blink: Boolean = state == LedState.Warn) {
    val r = relay()
    val color = when (state) { LedState.Off -> if (LocalOnCarbon.current) r.carbon3 else r.paper3; LedState.Live -> r.live; LedState.Signal -> r.sig; LedState.Warn -> r.warn; LedState.Bad -> r.bad }
    val lit by animateFloatAsState(if (state == LedState.Off) 0f else 1f, tween(200, delayMillis = 80), label = "lit")
    val b = if (blink) rememberInfiniteTransition(label = "blink").animateFloat(1f, 0.3f, infiniteRepeatable(tween(1000, easing = { if (it < 0.5f) 0f else 1f })), label = "b").value else 1f
    Canvas(Modifier.size(size).alpha(b).clearAndSetSemantics { }) {
        if (lit > 0f) drawCircle(color.copy(alpha = 0.28f * lit), radius = this.size.minDimension * 0.95f)
        drawCircle(color, radius = this.size.minDimension / 2)
    }
}

// ------------------------------------------------------------------ the seam

enum class SeamMode { Live, Moving, Flat, Voice }

/**
 * The link between the two halves. Live: a slow trace. Moving: faster, taller, with packets
 * rising through it. Flat: a line with one search ping crossing. Voice: bars, during a call.
 */
@Composable fun Seam(mode: SeamMode, modifier: Modifier = Modifier, height: Dp = 64.dp, left: String? = null, right: String? = null, rightColor: Color? = null, description: String) {
    val r = relay()
    val t = rememberInfiniteTransition(label = "seam")
    val phase by t.animateFloat(0f, 1f, infiniteRepeatable(tween(if (mode == SeamMode.Moving) 500 else 1100, easing = LinearEasing)), label = "phase")
    val slow by t.animateFloat(0f, 1f, infiniteRepeatable(tween(if (mode == SeamMode.Flat) 3000 else 1600, easing = FastOutSlowInEasing)), label = "slow")
    val voice by t.animateFloat(0f, (2 * PI).toFloat(), infiniteRepeatable(tween(1000, easing = LinearEasing)), label = "voice")
    val line = if (mode == SeamMode.Voice) r.live else r.sig
    Carbon(modifier.fillMaxWidth().height(height)) {
        Canvas(Modifier.fillMaxSize().clipToBounds().semantics { contentDescription = description }) {
            drawRect(r.carbon3, size = Size(size.width, 1.dp.toPx()))
            val mid = size.height / 2
            when (mode) {
                SeamMode.Live, SeamMode.Moving -> {
                    val wl = 40.dp.toPx(); val amp = size.height * if (mode == SeamMode.Moving) 0.26f else 0.17f
                    val p = Path(); var x = -wl
                    p.moveTo(x, mid)
                    while (x <= size.width + wl) { p.lineTo(x, mid - amp * sin(2 * PI * (x / wl - phase)).toFloat()); x += 3f }
                    drawPath(p, line, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round))
                    if (mode == SeamMode.Moving) listOf(0.47f to 0f, 0.58f to 0.35f).forEach { (fx, delay) ->
                        val k = (slow + delay) % 1f
                        val y = size.height + 10.dp.toPx() - k * (size.height + 40.dp.toPx())
                        val a = when { k < 0.2f -> k / 0.2f; k > 0.8f -> (1 - k) / 0.2f; else -> 1f }
                        val w = 10.dp.toPx(); val h = 18.dp.toPx()
                        drawRoundRect(r.sig.copy(alpha = 0.3f * a), Offset(size.width * fx - w, y - 4.dp.toPx()), Size(w * 2, h + 8.dp.toPx()), CornerRadius(w))
                        drawRoundRect(r.sig.copy(alpha = a), Offset(size.width * fx - w / 2, y), Size(w, h), CornerRadius(w / 2))
                    }
                }
                SeamMode.Flat -> {
                    drawRect(r.carbon3, Offset(0f, mid - 1.dp.toPx()), Size(size.width, 2.dp.toPx()))
                    val a = when { slow < 0.15f -> slow / 0.15f; slow > 0.85f -> (1 - slow) / 0.15f; else -> 1f }
                    drawCircle(r.chalk3.copy(alpha = a), 4.dp.toPx(), Offset(slow * size.width, mid))
                }
                SeamMode.Voice -> {
                    val step = 7.dp.toPx(); val w = 4.dp.toPx(); val n = ((size.width - 32.dp.toPx()) / step).toInt()
                    val x0 = (size.width - n * step) / 2
                    for (i in 0 until n) {
                        val base = 0.35f + 0.65f * abs(sin(i * 1.7f))
                        val h = size.height * 0.8f * base * (0.25f + 0.75f * abs(sin(voice + i * 0.6f)))
                        drawRoundRect(line, Offset(x0 + i * step, mid - h / 2), Size(w, h), CornerRadius(w / 2))
                    }
                }
            }
        }
        if (left != null || right != null) Row(Modifier.fillMaxSize().sideSafe().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            if (left != null) Box(Modifier.background(r.carbon, RoundedCornerShape(7.dp)).padding(horizontal = 7.dp, vertical = 4.dp)) { Lbl(left, r.chalk2) }
            Spacer(Modifier.weight(1f))
            if (right != null) Box(Modifier.background(r.carbon, RoundedCornerShape(7.dp)).padding(horizontal = 7.dp, vertical = 4.dp)) { Dm(right, 20, rightColor ?: r.chalk) }
        }
    }
}

// ------------------------------------------------------------------ the dial

val DIAL = listOf("Home", "Clip", "Files", "Panel")

/** Navigation as a four-position slide switch: the thumb springs between detents with a tick. */
@Composable fun Dial(active: Int, badge: Int = 0, modifier: Modifier = Modifier, onChange: (Int) -> Unit) {
    val r = relay()
    val view = LocalView.current
    BoxWithConstraints(modifier.fillMaxWidth().height(66.dp).shadow(18.dp, CircleShape).background(Color(0xFF121211), CircleShape).padding(6.dp)) {
        val gap = 4.dp
        val w = (maxWidth - gap * 3) / 4
        val x by animateDpAsState((w + gap) * active, spring(dampingRatio = 0.7f, stiffness = 500f), label = "dial")
        Box(Modifier.offset(x = x).width(w).fillMaxHeight().padding(bottom = 2.dp).drawBehind { drawRoundRect(Color(0xFFC4BBAB), Offset(0f, 2.dp.toPx()), size, CornerRadius(size.height / 2)) }.background(Color(0xFFF9F7F2), CircleShape))
        Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(gap)) {
            DIAL.forEachIndexed { i, name ->
                val on = i == active
                Column(Modifier.weight(1f).fillMaxHeight().clip(CircleShape).selectable(on, role = Role.Tab) { if (!on) { Haptics.tick(view); onChange(i) } },
                    horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically)) {
                    Box(Modifier.size(4.dp).background(if (on) r.sig else Color(0xFF2D2D2A), CircleShape))
                    Lbl(if (i == 2 && badge > 0) "$name $badge" else name, if (on) Color(0xFF161512) else Color(0xFF97938B))
                }
            }
        }
    }
}

// ------------------------------------------------------------------ tickets and slips

/** A ticket: rounded, with two notches at the perforation and a dashed tear line before the stub. */
class TicketShape(private val stub: Dp, private val radius: Dp = 16.dp, private val notch: Dp = 7.dp) : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline = with(density) {
        val x = size.width - stub.toPx(); val n = notch.toPx()
        val body = Path().apply { addRoundRect(RoundRect(0f, 0f, size.width, size.height, CornerRadius(radius.toPx()))) }
        val holes = Path().apply { addOval(Rect(Offset(x, 0f), n)); addOval(Rect(Offset(x, size.height), n)) }
        Outline.Generic(Path.combine(PathOperation.Difference, body, holes))
    }
}

/** A tear-off slip: scalloped along the top. */
object SlipShape : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline = with(density) {
        val top = 3.dp.toPx(); val bottom = 18.dp.toPx(); val r = 4.dp.toPx(); val step = 12.dp.toPx()
        val body = Path().apply { addRoundRect(RoundRect(0f, 0f, size.width, size.height, CornerRadius(top), CornerRadius(top), CornerRadius(bottom), CornerRadius(bottom))) }
        val holes = Path(); var cx = step / 2
        while (cx < size.width) { holes.addOval(Rect(Offset(cx, 0f), r)); cx += step }
        Outline.Generic(Path.combine(PathOperation.Difference, body, holes))
    }
}

/** Material is origin: [carbon] for things from the laptop, paper for things from this phone. */
@Composable fun Ticket(carbon: Boolean, modifier: Modifier = Modifier, faded: Boolean = false, stubWidth: Dp = 84.dp, description: String? = null,
                       body: @Composable ColumnScope.() -> Unit, stub: @Composable ColumnScope.() -> Unit) {
    val r = relay()
    val content = @Composable {
        Row(modifier.fillMaxWidth().height(IntrinsicSize.Min).alpha(if (faded) 0.55f else 1f).clip(TicketShape(stubWidth))
            .background(if (carbon) r.carbon else r.cap).then(if (description != null) Modifier.semantics(mergeDescendants = true) { contentDescription = description } else Modifier)) {
            Column(Modifier.weight(1f).padding(start = 16.dp, end = 12.dp, top = 13.dp, bottom = 13.dp), verticalArrangement = Arrangement.spacedBy(8.dp), content = body)
            val tear = if (carbon) r.carbon3 else r.paper3
            Column(Modifier.width(stubWidth).fillMaxHeight().drawBehind {
                drawLine(tear, Offset(1.dp.toPx(), 9.dp.toPx()), Offset(1.dp.toPx(), size.height - 9.dp.toPx()), 2.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(5.dp.toPx(), 4.dp.toPx())))
            }.padding(horizontal = 6.dp, vertical = 8.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically), content = stub)
        }
    }
    if (carbon) Carbon { content() } else Paper { content() }
}

@Composable fun Slip(carbon: Boolean, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val r = relay()
    val inner = @Composable { Column(modifier.fillMaxWidth().clip(SlipShape).background(if (carbon) r.carbon else r.cap).padding(start = 16.dp, end = 12.dp, top = 14.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp), content = content) }
    if (carbon) Carbon { inner() } else Paper { inner() }
}

/** Progress as a row of dots, lit one by one; the leading dot blinks. */
@Composable fun DotBar(fraction: Float?, modifier: Modifier = Modifier, count: Int = 22) {
    val r = relay()
    val off = if (LocalOnCarbon.current) r.carbon3 else r.paper3
    val blink by rememberInfiniteTransition(label = "lead").animateFloat(1f, 0.25f, infiniteRepeatable(tween(800, easing = { if (it < 0.5f) 0f else 1f })), label = "b")
    val sweep by rememberInfiniteTransition(label = "sweep").animateFloat(0f, 1f, infiniteRepeatable(tween(1400, easing = LinearEasing)), label = "s")
    Canvas(modifier.fillMaxWidth().height(8.dp).semantics { if (fraction != null) progressBarRangeInfo = ProgressBarRangeInfo(fraction.coerceIn(0f, 1f), 0f..1f) }) {
        val gap = 3.dp.toPx(); val d = ((size.width - gap * (count - 1)) / count).coerceAtMost(size.height)
        val lit = fraction?.let { (it.coerceIn(0f, 1f) * count).toInt() }
        for (i in 0 until count) {
            val c = Offset(i * (d + gap) + d / 2, size.height / 2)
            val color = when {
                lit == null -> if (i == (sweep * count).toInt()) r.sig else off
                i < lit -> r.sig
                i == lit -> r.sig.copy(alpha = blink)
                else -> off
            }
            drawCircle(color, d / 2, c)
        }
    }
}

// ------------------------------------------------------------------ fields

/** A mono field on black, outlined in signal while focused and in bad when wrong. */
@Composable fun Field(value: String, label: String, placeholder: String = "", error: String? = null, tag: String? = null, keyboard: KeyboardOptions = KeyboardOptions.Default, onChange: (String) -> Unit) {
    val r = relay()
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val edge = when { error != null -> r.bad; focused -> r.sig; else -> Color(0xFF2D2D2A) }
    BasicTextField(value, onChange, Modifier.fillMaxWidth().semantics { contentDescription = label }, interactionSource = source, keyboardOptions = keyboard,
        textStyle = TextStyle(fontFamily = Mono, fontSize = 16.sp, lineHeight = 23.sp, color = Color(0xFFF2EFE8)), cursorBrush = SolidColor(r.sig),
        decorationBox = { inner ->
            Column(Modifier.fillMaxWidth().background(Color.Black, RoundedCornerShape(18.dp)).border(1.5.dp, edge, RoundedCornerShape(18.dp)).padding(horizontal = 16.dp, vertical = 13.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Lbl(error ?: label, if (error != null) r.bad else if (focused) r.sig else Color(0xFF97938B), Modifier.weight(1f))
                    if (tag != null) Box(Modifier.background(Color(0xFF1E1E1C), RoundedCornerShape(5.dp)).padding(horizontal = 6.dp, vertical = 2.dp)) { Lbl(tag, Color(0xFF97938B)) }
                }
                Box { if (value.isEmpty()) MonoText(placeholder, 16, Color(0xFF6E6A62)); inner() }
            }
        })
}

// ------------------------------------------------------------------ feedback

data class Snack(val text: String, val tone: String? = null, val icon: String? = null, val action: String? = null, val id: Long = System.nanoTime(), val onAction: (() -> Unit)? = null)

/** A carbon strip that rises over the dial: an LED, one line, and at most one key. */
@Composable fun SnackbarHost(snack: Snack?, onDone: () -> Unit) {
    if (snack == null) return
    val view = LocalView.current
    LaunchedEffect(snack.id) {
        if (snack.tone == "danger") Haptics.reject(view)
        kotlinx.coroutines.delay(if (snack.action != null) 8000 else 4000); onDone()
    }
    val rise = remember(snack.id) { Animatable(20f) }
    LaunchedEffect(snack.id) { rise.animateTo(0f, spring(dampingRatio = 0.6f, stiffness = Spring.StiffnessMediumLow)) }
    Carbon(Modifier.padding(horizontal = 12.dp, vertical = 10.dp).offset(y = rise.value.dp).fillMaxWidth().shadow(16.dp, RoundedCornerShape(16.dp)).clip(RoundedCornerShape(16.dp)), Color(0xFF121211)) {
        Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(start = 14.dp, end = 6.dp, top = 6.dp, bottom = 6.dp).semantics { liveRegion = LiveRegionMode.Polite },
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Led(when (snack.tone) { "danger" -> LedState.Bad; "warning" -> LedState.Warn; null -> LedState.Signal; else -> LedState.Live }, blink = false)
            T(snack.text, 14, 500, Color(0xFFF2EFE8), Modifier.weight(1f), maxLines = 2)
            if (snack.action != null) Key(snack.action, tone = Tone.Carbon, height = 38.dp, size = 13, radius = 11.dp) { snack.onAction?.invoke(); onDone() }
        }
    }
}

/** A banner on paper: an LED, a title, a body and an optional key. */
@Composable fun Banner(title: String, body: String, state: LedState = LedState.Warn, action: (@Composable () -> Unit)? = null) {
    val r = relay()
    Row(Modifier.fillMaxWidth().background(if (state == LedState.Bad) r.bad.copy(alpha = 0.12f) else if (state == LedState.Warn) r.warn.copy(alpha = 0.14f) else r.paper2, RoundedCornerShape(18.dp)).padding(14.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.padding(top = 6.dp)) { Led(state) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) { T(title, 15, 600); T(body, 13, color = ink(2)) }
            action?.invoke()
        }
    }
}

// ------------------------------------------------------------------ overlays

/**
 * A sheet. Where it enters says who's asking: [fromTop] for the laptop (pair requests), from the
 * bottom for this phone. Scrim fades in 200 ms; the sheet slides with a slight overshoot.
 */
@Composable fun Sheet(onDismiss: () -> Unit, fromTop: Boolean = false, carbon: Boolean = true, dismissable: Boolean = true,
                      footer: (@Composable ColumnScope.() -> Unit)? = null, content: @Composable ColumnScope.() -> Unit) {
    val r = relay()
    BackHandler(enabled = dismissable, onBack = onDismiss)
    val shown = remember { Animatable(0f) }
    LaunchedEffect(Unit) { shown.animateTo(1f, tween(450, easing = CubicBezierEasing(0.2f, 0.9f, 0.25f, 1.06f))) }
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().alpha(shown.value.coerceIn(0f, 1f)).background(Color.Black.copy(alpha = 0.55f))
            .clickable(remember { MutableInteractionSource() }, null, enabled = dismissable, onClickLabel = "Close", onClick = onDismiss))
        val shape = if (fromTop) RoundedCornerShape(bottomStart = 30.dp, bottomEnd = 30.dp) else RoundedCornerShape(topStart = 30.dp, topEnd = 30.dp)
        val body = @Composable {
            Column(Modifier.fillMaxWidth().fillMaxHeight(0.94f).wrapContentHeight(if (fromTop) Alignment.Top else Alignment.Bottom)
                .graphicsLayer { translationY = (1f - shown.value) * size.height * (if (fromTop) -1f else 1f) }
                .shadow(24.dp, shape).clip(shape).background(if (carbon) r.carbon else r.paper)
                .clickable(remember { MutableInteractionSource() }, null) {}
                .then(if (fromTop) Modifier.windowInsetsPadding(WindowInsets.statusBars) else Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)).imePadding())
                .sideSafe().padding(top = if (fromTop) 18.dp else 12.dp, bottom = if (fromTop) 12.dp else 22.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp)) {
                if (!fromTop) Box(Modifier.size(40.dp, 5.dp).background(if (carbon) r.carbon3 else r.paper3, CircleShape).align(Alignment.CenterHorizontally))
                // The body scrolls when the window is short; the footer's keys always stay on screen.
                Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(14.dp), content = content)
                if (footer != null) Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp), content = footer)
                if (fromTop) Box(Modifier.size(40.dp, 5.dp).background(r.carbon3, CircleShape).align(Alignment.CenterHorizontally))
            }
        }
        Box(Modifier.align(if (fromTop) Alignment.TopCenter else Alignment.BottomCenter)) { if (carbon) Carbon { body() } else Paper { body() } }
    }
}

/**
 * A key you hold to confirm something you can't take back: it fills left to right over [millis],
 * the haptics tighten as it fills, and it fires when full. TalkBack gets a plain action.
 */
@Composable fun HoldKey(label: String, millis: Int = 1000, modifier: Modifier = Modifier, onDone: () -> Unit) {
    val r = relay()
    val view = LocalView.current
    val scope = rememberCoroutineScope()
    val fill = remember { Animatable(0f) }
    var holding by remember { mutableStateOf(false) }
    val done by rememberUpdatedState(onDone)
    val down by animateDpAsState(if (holding) 3.dp else 0.dp, tween(60), label = "down")
    LaunchedEffect(holding) {
        var last = 0
        while (holding) { val step = (fill.value * 5).toInt(); if (step > last) { Haptics.tick(view); last = step }; kotlinx.coroutines.delay(30) }
    }
    Box(modifier.fillMaxWidth().padding(bottom = 3.dp).height(64.dp).offset(y = down)
        .drawBehind { drawRoundRect(Color(0xFF1C0607), Offset(0f, (3.dp - down).toPx()), size, CornerRadius(16.dp.toPx())) }
        .clip(RoundedCornerShape(16.dp)).background(Color(0xFF3A1012))
        .semantics { contentDescription = label; onClick(label) { done(); true } }
        .pointerInput(Unit) {
            detectTapGestures(onPress = {
                holding = true
                val job = scope.launch {
                    fill.animateTo(1f, tween((millis * (1 - fill.value)).toInt(), easing = LinearEasing))
                    Haptics.heavy(view); holding = false; done()
                }
                tryAwaitRelease()
                if (fill.value < 1f) { job.cancel(); holding = false; scope.launch { fill.animateTo(0f, tween(200)) } }
            })
        }, contentAlignment = Alignment.Center) {
        Box(Modifier.align(Alignment.CenterStart).fillMaxHeight().fillMaxWidth(fill.value).background(r.bad))
        T(label, 16, 600, Color(0xFFFFD9DA))
    }
}

// ------------------------------------------------------------------ small helpers

/** "12 min ago" within a day, then "Yesterday 18:22", then the date. */
fun relativeTime(at: Long, now: Long = System.currentTimeMillis()): String {
    val mins = (now - at) / 60_000
    val time = java.text.SimpleDateFormat("HH:mm", java.util.Locale.getDefault()).format(java.util.Date(at))
    return when {
        mins < 1 -> "now"
        mins < 60 -> "${mins}m ago"
        android.text.format.DateUtils.isToday(at) -> time
        android.text.format.DateUtils.isToday(at + 86_400_000) -> "Yesterday $time"
        else -> java.text.SimpleDateFormat("d MMM HH:mm", java.util.Locale.getDefault()).format(java.util.Date(at))
    }
}

/** Base-1024 sizes: "4.1 MB". */
fun bytes(n: Long): String {
    if (n < 1024) return "$n B"
    val units = listOf("KB", "MB", "GB", "TB")
    var v = n / 1024.0; var i = 0
    while (v >= 1024 && i < units.lastIndex) { v /= 1024; i++ }
    return if (v >= 100 || i == 0) "${v.toLong()} ${units[i]}" else String.format(java.util.Locale.US, "%.1f %s", v, units[i])
}

/** Seconds as mm:ss. */
fun clock(secs: Long) = "%02d:%02d".format(secs / 60, secs % 60)
