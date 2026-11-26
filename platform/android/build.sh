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
# The controller mappings, and a mark of what they are, so the app unpacks them again only when they changed.
cp -r res/controllers "$OUT/assets/controllers"
(cd res/controllers && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 cat | sha256sum | cut -d' ' -f1) > "$OUT/assets/controllers.mark"
cp "target/$TARGET/release/libdjsugar.so" "$OUT/lib/$ABI/"
llvm-strip --strip-unneeded "$OUT/lib/$ABI/libdjsugar.so"
"$BT/aapt2" compile --dir platform/android/res -o "$OUT/res.zip"
"$BT/aapt2" link "$OUT/res.zip" -o "$OUT/base.apk" --manifest platform/android/AndroidManifest.xml -A "$OUT/assets" -I "$(ls -d "$SDK"/platforms/* | sort -V | tail -1)/android.jar"
# Packed small: the download is about half, and the phone unpacks the library when it installs the app.
(cd "$OUT" && zip -q -9 base.apk "lib/$ABI/libdjsugar.so")
"$BT/zipalign" -f -P 16 4 "$OUT/base.apk" "$OUT/aligned.apk"
KS=${KEYSTORE:-$HOME/.android/debug.keystore}
[ -f "$KS" ] || { mkdir -p "$(dirname "$KS")"; keytool -genkeypair -keystore "$KS" -storepass android -keypass android -alias androiddebugkey -dname CN=debug -keyalg RSA -validity 10000; }
"$BT/apksigner" sign --ks "$KS" --ks-pass pass:android --out target/djsugar.apk "$OUT/aligned.apk"
echo target/djsugar.apk
