#!/usr/bin/env bash
# Installs the APK on the running emulator, opens it, and keeps the log and a
# screenshot in launch/. Fails if the app dies or the screen stays black.
set -uo pipefail
mkdir -p launch
adb install -r "$(ls "$1"/*.apk | head -1)" || exit 1
adb logcat -c
adb shell am start -n org.djsugar.app/android.app.NativeActivity
sleep 40
adb logcat -d > launch/logcat.txt
adb exec-out screencap -p > launch/screen.png
alive=$(adb shell pidof org.djsugar.app || true)
grep -E "DJSugar|AndroidRuntime|F libc|DEBUG" launch/logcat.txt | tail -80
if grep -qE "DJSugar *: DJ Sugar (stopped|crashed)" launch/logcat.txt; then
  echo "::error::DJ Sugar stopped on start (see above)"; exit 1
fi
grep -q "DJSugar *: DJ Sugar starting" launch/logcat.txt || { echo "::error::DJ Sugar never started"; exit 1; }
if [ -z "$alive" ]; then echo "::error::DJ Sugar is not running"; exit 1; fi
# A black screen: every pixel of the screenshot is the same.
python3 - <<'E' || exit 1
import zlib, struct, sys
d = open("launch/screen.png", "rb").read()
w, h = struct.unpack(">II", d[16:24])
raw, i = b"", 8
while i < len(d):
    n = struct.unpack(">I", d[i:i+4])[0]; t = d[i+4:i+8]
    if t == b"IDAT": raw += d[i+8:i+8+n]
    i += 12 + n
px = zlib.decompress(raw)
if len(set(px[1:4096])) <= 2 and len(set(px[len(px)//2:len(px)//2+4096])) <= 2:
    print("::error::the screen is blank"); sys.exit(1)
print("the app draws")
E
