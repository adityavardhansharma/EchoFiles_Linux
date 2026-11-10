package app.echoconnect.ui

import android.Manifest
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.*
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.animation.core.*
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import app.echoconnect.*
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.common.InputImage
import org.json.JSONObject
import java.util.concurrent.Executors

/** The camera pointed at the code EchoFiles shows: signal brackets over the picture, a carbon panel reading below. */
@androidx.annotation.OptIn(androidx.camera.core.ExperimentalGetImage::class)
class ScanActivity : ComponentActivity() {
    private val executor = Executors.newSingleThreadExecutor()
    @Volatile private var consumed = false
    private val scanner = BarcodeScanning.getClient()
    private var status by mutableStateOf("Hold the phone so the code fills the corners")
    private var found by mutableStateOf(false)
    private var allowed by mutableStateOf<Boolean?>(null)
    private val permission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { allowed = it }

    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved); enableEdgeToEdge()
        allowed = if (Work.allowed(this, Manifest.permission.CAMERA)) true else { permission.launch(Manifest.permission.CAMERA); null }
        setContent { AppTheme {
            val r = relay()
            Box(Modifier.fillMaxSize().background(Color.Black)) {
                when (allowed) {
                    false -> Box(Modifier.fillMaxSize().background(Color(0xFF15161A)))
                    else -> if (allowed == true) AndroidView({ ctx -> PreviewView(ctx).also { bind(it) } }, Modifier.fillMaxSize())
                }
                Brackets(Modifier.align(Alignment.Center).offset(y = (-90).dp).size(270.dp), if (found) r.live else r.sig)
                Row(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.statusBars).sideSafe().padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Carbon(Modifier.clip(RoundedCornerShape(14.dp))) { Box(Modifier.size(48.dp).clickable(onClickLabel = "Close") { finish() }.semantics { contentDescription = "Close" }, contentAlignment = Alignment.Center) { Glyph("close") } }
                    Carbon(Modifier.clip(RoundedCornerShape(12.dp)), Color(0xCC121211)) { Lbl("Point at EchoFiles › Connect phone", Color(0xFFF2EFE8), Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) }
                }
                Carbon(Modifier.align(Alignment.BottomCenter).fillMaxWidth().clip(RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp))) {
                    Column(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.navigationBars).sideSafe().padding(start = 16.dp, end = 16.dp, top = 18.dp, bottom = 22.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                        Box(Modifier.fillMaxWidth().height(4.dp).background(r.sig))
                        if (allowed == false) Banner("Camera is off for EchoConnect", "Allow the camera to read the code, or go back and type the laptop's address.") {
                            Key("Allow camera", height = 40.dp, size = 13) { permission.launch(Manifest.permission.CAMERA) }
                        } else Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                            ReadGrid(found, Modifier.size(120.dp))
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                Lbl(if (found) "Code read" else "Reading code")
                                Dm(if (found) "FOUND" else "LOOK", 40, if (found) r.live else r.sig, glow = true)
                                Lbl(status, ink(2), maxLines = 3)
                            }
                        }
                        T("The code pins the laptop's certificate, so nobody else on the network can pose as it.", 13, color = ink(2))
                        Key("Type the address instead", Modifier.fillMaxWidth(), Tone.Carbon, height = 50.dp, size = 14) { finish() }
                    }
                }
            }
        } }
    }

    /** Four chunky corners that breathe while looking and snap green when a code is found. */
    @Composable private fun Brackets(modifier: Modifier, color: Color) {
        val breathe by rememberInfiniteTransition(label = "hunt").animateFloat(0f, 1f, infiniteRepeatable(tween(1000, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "b")
        Canvas(modifier.clearAndSetSemantics { }) {
            val inset = (if (found) 0f else breathe * 6f) * density; val len = 50.dp.toPx(); val w = 7.dp.toPx()
            val l = inset; val t = inset; val r = size.width - inset; val b = size.height - inset
            fun line(a: Offset, z: Offset) = drawLine(color, a, z, w, StrokeCap.Round)
            line(Offset(l, t), Offset(l + len, t)); line(Offset(l, t), Offset(l, t + len))
            line(Offset(r, t), Offset(r - len, t)); line(Offset(r, t), Offset(r, t + len))
            line(Offset(l, b), Offset(l + len, b)); line(Offset(l, b), Offset(l, b - len))
            line(Offset(r, b), Offset(r - len, b)); line(Offset(r, b), Offset(r, b - len))
        }
    }

    /** A 21 × 21 dot grid: a sweep lights it row by row while looking; all of it lights when found. */
    @Composable private fun ReadGrid(found: Boolean, modifier: Modifier) {
        val sig = relay().sig; val live = relay().live
        val sweep by rememberInfiniteTransition(label = "grid").animateFloat(0f, 21f, infiniteRepeatable(tween(1600, easing = LinearEasing)), label = "s")
        Canvas(modifier.clearAndSetSemantics { }) {
            val n = 21; val step = size.width / n; val rad = step * 0.36f
            for (y in 0 until n) for (x in 0 until n) {
                val finder = (x < 7 && y < 7) || (x > 13 && y < 7) || (x < 7 && y > 13)
                val on = when {
                    found -> finder || (x * 7 + y * 13) % 3 != 0
                    else -> finder && y.toFloat() < sweep || (y == sweep.toInt())
                }
                drawCircle(if (!on) Color(0xFF2D2D2A) else if (found) live else sig, rad, Offset(x * step + step / 2, y * step + step / 2))
            }
        }
    }

    private fun bind(preview: PreviewView) {
        val provider = ProcessCameraProvider.getInstance(this)
        provider.addListener({
            val p = provider.get()
            val display = Preview.Builder().build().apply { surfaceProvider = preview.surfaceProvider }
            val analysis = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
            analysis.setAnalyzer(executor) { frame ->
                val image = frame.image
                if (image == null || consumed) { frame.close(); return@setAnalyzer }
                scanner.process(InputImage.fromMediaImage(image, frame.imageInfo.rotationDegrees)).addOnSuccessListener { codes ->
                    codes.firstNotNullOfOrNull { it.rawValue }?.let { raw -> accept(raw) }
                }.addOnCompleteListener { frame.close() }
            }
            runCatching { p.unbindAll(); p.bindToLifecycle(this, CameraSelector.DEFAULT_BACK_CAMERA, display, analysis) }
                .onFailure { status = "The camera is busy. Close other camera apps and try again." }
        }, ContextCompat.getMainExecutor(this))
    }

    private fun accept(raw: String) {
        if (consumed) return
        val o = runCatching { JSONObject(raw) }.getOrNull()
        val ok = o != null && runCatching {
            require(o.optInt("version") == 1 && o.optString("app") == "echoconnect")
            require(o.getString("fingerprint").matches(Regex("[0-9a-fA-F]{64}")))
        }.isSuccess
        if (!ok) { status = "That's not an EchoFiles pairing code"; return }
        consumed = true
        found = true; status = "Found ${o!!.optString("name").ifBlank { "your laptop" }} · connecting…"
        Connect.expectQr(o.getString("id"), o.getString("fingerprint").lowercase())
        o.optString("bluetooth").takeIf { it.matches(Regex("([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}")) }?.let { Prefs.setLaptopBluetooth(this, it) }
        Work.run {
            val connected = Connect.connect(o.getString("host"), o.optInt("port", 1716))
            Work.main.post {
                if (connected) finish()
                else { consumed = false; found = false; status = "Couldn't reach the laptop. Check both are on the same Wi-Fi." }
            }
        }
    }

    override fun onDestroy() { scanner.close(); executor.shutdown(); super.onDestroy() }
}
