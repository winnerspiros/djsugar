#!/bin/bash
# Installs the APK on a running emulator and drives it like a user: portrait and
# landscape, view tabs, typing into search, home and back. Fails on any crash.
set -uo pipefail
APK=$1 OUT=$2 PKG=app.djsugar
mkdir -p "$OUT"
fail() { echo "::error::$*"; adb logcat -d > "$OUT/logcat.txt"; exit 1; }
shot() { sleep 2; adb exec-out screencap -p > "$OUT/$1.png"; }
alive() { adb shell pidof $PKG > /dev/null || fail "app died: $1"; }
size() { adb shell wm size | tail -1 | grep -o '[0-9]*x[0-9]*'; }
adb install -r -g "$APK" || fail "install failed"
adb logcat -c
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 0
adb shell am start -W -n $PKG/android.app.NativeActivity || fail "launch failed"
sleep 6
alive launch
shot portrait
read -r W H <<< "$(size | tr x ' ')"
# Bottom-row tabs sit in the first toolbar row; tap across it.
for x in 0.35 0.45 0.55 0.65 0.75 0.85 0.95; do
  adb shell input tap "$(awk "BEGIN{print int($W*$x)}")" 40
  sleep 1
  alive "tab at $x"
done
shot portrait-tabs
adb shell settings put system user_rotation 1
sleep 4
alive rotate
shot landscape
adb shell input keyevent KEYCODE_HOME
sleep 2
adb shell am start -n $PKG/android.app.NativeActivity
sleep 4
alive resume
shot resumed
adb shell input text "daft"
sleep 1
alive typing
adb logcat -d > "$OUT/logcat.txt"
if grep -E "panicked|FATAL EXCEPTION|SIGSEGV|SIGABRT" "$OUT/logcat.txt"; then
  fail "crash in logcat"
fi
echo "emulator smoke passed"
