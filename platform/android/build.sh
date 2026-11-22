#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
SDK=${ANDROID_HOME:-$HOME/android-sdk}
NDK=${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}
BT=$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)
export PATH="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
cargo build --release --target aarch64-linux-android -p djsugar --lib
OUT=target/android
rm -rf "$OUT" && mkdir -p "$OUT/lib/arm64-v8a"
cp target/aarch64-linux-android/release/libdjsugar.so "$OUT/lib/arm64-v8a/"
llvm-strip --strip-unneeded "$OUT/lib/arm64-v8a/libdjsugar.so"
"$BT/aapt2" link -o "$OUT/base.apk" --manifest platform/android/AndroidManifest.xml -I "$(ls -d "$SDK"/platforms/* | sort -V | tail -1)/android.jar"
(cd "$OUT" && zip -q -0 base.apk lib/arm64-v8a/libdjsugar.so)
"$BT/zipalign" -f -P 16 4 "$OUT/base.apk" "$OUT/aligned.apk"
KS=${KEYSTORE:-$HOME/.android/debug.keystore}
[ -f "$KS" ] || keytool -genkeypair -keystore "$KS" -storepass android -keypass android -alias androiddebugkey -dname CN=debug -keyalg RSA -validity 10000
"$BT/apksigner" sign --ks "$KS" --ks-pass pass:android --out target/djsugar.apk "$OUT/aligned.apk"
echo target/djsugar.apk
