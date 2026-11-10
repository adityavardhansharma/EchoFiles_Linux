package app.echoconnect

import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.view.View
import android.widget.RemoteViews

/**
 * Home in miniature on the home screen: carbon (the laptop: link state, name, battery) over the
 * seam over paper (this phone: Files ↑ and Clipboard ↑). Redrawn when the link changes.
 */
class LinkWidget : AppWidgetProvider() {
    override fun onUpdate(ctx: Context, mgr: AppWidgetManager, ids: IntArray) {
        ConnectService.start(ctx)
        mgr.updateAppWidget(ids, views(ctx))
    }

    companion object {
        @Volatile private var last: String? = null

        /** Redraws every placed widget if what they show has changed. Cheap enough to call on a tick. */
        fun updateIfChanged(ctx: Context) {
            val l = Connect.state.value.laptop
            val key = "${l?.id}|${l?.name}|${l?.connected}|${l?.lan}|${l?.bluetooth}|${l?.battery}|${l?.charging}"
            if (key == last) return
            last = key
            update(ctx)
        }

        fun update(ctx: Context) {
            val mgr = AppWidgetManager.getInstance(ctx) ?: return
            val ids = mgr.getAppWidgetIds(ComponentName(ctx, LinkWidget::class.java))
            if (ids.isNotEmpty()) mgr.updateAppWidget(ids, views(ctx))
        }

        private fun views(ctx: Context): RemoteViews {
            val l = Connect.state.value.laptop
            val linked = l?.connected == true
            return RemoteViews(ctx.packageName, R.layout.widget_link).apply {
                setTextViewText(R.id.widget_state, when {
                    l == null -> "NOT PAIRED"
                    !linked -> "NO SIGNAL"
                    l.lan && l.bluetooth -> "LINKED · WI‑FI + BT"
                    l.lan -> "LINKED · WI‑FI"
                    else -> "LINKED · BT"
                })
                setTextViewText(R.id.widget_name, l?.name ?: "EchoConnect")
                setTextViewText(R.id.widget_battery, if (linked && l?.battery != null) "${l.battery}%" else "--")
                setInt(R.id.widget_led, "setColorFilter", if (linked) 0xFF2BD67B.toInt() else 0xFF4A4A46.toInt())
                setViewVisibility(R.id.widget_seam, if (linked) View.VISIBLE else View.GONE)
                setViewVisibility(R.id.widget_flat, if (linked) View.GONE else View.VISIBLE)
                setOnClickPendingIntent(R.id.widget_top, Notify.openApp(ctx))
                setOnClickPendingIntent(R.id.widget_files, Notify.pickFiles(ctx, 31))
                setOnClickPendingIntent(R.id.widget_clip, Notify.sendClipboard(ctx, 32))
            }
        }
    }
}
