package org.mixxx;

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
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/// Thin wrapper around android.media.midi used by AndroidMidiEnumerator and
/// AndroidMidiController. All MIDI I/O happens in native code through AMidi;
/// this class only lists devices, opens them and reports hotplug events.
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
        sManager.registerDeviceCallback(new MidiManager.DeviceCallback() {
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
}
