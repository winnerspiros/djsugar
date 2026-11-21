#!/bin/sh
# Packages a compiled binary for Linux as
#   dist/djsugar-VERSION-linux-x86_64.tar.gz   (unpack anywhere)
#   dist/DJSugar-VERSION-x86_64.AppImage       (one file, updates itself)
#   dist/djsugar_VERSION_amd64.deb             (Debian, Ubuntu, Mint, Pop!_OS)
#   dist/djsugar-VERSION-1.x86_64.rpm          (Fedora, openSUSE, RHEL)
# Usage: scripts/bundle-linux.sh BINARY VERSION
# Needs ImageMagick (convert), dpkg-deb, rpmbuild (the "rpm" package on
# Ubuntu) and network access to fetch appimagetool.
set -eu
binary=$1
version=$2
root=$(pwd)
mkdir -p dist

# A usr/ tree, the same layout as an install under /usr or /usr/local.
appdir=dist/AppDir
rm -rf "$appdir"
id=io.github.winnerspiros.DJSugar
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/djsugar" \
  "$appdir/usr/share/applications" "$appdir/usr/share/metainfo"
cp "$binary" "$appdir/usr/bin/djsugar"
strip "$appdir/usr/bin/djsugar"
# The helper that runs audio plugins, built next to the app's binary.
helper="$(dirname "$binary")/djsugar-plugin-host"
if [ -f "$helper" ]; then
  cp "$helper" "$appdir/usr/bin/djsugar-plugin-host"
  strip "$appdir/usr/bin/djsugar-plugin-host"
else
  echo "warning: no $helper, so this build can't run audio plugins" >&2
fi
cp -R res/controllers "$appdir/usr/share/djsugar/controllers"
cp -R res/mods "$appdir/usr/share/djsugar/mods"
# The guided tour's demo tracks, once they are in the repository.
if [ -d res/demo ]; then cp -R res/demo "$appdir/usr/share/djsugar/demo"; fi
# Lets HID controllers (Traktor Kontrol) open without root; see the guide.
cp res/linux/70-djsugar-hid-controllers.rules "$appdir/usr/share/djsugar/"
for size in 64 128 256 512; do
  icons=$appdir/usr/share/icons/hicolor/${size}x$size/apps
  mkdir -p "$icons"
  convert res/macos/djsugar-1024.png -resize "${size}x$size" "$icons/$id.png"
done
cp "res/linux/$id.desktop" "$appdir/usr/share/applications/"
sed -e "s/@VERSION@/$version/g" -e "s/@DATE@/$(date -u +%Y-%m-%d)/" \
  "res/linux/$id.metainfo.xml" > "$appdir/usr/share/metainfo/$id.metainfo.xml"

# Tarball: unpack anywhere and run djsugar/bin/djsugar.
tarname=djsugar-$version-linux-x86_64
rm -rf "dist/$tarname"
cp -R "$appdir/usr" "dist/$tarname"
cp README.md LICENSE NOTICE "dist/$tarname/"
tar -C dist -czf "dist/$tarname.tar.gz" "$tarname"

# AppImage: one file, chmod +x and run. System libraries (ALSA, fontconfig,
# Vulkan, X11/Wayland) come from the host, as for any desktop app.
cp "$appdir/usr/share/applications/$id.desktop" "$appdir/$id.desktop"
cp "$appdir/usr/share/icons/hicolor/256x256/apps/$id.png" "$appdir/$id.png"
cat > "$appdir/AppRun" <<'APPRUN'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
exec "$here/usr/bin/djsugar" "$@"
APPRUN
chmod +x "$appdir/AppRun"
tool=dist/appimagetool
if [ ! -x "$tool" ]; then
  curl -fsSL -o "$tool" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$tool"
fi
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$appdir" "$root/dist/DJSugar-$version-x86_64.AppImage"

# .deb and .rpm: installed under /usr by the system's software installer.
# They also put the HID controller rule where udev reads it, so Traktor
# Kontrol controllers work without the manual step.
pkgver=$(printf %s "$version" | tr - '~') # 0.5.0-beta.1 sorts before 0.5.0
pkgroot=dist/pkgroot
rm -rf "$pkgroot"
cp -R "$appdir/usr" "$pkgroot"
mkdir -p "$pkgroot/lib/udev/rules.d" "$pkgroot/share/doc/djsugar"
mv "$pkgroot/share/djsugar/70-djsugar-hid-controllers.rules" "$pkgroot/lib/udev/rules.d/"
cp README.md NOTICE "$pkgroot/share/doc/djsugar/"
cp LICENSE "$pkgroot/share/doc/djsugar/copyright"
summary="Free, open-source DJ software: decks, stems, effects and controllers"
homepage=https://github.com/winnerspiros/djsugar
# Re-reads udev rules so a plugged-in controller is picked up.
postinst='if command -v udevadm >/dev/null 2>&1; then udevadm control --reload-rules || true; udevadm trigger --subsystem-match=hidraw || true; fi'

debroot=dist/deb
rm -rf "$debroot"
mkdir -p "$debroot/DEBIAN"
cp -R "$pkgroot" "$debroot/usr"
case $(uname -m) in x86_64) debarch=amd64 ;; aarch64) debarch=arm64 ;; *) debarch=$(uname -m) ;; esac
cat > "$debroot/DEBIAN/control" <<CONTROL
Package: djsugar
Version: $pkgver
Architecture: $debarch
Maintainer: DJ Sugar <https://github.com/winnerspiros/djsugar/issues>
Installed-Size: $(du -sk "$debroot/usr" | cut -f1)
Depends: libc6 (>= 2.35), libstdc++6, libgcc-s1, libasound2t64 | libasound2, libfontconfig1, libudev1, libxkbcommon0
Recommends: libxkbcommon-x11-0, libwayland-client0, libx11-xcb1, libvulkan1, mesa-vulkan-drivers
Section: sound
Priority: optional
Homepage: $homepage
Description: $summary
 DJ Sugar plays your music folders on two or four decks with sync, key lock,
 beat-synced effects, stems, a sampler and Automix, and works with the
 DJ controllers Mixxx supports.
CONTROL
printf '#!/bin/sh\nset -e\n%s\n' "$postinst" > "$debroot/DEBIAN/postinst"
chmod 755 "$debroot/DEBIAN/postinst"
dpkg-deb --root-owner-group --build "$debroot" "dist/djsugar_${version}_$debarch.deb"

rpmtop=$root/dist/rpmbuild
# The plugin helper ships next to the app when it was built.
helper_files=
if [ -e "$pkgroot/bin/djsugar-plugin-host" ]; then
  helper_files=/usr/bin/djsugar-plugin-host
fi
rm -rf "$rpmtop"
mkdir -p "$rpmtop/SPECS"
cat > "$rpmtop/SPECS/djsugar.spec" <<SPEC
Name: djsugar
Version: $pkgver
Release: 1
Summary: $summary
License: GPL-2.0-or-later
URL: $homepage
Requires: libasound.so.2()(64bit), libfontconfig.so.1()(64bit), libudev.so.1()(64bit), libstdc++.so.6()(64bit), libxkbcommon.so.0()(64bit)
Recommends: libxkbcommon-x11.so.0()(64bit), libwayland-client.so.0()(64bit), libvulkan.so.1()(64bit)
AutoReqProv: no

%description
DJ Sugar plays your music folders on two or four decks with sync, key lock,
beat-synced effects, stems, a sampler and Automix, and works with the
DJ controllers Mixxx supports.

%install
mkdir -p %{buildroot}/usr
cp -R $root/$pkgroot/. %{buildroot}/usr/

%post
$postinst

%files
/usr/bin/djsugar
$helper_files
/usr/share/djsugar
/usr/share/applications/$id.desktop
/usr/share/metainfo/$id.metainfo.xml
/usr/share/icons/hicolor/*/apps/$id.png
/usr/lib/udev/rules.d/70-djsugar-hid-controllers.rules
%doc /usr/share/doc/djsugar
SPEC
rpmbuild --quiet -bb --define "_topdir $rpmtop" --define "__strip /bin/true" \
  --define "debug_package %{nil}" --define "_build_id_links none" "$rpmtop/SPECS/djsugar.spec"
rpmarch=$(uname -m)
cp "$rpmtop/RPMS/$rpmarch/djsugar-$pkgver-1.$rpmarch.rpm" "dist/djsugar-$version-1.$rpmarch.rpm"
