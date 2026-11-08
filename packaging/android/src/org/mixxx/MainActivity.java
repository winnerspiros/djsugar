package org.mixxx;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.hardware.usb.UsbManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Process;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;
import org.mixxx.youtube.PoTokenManager;
import org.qtproject.qt.android.QtActivityBase;

public class MainActivity extends QtActivityBase {
    private final BroadcastReceiver mUsbHotplugReceiver = new BroadcastReceiver() {
        @Override
        public void onReceive(Context context, Intent intent) {
            UsbPermission.notifyDevicesChanged();
        }
    };

    @Override
    public void onCreate(Bundle savedInstanceState) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowManager.LayoutParams lp = getWindow().getAttributes();
            lp.layoutInDisplayCutoutMode =
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS;
            getWindow().setAttributes(lp);
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            WindowManager.LayoutParams lp = getWindow().getAttributes();
            lp.layoutInDisplayCutoutMode =
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
            getWindow().setAttributes(lp);
        }
        super.onCreate(savedInstanceState);

        // Warm up the on-device BotGuard WebView early so the YouTube
        // downloader can mint PO tokens without waiting on first use.
        PoTokenManager.getInstance().init(getApplicationContext());

        // Keep CPU awake and screen on during mixing
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            getWindow().setPreferMinimalPostProcessing(true);
        }

        // Fullscreen and notch (display cutout) edge-to-edge handling
        AndroidScreenManager.applyFullScreen(this);
        AndroidScreenManager.installCutoutAndFullscreenListeners(this);

        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_DISPLAY);

        final View decorView = getWindow().getDecorView();
        decorView.setOnCapturedPointerListener(new View.OnCapturedPointerListener() {
            @Override
            public boolean onCapturedPointer(View view, MotionEvent event) {
                return false;
            }
        });

        // USB_DEVICE_ATTACHED is delivered through onNewIntent, and
        // USB_DEVICE_DETACHED needs a receiver.
        IntentFilter usbFilter = new IntentFilter();
        usbFilter.addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED);
        usbFilter.addAction(UsbManager.ACTION_USB_DEVICE_DETACHED);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(mUsbHotplugReceiver, usbFilter, Context.RECEIVER_NOT_EXPORTED);
        } else {
            registerReceiver(mUsbHotplugReceiver, usbFilter);
        }

        MidiHelper.init(this);
    }

    @Override
    protected void onResume() {
        super.onResume();
        AndroidScreenManager.applyFullScreen(this);
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            AndroidScreenManager.applyFullScreen(this);
        }
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        return super.dispatchKeyEvent(event);
    }

    /// Saves the settings and the loaded tracks on the main thread of Mixxx,
    /// waits for it at most a few seconds
    private static native void saveBeforeExit();

    @Override
    protected void onDestroy() {
        try {
            unregisterReceiver(mUsbHotplugReceiver);
        } catch (IllegalArgumentException e) {
            // Receiver was not registered
        }
        if (isFinishing()) {
            PlaybackService.stop(this);
            try {
                saveBeforeExit();
            } catch (UnsatisfiedLinkError e) {
                // The native library isn't loaded (yet)
            }
            Process.killProcess(Process.myPid());
        }
        super.onDestroy();
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        if (UsbManager.ACTION_USB_DEVICE_ATTACHED.equals(intent.getAction())) {
            UsbPermission.notifyDevicesChanged();
        }
    }
}
