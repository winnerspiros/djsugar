package org.mixxx;

import android.app.Activity;
import android.graphics.Color;
import android.graphics.Rect;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.view.DisplayCutout;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewTreeObserver;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowManager;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

/**
 * Forces true edge-to-edge fullscreen across the display cutout (camera notch /
 * punch-hole) on Android API 28 through 35+ with Qt 6.9.
 *
 * Qt 6.9's QtDisplayManager.updateFullScreen() and QtActivityDelegate install
 * their own WindowInsets listeners on QtLayout and can reset
 * layoutInDisplayCutoutMode after Activity.onCreate(). This helper re-asserts
 * LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS (API 30+) / SHORT_EDGES (API 28-29) and
 * strips cutout insets/padding from both the DecorView and Qt's child views.
 */
@SuppressWarnings("deprecation")
public class AndroidScreenManager {
    private static final Handler sMainHandler = new Handler(Looper.getMainLooper());
    private static boolean sApplying = false;

    private static final View.OnApplyWindowInsetsListener sConsumeInsetsListener =
        new View.OnApplyWindowInsetsListener() {
            @Override
            public WindowInsets onApplyWindowInsets(View v, WindowInsets insets) {
                if (v != null && (v.getPaddingLeft() != 0 || v.getPaddingTop() != 0 || v.getPaddingRight() != 0 || v.getPaddingBottom() != 0)) {
                    v.setPadding(0, 0, 0, 0);
                }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    return WindowInsets.CONSUMED;
                }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                    return insets.consumeSystemWindowInsets().consumeDisplayCutout();
                }
                return insets.consumeSystemWindowInsets();
            }
        };

    private static void clearInsetsAndPaddingRecursive(View view) {
        if (view == null) {
            return;
        }
        view.setFitsSystemWindows(false);
        if (view.getPaddingLeft() != 0 || view.getPaddingTop() != 0
            || view.getPaddingRight() != 0 || view.getPaddingBottom() != 0) {
            view.setPadding(0, 0, 0, 0);
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            view.setOnApplyWindowInsetsListener(sConsumeInsetsListener);
        }
        if (view instanceof ViewGroup) {
            ViewGroup group = (ViewGroup) view;
            final int count = group.getChildCount();
            for (int i = 0; i < count; i++) {
                clearInsetsAndPaddingRecursive(group.getChildAt(i));
            }
        }
    }

    /**
     * The camera cutouts (notch / punch hole) of the display in the pixel
     * coordinates of the window in its current rotation, flattened as
     * left, top, right, bottom for each cutout. Empty without a cutout.
     * Read from the root window insets, which no view of the hierarchy can
     * consume, so the UI can draw edge to edge and keep its controls clear of
     * the camera.
     */
    public static int[] getCutoutRects(final Activity activity) {
        if (activity == null || Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return new int[0];
        }
        final Window window = activity.getWindow();
        final View decorView = window != null ? window.getDecorView() : null;
        final WindowInsets insets = decorView != null ? decorView.getRootWindowInsets() : null;
        final DisplayCutout cutout = insets != null ? insets.getDisplayCutout() : null;
        if (cutout == null) {
            return new int[0];
        }
        final java.util.List<Rect> rects = cutout.getBoundingRects();
        final int[] out = new int[rects.size() * 4];
        for (int i = 0; i < rects.size(); i++) {
            final Rect r = rects.get(i);
            out[i * 4] = r.left;
            out[i * 4 + 1] = r.top;
            out[i * 4 + 2] = r.right;
            out[i * 4 + 3] = r.bottom;
        }
        return out;
    }

    public static void applyFullScreen(final Activity activity) {
        if (activity == null || sApplying) {
            return;
        }
        if (Looper.myLooper() != Looper.getMainLooper()) {
            activity.runOnUiThread(() -> applyFullScreen(activity));
            return;
        }
        final Window window = activity.getWindow();
        if (window == null) {
            return;
        }
        sApplying = true;
        try {
            WindowCompat.setDecorFitsSystemWindows(window, false);

            window.addFlags(
                WindowManager.LayoutParams.FLAG_FULLSCREEN
                | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
                | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
                | WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS);
            window.clearFlags(WindowManager.LayoutParams.FLAG_FORCE_NOT_FULLSCREEN);
            window.setStatusBarColor(Color.TRANSPARENT);
            window.setNavigationBarColor(Color.TRANSPARENT);
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                window.setStatusBarContrastEnforced(false);
                window.setNavigationBarContrastEnforced(false);
            }

            final int desiredCutoutMode =
                (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R)
                ? WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
                : WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                WindowManager.LayoutParams lp = window.getAttributes();
                if (lp.layoutInDisplayCutoutMode != desiredCutoutMode) {
                    lp.layoutInDisplayCutoutMode = desiredCutoutMode;
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
                    controller.hide(
                        WindowInsetsCompat.Type.systemBars()
                        | WindowInsetsCompat.Type.displayCutout());
                }

                final int flags =
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE;
                if (decorView.getSystemUiVisibility() != flags) {
                    decorView.setSystemUiVisibility(flags);
                }

                clearInsetsAndPaddingRecursive(decorView);
            }
        } finally {
            sApplying = false;
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
        clearInsetsAndPaddingRecursive(decorView);

        decorView.setOnSystemUiVisibilityChangeListener(
            visibility -> applyFullScreen(activity));

        if (decorView instanceof ViewGroup) {
            ((ViewGroup) decorView).setOnHierarchyChangeListener(new ViewGroup.OnHierarchyChangeListener() {
                @Override
                public void onChildViewAdded(View parent, View child) {
                    clearInsetsAndPaddingRecursive(child);
                    applyFullScreen(activity);
                }

                @Override
                public void onChildViewRemoved(View parent, View child) {
                }
            });
        }

        decorView.getViewTreeObserver().addOnGlobalLayoutListener(
            new ViewTreeObserver.OnGlobalLayoutListener() {
                @Override
                public void onGlobalLayout() {
                    applyFullScreen(activity);
                }
            });

        // Re-apply after Qt's QAndroidPlatformWindow initialization settles
        sMainHandler.postDelayed(() -> applyFullScreen(activity), 150);
        sMainHandler.postDelayed(() -> applyFullScreen(activity), 600);
        sMainHandler.postDelayed(() -> applyFullScreen(activity), 1500);
    }
}
