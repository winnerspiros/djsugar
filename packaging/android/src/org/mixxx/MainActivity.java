package org.mixxx;

import android.Manifest;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.os.Process;
import android.provider.Settings;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;
import org.qtproject.qt.android.QtActivityBase;

public class MainActivity extends QtActivityBase {
    private static final int MEDIA_PERMISSION_REQUEST = 1001;
    private static final int BLUETOOTH_PERMISSION_REQUEST = 1002;
    private boolean mRequestedAllFilesAccess;
    private boolean mWaitingForMediaPermission;

    @Override
    public void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            getWindow().setPreferMinimalPostProcessing(true);
        }

        // Apply fullscreen handling for notch devices
        AndroidScreenManager.applyFullScreen(this);

        WindowInsetsControllerCompat windowInsetsController =
            WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
        windowInsetsController.setSystemBarsBehavior(
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
        windowInsetsController.hide(WindowInsetsCompat.Type.navigationBars());

        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_DISPLAY);

        final View decorView = getWindow().getDecorView();
        decorView.setOnCapturedPointerListener(new View.OnCapturedPointerListener() {
            @Override
            public boolean onCapturedPointer(View view, MotionEvent event) {
                return false;
            }
        });

        requestMusicLibraryPermissions();
    }

    private void requestBluetoothPermissions() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            java.util.List<String> needed = new java.util.ArrayList<>();
            if (checkSelfPermission(Manifest.permission.NEARBY_WIFI_DEVICES)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.NEARBY_WIFI_DEVICES);
            }
            if (checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.BLUETOOTH_CONNECT);
            }
            if (checkSelfPermission(Manifest.permission.BLUETOOTH_SCAN)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.BLUETOOTH_SCAN);
            }
            if (checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.ACCESS_FINE_LOCATION);
            }
            if (!needed.isEmpty()) {
                requestPermissions(
                    needed.toArray(new String[0]),
                    BLUETOOTH_PERMISSION_REQUEST);
            }
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            java.util.List<String> needed = new java.util.ArrayList<>();
            if (checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.BLUETOOTH_CONNECT);
            }
            if (checkSelfPermission(Manifest.permission.BLUETOOTH_SCAN)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.BLUETOOTH_SCAN);
            }
            if (checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION)
                != PackageManager.PERMISSION_GRANTED) {
                needed.add(Manifest.permission.ACCESS_FINE_LOCATION);
            }
            if (!needed.isEmpty()) {
                requestPermissions(
                    needed.toArray(new String[0]),
                    BLUETOOTH_PERMISSION_REQUEST);
            }
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            if (checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION)
                != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(
                    new String[] {Manifest.permission.ACCESS_COARSE_LOCATION},
                    BLUETOOTH_PERMISSION_REQUEST);
            }
        }
    }

    private void requestMusicLibraryPermissions() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (checkSelfPermission(Manifest.permission.READ_MEDIA_AUDIO)
                != PackageManager.PERMISSION_GRANTED) {
                mWaitingForMediaPermission = true;
                requestPermissions(
                    new String[] {Manifest.permission.READ_MEDIA_AUDIO},
                    MEDIA_PERMISSION_REQUEST);
                return;
            }
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            if (checkSelfPermission(Manifest.permission.READ_EXTERNAL_STORAGE)
                != PackageManager.PERMISSION_GRANTED) {
                mWaitingForMediaPermission = true;
                requestPermissions(
                    new String[] {Manifest.permission.READ_EXTERNAL_STORAGE},
                    MEDIA_PERMISSION_REQUEST);
                return;
            }
        }

        requestAllFilesAccessIfNeeded();
    }

    @Override
    public void onRequestPermissionsResult(
        int requestCode, String[] permissions, int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        if (requestCode == MEDIA_PERMISSION_REQUEST) {
            mWaitingForMediaPermission = false;
            requestBluetoothPermissions();
            requestAllFilesAccessIfNeeded();
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        requestAllFilesAccessIfNeeded();
    }

    private void requestAllFilesAccessIfNeeded() {
        if (mWaitingForMediaPermission
            || mRequestedAllFilesAccess
            || Build.VERSION.SDK_INT < Build.VERSION_CODES.R
            || Environment.isExternalStorageManager()) {
            return;
        }
        mRequestedAllFilesAccess = true;
        Intent intent = new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION);
        intent.setData(Uri.parse("package:" + getPackageName()));
        try {
            startActivity(intent);
        } catch (Exception e) {
            startActivity(new Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION));
        }
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        return super.dispatchKeyEvent(event);
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        View decorView = getWindow().getDecorView();
        if (hasFocus && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            decorView.requestPointerCapture();
        } else if (!hasFocus && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            decorView.releasePointerCapture();
        }
    }

    @Override
    public boolean dispatchTouchEvent(MotionEvent event) {
        return super.dispatchTouchEvent(event);
    }

    @Override
    public boolean dispatchGenericMotionEvent(MotionEvent event) {
        return super.dispatchGenericMotionEvent(event);
    }
}
