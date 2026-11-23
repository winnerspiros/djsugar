#!/usr/bin/env bash
# Desktop twin: the reference AppImage in its QML skin and our binary, each
# full screen on the same virtual display, shot once settled, then compared.
# Usage: twin.sh REF.AppImage OURS_BINARY OUT_DIR
set -u
REF=$(realpath "$1") OURS=$(realpath "$2") OUT=$3
mkdir -p "$OUT"
OUT=$(realpath "$OUT")
work=$(mktemp -d)
(cd "$work" && chmod +x "$REF" && "$REF" --appimage-extract > /dev/null)
REFBIN="$work/squashfs-root/AppRun"
# Both apps get a fresh home; cargo keeps the real one for the scoring step.
APPHOME="$work/home"
export XDG_RUNTIME_DIR="$work/run"
mkdir -p "$APPHOME/Music" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
cp -r media/. "$APPHOME/Music/" 2> /dev/null || true

# Waits until three shots 3 s apart match, then keeps the last.
settle() {
  local last="" same=0
  for _ in $(seq 1 "$2"); do
    sleep 3
    import -display :9 -window root "$1"
    local now
    now=$(md5sum < "$1")
    if [ "$now" = "$last" ]; then same=$((same + 1)); else same=0; fi
    [ $same -ge 2 ] && return 0
    last=$now
  done
  echo "::warning::$(basename "$1") never settled"
}

for o in land:2400x1080 dex:1600x900; do
  name=${o%%:*} size=${o#*:}
  Xvfb :9 -screen 0 "${size}x24" -nolisten tcp > /dev/null 2>&1 &
  xv=$!
  sleep 2
  export DISPLAY=:9
  HOME=$APPHOME QT_QPA_PLATFORM=xcb "$REFBIN" --qml --full-screen \
    > "$OUT/ref-$name.log" 2>&1 &
  pid=$!
  settle "$OUT/launch-$name.ref.png" 60
  kill $pid 2> /dev/null; wait $pid 2> /dev/null
  HOME=$APPHOME "$OURS" --full-screen > "$OUT/ours-$name.log" 2>&1 &
  pid=$!
  settle "$OUT/launch-$name.ours.png" 30
  kill $pid 2> /dev/null; wait $pid 2> /dev/null
  kill $xv; wait $xv 2> /dev/null
done
rm -rf "$work"
cargo run -q --release --locked -p djsugar --example twin -- "$OUT"
