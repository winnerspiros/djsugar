#!/bin/bash
# Runs the reference APK and ours through the same screens on one emulator and
# saves <screen>.ref.png and <screen>.ours.png pairs for the twin comparison.
# The reference also gets a tap tour, so its controls can be mapped from the shots.
set -uo pipefail
REF=$1 OURS=$2 OUT=$3
mkdir -p "$OUT/tour"
# The reference identity comes from its own manifest.
BADGE=$("$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)/aapt2" dump badging "$REF")
RPKG=$(echo "$BADGE" | sed -n "s/^package: name='\([^']*\)'.*/\1/p")
RACT=$RPKG/$(echo "$BADGE" | sed -n "s/^launchable-activity: name='\([^']*\)'.*/\1/p")
echo "reference: $RACT"
OPKG=app.djsugar OACT=app.djsugar/android.app.NativeActivity
shot() { sleep "${2:-3}"; adb exec-out screencap -p > "$OUT/$1"; }
rotate() {
  adb shell settings put system accelerometer_rotation 0
  adb shell settings put system user_rotation "$1"
  adb shell wm user-rotation lock "$1" 2>/dev/null || true
  sleep 3
}
# Waits until three shots 4 s apart match, so slow first launches finish loading.
settle() {
  local last="" same=0
  for _ in $(seq 1 "$1"); do
    sleep 4
    local now
    now=$(adb exec-out screencap -p | md5sum)
    if [ "$now" = "$last" ]; then same=$((same + 1)); else same=0; fi
    [ $same -ge 2 ] && return 0
    last=$now
  done
  echo "::warning::screen never settled"
}
start() {
  adb shell am force-stop "$1"
  adb shell am start -W -n "$2" > /dev/null
  sleep 3
  settle "$3"
}
front() { adb shell dumpsys activity activities | grep -q "ResumedActivity.*$1"; }
size() { adb shell wm size | tail -1 | grep -o '[0-9]*x[0-9]*'; }
# Both libraries see the same music.
adb shell mkdir -p /sdcard/Music
for f in platform/android/twin-media/*; do [ -f "$f" ] && adb push "$f" /sdcard/Music/ > /dev/null; done

adb install -r -g "$REF" || { echo "::error::reference install failed"; exit 1; }
adb install -r -g "$OURS" || { echo "::error::our install failed"; exit 1; }
adb shell appops set $RPKG MANAGE_EXTERNAL_STORAGE allow 2>/dev/null || true

# Pass 1: the first screens, both orientations, both apps.
for app in ref ours; do
  if [ $app = ref ]; then P=$RPKG A=$RACT T=60; else P=$OPKG A=$OACT T=10; fi
  rotate 1
  start $P $A $T
  shot "launch-land.$app.png" 2
  adb shell input keyevent KEYCODE_BACK; shot "back-land.$app.png" 2
  start $P $A 30
  rotate 0
  shot "launch-port.$app.png" 4
  adb shell am force-stop $P
done

# Pass 2: tap tour of the reference in landscape without restarting: an 8 by 6
# grid, a shot after each tap, back out of whatever opened, relaunch if it left.
rotate 1
read -r W H <<< "$(size | tr x ' ')"
[ "$W" -lt "$H" ] && { t=$W; W=$H; H=$t; }
start $RPKG $RACT 30
for gy in 0 1 2 3 4 5; do
  for gx in 0 1 2 3 4 5 6 7; do
    x=$(( W * (2 * gx + 1) / 16 )) y=$(( H * (2 * gy + 1) / 12 ))
    adb shell input tap $x $y
    shot "tour/tap-$gx-$gy.png" 2
    adb shell input tap $x $y
    sleep 1
    front "$RPKG" || start $RPKG $RACT 30
  done
done
adb logcat -d > "$OUT/logcat.txt"
grep -E "FATAL|Abort message|died" "$OUT/logcat.txt" | head -20
echo "twin capture done"
