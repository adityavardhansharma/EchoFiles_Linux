package app.echoconnect.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.*
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/*
 * EchoConnect's illustrations: flat Omarchy drawings built from theme tokens, so every theme
 * recolours them. No gradients, glows or shadows: grounds, hairlines, the accent edge and the
 * EchoFiles colour icons. Each drawing has its own unit box (the laptop is 320 × 196).
 */

private val pathCache = HashMap<String, Path>()
private fun parse(d: String) = pathCache.getOrPut(d) { PathParser().parsePathString(d).toPath() }

/** Draws in a unit box at ([ox], [oy]) scaled by [k]. */
internal class Art(val d: DrawScope, val t: Map<String, Long>, val ox: Float, val oy: Float, val k: Float, val measurer: TextMeasurer?) {
    fun c(token: String) = t.color(token)
    private fun o(x: Float, y: Float) = Offset(ox + x * k, oy + y * k)
    fun sub(x: Float, y: Float, scale: Float) = Art(d, t, ox + x * k, oy + y * k, k * scale, measurer)

    fun rect(x: Float, y: Float, w: Float, h: Float, color: Color, r: Float = 0f, a: Float = 1f) =
        d.drawRoundRect(color, o(x, y), Size(w * k, h * k), CornerRadius(r * k), alpha = a)
    fun rect(x: Float, y: Float, w: Float, h: Float, token: String, r: Float = 0f, a: Float = 1f) = rect(x, y, w, h, c(token), r, a)
    fun edge(x: Float, y: Float, w: Float, h: Float, token: String, width: Float, r: Float = 0f, dashed: Boolean = false) =
        d.drawRoundRect(c(token), o(x + width / 2, y + width / 2), Size((w - width) * k, (h - width) * k), CornerRadius(r * k),
            style = Stroke(width * k, pathEffect = if (dashed) PathEffect.dashPathEffect(floatArrayOf(3 * k, 2.5f * k)) else null))
    fun circle(cx: Float, cy: Float, r: Float, color: Color, a: Float = 1f) = d.drawCircle(color, r * k, o(cx, cy), alpha = a)
    fun circle(cx: Float, cy: Float, r: Float, token: String, a: Float = 1f) = circle(cx, cy, r, c(token), a)
    fun ring(cx: Float, cy: Float, r: Float, token: String, width: Float) = d.drawCircle(c(token), r * k, o(cx, cy), style = Stroke(width * k))
    fun line(x1: Float, y1: Float, x2: Float, y2: Float, token: String, width: Float, dashed: Boolean = false) =
        d.drawLine(c(token), o(x1, y1), o(x2, y2), width * k, StrokeCap.Round, if (dashed) PathEffect.dashPathEffect(floatArrayOf(4 * k, 4 * k)) else null)
    /** An SVG path in this box, filled (or stroked with [stroke] > 0). */
    fun path(svg: String, color: Color, stroke: Float = 0f, a: Float = 1f) = d.withTransform({ translate(ox, oy); scale(k, k, Offset.Zero) }) {
        if (stroke > 0) drawPath(parse(svg), color, a, Stroke(stroke, cap = StrokeCap.Round, join = StrokeJoin.Round)) else drawPath(parse(svg), color, a)
    }
    fun path(svg: String, token: String, stroke: Float = 0f, a: Float = 1f) = path(svg, c(token), stroke, a)
    /** One of the 24-unit glyphs, [size] units wide. */
    fun glyph(name: String, x: Float, y: Float, size: Float, token: String, width: Float = 2f) = d.withTransform({ translate(ox + x * k, oy + y * k); scale(k * size / 24f, k * size / 24f, Offset.Zero) }) {
        GLYPHS[name].orEmpty().forEach { drawPath(parse(it), c(token), style = Stroke(width, cap = StrokeCap.Round, join = StrokeJoin.Round)) }
    }
    /** One of EchoFiles' 48-unit colour icons, [size] units wide. */
    fun icon(name: String, x: Float, y: Float, size: Float) {
        val parts = COLOR_ICONS[name] ?: COLOR_ICONS.getValue("file")
        d.withTransform({ translate(ox + x * k, oy + y * k); scale(k * size / 48f, k * size / 48f, Offset.Zero) }) {
            parts.forEach { p ->
                withTransform({ translate(p.tx, p.ty); scale(p.scale, p.scale, Offset.Zero) }) {
                    p.fill?.let { drawPath(parse(p.d), c(it), p.alpha) }
                    p.stroke?.let { drawPath(parse(p.d), c(it), p.alpha, Stroke(p.width, cap = StrokeCap.Round, join = StrokeJoin.Round)) }
                }
            }
        }
    }
    fun text(s: String, cx: Float, baseline: Float, size: Float, token: String, bold: Boolean = false) {
        val m = measurer ?: return
        val layout = m.measure(s, TextStyle(fontFamily = Mono, fontSize = with(d) { (size * k).toSp() }, fontWeight = if (bold) FontWeight.Bold else FontWeight.Normal, color = c(token), textAlign = TextAlign.Center))
        d.drawText(layout, topLeft = Offset(ox + cx * k - layout.size.width / 2f, oy + baseline * k - layout.firstBaseline))
    }
}

enum class LaptopLook { Connected, Away, Locked }

// ------------------------------------------------------------------ the laptop (320 × 196)

internal fun Art.laptop(look: LaptopLook, time: String) {
    // Lid: a lighter rim around the bezel, the camera, then the screen.
    rect(22f, 2f, 276f, 172f, "icon-slate", 11f)
    rect(23.5f, 3.5f, 273f, 169f, "icon-slate-deep", 9.5f)
    circle(160f, 8.6f, 2f, Color(0xFF0A0B10)); circle(160f, 8.6f, .8f, "info", .7f)
    when (look) {
        LaptopLook.Connected -> sub(30f, 14f, 1f).echoFiles()
        LaptopLook.Away -> {
            rect(30f, 14f, 260f, 152f, Color(0xFF07080D), 1.5f)
            path("M30 14H150L76 166H30Z", Color.White, a = .035f)
            path("M182 14H214L140 166H108Z", Color.White, a = .025f)
            glyph("moon", 151f, 81f, 18f, "ink-faint", 1.6f)
        }
        LaptopLook.Locked -> sub(30f, 14f, 1f).lockScreen(time)
    }
    // Base: a deck wider than the lid, its front lip and the opening notch.
    rect(20f, 172.5f, 280f, 3.5f, "icon-slate-deep")
    path("M2 176H318L311.5 188.6A6 6 0 0 1 306.2 191.8H13.8A6 6 0 0 1 8.5 188.6Z", "icon-slate")
    rect(2f, 176f, 316f, 1.4f, "icon-paper-fold", a = .45f)
    rect(132f, 176f, 56f, 3.6f, "icon-slate-deep", 1.8f)
    rect(14f, 191.8f, 292f, 1.6f, "icon-slate-deep", .8f, .6f)
}

/** EchoFiles open on the laptop: tabs, toolbar and path bar, the sidebar with its world marks, a grid of files, and a status bar showing this phone. Box 260 × 152. */
private fun Art.echoFiles() {
    rect(0f, 0f, 260f, 152f, "bg-deep", 1.5f)
    rect(2f, 2f, 256f, 148f, "bg"); edge(2f, 2f, 256f, 148f, "accent", 1.2f)
    // Tabs
    rect(3.2f, 3.2f, 253.6f, 10f, "bg-deep")
    rect(3.2f, 3.2f, 60f, 10f, "bg"); rect(3.2f, 3.2f, 60f, 1.4f, "accent")
    icon("folder", 7f, 4.4f, 7.5f); rect(17f, 7.2f, 30f, 2.4f, "ink-strong", 1f); glyph("close", 53f, 5.6f, 5.4f, "ink-faint", 2.6f)
    icon("folder", 68f, 4.4f, 7.5f); rect(78f, 7.2f, 26f, 2.4f, "ink-faint", 1f)
    glyph("plus", 112f, 5.6f, 5.4f, "ink-faint", 2.6f)
    listOf(234f, 242f, 250f).forEach { circle(it, 8.2f, 1.7f, "ink-faint") }
    // Toolbar: back/forward, the path bar with crumbs, search, view toggles
    glyph("chevron-left", 6f, 15.6f, 8f, "ink-muted", 2.6f); glyph("chevron-right", 15f, 15.6f, 8f, "ink-faint", 2.6f)
    rect(28f, 15.4f, 140f, 8f, "bg-deep", 2f); edge(28f, 15.4f, 140f, 8f, "line-strong", .6f, 2f)
    glyph("home", 31f, 16.4f, 6f, "ink-muted", 2.4f)
    rect(40f, 18.3f, 18f, 2.2f, "ink-muted", 1f); glyph("chevron-right", 59.5f, 16.9f, 5f, "ink-faint", 2.6f)
    rect(66f, 18.3f, 24f, 2.2f, "ink-muted", 1f); glyph("chevron-right", 91.5f, 16.9f, 5f, "ink-faint", 2.6f)
    rect(98f, 18.3f, 28f, 2.2f, "ink-strong", 1f)
    rect(174f, 15.4f, 56f, 8f, "bg-deep", 2f); edge(174f, 15.4f, 56f, 8f, "line-strong", .6f, 2f)
    glyph("search", 176.5f, 16.6f, 5.8f, "ink-faint", 2.6f); rect(185f, 18.3f, 26f, 2.2f, "ink-faint", 1f)
    rect(235f, 15.4f, 9f, 8f, "state-active", 1.5f); glyph("grid", 236.5f, 16.7f, 6f, "accent-ink", 2.4f); glyph("list", 247f, 16.7f, 6f, "ink-faint", 2.4f)
    rect(3.2f, 26f, 253.6f, .7f, "line")
    // Sidebar: places in their worlds (this laptop, Windows, the phone, network), then drives and usage
    rect(3.2f, 26.7f, 56f, 117f, "bg-sunken"); rect(59.2f, 26.7f, .7f, 117f, "line")
    rect(7f, 31f, 20f, 1.8f, "ink-faint", .9f)
    listOf("linux" to 26f, "windows" to 22f, "phone" to 30f, "network" to 18f).forEachIndexed { i, (world, len) ->
        val y = 37f + i * 8.4f
        if (world == "phone") { rect(3.9f, y - 2.6f, 55f, 8f, "selection"); rect(3.9f, y - 2.6f, 1.4f, 8f, "accent") }
        rect(8f, y, 3.2f, 3.2f, "world-$world")
        rect(14f, y + .5f, len, 2.2f, if (world == "phone") "ink-strong" else "ink-muted", 1f)
    }
    rect(7f, 74f, 16f, 1.8f, "ink-faint", .9f)
    listOf(28f, 22f, 25f).forEachIndexed { i, len ->
        val y = 79f + i * 8.4f
        rect(7.5f, y - .3f, 4.4f, 3.8f, if (i == 2) "icon-orange" else "icon-blue", .8f)
        rect(14f, y + .5f, len, 2.2f, "ink-muted", 1f)
    }
    rect(7f, 131f, 46f, 2.4f, "line", 1.2f); rect(7f, 131f, 29f, 2.4f, "accent", 1.2f)
    rect(7f, 136f, 30f, 1.8f, "ink-faint", .9f)
    // Content: the folder title, then a grid of files
    rect(65f, 30.5f, 44f, 3.2f, "ink-strong", 1.6f); rect(65f, 36f, 28f, 1.9f, "ink-faint", .9f)
    glyph("sort", 244f, 30f, 6.5f, "ink-faint", 2.4f)
    val files = listOf("folder", "folder", "folder", "file-image", "file-pdf", "file-sheet", "file-doc", "file-image", "file-video", "file-archive",
        "file-code", "file-audio", "file-slides", "file-text", "file-package")
    files.forEachIndexed { i, name ->
        val col = i % 5; val row = i / 5
        val x = 64f + col * 38.4f; val y = 42f + row * 33f
        if (i == 7) rect(x, y - 1f, 36f, 31f, "selection", 2.5f)
        icon(name, x + 9f, y + 1f, 18f)
        val len = 14f + (i * 7 % 13)
        rect(x + 18f - len / 2f, y + 22.5f, len, 2.1f, if (i == 7) "ink-strong" else "ink-muted", 1f)
        rect(x + 18f - (len - 6f) / 2f, y + 26f, len - 6f, 1.6f, "ink-faint", .8f)
    }
    // Status bar: item count, and the phone connected
    rect(3.2f, 143.7f, 253.6f, 6.1f, "bg-deep")
    rect(7f, 146f, 32f, 1.8f, "ink-faint", .9f)
    rect(196f, 145.4f, 2.8f, 2.8f, "world-phone"); rect(201.5f, 146f, 30f, 1.8f, "success", .9f)
    edge(236f, 145f, 9f, 3.8f, "ink-faint", .5f, .8f); rect(236.8f, 145.8f, 5.6f, 2.2f, "success", .4f); rect(245.4f, 146.1f, .8f, 1.6f, "ink-faint")
}

/** The laptop locked from the phone: time, date, the user and the password field. Box 260 × 152. */
private fun Art.lockScreen(time: String) {
    rect(0f, 0f, 260f, 152f, "bg-deep", 1.5f)
    path("M0 104L118 30L260 92V152H0Z", c("accent"), a = .07f)
    path("M0 152V128L88 84L210 152Z", c("world-linux"), a = .07f)
    text(time, 130f, 50f, 25f, "ink-strong", true)
    text(java.text.SimpleDateFormat("EEEE d MMMM", java.util.Locale.getDefault()).format(java.util.Date()), 130f, 62f, 6.4f, "ink-muted")
    circle(130f, 84f, 10.5f, "accent-soft"); glyph("user", 123f, 77f, 14f, "accent-ink", 2f)
    rect(96f, 101f, 68f, 12f, "bg", 2f); edge(96f, 101f, 68f, 12f, "accent", 1.1f, 2f)
    for (i in 0 until 5) circle(106f + i * 6f, 107f, 1.4f, "ink-muted")
    glyph("arrow-right", 150f, 103.5f, 7f, "accent-ink", 2.6f)
    glyph("lock", 92f, 133f, 7f, "ink-muted", 2.4f)
    text("Locked from your phone", 133f, 139f, 5.6f, "ink-muted")
}

// ------------------------------------------------------------------ this phone (124 × 244)

/** EchoConnect's own Home in miniature, on a flat phone. */
internal fun Art.phone() {
    rect(121.5f, 54f, 2.5f, 24f, "icon-slate-deep", 1f); rect(121.5f, 84f, 2.5f, 14f, "icon-slate-deep", 1f)
    rect(0f, 0f, 122f, 244f, "icon-slate", 19f)
    rect(1.6f, 1.6f, 118.8f, 240.8f, "icon-slate-deep", 17.5f)
    rect(6f, 6f, 110f, 232f, "bg", 13f)
    circle(61f, 13.5f, 2.6f, Color(0xFF0A0B10))
    rect(15f, 12f, 11f, 2.6f, "ink-muted", 1.3f); rect(92f, 12f, 14f, 2.6f, "ink-muted", 1.3f)
    // App bar
    rect(14f, 25f, 46f, 3.8f, "ink-strong", 1.9f); glyph("qr", 99f, 22.5f, 8f, "ink-muted", 2.4f)
    rect(6f, 34.5f, 110f, .7f, "line")
    // Laptop card
    rect(12f, 40f, 98f, 72f, "bg-raised", 3f); edge(12f, 40f, 98f, 72f, "line", .7f, 3f)
    rect(12.7f, 40.7f, 96.6f, 41f, "bg-sunken", 2.4f); rect(12.7f, 81f, 96.6f, .7f, "line")
    sub(25f, 44.5f, 72f / 320f).laptop(LaptopLook.Connected, "")
    rect(17f, 86f, 26f, 5.6f, "success-soft", 1f); rect(19.3f, 87.8f, 2f, 2f, "success-ink"); rect(23f, 88f, 16f, 1.6f, "success-ink", .8f)
    edge(84f, 86.6f, 9f, 4.4f, "ink-muted", .5f, .8f); rect(85f, 87.6f, 5f, 2.4f, "ink-muted", .4f)
    rect(17f, 96f, 2.6f, 2.6f, "world-linux"); rect(22f, 95.6f, 50f, 3.6f, "ink-strong", 1.8f)
    edge(17f, 102f, 22f, 6f, "line-strong", .5f, 1f); edge(42f, 102f, 26f, 6f, "line-strong", .5f, 1f, dashed = true)
    // Quick actions, two by two
    listOf("upload", "clipboard", "external", "lock").forEachIndexed { i, g ->
        val x = 12f + (i % 2) * 50f; val y = 117f + (i / 2) * 31f
        rect(x, y, 48f, 28f, "bg-raised", 3f); edge(x, y, 48f, 28f, "line", .7f, 3f)
        rect(x + 4f, y + 4f, 10f, 10f, "state-hover", 2f); glyph(g, x + 5.5f, y + 5.5f, 7f, "world-linux", 2.6f)
        rect(x + 4f, y + 18f, 30f - (i % 2) * 4f, 2.6f, "ink-strong", 1.3f); rect(x + 4f, y + 22.4f, 24f, 1.8f, "ink-faint", .9f)
    }
    // A clip from the laptop
    rect(12f, 182f, 98f, 22f, "bg-raised", 3f); edge(12f, 182f, 98f, 22f, "line", .7f, 3f)
    rect(16f, 187f, 11f, 11f, "state-hover", 2f); glyph("arrow-down", 17.5f, 188.5f, 8f, "world-phone", 2.6f)
    rect(31f, 188f, 56f, 2.6f, "ink-strong", 1.3f); rect(31f, 193.5f, 34f, 1.8f, "ink-faint", .9f)
    // Bottom nav and gesture bar
    rect(6f, 212f, 110f, 26f, "bg-deep", 13f); rect(6f, 212f, 110f, 12f, "bg-deep"); rect(6f, 212f, 110f, .7f, "line")
    rect(6f, 212.7f, 27.5f, 15f, "bg"); rect(6f, 212f, 27.5f, 1.3f, "accent")
    listOf("home", "clipboard", "swap", "settings").forEachIndexed { i, g -> glyph(g, 15.5f + i * 27.5f, 215.5f, 8.5f, if (i == 0) "accent-ink" else "ink-muted", 2.4f) }
    rect(49f, 232f, 24f, 1.8f, "ink-muted", .9f)
}

// ------------------------------------------------------------------ composables

/** The laptop, drawn flat from icon slots, with EchoFiles open on its screen in live tokens. */
@Composable fun LaptopDevice(look: LaptopLook = LaptopLook.Connected, width: Dp = 236.dp, label: String = "Your laptop") {
    val t = LocalTokens.current
    val measurer = rememberTextMeasurer()
    val time = remember { java.text.SimpleDateFormat("HH:mm", java.util.Locale.getDefault()).format(java.util.Date()) }
    Canvas(Modifier.size(width, width * 196f / 320f).semantics { contentDescription = label + when (look) { LaptopLook.Away -> ", not nearby"; LaptopLook.Locked -> ", locked"; else -> ", EchoFiles open" } }) {
        Art(this, t, 0f, 0f, size.width / 320f, measurer).laptop(look, time)
    }
}

/** This phone with EchoConnect open. */
@Composable fun PhoneDevice(width: Dp = 96.dp) {
    val t = LocalTokens.current
    Canvas(Modifier.size(width, width * 244f / 124f).semantics { contentDescription = "This phone" }) { Art(this, t, 0f, 0f, size.width / 124f, null).phone() }
}

/** The two devices and how they're linked: Wi-Fi for everything, Bluetooth for calls and clipboard. */
@Composable fun PairHero(connected: Boolean, modifier: Modifier = Modifier) {
    val t = LocalTokens.current
    val measurer = rememberTextMeasurer()
    Canvas(modifier.fillMaxWidth().aspectRatio(320f / 176f).semantics { contentDescription = if (connected) "This phone connected to your laptop" else "This phone and your laptop" }) {
        val a = Art(this, t, 0f, 0f, size.width / 320f, measurer)
        a.rect(0f, 168f, 320f, 1.2f, "line-strong")
        a.sub(0f, 45.5f, 200f / 320f).laptop(LaptopLook.Connected, "")
        a.sub(240f, 26f, 72f / 124f).phone()
        listOf(Triple("wifi", 84f, "accent"), Triple("bluetooth", 114f, "info")).forEach { (g, y, tone) ->
            a.line(190f, y, 236f, y, tone, 1.6f, dashed = !connected)
            a.circle(190f, y, 2.4f, tone); a.circle(236f, y, 2.4f, tone)
            a.circle(213f, y, 9.5f, "bg-raised"); a.ring(213f, y, 9.5f, "line-strong", .8f)
            a.glyph(g, 207f, y - 6f, 12f, "$tone-ink", 2.2f)
        }
        if (connected) { a.circle(213f, 140f, 8f, "success"); a.glyph("check", 208f, 135f, 10f, "on-success", 3f) }
    }
}

enum class EmptyArt { Clipboard, Transfers }

/** A small drawing for an empty list, with what will appear there. */
@Composable fun EmptyState(art: EmptyArt, title: String, sub: String) {
    val t = LocalTokens.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 20.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Canvas(Modifier.padding(bottom = 6.dp).size(160.dp, 96.dp).alpha(.95f)) {
            val a = Art(this, t, 0f, 0f, size.width / 160f, null)
            when (art) {
                EmptyArt.Clipboard -> {
                    a.rect(56f, 12f, 48f, 70f, "icon-slate", 5f); a.rect(61f, 20f, 38f, 56f, "icon-paper", 2f)
                    a.rect(69f, 7f, 22f, 11f, "icon-slate-deep", 3f); a.rect(75f, 9.5f, 10f, 3f, "icon-slate", 1.5f)
                    listOf(28f, 24f, 30f, 18f).forEachIndexed { i, w -> a.rect(66f, 30f + i * 9f, w, 3f, "icon-paper-fold", 1.5f) }
                    a.circle(30f, 47f, 14f, "state-hover"); a.glyph("arrow-up", 22f, 39f, 16f, "world-linux", 2.4f)
                    a.circle(130f, 47f, 14f, "state-hover"); a.glyph("arrow-down", 122f, 39f, 16f, "world-phone", 2.4f)
                    a.line(45f, 47f, 54f, 47f, "line-strong", 1.2f, true); a.line(106f, 47f, 115f, 47f, "line-strong", 1.2f, true)
                }
                EmptyArt.Transfers -> {
                    a.icon("folder", 48f, 22f, 64f)
                    a.icon("file-image", 20f, 6f, 30f); a.icon("file-pdf", 110f, 4f, 30f)
                    a.circle(80f, 80f, 11f, "bg-raised"); a.ring(80f, 80f, 11f, "line-strong", .8f); a.glyph("swap", 73f, 73f, 14f, "accent-ink", 2.4f)
                    a.line(48f, 22f, 58f, 32f, "line-strong", 1.2f, true); a.line(112f, 22f, 102f, 32f, "line-strong", 1.2f, true)
                }
            }
        }
        Txt(title, bold = true, color = token("ink-strong"), align = TextAlign.Center)
        Txt(sub, muted = true, size = 13, line = 18, align = TextAlign.Center)
    }
}
