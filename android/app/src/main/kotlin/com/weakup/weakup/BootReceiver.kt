package com.weakup.weakup

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Receives BOOT_COMPLETED broadcasts to re-register pending jobs after device reboot.
 *
 * On Android, pending jobs are persisted in the app's SQLite database.
 * After a reboot, the main Flutter engine must start to reconcile and re-register them.
 * This receiver starts the app in a minimal way so the scheduler can re-arm.
 */
class BootReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return

        Log.d("BootReceiver", "Boot completed: re-registering pending Weakup jobs.")

        // Launch the app's main activity in the background.
        // The Flutter engine will initialize, run main(), and the scheduler
        // will call reconcile() to re-arm any pending jobs.
        val launchIntent = context.packageManager
            .getLaunchIntentForPackage(context.packageName)

        if (launchIntent != null) {
            launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            context.startActivity(launchIntent)
        }
    }
}
