package org.mixxx;

import android.content.Context;
import android.media.midi.MidiDevice;
import android.media.midi.MidiDeviceInfo;
import android.media.midi.MidiInputPort;
import android.media.midi.MidiManager;
import android.media.midi.MidiOutputPort;
import android.media.midi.MidiReceiver;
import android.os.Bundle;
import android.util.Log;
import java.io.IOException;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Android USB MIDI device manager.
 * Uses android.media.midi API to enumerate and communicate with USB MIDI devices.
 * <p>
 * This is how Rekordbox and other Android DJ apps access the DDJ-FLX4 on Android.
 * The FLX4's main chip exposes a MIDI interface (interface 3) that is claimed by
 * Android's MIDI subsystem. This class bridges that subsystem to Mixxx.
 */
public class UsbMidiDevice {
    private static final String TAG = "MixxxUsbMidi";
    private static MidiManager sMidiManager = null;
    private static final Map<Integer, UsbMidiDevice> sOpenDevices = new ConcurrentHashMap<>();
    private static int sNextDeviceId = 1;

    // Native callbacks
    private static native void onMidiDataReceived(int deviceId, byte[] data,
        int offset, int count);
    private static native void onDeviceDisconnected(int deviceId);
    private static native void onDeviceConnected(int vendorId, int productId,
        String manufacturer, String product, int interfaceNumber);

    private final int mDeviceId;
    private final MidiDevice mMidiDevice;
    private final MidiDeviceInfo mMidiDeviceInfo;
    private MidiInputPort mInputPort;
    private MidiOutputPort mOutputPort;
    private final MidiReceiver mInputReceiver;
    private boolean mOpen = false;

    private UsbMidiDevice(int deviceId, MidiDevice midiDevice) {
        mDeviceId = deviceId;
        mMidiDevice = midiDevice;
        mMidiDeviceInfo = midiDevice.getInfo();
        mInputReceiver = new MidiReceiver() {
            @Override
            public void onSend(byte[] data, int offset, int count, long timestamp) {
                onMidiDataReceived(mDeviceId, data, offset, count);
            }
        };
    }

    /**
     * Initialize the MIDI manager. Must be called from the main thread with
     * a valid Context (e.g. the Qt activity).
     */
    public static boolean initialize(Context context) {
        try {
            sMidiManager = (MidiManager) context.getSystemService(Context.MIDI_SERVICE);
            if (sMidiManager == null) {
                Log.w(TAG, "MIDI service not available on this device");
                return false;
            }
            Log.i(TAG, "MIDI manager initialized");
            return true;
        } catch (Exception e) {
            Log.e(TAG, "Failed to initialize MIDI manager: " + e.getMessage());
            return false;
        }
    }

    /**
     * Enumerate USB MIDI devices and notify native code for each connected device.
     * Returns the number of devices found.
     */
    public static int enumerateDevices() {
        if (sMidiManager == null)
            return 0;

        MidiDeviceInfo[] infos;
        try {
            infos = sMidiManager.getDevices();
        } catch (Exception e) {
            return 0;
        }
        int count = 0;
        for (MidiDeviceInfo info : infos) {
            if (info.getType() != MidiDeviceInfo.TYPE_USB)
                continue;

            Bundle props = info.getProperties();
            int vendorId = props.getInt("vendor", 0);
            int productId = props.getInt("product", 0);
            String manufacturer = props.getString("manufacturer", "");
            String product = props.getString("name", "");
            int outputPortCount = info.getOutputPortCount();

            if (vendorId == 0 && productId == 0)
                continue;

            onDeviceConnected(vendorId, productId, manufacturer, product, outputPortCount);
            count++;
        }
        return count;
    }

    /**
     * Open a USB MIDI device for I/O. Called from native after enumeration.
     * The device is identified by its MidiDeviceInfo matching vendor/product.
     */
    public static UsbMidiDevice openDevice(int vendorId, int productId) {
        if (sMidiManager == null)
            return null;

        MidiDeviceInfo[] infos = sMidiManager.getDevices();
        for (MidiDeviceInfo info : infos) {
            if (info.getType() != MidiDeviceInfo.TYPE_USB)
                continue;

            Bundle props = info.getProperties();
            int devVendorId = props.getInt("vendor", 0);
            int devProductId = props.getInt("product", 0);

            if (devVendorId == vendorId && devProductId == productId) {
                return openDevice(info);
            }
        }
        return null;
    }

    private static UsbMidiDevice openDevice(MidiDeviceInfo info) {
        final int deviceId = sNextDeviceId++;

        final UsbMidiDevice[] result = new UsbMidiDevice[1];
        final boolean[] opened = {false};

        sMidiManager.openDevice(info, new MidiManager.OnDeviceOpenedListener() {
            @Override
            public void onDeviceOpened(MidiDevice device) {
                if (device == null) {
                    Log.e(TAG, "Failed to open USB MIDI device");
                    return;
                }
                UsbMidiDevice usbDevice = new UsbMidiDevice(deviceId, device);
                usbDevice.mOpen = true;

                // Open input port (device → app)
                MidiInputPort inputPort = device.openInputPort(0);
                if (inputPort != null) {
                    usbDevice.mInputPort = inputPort;
                    inputPort.connect(usbDevice.mInputReceiver);
                    Log.i(TAG, "Opened MIDI input port for device " + deviceId);
                }

                // Open output port (app → device)
                MidiOutputPort outputPort = device.openOutputPort(0);
                if (outputPort != null) {
                    usbDevice.mOutputPort = outputPort;
                    Log.i(TAG, "Opened MIDI output port for device " + deviceId);
                }

                sOpenDevices.put(deviceId, usbDevice);
                result[0] = usbDevice;
                opened[0] = true;
            }
        }, null);

        // Wait briefly for the async callback, with a timeout
        long timeout = System.currentTimeMillis() + 3000; // 3s timeout
        while (!opened[0] && System.currentTimeMillis() < timeout) {
            try {
                Thread.sleep(10);
            } catch (InterruptedException e) {
                break;
            }
        }

        return result[0];
    }

    /**
     * Send MIDI data to the device.
     */
    public static boolean sendMidiData(int deviceId, byte[] data, int offset, int count) {
        UsbMidiDevice device = sOpenDevices.get(deviceId);
        if (device == null || device.mOutputPort == null)
            return false;

        try {
            device.mOutputPort.send(data, offset, count);
            return true;
        } catch (IOException e) {
            Log.e(TAG, "Failed to send MIDI data: " + e.getMessage());
            return false;
        }
    }

    /**
     * Close a device.
     */
    public static void closeDevice(int deviceId) {
        UsbMidiDevice device = sOpenDevices.remove(deviceId);
        if (device == null)
            return;
        device.close();
    }

    private void close() {
        mOpen = false;
        try {
            if (mInputPort != null) {
                mInputPort.disconnect(mInputReceiver);
                mInputPort.close();
                mInputPort = null;
            }
            if (mOutputPort != null) {
                mOutputPort.close();
                mOutputPort = null;
            }
            mMidiDevice.close();
        } catch (IOException e) {
            Log.e(TAG, "Error closing MIDI device: " + e.getMessage());
        }
        Log.i(TAG, "Closed USB MIDI device " + mDeviceId);
    }

    public int getDeviceId() {
        return mDeviceId;
    }
}
