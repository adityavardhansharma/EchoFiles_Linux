package app.echoconnect.capture

import android.app.PendingIntent
import android.content.*
import android.os.Bundle
import com.google.mlkit.vision.documentscanner.*
import androidx.activity.result.IntentSenderRequest
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import app.echoconnect.*
import org.json.JSONObject
import java.io.File

object Capture : Plugin {
    override val handles = setOf("echofiles.capture.request")
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "capture")) return
        val intent = Intent(ctx, CaptureActivity::class.java).putExtra("scan", body.optBoolean("scan")).putExtra("requestId", body.optString("requestId"))
        val tap = PendingIntent.getActivity(ctx, 50, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        Notify.info(ctx, 50, if (body.optBoolean("scan")) "Scan a document for your laptop" else "Take a photo for your laptop", "Tap to open the camera. The result goes to the folder open in EchoFiles.", tap)
    }
}
class CaptureActivity : ComponentActivity() {
    private lateinit var file: File
    private val permission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) takePhoto() else { Notify.info(this, 50, "Camera permission needed", "Allow camera access to take a photo for your laptop."); finish() }
    }
    private fun takePhoto() { runCatching { photo.launch(FileProvider.getUriForFile(this, "$packageName.files", file)) }.onFailure { Notify.info(this, 50, "Couldn't open camera", it.message ?: "No camera app available"); finish() } }
    private val scan = registerForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) { result ->
        val document = GmsDocumentScanningResult.fromActivityResultIntent(result.data)
        document?.pdf?.uri?.let { uri -> Work.run { Connect.sendUri(uri, "echofiles.capture", JSONObject().put("requestId", intent.getStringExtra("requestId"))) } }
        finish()
    }
    private val photo = registerForActivityResult(ActivityResultContracts.TakePicture()) { ok ->
        if (ok) Work.run { Connect.sendFile(file, "echofiles.capture", JSONObject().put("requestId", intent.getStringExtra("requestId"))) }
        finish()
    }
    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved)
        file = File(File(cacheDir, "capture").apply { mkdirs() }, saved?.getString("file") ?: "photo-${System.currentTimeMillis()}.jpg")
        if (saved == null) {
            if (intent.getBooleanExtra("scan", false)) {
                val options = GmsDocumentScannerOptions.Builder().setScannerMode(GmsDocumentScannerOptions.SCANNER_MODE_FULL).setResultFormats(GmsDocumentScannerOptions.RESULT_FORMAT_PDF).setPageLimit(30).setGalleryImportAllowed(true).build()
                GmsDocumentScanning.getClient(options).getStartScanIntent(this).addOnSuccessListener { scan.launch(IntentSenderRequest.Builder(it).build()) }.addOnFailureListener { Notify.info(this, 50, "Couldn't open document scanner", it.message ?: "Check Google Play services and try again."); finish() }
            } else if (Work.allowed(this, android.Manifest.permission.CAMERA)) takePhoto() else permission.launch(android.Manifest.permission.CAMERA)
        }
    }
    override fun onSaveInstanceState(out: Bundle) { out.putString("file", file.name); super.onSaveInstanceState(out) }
}
