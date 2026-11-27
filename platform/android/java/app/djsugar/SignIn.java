package app.djsugar;

import android.annotation.SuppressLint;
import android.app.Activity;
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
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

/// A service's own sign-in page inside the app. Once the page is back on the service with the sign-in cookie
/// set, the service's cookies are written to the file the app waits for and the page closes.
public class SignIn extends Activity {
    // A page that signs in without loading another gives no other sign than its cookies.
    private static final long CHECK_MS = 1000;
    private boolean answered;
    private FrameLayout frame;
    private WebViewClient client;
    private String host;
    private String cookie;
    private String out;
    private boolean wide;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final Runnable check = new Runnable() {
        @Override
        public void run() {
            if (!signedIn()) {
                handler.postDelayed(this, CHECK_MS);
            }
        }
    };

    @Override
    protected void onCreate(Bundle saved) {
        super.onCreate(saved);
        host = getIntent().getStringExtra("host");
        cookie = getIntent().getStringExtra("cookie") + "=";
        out = getIntent().getStringExtra("out");
        wide = "1".equals(getIntent().getStringExtra("wide"));
        final String url = getIntent().getStringExtra("url");
        client = new WebViewClient() {
            @Override
            public void onPageFinished(WebView page, String at) {
                signedIn();
            }

            @Override
            public boolean shouldOverrideUrlLoading(WebView page, WebResourceRequest request) {
                // Links into the service's own app open their web page instead: the sign-in stays here.
                Uri to = request.getUrl();
                String scheme = to.getScheme();
                if ("http".equals(scheme) || "https".equals(scheme)) {
                    return false;
                }
                String text = to.toString();
                String key = "S.browser_fallback_url=";
                int start = text.indexOf(key);
                if ("intent".equals(scheme) && start >= 0) {
                    start += key.length();
                    int end = text.indexOf(';', start);
                    String web = Uri.decode(end < 0 ? text.substring(start) : text.substring(start, end));
                    if (web.startsWith("http")) {
                        page.loadUrl(web);
                    }
                }
                return true;
            }
        };
        frame = new FrameLayout(this);
        final WebView view = page();
        frame.addView(view);
        setContentView(frame);
        CookieManager cookies = CookieManager.getInstance();
        cookies.setAcceptCookie(true);
        // A session left from before would sign in at once as the same account, or as nobody: it starts fresh.
        cookies.removeAllCookies(done -> {
            view.loadUrl(url);
            handler.postDelayed(check, CHECK_MS);
        });
    }

    /// Writes the cookies and closes once the service's sign-in cookie is set.
    private boolean signedIn() {
        if (answered) {
            return true;
        }
        CookieManager cookies = CookieManager.getInstance();
        String all = cookies.getCookie("https://" + host);
        if (all == null) {
            return false;
        }
        boolean found = false;
        for (String part : all.split(";")) {
            String pair = part.trim();
            found |= pair.startsWith(cookie) && pair.length() > cookie.length();
        }
        if (!found) {
            return false;
        }
        cookies.flush();
        answer(all);
        finish();
        return true;
    }

    /// The sign-in page, or a popup of it.
    @SuppressLint("SetJavaScriptEnabled")
    private WebView page() {
        WebView view = new WebView(this);
        view.setLayoutParams(new FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        view.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_YES);
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setSupportMultipleWindows(true);
        settings.setJavaScriptCanOpenWindowsAutomatically(true);
        // Google turns down a sign-in from an app's page: the page says it is the browser it is built on.
        String agent = settings.getUserAgentString().replace("; wv)", ")").replace(" Version/4.0", "");
        if (wide) {
            // A phone site that only offers the service's app: the desktop site has the sign-in.
            agent = agent.replaceAll("\\(Linux; Android[^)]*\\)", "(X11; Linux x86_64)").replace(" Mobile", "");
            settings.setUseWideViewPort(true);
            settings.setLoadWithOverviewMode(true);
            settings.setBuiltInZoomControls(true);
            settings.setDisplayZoomControls(false);
        }
        settings.setUserAgentString(agent);
        CookieManager.getInstance().setAcceptThirdPartyCookies(view, true);
        view.setWebViewClient(client);
        view.setWebChromeClient(new WebChromeClient() {
            @Override
            public boolean onCreateWindow(WebView parent, boolean dialog, boolean gesture, Message result) {
                WebView popup = page();
                frame.addView(popup);
                ((WebView.WebViewTransport) result.obj).setWebView(popup);
                result.sendToTarget();
                return true;
            }

            @Override
            public void onCloseWindow(WebView popup) {
                frame.removeView(popup);
                popup.destroy();
            }
        });
        return view;
    }

    @Override
    public boolean onKeyDown(int key, KeyEvent event) {
        if (key != KeyEvent.KEYCODE_BACK) {
            return super.onKeyDown(key, event);
        }
        // Back goes back in the page, then closes a popup, then closes the sign-in.
        int count = frame.getChildCount();
        WebView top = (WebView) frame.getChildAt(count - 1);
        if (top.canGoBack()) {
            top.goBack();
        } else if (count > 1) {
            frame.removeView(top);
            top.destroy();
        } else {
            finish();
        }
        return true;
    }

    /// The answer is written whole under another name first, so the app never reads half of one.
    private void answer(String cookies) {
        if (answered) {
            return;
        }
        answered = true;
        File part = new File(out + ".part");
        try (FileOutputStream file = new FileOutputStream(part)) {
            file.write(cookies.getBytes(StandardCharsets.UTF_8));
        } catch (IOException e) {
            part.delete();
            return;
        }
        if (!part.renameTo(new File(out))) {
            part.delete();
        }
    }

    @Override
    protected void onDestroy() {
        handler.removeCallbacks(check);
        answer("");
        for (int i = frame.getChildCount() - 1; i >= 0; i--) {
            ((WebView) frame.getChildAt(i)).destroy();
        }
        frame.removeAllViews();
        // The cookies are the app's now; none stay in the page for the next one to find.
        CookieManager.getInstance().removeAllCookies(null);
        super.onDestroy();
    }
}
