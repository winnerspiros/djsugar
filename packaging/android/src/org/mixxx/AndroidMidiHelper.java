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

public class AndroidMidiHelper {
    private static final String TAG = "MixxxMidi";

    private static native void midiReceive(int controllerId, byte[] data,
            int offset, int count, long timestamp);

    private MidiDevice mDevice;
    private MidiInputPort mInputPort;
    private MidiOutputPort mOutputPort;
    private int mControllerId;
    private boolean mOpenDone;

    /** Custom receiver that forwards incoming MIDI data to native code. */
    private final MidiReceiver mNativeReceiver = new MidiReceiver() {
        @Override
        public void onSend(byte[] data, int offset, int count, long timestamp) {
            midiReceive(mControllerId, data, offset, count, timestamp);
        }
    };

    // Static helpers used by C++ enumerator
    public static MidiDeviceInfo[] getDevices(MidiManager mgr) {
        return mgr.getDevices();
    }

    public static String getDeviceName(MidiDeviceInfo info) {
        Bundle p = info.getProperties();
        String n = p.getString(MidiDeviceInfo.PROPERTY_NAME);
        return n != null ? n : "Unknown";
    }

    // Instance methods for device/port management
    public boolean open(MidiManager mgr, MidiDeviceInfo info, int controllerId) {
        mControllerId = controllerId;
        mOpenDone = false;
        mgr.openDevice(info, new MidiManager.OnDeviceOpenedListener() {
            @Override
            public void onDeviceOpened(MidiDevice device) {
                mDevice = device;
                mOpenDone = true;
            }
        }, null);
        long deadline = System.currentTimeMillis() + 3000;
        while (!mOpenDone && System.currentTimeMillis() < deadline) {
            try {
                Thread.sleep(10);
            } catch (InterruptedException e) {
                break;
            }
        }
        return mDevice != null;
    }

    public boolean openPorts(int inIdx, int outIdx) {
        if (mDevice == null) {
            return false;
        }
        MidiDeviceInfo info = mDevice.getInfo();
        try {
            if (inIdx >= 0 && inIdx < info.getInputPortCount()) {
                mInputPort = mDevice.openInputPort(inIdx);
            }
            if (outIdx >= 0 && outIdx < info.getOutputPortCount()) {
                mOutputPort = mDevice.openOutputPort(outIdx);
            }
            // Route output port data through our native receiver
            if (mOutputPort != null) {
                mOutputPort.connect(mNativeReceiver);
            }
            return true;
        } catch (IOException e) {
            Log.e(TAG, "openPorts failed: " + e.getMessage());
            return false;
        }
    }

    public void send(byte[] data, int offset, int count) {
        if (mInputPort == null) {
            return;
        }
        try {
            mInputPort.send(data, offset, count, 0);
        } catch (IOException e) {
            Log.e(TAG, "send failed: " + e.getMessage());
        }
    }

    public void close() {
        try {
            if (mInputPort != null) {
                mInputPort.close();
                mInputPort = null;
            }
            if (mOutputPort != null) {
                mOutputPort.close();
                mOutputPort = null;
            }
            if (mDevice != null) {
                mDevice.close();
                mDevice = null;
            }
        } catch (IOException e) {
            Log.e(TAG, "close failed: " + e.getMessage());
        }
    }
}
