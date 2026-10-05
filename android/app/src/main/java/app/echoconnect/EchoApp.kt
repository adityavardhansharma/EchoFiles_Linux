package app.echoconnect

import android.app.Application
import app.echoconnect.battery.Battery
import app.echoconnect.calls.Calls
import app.echoconnect.capture.Capture
import app.echoconnect.clipboard.ClipboardSync
import app.echoconnect.contacts.Contacts
import app.echoconnect.files.Files
import app.echoconnect.laptop.Dnd
import app.echoconnect.laptop.LaptopCommands
import app.echoconnect.media.Media
import app.echoconnect.messages.Messages
import app.echoconnect.notifications.Notifications
import app.echoconnect.photos.Photos
import app.echoconnect.ring.Ring
import app.echoconnect.share.Share

class EchoApp : Application() {
    override fun onCreate() {
        super.onCreate()
        Connect.init(this)
        Notify.channels(this)
        listOf(Battery, ClipboardSync, Notifications, Messages, Contacts, Calls, Media, Share, Ring, Files, Photos, Capture, LaptopCommands, Dnd, app.echoconnect.signal.Signal)
            .forEach(Connect::register)
        ConnectService.start(this)
    }
}
