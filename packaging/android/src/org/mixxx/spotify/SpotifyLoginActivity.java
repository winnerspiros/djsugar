package org.mixxx.spotify;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.os.Bundle;
import android.webkit.CookieManager;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

/// Spotify's login page inside DJ Sugar: the app stays in front (Android
/// would cut the network of an app in the background) and the redirect to
/// the login's loopback address is caught here, never loaded
public class SpotifyLoginActivity extends Activity {
    private static final String EXTRA_URL = "url";
    private static final String REDIRECT_PREFIX = "http://127.0.0.1:5588/";

    private boolean mAnswered;

    /// The login's answer (the redirect URL), or "" when the page was closed
    private static native void nativeRedirect(String url);

    public static void start(Context context, String url) {
        Intent intent = new Intent(context, SpotifyLoginActivity.class);
        intent.putExtra(EXTRA_URL, url);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }

    @SuppressLint("SetJavaScriptEnabled") // Spotify's login page needs it
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        WebView view = new WebView(this);
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        CookieManager.getInstance().setAcceptCookie(true);
        view.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest request) {
                return redirect(request.getUrl().toString());
            }
        });
        setContentView(view);
        if (savedInstanceState == null) {
            view.loadUrl(getIntent().getStringExtra(EXTRA_URL));
        } else {
            view.restoreState(savedInstanceState);
        }
    }

    private boolean redirect(String url) {
        if (!url.startsWith(REDIRECT_PREFIX)) {
            return false; // Spotify's own pages
        }
        answer(url);
        finish();
        return true;
    }

    private void answer(String url) {
        if (mAnswered) {
            return;
        }
        mAnswered = true;
        try {
            nativeRedirect(url);
        } catch (UnsatisfiedLinkError e) {
            // Mixxx not loaded: nobody waits for the answer
        }
    }

    @Override
    protected void onDestroy() {
        if (isFinishing()) {
            answer(""); // closed without logging in
        }
        super.onDestroy();
    }
}
