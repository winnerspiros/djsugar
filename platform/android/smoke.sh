#!/bin/bash
# Drives the APK on an emulator like a user and collects what is needed to debug it:
# screenshots, logcat, frame stats and memory. Fails on a crash or a dead process.
set -uo pipefail
APK=$1 OUT=$2 PKG=app.djsugar ACT=app.djsugar/android.app.NativeActivity
mkdir -p "$OUT"
SUM=${GITHUB_STEP_SUMMARY:-/dev/null}
collect() {
  adb logcat -d > "$OUT/logcat.txt"
  adb shell dumpsys gfxinfo $PKG > "$OUT/gfxinfo.txt"
  adb shell dumpsys meminfo $PKG > "$OUT/meminfo.txt"
  {
    echo "### Emulator"
    echo '```'
    grep -E "Total frames|Janky|percentile" "$OUT/gfxinfo.txt"
    grep -E "TOTAL PSS|Native Heap|Graphics" "$OUT/meminfo.txt" | head -5
    grep -E " djsugar *:" "$OUT/logcat.txt" | tail -30
    grep -E "ANR in|Reason:|FATAL|panicked" "$OUT/logcat.txt" | head -10
    echo '```'
  } >> "$SUM"
}
fail() {
  echo "::error::$*"
  pid=$(adb shell pidof $PKG)
  [ -n "$pid" ] && adb shell debuggerd -b "$pid" > "$OUT/stacks.txt" 2>&1
  [ -s "$OUT/stacks.txt" ] && { echo '### Thread stacks at failure'; echo '```'; grep -E '^"|#0[0-9] ' "$OUT/stacks.txt" | head -120; echo '```'; } >> "$SUM"
  collect
  exit 1
}
shot() { sleep 2; adb exec-out screencap -p > "$OUT/$1.png"; }
frozen() {
  adb shell dumpsys window | grep -q "Application Not Responding: $PKG" && return 0
  adb logcat -d -b events | grep -q "am_anr.*$PKG"
}
alive() {
  adb shell pidof $PKG > /dev/null || fail "app died: $1"
  if frozen; then shot "frozen-$(echo "$1" | tr -c 'a-z0-9' '-')"; fail "app not responding after: $1"; fi
}
drawn() {
  for _ in $(seq 1 "$2"); do
    adb logcat -d -s djsugar | grep -q "first frame" && return 0
    sleep 1
  done
  fail "no first frame within $2 s after: $1"
}
D=$(adb shell wm density | grep -o '[0-9]*' | tail -1)
px() { awk "BEGIN{print int($1*$D/160)}"; }
tap() { adb shell input tap "$(px "$1")" "$(px "$2")"; sleep 1; alive "tap $1,$2"; }
adb install -r -g "$APK" || fail "install failed"
adb logcat -c
adb logcat -b events -c
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 0
adb shell am start -W -n $ACT || fail "launch failed"
drawn launch 30
adb logcat -d -s djsugar | tee "$OUT/startup.txt"
alive launch
shot 01-portrait
read -r W _ <<< "$(adb shell wm size | tail -1 | grep -o '[0-9]*x[0-9]*' | tr x ' ')"
WD=$(awk "BEGIN{print $W*160/$D}")
for i in 0 1 2 3 4 5; do
  tap "$(awk "BEGIN{print 100+($WD-106)*($i+0.5)/6}")" 18
  shot "02-tab-$i"
done
tap 60 90
adb shell input text "daft"
sleep 1
alive typing
shot 03-typed
adb shell input keyevent KEYCODE_BACK
sleep 1
adb shell settings put system user_rotation 1
sleep 5
alive rotate
read -r RW RH <<< "$(adb exec-out screencap -p | python3 -c 'import sys,struct;d=sys.stdin.buffer.read(24);print(*struct.unpack(">II",d[16:24]))')"
[ "$RW" -gt "$RH" ] || fail "screen did not rotate to landscape (${RW}x${RH})"
shot 04-landscape
adb shell input keyevent KEYCODE_HOME
sleep 3
adb logcat -c
adb shell am start -n $ACT
drawn resume 20
alive resume
shot 05-resumed
adb shell input keyevent KEYCODE_APP_SWITCH
sleep 2
adb shell input keyevent KEYCODE_BACK
sleep 3
adb shell am start -n $ACT
sleep 4
alive "app switcher"
shot 06-after-switcher
collect
if grep -E "panicked|FATAL EXCEPTION|SIGSEGV|SIGABRT" "$OUT/logcat.txt"; then
  fail "crash in logcat"
fi
echo "emulator smoke passed"
