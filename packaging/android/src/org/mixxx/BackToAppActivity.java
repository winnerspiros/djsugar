package org.mixxx;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;

/// djsugar://back (the Spotify login page links to it): brings the running
/// DJ Sugar to the front, as its launcher icon would, and closes itself
public class BackToAppActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Intent launch = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (launch != null) {
            launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            startActivity(launch);
        }
        finish();
    }
}
