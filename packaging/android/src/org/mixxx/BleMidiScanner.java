package org.mixxx;

import android.app.Activity;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.content.Context;
import android.util.Log;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * BLE MIDI Scanner bridge for C++ JNI and QmlMobileControllersProxy.
 */
public class BleMidiScanner {
    private static final String TAG = "MixxxBleScan";
    private static Context sContext;
    private static volatile boolean sConnected = false;

    public static boolean startScan(Context context, String serviceUuid) {
        sContext = context;
        boolean result = BleMidiController.startScan(context);
        Log.i(TAG, "startScan() -> " + result);
        return result;
    }

    public static void stopScan() {
        Log.i(TAG, "stopScan()");
        BleMidiController.stopScan();
    }

    public static boolean isScanning() {
        return BleMidiController.isScanning();
    }

    public static boolean hasRequiredPermissions(Context context) {
        return BleMidiController.hasRequiredPermissions(context);
    }

    public static void requestPermissionsIfNeeded(Activity activity) {
        BleMidiController.requestPermissionsIfNeeded(activity);
    }

    public static boolean isBluetoothEnabled(Context context) {
        return BleMidiController.isBluetoothEnabled(context);
    }

    /**
     * Get discovered BLE devices for C++ BleMidiEnumerator.
     * Format from BleMidiController.getDiscoveredDevices() is:
     *   "address|name|isMidi|isConnected"
     */
    public static List<Map<String, String>> getDiscoveredDevices() {
        List<Map<String, String>> result = new ArrayList<>();
        List<String> devices = BleMidiController.getDiscoveredDevices();
        for (String deviceStr : devices) {
            String[] parts = deviceStr.split("\\|", 4);
            if (parts.length >= 2) {
                Map<String, String> deviceInfo = new HashMap<>();
                deviceInfo.put("address", parts[0]);
                deviceInfo.put("name", parts[1]);
                deviceInfo.put("isMidi", parts.length >= 3 ? parts[2] : "0");
                deviceInfo.put("connected", parts.length >= 4 ? parts[3] : "0");
                result.add(deviceInfo);
            }
        }
        return result;
    }

    /**
     * Returns discovered BLE devices as an array of "address|name|isMidi|connected"
     * strings for QmlMobileControllersProxy.
     */
    public static String[] getDiscoveredDeviceStrings() {
        List<String> devices = BleMidiController.getDiscoveredDevices();
        return devices.toArray(new String[0]);
    }

    public static boolean connectDevice(Context context, String address, String name) {
        sContext = context;
        boolean ok = BleMidiController.connectDevice(context, address, name);
        if (ok) {
            sConnected = true;
        }
        return ok;
    }

    public static void disconnectDevice(String address) {
        BleMidiController.disconnectDevice(address);
    }

    public static boolean isConnected() {
        return sConnected || BleMidiController.isConnected();
    }

    public static void setConnected(boolean connected) {
        sConnected = connected;
    }

    public static void writeMidiData(String deviceAddress, String hexData) {
        BleMidiController.writeMidiData(deviceAddress, hexData);
    }

    public static void disconnectAll() {
        BleMidiController.disconnectAll();
        sConnected = false;
    }

    public static BluetoothDevice getDeviceByAddress(String address) {
        try {
            if (sContext == null) {
                return null;
            }
            BluetoothManager bluetoothManager =
                (BluetoothManager) sContext.getSystemService(Context.BLUETOOTH_SERVICE);
            if (bluetoothManager != null && bluetoothManager.getAdapter() != null) {
                return bluetoothManager.getAdapter().getRemoteDevice(address);
            }
        } catch (Exception e) {
            Log.e(TAG, "Failed to get device by address: " + e.getMessage());
        }
        return null;
    }
}
