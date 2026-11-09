package org.mixxx.spotify;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import com.spotify.sdk.android.auth.AuthorizationClient;
import com.spotify.sdk.android.auth.AuthorizationRequest;
import com.spotify.sdk.android.auth.AuthorizationResponse;
import com.spotify.sdk.android.auth.PKCEInformation;
import com.spotify.sdk.android.auth.PKCEInformationFactory;
import com.spotify.sdk.android.auth.TokenExchangeRequest;
import com.spotify.sdk.android.auth.TokenExchangeResponse;

/// Spotify login, two ways:
/// - app: the installed Spotify app is asked for the account it is signed in
///   to (one "Agree" tap, nothing to type)
/// - web: Spotify's login page in the browser (any account, or no Spotify
///   app); it comes back through djsugar://spotify-callback to this activity
/// The authorization code is exchanged with PKCE (no client secret in the app)
/// and the tokens are handed to C++ (QmlSpotifyProxy), which browses with the
/// Web API and downloads the audio with librespot (lib/spotify-bridge).
public class SpotifyAuthActivity extends Activity {
    private static final int REQUEST_CODE = 0x5907;
    private static final String EXTRA_CLIENT_ID = "clientId";
    private static final String EXTRA_REDIRECT_URI = "redirectUri";
    private static final String EXTRA_BROWSER = "browser";
    // The web login can outlive this process (the browser is in front):
    // what the answer is checked against is kept on disk until it arrives
    private static final String PREFS = "spotify_login";
    private static final String KEY_VERIFIER = "verifier";
    private static final String KEY_STATE = "state";
    private static final String KEY_CLIENT_ID = "clientId";
    private static final String KEY_REDIRECT_URI = "redirectUri";
    private static final String[] SCOPES = {
        // Playing the audio (librespot, Spotify Premium)
        "streaming",
        "user-read-private",
        "user-library-read",
        "playlist-read-private",
        "playlist-read-collaborative",
        "user-top-read",
        "user-read-recently-played",
    };

    private String mVerifier;
    private String mClientId;
    private String mRedirectUri;
    private boolean mDone;

    /// Tokens on success, or the error ("" access token). Runs on any thread.
    private static native void nativeAuthResult(
        String accessToken, String refreshToken, int expiresIn, String error);

    /// Called from C++
    public static void login(
            Context context, String clientId, String redirectUri, boolean browser) {
        Intent intent = new Intent(context, SpotifyAuthActivity.class);
        intent.putExtra(EXTRA_CLIENT_ID, clientId);
        intent.putExtra(EXTRA_REDIRECT_URI, redirectUri);
        intent.putExtra(EXTRA_BROWSER, browser || !isSpotifyInstalled(context));
        if (!(context instanceof Activity)) {
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        }
        context.startActivity(intent);
    }

    /// Called from C++: true when the Spotify app is installed
    public static boolean isSpotifyInstalled(Context context) {
        try {
            context.getPackageManager().getPackageInfo("com.spotify.music", 0);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        if (getIntent().getData() != null) {
            onRedirect(getIntent().getData()); // web login, app restarted
            return;
        }
        SharedPreferences prefs = getSharedPreferences(PREFS, MODE_PRIVATE);
        if (savedInstanceState != null) {
            // Recreated while Spotify was in front: the login is already
            // running, its result still arrives here
            mClientId = prefs.getString(KEY_CLIENT_ID, "");
            mRedirectUri = prefs.getString(KEY_REDIRECT_URI, "");
            mVerifier = prefs.getString(KEY_VERIFIER, "");
            return;
        }
        mClientId = getIntent().getStringExtra(EXTRA_CLIENT_ID);
        mRedirectUri = getIntent().getStringExtra(EXTRA_REDIRECT_URI);
        PKCEInformation pkce;
        try {
            pkce = PKCEInformationFactory.create();
        } catch (Exception e) {
            finishWith(null, null, 0, "PKCE: " + e.getMessage());
            return;
        }
        mVerifier = pkce.getVerifier();
        final String state = Long.toHexString(new java.security.SecureRandom().nextLong());
        prefs.edit()
                .putString(KEY_CLIENT_ID, mClientId)
                .putString(KEY_REDIRECT_URI, mRedirectUri)
                .putString(KEY_VERIFIER, mVerifier)
                .putString(KEY_STATE, state)
                .apply();
        AuthorizationRequest request = new AuthorizationRequest.Builder(
                mClientId, AuthorizationResponse.Type.CODE, mRedirectUri)
                                               .setScopes(SCOPES)
                                               .setPkceInformation(pkce)
                                               .setState(state)
                                               .build();
        if (getIntent().getBooleanExtra(EXTRA_BROWSER, false)) {
            AuthorizationClient.openLoginInBrowser(this, request);
        } else {
            AuthorizationClient.openLoginActivity(this, REQUEST_CODE, request);
        }
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        if (intent.getData() != null) {
            onRedirect(intent.getData());
        }
    }

    /// The web login's answer (djsugar://spotify-callback?code=...&state=...)
    private void onRedirect(Uri uri) {
        SharedPreferences prefs = getSharedPreferences(PREFS, MODE_PRIVATE);
        mClientId = prefs.getString(KEY_CLIENT_ID, "");
        mRedirectUri = prefs.getString(KEY_REDIRECT_URI, "");
        mVerifier = prefs.getString(KEY_VERIFIER, "");
        final String state = prefs.getString(KEY_STATE, "");
        if (state.isEmpty() || !state.equals(uri.getQueryParameter("state"))) {
            // Not an answer to a login started here
            finishWith(null, null, 0, "unexpected login answer");
            return;
        }
        handle(AuthorizationResponse.fromUri(uri));
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQUEST_CODE) {
            return;
        }
        handle(AuthorizationClient.getResponse(resultCode, data));
    }

    private void handle(AuthorizationResponse response) {
        if (mDone) {
            return; // the web answer came first (the app login fell back to the web)
        }
        switch (response.getType()) {
            case CODE:
                exchange(response.getCode());
                break;
            case TOKEN: // older Spotify apps convert the request to a token
                finishWith(response.getAccessToken(), response.getRefreshToken(),
                    response.getExpiresIn(), null);
                break;
            case ERROR:
                finishWith(null, null, 0, response.getError());
                break;
            default:
                finishWith(null, null, 0, "cancelled");
                break;
        }
    }

    private void exchange(String code) {
        final TokenExchangeRequest request = new TokenExchangeRequest.Builder()
                                                 .setClientId(mClientId)
                                                 .setCode(code)
                                                 .setRedirectUri(mRedirectUri)
                                                 .setCodeVerifier(mVerifier)
                                                 .build();
        new Thread(() -> {
            TokenExchangeResponse tokens;
            try {
                tokens = request.execute();
            } catch (Exception e) {
                runOnUiThread(() -> finishWith(null, null, 0, e.getMessage()));
                return;
            }
            final TokenExchangeResponse result = tokens;
            runOnUiThread(() -> {
                if (result.isSuccess()) {
                    finishWith(result.getAccessToken(), result.getRefreshToken(),
                        result.getExpiresIn(), null);
                } else {
                    finishWith(null, null, 0, result.getError() + ": " + result.getErrorDescription());
                }
            });
        }, "SpotifyTokenExchange").start();
    }

    private void finishWith(String access, String refresh, int expiresIn, String error) {
        if (mDone) {
            return;
        }
        mDone = true;
        getSharedPreferences(PREFS, MODE_PRIVATE).edit().clear().apply();
        try {
            nativeAuthResult(access != null ? access : "",
                refresh != null ? refresh : "",
                expiresIn,
                error != null ? error : "");
        } catch (UnsatisfiedLinkError e) {
            // The native library isn't loaded (yet)
        }
        finish();
        noCloseAnimation();
    }

    // overrideActivityTransition() is API 34+; minSdk is lower
    @SuppressWarnings("deprecation")
    private void noCloseAnimation() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            overrideActivityTransition(OVERRIDE_TRANSITION_CLOSE, 0, 0);
        } else {
            overridePendingTransition(0, 0);
        }
    }
}
