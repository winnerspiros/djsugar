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
import java.lang.reflect.Method;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Android USB MIDI device manager.
 * Uses android.media.midi API to enumerate and communicate with USB MIDI devices.
 * Compatible with API 23+ (minimal dependency on newer API methods).
 */
public class UsbMidiDevice {
    private static final String TAG = "MixxxUsbMidi";
    private static MidiManager sMidiManager = null;
    private static final Map<Integer, UsbMidiDevice> sOpenDevices =
        new ConcurrentHashMap<>();
    private static int sNextDeviceId = 1;

    // Native callbacks
    private static native void onMidiDataReceived(
        int deviceId, byte[] data, int offset, int count);
    private static native void onDeviceDisconnected(int deviceId);
    private static native void onDeviceConnected(
        int vendorId, int productId,
        String manufacturer, String product, int interfaceNumber);

    private final int mDeviceId;
    private final MidiDevice mMidiDevice;
    private final MidiDeviceInfo mMidiDeviceInfo;
    private MidiInputPort mInputPort;
    private MidiOutputPort mOutputPort;
    private final MidiReceiver mInputReceiver;
    private boolean mOpen = false;

    // Reflection cache for connect/disconnect/send
    private static Method sConnectMethod;
    private static Method sDisconnectMethod;
    private static Method sSendMethod;
    private static boolean sReflectInit = false;

    private UsbMidiDevice(int deviceId, MidiDevice midiDevice) {
        mDeviceId = deviceId;
        mMidiDevice = midiDevice;
        mMidiDeviceInfo = midiDevice.getInfo();
        mInputReceiver = new MidiReceiver() {
            @Override
            public void onSend(
                byte[] data, int offset, int count, long timestamp) {
                onMidiDataReceived(mDeviceId, data, offset, count);
            }
        };
    }

    private static void initReflection() {
        if (sReflectInit)
            return;
        sReflectInit = true;
        try {
            sConnectMethod = MidiInputPort.class.getMethod(
                "connect", MidiReceiver.class);
        } catch (NoSuchMethodException e) {
            sConnectMethod = null;
        }
        try {
            sDisconnectMethod = MidiInputPort.class.getMethod(
                "disconnect", MidiReceiver.class);
        } catch (NoSuchMethodException e) {
            sDisconnectMethod = null;
        }
        try {
            sSendMethod = MidiOutputPort.class.getMethod(
                "send", byte[].class, int.class, int.class);
        } catch (NoSuchMethodException e) {
            sSendMethod = null;
        }
        Log.i(TAG, "Reflection: connect=" + (sConnectMethod != null) + " disconnect=" + (sDisconnectMethod != null) + " send=" + (sSendMethod != null));
    }

    /**
     * Initialize the MIDI manager.
     */
    public static boolean initialize(Context context) {
        try {
            sMidiManager = (MidiManager) context.getSystemService(
                Context.MIDI_SERVICE);
            if (sMidiManager == null) {
                Log.w(TAG, "MIDI service not available");
                return false;
            }
            initReflection();
            Log.i(TAG, "MIDI manager initialized");
            return true;
        } catch (Exception e) {
            Log.e(TAG, "Failed to init MIDI: " + e.getMessage());
            return false;
        }
    }

    /**
     * Enumerate USB MIDI devices.
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

            onDeviceConnected(vendorId, productId,
                manufacturer, product, outputPortCount);
            count++;
        }
        return count;
    }

    /**
     * Open a USB MIDI device.
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

        sMidiManager.openDevice(info,
            new MidiManager.OnDeviceOpenedListener() {
                @Override
                public void onDeviceOpened(MidiDevice device) {
                    if (device == null) {
                        Log.e(TAG,
                            "Failed to open USB MIDI device");
                        return;
                    }
                    UsbMidiDevice usbDevice =
                        new UsbMidiDevice(deviceId, device);
                    usbDevice.mOpen = true;

                    // Open input port (device -> app)
                    MidiInputPort inputPort =
                        device.openInputPort(0);
                    if (inputPort != null) {
                        usbDevice.mInputPort = inputPort;
                        attachReceiver(inputPort,
                            usbDevice.mInputReceiver);
                        Log.i(TAG,
                            "Opened MIDI input for device "
                                + deviceId);
                    }

                    // Open output port (app -> device)
                    MidiOutputPort outputPort =
                        device.openOutputPort(0);
                    if (outputPort != null) {
                        usbDevice.mOutputPort = outputPort;
                        Log.i(TAG,
                            "Opened MIDI output for device "
                                + deviceId);
                    }

                    sOpenDevices.put(deviceId, usbDevice);
                    result[0] = usbDevice;
                    opened[0] = true;
                }
            },
            null);

        // Wait for the async callback with 3s timeout
        long timeout = System.currentTimeMillis() + 3000;
        while (!opened[0]
            && System.currentTimeMillis() < timeout) {
            try {
                Thread.sleep(10);
            } catch (InterruptedException e) {
                break;
            }
        }

        return result[0];
    }

    /**
     * Attach a receiver to an input port using reflection.
     */
    private static void attachReceiver(
        MidiInputPort port, MidiReceiver receiver) {
        if (sConnectMethod != null) {
            try {
                sConnectMethod.invoke(port, receiver);
                return;
            } catch (Exception e) {
                Log.w(TAG, "connect() via reflection failed", e);
            }
        }
        // Fallback: on API 23-25, data may not arrive via callback
        Log.w(TAG, "MidiInputPort.connect() not available"
                + " — data may not arrive");
    }

    /**
     * Detach a receiver from an input port using reflection.
     */
    private static void detachReceiver(
        MidiInputPort port, MidiReceiver receiver) {
        if (sDisconnectMethod != null) {
            try {
                sDisconnectMethod.invoke(port, receiver);
            } catch (Exception e) {
                Log.w(TAG, "disconnect() via reflection failed", e);
            }
        }
    }

    /**
     * Send MIDI data to the device.
     */
    public static boolean sendMidiData(
        int deviceId, byte[] data, int offset, int count) {
        UsbMidiDevice device = sOpenDevices.get(deviceId);
        if (device == null || device.mOutputPort == null)
            return false;

        // Try send() via reflection
        if (sSendMethod != null) {
            try {
                sSendMethod.invoke(
                    device.mOutputPort, data, offset, count);
                return true;
            } catch (Exception e) {
                Log.w(TAG, "send() via reflection failed", e);
            }
        }

        // Fallback: on API < 26, send() may not be available
        Log.w(TAG, "send() not available via reflection"
                + " — output may not work");
        return false;
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
                detachReceiver(mInputPort, mInputReceiver);
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
