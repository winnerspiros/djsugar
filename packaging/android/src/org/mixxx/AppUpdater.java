package org.mixxx;

import android.app.DownloadManager;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.provider.Settings;
import android.util.Log;
import androidx.core.content.FileProvider;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

/// Network and installer side of the in-app update (QmlAppUpdateProxy). The
/// TLS backend of Qt has no OpenSSL in the APK, so the requests are made by
/// Android. The APK is downloaded by the DownloadManager, which shows its
/// progress in a notification and goes on while Mixxx is in the background.
public class AppUpdater {
    private static final String TAG = "MixxxAppUpdater";
    private static final String APK_MIME_TYPE = "application/vnd.android.package-archive";
    private static final String UPDATES_DIR = "updates";
    private static final int CONNECT_TIMEOUT_MILLIS = 10000;
    private static final int READ_TIMEOUT_MILLIS = 15000;

    public static final int INSTALL_STARTED = 0;
    public static final int INSTALL_NEEDS_PERMISSION = 1;
    public static final int INSTALL_FAILED = 2;

    /// The body of a GET request, or null if it failed. Blocking: not to be
    /// called on the main thread of Android.
    public static String fetch(String url, String userAgent) {
        HttpURLConnection connection = null;
        try {
            connection = (HttpURLConnection) new URL(url).openConnection();
            connection.setConnectTimeout(CONNECT_TIMEOUT_MILLIS);
            connection.setReadTimeout(READ_TIMEOUT_MILLIS);
            connection.setRequestProperty("Accept", "application/vnd.github+json");
            connection.setRequestProperty("User-Agent", userAgent);
            final int responseCode = connection.getResponseCode();
            if (responseCode != HttpURLConnection.HTTP_OK) {
                Log.w(TAG, "GET " + url + " returned " + responseCode);
                return null;
            }
            try (InputStream input = connection.getInputStream()) {
                ByteArrayOutputStream output = new ByteArrayOutputStream();
                byte[] buffer = new byte[16384];
                int length;
                while ((length = input.read(buffer)) != -1) {
                    output.write(buffer, 0, length);
                }
                return new String(output.toByteArray(), StandardCharsets.UTF_8);
            }
        } catch (IOException | ClassCastException e) {
            Log.w(TAG, "GET " + url + " failed", e);
            return null;
        } finally {
            if (connection != null) {
                connection.disconnect();
            }
        }
    }

    /// The folder of the downloaded APKs (in the app's external files)
    public static String updatesDir(Context context) {
        File dir = new File(context.getExternalFilesDir(null), UPDATES_DIR);
        dir.mkdirs();
        return dir.getAbsolutePath();
    }

    /// Starts downloading url to fileName in updatesDir(). Returns the id of
    /// the download, or -1.
    public static long startDownload(Context context, String url, String fileName, String title) {
        DownloadManager manager =
            (DownloadManager) context.getSystemService(Context.DOWNLOAD_SERVICE);
        if (manager == null) {
            return -1;
        }
        // The DownloadManager doesn't overwrite, it would add a suffix
        new File(updatesDir(context), fileName).delete();
        try {
            DownloadManager.Request request = new DownloadManager.Request(Uri.parse(url));
            request.setTitle(title);
            request.setMimeType(APK_MIME_TYPE);
            request.setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE);
            request.setDestinationInExternalFilesDir(context, null, UPDATES_DIR + "/" + fileName);
            return manager.enqueue(request);
        } catch (IllegalArgumentException | IllegalStateException | SecurityException e) {
            Log.w(TAG, "Unable to start downloading " + url, e);
            return -1;
        }
    }

    /// {status, bytes downloaded, total bytes} of a download: status is one
    /// of DownloadManager.STATUS_*, or -1 if the download is unknown
    public static long[] queryDownload(Context context, long id) {
        long[] result = {-1, 0, 0};
        DownloadManager manager =
            (DownloadManager) context.getSystemService(Context.DOWNLOAD_SERVICE);
        if (manager == null) {
            return result;
        }
        try (Cursor cursor = manager.query(new DownloadManager.Query().setFilterById(id))) {
            if (cursor != null && cursor.moveToFirst()) {
                result[0] = cursor.getInt(
                    cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS));
                result[1] = cursor.getLong(cursor.getColumnIndexOrThrow(
                    DownloadManager.COLUMN_BYTES_DOWNLOADED_SO_FAR));
                result[2] = cursor.getLong(
                    cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_TOTAL_SIZE_BYTES));
            }
        } catch (IllegalArgumentException e) {
            Log.w(TAG, "Unable to query download " + id, e);
        }
        return result;
    }

    public static void cancelDownload(Context context, long id) {
        DownloadManager manager =
            (DownloadManager) context.getSystemService(Context.DOWNLOAD_SERVICE);
        if (manager != null) {
            manager.remove(id);
        }
    }

    /// Opens the Android installer for the APK at path. Without the
    /// permission to install apps, opens its setting instead and returns
    /// INSTALL_NEEDS_PERMISSION.
    public static int install(Context context, String path) {
        try {
            if (!context.getPackageManager().canRequestPackageInstalls()) {
                Intent settings = new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:" + context.getPackageName()));
                settings.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                context.startActivity(settings);
                return INSTALL_NEEDS_PERMISSION;
            }
            Uri uri = FileProvider.getUriForFile(
                context, context.getPackageName() + ".qtprovider", new File(path));
            Intent intent = new Intent(Intent.ACTION_VIEW);
            intent.setDataAndType(uri, APK_MIME_TYPE);
            intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_ACTIVITY_NEW_TASK);
            context.startActivity(intent);
            return INSTALL_STARTED;
        } catch (RuntimeException e) {
            Log.w(TAG, "Unable to open the installer for " + path, e);
            return INSTALL_FAILED;
        }
    }
}
