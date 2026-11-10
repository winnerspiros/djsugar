package org.mixxx;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Message;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.CookieManager;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;
import androidx.webkit.WebSettingsCompat;
import androidx.webkit.WebViewFeature;

/// A service's own sign-in page inside DJ Sugar (YouTube, SoundCloud): once
/// the page lands back on the service with the sign-in cookie set, the
/// service's cookies go to the app and the page closes. The device's saved
/// passwords and passkeys (Google Password Manager, Samsung Pass) are
/// offered on the form. "Continue with Google/Facebook" opens as a popup
/// over the page, as in a browser, and once signed in to Google here, the
/// other service's Google button only asks which account
public class AccountLoginActivity extends Activity {
    private static final String EXTRA_SERVICE = "service";
    private static final String EXTRA_URL = "url";
    private static final String EXTRA_DONE_HOST = "doneHost";
    private static final String EXTRA_COOKIE = "cookie";

    private String mService;
    private boolean mAnswered;
    private FrameLayout mFrame;
    private WebViewClient mClient;

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

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Intent intent = getIntent();
        mService = intent.getStringExtra(EXTRA_SERVICE);
        final String doneHost = intent.getStringExtra(EXTRA_DONE_HOST);
        final String cookie = intent.getStringExtra(EXTRA_COOKIE) + "=";
        CookieManager cookies = CookieManager.getInstance();
        cookies.setAcceptCookie(true);
        mClient = new WebViewClient() {
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
        };
        mFrame = new FrameLayout(this);
        WebView view = newPage();
        mFrame.addView(view);
        setContentView(mFrame);
        view.loadUrl(intent.getStringExtra(EXTRA_URL));
    }

    /// The sign-in page, or a popup of it
    @SuppressLint("SetJavaScriptEnabled") // the sign-in pages need it
    private WebView newPage() {
        WebView view = new WebView(this);
        view.setLayoutParams(new FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        view.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_YES);
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setSupportMultipleWindows(true);
        settings.setJavaScriptCanOpenWindowsAutomatically(true);
        // Google refuses its sign-in in an app's page ("; wv"): the page
        // presents itself as the Chrome it is built on
        settings.setUserAgentString(settings.getUserAgentString()
                .replace("; wv)", ")")
                .replace(" Version/4.0", ""));
        // Passkeys of the device's password manager
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_AUTHENTICATION)) {
            WebSettingsCompat.setWebAuthenticationSupport(
                settings, WebSettingsCompat.WEB_AUTHENTICATION_SUPPORT_FOR_BROWSER);
        }
        CookieManager.getInstance().setAcceptThirdPartyCookies(view, true);
        view.setWebViewClient(mClient);
        view.setWebChromeClient(new WebChromeClient() {
            @Override
            public boolean onCreateWindow(
                WebView parent, boolean dialog, boolean userGesture, Message result) {
                WebView popup = newPage();
                mFrame.addView(popup);
                ((WebView.WebViewTransport) result.obj).setWebView(popup);
                result.sendToTarget();
                return true;
            }

            @Override
            public void onCloseWindow(WebView popup) {
                mFrame.removeView(popup);
                popup.destroy();
            }
        });
        return view;
    }

    @Override
    public boolean onKeyDown(int keyCode, KeyEvent event) {
        if (keyCode != KeyEvent.KEYCODE_BACK) {
            return super.onKeyDown(keyCode, event);
        }
        // Back closes a popup first, then goes back in the page, then
        // closes the sign-in
        int count = mFrame.getChildCount();
        WebView top = (WebView) mFrame.getChildAt(count - 1);
        if (top.canGoBack()) {
            top.goBack();
        } else if (count > 1) {
            mFrame.removeView(top);
            top.destroy();
        } else {
            finish();
        }
        return true;
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
