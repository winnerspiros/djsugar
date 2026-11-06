package org.mixxx;

import android.content.Context;
import android.os.Build;
import android.os.Environment;
import android.os.storage.StorageManager;
import android.os.storage.StorageVolume;
import android.util.Log;

import java.io.File;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/// The storage volumes Android has mounted (internal shared storage, SD
/// cards, USB drives), for the FOLDER source of the library and the music
/// folder picker. See mixxx::androidStorageVolumes().
public class StorageHelper {
    private static final String TAG = "MixxxStorage";

    public static final String KIND_INTERNAL = "internal";
    public static final String KIND_SD = "sd";
    public static final String KIND_USB = "usb";

    /// One line per mounted volume: path, name and kind (internal, sd or
    /// usb), separated by tabs
    public static String[] volumes(Context context) {
        List<String> lines = new ArrayList<>();
        try {
            StorageManager manager = context.getSystemService(StorageManager.class);
            if (manager == null) {
                return new String[0];
            }
            for (StorageVolume volume : manager.getStorageVolumes()) {
                final String state = volume.getState();
                if (!Environment.MEDIA_MOUNTED.equals(state) &&
                        !Environment.MEDIA_MOUNTED_READ_ONLY.equals(state)) {
                    continue;
                }
                final File directory = directoryOf(volume);
                if (directory == null) {
                    continue;
                }
                final String name = volume.getDescription(context);
                lines.add(directory.getAbsolutePath() + "\t" +
                        (name != null ? name.replace('\t', ' ') : directory.getName()) + "\t" +
                        kindOf(volume, name));
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Unable to list the storage volumes", e);
        }
        return lines.toArray(new String[0]);
    }

    private static File directoryOf(StorageVolume volume) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            return volume.getDirectory();
        }
        // Android 10
        if (volume.isPrimary()) {
            return Environment.getExternalStorageDirectory();
        }
        final String uuid = volume.getUuid();
        return uuid != null ? new File("/storage/" + uuid) : null;
    }

    /// Android doesn't tell SD cards and USB drives apart, but names them
    /// e.g. "SD card" / "Tarjeta SD" and "USB drive" / "Unidad USB". A
    /// removable volume whose name doesn't say SD is taken as a USB drive.
    private static String kindOf(StorageVolume volume, String name) {
        if (volume.isPrimary() || !volume.isRemovable()) {
            return KIND_INTERNAL;
        }
        final String upper = name != null ? name.toUpperCase(Locale.ROOT) : "";
        if (upper.matches(".*\\b(MICRO)?SD\\b.*") && !upper.contains("USB")) {
            return KIND_SD;
        }
        return KIND_USB;
    }
}
