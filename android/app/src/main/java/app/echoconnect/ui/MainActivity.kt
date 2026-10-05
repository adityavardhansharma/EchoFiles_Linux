package app.echoconnect.ui

import android.Manifest
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.echoconnect.*
import app.echoconnect.bluetooth.BluetoothLink
import app.echoconnect.clipboard.ClipboardSync

class MainActivity : ComponentActivity(), Host {
    private var rev by mutableIntStateOf(0)
    private var page by mutableStateOf("home")
    private var tab by mutableStateOf("home")
    private var snackbar by mutableStateOf<Snack?>(null)

    private val files = registerForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isEmpty()) return@registerForActivityResult
        Work.run { uris.forEach { Connect.sendUri(it, count = uris.size) } }
        snack(Snack(if (uris.size == 1) "Sending 1 file to ${laptopName()}" else "Sending ${uris.size} files to ${laptopName()}", icon = "upload", action = "View") { go("transfers") })
    }
    private val permissions = registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { startFeatures(); refresh() }
    private val prefsListener = android.content.SharedPreferences.OnSharedPreferenceChangeListener { _, _ -> refresh() }

    override val context: Context get() = this
    override val revision: Int get() = rev

    override fun onCreate(saved: Bundle?) {
        super.onCreate(saved)
        enableEdgeToEdge()
        ConnectService.start(this)
        page = saved?.getString("page") ?: if (Prefs.pairedOnce(this)) "home" else "first-run"
        tab = saved?.getString("tab") ?: "home"
        intent?.getStringExtra(EXTRA_PAGE)?.let { go(it) }
        getSharedPreferences("echoconnect", MODE_PRIVATE).registerOnSharedPreferenceChangeListener(prefsListener)
        setContent { AppTheme { App() } }
    }
    override fun onNewIntent(intent: Intent) { super.onNewIntent(intent); intent.getStringExtra(EXTRA_PAGE)?.let { go(it) } }
    override fun onDestroy() { getSharedPreferences("echoconnect", MODE_PRIVATE).unregisterOnSharedPreferenceChangeListener(prefsListener); super.onDestroy() }
    override fun onSaveInstanceState(out: Bundle) { super.onSaveInstanceState(out); out.putString("page", page); out.putString("tab", tab) }
    override fun onResume() {
        super.onResume(); refresh(); startFeatures()
        // The app in front may read the clipboard: send anything copied while it was away.
        Work.main.postDelayed({ ClipboardSync.send(this); ClipboardSync.resume(this) }, 250)
    }

    private fun startFeatures() {
        BluetoothLink.start(this); app.echoconnect.calls.Calls.start(this); app.echoconnect.messages.Messages.start(this); app.echoconnect.signal.Signal.start(this)
    }
    private fun laptopName() = Connect.state.value.laptop?.name ?: "the laptop"

    // ------------------------------------------------------------------ Host
    override fun refresh() { rev++ }
    override fun request(vararg permissions: String) { this.permissions.launch(arrayOf(*permissions)) }
    override fun settings(action: String, packageUri: Boolean) {
        runCatching { startActivity(Intent(action).apply { if (packageUri) data = Uri.parse("package:$packageName") }) }
            .onFailure { runCatching { startActivity(Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:$packageName"))) } }
    }
    override fun requestPhotos() = request(*when {
        Build.VERSION.SDK_INT >= 34 -> arrayOf(Manifest.permission.READ_MEDIA_IMAGES, Manifest.permission.READ_MEDIA_VIDEO, Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED)
        Build.VERSION.SDK_INT >= 33 -> arrayOf(Manifest.permission.READ_MEDIA_IMAGES, Manifest.permission.READ_MEDIA_VIDEO)
        else -> arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
    })
    override fun pickFiles() = files.launch(arrayOf("*/*"))
    override fun scan() { startActivity(Intent(this, ScanActivity::class.java)) }
    override fun snack(s: Snack) { snackbar = s }
    override fun go(page: String) { if (page in TABS) tab = page; this.page = page }
    override fun open(url: String) { runCatching { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) } }

    // ------------------------------------------------------------------ UI
    @Composable private fun App() {
        val s by Connect.state.collectAsStateWithLifecycle()
        var linkSheet by remember { mutableStateOf(false) }
        var forget by remember { mutableStateOf(false) }
        val back = { page = tab }
        BackHandler(page !in TABS && page != "first-run") { back() }
        BackHandler(page in TABS && page != "home") { go("home") }
        Box(Modifier.fillMaxSize().background(token("bg"))) {
            Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal))) {
                Box(Modifier.weight(1f)) {
                    when (page) {
                        "first-run", "pair" -> PairingScreen(s, this@MainActivity, page == "first-run") { clip -> go(if (clip) "clip-setup" else "home"); if (clip) tab = "clipboard" }
                        "clipboard" -> ClipboardScreen(s, this@MainActivity)
                        "transfers" -> TransfersScreen(s, this@MainActivity)
                        "settings" -> SettingsScreen(s, this@MainActivity) { forget = true }
                        "permissions" -> PermissionsScreen(this@MainActivity, back)
                        "clip-setup" -> ClipboardSetupScreen(s, this@MainActivity, back)
                        "notify-apps" -> NotificationAppsScreen(this@MainActivity, back)
                        else -> HomeScreen(s, this@MainActivity) { linkSheet = true }
                    }
                    Box(Modifier.align(Alignment.BottomCenter)) { SnackbarHost(snackbar) { snackbar = null } }
                }
                if (page in TABS) Box(Modifier.background(token("bg-deep")).windowInsetsPadding(WindowInsets.navigationBars.only(WindowInsetsSides.Bottom))) {
                    BottomNav(TABS.indexOf(page), mapOf(2 to s.transfers.count { !it.finished && it.error == null })) { go(TABS[it]) }
                } else Spacer(Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)))
            }
            if (linkSheet) Box(Modifier.fillMaxSize()) { OpenLinkSheet(s, this@MainActivity) { linkSheet = false } }
        }
        if (page != "first-run" && page != "pair") s.pair?.let { PairDialog(it) }
        if (forget) s.laptop?.let { l ->
            EchoDialog("Forget ${l.name}?", { forget = false }, danger = true, body = "It stops seeing this phone and its cached file names are removed. Pair again with its code to undo.") {
                Button("Keep laptop", Variant.Ghost) { forget = false }
                Button("Forget laptop", Variant.Danger) { Connect.forget(l.id); forget = false; snack(Snack("Forgot ${l.name}", icon = "unlink")) }
            }
        }
    }

    companion object { const val EXTRA_PAGE = "page" }
}
