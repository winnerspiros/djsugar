#!/usr/bin/env bash
# build_lv2_android.sh — Build selected LV2 plugins for Android ARM64
# and bundle them into packaging/android/assets/lv2/
#
# Prerequisites:
#   - ANDROID_NDK_HOME set
#   - CMake >= 3.21
#   - Submodules initialized: lib/calf-plugins, lib/dragonfly-reverb

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${PROJECT_ROOT}/build-lv2-android"
ASSETS_DIR="${PROJECT_ROOT}/packaging/android/assets/lv2"

# Android NDK
: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME must be set}"
NDK="${ANDROID_NDK_HOME}"
TOOLCHAIN="${NDK}/build/cmake/android.toolchain.cmake"

# Target
ANDROID_ABI="arm64-v8a"
ANDROID_PLATFORM="android-35"
ANDROID_STL="c++_shared"

# Number of parallel jobs
JOBS=$(nproc 2>/dev/null || echo 4)

# NDK toolchain binaries
TC_PREFIX="${NDK}/toolchains/llvm/prebuilt/linux-x86_64"
export CC="${TC_PREFIX}/bin/aarch64-linux-android35-clang"
export CXX="${TC_PREFIX}/bin/aarch64-linux-android35-clang++"
export AR="${TC_PREFIX}/bin/llvm-ar"
export RANLIB="${TC_PREFIX}/bin/llvm-ranlib"
export STRIP="${TC_PREFIX}/bin/llvm-strip"

echo "=== Building LV2 plugins for Android ARM64 ==="
echo "NDK: ${NDK}"
echo "Build dir: ${BUILD_DIR}"
echo "Assets dir: ${ASSETS_DIR}"
echo "Parallel jobs: ${JOBS}"

rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"
mkdir -p "${ASSETS_DIR}"

###############################################################################
# 0. Build EXPAT for Android (required by Calf)
###############################################################################
build_expat() {
    echo ""
    echo "=== Building EXPAT for Android ==="

    local expat_src="${PROJECT_ROOT}/expat-src"
    local expat_install="${BUILD_DIR}/install/expat"

    if [ ! -d "${expat_src}" ]; then
        git clone --depth 1 https://github.com/libexpat/libexpat.git "${expat_src}"
    fi

    cd "${expat_src}"
    rm -rf build-android
    mkdir -p build-android && cd build-android

    cmake ../expat \
        -G Ninja \
        -DCMAKE_TOOLCHAIN_FILE="${TOOLCHAIN}" \
        -DANDROID_ABI="${ANDROID_ABI}" \
        -DANDROID_PLATFORM="${ANDROID_PLATFORM}" \
        -DANDROID_STL="${ANDROID_STL}" \
        -DCMAKE_BUILD_TYPE=MinSizeRel \
        -DCMAKE_INSTALL_PREFIX="${expat_install}" \
        -DEXPAT_BUILD_EXAMPLES=OFF \
        -DEXPAT_BUILD_TESTS=OFF \
        -DEXPAT_SHARED_LIBS=ON

    cmake --build . --parallel "${JOBS}"
    cmake --install .

    echo "=== EXPAT built ==="
}

###############################################################################
# 1. Calf Studio Gear (CMake-based)
###############################################################################
build_calf_plugins() {
    echo ""
    echo "=== Building Calf Plugins (LV2 only) ==="

    local calf_src="${PROJECT_ROOT}/lib/calf-plugins"
    if [ ! -d "${calf_src}" ]; then
        echo "WARNING: lib/calf-plugins not found, skipping"
        return
    fi

    local calf_build="${BUILD_DIR}/calf-plugins"
    local calf_install="${BUILD_DIR}/install/calf-plugins"
    local expat_install="${BUILD_DIR}/install/expat"

    mkdir -p "${calf_build}"
    cd "${calf_build}"

    cmake "${calf_src}" \
        -G Ninja \
        -DCMAKE_TOOLCHAIN_FILE="${TOOLCHAIN}" \
        -DANDROID_ABI="${ANDROID_ABI}" \
        -DANDROID_PLATFORM="${ANDROID_PLATFORM}" \
        -DANDROID_STL="${ANDROID_STL}" \
        -DCMAKE_BUILD_TYPE=MinSizeRel \
        -DCMAKE_INSTALL_PREFIX="${calf_install}" \
        -DCMAKE_C_FLAGS="-ffunction-sections -fdata-sections -fvisibility=hidden -O2" \
        -DCMAKE_CXX_FLAGS="-ffunction-sections -fdata-sections -fvisibility=hidden -fvisibility-inlines-hidden -O2" \
        -DCMAKE_SHARED_LINKER_FLAGS="-Wl,--gc-sections,--icf=safe,-O2,-s" \
        -DWANT_GUI=OFF \
        -DWANT_JACK=OFF \
        -DWANT_LASH=OFF \
        -DWANT_LV2=ON \
        -DWANT_LV2_GUI=OFF \
        -DWANT_SORDI=OFF \
        -DWANT_EXPERIMENTAL=OFF \
        -DWANT_SSE=OFF \
        -DBUILD_SHARED_LIBS=ON \
        -DCMAKE_FIND_ROOT_PATH="${expat_install}" \
        -DEXPAT_INCLUDE_DIR="${expat_install}/include" \
        -DEXPAT_LIBRARY="${expat_install}/lib/libexpat.so"

    cmake --build . --parallel "${JOBS}"
    cmake --install .

    echo "=== Calf Plugins built ==="
}

###############################################################################
# 2. Dragonfly Reverb (DPF Makefile-based)
###############################################################################
build_dragonfly() {
    echo ""
    echo "=== Building Dragonfly Reverb ==="

    local df_src="${PROJECT_ROOT}/lib/dragonfly-reverb"
    if [ ! -d "${df_src}" ]; then
        echo "WARNING: lib/dragonfly-reverb not found, skipping"
        return
    fi

    cd "${df_src}"

    # Install LV2 headers into NDK sysroot for cross-compilation
    local ndk_sysroot="${TC_PREFIX}/sysroot"
    for src in /usr/include/lv2.h \
              /usr/include/lv2 \
              /usr/include/lilv-0; do
        if [ -e "$src" ]; then
            dest="${ndk_sysroot}/usr/include/$(basename $src)"
            mkdir -p "$(dirname $dest)"
            cp -r "$src" "$dest"
        fi
    done

    # Build with DPF Makefile, cross-compiling via CC/CXX
    make -j"${JOBS}" \
        CC="${CC}" \
        CXX="${CXX}" \
        AR="${AR}" \
        RANLIB="${RANLIB}" \
        STRIP="${STRIP}" \
        CFLAGS="--target=aarch64-linux-android35 -ffunction-sections -fdata-sections -fvisibility=hidden -O2" \
        CXXFLAGS="--target=aarch64-linux-android35 -ffunction-sections -fdata-sections -fvisibility=hidden -fvisibility-inlines-hidden -O2" \
        LDFLAGS="--target=aarch64-linux-android35 -Wl,--gc-sections,-O2,-s" \
        CROSS_COMPILING=true \
        plugins

    # Copy LV2 bundles
    local df_install="${BUILD_DIR}/install/dragonfly-reverb"
    mkdir -p "${df_install}"
    for plugin in DragonflyHallReverb DragonflyRoomReverb \
                 DragonflyPlateReverb DragonflyEarlyReflections; do
        if [ -d "bin/${plugin}.lv2" ]; then
            echo "  Copying: ${plugin}.lv2"
            cp -r "bin/${plugin}.lv2" "${df_install}/${plugin}.lv2"
        fi
    done

    echo "=== Dragonfly Reverb built ==="
}

###############################################################################
# 3. Bundle all LV2 plugins into assets directory
###############################################################################
bundle_plugins() {
    echo ""
    echo "=== Bundling LV2 plugins ==="

    find "${BUILD_DIR}/install" -name "*.lv2" -type d 2>/dev/null | while IFS= read -r bundle; do
        name="$(basename "${bundle}")"
        echo "  Bundling: ${name}"
        cp -r "${bundle}" "${ASSETS_DIR}/${name}"
    done

    echo ""
    echo "=== LV2 bundles created ==="
    ls -la "${ASSETS_DIR}/" 2>/dev/null || echo "  (none found)"
}

###############################################################################
# Main
###############################################################################
main() {
    echo "Starting LV2 Android build..."
    echo "Timestamp: $(date -u)"

    # Check prerequisites
    if [ ! -f "${TOOLCHAIN}" ]; then
        echo "ERROR: Android NDK toolchain not found at ${TOOLCHAIN}"
        echo "Set ANDROID_NDK_HOME to your NDK installation"
        exit 1
    fi

    # Initialize submodules if needed (fail fast on error)
    cd "${PROJECT_ROOT}"
    git submodule update --init lib/calf-plugins lib/dragonfly-reverb

    # Build
    build_expat
    build_calf_plugins
    build_dragonfly

    # Bundle
    bundle_plugins

    echo ""
    echo "=== DONE ==="
    echo "LV2 plugins are in: ${ASSETS_DIR}"
    echo "Timestamp: $(date -u)"
}

main "$@"
