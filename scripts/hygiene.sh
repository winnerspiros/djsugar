#!/bin/bash
set -uo pipefail
cd "$(dirname "$0")/.."
fail=0
bad() { echo "::error::$1"; fail=1; }

html=$(git ls-files '*.html' '*.htm')
[ -z "$html" ] || bad "HTML files are not allowed: $html"

names=$(git grep -n -i -I -E 'mixxx|serato|rekordbox|traktor|virtual ?dj|\bdjay\b|engine dj|opendj|openfader' -- crates platform ':!*.md')
[ -z "$names" ] || bad "Other apps named in code:
$names"

symbols=$(git grep -n -I -P '[\x{1F000}-\x{1FAFF}\x{2600}-\x{27BF}\x{2190}-\x{21FF}\x{2700}-\x{27BF}\x{2B00}-\x{2BFF}]' -- '*.md' '*.rs' '*.toml' '*.yml')
[ -z "$symbols" ] || bad "Emoji or symbols:
$symbols"

walls=$(git ls-files '*.rs' | xargs awk 'FNR==1{n=0} /^[[:space:]]*\/\//{if(++n==4)print FILENAME":"FNR; next} {n=0}')
[ -z "$walls" ] || bad "Comment walls (4+ comment lines):
$walls"

markers=$(git grep -n -E '\b(TODO|FIXME|XXX|HACK|dbg!|todo!|unimplemented!)' -- '*.rs')
[ -z "$markers" ] || bad "Leftover markers:
$markers"

big=$(git ls-files -z | xargs -0 du -k 2>/dev/null | awk '$1 > 2048 {print $2" "$1"K"}')
[ -z "$big" ] || bad "Files over 2 MB: $big"

exit $fail
