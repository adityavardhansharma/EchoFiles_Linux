package app.echoconnect.contacts

import android.Manifest
import android.content.Context
import android.net.Uri
import android.provider.ContactsContract
import app.echoconnect.*
import org.json.JSONObject

object Contacts : Plugin {
    override val handles = setOf("kdeconnect.contacts.request_all_uids_timestamps", "kdeconnect.contacts.request_vcards_by_uid")
    override fun onPacket(ctx: Context, laptop: String, type: String, body: JSONObject) {
        if (!Prefs.on(ctx, "contacts") || !Work.allowed(ctx, Manifest.permission.READ_CONTACTS)) return
        Work.run {
            val out = JSONObject()
            if (type.endsWith("all_uids_timestamps")) {
                ctx.contentResolver.query(ContactsContract.Contacts.CONTENT_URI, arrayOf("lookup", "contact_last_updated_timestamp"), null, null, null)?.use { c ->
                    while (c.moveToNext()) out.put(c.getString(0), c.getLong(1))
                }
                Connect.send("kdeconnect.contacts.response_uids_timestamps", out, laptop)
            } else {
                val ids = body.optJSONArray("uids") ?: return@run
                for (i in 0 until ids.length().coerceAtMost(200)) {
                    val id = ids.getString(i)
                    val uri = Uri.withAppendedPath(ContactsContract.Contacts.CONTENT_VCARD_URI, Uri.encode(id))
                    ctx.contentResolver.openInputStream(uri)?.bufferedReader()?.use { out.put(id, it.readText()) }
                }
                Connect.send("kdeconnect.contacts.response_vcards", out, laptop)
            }
        }
    }
    fun name(ctx: Context, number: String): String = runCatching {
        if (!Prefs.on(ctx, "contacts") || !Work.allowed(ctx, Manifest.permission.READ_CONTACTS)) return number
        ctx.contentResolver.query(Uri.withAppendedPath(ContactsContract.PhoneLookup.CONTENT_FILTER_URI, Uri.encode(number)), arrayOf("display_name"), null, null, null)?.use { if (it.moveToFirst()) it.getString(0) else null } ?: number
    }.getOrDefault(number)
}
