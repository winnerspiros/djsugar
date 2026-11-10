package org.mixxx;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.view.View;
import android.webkit.CookieManager;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

/// A service's own sign-in page inside DJ Sugar (YouTube, SoundCloud): once
/// the page lands back on the service with the sign-in cookie set, the
/// service's cookies go to the app and the page closes. The device's saved
/// passwords (Google Password Manager, Samsung Pass) are offered on the form
public class AccountLoginActivity extends Activity {
    private static final String EXTRA_SERVICE = "service";
    private static final String EXTRA_URL = "url";
    private static final String EXTRA_DONE_HOST = "doneHost";
    private static final String EXTRA_COOKIE = "cookie";

    private String mService;
    private boolean mAnswered;

    /// The service's cookies after a sign-in, "" when the page was closed
    private static native void nativeSignedIn(String service, String cookies);

    /// `doneHost`: the page the sign-in ends on; `cookie`: the cookie that
    /// is only there when signed in
    public static void start(
        Context context, String service, String url, String doneHost, String cookie) {
        Intent intent = new Intent(context, AccountLoginActivity.class);
        intent.putExtra(EXTRA_SERVICE, service);
        intent.putExtra(EXTRA_URL, url);
        intent.putExtra(EXTRA_DONE_HOST, doneHost);
        intent.putExtra(EXTRA_COOKIE, cookie);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }

    @SuppressLint("SetJavaScriptEnabled") // the sign-in pages need it
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Intent intent = getIntent();
        mService = intent.getStringExtra(EXTRA_SERVICE);
        final String doneHost = intent.getStringExtra(EXTRA_DONE_HOST);
        final String cookie = intent.getStringExtra(EXTRA_COOKIE) + "=";
        WebView view = new WebView(this);
        view.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_YES);
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        CookieManager cookies = CookieManager.getInstance();
        cookies.setAcceptCookie(true);
        cookies.setAcceptThirdPartyCookies(view, true);
        view.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageFinished(WebView page, String url) {
                if (mAnswered || !doneHost.equals(Uri.parse(url).getHost())) {
                    return;
                }
                String signedIn = cookies.getCookie("https://" + doneHost);
                if (signedIn == null || !(signedIn.startsWith(cookie) || signedIn.contains("; " + cookie))) {
                    return; // still signed out (the start page itself)
                }
                cookies.flush();
                answer(signedIn);
                finish();
            }
        });
        setContentView(view);
        view.loadUrl(intent.getStringExtra(EXTRA_URL));
    }

    private void answer(String cookies) {
        if (!mAnswered) {
            mAnswered = true;
            nativeSignedIn(mService, cookies);
        }
    }

    @Override
    protected void onDestroy() {
        answer("");
        super.onDestroy();
    }
}
