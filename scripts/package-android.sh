#!/usr/bin/env bash
# Packages DJ Sugar for Android arm64 into dist/DJSugar-<version>-android-arm64.apk,
# without Gradle: the native library is the whole app (a NativeActivity).
#
#   scripts/package-android.sh <version>
#
# Needs ANDROID_HOME (build-tools and an android-35 platform), ANDROID_NDK_HOME
# and cargo-ndk. Signs with ANDROID_KEYSTORE (base64) and ANDROID_KEYSTORE_PASSWORD
# when set, else with a throwaway debug key (installable, not updatable from
# another build).
set -euo pipefail

version="${1:-0.0.0}"
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/target/android-apk"
tools="$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)"
platform="$ANDROID_HOME/platforms/android-35/android.jar"
[ -f "$platform" ] || yes | "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" "platforms;android-35" >/dev/null
# Slint's Android backend compiles a Java helper against the same platform.
export ANDROID_JAR="$platform"
rm -rf "$out" && mkdir -p "$out/lib/arm64-v8a" "$root/dist"

cd "$root"
# PROFILE=release for a quicker build than the dist profile's fat LTO.
profile="${PROFILE:-dist}"
cargo ndk -t arm64-v8a -P 29 build --profile "$profile" -p djsugar-android --locked
cp "target/aarch64-linux-android/$profile/libdjsugar_android.so" "$out/lib/arm64-v8a/"
# The C++ runtime whisper.cpp and other C++ dependencies link against.
cp "$(find "$ANDROID_NDK_HOME" -path '*aarch64-linux-android/libc++_shared.so' | head -1)" \
  "$out/lib/arm64-v8a/"

sed "s/android:versionName=\"[^\"]*\"/android:versionName=\"$version\"/" \
  res/android/AndroidManifest.xml > "$out/AndroidManifest.xml"
# Controller mappings and the app's own mods as assets, with a list of them: Android can't list
# folders inside the APK. The app copies them out on start.
mkdir -p "$out/assets"
for part in controllers mods; do
  cp -R "res/$part" "$out/assets/$part"
  (cd "$out/assets/$part" && find . -type f ! -name index.txt | sed 's|^\./||' | sort > index.txt)
done
"$tools/aapt2" link -o "$out/base.apk" --manifest "$out/AndroidManifest.xml" -I "$platform" \
  -A "$out/assets"
# Native libraries stored uncompressed, so they load straight from the APK.
(cd "$out" && zip -q -0 -r base.apk lib)
"$tools/zipalign" -f -P 16 4 "$out/base.apk" "$out/aligned.apk"

if [ -n "${ANDROID_KEYSTORE:-}" ]; then
  echo "$ANDROID_KEYSTORE" | base64 --decode > "$out/release.jks"
  ks="$out/release.jks" pass="$ANDROID_KEYSTORE_PASSWORD"
else
  ks="$out/debug.jks" pass="android"
  keytool -genkeypair -keystore "$ks" -storepass "$pass" -keypass "$pass" -alias djsugar \
    -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=DJ Sugar debug" >/dev/null 2>&1
fi
apk="$root/dist/DJSugar-$version-android-arm64.apk"
"$tools/apksigner" sign --ks "$ks" --ks-pass "pass:$pass" --out "$apk" "$out/aligned.apk"
"$tools/apksigner" verify "$apk"
ls -l "$apk"
