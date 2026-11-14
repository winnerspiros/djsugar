package org.mixxx;

import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.content.Context;
import android.content.pm.PackageManager;
import android.hardware.usb.UsbDevice;
import android.media.midi.MidiDevice;
import android.media.midi.MidiDeviceInfo;
import android.media.midi.MidiManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.HandlerThread;
import android.util.Log;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/// Thin android.media.midi wrapper used by AndroidMidiEnumerator and
/// AndroidMidiController. Lists devices, opens them, reports hotplug events.
/// MIDI I/O happens in native code through AMidi.
public class MidiHelper {
    private static final String TAG = "MixxxMidi";
    private static MidiManager sManager = null;
    private static Handler sHandler = null;

    private static native void midiDevicesChanged();

    private static void notifyDevicesChanged() {
        try {
            midiDevicesChanged();
        } catch (UnsatisfiedLinkError e) {
            Log.v(TAG, "Native library not loaded yet, skipping rescan");
        }
    }

    public static synchronized boolean init(Context context) {
        if (sManager != null) {
            return true;
        }
        if (!context.getPackageManager().hasSystemFeature(PackageManager.FEATURE_MIDI)) {
            Log.w(TAG, "This device has no MIDI support");
            return false;
        }
        sManager = (MidiManager) context.getSystemService(Context.MIDI_SERVICE);
        if (sManager == null) {
            Log.w(TAG, "MidiManager unavailable");
            return false;
        }
        // Callbacks run on a dedicated thread so that a blocked Qt main
        // thread can never deadlock openDevice().
        HandlerThread thread = new HandlerThread("MixxxMidiCallbacks");
        thread.start();
        sHandler = new Handler(thread.getLooper());
        MidiManager.DeviceCallback callback = new MidiManager.DeviceCallback() {
            @Override
            public void onDeviceAdded(MidiDeviceInfo info) {
                Log.i(TAG, "MIDI device added: " + getName(info));
                notifyDevicesChanged();
            }

            @Override
            public void onDeviceRemoved(MidiDeviceInfo info) {
                Log.i(TAG, "MIDI device removed: " + getName(info));
                notifyDevicesChanged();
            }
        };
        registerCallback(callback);
        return true;
    }

    // The Handler overload is deprecated since API 33, but the Executor overload
    // did not compile in the Android 15 CI build, so it stays for now.
    @SuppressWarnings("deprecation")
    private static void registerCallback(MidiManager.DeviceCallback callback) {
        sManager.registerDeviceCallback(callback, sHandler);
    }

    @SuppressWarnings("deprecation")
    public static MidiDeviceInfo[] getDevices() {
        if (sManager == null) {
            return new MidiDeviceInfo[0];
        }
        return sManager.getDevices();
    }

    private static String getProperty(MidiDeviceInfo info, String key) {
        Bundle properties = info.getProperties();
        if (properties == null) {
            return "";
        }
        String value = properties.getString(key);
        return value == null ? "" : value.trim();
    }

    public static String getName(MidiDeviceInfo info) {
        String product = getProperty(info, MidiDeviceInfo.PROPERTY_PRODUCT);
        if (!product.isEmpty()) {
            return product;
        }
        String name = getProperty(info, MidiDeviceInfo.PROPERTY_NAME);
        if (!name.isEmpty()) {
            return name;
        }
        return "MIDI device " + info.getId();
    }

    public static String getManufacturer(MidiDeviceInfo info) {
        return getProperty(info, MidiDeviceInfo.PROPERTY_MANUFACTURER);
    }

    public static String getSerialNumber(MidiDeviceInfo info) {
        return getProperty(info, MidiDeviceInfo.PROPERTY_SERIAL_NUMBER);
    }

    @SuppressWarnings("deprecation")
    private static UsbDevice getUsbDevice(MidiDeviceInfo info) {
        Bundle properties = info.getProperties();
        if (properties == null) {
            return null;
        }
        Object device = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU
            ? properties.getParcelable(MidiDeviceInfo.PROPERTY_USB_DEVICE, UsbDevice.class)
            : getParcelableLegacy(properties, MidiDeviceInfo.PROPERTY_USB_DEVICE);
        return device instanceof UsbDevice ? (UsbDevice) device : null;
    }

    /// USB vendor id, -1 if not USB.
    public static int getUsbVendorId(MidiDeviceInfo info) {
        UsbDevice device = getUsbDevice(info);
        return device == null ? -1 : device.getVendorId();
    }

    /// USB product id, -1 if not USB.
    public static int getUsbProductId(MidiDeviceInfo info) {
        UsbDevice device = getUsbDevice(info);
        return device == null ? -1 : device.getProductId();
    }

    /// Opens the device synchronously. Must not be called from the thread
    /// running sHandler.
    public static MidiDevice openDevice(MidiDeviceInfo info, int timeoutMs) {
        if (sManager == null) {
            return null;
        }
        final MidiDevice[] result = new MidiDevice[1];
        final CountDownLatch latch = new CountDownLatch(1);
        sManager.openDevice(info, device -> {
            result[0] = device;
            latch.countDown();
        }, sHandler);
        try {
            if (!latch.await(timeoutMs, TimeUnit.MILLISECONDS)) {
                Log.w(TAG, "Timeout opening MIDI device " + getName(info));
                return null;
            }
        } catch (InterruptedException e) {
            return null;
        }
        if (result[0] == null) {
            Log.w(TAG, "Unable to open MIDI device " + getName(info));
        }
        return result[0];
    }

    public static void closeDevice(MidiDevice device) {
        if (device == null) {
            return;
        }
        try {
            device.close();
        } catch (Exception e) {
            Log.w(TAG, "Error closing MIDI device: " + e.toString());
        }
    }

    @SuppressWarnings("deprecation")
    public static boolean connectBluetoothDevice(Context context, String address, int timeoutMs) {
        if (sManager == null || context == null || address == null || address.isEmpty()) {
            return false;
        }
        // A BLE-MIDI peripheral only appears in MidiManager.getDevices() after
        // openBluetoothDevice() has been called for its BluetoothDevice (see
        // the MidiManager docs). Matching by name, as this code used to do,
        // never found it, so the connect always fell through to GATT.
        BluetoothManager bm = (BluetoothManager) context.getSystemService(Context.BLUETOOTH_SERVICE);
        BluetoothAdapter adapter = bm != null ? bm.getAdapter() : BluetoothAdapter.getDefaultAdapter();
        if (adapter == null) {
            return false;
        }
        BluetoothDevice bluetoothDevice;
        try {
            bluetoothDevice = adapter.getRemoteDevice(address);
        } catch (IllegalArgumentException e) {
            Log.w(TAG, "Invalid Bluetooth address " + address);
            return false;
        }
        final MidiDevice[] result = new MidiDevice[1];
        final CountDownLatch latch = new CountDownLatch(1);
        sManager.openBluetoothDevice(bluetoothDevice, device -> {
            result[0] = device;
            latch.countDown();
        }, sHandler);
        try {
            if (!latch.await(timeoutMs, TimeUnit.MILLISECONDS)) {
                Log.w(TAG, "Timeout opening Bluetooth MIDI device " + address);
                return false;
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return false;
        }
        if (result[0] != null) {
            Log.i(TAG, "Opened Bluetooth MIDI device " + address + " via MidiManager");
            return true;
        }
        return false;
    }

    public static void disconnectBluetoothDevice(String address) {
        if (sManager == null) {
            return;
        }
        MidiDeviceInfo[] devices = getDevices();
        for (MidiDeviceInfo info : devices) {
            String name = getName(info);
            if (name.contains(address) || address.contains(name)) {
                Log.i(TAG, "Disconnected Bluetooth MIDI device " + name);
            }
        }
    }

    /// MidiManager already exposes this BT device? GATT fallback must skip it.
    public static boolean isBluetoothDeviceOpen(String address) {
        if (sManager == null || address == null || address.isEmpty()) {
            return false;
        }
        for (MidiDeviceInfo info : getDevices()) {
            String deviceAddress = getBluetoothAddress(info);
            if (deviceAddress != null && deviceAddress.equalsIgnoreCase(address)) {
                return true;
            }
        }
        return false;
    }

    // Pre-API-33 Bundle lookup
    @SuppressWarnings("deprecation")
    private static Object getParcelableLegacy(Bundle properties, String key) {
        return properties.getParcelable(key);
    }

    /// MAC address of a BT MIDI device, null for non-BT.
    private static String getBluetoothAddress(MidiDeviceInfo info) {
        Bundle properties = info.getProperties();
        if (properties == null) {
            return null;
        }
        Object device;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            device = properties.getParcelable(
                MidiDeviceInfo.PROPERTY_BLUETOOTH_DEVICE, BluetoothDevice.class);
        } else {
            device = getParcelableLegacy(properties, MidiDeviceInfo.PROPERTY_BLUETOOTH_DEVICE);
        }
        return device instanceof BluetoothDevice ? ((BluetoothDevice) device).getAddress() : null;
    }
}
