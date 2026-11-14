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

HOST_ARCH=$(uname -m)  # One of x86_64, arm64

# iOS buildenv is built on macOS (Intel or Apple Silicon) for arm64-ios-release
# The vcpkg triplet for iOS device builds is arm64-ios-release.
# The host triplet (for build tools like moc, rcc, etc.) is x64-osx-min1100-release
# when cross-compiling on Intel, or arm64-osx-min1100-release on Apple Silicon.

# Built by github.com/winnerspiros/vcpkg (branch djsugar-qt6.12): current
# microsoft/vcpkg + Mixxx 2.7 overlays, Qt 6.12.1 branch with Vulkan,
# release-only libraries, Qt without ICU and with only the Basic and Fusion
# Quick Controls styles.
VCPKG_TARGET_TRIPLET="arm64-ios-release"
BUILDENV_NAME="mixxx-deps-2.7-djsugar-arm64-ios-rel-a5313793"
BUILDENV_TAG="${BUILDENV_NAME}"
BUILDENV_SHA256="7df91633fec93cce18716b60d80da162ee0ad38361058916a9500a698d08ea12"

# Host triplet for build tools (moc, rcc, etc.) when cross-compiling for iOS
# On Intel macOS runners, use x64-osx-min1100-release
# On Apple Silicon macOS runners, use arm64-osx-min1100-release
if [ "$HOST_ARCH" = "x86_64" ]; then
    VCPKG_HOST_TRIPLET="x64-osx-min1100-release"
elif [ "$HOST_ARCH" = "arm64" ]; then
    VCPKG_HOST_TRIPLET="arm64-osx-min1100-release"
else
    echo "ERROR: Unsupported architecture detected: $HOST_ARCH"
    echo "iOS buildenv requires macOS (Intel or Apple Silicon)"
    exit 1
fi

BUILDENV_URL="https://github.com/winnerspiros/vcpkg/releases/download/${BUILDENV_TAG}/${BUILDENV_NAME}.zip"
MIXXX_ROOT="$(realpath "$(dirname "$THIS_SCRIPT_NAME")/..")"

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
        BUILDENV_PATH="${BUILDENV_BASEPATH}/${BUILDENV_NAME}"

        export BUILDENV_NAME
        export BUILDENV_BASEPATH
        export BUILDENV_URL
        export BUILDENV_SHA256
        export MIXXX_VCPKG_ROOT="${BUILDENV_PATH}"
        export CMAKE_GENERATOR=Ninja
        export VCPKG_TARGET_TRIPLET="${VCPKG_TARGET_TRIPLET}"
        export VCPKG_HOST_TRIPLET="${VCPKG_HOST_TRIPLET}"
        # QT_HOST_PATH points to the host triplet install prefix inside the buildenv
        # This is needed for cross-compiling Qt for iOS (moc, rcc, qmlcachegen, etc.)
        export QT_HOST_PATH="${BUILDENV_PATH}/installed/${VCPKG_HOST_TRIPLET}"

        echo_exported_variables() {
            echo "BUILDENV_NAME=${BUILDENV_NAME}"
            echo "BUILDENV_BASEPATH=${BUILDENV_BASEPATH}"
            echo "BUILDENV_URL=${BUILDENV_URL}"
            echo "BUILDENV_SHA256=${BUILDENV_SHA256}"
            echo "MIXXX_VCPKG_ROOT=${MIXXX_VCPKG_ROOT}"
            echo "CMAKE_GENERATOR=${CMAKE_GENERATOR}"
            echo "VCPKG_TARGET_TRIPLET=${VCPKG_TARGET_TRIPLET}"
            echo "VCPKG_HOST_TRIPLET=${VCPKG_HOST_TRIPLET}"
            echo "QT_HOST_PATH=${QT_HOST_PATH}"
        }

        if [ -n "${GITHUB_ENV}" ]; then
            echo_exported_variables >> "${GITHUB_ENV}"
        elif [ "$1" != "--profile" ]; then
            echo ""
            echo "Exported environment variables:"
            echo_exported_variables
            echo "You can now configure cmake from the command line in an EMPTY build directory via:"
            echo "cmake -DCMAKE_TOOLCHAIN_FILE=${MIXXX_VCPKG_ROOT}/scripts/buildsystems/vcpkg.cmake \\"
            echo "      -DCMAKE_SYSTEM_NAME=iOS \\"
            echo "      -DCMAKE_OSX_ARCHITECTURES=arm64 \\"
            echo "      -DCMAKE_OSX_DEPLOYMENT_TARGET=14.0 \\"
            echo "      -DQT_HOST_PATH=${QT_HOST_PATH} \\"
            echo "      ${MIXXX_ROOT}"
        fi
        ;;

    *)
        echo "Usage: source $0 {name|setup}"
        return 1
        ;;
esac
