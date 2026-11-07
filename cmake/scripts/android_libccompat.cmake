# Makes a library built against the API 35 C library load on Android 10 to
# 13: it loads libmixxx_libccompat.so (src/util/androidlibccompat.c) and
# imports SYMBOLS without the version of the C library, so that they are
# found there when the C library lacks them.
#
# cmake -DPATCHELF=<patchelf> -DSTRIP=<llvm-strip> -DLIBRARY=<.so> -DSYMBOLS=<a,b>
#       -P android_libccompat.cmake
#
# The library is stripped first, as Gradle strips the libraries of the APK:
# stripping a patched library moves the segment patchelf adds, which then
# isn't aligned and the library can't be loaded ("empty/missing
# DT_HASH/DT_GNU_HASH"). Stripped before, Gradle has nothing to remove and
# leaves the layout as it is.
#
# Does nothing if the library was patched already (e.g. the Qt library of the
# build environment, which is patched in place).

set(COMPAT_LIBRARY "libmixxx_libccompat.so")

execute_process(
  COMMAND "${PATCHELF}" --print-needed "${LIBRARY}"
  OUTPUT_VARIABLE NEEDED
  RESULT_VARIABLE RESULT
)
if(NOT RESULT EQUAL 0)
  message(FATAL_ERROR "patchelf could not read ${LIBRARY}")
endif()
if(NEEDED MATCHES "${COMPAT_LIBRARY}")
  return()
endif()

execute_process(
  COMMAND "${STRIP}" --strip-unneeded "${LIBRARY}"
  RESULT_VARIABLE RESULT
)
if(NOT RESULT EQUAL 0)
  message(FATAL_ERROR "${STRIP} could not strip ${LIBRARY}")
endif()

set(ARGUMENTS --page-size 16384 --add-needed "${COMPAT_LIBRARY}")
string(REPLACE "," ";" SYMBOL_LIST "${SYMBOLS}")
foreach(SYMBOL IN LISTS SYMBOL_LIST)
  list(APPEND ARGUMENTS --clear-symbol-version "${SYMBOL}")
endforeach()
execute_process(
  COMMAND "${PATCHELF}" ${ARGUMENTS} "${LIBRARY}"
  RESULT_VARIABLE RESULT
)
if(NOT RESULT EQUAL 0)
  message(FATAL_ERROR "patchelf could not patch ${LIBRARY}")
endif()
message(STATUS "Patched ${LIBRARY} for Android 10 to 13")
