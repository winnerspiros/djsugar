#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
SDK=${ANDROID_HOME:-$HOME/android-sdk}
NDK=${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}
BT=$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)
export PATH="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
TARGET=${TARGET:-aarch64-linux-android}
ABI=${ABI:-arm64-v8a}
cargo build --release --target "$TARGET" -p djsugar --lib
OUT=target/android
rm -rf "$OUT" && mkdir -p "$OUT/lib/$ABI" "$OUT/assets"
cp -r res/samples "$OUT/assets/samples"
rm -f "$OUT/assets/samples/README.md"
cp "target/$TARGET/release/libdjsugar.so" "$OUT/lib/$ABI/"
llvm-strip --strip-unneeded "$OUT/lib/$ABI/libdjsugar.so"
"$BT/aapt2" compile --dir platform/android/res -o "$OUT/res.zip"
"$BT/aapt2" link "$OUT/res.zip" -o "$OUT/base.apk" --manifest platform/android/AndroidManifest.xml -A "$OUT/assets" -I "$(ls -d "$SDK"/platforms/* | sort -V | tail -1)/android.jar"
(cd "$OUT" && zip -q -0 base.apk "lib/$ABI/libdjsugar.so")
"$BT/zipalign" -f -P 16 4 "$OUT/base.apk" "$OUT/aligned.apk"
KS=${KEYSTORE:-$HOME/.android/debug.keystore}
[ -f "$KS" ] || { mkdir -p "$(dirname "$KS")"; keytool -genkeypair -keystore "$KS" -storepass android -keypass android -alias androiddebugkey -dname CN=debug -keyalg RSA -validity 10000; }
"$BT/apksigner" sign --ks "$KS" --ks-pass pass:android --out target/djsugar.apk "$OUT/aligned.apk"
echo target/djsugar.apk
