#!/usr/bin/env bash
# Ignored in case of a source call, but needed for bash specific sourcing detection

set -o pipefail

# shellcheck disable=SC2091
if [ -z "${GITHUB_ENV}" ] && ! $(return 0 2>/dev/null); then
  echo "This script must be run by sourcing it:"
  echo "source $0 $*"
  exit 1
fi

realpath() {
    OLDPWD="${PWD}"
    cd "$1" || exit 1
    pwd
    cd "${OLDPWD}" || exit 1
}

# Get script file location, compatible with bash and zsh
if [ -n "$BASH_VERSION" ]; then
  THIS_SCRIPT_NAME="${BASH_SOURCE[0]}"
elif [ -n "$ZSH_VERSION" ]; then
  # shellcheck disable=SC2296
  THIS_SCRIPT_NAME="${(%):-%N}"
else
  THIS_SCRIPT_NAME="$0"
fi

HOST_ARCH=$(uname -m)  # One of x86_64, arm64, i386, ppc or ppc64

# Target ABI of the device Mixxx is built for. arm64-v8a covers phones, ARM
# tablets and ARM Chromebooks. Other ABIs (notably x86_64 for Intel
# Chromebooks and the emulator) require a matching vcpkg dependency tree,
# which is not published as a prebuilt archive yet.
[ -z "$ANDROID_ABI" ] && ANDROID_ABI="arm64-v8a"
if [ "$ANDROID_ABI" != "arm64-v8a" ]; then
    echo "ERROR: No prebuilt dependencies are published for ANDROID_ABI=${ANDROID_ABI}."
    echo "Only arm64-v8a is available as a download. To target another ABI,"
    echo "build the vcpkg environment yourself as described in:"
    echo "https://github.com/mixxxdj/mixxx/wiki/Compiling-dependencies-for-android"
    echo "and then configure with -DANDROID_ABI=${ANDROID_ABI}."
    exit 1
fi

if [ "$HOST_ARCH" == "x86_64" ]; then
    # Built by github.com/winnerspiros/vcpkg (branch djsugar-qt6.12): current
    # microsoft/vcpkg + Mixxx 2.7 overlays, Qt 6.12.1 branch with Vulkan,
    # Oboe from google/oboe main, release-only libraries (NDK 28).
    # BUILDENV_RELEASE is kept for compatibility; there is one buildenv.
    VCPKG_TARGET_TRIPLET="arm64-android-release"
    BUILDENV_NAME="mixxx-deps-2.7-djsugar-arm64-android-rel-48fbdde1"
    BUILDENV_TAG="${BUILDENV_NAME}"
    BUILDENV_SHA256="80e681c484f1a6a7dea9932a6f8773582c739abc7bf6c9e34d93a1a8f8faec33"
else
    echo "ERROR: Unsupported architecture detected: $HOST_ARCH"
    echo "Please refer to the following guide to manually build the vcpkg environment:"
    echo "https://github.com/mixxxdj/mixxx/wiki/Compiling-dependencies-for-android"
    exit 1
fi

BUILDENV_URL="https://github.com/winnerspiros/vcpkg/releases/download/${BUILDENV_TAG}/${BUILDENV_NAME}.zip"
MIXXX_ROOT="$(realpath "$(dirname "$THIS_SCRIPT_NAME")/..")"
ANDROID_API=35
ANDROID_VERSION=35.0.0
# NDK r28 (stable) — ships Clang 19, defaults to 16 KB page sizes (required
# by Pixel 8a+ and any device declaring `ro.build.16k_page=true`), and is
# the first NDK release fully supporting Android 15 (API 35) build-time
# features. Bumping from r27.2 (Clang 18). The `lib/clang/<N>` libomp path
# below is now discovered dynamically so a future NDK bump just works.
ANDROID_NDK=28.0.13004108

[ -z "$BUILDENV_BASEPATH" ] && BUILDENV_BASEPATH="${MIXXX_ROOT}/buildenv"

case "$1" in
    name)
        if [ -n "${GITHUB_ENV}" ]; then
            echo "BUILDENV_NAME=$BUILDENV_NAME" >> "${GITHUB_ENV}"
        else
            echo "$BUILDENV_NAME"
        fi
        ;;

    setup)
        sudo apt-get update && sudo apt-get install -y --no-install-recommends -- \
            ccache \
            cmake \
            make \
            build-essential \
            '^libxcb.*-dev' \
            autoconf \
            autoconf-archive \
            bison \
            flex \
            google-android-cmdline-tools-13.0-installer \
            libasound2-dev \
            libegl1-mesa-dev \
            libghc-resolv-dev \
            libglu1-mesa-dev \
            libltdl-dev \
            libx11-xcb-dev \
            libxi-dev \
            libxkbcommon-dev \
            libxkbcommon-x11-dev \
            libxrender-dev \
            linux-libc-dev \
            openjdk-17-jdk \
            pkg-config \
            python3-jinja2
        (yes | sudo sdkmanager --licenses) || true
        # Qt 6.12 packages the APK with Android Gradle Plugin 9.2, which needs
        # build-tools 36 and only one SDK path in the environment.
        sudo sdkmanager "platforms;android-${ANDROID_API}" "platform-tools" "build-tools;${ANDROID_VERSION}" "build-tools;36.0.0" "ndk;${ANDROID_NDK}"
        ANDROID_SDK=/usr/lib/android-sdk
        ANDROID_HOME="${ANDROID_SDK}"
        ANDROID_SDK_ROOT="${ANDROID_SDK}"
        export ANDROID_HOME
        export ANDROID_SDK_ROOT
        ANDROID_NDK_HOME=/usr/lib/android-sdk/ndk/${ANDROID_NDK}
        JAVA_HOME=$(find /usr/lib/jvm -maxdepth 1 -name 'java-17-openjdk*')
        export ANDROID_SDK
        export ANDROID_NDK_HOME
        export JAVA_HOME
        BUILDENV_PATH="${BUILDENV_BASEPATH}/${BUILDENV_NAME}"

        export BUILDENV_NAME
        export BUILDENV_BASEPATH
        export BUILDENV_URL
        export BUILDENV_SHA256
        export MIXXX_VCPKG_ROOT="${BUILDENV_PATH}"
        export VCPKG_TARGET_TRIPLET="${VCPKG_TARGET_TRIPLET}"
        export ANDROID_ABI="${ANDROID_ABI}"

        echo_exported_variables() {
            echo "ANDROID_SDK=${ANDROID_SDK}"
            echo "ANDROID_HOME=${ANDROID_HOME}"
            echo "ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT}"
            echo "ANDROID_NDK_HOME=${ANDROID_NDK_HOME}"
            echo "JAVA_HOME=${JAVA_HOME}"
            echo "BUILDENV_NAME=${BUILDENV_NAME}"
            echo "BUILDENV_BASEPATH=${BUILDENV_BASEPATH}"
            echo "BUILDENV_URL=${BUILDENV_URL}"
            echo "BUILDENV_SHA256=${BUILDENV_SHA256}"
            echo "MIXXX_VCPKG_ROOT=${MIXXX_VCPKG_ROOT}"
            echo "VCPKG_TARGET_TRIPLET=${VCPKG_TARGET_TRIPLET}"
            echo "ANDROID_ABI=${ANDROID_ABI}"
        }

        if [ -n "${GITHUB_ENV}" ]; then
            echo_exported_variables >> "${GITHUB_ENV}"
        elif [ "$1" != "--profile" ]; then
            echo ""
            echo "Exported environment variables:"
            echo_exported_variables
            echo "You can now configure cmake from the command line in an EMPTY build directory via:"
            echo "cmake -DCMAKE_TOOLCHAIN_FILE=${MIXXX_VCPKG_ROOT}/scripts/buildsystems/vcpkg.cmake -DCMAKE_SYSTEM_NAME=Android -DANDROID_ABI=${ANDROID_ABI} ${MIXXX_ROOT}"
        fi
        ;;
    *)
        echo "Usage: source android_buildenv.sh [options]"
        echo ""
        echo "options:"
        echo "   help       Displays this help."
        echo "   name       Displays the name of the required build environment."
        echo "   setup      Setup the build environment variables for download during CMake configuration and install the build environment."
        ;;
esac
