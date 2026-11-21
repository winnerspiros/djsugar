#!/usr/bin/env bash
# Installs the APK on the running emulator, opens it, and waits up to three
# minutes for its first frame. Keeps the log and a screenshot in launch/.
# Fails if the app stops, never draws, or the screen is all one colour.
set -uo pipefail
mkdir -p launch
adb install -r "$(ls "$1"/*.apk | head -1)" || exit 1
adb logcat -c
adb shell am start -n org.djsugar.app/android.app.NativeActivity
drawn=""
for _ in $(seq 36); do
  sleep 5
  if adb logcat -d -s DJSugar | grep -q "first frame drawn"; then drawn=1; sleep 5; break; fi
  adb logcat -d -s DJSugar | grep -qE "DJ Sugar (stopped|crashed)" && break
done
adb logcat -d > launch/logcat.txt
adb exec-out screencap -p > launch/screen.png
adb exec-out screencap > launch/screen.raw
grep -E "DJSugar|AndroidRuntime|F libc|F DEBUG" launch/logcat.txt | tail -80
if grep -qE "DJ Sugar (stopped|crashed)" launch/logcat.txt; then
  echo "::error::DJ Sugar stopped on start (see above)"; exit 1
fi
[ -n "$drawn" ] || { echo "::error::no frame drawn in three minutes"; exit 1; }
# screencap's raw form: width, height, format (and on newer Android a colour
# space), then RGBA pixels.
python3 - <<'E'
import struct, sys
d = open("launch/screen.raw", "rb").read()
w, h = struct.unpack("<II", d[:8])
px = d[len(d) - w * h * 4:]
colours = {px[i:i + 3] for i in range(0, len(px), 4 * 97)}
print(f"{w}x{h}, {len(colours)} colours sampled")
if len(colours) < 8:
    print("::error::the screen is blank"); sys.exit(1)
E
