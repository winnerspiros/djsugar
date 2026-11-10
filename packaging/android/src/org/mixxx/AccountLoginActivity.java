package org.mixxx;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.Message;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.CookieManager;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
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
    // How often the cookies are looked at: a page that signs in without
    // loading a new page (SoundCloud's) gives no other sign
    private static final long CHECK_MS = 1000;

    private String mService;
    private boolean mAnswered;
    private FrameLayout mFrame;
    private WebViewClient mClient;
    private String mDoneHost;
    private String mCookie;
    private final Handler mHandler = new Handler(Looper.getMainLooper());
    private final Runnable mCheck = new Runnable() {
        @Override
        public void run() {
            if (!checkSignedIn()) {
                mHandler.postDelayed(this, CHECK_MS);
            }
        }
    };

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
        mDoneHost = intent.getStringExtra(EXTRA_DONE_HOST);
        mCookie = intent.getStringExtra(EXTRA_COOKIE) + "=";
        CookieManager.getInstance().setAcceptCookie(true);
        mClient = new WebViewClient() {
            @Override
            public void onPageFinished(WebView page, String url) {
                checkSignedIn();
            }

            @Override
            public boolean shouldOverrideUrlLoading(WebView page, WebResourceRequest request) {
                // The sign-in stays here: links into the service's app
                // (intent://, soundcloud://) open their web page instead
                Uri url = request.getUrl();
                String scheme = url.getScheme();
                if ("http".equals(scheme) || "https".equals(scheme)) {
                    return false;
                }
                String fallback = null;
                if ("intent".equals(scheme)) {
                    String text = url.toString();
                    int start = text.indexOf("S.browser_fallback_url=");
                    if (start >= 0) {
                        start += "S.browser_fallback_url=".length();
                        int end = text.indexOf(';', start);
                        fallback = Uri.decode(end < 0 ? text.substring(start) : text.substring(start, end));
                    }
                }
                if (fallback != null && fallback.startsWith("http")) {
                    page.loadUrl(fallback);
                }
                return true;
            }
        };
        mFrame = new FrameLayout(this);
        WebView view = newPage();
        mFrame.addView(view);
        setContentView(mFrame);
        view.loadUrl(intent.getStringExtra(EXTRA_URL));
        mHandler.postDelayed(mCheck, CHECK_MS);
    }

    /// Answers and closes once the service's sign-in cookie is set
    private boolean checkSignedIn() {
        if (mAnswered) {
            return true;
        }
        CookieManager cookies = CookieManager.getInstance();
        String signedIn = cookies.getCookie("https://" + mDoneHost);
        if (signedIn == null
            || !(signedIn.startsWith(mCookie) || signedIn.contains("; " + mCookie))) {
            return false; // still signed out
        }
        cookies.flush();
        answer(signedIn);
        finish();
        return true;
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
        String agent = settings.getUserAgentString()
                           .replace("; wv)", ")")
                           .replace(" Version/4.0", "");
        if ("soundcloud".equals(mService)) {
            // SoundCloud's phone site only offers its app: the desktop
            // site has the sign-in
            agent = agent.replaceAll("\\(Linux; Android[^)]*\\)", "(X11; Linux x86_64)")
                        .replace(" Mobile", "");
            settings.setUseWideViewPort(true);
            settings.setLoadWithOverviewMode(true);
        }
        settings.setUserAgentString(agent);
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
        mHandler.removeCallbacks(mCheck);
        answer("");
        super.onDestroy();
    }
}
