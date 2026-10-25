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
    -DQML_DIR="${CMAKE_CURRENT_BINARY_DIR}/android-build/assets/qml" -P
    ${CMAKE_SOURCE_DIR}/cmake/modules/apply_android_qml_overrides.cmake
)
add_dependencies(mixxx-android-qml-overrides copy-resource-android)
add_dependencies(mixxx mixxx-android-qml-overrides)
