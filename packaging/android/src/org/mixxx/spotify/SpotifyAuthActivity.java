package org.mixxx.spotify;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.os.Bundle;

import com.spotify.sdk.android.auth.AuthorizationClient;
import com.spotify.sdk.android.auth.AuthorizationRequest;
import com.spotify.sdk.android.auth.AuthorizationResponse;
import com.spotify.sdk.android.auth.PKCEInformation;
import com.spotify.sdk.android.auth.PKCEInformationFactory;
import com.spotify.sdk.android.auth.TokenExchangeRequest;
import com.spotify.sdk.android.auth.TokenExchangeResponse;

/// Spotify login without a browser: the installed Spotify app is asked for the
/// account it is signed in to (one "Agree" tap, nothing to type). Without the
/// app, the Spotify auth library shows its own in-app login page instead.
/// The authorization code is exchanged with PKCE (no client secret in the app)
/// and the tokens are handed to C++ (QmlSpotifyProxy), which browses with the
/// Web API and downloads the audio with librespot (lib/spotify-bridge).
public class SpotifyAuthActivity extends Activity {
    private static final int REQUEST_CODE = 0x5907;
    private static final String EXTRA_CLIENT_ID = "clientId";
    private static final String EXTRA_REDIRECT_URI = "redirectUri";
    private static final String STATE_PKCE = "pkce";
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

    private PKCEInformation mPkce;
    private String mClientId;
    private String mRedirectUri;

    /// Tokens on success, or the error ("" access token). Runs on any thread.
    private static native void nativeAuthResult(
            String accessToken, String refreshToken, int expiresIn, String error);

    /// Called from C++
    public static void login(Context context, String clientId, String redirectUri) {
        Intent intent = new Intent(context, SpotifyAuthActivity.class);
        intent.putExtra(EXTRA_CLIENT_ID, clientId);
        intent.putExtra(EXTRA_REDIRECT_URI, redirectUri);
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
        mClientId = getIntent().getStringExtra(EXTRA_CLIENT_ID);
        mRedirectUri = getIntent().getStringExtra(EXTRA_REDIRECT_URI);
        if (savedInstanceState != null) {
            // Recreated while the Spotify app was in front: the login is
            // already running, its result still arrives here
            mPkce = savedInstanceState.getParcelable(STATE_PKCE);
            return;
        }
        try {
            mPkce = PKCEInformationFactory.create();
        } catch (Exception e) {
            finishWith(null, null, 0, "PKCE: " + e.getMessage());
            return;
        }
        AuthorizationRequest request = new AuthorizationRequest.Builder(
                mClientId, AuthorizationResponse.Type.CODE, mRedirectUri)
                .setScopes(SCOPES)
                .setPkceInformation(mPkce)
                .build();
        AuthorizationClient.openLoginActivity(this, REQUEST_CODE, request);
    }

    @Override
    protected void onSaveInstanceState(Bundle outState) {
        super.onSaveInstanceState(outState);
        outState.putParcelable(STATE_PKCE, mPkce);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQUEST_CODE) {
            return;
        }
        AuthorizationResponse response = AuthorizationClient.getResponse(resultCode, data);
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
                .setCodeVerifier(mPkce != null ? mPkce.getVerifier() : "")
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
                    finishWith(null, null, 0, result.getError() + ": "
                            + result.getErrorDescription());
                }
            });
        }, "SpotifyTokenExchange").start();
    }

    private void finishWith(String access, String refresh, int expiresIn, String error) {
        try {
            nativeAuthResult(access != null ? access : "",
                    refresh != null ? refresh : "",
                    expiresIn,
                    error != null ? error : "");
        } catch (UnsatisfiedLinkError e) {
            // The native library isn't loaded (yet)
        }
        finish();
        overridePendingTransition(0, 0);
    }
}
