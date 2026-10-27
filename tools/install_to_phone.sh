#!/bin/bash
# Deploy the locally-built signed APK to a connected phone over ADB.
# From JaviLabarumDJ/mixxx-android (tools/install_to_phone.sh), translated
# to English and pointed at this repo's defaults.
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$DIR"

APK="${1:-mixxx-android-release-signed.apk}"
if [ ! -f "$APK" ]; then
    echo "APK not found: $APK"
    echo "Usage: $0 [path-to-apk]"
    echo "Build it first, e.g.: ./tools/android_buildenv.sh ... && <gradle assemble>"
    exit 1
fi

echo "Waiting for device..."
adb wait-for-device
DEV=$(adb devices | grep -v 'List' | grep -w 'device' | awk '{print $1}' | head -n 1)
if [ -z "$DEV" ]; then
    echo "No device found after adb wait-for-device"
    exit 1
fi
echo "Device detected: $DEV"
export ANDROID_SERIAL="$DEV"

echo "Waiting for Android to finish booting..."
adb -s "$DEV" wait-for-device
for i in $(seq 1 60); do
    BOOT=$(adb -s "$DEV" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')
    if [ "$BOOT" = "1" ]; then
        break
    fi
    sleep 2
done
BOOT=$(adb -s "$DEV" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')
if [ "$BOOT" != "1" ]; then
    echo "Device did not finish booting in time"
    exit 1
fi

echo "Ensuring controller/recordings dir exists on device..."
adb -s "$DEV" shell mkdir -p /storage/emulated/0/Mixxx/controllers

echo "Installing $APK ..."
adb -s "$DEV" install -r -d "$APK"

echo "Copying APK to /sdcard/Download/mixxx-android.apk ..."
adb -s "$DEV" push "$APK" /sdcard/Download/mixxx-android.apk

echo "Launching Mixxx on device..."
adb -s "$DEV" shell am start -S -n org.mixxx/.MainActivity

echo "Done."
