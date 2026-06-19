#!/usr/bin/env bash
# build_lv2_android.sh — Build selected LV2 plugins for Android ARM64
# and bundle them into packaging/android/assets/lv2/
#
# Prerequisites:
#   - ANDROID_NDK_HOME set
#   - CMake >= 3.21
#   - Submodules initialized: lib/lsp-plugins, lib/calf-plugins, lib/dragonfly-reverb

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

echo "=== Building LV2 plugins for Android ARM64 ==="
echo "NDK: ${NDK}"
echo "Build dir: ${BUILD_DIR}"
echo "Assets dir: ${ASSETS_DIR}"
echo "Parallel jobs: ${JOBS}"

rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"
mkdir -p "${ASSETS_DIR}"

###############################################################################
# Helper: build a CMake-based LV2 project
###############################################################################
build_cmake_lv2() {
    local name="$1"
    local src_dir="$2"
    local build_subdir="${BUILD_DIR}/${name}"
    local install_prefix="${BUILD_DIR}/install/${name}"

    echo ""
    echo "=== Building ${name} ==="

    mkdir -p "${build_subdir}"
    cd "${build_subdir}"

    cmake "${src_dir}" \
        -G "Unix Makefiles" \
        -DCMAKE_TOOLCHAIN_FILE="${TOOLCHAIN}" \
        -DANDROID_ABI="${ANDROID_ABI}" \
        -DANDROID_PLATFORM="${ANDROID_PLATFORM}" \
        -DANDROID_STL="${ANDROID_STL}" \
        -DCMAKE_BUILD_TYPE=MinSizeRel \
        -DCMAKE_INSTALL_PREFIX="${install_prefix}" \
        -DCMAKE_C_FLAGS="-ffunction-sections -fdata-sections -fvisibility=hidden -O2" \
        -DCMAKE_CXX_FLAGS="-ffunction-sections -fdata-sections -fvisibility=hidden -fvisibility-inlines-hidden -O2" \
        -DCMAKE_SHARED_LINKER_FLAGS="-Wl,--gc-sections,--icf=safe,-O2,-s" \
        "${@:3}"

    cmake --build . --parallel "${JOBS}"
    cmake --install .

    echo "=== ${name} built successfully ==="
}

###############################################################################
# 1. LSP Plugins (custom make-based build system)
###############################################################################
build_lsp_plugins() {
    echo ""
    echo "=== Building LSP Plugins (selected) ==="

    local lsp_src="${PROJECT_ROOT}/lib/lsp-plugins"
    local lsp_install="${BUILD_DIR}/install/lsp-plugins"

    cd "${lsp_src}"

    # Configure for Android cross-compilation
    # LSP uses a custom Makefile system; we need to set cross-compilation vars
    export CC="${NDK}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android35-clang"
    export CXX="${NDK}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android35-clang++"
    export AR="${NDK}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
    export RANLIB="${NDK}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"
    export STRIP="${NDK}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip"
    export SYSROOT="${NDK}/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
    export CFLAGS="--sysroot=${SYSROOT} -ffunction-sections -fdata-sections -fvisibility=hidden -O2 -fmerge-all-constants"
    export CXXFLAGS="--sysroot=${SYSROOT} -ffunction-sections -fdata-sections -fvisibility=hidden -fvisibility-inlines-hidden -O2 -fmerge-all-constants"
    export LDFLAGS="--sysroot=${SYSROOT} -Wl,--gc-sections,--icf=safe,-O2,-s"

    # Build only the plugins we need (much faster than building all)
    # Selected: slap-delay, filter, flanger, phaser, compressor, clipper,
    #           para-equalizer, noise-generator, beat-breather
    make config \
        ARCH=aarch64 \
        HOST_ARCH=x86_64 \
        CROSS_COMPILE=1 \
        SYSROOT="${SYSROOT}" \
        CC="${CC}" \
        CXX="${CXX}" \
        AR="${AR}" \
        RANLIB="${RANLIB}" \
        CFLAGS="${CFLAGS}" \
        CXXFLAGS="${CXXFLAGS}" \
        LDFLAGS="${LDFLAGS}" \
        VERBOSE=1 \
        -j"${JOBS}" || true

    # Build individual plugins
    local plugins=(
        "slap-delay"
        "filter"
        "flanger"
        "phaser"
        "compressor"
        "clipper"
        "para-equalizer"
        "noise-generator"
        "beat-breather"
    )

    for plugin in "${plugins[@]}"; do
        echo "Building LSP plugin: ${plugin}"
        make -j"${JOBS}" "${plugin}" || echo "WARNING: ${plugin} build failed, skipping"
    done

    # Install — copy LV2 bundles to install dir
    mkdir -p "${lsp_install}"
    find "${lsp_src}/.build" -name "*.lv2" -type d -exec cp -r {} "${lsp_install}/" \; 2>/dev/null || true

    echo "=== LSP Plugins built ==="
}

###############################################################################
# 2. Calf Studio Gear (CMake-based, build only LV2 plugins we want)
###############################################################################
build_calf_plugins() {
    echo ""
    echo "=== Building Calf Plugins (selected) ==="

    local calf_src="${PROJECT_ROOT}/lib/calf-plugins"
    local calf_build="${BUILD_DIR}/calf-plugins"
    local calf_install="${BUILD_DIR}/install/calf-plugins"

    mkdir -p "${calf_build}"
    cd "${calf_build}"

    # Build Calf with only LV2, no GUI, no JACK
    cmake "${calf_src}" \
        -G "Unix Makefiles" \
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
        -DBUILD_SHARED_LIBS=ON

    cmake --build . --parallel "${JOBS}"
    cmake --install .

    echo "=== Calf Plugins built ==="
}

###############################################################################
# 3. Dragonfly Reverb (CMake-based, already in repo)
###############################################################################
build_dragonfly() {
    echo ""
    echo "=== Building Dragonfly Reverb ==="

    local df_src="${PROJECT_ROOT}/lib/dragonfly-reverb"
    local df_build="${BUILD_DIR}/dragonfly-reverb"
    local df_install="${BUILD_DIR}/install/dragonfly-reverb"

    mkdir -p "${df_build}"
    cd "${df_build}"

    cmake "${df_src}" \
        -G "Unix Makefiles" \
        -DCMAKE_TOOLCHAIN_FILE="${TOOLCHAIN}" \
        -DANDROID_ABI="${ANDROID_ABI}" \
        -DANDROID_PLATFORM="${ANDROID_PLATFORM}" \
        -DANDROID_STL="${ANDROID_STL}" \
        -DCMAKE_BUILD_TYPE=MinSizeRel \
        -DCMAKE_INSTALL_PREFIX="${df_install}" \
        -DCMAKE_C_FLAGS="-ffunction-sections -fdata-sections -fvisibility=hidden -O2" \
        -DCMAKE_CXX_FLAGS="-ffunction-sections -fdata-sections -fvisibility=hidden -fvisibility-inlines-hidden -O2" \
        -DCMAKE_SHARED_LINKER_FLAGS="-Wl,--gc-sections,--icf=safe,-O2,-s"

    cmake --build . --parallel "${JOBS}"
    cmake --install .

    echo "=== Dragonfly Reverb built ==="
}

###############################################################################
# 4. Copy all LV2 bundles to assets
###############################################################################
bundle_plugins() {
    echo ""
    echo "=== Bundling LV2 plugins into assets ==="

    local bundle_count=0

    # Copy LSP plugins
    if [ -d "${BUILD_DIR}/install/lsp-plugins" ]; then
        # shellcheck disable=SC2034
        find "${BUILD_DIR}/install/lsp-plugins" -name "*.lv2" -type d | while read -r bundle; do
            bundle_name=$(basename "${bundle}")
            echo "  Bundling: ${bundle_name}"
            cp -r "${bundle}" "${ASSETS_DIR}/${bundle_name}"
            bundle_count=$((bundle_count + 1))
        done
    fi

    # Copy Calf plugins
    if [ -d "${BUILD_DIR}/install/calf-plugins" ]; then
        # shellcheck disable=SC2034
        find "${BUILD_DIR}/install/calf-plugins" -name "*.lv2" -type d | while read -r bundle; do
            bundle_name=$(basename "${bundle}")
            echo "  Bundling: ${bundle_name}"
            cp -r "${bundle}" "${ASSETS_DIR}/${bundle_name}"
            bundle_count=$((bundle_count + 1))
        done
    fi

    # Copy Dragonfly plugins
    if [ -d "${BUILD_DIR}/install/dragonfly-reverb" ]; then
        # shellcheck disable=SC2034
        find "${BUILD_DIR}/install/dragonfly-reverb" -name "*.lv2" -type d | while read -r bundle; do
            bundle_name=$(basename "${bundle}")
            echo "  Bundling: ${bundle_name}"
            cp -r "${bundle}" "${ASSETS_DIR}/${bundle_name}"
            bundle_count=$((bundle_count + 1))
        done
    fi

    echo ""
    echo "=== Bundle complete ==="
    echo "Total LV2 bundles in assets:"
    ls -1 "${ASSETS_DIR}" 2>/dev/null || echo "  (none found)"
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

    # Initialize submodules if needed
    cd "${PROJECT_ROOT}"
    git submodule update --init lib/lsp-plugins lib/calf-plugins lib/dragonfly-reverb 2>/dev/null || true

    # Build
    build_lsp_plugins
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
# LV2 build trigger
