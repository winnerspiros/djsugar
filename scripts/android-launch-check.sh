#!/usr/bin/env bash
# Installs the APK on the running emulator, opens it, and waits up to three
# minutes for its first frame. Keeps the log and a screenshot in launch/.
# Fails if the app stops, never draws, or the screen is all one colour.
set -uo pipefail
mkdir -p launch
adb install -r "$(ls "$1"/*.apk | head -1)" || exit 1
# A track in the phone's Music folder, and the permission to read it.
python3 - <<'E'
import math, struct, wave
with wave.open("launch-tone.wav", "wb") as w:
    w.setnchannels(2); w.setsampwidth(2); w.setframerate(44100)
    w.writeframes(b"".join(struct.pack("<hh", s, s) for s in
        (int(8000 * math.sin(i * 440 * 6.283 / 44100)) for i in range(44100 * 5))))
E
adb shell mkdir -p /sdcard/Music
adb push launch-tone.wav /sdcard/Music/launch-tone.wav
adb shell pm grant org.djsugar.app android.permission.READ_MEDIA_AUDIO || true
# A real phone's screen (2340x1080 at 420 dpi, landscape), not the
# emulator's tiny default.
adb shell wm size 1080x2340
adb shell wm density 420
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 1
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
# The main screen behind the welcome: Back closes it.
adb shell input keyevent 4
sleep 3
adb exec-out screencap -p > launch/screen-main.png
if ! adb shell dumpsys window | grep -E "mCurrentFocus|mFocusedApp" | grep -q org.djsugar.app; then
  echo "::error::Back closed DJ Sugar instead of the welcome"; exit 1
fi
# Each phone page, by its tab along the bottom: four equal tabs across the
# screen (landscape: the longer side across), their middle 45 pixels up.
size=$(adb shell wm size | tail -1 | grep -oE '[0-9]+x[0-9]+')
a=${size%x*}; b=${size#*x}
w=$(( a > b ? a : b )); h=$(( a > b ? b : a ))
adb shell input tap $(( w * 3 / 8 )) $(( h - 45 ))
sleep 2
adb exec-out screencap -p > launch/screen-mix.png
adb shell input tap $(( w * 7 / 8 )) $(( h - 45 ))
sleep 2
adb exec-out screencap -p > launch/screen-library.png
grep -E "DJSugar|AndroidRuntime|F libc|F DEBUG" launch/logcat.txt | tail -80
if grep -qE "DJ Sugar (stopped|crashed)" launch/logcat.txt; then
  echo "::error::DJ Sugar stopped on start (see above)"; exit 1
fi
grep -q "DJSugar *: allowed to read audio files" launch/logcat.txt \
  || echo "::warning::no sign the app may read audio files"
if grep -q "controller mappings not found" launch/logcat.txt; then
  echo "::error::the controller mappings did not load"; exit 1
fi
grep -q "mods: .* in use" launch/logcat.txt || { echo "::error::the mods did not load"; exit 1; }
grep -E "mods: .* left out" launch/logcat.txt && { echo "::error::a shipped mod was left out"; exit 1; }
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
