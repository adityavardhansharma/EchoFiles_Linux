package app.echoconnect.ui

import android.app.Activity
import android.content.ContextWrapper
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.*
import androidx.compose.foundation.*
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
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.*
import androidx.compose.ui.draw.*
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.*
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.*
import androidx.compose.ui.text.font.*
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.*
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.echoconnect.Clip
import app.echoconnect.Connect
import app.echoconnect.Prefs
import app.echoconnect.R

// ------------------------------------------------------------------ theme

val Mono = FontFamily(Font(R.font.jetbrains_mono), Font(R.font.jetbrains_mono_bold, FontWeight.Bold))
val LocalTokens = staticCompositionLocalOf { THEMES.getValue("echo") }
@Composable fun token(name: String): Color = Color(LocalTokens.current[name] ?: THEMES.getValue("echo").getValue(name))
internal fun Map<String, Long>.color(name: String) = Color(this[name] ?: THEMES.getValue("echo").getValue(name))

@Composable fun EchoTheme(tokens: Map<String, Long>, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalTokens provides tokens) {
        val scheme = darkColorScheme(primary = token("accent"), onPrimary = token("on-accent"), background = token("bg"), onBackground = token("ink"), surface = token("bg-raised"), onSurface = token("ink"), outline = token("line-strong"), error = token("danger"))
        MaterialTheme(colorScheme = scheme, typography = Typography(bodyLarge = TextStyle(fontFamily = Mono, fontSize = 15.sp, lineHeight = 22.sp)), content = content)
    }
}

/**
 * The theme every EchoConnect window uses: Match laptop (its Omarchy tokens, sent over the
 * connection) or Phone (Echo in dark mode, Catppuccin Latte in light). Also tints the system bars.
 */
@Composable fun AppTheme(content: @Composable () -> Unit) {
    val ctx = LocalContext.current
    val state by Connect.state.collectAsStateWithLifecycle()
    val dark = isSystemInDarkTheme()
    var revision by remember { mutableIntStateOf(0) }
    DisposableEffect(Unit) {
        val prefs = ctx.getSharedPreferences("echoconnect", android.content.Context.MODE_PRIVATE)
        val listener = android.content.SharedPreferences.OnSharedPreferenceChangeListener { _, key -> if (key?.startsWith("theme.") == true) revision++ }
        prefs.registerOnSharedPreferenceChangeListener(listener)
        onDispose { prefs.unregisterOnSharedPreferenceChangeListener(listener) }
    }
    val tokens = remember(revision, state.theme, dark) {
        val choice = Prefs.themeMode(ctx)
        val phone = THEMES.getValue(if (dark) "echo" else "catppuccin-latte")
        when {
            choice == "laptop" && state.theme != null -> THEMES.getValue("echo") + state.theme!!
            choice in THEMES -> THEMES.getValue(choice)
            else -> phone
        }
    }
    val view = LocalView.current
    if (!view.isInEditMode) SideEffect {
        var c = view.context
        while (c is ContextWrapper && c !is Activity) c = c.baseContext
        (c as? Activity)?.window?.let { window ->
            val light = android.graphics.Color.luminance((tokens["bg"] ?: 0xFF0B1030).toInt()) > 0.5f
            androidx.core.view.WindowCompat.getInsetsController(window, window.decorView).apply { isAppearanceLightStatusBars = light; isAppearanceLightNavigationBars = light }
            window.isNavigationBarContrastEnforced = false
        }
    }
    EchoTheme(tokens, content)
}

// ------------------------------------------------------------------ type

@Composable fun Txt(text: String, muted: Boolean = false, size: Int = 15, bold: Boolean = false, modifier: Modifier = Modifier, color: Color? = null, line: Int = size + 7, align: TextAlign? = null, maxLines: Int = Int.MAX_VALUE) {
    Text(text, modifier, color = color ?: token(if (muted) "ink-muted" else "ink"), fontFamily = Mono, fontSize = size.sp, lineHeight = line.sp, fontWeight = if (bold) FontWeight.Bold else FontWeight.Normal, textAlign = align, maxLines = maxLines, overflow = TextOverflow.Ellipsis)
}
/** Muted 14/21 paragraph that opens a screen or step. */
@Composable fun Lede(text: AnnotatedString, modifier: Modifier = Modifier, align: TextAlign? = null) {
    Text(text, modifier, color = token("ink-muted"), fontFamily = Mono, fontSize = 14.sp, lineHeight = 21.sp, textAlign = align)
}
@Composable fun Lede(text: String, modifier: Modifier = Modifier, align: TextAlign? = null) = Lede(AnnotatedString(text), modifier, align)
/** 28/34, once per screen at most. */
@Composable fun Display(text: String, modifier: Modifier = Modifier, align: TextAlign? = null) {
    Text(text, modifier, color = token("ink-strong"), fontFamily = Mono, fontSize = 28.sp, lineHeight = 34.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.56).sp, textAlign = align)
}
/** Plain text with **strong** spans in `ink-strong`. */
@Composable fun strong(text: String): AnnotatedString {
    val ink = token("ink-strong")
    return buildAnnotatedString {
        text.split("**").forEachIndexed { i, part -> if (i % 2 == 1) withStyle(SpanStyle(color = ink, fontWeight = FontWeight.Bold)) { append(part) } else append(part) }
    }
}

// ------------------------------------------------------------------ glyphs and icons

@Composable fun Glyph(name: String, description: String? = null, color: Color = token("ink-muted"), size: Int = 20, stroke: Float = 2f) {
    val paths = remember(name) { GLYPHS[name].orEmpty().map { PathParser().parsePathString(it).toPath() } }
    Canvas(Modifier.size(size.dp).then(if (description != null) Modifier.semantics { contentDescription = description } else Modifier.clearAndSetSemantics { })) {
        val s = this.size.width / 24f
        withTransform({ scale(s, s, Offset.Zero) }) { paths.forEach { drawPath(it, color, style = Stroke(stroke, cap = StrokeCap.Round, join = StrokeJoin.Round)) } }
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

/** EchoFiles' 48-unit colour icon for a file, recoloured by the theme's `icon-*` slots. */
@Composable fun FileIcon(fileName: String, size: Int = 36) {
    val parts = COLOR_ICONS[iconFor(fileName)] ?: COLOR_ICONS.getValue("file")
    val tokens = LocalTokens.current
    val paths = remember(parts) { parts.map { PathParser().parsePathString(it.d).toPath() } }
    Canvas(Modifier.size(size.dp).clearAndSetSemantics { }) {
        val s = this.size.width / 48f
        parts.forEachIndexed { i, p ->
            withTransform({ scale(s, s, Offset.Zero); translate(p.tx, p.ty); scale(p.scale, p.scale, Offset.Zero) }) {
                p.fill?.let { drawPath(paths[i], tokens.color(it), alpha = p.alpha) }
                p.stroke?.let { drawPath(paths[i], tokens.color(it), alpha = p.alpha, style = Stroke(p.width, cap = StrokeCap.Round, join = StrokeJoin.Round)) }
            }
        }
    }
}

// ------------------------------------------------------------------ interaction

/** Click with the flat `state-press` layer (no ripple), drawn under the content. */
@Composable fun Modifier.tap(enabled: Boolean = true, role: Role? = Role.Button, label: String? = null, radius: Dp = 0.dp, onClick: () -> Unit): Modifier {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val press = token("state-press")
    return clickable(source, null, enabled, label, role, onClick).drawBehind { if (pressed) drawRoundRect(press, cornerRadius = CornerRadius(radius.toPx())) }
}

/** Disabled things keep their label at `opacity-disabled`. */
fun Modifier.disabled(off: Boolean) = if (off) alpha(0.45f) else this

// ------------------------------------------------------------------ controls

enum class Variant { Default, Primary, Danger, Ghost }
enum class BtnSize(val height: Int, val text: Int, val pad: Int, val icon: Int) { Sm(32, 13, 12, 16), Md(40, 14, 16, 18), Lg(48, 15, 16, 18) }

/** A button whose label says exactly what happens. The hit area is never under 48dp. */
@Composable fun Button(label: String, variant: Variant = Variant.Default, size: BtnSize = BtnSize.Md, icon: String? = null, block: Boolean = false, enabled: Boolean = true, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val (bg, edge, ink) = when (variant) {
        Variant.Primary -> Triple(token("accent"), token("accent"), token("on-accent"))
        Variant.Danger -> Triple(token("danger"), token("danger"), token("on-danger"))
        Variant.Ghost -> Triple(Color.Transparent, Color.Transparent, token("accent-ink"))
        Variant.Default -> Triple(Color.Transparent, token("line-strong"), token("ink"))
    }
    val press = token("state-press")
    val shape = RoundedCornerShape(4.dp)
    Box(modifier.then(if (block) Modifier.fillMaxWidth() else Modifier).heightIn(min = 48.dp).disabled(!enabled)
        .clickable(source, null, enabled, role = Role.Button, onClick = onClick), contentAlignment = Alignment.Center) {
        Row(Modifier.then(if (block) Modifier.fillMaxWidth() else Modifier).height(size.height.dp).background(bg, shape).border(1.dp, edge, shape)
            .drawBehind { if (pressed) drawRoundRect(press, cornerRadius = CornerRadius(4.dp.toPx())) }.padding(horizontal = size.pad.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally), verticalAlignment = Alignment.CenterVertically) {
            if (icon != null) Glyph(icon, color = ink, size = size.icon)
            Text(label, color = ink, fontFamily = Mono, fontSize = size.text.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** A 48dp icon-only button; TalkBack reads [label]. */
@Composable fun IconButton(icon: String, label: String, size: Int = 22, pressed: Boolean = false, enabled: Boolean = true, onClick: () -> Unit) {
    Box(Modifier.size(48.dp).disabled(!enabled).background(if (pressed) token("state-active") else Color.Transparent, RoundedCornerShape(4.dp))
        .tap(enabled, label = label, radius = 4.dp, onClick = onClick).semantics { contentDescription = label }, contentAlignment = Alignment.Center) {
        Glyph(icon, color = token(if (pressed) "accent-ink" else "ink-muted"), size = size)
    }
}

/** On/off that applies at once: 44 × 24, accent when on, knob slides in `dur-base`. */
@Composable fun Switch(checked: Boolean, label: String, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    val knob by animateDpAsState(if (checked) 23.dp else 3.dp, tween(140), label = "knob")
    val track = token(if (checked) "accent" else "bg-deep")
    val edge = token(if (checked) "accent" else "line-strong")
    val dot = token(if (checked) "on-accent" else "ink-muted")
    Box(Modifier.size(48.dp).disabled(!enabled).toggleable(checked, remember { MutableInteractionSource() }, null, enabled, Role.Switch, onChange).semantics { contentDescription = label }, contentAlignment = Alignment.Center) {
        Box(Modifier.size(44.dp, 24.dp).background(track, CircleShape).border(1.dp, edge, CircleShape)) {
            Box(Modifier.offset(x = knob - 1.dp, y = 3.dp).size(16.dp).background(dot, CircleShape))
        }
    }
}

/** Two to four exclusive options in one 40dp well. Selected lifts to `bg-raised` with `accent-ink`. */
@Composable fun Segmented(options: List<Pair<String, String>>, value: String, label: String, onChange: (String) -> Unit) {
    Row(Modifier.fillMaxWidth().height(40.dp).background(token("bg-deep"), RoundedCornerShape(4.dp)).border(1.dp, token("line-strong"), RoundedCornerShape(4.dp)).padding(3.dp)
        .semantics { contentDescription = label }, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
        options.forEach { (key, text) ->
            val on = key == value
            Box(Modifier.weight(1f).fillMaxHeight().then(if (on) Modifier.background(token("bg-raised"), RoundedCornerShape(2.dp)).border(1.dp, token("line"), RoundedCornerShape(2.dp)) else Modifier)
                .selectable(on, role = Role.RadioButton) { onChange(key) }, contentAlignment = Alignment.Center) {
                Txt(text, size = 13, line = 18, color = token(if (on) "accent-ink" else "ink-muted"), bold = on, maxLines = 1)
            }
        }
    }
}

/** A labelled 48dp field on `bg-deep`; errors name the problem and the fix. */
@Composable fun TextField(value: String, label: String, placeholder: String = "", hint: String? = null, error: String? = null, icon: String? = null, keyboard: KeyboardOptions = KeyboardOptions.Default, onChange: (String) -> Unit) {
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val edge = token(when { error != null -> "danger"; focused -> "focus-ring"; else -> "line-strong" })
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Txt(label, muted = true, size = 13, line = 18)
        BasicTextField(value, onChange, Modifier.fillMaxWidth().semantics { contentDescription = label }, singleLine = true, interactionSource = source, keyboardOptions = keyboard,
            textStyle = TextStyle(fontFamily = Mono, fontSize = 15.sp, color = token("ink")), cursorBrush = SolidColor(token("accent")),
            decorationBox = { inner ->
                Row(Modifier.fillMaxWidth().height(48.dp).background(token("bg-deep"), RoundedCornerShape(4.dp)).border(if (focused || error != null) 2.dp else 1.dp, edge, RoundedCornerShape(4.dp)).padding(horizontal = 12.dp),
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (icon != null) Glyph(icon, size = 18)
                    Box(Modifier.weight(1f)) { if (value.isEmpty()) Txt(placeholder, muted = true); inner() }
                }
            })
        if (error != null) Txt(error, size = 13, line = 18, color = token("danger-ink")) else if (hint != null) Txt(hint, muted = true, size = 13, line = 18)
    }
}

// ------------------------------------------------------------------ feedback

/** A state in a word with a 6dp square mark (or an icon). Never colour alone. */
@Composable fun StatePill(text: String, tone: String? = null, icon: String? = null, small: Boolean = false) {
    val ink = token(if (tone == null) "ink-muted" else "$tone-ink")
    val ground = token(if (tone == null) "state-hover" else "$tone-soft")
    Row(Modifier.height(if (small) 18.dp else 22.dp).background(ground, RoundedCornerShape(2.dp)).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        if (icon != null) Glyph(icon, color = ink, size = 13, stroke = 2.4f) else Box(Modifier.size(6.dp).background(ink))
        Text(text, color = ink, fontFamily = Mono, fontSize = if (small) 11.sp else 12.sp, lineHeight = 16.sp, maxLines = 1)
    }
}

/** A 4dp bar from real bytes; `success` when done, a sweep when the size isn't known. */
@Composable fun ProgressBar(fraction: Float?, done: Boolean = false, modifier: Modifier = Modifier) {
    val fill = token(if (done) "success" else "accent")
    val sweep = if (fraction == null) rememberInfiniteTransition(label = "sweep").animateFloat(-0.3f, 1f, infiniteRepeatable(tween(1400, easing = FastOutSlowInEasing)), label = "x").value else 0f
    val width by animateFloatAsState(fraction ?: 0f, tween(140, easing = LinearEasing), label = "progress")
    Box(modifier.fillMaxWidth().padding(top = 6.dp).height(4.dp).clip(RoundedCornerShape(2.dp)).background(token("line")).drawBehind {
        if (fraction == null) drawRect(fill, Offset(size.width * sweep, 0f), Size(size.width * .3f, size.height))
        else drawRect(fill, size = Size(size.width * width.coerceIn(0f, 1f), size.height))
    }.semantics { if (fraction != null) progressBarRangeInfo = ProgressBarRangeInfo(fraction, 0f..1f) })
}

@Composable fun Spinner(size: Int = 16) {
    CircularProgressIndicator(Modifier.size(size.dp).semantics { contentDescription = "Working" }, color = token("accent"), trackColor = token("line"), strokeWidth = 2.dp)
}

/** A colour block that explains something, with an optional action. */
@Composable fun Banner(title: String?, body: String, tone: String = "info", icon: String? = null, action: (@Composable () -> Unit)? = null) {
    val ink = token("ink-strong")
    Row(Modifier.fillMaxWidth().background(token("$tone-soft"), RoundedCornerShape(4.dp)).padding(12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.padding(top = 1.dp)) { Glyph(icon ?: mapOf("warning" to "alert", "danger" to "error", "success" to "check").getOrDefault(tone, "info"), color = token("$tone-ink"), size = 18) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(buildAnnotatedString { if (title != null) { withStyle(SpanStyle(color = ink, fontWeight = FontWeight.Bold)) { append(title) }; append(" ") }; append(body) },
                color = token("ink"), fontFamily = Mono, fontSize = 13.sp, lineHeight = 19.sp)
            action?.invoke()
        }
    }
}

data class Snack(val text: String, val tone: String? = null, val icon: String? = null, val action: String? = null, val id: Long = System.nanoTime(), val onAction: (() -> Unit)? = null)

/** A short result with at most one action: the 2dp accent edge EchoFiles' toasts use. Leaves after 4 s, or 8 with an action. */
@Composable fun SnackbarHost(snack: Snack?, onDone: () -> Unit) {
    if (snack == null) return
    LaunchedEffect(snack.id) { kotlinx.coroutines.delay(if (snack.action != null) 8000 else 4000); onDone() }
    val rise = remember(snack.id) { Animatable(12f) }
    LaunchedEffect(snack.id) { rise.animateTo(0f, spring(dampingRatio = 0.6f, stiffness = Spring.StiffnessMediumLow)) }
    val edge = token(snack.tone ?: "accent")
    Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp).offset(y = rise.value.dp).fillMaxWidth().heightIn(min = 48.dp)
        .shadow(12.dp, RectangleShape).background(token("bg-raised")).border(2.dp, edge).padding(start = 12.dp, end = 6.dp, top = 6.dp, bottom = 6.dp)
        .semantics { liveRegion = LiveRegionMode.Polite }, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        snack.icon?.let { Glyph(it, color = token(if (snack.tone != null) "${snack.tone}-ink" else "accent-ink"), size = 18) }
        Txt(snack.text, size = 14, line = 20, color = token("ink-strong"), modifier = Modifier.weight(1f))
        if (snack.action != null) Box(Modifier.height(40.dp).tap(radius = 4.dp) { snack.onAction?.invoke(); onDone() }.padding(horizontal = 12.dp), contentAlignment = Alignment.Center) {
            Txt(snack.action, size = 14, bold = true, color = token("accent-ink"))
        }
    }
}

// ------------------------------------------------------------------ structure

data class BarAction(val icon: String, val label: String, val enabled: Boolean = true, val onClick: () -> Unit)

/** The 56dp top bar: Back or nothing, the title, up to two icon buttons. */
@Composable fun AppBar(title: String, back: (() -> Unit)? = null, sub: String? = null, actions: List<BarAction> = emptyList()) {
    Row(Modifier.fillMaxWidth().height(56.dp).background(token("bg")).hairline(bottom = true).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        if (back != null) IconButton("arrow-left", "Back", onClick = back)
        Column(Modifier.weight(1f).padding(start = if (back == null) 12.dp else 0.dp).semantics { heading() }) {
            Text(title, color = token("ink-strong"), fontFamily = Mono, fontSize = 20.sp, lineHeight = 26.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.2).sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (sub != null) Txt(sub, muted = true, size = 12, line = 16)
        }
        actions.forEach { IconButton(it.icon, it.label, enabled = it.enabled, onClick = it.onClick) }
    }
}

/** A 1dp `line` hairline on the top or bottom edge. */
@Composable fun Modifier.hairline(bottom: Boolean = false): Modifier {
    val line = token("line")
    return drawBehind { val y = if (bottom) size.height - 1.dp.toPx() else 0f; drawRect(line, Offset(0f, y), Size(size.width, 1.dp.toPx())) }
}

val NAV = listOf("home" to "Home", "clipboard" to "Clipboard", "swap" to "Transfers", "settings" to "Settings")

/** Four destinations on a 64dp `bg-deep` bar; the current one gets `bg`, a 2dp accent top edge and `accent-ink`. */
@Composable fun BottomNav(active: Int, badges: Map<Int, Int> = emptyMap(), onChange: (Int) -> Unit) {
    Row(Modifier.fillMaxWidth().height(64.dp).background(token("bg-deep")).hairline()) {
        NAV.forEachIndexed { i, (icon, title) ->
            val on = i == active
            val ink = token(if (on) "accent-ink" else "ink-muted")
            val accent = token("accent")
            Column(Modifier.weight(1f).fillMaxHeight().background(if (on) token("bg") else Color.Transparent)
                .drawBehind { if (on) drawRect(accent, size = Size(size.width, 2.dp.toPx())) }
                .selectable(on, role = Role.Tab) { onChange(i) }, horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(2.dp, Alignment.CenterVertically)) {
                Box {
                    Glyph(icon, color = ink, size = 22)
                    badges[i]?.takeIf { it > 0 }?.let { n ->
                        Box(Modifier.align(Alignment.TopEnd).offset(x = 10.dp, y = (-4).dp).heightIn(min = 16.dp).widthIn(min = 16.dp).background(token("accent"), CircleShape).padding(horizontal = 4.dp), contentAlignment = Alignment.Center) {
                            Text("$n", color = token("on-accent"), fontFamily = Mono, fontSize = 10.sp, lineHeight = 16.sp, fontWeight = FontWeight.Bold)
                        }
                    }
                }
                Text(title, color = ink, fontFamily = Mono, fontSize = 12.sp, lineHeight = 16.sp, fontWeight = if (on) FontWeight.Bold else FontWeight.Normal)
            }
        }
    }
}

/** Uppercase label header with an optional action link ("See all"). */
@Composable fun SectionHeader(title: String, action: String? = null, onAction: (() -> Unit)? = null) {
    Row(Modifier.fillMaxWidth().heightIn(min = 24.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title.uppercase(), Modifier.weight(1f).semantics { heading() }, color = token("ink-muted"), fontFamily = Mono, fontSize = 11.sp, lineHeight = 16.sp, fontWeight = FontWeight.Bold, letterSpacing = 0.88.sp)
        if (action != null && onAction != null) Box(Modifier.heightIn(min = 32.dp).tap(radius = 4.dp, onClick = onAction).padding(horizontal = 4.dp), contentAlignment = Alignment.Center) {
            Txt(action, size = 13, bold = true, color = token("accent-ink"))
        }
    }
}

/** A titled `bg-raised` box of rows with hairlines between them, and an optional muted foot. */
@Composable fun ListGroup(title: String? = null, action: String? = null, onAction: (() -> Unit)? = null, foot: String? = null, edge: Color? = null, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (title != null) SectionHeader(title, action, onAction)
        Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(4.dp)).background(token("bg-raised")).border(1.dp, edge ?: token("line"), RoundedCornerShape(4.dp))) { Divided(content) }
        if (foot != null) Txt(foot, muted = true, size = 12, line = 17)
    }
}

/** A column that puts a 1dp `line` between its children. */
@Composable fun Divided(content: @Composable () -> Unit) {
    val line = token("line")
    val cuts = remember { IntArray(64) }
    var count by remember { mutableIntStateOf(0) }
    Layout(content, Modifier.drawWithContent {
        drawContent()
        for (i in 0 until minOf(count, cuts.size)) drawRect(line, Offset(0f, cuts[i].toFloat()), Size(size.width, 1.dp.toPx()))
    }) { measurables, constraints ->
        val gap = 1.dp.roundToPx()
        val items = measurables.map { it.measure(constraints.copy(minHeight = 0)) }
        val height = items.sumOf { it.height } + gap * (items.size - 1).coerceAtLeast(0)
        layout(constraints.maxWidth, height.coerceIn(constraints.minHeight, constraints.maxHeight)) {
            var y = 0
            items.forEachIndexed { i, p ->
                if (i > 0) { if (i - 1 < cuts.size) cuts[i - 1] = y; y += gap }
                p.place(0, y); y += p.height
            }
            count = (items.size - 1).coerceAtLeast(0)
        }
    }
}

/** One row: lead, title, up to two lines of subtitle, trailing control or chevron. 56dp, 72 with a subtitle. */
@Composable fun ListRow(
    title: String, sub: String? = null, icon: String? = null, ok: Boolean = false, lead: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null, chevron: Boolean = false, subColor: Color? = null, disabled: Boolean = false,
    below: (@Composable ColumnScope.() -> Unit)? = null, onClick: (() -> Unit)? = null,
) {
    Row(Modifier.fillMaxWidth().heightIn(min = if (sub != null) 72.dp else 56.dp).disabled(disabled).then(if (onClick != null) Modifier.tap(!disabled, onClick = onClick) else Modifier)
        .padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        if (icon != null) Box(Modifier.size(36.dp).background(token(if (ok) "success-soft" else "state-hover"), RoundedCornerShape(4.dp)), contentAlignment = Alignment.Center) {
            Glyph(icon, color = token(if (ok) "success-ink" else "ink-muted"))
        } else lead?.invoke()
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Txt(title, color = token("ink-strong"), maxLines = 1)
            if (sub != null) Txt(sub, size = 13, line = 18, color = subColor ?: token("ink-muted"), maxLines = 2)
            below?.invoke(this)
        }
        if (trailing != null) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) { trailing() }
        if (chevron) Glyph("chevron-right", color = token("ink-faint"), size = 18)
    }
}

/** A decision that blocks: square, 2dp accent (or danger) border, buttons on the right that name the outcome. */
@Composable fun EchoDialog(title: String, onDismiss: () -> Unit, danger: Boolean = false, body: String? = null, content: (@Composable ColumnScope.() -> Unit)? = null, actions: @Composable RowScope.() -> Unit) {
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        val scale = remember { Animatable(0.98f) }
        LaunchedEffect(Unit) { scale.animateTo(1f, tween(220)) }
        Column(Modifier.padding(24.dp).widthIn(max = 360.dp).fillMaxWidth().scale(scale.value).shadow(12.dp, RectangleShape).background(token("bg-raised"))
            .border(2.dp, token(if (danger) "danger" else "accent")).padding(start = 24.dp, end = 24.dp, top = 24.dp, bottom = 8.dp)) {
            Text(title, Modifier.padding(bottom = 8.dp).semantics { heading() }, color = token("ink-strong"), fontFamily = Mono, fontSize = 17.sp, lineHeight = 24.sp, fontWeight = FontWeight.Bold)
            if (body != null) Txt(body, muted = true, size = 14, line = 21, modifier = Modifier.padding(bottom = 8.dp))
            content?.invoke(this)
            Row(Modifier.fillMaxWidth().offset(x = 8.dp).padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.End), content = actions)
        }
    }
}

/** A task over the screen: scrim, square `bg-raised` sheet with a 2dp accent top edge, rising 24dp. */
@Composable fun BottomSheet(title: String, icon: String? = null, onDismiss: () -> Unit, footer: @Composable RowScope.() -> Unit, content: @Composable ColumnScope.() -> Unit) {
    BackHandler(onBack = onDismiss)
    val shown = remember { Animatable(0f) }
    LaunchedEffect(Unit) { shown.animateTo(1f, tween(220)) }
    val accent = token("accent")
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().alpha(shown.value).background(token("scrim")).clickable(remember { MutableInteractionSource() }, null, onClickLabel = "Cancel", onClick = onDismiss))
        Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth().fillMaxHeight(0.85f).wrapContentHeight(Alignment.Bottom).offset(y = (24 * (1 - shown.value)).dp)
            .shadow(12.dp, RectangleShape).background(token("bg-raised")).drawBehind { drawRect(accent, size = Size(size.width, 2.dp.toPx())) }.clickable(remember { MutableInteractionSource() }, null) {}) {
            Box(Modifier.padding(top = 8.dp).size(32.dp, 4.dp).background(token("line-strong"), CircleShape).align(Alignment.CenterHorizontally))
            Row(Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (icon != null) Glyph(icon, color = token("accent-ink"))
                Text(title, Modifier.semantics { heading() }, color = token("ink-strong"), fontFamily = Mono, fontSize = 17.sp, lineHeight = 24.sp, fontWeight = FontWeight.Bold)
            }
            Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(start = 6.dp, end = 6.dp, bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp), content = content)
            Row(Modifier.fillMaxWidth().background(token("bg")).hairline().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)).padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically, content = footer)
        }
    }
}

// ------------------------------------------------------------------ the laptop

/** The laptop's battery: `success` when charging (with a bolt), `warning` at 20% and below, `danger` at 10%. */
@Composable fun BatteryMeter(level: Int, charging: Boolean, label: Boolean = true) {
    val lv = level.coerceIn(0, 100)
    val ink = token("ink-muted")
    val fill = when { charging -> token("success"); lv <= 10 -> token("danger"); lv <= 20 -> token("warning"); else -> ink }
    Row(Modifier.semantics(mergeDescendants = true) { contentDescription = "Laptop battery $lv%" + if (charging) ", charging" else "" }, verticalAlignment = Alignment.CenterVertically) {
        Canvas(Modifier.size(25.dp, 11.dp)) {
            val w = 22.dp.toPx(); val r = 3.dp.toPx(); val px = 1.dp.toPx()
            drawRoundRect(ink, size = Size(w, size.height), cornerRadius = CornerRadius(r), style = Stroke(px))
            drawRoundRect(fill, Offset(2 * px, 2 * px), Size((w - 4 * px) * lv / 100f, size.height - 4 * px), CornerRadius(px))
            drawRect(ink, Offset(w, size.height / 2 - 1.5f * px), Size(2 * px, 3 * px))
        }
        if (charging) Glyph("bolt", color = token("success-ink"), size = 12)
        if (label) Txt("$lv%", muted = true, size = 12, line = 16, modifier = Modifier.padding(start = 5.dp))
    }
}

/** How the phone reaches the laptop now: Wi-Fi (everything) and Bluetooth (calls and clipboard). */
@OptIn(ExperimentalLayoutApi::class)
@Composable fun LinkPills(wifi: Boolean, bluetooth: Boolean) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        LinkPill("wifi", if (wifi) "Wi-Fi" else "No Wi-Fi", wifi)
        LinkPill("bluetooth", if (bluetooth) "Bluetooth" else "Bluetooth off", bluetooth)
    }
}
@Composable private fun LinkPill(icon: String, text: String, on: Boolean) {
    val edge = token("line-strong")
    Row(Modifier.height(26.dp).drawBehind {
        val px = 1.dp.toPx()
        drawRoundRect(edge, Offset(px / 2, px / 2), Size(size.width - px, size.height - px), CornerRadius(2.dp.toPx()), style = Stroke(px, pathEffect = if (on) null else PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 3.dp.toPx()))))
    }.padding(horizontal = 10.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Glyph(icon, color = token(if (on) "accent-ink" else "ink-faint"), size = 14)
        Txt(text, size = 12, line = 16, color = token(if (on) "ink" else "ink-muted"))
    }
}

/** The 6dp `world-linux` square: the laptop's colour in EchoFiles' sidebar. */
@Composable fun WorldMark(world: String = "linux") = Box(Modifier.size(6.dp).background(token("world-$world")))

/** A call running through the laptop's mic and speakers: who, how long, and Use phone. */
@Composable fun CallBar(who: String, time: String, onLaptop: Boolean, onUsePhone: () -> Unit) {
    Row(Modifier.fillMaxWidth().background(token("success-soft"), RoundedCornerShape(4.dp)).padding(start = 12.dp, end = 8.dp, top = 8.dp, bottom = 8.dp).semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.size(36.dp).background(token("success"), CircleShape), contentAlignment = Alignment.Center) { Glyph("call", color = token("on-success"), size = 18) }
        Column(Modifier.weight(1f)) {
            Txt("$who · $time", bold = true, color = token("ink-strong"), line = 21, maxLines = 1)
            Txt(if (onLaptop) "On laptop mic and speakers · Bluetooth" else "On this phone", muted = true, size = 12, line = 17)
        }
        if (onLaptop) Button("Use phone", size = BtnSize.Sm, onClick = onUsePhone)
    }
}

// ------------------------------------------------------------------ clipboard

/** One thing that crossed the clipboard: direction, text (or image), where and when, Copy again. */
@Composable fun ClipItem(c: Clip, onCopy: (() -> Unit)?) {
    val when_ = relativeTime(c.at)
    Row(Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(start = 12.dp, end = 6.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.size(28.dp).background(token("state-hover"), RoundedCornerShape(4.dp)).semantics { contentDescription = if (c.toLaptop) "Phone to laptop" else "Laptop to phone" }, contentAlignment = Alignment.Center) {
            Glyph(if (c.toLaptop) "arrow-up" else "arrow-down", color = token(if (c.toLaptop) "world-linux" else "world-phone"), size = 16)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            when {
                c.sensitive -> Text("•••• •••• ••••", color = token("ink-muted"), fontFamily = Mono, fontSize = 14.sp, lineHeight = 20.sp, letterSpacing = 1.1.sp, maxLines = 1)
                c.image -> Txt("Image", size = 14, line = 20, color = token("ink-strong"), maxLines = 1)
                else -> Txt(c.text.replace('\n', ' '), size = 14, line = 20, color = token("ink-strong"), maxLines = 1)
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Txt((if (c.toLaptop) "To laptop" else "From laptop") + " · " + when_ + if (c.bluetooth) " · Bluetooth" else "", muted = true, size = 12, line = 16, maxLines = 1, modifier = Modifier.weight(1f, fill = false))
                if (c.sensitive) StatePill("Hidden", icon = "eye-off", small = true)
            }
        }
        if (onCopy != null) IconButton("copy", "Copy again", size = 18, onClick = onCopy)
    }
}

enum class ClipMode { Auto, Paused, Manual }

/** How phone → laptop clipboard works here; the foot says laptop → phone is always automatic. */
@Composable fun ClipModeCard(mode: ClipMode, body: String, action: String?, onAction: () -> Unit) {
    Column(Modifier.fillMaxWidth().background(token("bg-raised"), RoundedCornerShape(4.dp)).border(1.dp, token(if (mode == ClipMode.Paused) "warning" else "line"), RoundedCornerShape(4.dp)).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Glyph("clipboard", color = token("accent-ink"))
            Txt("Phone → laptop", bold = true, color = token("ink-strong"), modifier = Modifier.weight(1f))
            when (mode) { ClipMode.Auto -> StatePill("Automatic", "success"); ClipMode.Paused -> StatePill("Paused", "warning"); ClipMode.Manual -> StatePill("Tap to send") }
        }
        Txt(body, muted = true, size = 13, line = 19)
        if (action != null) Button(action, if (mode == ClipMode.Paused) Variant.Primary else Variant.Default, icon = if (mode == ClipMode.Paused) "refresh" else "bolt", block = true, onClick = onAction)
        Row(Modifier.fillMaxWidth().hairline().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Glyph("check", color = token("success-ink"), size = 14); Txt("Laptop → phone is always automatic.", muted = true, size = 12, line = 16)
        }
    }
}

// ------------------------------------------------------------------ setup

/** One Android permission: what it unlocks in plain words, and Allowed or the button that grants it. */
@Composable fun PermissionRow(icon: String, title: String, why: String, granted: Boolean, action: String = "Allow", optional: Boolean = false, onClick: () -> Unit) {
    ListRow(title, why, icon = icon, ok = granted, trailing = {
        if (granted) StatePill("Allowed", "success") else Button(action, if (optional) Variant.Default else Variant.Primary, BtnSize.Sm, onClick = onClick)
    })
}

class Step(val title: String, val done: String? = null, val body: @Composable ColumnScope.() -> Unit = {})

/** Steps down the screen: done ones collapse to a `success` line, the current one opens, later ones wait. */
@Composable fun StepList(steps: List<Step>, current: Int) {
    val line = token("line"); val success = token("success")
    Column {
        steps.forEachIndexed { i, s ->
            val state = if (i < current) 0 else if (i == current) 1 else 2
            val last = i == steps.lastIndex
            Row(Modifier.fillMaxWidth().drawBehind { if (!last) drawRect(if (state == 0) success else line, Offset(13.dp.toPx(), 30.dp.toPx()), Size(2.dp.toPx(), size.height - 32.dp.toPx())) }
                .padding(bottom = if (last) 0.dp else 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                val (ground, edge, ink) = when (state) { 0 -> Triple("success", "success", "on-success"); 1 -> Triple("accent", "accent", "on-accent"); else -> Triple("bg", "line-strong", "ink-muted") }
                Box(Modifier.size(28.dp).background(token(ground), CircleShape).border(1.dp, token(edge), CircleShape), contentAlignment = Alignment.Center) {
                    if (state == 0) Glyph("check", "Done", token(ink), 13, 3f) else Txt("${i + 1}", size = 13, bold = true, color = token(ink))
                }
                Column(Modifier.weight(1f).padding(top = 3.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Txt(s.title, bold = true, color = token(when (state) { 0 -> "ink"; 1 -> "ink-strong"; else -> "ink-muted" }))
                    if (state == 1) Column(Modifier.padding(top = 4.dp).fillMaxWidth().background(token("bg-raised"), RoundedCornerShape(4.dp)).border(1.dp, token("line"), RoundedCornerShape(4.dp)).padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp), content = s.body)
                    if (state == 0 && s.done != null) Txt(s.done, size = 12, line = 16, color = token("success-ink"))
                }
            }
        }
    }
}

/** Progress through a flow: one bar per step, like the EchoFiles stepper. */
@Composable fun Stepper(current: Int, count: Int, label: String, modifier: Modifier = Modifier) {
    Column(modifier.semantics(mergeDescendants = true) { contentDescription = "Step ${current + 1} of $count, $label" }, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            repeat(count) { i -> Box(Modifier.weight(1f).height(3.dp).background(token(if (i < current) "success" else if (i == current) "accent" else "line"))) }
        }
        Txt(if (current >= count) "Done" else "Step ${current + 1} of $count · $label", muted = true, size = 12, line = 16)
    }
}

/** The four-pair code both screens show while pairing, identical to EchoFiles'. */
@Composable fun PairCode(code: String) {
    val accent = token("accent")
    val pairs = code.chunked(2)
    Row(Modifier.fillMaxWidth().padding(vertical = 8.dp).semantics(mergeDescendants = true) { contentDescription = "Pairing code " + pairs.joinToString(" ") }, horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally)) {
        pairs.forEach { p ->
            Box(Modifier.widthIn(min = 60.dp).background(token("bg-deep"), RoundedCornerShape(4.dp)).border(1.dp, token("line-strong"), RoundedCornerShape(4.dp))
                .drawBehind { drawRect(accent, Offset(0f, size.height - 2.dp.toPx()), Size(size.width, 2.dp.toPx())) }.padding(vertical = 8.dp, horizontal = 6.dp), contentAlignment = Alignment.Center) {
                Text(p, color = token("ink-strong"), fontFamily = Mono, fontSize = 28.sp, lineHeight = 34.sp, fontWeight = FontWeight.Bold, letterSpacing = 1.1.sp)
            }
        }
    }
}

/** A choice card: 2dp accent edge when selected. */
@Composable fun ChoiceCard(icon: String, title: String, body: String, selected: Boolean, pill: (@Composable () -> Unit)? = null, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().background(token("bg-raised"), RoundedCornerShape(4.dp)).border(if (selected) 2.dp else 1.dp, token(if (selected) "accent" else "line-strong"), RoundedCornerShape(4.dp))
        .selectable(selected, role = Role.RadioButton, onClick = onClick).padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Glyph(icon, color = token("accent-ink"), size = 18); Txt(title, bold = true, color = token("ink-strong"), modifier = Modifier.weight(1f)); pill?.invoke()
        }
        Txt(body, muted = true, size = 13, line = 19)
    }
}

/** A line with a success tick. */
@Composable fun Note(text: String, icon: String = "check", tone: String = "success") {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) { Glyph(icon, color = token("$tone-ink"), size = 14); Txt(text, muted = true, size = 13, line = 18) }
}

/** Something EchoConnect is waiting for, with a spinner or a pill. */
@Composable fun Wait(text: String, lead: @Composable () -> Unit = { Spinner() }) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) { lead(); Txt(text, muted = true, size = 13, line = 18) }
}

/** "12 min ago" within a day, then "Yesterday 18:22", then the date. */
fun relativeTime(at: Long, now: Long = System.currentTimeMillis()): String {
    val mins = (now - at) / 60_000
    val time = java.text.SimpleDateFormat("HH:mm", java.util.Locale.getDefault()).format(java.util.Date(at))
    return when {
        mins < 1 -> "now"
        mins < 60 -> "$mins min ago"
        android.text.format.DateUtils.isToday(at) -> time
        android.text.format.DateUtils.isToday(at + 86_400_000) -> "Yesterday $time"
        else -> java.text.SimpleDateFormat("d MMM HH:mm", java.util.Locale.getDefault()).format(java.util.Date(at))
    }
}

/** Base-1024 sizes, as the design system writes them: "4.1 MB". */
fun bytes(n: Long): String {
    if (n < 1024) return "$n B"
    val units = listOf("KB", "MB", "GB", "TB")
    var v = n / 1024.0; var i = 0
    while (v >= 1024 && i < units.lastIndex) { v /= 1024; i++ }
    return if (v >= 100 || i == 0) "${v.toLong()} ${units[i]}" else String.format(java.util.Locale.US, "%.1f %s", v, units[i])
}

