# Copies every res/qml/**/+android/<file> over the upstream <file> next to
# its +android folder, then removes the +android folders.
#
# Run on the QML assets of the Android package, so the app gets the same
# file layout as if the upstream files had been edited, independently of
# how a Qt version resolves relative imports of QML file selectors.
#
# Usage: cmake -DQML_DIR=<assets>/qml -P apply_android_qml_overrides.cmake

if(NOT QML_DIR OR NOT IS_DIRECTORY "${QML_DIR}")
  message(FATAL_ERROR "QML_DIR '${QML_DIR}' is not a directory")
endif()

file(GLOB_RECURSE QML_FILES LIST_DIRECTORIES false "${QML_DIR}/*")
set(OVERRIDE_COUNT 0)
foreach(OVERRIDE ${QML_FILES})
  if(OVERRIDE MATCHES "/\\+android/")
    string(REGEX REPLACE "/\\+android/" "/" TARGET "${OVERRIDE}")
    file(COPY_FILE "${OVERRIDE}" "${TARGET}")
    math(EXPR OVERRIDE_COUNT "${OVERRIDE_COUNT} + 1")
  endif()
endforeach()

file(GLOB_RECURSE QML_DIRS LIST_DIRECTORIES true "${QML_DIR}/*")
foreach(DIR ${QML_DIRS})
  if(IS_DIRECTORY "${DIR}" AND DIR MATCHES "/\\+android$")
    file(REMOVE_RECURSE "${DIR}")
  endif()
endforeach()

message(STATUS "Applied ${OVERRIDE_COUNT} Android QML overrides in ${QML_DIR}")
