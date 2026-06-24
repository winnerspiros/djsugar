package org.mixxx;

import android.Manifest;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.bluetooth.BluetoothGattService;
import android.bluetooth.BluetoothManager;
import android.bluetooth.BluetoothProfile;
import android.bluetooth.le.BluetoothLeScanner;
import android.bluetooth.le.ScanCallback;
import android.bluetooth.le.ScanResult;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;
import android.util.Log;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

/**
 * BLE MIDI scanner for DJ controllers (e.g. DDJ-FLX4).
 * Scans for devices advertising the BLE-MIDI service UUID
 * (03B80E5A-EDE8-4B33-A751-6CE34EC4C700).
 */
public class BleMidiScanner {
    private static final String TAG = "MixxxBleMidiScanner";
    private static final UUID BLE_MIDI_SERVICE_UUID =
            UUID.fromString("03B80E5A-EDE8-4B33-A751-6CE34EC4C700");

    private final Context mContext;
    private final BluetoothManager mBluetoothManager;
    private final BluetoothAdapter mBluetoothAdapter;
    private BluetoothLeScanner mLeScanner;
    private BluetoothGatt mGatt;
    private boolean mScanning;
    private boolean mConnected;
    private final List<BluetoothDevice> mDiscoveredDevices = new ArrayList<>();
    private final BluetoothGattCallback mGattCallback = new BluetoothGattCallback() {
        @Override
        public void onConnectionStateChange(BluetoothGatt gatt, int status, int newState) {
            if (newState == BluetoothProfile.STATE_CONNECTED) {
                Log.i(TAG, "Connected to GATT server.");
                mConnected = true;
                gatt.discoverServices();
            } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                Log.i(TAG, "Disconnected from GATT server.");
                mConnected = false;
                mGatt = null;
            }
        }

        @Override
        public void onServicesDiscovered(BluetoothGatt gatt, int status) {
            if (status == BluetoothGatt.GATT_SUCCESS) {
                BluetoothGattService midiService = gatt.getService(BLE_MIDI_SERVICE_UUID);
                if (midiService != null) {
                    Log.i(TAG, "BLE MIDI service found on device: " + gatt.getDevice().getName());
                }
            }
        }
    };

    private final ScanCallback mScanCallback = new ScanCallback() {
        @Override
        public void onScanResult(int callbackType, ScanResult result) {
            BluetoothDevice device = result.getDevice();
            String name = device.getName();
            if (name == null) {
                name = "Unknown";
            }
            Log.i(TAG, "BLE device found: " + name + " [" + device.getAddress() + "]");
            if (!mDiscoveredDevices.contains(device)) {
                mDiscoveredDevices.add(device);
                connectToDevice(device);
            }
        }

        @Override
        public void onScanFailed(int errorCode) {
            Log.w(TAG, "BLE scan failed with error: " + errorCode);
        }
    };

    public BleMidiScanner(Context context) {
        mContext = context;
        mBluetoothManager = (BluetoothManager) context.getSystemService(Context.BLUETOOTH_SERVICE);
        mBluetoothAdapter = mBluetoothManager != null ? mBluetoothManager.getAdapter() : null;
        if (mBluetoothAdapter != null) {
            mLeScanner = mBluetoothAdapter.getBluetoothLeScanner();
        }
    }

    public boolean startScan(Context context, String serviceUuid) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            if (context.checkSelfPermission(Manifest.permission.BLUETOOTH_SCAN)
                    != PackageManager.PERMISSION_GRANTED) {
                Log.w(TAG, "BLUETOOTH_SCAN permission not granted");
                return false;
            }
        } else {
            if (context.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION)
                    != PackageManager.PERMISSION_GRANTED) {
                Log.w(TAG, "ACCESS_FINE_LOCATION permission not granted");
                return false;
            }
        }

        if (mLeScanner == null) {
            Log.w(TAG, "Bluetooth LE scanner not available");
            return false;
        }

        Log.i(TAG, "Starting BLE scan for MIDI service: " + serviceUuid);
        mDiscoveredDevices.clear();
        mScanning = true;
        mLeScanner.startScan(mScanCallback);
        return true;
    }

    public boolean isConnected() {
        return mConnected && mGatt != null;
    }

    public List<BluetoothDevice> getDiscoveredDevices() {
        return new ArrayList<>(mDiscoveredDevices);
    }

    private void connectToDevice(BluetoothDevice device) {
        Log.i(TAG, "Connecting to BLE MIDI device: " + device.getName());
        mGatt = device.connectGatt(mContext, false, mGattCallback);
    }

    public void stopScan() {
        if (mScanning && mLeScanner != null) {
            mLeScanner.stopScan(mScanCallback);
            mScanning = false;
            Log.i(TAG, "BLE scan stopped");
        }
    }

    public void disconnect() {
        if (mGatt != null) {
            mGatt.disconnect();
            mGatt.close();
            mGatt = null;
        }
        mConnected = false;
    }
}
