package org.mixxx;

import android.media.midi.MidiDevice;
import android.media.midi.MidiDeviceInfo;
import android.media.midi.MidiInputPort;
import android.media.midi.MidiManager;
import android.media.midi.MidiOutputPort;
import android.media.midi.MidiReceiver;
import android.os.Bundle;
import android.util.Log;
import java.io.IOException;

/**
 * Bridges Android MIDI API to native Mixxx code.
 * Mixxx calls openDevice() to open a MidiDevice and get input/output ports.
 * Incoming MIDI data is forwarded to native midiReceive().
 */
public class AndroidMidiHelper {
    private static final String TAG = "MixxxMidi";
    private static native void midiReceive(int controllerId, byte[] data, int offset, int count, long timestamp);

    private MidiManager m_manager;
    private MidiDevice m_device;
    private MidiInputPort m_inputPort;
    private MidiOutputPort m_outputPort;
    private int m_controllerId;

    public interface OpenCallback {
        void onDeviceOpened(AndroidMidiHelper helper);
        void onError(String error);
    }

    private final MidiManager.OnDeviceOpenedListener m_openListener =
        new MidiManager.OnDeviceOpenedListener() {
            @Override
            public void onDeviceOpened(MidiDevice device) {
                if (device == null) {
                    Log.e(TAG, "Failed to open MIDI device");
                    return;
                }
                m_device = device;
                Log.i(TAG, "MIDI device opened: " + device.getInfo().getProperties().getString(MidiDeviceInfo.PROPERTY_NAME));
            }
        };

    private final MidiReceiver m_receiver = new MidiReceiver() {
        @Override
        public void onSend(byte[] data, int offset, int count, long timestamp) {
            midiReceive(m_controllerId, data, offset, count, timestamp);
        }
    };

    public AndroidMidiHelper() {}

    public static MidiDeviceInfo[] getDevices(MidiManager manager) {
        return manager.getDevices();
    }

    public static String getDeviceName(MidiDeviceInfo info) {
        Bundle props = info.getProperties();
        String name = props.getString(MidiDeviceInfo.PROPERTY_NAME);
        return (name != null) ? name : "Unknown MIDI Device";
    }

    public static String getDeviceManufacturer(MidiDeviceInfo info) {
        Bundle props = info.getProperties();
        return props.getString(MidiDeviceInfo.PROPERTY_MANUFACTURER, "");
    }

    public static String getDeviceProduct(MidiDeviceInfo info) {
        Bundle props = info.getProperties();
        return props.getString(MidiDeviceInfo.PROPERTY_PRODUCT, "");
    }

    public static int getInputPortCount(MidiDeviceInfo info) {
        return info.getInputPortCount();
    }

    public static int getOutputPortCount(MidiDeviceInfo info) {
        return info.getOutputPortCount();
    }

    public boolean open(MidiManager manager, MidiDeviceInfo deviceInfo, int controllerId) {
        m_manager = manager;
        m_controllerId = controllerId;
        manager.openDevice(deviceInfo, new MidiManager.OnDeviceOpenedListener() {
            @Override
            public void onDeviceOpened(MidiDevice device) {
                m_device = device;
                Log.i(TAG, "MIDI device opened: " + 
                    (device != null ? device.getInfo().getProperties().getString(MidiDeviceInfo.PROPERTY_NAME) : "null"));
            }
        }, null); // handler must be null or a Handler for the callback thread
        return true;
    }

    public boolean openPorts(int inputPortIndex, int outputPortIndex) {
        if (m_device == null) {
            Log.e(TAG, "Device not open, cannot open ports");
            return false;
        }
        try {
            if (inputPortIndex >= 0 && inputPortIndex < m_device.getInfo().getInputPortCount()) {
                m_inputPort = m_device.openInputPort(inputPortIndex);
                if (m_inputPort != null) {
                    m_inputPort.connect(m_receiver);
                    Log.i(TAG, "Input port " + inputPortIndex + " opened");
                }
            }
            if (outputPortIndex >= 0 && outputPortIndex < m_device.getInfo().getOutputPortCount()) {
                m_outputPort = m_device.openOutputPort(outputPortIndex);
                Log.i(TAG, "Output port " + outputPortIndex + " opened");
            }
            return true;
        } catch (IOException e) {
            Log.e(TAG, "Failed to open MIDI ports: " + e.getMessage());
            return false;
        }
    }

    public void send(byte[] data, int offset, int count) {
        if (m_outputPort == null) return;
        try {
            m_outputPort.write(data, offset, count);
        } catch (IOException e) {
            Log.e(TAG, "MIDI send failed: " + e.getMessage());
        }
    }

    public void close() {
        try {
            if (m_inputPort != null) { m_inputPort.close(); m_inputPort = null; }
            if (m_outputPort != null) { m_outputPort.close(); m_outputPort = null; }
            if (m_device != null) { m_device.close(); m_device = null; }
        } catch (IOException e) {
            Log.e(TAG, "Error closing MIDI: " + e.getMessage());
        }
    }
}
