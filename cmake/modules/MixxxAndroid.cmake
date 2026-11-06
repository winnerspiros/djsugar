# Sources, definitions and build steps of the Android port.
#
# Kept in this module, included once at the end of CMakeLists.txt, so that
# merging upstream Mixxx does not conflict on the long source lists of
# CMakeLists.txt. Only what has to happen at a specific point of the main
# file (toolchain setup, Qt components, the Android packaging properties and
# permissions) is still done there.

target_sources(
  mixxx-lib
  PRIVATE
    src/soundio/soundmanagerandroid.cpp
    src/library/androidscanorder.cpp
    src/library/rekordbox/rekordboxdrivebackup.cpp
    src/library/rekordbox/rekordboxplaylistcopy.cpp
    src/util/androidbackup.cpp
    src/util/androidcrashreport.cpp
    src/util/androidpermissions.cpp
    src/track/androidautocue.cpp
    src/analyzer/androidkickgrid.cpp
)

if(QML)
  target_sources(
    mixxx-qml-lib
    PRIVATE
      src/qml/androidpresetnames.cpp
      src/qml/qmlappupdateproxy.cpp
      src/qml/librarycoverloader.cpp
      src/qml/qmllanguageproxy.cpp
      src/qml/qmllibraryfeaturesource.cpp
      src/qml/qmlmobileappproxy.cpp
      src/qml/qmlmobileaudioproxy.cpp
      src/qml/qmlmobilecontrollersproxy.cpp
      src/qml/qmlmobilelibraryproxy.cpp
      src/qml/qmlmobilesingletons.cpp
      src/qml/qmlstreamingproxy.cpp
  )
endif()

# Android 10 to 13 (API 29-33). Qt and the dependencies are built against
# the C library of API 35, and libQt6Core, libQt6Test and libmixxx import
# functions that it only has from Android 11/13/14 on (pthread_cond_clockwait,
# backtrace, copy_file_range), so they can't be loaded on older versions. libmixxx_libccompat.so provides them
# (src/util/androidlibccompat.c), and both libraries are patched to load it
# and to import these functions without the version of the C library.
# Check with tools: llvm-readelf --dyn-syms on the libraries of the APK.
find_program(PATCHELF_EXECUTABLE patchelf REQUIRED)
add_library(mixxx_libccompat SHARED src/util/androidlibccompat.c)
target_link_libraries(mixxx_libccompat PRIVATE dl)
add_dependencies(mixxx mixxx_libccompat)
set_property(
  TARGET mixxx
  APPEND
  PROPERTY QT_ANDROID_EXTRA_LIBS "$<TARGET_FILE:mixxx_libccompat>"
)
# The Qt library of the build environment, patched in place once
add_custom_target(
  android-qtcore-libccompat
  COMMAND
    ${CMAKE_COMMAND} -DPATCHELF=${PATCHELF_EXECUTABLE} -DSTRIP=${CMAKE_STRIP}
    -DLIBRARY=$<TARGET_FILE:Qt6::Core> -DSYMBOLS=copy_file_range,pthread_cond_clockwait
    -P
    ${CMAKE_SOURCE_DIR}/cmake/scripts/android_libccompat.cmake
  VERBATIM
)
add_dependencies(mixxx android-qtcore-libccompat)
# libmixxx links QtTest, which uses pthread_cond_clockwait too
add_custom_target(
  android-qttest-libccompat
  COMMAND
    ${CMAKE_COMMAND} -DPATCHELF=${PATCHELF_EXECUTABLE} -DSTRIP=${CMAKE_STRIP}
    -DLIBRARY=$<TARGET_FILE:Qt6::Test> -DSYMBOLS=pthread_cond_clockwait -P
    ${CMAKE_SOURCE_DIR}/cmake/scripts/android_libccompat.cmake
  VERBATIM
)
add_dependencies(mixxx android-qttest-libccompat)
add_custom_command(
  TARGET mixxx
  POST_BUILD
  COMMAND
    ${CMAKE_COMMAND} -DPATCHELF=${PATCHELF_EXECUTABLE} -DSTRIP=${CMAKE_STRIP}
    -DLIBRARY=$<TARGET_FILE:mixxx>
    -DSYMBOLS=copy_file_range,backtrace,pthread_cond_clockwait -P
    ${CMAKE_SOURCE_DIR}/cmake/scripts/android_libccompat.cmake
  VERBATIM
)

if(HID)
  # Android MIDI (android.media.midi + NDK AMidi, resolved at runtime). It
  # relies on the JNI glue in android.cpp.
  target_compile_definitions(mixxx-lib PUBLIC __ANDROID_MIDI__)
  target_sources(
    mixxx-lib
    PRIVATE
      src/controllers/midi/androidmidicontroller.cpp
      src/controllers/midi/androidmidienumerator.cpp
  )
endif()

if(BULK)
  # Direct USB audio/MIDI driver for sound cards that Android can't play to
  target_compile_definitions(mixxx-lib PUBLIC __ANDROID_USB_AUDIO__)
  target_sources(
    mixxx-lib
    PRIVATE
      src/soundio/androidusbaudiodevice.cpp
      src/soundio/androidusbsounddevices.cpp
      src/soundio/usbaudiodescriptors.cpp
      src/soundio/sounddeviceandroidusb.cpp
      # Read-only reader for USB drives that Android doesn't mount
      src/library/usbdrive/androidusbdrive.cpp
      src/library/usbdrive/fatvolume.cpp
      src/library/usbdrive/usbmassstorage.cpp
  )
  target_compile_definitions(mixxx-lib PUBLIC __ANDROID_USB_DRIVE__)
  if(QML)
    # The QML proxies copy the tracks of these drives before loading them
    target_compile_definitions(mixxx-qml-lib PUBLIC __ANDROID_USB_DRIVE__)
  endif()
endif()

# The Oboe host API of PortAudio copies the device names with
# malloc(strlen(name)) and strcpy(), writing one byte past the allocation.
# That corrupts the heap (Scudo aborts later, at random places) when a name
# length is a multiple of 16, e.g. "TB331FC: Speaker". Its allocations are
# redirected to mixxx_portaudio_malloc(), which adds the missing byte.
set(MIXXX_PATCHED_PORTAUDIO "${CMAKE_CURRENT_BINARY_DIR}/libportaudio-mixxx.a")
add_custom_command(
  OUTPUT "${MIXXX_PATCHED_PORTAUDIO}"
  COMMAND
    "${CMAKE_OBJCOPY}" --redefine-sym malloc=mixxx_portaudio_malloc
    "${PortAudio_LIBRARY}" "${MIXXX_PATCHED_PORTAUDIO}"
  DEPENDS "${PortAudio_LIBRARY}"
  COMMENT "Fixing the device name allocation of PortAudio"
)
add_custom_target(mixxx-portaudio-patched DEPENDS "${MIXXX_PATCHED_PORTAUDIO}")
set_target_properties(
  PortAudio::PortAudio
  PROPERTIES IMPORTED_LOCATION "${MIXXX_PATCHED_PORTAUDIO}"
)
add_dependencies(mixxx-lib mixxx-portaudio-patched)

# The USB devices that start Mixxx when plugged in, generated from the
# controller mappings
add_custom_target(
  mixxx-android-usb-device-filter
  COMMAND
    ${CMAKE_COMMAND}
    -DTEMPLATE_FILE="${CMAKE_SOURCE_DIR}/packaging/android/usb_device_filter.xml.in"
    -DTARGET_FILE="${CMAKE_CURRENT_BINARY_DIR}/android/res/xml/usb_device_filter.xml"
    -P ${CMAKE_SOURCE_DIR}/cmake/modules/gen_usb_device_filter.cmake
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
)
add_dependencies(mixxx-android-usb-device-filter copy-resource-android)
add_dependencies(mixxx mixxx-android-usb-device-filter)

# The Android versions of upstream QML files live in +android folders next
# to them (see packaging/android/README.md). They replace the upstream files
# in the package, after copy-resource-android copied res/ to the assets.
add_custom_target(
  mixxx-android-qml-overrides
  COMMAND
    ${CMAKE_COMMAND}
    -DQML_DIR="${CMAKE_CURRENT_BINARY_DIR}/android-build/assets/qml"
    -P ${CMAKE_SOURCE_DIR}/cmake/modules/apply_android_qml_overrides.cmake
)
add_dependencies(mixxx-android-qml-overrides copy-resource-android)
add_dependencies(mixxx mixxx-android-qml-overrides)

# Stem separation: splits tracks into .stem.mp4 files with ONNX models that
# the app downloads (see src/stems/). ONNX Runtime is only loaded when a
# track is separated, so it is packaged but not linked.
if(STEM)
  set(MIXXX_ONNXRUNTIME_VERSION 1.31.0)
  set(
    MIXXX_ONNXRUNTIME_AAR
    "${CMAKE_CURRENT_BINARY_DIR}/onnxruntime-android-${MIXXX_ONNXRUNTIME_VERSION}.aar"
  )
  set(MIXXX_ONNXRUNTIME_DIR "${CMAKE_CURRENT_BINARY_DIR}/onnxruntime")
  if(NOT EXISTS "${MIXXX_ONNXRUNTIME_DIR}/headers/onnxruntime_cxx_api.h")
    file(
      DOWNLOAD
        "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/${MIXXX_ONNXRUNTIME_VERSION}/onnxruntime-android-${MIXXX_ONNXRUNTIME_VERSION}.aar"
        "${MIXXX_ONNXRUNTIME_AAR}"
      EXPECTED_HASH
        SHA256=72683787a80d0608a3121b967d8e1239aef48286826661c953211626690540a7
      SHOW_PROGRESS
    )
    file(MAKE_DIRECTORY "${MIXXX_ONNXRUNTIME_DIR}")
    file(
      ARCHIVE_EXTRACT
      INPUT "${MIXXX_ONNXRUNTIME_AAR}"
      DESTINATION "${MIXXX_ONNXRUNTIME_DIR}"
      PATTERNS "headers/*" "jni/${ANDROID_ABI}/libonnxruntime.so"
    )
    file(REMOVE "${MIXXX_ONNXRUNTIME_AAR}")
  endif()

  target_compile_definitions(mixxx-lib PUBLIC __STEM_SEPARATION__)
  target_include_directories(
    mixxx-lib
    SYSTEM
    PRIVATE "${MIXXX_ONNXRUNTIME_DIR}/headers"
  )
  target_sources(
    mixxx-lib
    PRIVATE
      src/stems/htdemucsseparator.cpp
      src/stems/onnxruntimeloader.cpp
      src/stems/spleeterseparator.cpp
      src/stems/stemcompanion.cpp
      src/stems/stemfilewriter.cpp
      src/stems/stemmodels.cpp
      src/stems/stemseparator.cpp
      src/stems/stemsmanager.cpp
      src/stems/stemsworker.cpp
  )
  # The Wiener filter of Spleeter: plain complex arithmetic, without the
  # slow handling of infinities and NaN (std::abs, products, divisions)
  set_source_files_properties(
    src/stems/spleeterseparator.cpp
    PROPERTIES COMPILE_OPTIONS "-fcx-limited-range"
  )
  # The STFT of Spleeter
  target_link_libraries(mixxx-lib PRIVATE "${FFTW3_INCLUDE_DIR}/../lib/libfftw3f.a")
  set_property(
    TARGET mixxx
    APPEND
    PROPERTY
      QT_ANDROID_EXTRA_LIBS
        "${MIXXX_ONNXRUNTIME_DIR}/jni/${ANDROID_ABI}/libonnxruntime.so"
  )
  if(QML)
    target_compile_definitions(mixxx-qml-lib PUBLIC __STEM_SEPARATION__)
    target_include_directories(
      mixxx-qml-lib
      SYSTEM
      PRIVATE "${MIXXX_ONNXRUNTIME_DIR}/headers"
    )
    target_sources(mixxx-qml-lib PRIVATE src/qml/qmlstemsproxy.cpp)
  endif()
endif()
