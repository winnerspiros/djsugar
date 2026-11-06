package org.mixxx;

import android.annotation.SuppressLint;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.content.Context;
import android.content.pm.PackageManager;
import android.hardware.usb.UsbDevice;
import android.media.midi.MidiDevice;
import android.media.midi.MidiDeviceInfo;
import android.media.midi.MidiManager;
import android.os.Bundle;
import android.os.Handler;
import android.os.HandlerThread;
import android.util.Log;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/// Thin wrapper around android.media.midi used by AndroidMidiEnumerator and
/// AndroidMidiController. All MIDI I/O happens in native code through AMidi;
/// this class lists devices, opens USB & Bluetooth MIDI devices, and reports
/// hotplug events.
public class MidiHelper {
    private static final String TAG = "MixxxMidi";
    private static MidiManager sManager = null;
    private static Handler sHandler = null;
    private static final ConcurrentHashMap<Integer, MidiDevice> sBluetoothMidiDevices =
        new ConcurrentHashMap<>();
    private static final ConcurrentHashMap<String, MidiDevice> sBluetoothMidiByAddress =
        new ConcurrentHashMap<>();

    private static native void midiDevicesChanged();

    public static void notifyDevicesChanged() {
        try {
            midiDevicesChanged();
        } catch (UnsatisfiedLinkError e) {
            Log.v(TAG, "Native library not loaded yet, skipping rescan");
        }
    }

    @SuppressWarnings("deprecation")
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
        // NOTE: registerDeviceCallback(DeviceCallback, Handler) is deprecated
        // since API 34, but the Executor replacement overload only exists in
        // compileSdk 34+. Keep the old call so we still build against older
        // SDKs; the callback thread behavior is identical.
        sManager.registerDeviceCallback(new MidiManager.DeviceCallback() {
            @Override
            public void onDeviceAdded(MidiDeviceInfo info) {
                Log.i(TAG, "MIDI device added: " + getName(info));
                notifyDevicesChanged();
            }

            @Override
            public void onDeviceRemoved(MidiDeviceInfo info) {
                Log.i(TAG, "MIDI device removed: " + getName(info));
                sBluetoothMidiDevices.remove(info.getId());
                notifyDevicesChanged();
            }
        }, sHandler);
        return true;
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
        Object device = properties.getParcelable(MidiDeviceInfo.PROPERTY_USB_DEVICE);
        return device instanceof UsbDevice ? (UsbDevice) device : null;
    }

    /// Returns the USB vendor id, or -1 when the device is not a USB device.
    public static int getUsbVendorId(MidiDeviceInfo info) {
        UsbDevice device = getUsbDevice(info);
        return device == null ? -1 : device.getVendorId();
    }

    /// Returns the USB product id, or -1 when the device is not a USB device.
    public static int getUsbProductId(MidiDeviceInfo info) {
        UsbDevice device = getUsbDevice(info);
        return device == null ? -1 : device.getProductId();
    }

    /// Connects a Bluetooth LE MIDI device through Android's MidiManager so it
    /// appears as a native AndroidMidiController via AMidi.
    @SuppressLint("MissingPermission")
    public static boolean connectBluetoothDevice(Context context, String address, int timeoutMs) {
        if (context == null || address == null || address.isEmpty()) {
            return false;
        }
        if (sManager == null && !init(context)) {
            return false;
        }
        if (sBluetoothMidiByAddress.containsKey(address)) {
            return true;
        }
        try {
            BluetoothManager bm =
                (BluetoothManager) context.getSystemService(Context.BLUETOOTH_SERVICE);
            BluetoothAdapter adapter = bm != null ? bm.getAdapter() : null;
            if (adapter == null || !adapter.isEnabled()) {
                return false;
            }
            BluetoothDevice btDevice = adapter.getRemoteDevice(address);
            if (btDevice == null) {
                return false;
            }
            final MidiDevice[] result = new MidiDevice[1];
            final CountDownLatch latch = new CountDownLatch(1);
            sManager.openBluetoothDevice(btDevice, device -> {
                result[0] = device;
                latch.countDown();
            }, sHandler);
            if (!latch.await(timeoutMs, TimeUnit.MILLISECONDS)) {
                Log.w(TAG, "Timeout opening Bluetooth MIDI device " + address);
                return false;
            }
            if (result[0] == null) {
                Log.w(TAG, "MidiManager.openBluetoothDevice returned null for " + address);
                return false;
            }
            MidiDeviceInfo info = result[0].getInfo();
            if (info != null) {
                sBluetoothMidiDevices.put(info.getId(), result[0]);
            }
            sBluetoothMidiByAddress.put(address, result[0]);
            Log.i(TAG, "Connected Bluetooth MIDI device via MidiManager: " + address);
            notifyDevicesChanged();
            return true;
        } catch (Exception e) {
            Log.w(TAG, "Failed to open Bluetooth MIDI device via MidiManager: " + e.toString());
            return false;
        }
    }

    public static boolean isBluetoothDeviceOpen(String address) {
        return address != null && sBluetoothMidiByAddress.containsKey(address);
    }

    public static void disconnectBluetoothDevice(String address) {
        if (address == null) {
            return;
        }
        MidiDevice device = sBluetoothMidiByAddress.remove(address);
        if (device != null) {
            MidiDeviceInfo info = device.getInfo();
            if (info != null) {
                sBluetoothMidiDevices.remove(info.getId());
            }
            try {
                device.close();
            } catch (Exception e) {
                Log.w(TAG, "Error closing Bluetooth MIDI device: " + e.toString());
            }
            notifyDevicesChanged();
        }
    }

    /// Opens the device synchronously. Must not be called from the thread
    /// running sHandler.
    public static MidiDevice openDevice(MidiDeviceInfo info, int timeoutMs) {
        if (sManager == null || info == null) {
            return null;
        }
        MidiDevice cachedBt = sBluetoothMidiDevices.get(info.getId());
        if (cachedBt != null) {
            return cachedBt;
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
        if (sBluetoothMidiDevices.containsValue(device)) {
            // Keep the Bluetooth MIDI link open until explicitly disconnected
            return;
        }
        try {
            device.close();
        } catch (Exception e) {
            Log.w(TAG, "Error closing MIDI device: " + e.toString());
        }
    }
}
