package app.echoconnect

import android.app.Notification
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import app.echoconnect.bluetooth.BluetoothLink
import app.echoconnect.clipboard.ClipboardReadActivity
import app.echoconnect.clipboard.ClipboardSync

/**
 * Keeps EchoConnect connected while the screen is off: the core's threads live here, it
 * re-announces when the phone joins a network, holds the multicast lock UDP discovery needs,
 * and shows the one quiet notification Android requires.
 */
class ConnectService : Service() {
    private var multicast: WifiManager.MulticastLock? = null
    private val main = Handler(Looper.getMainLooper())
    private val net = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) {
            main.postDelayed({ Connect.announce() }, 1500)
        }
    }
    private var tickCount = 0
    private val tick = object : Runnable {
        override fun run() {
            if (Connect.state.value.transfers.any { !it.finished }) Connect.tickTransfers()
            if (++tickCount % 30 == 0) BluetoothLink.start(this@ConnectService)
            if (Connect.laptopId == null) ClipboardSync.stopReader()
            main.postDelayed(this, 500)
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        startForeground(Notify.ID_SERVICE, notification(this), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        multicast = (applicationContext.getSystemService(WIFI_SERVICE) as WifiManager).createMulticastLock("echoconnect").apply {
            setReferenceCounted(false)
            acquire()
        }
        Connect.start()
        val cm = getSystemService(ConnectivityManager::class.java)
        cm.registerNetworkCallback(NetworkRequest.Builder().addTransportType(NetworkCapabilities.TRANSPORT_WIFI).build(), net)
        main.post(tick)
        BluetoothLink.start(this)
        ClipboardSync.startWatching(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        refresh(this)
        return START_STICKY
    }

    override fun onDestroy() {
        runCatching { getSystemService(ConnectivityManager::class.java).unregisterNetworkCallback(net) }
        multicast?.release()
        main.removeCallbacksAndMessages(null)
        BluetoothLink.stop()
        ClipboardSync.stopReader()
        Connect.stop()
        super.onDestroy()
    }

    companion object {
        fun start(ctx: Context) {
            runCatching { ctx.startForegroundService(Intent(ctx, ConnectService::class.java)) }
        }

        fun refresh(ctx: Context) {
            val nm = ctx.getSystemService(NotificationManager::class.java)
            nm.notify(Notify.ID_SERVICE, notification(ctx))
        }

        private fun notification(ctx: Context): Notification {
            val s = Connect.state.value
            val l = s.laptops.firstOrNull { it.paired && it.connected }
            val title = when {
                l != null -> "Connected to ${l.name}"
                s.laptops.any { it.paired } -> "Waiting for ${s.laptops.first { it.paired }.name}"
                else -> "Not paired yet"
            }
            val how = buildList {
                if (l?.lan == true) add("Wi-Fi")
                if (l?.bluetooth == true) add("Bluetooth")
                add(if (Prefs.clipMode(ctx) == "auto") "clipboard automatic" else "Tap to send clipboard")
            }.joinToString(" · ")
            val sendClip = PendingIntent.getActivity(ctx, 10, Intent(ctx, ClipboardReadActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK).putExtra("send", true), PendingIntent.FLAG_IMMUTABLE)
            val b = Notification.Builder(ctx, Notify.SERVICE)
                .setSmallIcon(R.drawable.ic_laptop)
                .setContentTitle(title)
                .setContentText(if (l != null) how else "Open EchoConnect to pair with EchoFiles")
                .setContentIntent(Notify.openApp(ctx))
                .setOngoing(true)
                .setShowWhen(false)
            if (l != null) {
                b.addAction(Notification.Action.Builder(null, "Send clipboard", sendClip).build())
                if (l.echofiles && l.lan) b.addAction(Notification.Action.Builder(null, "Lock laptop", Notify.action(ctx, NotificationActions.LOCK, 11)).build())
            }
            return b.build()
        }
    }
}
