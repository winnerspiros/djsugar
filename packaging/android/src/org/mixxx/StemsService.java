package org.mixxx;

import android.Manifest;
import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.content.pm.ServiceInfo;
import android.os.BatteryManager;
import android.os.Build;
import android.os.IBinder;
import android.os.PowerManager;
import android.util.Log;

/// Keeps Mixxx running while it separates tracks into stems: a foreground
/// service with a progress notification and a partial wake lock, so that
/// Android doesn't stop the work when the screen turns off or the user
/// switches to another app. The work itself runs in Mixxx' native threads.
public class StemsService extends Service {
    private static final String TAG = "MixxxStems";
    private static final String CHANNEL_ID = "stems";
    private static final int NOTIFICATION_ID = 7421;
    private static final String EXTRA_TEXT = "text";

    private PowerManager.WakeLock mWakeLock;

    /// Starts the service, or updates its notification
    public static void start(Context context, String title, String text) {
        Intent intent = new Intent(context, StemsService.class);
        intent.putExtra(Intent.EXTRA_TITLE, title);
        intent.putExtra(EXTRA_TEXT, text);
        try {
            context.startForegroundService(intent);
        } catch (RuntimeException e) {
            // E.g. ForegroundServiceStartNotAllowedException in the
            // background: the work goes on while the app is visible
            Log.w(TAG, "Unable to start the stems service", e);
        }
    }

    public static void stop(Context context) {
        context.stopService(new Intent(context, StemsService.class));
    }

    public static boolean isCharging(Context context) {
        Intent battery = context.registerReceiver(
                null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
        if (battery == null) {
            return false;
        }
        int status = battery.getIntExtra(BatteryManager.EXTRA_STATUS, -1);
        return status == BatteryManager.BATTERY_STATUS_CHARGING
                || status == BatteryManager.BATTERY_STATUS_FULL;
    }

    /// Asks for the permission to show the progress notification
    /// (Android 13+). Must run on the Android UI thread.
    public static void requestNotificationPermission(Context context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU
                || !(context instanceof Activity)) {
            return;
        }
        Activity activity = (Activity) context;
        if (activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED) {
            activity.requestPermissions(
                    new String[] {Manifest.permission.POST_NOTIFICATIONS}, NOTIFICATION_ID);
        }
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        NotificationManager manager = getSystemService(NotificationManager.class);
        if (manager.getNotificationChannel(CHANNEL_ID) == null) {
            NotificationChannel channel = new NotificationChannel(
                    CHANNEL_ID, "Stems", NotificationManager.IMPORTANCE_LOW);
            manager.createNotificationChannel(channel);
        }
        String title = intent != null ? intent.getStringExtra(Intent.EXTRA_TITLE) : null;
        String text = intent != null ? intent.getStringExtra(EXTRA_TEXT) : null;
        Intent openApp = getPackageManager().getLaunchIntentForPackage(getPackageName());
        Notification.Builder builder = new Notification.Builder(this, CHANNEL_ID)
                .setContentTitle(title != null ? title : "Mixxx")
                .setContentText(text != null ? text : "")
                .setSmallIcon(getApplicationInfo().icon)
                .setOngoing(true)
                .setOnlyAlertOnce(true);
        if (openApp != null) {
            builder.setContentIntent(android.app.PendingIntent.getActivity(this, 0, openApp,
                    android.app.PendingIntent.FLAG_IMMUTABLE
                            | android.app.PendingIntent.FLAG_UPDATE_CURRENT));
        }
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(NOTIFICATION_ID, builder.build(),
                        ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC);
            } else {
                startForeground(NOTIFICATION_ID, builder.build());
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Unable to run the stems service in the foreground", e);
            stopSelf();
            return START_NOT_STICKY;
        }
        if (mWakeLock == null) {
            PowerManager power = getSystemService(PowerManager.class);
            mWakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "mixxx:stems");
            mWakeLock.acquire();
        }
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        if (mWakeLock != null && mWakeLock.isHeld()) {
            mWakeLock.release();
        }
        mWakeLock = null;
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
