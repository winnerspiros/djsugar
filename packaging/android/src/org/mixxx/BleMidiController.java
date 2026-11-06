package org.mixxx;

import android.Manifest;
import android.annotation.SuppressLint;
import android.app.Activity;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattDescriptor;
import android.bluetooth.BluetoothGattService;
import android.bluetooth.BluetoothManager;
import android.bluetooth.BluetoothProfile;
import android.bluetooth.le.BluetoothLeScanner;
import android.bluetooth.le.ScanCallback;
import android.bluetooth.le.ScanRecord;
import android.bluetooth.le.ScanResult;
import android.bluetooth.le.ScanSettings;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.ParcelUuid;
import android.util.Log;
import androidx.core.app.ActivityCompat;
import androidx.core.content.ContextCompat;
import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * BLE MIDI scanner and GATT controller for Android.
 *
 * Supports both:
 * 1. Android's system MidiManager.openBluetoothDevice (via MidiHelper)
 * 2. Direct BLE GATT MIDI service (03B80E5A-EDE8-4B33-A751-6CE34EC4C700)
 *    with full Apple / MMA BLE-MIDI 1.0 packet framing and parsing.
 */
@SuppressWarnings("deprecation")
public class BleMidiController {
    private static final String TAG = "MixxxBleMidi";
    private static final int REQUEST_BLE_PERMISSIONS = 0x424C;

    // BLE MIDI UUIDs (Apple / MMA BLE-MIDI 1.0 specification)
    public static final UUID BLE_MIDI_SERVICE_UUID =
        UUID.fromString("03B80E5A-EDE8-4B33-A751-6CE34EC4C700");
    public static final UUID BLE_MIDI_CHAR_UUID =
        UUID.fromString("7772E5DB-3868-4112-A1A9-F2669D106BF3");
    private static final UUID CCC_DESCRIPTOR_UUID =
        UUID.fromString("00002902-0000-1000-8000-00805f9b34fb");
    private static final ParcelUuid BLE_MIDI_PARCEL_UUID =
        new ParcelUuid(BLE_MIDI_SERVICE_UUID);

    private static Context sContext;
    private static BluetoothAdapter sBluetoothAdapter;
    private static BluetoothLeScanner sLeScanner;
    private static volatile boolean sScanning = false;
    private static volatile boolean sConnected = false;

    // Discovered devices: address -> name
    private static final Map<String, String> sDiscoveredDevices = new ConcurrentHashMap<>();
    // Whether device advertised BLE MIDI UUID or matches a known controller name
    private static final Map<String, Boolean> sIsMidiCandidate = new ConcurrentHashMap<>();
    // Connected GATT instances: address -> Gatt
    private static final Map<String, BluetoothGatt> sConnectedGatts = new ConcurrentHashMap<>();
    // MIDI characteristics: address -> characteristic
    private static final Map<String, BluetoothGattCharacteristic> sMidiChars =
        new ConcurrentHashMap<>();

    // JNI callbacks implemented in blemidicontroller.cpp
    private static native void nativeOnMidiDataReceived(String deviceAddress, byte[] data);
    private static native void nativeOnDeviceConnected(String deviceAddress, String deviceName);
    private static native void nativeOnDeviceDisconnected(String deviceAddress);

    private static void safeOnMidiDataReceived(String addr, byte[] data) {
        try {
            nativeOnMidiDataReceived(addr, data);
        } catch (UnsatisfiedLinkError e) {
            Log.w(TAG, "nativeOnMidiDataReceived not linked: " + e.getMessage());
        }
    }

    private static void safeOnDeviceConnected(String addr, String name) {
        try {
            nativeOnDeviceConnected(addr, name);
        } catch (UnsatisfiedLinkError e) {
            Log.w(TAG, "nativeOnDeviceConnected not linked: " + e.getMessage());
        }
    }

    private static void safeOnDeviceDisconnected(String addr) {
        try {
            nativeOnDeviceDisconnected(addr);
        } catch (UnsatisfiedLinkError e) {
            Log.w(TAG, "nativeOnDeviceDisconnected not linked: " + e.getMessage());
        }
    }

    public static void initialize(Context context) {
        if (context == null) {
            return;
        }
        sContext = context.getApplicationContext();
        BluetoothManager bm =
            (BluetoothManager) sContext.getSystemService(Context.BLUETOOTH_SERVICE);
        if (bm == null) {
            Log.e(TAG, "Bluetooth not supported");
            return;
        }
        sBluetoothAdapter = bm.getAdapter();
    }

    public static boolean hasRequiredPermissions(Context context) {
        if (context == null) {
            return false;
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            return ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_SCAN)
                    == PackageManager.PERMISSION_GRANTED
                && ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_CONNECT)
                    == PackageManager.PERMISSION_GRANTED;
        } else {
            return ContextCompat.checkSelfPermission(
                       context, Manifest.permission.ACCESS_FINE_LOCATION)
                == PackageManager.PERMISSION_GRANTED;
        }
    }

    public static void requestPermissionsIfNeeded(Activity activity) {
        if (activity == null || hasRequiredPermissions(activity)) {
            return;
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            ActivityCompat.requestPermissions(activity,
                new String[] {
                    Manifest.permission.BLUETOOTH_SCAN,
                    Manifest.permission.BLUETOOTH_CONNECT,
                },
                REQUEST_BLE_PERMISSIONS);
        } else {
            ActivityCompat.requestPermissions(activity,
                new String[] {
                    Manifest.permission.ACCESS_FINE_LOCATION,
                },
                REQUEST_BLE_PERMISSIONS);
        }
    }

    public static boolean isBluetoothEnabled(Context context) {
        initialize(context);
        return sBluetoothAdapter != null && sBluetoothAdapter.isEnabled();
    }

    public static boolean isScanning() {
        return sScanning;
    }

    private static boolean matchesControllerName(String name) {
        if (name == null || name.isEmpty()) {
            return false;
        }
        String lower = name.toLowerCase(Locale.ROOT);
        return lower.contains("ddj")
            || lower.contains("flx")
            || lower.contains("pioneer")
            || lower.contains("alphatheta")
            || lower.contains("omnis")
            || lower.contains("hercules")
            || lower.contains("djcontrol")
            || lower.contains("inpulse")
            || lower.contains("starlight")
            || lower.contains("numark")
            || lower.contains("dj2go")
            || lower.contains("mixstream")
            || lower.contains("party mix")
            || lower.contains("reloop")
            || lower.contains("denon")
            || lower.contains("roland")
            || lower.contains("korg")
            || lower.contains("nanokontrol")
            || lower.contains("yamaha")
            || lower.contains("akai")
            || lower.contains("mpk")
            || lower.contains("lpd8")
            || lower.contains("arturia")
            || lower.contains("novation")
            || lower.contains("launch")
            || lower.contains("m-audio")
            || lower.contains("midi")
            || lower.contains("ble");
    }

    @SuppressLint("MissingPermission")
    private static void populateBondedAndConnectedDevices(Context context) {
        if (sBluetoothAdapter == null || !hasRequiredPermissions(context)) {
            return;
        }
        try {
            Set<BluetoothDevice> bonded = sBluetoothAdapter.getBondedDevices();
            if (bonded != null) {
                for (BluetoothDevice device : bonded) {
                    if (device == null) {
                        continue;
                    }
                    int type = device.getType();
                    if (type != BluetoothDevice.DEVICE_TYPE_LE
                        && type != BluetoothDevice.DEVICE_TYPE_DUAL
                        && type != BluetoothDevice.DEVICE_TYPE_UNKNOWN) {
                        continue;
                    }
                    String addr = device.getAddress();
                    String name = device.getName();
                    if (name == null || name.isEmpty()) {
                        continue;
                    }
                    sDiscoveredDevices.put(addr, name);
                    if (matchesControllerName(name)) {
                        sIsMidiCandidate.put(addr, true);
                    }
                }
            }
            BluetoothManager bm =
                (BluetoothManager) context.getSystemService(Context.BLUETOOTH_SERVICE);
            if (bm != null) {
                List<BluetoothDevice> gattConnected =
                    bm.getConnectedDevices(BluetoothProfile.GATT);
                if (gattConnected != null) {
                    for (BluetoothDevice device : gattConnected) {
                        if (device == null) {
                            continue;
                        }
                        String addr = device.getAddress();
                        String name = device.getName();
                        if (name == null || name.isEmpty()) {
                            name = addr;
                        }
                        sDiscoveredDevices.put(addr, name);
                        sIsMidiCandidate.put(addr, true);
                    }
                }
            }
        } catch (Exception e) {
            Log.w(TAG, "Error inspecting bonded/connected BLE devices: " + e.toString());
        }
    }

    @SuppressLint("MissingPermission")
    public static boolean startScan(Context context) {
        initialize(context);
        if (!hasRequiredPermissions(context)) {
            if (context instanceof Activity) {
                requestPermissionsIfNeeded((Activity) context);
            }
            Log.w(TAG, "BLE permissions not yet granted; requested from user");
            return false;
        }
        if (sBluetoothAdapter == null || !sBluetoothAdapter.isEnabled()) {
            Log.w(TAG, "Bluetooth not available or disabled");
            return false;
        }

        stopScan();
        sDiscoveredDevices.clear();
        sIsMidiCandidate.clear();
        populateBondedAndConnectedDevices(context);

        sLeScanner = sBluetoothAdapter.getBluetoothLeScanner();
        if (sLeScanner == null) {
            Log.e(TAG, "BLE scanner not available");
            return !sDiscoveredDevices.isEmpty();
        }

        ScanSettings settings = new ScanSettings.Builder()
                                    .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
                                    .setCallbackType(ScanSettings.CALLBACK_TYPE_ALL_MATCHES)
                                    .build();

        try {
            sLeScanner.startScan(null, settings, sScanCallback);
            sScanning = true;
            Log.i(TAG, "BLE scan started");
            return true;
        } catch (SecurityException e) {
            Log.e(TAG, "SecurityException starting scan: " + e.getMessage());
            return false;
        }
    }

    @SuppressLint("MissingPermission")
    public static void stopScan() {
        if (sLeScanner != null && sScanning) {
            try {
                sLeScanner.stopScan(sScanCallback);
                Log.i(TAG, "BLE scan stopped");
            } catch (Exception e) {
                Log.w(TAG, "Exception stopping scan: " + e.getMessage());
            }
        }
        sScanning = false;
    }

    /**
     * Get discovered devices as a list of "address|name|isMidi|isConnected" strings.
     */
    public static List<String> getDiscoveredDevices() {
        List<String> result = new ArrayList<>();
        for (Map.Entry<String, String> entry : sDiscoveredDevices.entrySet()) {
            String addr = entry.getKey();
            String name = entry.getValue().replace('|', ' ');
            boolean isMidi = Boolean.TRUE.equals(sIsMidiCandidate.get(addr))
                || matchesControllerName(name);
            boolean connected = isDeviceConnected(addr);
            result.add(addr + "|" + name + "|" + (isMidi ? "1" : "0") + "|"
                + (connected ? "1" : "0"));
        }
        return result;
    }

    public static boolean isDeviceConnected(String address) {
        if (address == null) {
            return false;
        }
        return sMidiChars.containsKey(address)
            || MidiHelper.isBluetoothDeviceOpen(address);
    }

    public static boolean isConnected() {
        return (sConnected && !sMidiChars.isEmpty())
            || !sDiscoveredDevices.isEmpty();
    }

    /**
     * Connects to a specific BLE MIDI device by MAC address.
     * Tries Android's MidiManager.openBluetoothDevice first, then falls back
     * to direct GATT BLE-MIDI connection.
     */
    @SuppressLint("MissingPermission")
    public static boolean connectDevice(Context context, String address, String name) {
        if (context != null) {
            initialize(context);
        }
        if (address == null || address.isEmpty()) {
            return false;
        }
        if (name != null && !name.isEmpty()) {
            sDiscoveredDevices.put(address, name);
            sIsMidiCandidate.put(address, true);
        }

        // 1. Try Android's native MidiManager.openBluetoothDevice first so the
        //    controller is exposed via AMidi / AndroidMidiEnumerator.
        if (sContext != null && MidiHelper.connectBluetoothDevice(sContext, address, 3500)) {
            Log.i(TAG, "Connected " + address + " via Android MidiManager");
            sConnected = true;
            UsbPermission.notifyDevicesChanged();
            return true;
        }

        // 2. Fallback: direct BluetoothGatt BLE-MIDI connection
        if (sMidiChars.containsKey(address)) {
            sConnected = true;
            String devName = sDiscoveredDevices.getOrDefault(address, address);
            safeOnDeviceConnected(address, devName);
            UsbPermission.notifyDevicesChanged();
            return true;
        }

        if (sBluetoothAdapter == null) {
            return false;
        }
        try {
            BluetoothDevice device = sBluetoothAdapter.getRemoteDevice(address);
            if (device == null) {
                return false;
            }
            BluetoothGatt existing = sConnectedGatts.remove(address);
            if (existing != null) {
                try {
                    existing.disconnect();
                    existing.close();
                } catch (Exception ignored) {
                }
            }
            BluetoothGatt gatt;
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                gatt = device.connectGatt(
                    sContext, false, sGattCallback, BluetoothDevice.TRANSPORT_LE);
            } else {
                gatt = device.connectGatt(sContext, false, sGattCallback);
            }
            if (gatt != null) {
                sConnectedGatts.put(address, gatt);
                Log.i(TAG, "Initiated direct BLE GATT connection to " + address);
                return true;
            }
        } catch (Exception e) {
            Log.e(TAG, "Error connecting BLE GATT to " + address + ": " + e.toString());
        }
        return false;
    }

    @SuppressLint("MissingPermission")
    public static void disconnectDevice(String address) {
        if (address == null || address.isEmpty()) {
            return;
        }
        MidiHelper.disconnectBluetoothDevice(address);
        sMidiChars.remove(address);
        BluetoothGatt gatt = sConnectedGatts.remove(address);
        if (gatt != null) {
            try {
                gatt.disconnect();
                gatt.close();
            } catch (Exception ignored) {
            }
        }
        if (sMidiChars.isEmpty()) {
            sConnected = false;
        }
        safeOnDeviceDisconnected(address);
        UsbPermission.notifyDevicesChanged();
    }

    @SuppressLint("MissingPermission")
    public static void disconnectAll() {
        for (String addr : new ArrayList<>(sConnectedGatts.keySet())) {
            disconnectDevice(addr);
        }
        sConnectedGatts.clear();
        sMidiChars.clear();
        sConnected = false;
    }

    /**
     * Send raw MIDI bytes to a connected BLE device.
     * C++ passes raw MIDI bytes (hex-encoded); Java wraps them once in the
     * Apple BLE-MIDI 1.0 [header][timestamp_low][midi...] packet.
     */
    @SuppressLint("MissingPermission")
    public static void writeMidiData(String deviceAddress, String hexData) {
        BluetoothGattCharacteristic ch = sMidiChars.get(deviceAddress);
        BluetoothGatt gatt = sConnectedGatts.get(deviceAddress);
        if (ch == null || gatt == null) {
            return;
        }

        byte[] midiData = hexStringToByteArray(hexData);
        if (midiData == null || midiData.length == 0) {
            return;
        }

        byte[] packet = wrapBleMidiPacket(midiData);
        if (packet == null) {
            return;
        }

        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                gatt.writeCharacteristic(ch, packet,
                    BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE);
            } else {
                ch.setValue(packet);
                ch.setWriteType(BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE);
                gatt.writeCharacteristic(ch);
            }
        } catch (Exception e) {
            Log.w(TAG, "BLE MIDI write failed: " + e.toString());
        }
    }

    private static byte[] wrapBleMidiPacket(byte[] midiData) {
        if (midiData == null || midiData.length == 0) {
            return null;
        }
        long nowMs = System.currentTimeMillis() & 0x1FFF;
        byte header = (byte) (0x80 | ((nowMs >> 7) & 0x3F));
        byte tsLow = (byte) (0x80 | (nowMs & 0x7F));

        // If midiData is a SysEx message ending with 0xF7, BLE-MIDI requires a
        // timestamp byte immediately before the 0xF7 End-Of-Exclusive byte.
        if ((midiData[0] & 0xFF) == 0xF0
            && midiData.length >= 2
            && (midiData[midiData.length - 1] & 0xFF) == 0xF7) {
            byte[] packet = new byte[midiData.length + 3];
            packet[0] = header;
            packet[1] = tsLow;
            System.arraycopy(midiData, 0, packet, 2, midiData.length - 1);
            packet[packet.length - 2] = tsLow;
            packet[packet.length - 1] = (byte) 0xF7;
            return packet;
        }

        byte[] packet = new byte[midiData.length + 2];
        packet[0] = header;
        packet[1] = tsLow;
        System.arraycopy(midiData, 0, packet, 2, midiData.length);
        return packet;
    }

    private static byte[] hexStringToByteArray(String hex) {
        if (hex == null || hex.length() % 2 != 0) {
            return null;
        }
        byte[] data = new byte[hex.length() / 2];
        for (int i = 0; i < hex.length(); i += 2) {
            data[i / 2] = (byte) ((Character.digit(hex.charAt(i), 16) << 4)
                + Character.digit(hex.charAt(i + 1), 16));
        }
        return data;
    }

    private static final ScanCallback sScanCallback = new ScanCallback() {
        @Override
        @SuppressLint("MissingPermission")
        public void onScanResult(int callbackType, ScanResult result) {
            if (result == null || result.getDevice() == null) {
                return;
            }
            BluetoothDevice device = result.getDevice();
            String address = device.getAddress();
            if (address == null || address.isEmpty()) {
                return;
            }

            ScanRecord record = result.getScanRecord();
            boolean hasMidiUuid = false;
            if (record != null && record.getServiceUuids() != null) {
                hasMidiUuid = record.getServiceUuids().contains(BLE_MIDI_PARCEL_UUID);
            }

            String name = device.getName();
            if ((name == null || name.isEmpty()) && record != null) {
                name = record.getDeviceName();
            }
            boolean knownController = matchesControllerName(name);

            // Ignore anonymous beacons that neither advertise BLE-MIDI nor have a name
            if ((name == null || name.isEmpty()) && !hasMidiUuid) {
                return;
            }
            if (name == null || name.isEmpty()) {
                name = "BLE MIDI (" + address + ")";
            }

            boolean isNew = !sDiscoveredDevices.containsKey(address);
            sDiscoveredDevices.put(address, name);
            if (hasMidiUuid || knownController) {
                sIsMidiCandidate.put(address, true);
            }

            // Automatically connect if the peripheral explicitly advertises the
            // BLE-MIDI service UUID or is a known DJ controller (e.g. DDJ-FLX4).
            if (isNew && (hasMidiUuid || knownController) && !isDeviceConnected(address)) {
                Log.i(TAG, "Auto-connecting discovered BLE MIDI controller: "
                    + name + " (" + address + ")");
                connectDevice(sContext, address, name);
            }
        }

        @Override
        public void onScanFailed(int errorCode) {
            Log.e(TAG, "BLE scan failed with error code: " + errorCode);
            sScanning = false;
        }
    };

    private static final BluetoothGattCallback sGattCallback = new BluetoothGattCallback() {
        @Override
        @SuppressLint("MissingPermission")
        public void onConnectionStateChange(BluetoothGatt gatt, int status, int newState) {
            if (gatt.getDevice() == null) {
                return;
            }
            String addr = gatt.getDevice().getAddress();

            if (newState == BluetoothProfile.STATE_CONNECTED) {
                Log.i(TAG, "GATT connected: " + addr);
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                    gatt.requestConnectionPriority(BluetoothGatt.CONNECTION_PRIORITY_HIGH);
                    gatt.requestMtu(247);
                }
                gatt.discoverServices();
            } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                Log.i(TAG, "GATT disconnected: " + addr);
                sConnectedGatts.remove(addr);
                sMidiChars.remove(addr);
                if (sMidiChars.isEmpty()) {
                    sConnected = false;
                }
                try {
                    gatt.close();
                } catch (Exception ignored) {
                }
                safeOnDeviceDisconnected(addr);
                UsbPermission.notifyDevicesChanged();
            }
        }

        @Override
        @SuppressLint("MissingPermission")
        public void onMtuChanged(BluetoothGatt gatt, int mtu, int status) {
            Log.d(TAG, "BLE MTU changed to " + mtu + " (status=" + status + ")");
        }

        @Override
        @SuppressLint("MissingPermission")
        public void onServicesDiscovered(BluetoothGatt gatt, int status) {
            if (gatt.getDevice() == null) {
                return;
            }
            String addr = gatt.getDevice().getAddress();

            if (status != BluetoothGatt.GATT_SUCCESS) {
                Log.w(TAG, "Service discovery failed for " + addr + ": " + status);
                return;
            }

            BluetoothGattService midiService = gatt.getService(BLE_MIDI_SERVICE_UUID);
            if (midiService == null) {
                Log.d(TAG, addr + " has no BLE MIDI service, disconnecting");
                sConnectedGatts.remove(addr);
                gatt.disconnect();
                return;
            }

            BluetoothGattCharacteristic midiChar =
                midiService.getCharacteristic(BLE_MIDI_CHAR_UUID);
            if (midiChar == null) {
                Log.w(TAG, "No MIDI characteristic on " + addr + ", disconnecting");
                sConnectedGatts.remove(addr);
                gatt.disconnect();
                return;
            }

            gatt.setCharacteristicNotification(midiChar, true);

            BluetoothGattDescriptor ccc = midiChar.getDescriptor(CCC_DESCRIPTOR_UUID);
            if (ccc != null) {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    gatt.writeDescriptor(ccc, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
                } else {
                    ccc.setValue(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE);
                    gatt.writeDescriptor(ccc);
                }
            }

            sConnectedGatts.put(addr, gatt);
            sMidiChars.put(addr, midiChar);
            sIsMidiCandidate.put(addr, true);
            sConnected = true;

            String deviceName = sDiscoveredDevices.get(addr);
            if (deviceName == null || deviceName.isEmpty()) {
                deviceName = gatt.getDevice().getName();
            }
            if (deviceName == null || deviceName.isEmpty()) {
                deviceName = "BLE MIDI " + addr;
            }
            sDiscoveredDevices.put(addr, deviceName);

            Log.i(TAG, "BLE MIDI device ready: " + deviceName + " (" + addr + ")");
            safeOnDeviceConnected(addr, deviceName);
            UsbPermission.notifyDevicesChanged();
        }

        @Override
        public void onCharacteristicChanged(BluetoothGatt gatt,
            BluetoothGattCharacteristic characteristic) {
            if (gatt.getDevice() == null) {
                return;
            }
            String addr = gatt.getDevice().getAddress();
            byte[] value = characteristic.getValue();
            if (value != null && value.length > 0) {
                byte[] midiData = parseBleMidiPacket(value);
                if (midiData != null && midiData.length > 0) {
                    safeOnMidiDataReceived(addr, midiData);
                }
            }
        }

        @Override
        public void onCharacteristicChanged(BluetoothGatt gatt,
            BluetoothGattCharacteristic characteristic, byte[] value) {
            if (gatt.getDevice() == null) {
                return;
            }
            String addr = gatt.getDevice().getAddress();
            if (value != null && value.length > 0) {
                byte[] midiData = parseBleMidiPacket(value);
                if (midiData != null && midiData.length > 0) {
                    safeOnMidiDataReceived(addr, midiData);
                }
            }
        }
    };

    private static int expectedDataBytes(int status) {
        if (status < 0x80) {
            return 0;
        }
        if (status < 0xF0) {
            int high = status & 0xF0;
            return (high == 0xC0 || high == 0xD0) ? 1 : 2;
        }
        switch (status) {
            case 0xF1:
            case 0xF3:
                return 1;
            case 0xF2:
                return 2;
            default:
                return 0;
        }
    }

    /**
     * Parses an Apple / MMA BLE-MIDI 1.0 packet and extracts pure MIDI bytes,
     * stripping both the header byte and per-message timestamp_low bytes while
     * preserving MIDI status bytes, running status, real-time messages, and SysEx.
     */
    public static byte[] parseBleMidiPacket(byte[] packet) {
        if (packet == null || packet.length < 3) {
            return null;
        }
        // Byte 0 is the BLE-MIDI header byte (bit 7 = 1, bit 6 = 1)
        if ((packet[0] & 0x80) == 0) {
            return null;
        }

        ByteArrayOutputStream out = new ByteArrayOutputStream(packet.length);
        int i = 1;
        int runningStatus = 0;
        boolean inSysex = false;

        while (i < packet.length) {
            int b = packet[i] & 0xFF;

            if (inSysex) {
                if ((b & 0x80) == 0) {
                    out.write(b);
                    i++;
                    continue;
                }
                // Inside SysEx, a byte with bit 7 set is a timestamp_low byte
                // preceding either 0xF7 (End of SysEx) or a System Real-Time byte.
                if (i + 1 < packet.length && (packet[i + 1] & 0x80) != 0) {
                    int next = packet[i + 1] & 0xFF;
                    if (next == 0xF7) {
                        out.write(0xF7);
                        inSysex = false;
                        i += 2;
                        continue;
                    } else if (next >= 0xF8) {
                        out.write(next);
                        i += 2;
                        continue;
                    }
                }
                // Fallback if device sent 0xF7 without timestamp
                if (b == 0xF7) {
                    out.write(0xF7);
                    inSysex = false;
                }
                i++;
                continue;
            }

            // At the start of an event: if bit 7 is set, it's a timestamp_low byte
            if ((b & 0x80) != 0) {
                i++;
                if (i >= packet.length) {
                    break;
                }
                b = packet[i] & 0xFF;
            }

            // Now b is either a new MIDI status byte (bit 7 == 1) or a running-status data byte (bit 7 == 0)
            if ((b & 0x80) != 0) {
                i++;
                if (b >= 0xF8) {
                    // System Real-Time message (1 byte, does not affect running status)
                    out.write(b);
                    continue;
                }
                if (b == 0xF0) {
                    inSysex = true;
                    runningStatus = 0;
                    out.write(0xF0);
                    continue;
                }
                runningStatus = b;
                out.write(b);
                int needed = expectedDataBytes(runningStatus);
                for (int d = 0; d < needed && i < packet.length; d++) {
                    int dataByte = packet[i] & 0xFF;
                    if ((dataByte & 0x80) != 0) {
                        break;
                    }
                    out.write(dataByte);
                    i++;
                }
                if (runningStatus >= 0xF0) {
                    runningStatus = 0;
                }
            } else {
                // Running status data bytes
                if (runningStatus == 0) {
                    i++;
                    continue;
                }
                int needed = expectedDataBytes(runningStatus);
                if (needed <= 0) {
                    i++;
                    continue;
                }
                out.write(runningStatus);
                for (int d = 0; d < needed && i < packet.length; d++) {
                    int dataByte = packet[i] & 0xFF;
                    if ((dataByte & 0x80) != 0) {
                        break;
                    }
                    out.write(dataByte);
                    i++;
                }
            }
        }

        return out.toByteArray();
    }

    public static void cleanup() {
        stopScan();
        disconnectAll();
    }
}
