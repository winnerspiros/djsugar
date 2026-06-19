# copy_lv2_bundles.cmake — Copy LV2 plugin bundles if they exist
# This is called from CMakeLists.txt during Android builds.
# If the LV2 bundles haven't been built yet (e.g. local dev build),
# this silently does nothing.
if(IS_DIRECTORY "${SRC_DIR}")
  file(GLOB lv2_bundles RELATIVE "${SRC_DIR}" "${SRC_DIR}/*.lv2")
  if(lv2_bundles)
    message(STATUS "Copying LV2 bundles: ${lv2_bundles}")
    file(COPY "${SRC_DIR}/" DESTINATION "${DST_DIR}")
  else()
    message(STATUS "No LV2 bundles found in ${SRC_DIR}, skipping")
  endif()
else()
  message(STATUS "LV2 bundles source dir ${SRC_DIR} does not exist, skipping")
endif()
