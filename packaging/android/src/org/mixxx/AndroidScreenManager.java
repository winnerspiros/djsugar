package org.mixxx;

import android.app.Activity;
import android.os.Build;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowManager;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

/**
 * Helper class to force edge-to-edge fullscreen and draw over display cutouts
 * (notches / punch-hole cameras) across Android API levels (API 28 through 35+).
 */
@SuppressWarnings("deprecation")
public class AndroidScreenManager {
    public static void applyFullScreen(final Activity activity) {
        if (activity == null) {
            return;
        }
        final Window window = activity.getWindow();
        if (window == null) {
            return;
        }

        // Draw behind system bars and display cutout on all API levels
        WindowCompat.setDecorFitsSystemWindows(window, false);

        window.addFlags(
                WindowManager.LayoutParams.FLAG_FULLSCREEN
                | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
                | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS);

        // Allow layout into the display cutout area on both short and long edges.
        // API 30+ (and required on API 35 / Android 15): LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS.
        // API 28-29: LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowManager.LayoutParams lp = window.getAttributes();
            if (lp.layoutInDisplayCutoutMode
                    != WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS) {
                lp.layoutInDisplayCutoutMode =
                        WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS;
                window.setAttributes(lp);
            }
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            WindowManager.LayoutParams lp = window.getAttributes();
            if (lp.layoutInDisplayCutoutMode
                    != WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES) {
                lp.layoutInDisplayCutoutMode =
                        WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
                window.setAttributes(lp);
            }
        }

        final View decorView = window.getDecorView();
        if (decorView != null) {
            WindowInsetsControllerCompat controller =
                    WindowCompat.getInsetsController(window, decorView);
            if (controller != null) {
                controller.setSystemBarsBehavior(
                        WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                controller.hide(WindowInsetsCompat.Type.systemBars());
            }

            decorView.setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
        }
    }

    public static void installCutoutAndFullscreenListeners(final Activity activity) {
        if (activity == null || activity.getWindow() == null) {
            return;
        }
        final View decorView = activity.getWindow().getDecorView();
        if (decorView == null) {
            return;
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            decorView.setOnApplyWindowInsetsListener(new View.OnApplyWindowInsetsListener() {
                @Override
                public WindowInsets onApplyWindowInsets(View v, WindowInsets insets) {
                    applyFullScreen(activity);
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                        return WindowInsets.CONSUMED;
                    }
                    return insets.consumeSystemWindowInsets().consumeDisplayCutout();
                }
            });
        }
        decorView.setOnSystemUiVisibilityChangeListener(
                new View.OnSystemUiVisibilityChangeListener() {
                    @Override
                    public void onSystemUiVisibilityChange(int visibility) {
                        applyFullScreen(activity);
                    }
                });
    }
}
