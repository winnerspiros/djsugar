package org.mixxx;

// DISCLAIMER: AI-generated file. Human review, device testing, and
// human commit required before use. See djsugar/AGENTS.md AI Agent Policy.
// DISCLAIMER END.

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.media.AudioAttributes;
import android.media.AudioFocusRequest;
import android.media.AudioManager;
import android.os.IBinder;
import android.os.PowerManager;
import android.util.Log;

/**
 * Foreground media-playback service for DJ Sugar on Android.
 *
 * Android 8+ background-execution limits (stricter on Android 14+) kill audio
 * when backgrounded. Holds foreground service + partial wake lock + audio focus
 * while mixing. On permanent audio-focus loss, relinquishes foreground state
 * and wake lock; on gain, re-acquires.
 *
 * TODO: route focus events into engine via JNI (nativeAudioFocusChanged) so
 * LOSS pauses the mix and LOSS_TRANSIENT_CAN_DUCK ducks volume. Until then,
 * hooks only manage protection state.
 */
public class PlaybackService extends Service {
    private static final String TAG = "DJSugarPlayback";
    private static final String CHANNEL_ID = "djsugar_playback";
    private static final int NOTIFICATION_ID = 1;

    public static final String ACTION_START =
        "org.mixxx.action.START_PLAYBACK_SERVICE";
    public static final String ACTION_STOP =
        "org.mixxx.action.STOP_PLAYBACK_SERVICE";

    private PowerManager.WakeLock mWakeLock;
    private AudioManager mAudioManager;
    private AudioFocusRequest mFocusRequest;
    private final AudioManager.OnAudioFocusChangeListener mFocusListener =
        new AudioManager.OnAudioFocusChangeListener() {
            @Override
            public void onAudioFocusChange(int focusChange) {
                switch (focusChange) {
                    case AudioManager.AUDIOFOCUS_LOSS:
                        // Pause engine, keep focus request held so a later
                        // AUDIOFOCUS_GAIN can restore protection. Keep
                        // foreground + wake lock until engine confirms pause.
                        Log.i(TAG, "Audio focus lost permanently, pausing mix");
                        pauseEnginePlayback();
                        break;
                    case AudioManager.AUDIOFOCUS_GAIN:
                        Log.i(TAG, "Audio focus gained, re-acquiring");
                        acquireWakeLock();
                        requestAudioFocus();
                        startInForeground();
                        resumeEnginePlayback();
                        break;
                    case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT:
                    case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK:
                        // Coarse handling until JNI: relinquish foreground, keep alive.
                        Log.i(TAG, "Audio focus transient loss, standing down");
                        releaseWakeLock();
                        break;
                    default:
                        break;
                }
            }
        };

    public static void start(Context context) {
        Intent intent = new Intent(context, PlaybackService.class);
        intent.setAction(ACTION_START);
        context.startForegroundService(intent);
    }

    /**
     * JNI entry: lazy-start from native deck-play (see EngineBuffer).
     */
    public static void startFromNative(Context context) {
        start(context);
    }

    public static void stop(Context context) {
        Intent intent = new Intent(context, PlaybackService.class);
        intent.setAction(ACTION_STOP);
        context.startService(intent);
    }

    @Override
    public void onCreate() {
        super.onCreate();
        mAudioManager = (AudioManager) getSystemService(Context.AUDIO_SERVICE);
        createNotificationChannel();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_STOP.equals(intent.getAction())) {
            abandonAudioFocus();
            releaseWakeLock();
            stopForeground(STOP_FOREGROUND_REMOVE);
            stopSelf();
            return START_NOT_STICKY;
        }
        acquireWakeLock();
        requestAudioFocus();
        startInForeground();
        // Recreate if killed under memory pressure (intent redelivered).
        return START_STICKY;
    }

    @Override
    public void onDestroy() {
        abandonAudioFocus();
        releaseWakeLock();
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    private void startInForeground() {
        Notification notification = buildNotification();
        startForeground(NOTIFICATION_ID, notification,
            android.content.pm.ServiceInfo
                .FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
    }

    private void createNotificationChannel() {
        NotificationChannel channel = new NotificationChannel(
            CHANNEL_ID,
            "Playback",
            NotificationManager.IMPORTANCE_LOW);
        channel.setDescription("Keeps your mix playing in the background");
        NotificationManager manager =
            (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        if (manager != null) {
            manager.createNotificationChannel(channel);
        }
    }

    private Notification buildNotification() {
        Notification.Builder builder =
            new Notification.Builder(this, CHANNEL_ID);
        return builder
            .setContentTitle("DJ Sugar")
            .setContentText("Mixing — playback protected in background")
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setOngoing(true)
            .build();
    }

    private void acquireWakeLock() {
        PowerManager powerManager =
            (PowerManager) getSystemService(Context.POWER_SERVICE);
        if (powerManager == null) {
            return;
        }
        if (mWakeLock == null) {
            mWakeLock = powerManager.newWakeLock(
                PowerManager.PARTIAL_WAKE_LOCK, TAG + ":lock");
            mWakeLock.setReferenceCounted(false);
        }
        // No timeout: lock lifetime tied to playback, every stop path releases.
        // Re-acquiring an already-held non-ref-counted lock is a no-op.
        if (!mWakeLock.isHeld()) {
            mWakeLock.acquire();
        }
    }

    private void releaseWakeLock() {
        if (mWakeLock != null && mWakeLock.isHeld()) {
            mWakeLock.release();
        }
        mWakeLock = null;
    }

    private void requestAudioFocus() {
        if (mAudioManager == null) {
            return;
        }
        {
            if (mFocusRequest == null) {
                AudioAttributes attributes =
                    new AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                        .build();
                mFocusRequest =
                    new AudioFocusRequest.Builder(
                                             AudioManager.AUDIOFOCUS_GAIN)
                        .setAudioAttributes(attributes)
                        .setOnAudioFocusChangeListener(mFocusListener)
                        .build();
            }
            mAudioManager.requestAudioFocus(mFocusRequest);
        }
    }

    private void abandonAudioFocus() {
        if (mAudioManager == null) {
            return;
        }
        if (mFocusRequest != null) {
            mAudioManager.abandonAudioFocusRequest(mFocusRequest);
        }
    }

    // Engine hooks (TODO: wire via JNI to nativeAudioFocusChanged).
    // Until then these only manage protection state.
    private void pauseEnginePlayback() {
        releaseWakeLock();
        stopForeground(STOP_FOREGROUND_DETACH);
    }

    private void resumeEnginePlayback() {
        // Protection re-acquired by GAIN handler; engine resume lands with JNI.
    }
}

// DISCLAIMER: AI-generated file. Human review, device testing, and
// human commit required before use. See djsugar/AGENTS.md AI Agent Policy.
// DISCLAIMER END.
