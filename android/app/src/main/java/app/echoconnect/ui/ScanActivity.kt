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
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import app.echoconnect.*
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.common.InputImage
import org.json.JSONObject
import java.util.concurrent.Executors

/** The camera pointed at the code EchoFiles shows: accent corner brackets and one status line. */
@androidx.annotation.OptIn(androidx.camera.core.ExperimentalGetImage::class)
class ScanActivity : ComponentActivity() {
    private val executor = Executors.newSingleThreadExecutor()
    @Volatile private var consumed = false
    private val scanner = BarcodeScanning.getClient()
    private var status by mutableStateOf("Point at the code in EchoFiles → Connect phone")
    private var found by mutableStateOf(false)
    private var allowed by mutableStateOf<Boolean?>(null)
    private val permission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { allowed = it }

    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved); enableEdgeToEdge()
        allowed = if (Work.allowed(this, Manifest.permission.CAMERA)) true else { permission.launch(Manifest.permission.CAMERA); null }
        setContent { AppTheme {
            Column(Modifier.fillMaxSize().background(token("bg")).safeDrawingPadding()) {
                AppBar("Scan the laptop's code", back = { finish() })
                ScreenBody(Modifier.weight(1f)) {
                    Lede(strong("On the laptop, open EchoFiles and click **Connect phone** in the sidebar. The code pins the laptop's certificate, so nobody else can pose as it."))
                    when (allowed) {
                        false -> Banner("Camera is off for EchoConnect.", "Allow the camera to scan, or go back and type the laptop's address.", "warning") {
                            Button("Allow camera", size = BtnSize.Sm) { permission.launch(Manifest.permission.CAMERA) }
                        }
                        else -> Viewfinder(allowed == true)
                    }
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                        if (found) Glyph("check", color = token("success-ink"), size = 16)
                        Txt(status, size = 13, line = 18, color = token(if (found) "success-ink" else "ink-muted"), align = TextAlign.Center, modifier = Modifier.padding(start = 8.dp))
                    }
                }
                Column(Modifier.fillMaxWidth().hairline().padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 16.dp)) {
                    Button("Type the laptop's address instead", Variant.Ghost, block = true) { finish() }
                }
            }
        } }
    }

    @Composable private fun Viewfinder(camera: Boolean) {
        val corner = token(if (found) "success" else "accent")
        Box(Modifier.fillMaxWidth().aspectRatio(1f).widthIn(max = 300.dp).clip(RoundedCornerShape(6.dp)).background(Color(0xFF0A0B10)), contentAlignment = Alignment.Center) {
            if (camera) AndroidView({ ctx -> PreviewView(ctx).also { bind(it) } }, Modifier.fillMaxSize())
            Canvas(Modifier.fillMaxSize()) {
                val inset = size.width * .14f; val len = 36.dp.toPx(); val w = 4.dp.toPx()
                val l = inset; val t = inset; val r = size.width - inset; val b = size.height - inset
                fun line(a: Offset, z: Offset) = drawLine(corner, a, z, w, StrokeCap.Square)
                line(Offset(l, t), Offset(l + len, t)); line(Offset(l, t), Offset(l, t + len))
                line(Offset(r, t), Offset(r - len, t)); line(Offset(r, t), Offset(r, t + len))
                line(Offset(l, b), Offset(l + len, b)); line(Offset(l, b), Offset(l, b - len))
                line(Offset(r, b), Offset(r - len, b)); line(Offset(r, b), Offset(r, b - len))
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
