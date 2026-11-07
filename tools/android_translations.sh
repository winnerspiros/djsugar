#!/bin/bash
# Updates the translation catalogs of the Android/mobile UI.
#
# The strings of the QML UI and of the Android-only C++ sources live in
# their own catalogs (res/translations/mixxx-android_<lang>.ts), separate
# from the upstream mixxx_<lang>.ts files. This keeps merges from
# mixxxdj/mixxx free of translation conflicts.
#
# Usage:
#   tools/android_translations.sh            # update all catalogs
#   tools/android_translations.sh fr de      # also create new catalogs
#
# Requires lupdate and lrelease from Qt 6 in PATH, or QT_BIN pointing to
# the directory containing them.
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$DIR"

LUPDATE="${QT_BIN:+$QT_BIN/}lupdate"
LRELEASE="${QT_BIN:+$QT_BIN/}lrelease"

SOURCES=(
    res/qml
    src/qml/qmlstreamingproxy.cpp
    src/qml/qmlmobilelibraryproxy.cpp
    src/library/rekordbox/rekordboxdrivebackup.cpp
    src/library/rekordbox/rekordboxplaylistcopy.cpp
    src/util/androidbackup.cpp
    src/util/removablestorage.cpp
)

TS_FILES=(res/translations/mixxx-android.ts)
for ts in res/translations/mixxx-android_*.ts; do
    [ -e "$ts" ] && TS_FILES+=("$ts")
done
for lang in "$@"; do
    ts="res/translations/mixxx-android_${lang}.ts"
    [[ " ${TS_FILES[*]} " == *" $ts "* ]] || TS_FILES+=("$ts")
done

"$LUPDATE" -no-obsolete -locations none -extensions qml,js,cpp -I src \
    "${SOURCES[@]}" -ts "${TS_FILES[@]}"

for ts in "${TS_FILES[@]}"; do
    [ "$ts" = res/translations/mixxx-android.ts ] && continue
    "$LRELEASE" -nounfinished "$ts" -qm "${ts%.ts}.qm"
done
