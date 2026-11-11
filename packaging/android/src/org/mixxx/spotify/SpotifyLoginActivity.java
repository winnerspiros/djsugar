package org.mixxx.spotify;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.net.Uri;
import android.os.Bundle;
import android.view.View;
import android.webkit.CookieManager;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Toast;
import java.io.ByteArrayInputStream;

/// Spotify's login page inside DJ Sugar: the app stays in front (Android
/// would cut the network of an app in the background) and the redirect to
/// the login's loopback address is caught here, never loaded
public class SpotifyLoginActivity extends Activity {
    private static final String EXTRA_URL = "url";
    private static final String REDIRECT_PREFIX = "http://127.0.0.1:5588/";
    /// Google, Facebook and Apple refuse their logins inside an app's
    /// WebView ("disallowed_useragent"): those go to the browser
    private static final String[] BROWSER_ONLY_HOSTS = {
        "accounts.google.com", "www.facebook.com", "m.facebook.com", "appleid.apple.com"};

    private boolean mAnswered;
    // The last page (no query: no codes in the log), for "closed at ..."
    private String mLastPage = "";

    /// The login's answer (the redirect URL), or "closed:<last page>" when
    /// the page was closed without one
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
        // The device's saved password (Google Password Manager, Samsung
        // Pass, ...) is offered on the login form
        view.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_YES);
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        CookieManager.getInstance().setAcceptCookie(true);
        view.setWebViewClient(new WebViewClient() {
            // The redirect comes after the login form's POST: Android skips
            // shouldOverrideUrlLoading then, so it is caught in all three
            @Override
            public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest request) {
                final String url = request.getUrl().toString();
                return redirect(url) || toBrowser(url);
            }

            @Override
            public void onPageStarted(WebView v, String url, Bitmap favicon) {
                mLastPage = url.split("[?#]", 2)[0];
                if (toBrowser(url)) {
                    v.stopLoading();
                } else if (!redirect(url)) {
                    super.onPageStarted(v, url, favicon);
                }
            }

            // Any thread; never lets the WebView load the loopback address
            @Override
            public WebResourceResponse shouldInterceptRequest(
                WebView v, WebResourceRequest request) {
                final String url = request.getUrl().toString();
                if (!url.startsWith(REDIRECT_PREFIX)) {
                    return null;
                }
                runOnUiThread(() -> redirect(url));
                return new WebResourceResponse(
                    "text/plain", "utf-8", new ByteArrayInputStream(new byte[0]));
            }
        });
        setContentView(view);
        if (savedInstanceState == null) {
            view.loadUrl(getIntent().getStringExtra(EXTRA_URL));
        } else {
            view.restoreState(savedInstanceState);
        }
    }

    /// A login the WebView can't do: the whole login goes on in the browser
    /// from Spotify's first page (the WebView's cookies are not there). Its
    /// redirect reaches the bridge's loopback port, so the login keeps waiting
    private boolean toBrowser(String url) {
        final String host = Uri.parse(url).getHost();
        boolean browserOnly = false;
        for (String h : BROWSER_ONLY_HOSTS) {
            browserOnly |= h.equalsIgnoreCase(host);
        }
        if (!browserOnly || mAnswered) {
            return false;
        }
        try {
            startActivity(new Intent(Intent.ACTION_VIEW,
                Uri.parse(getIntent().getStringExtra(EXTRA_URL)))
                    .addCategory(Intent.CATEGORY_BROWSABLE)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        } catch (ActivityNotFoundException e) {
            return false; // no browser: the WebView shows Google's refusal
        }
        Toast.makeText(this, "Continue the Spotify login in the browser",
                 Toast.LENGTH_LONG)
            .show();
        mAnswered = true; // not a cancel: the browser answers
        finish();
        return true;
    }

    private boolean redirect(String url) {
        if (!url.startsWith(REDIRECT_PREFIX)) {
            return false; // Spotify's own pages
        }
        answer(url);
        if (!isFinishing()) {
            finish();
        }
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
            answer("closed:" + mLastPage); // without logging in
        }
        super.onDestroy();
    }
}
