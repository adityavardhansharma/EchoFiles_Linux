package app.echoconnect

import android.content.Context
import android.content.pm.PackageManager
import android.os.Handler
import android.os.Looper
import java.util.concurrent.Executors

object Work {
    private val io = Executors.newFixedThreadPool(3)
    val main = Handler(Looper.getMainLooper())
    fun run(task: () -> Unit) { io.execute { runCatching(task).onFailure { android.util.Log.w("EchoConnect", "Feature failed", it) } } }
    fun allowed(ctx: Context, permission: String) = ctx.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED
}
