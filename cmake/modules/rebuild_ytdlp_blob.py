"""Pack a shebang+zlib zipimport blob from yt-dlp + yt-dlp-ejs wheels.

Usage: python3 rebuild_ytdlp_blob.py <out_blob> <ytdlp.whl> <ejs.whl>

youtubedl-android runs the blob as `python <blob> <args>`. Imported straight
from the zip, Python inflates and compiles every yt-dlp module on every run
(it cannot cache bytecode inside a zip): measured 2.5 s vs 1.0 s per run on
an ARM server, more on a phone. The __main__.py below therefore unpacks the
packages next to the blob once, so later runs import files with cached .pyc.

Auto-update: run with `--djsugar-update` (YouTubeService does this in the
background at most once a day) it fetches the newest yt-dlp and yt-dlp-ejs
wheels from PyPI into `<blob dir>/updates/`. Normal runs prefer that copy and
fall back to the bundled one if it fails to import.
"""

import hashlib
import io
import sys
import zipfile

MAIN_TEMPLATE = r"""#!/usr/bin/env python3
import os
import shutil
import sys
import zipfile

BUNDLE_ID = "@BUNDLE_ID@"
PACKAGES = ("yt_dlp", "yt_dlp_ejs")


def _blob():
    path = os.path.realpath(os.path.abspath(__file__))
    return os.path.dirname(path) if os.path.basename(path) == "__main__.py" else path


def _unpacked(blob):
    base = os.path.dirname(blob)
    target = os.path.join(base, "unpacked-" + BUNDLE_ID)
    if os.path.isdir(os.path.join(target, "yt_dlp")):
        return target
    tmp = "%s.tmp-%d" % (target, os.getpid())
    try:
        with zipfile.ZipFile(blob) as z:
            z.extractall(tmp, [n for n in z.namelist() if n.split("/")[0] in PACKAGES])
        os.rename(tmp, target)
    except OSError:
        shutil.rmtree(tmp, ignore_errors=True)
        if not os.path.isdir(os.path.join(target, "yt_dlp")):
            return None  # read-only or full: import from the zip itself
    for name in os.listdir(base):  # bundles of older app versions
        if name.startswith("unpacked-") and name != os.path.basename(target):
            shutil.rmtree(os.path.join(base, name), ignore_errors=True)
    return target


def _updates_dir(blob):
    return os.path.join(os.path.dirname(blob), "updates")


def _updated(blob):
    updates = _updates_dir(blob)
    try:
        with open(os.path.join(updates, "current")) as f:
            name = f.read().strip()
    except OSError:
        return None
    path = os.path.join(updates, name)
    return path if name and os.path.isdir(os.path.join(path, "yt_dlp")) else None


def _forget_modules():
    for mod in [m for m in sys.modules if m.split(".")[0] in PACKAGES]:
        del sys.modules[mod]


def _update(blob):
    import io as _io
    import json
    import ssl
    import urllib.request

    try:
        import certifi
        ctx = ssl.create_default_context(cafile=certifi.where())
    except Exception:
        ctx = ssl.create_default_context()

    def get(url):
        req = urllib.request.Request(url, headers={"User-Agent": "djsugar"})
        with urllib.request.urlopen(req, timeout=30, context=ctx) as r:
            return r.read()

    wheels = []
    versions = []
    for project in ("yt-dlp", "yt-dlp-ejs"):
        info = json.loads(get("https://pypi.org/pypi/%s/json" % project))
        url = next(u for u in info["urls"] if u["filename"].endswith(".whl"))
        wheels.append(url)
        versions.append(info["info"]["version"])
    name = "-".join(versions)
    updates = _updates_dir(blob)
    target = os.path.join(updates, name)
    if _updated(blob) == target:
        print("djsugar-update: up to date", name)
        return 0
    import hashlib as _hashlib

    tmp = target + ".tmp-%d" % os.getpid()
    shutil.rmtree(tmp, ignore_errors=True)
    for url in wheels:
        data = get(url["url"])
        if _hashlib.sha256(data).hexdigest() != url["digests"]["sha256"]:
            raise RuntimeError("hash mismatch for " + url["filename"])
        with zipfile.ZipFile(_io.BytesIO(data)) as z:
            z.extractall(tmp, [n for n in z.namelist() if n.split("/")[0] in PACKAGES])
    # Smoke test before switching to it.
    sys.path.insert(0, tmp)
    _forget_modules()
    import yt_dlp
    import yt_dlp.extractor.youtube  # noqa: F401

    sys.path.remove(tmp)
    shutil.rmtree(target, ignore_errors=True)
    os.rename(tmp, target)
    with open(os.path.join(updates, "current.tmp"), "w") as f:
        f.write(name)
    os.replace(os.path.join(updates, "current.tmp"), os.path.join(updates, "current"))
    for old in os.listdir(updates):
        if old not in (name, "current"):
            p = os.path.join(updates, old)
            shutil.rmtree(p, ignore_errors=True) if os.path.isdir(p) else os.remove(p)
    print("djsugar-update: installed", name)
    return 0


def _main():
    blob = _blob()
    bundled = _unpacked(blob) if zipfile.is_zipfile(blob) else None
    if bundled:
        sys.path.insert(0, bundled)
    if "--djsugar-update" in sys.argv:
        sys.exit(_update(blob))
    update = _updated(blob)
    if update:
        sys.path.insert(0, update)
        try:
            import yt_dlp  # noqa: F401
        except Exception:
            sys.path.remove(update)
            _forget_modules()
    import yt_dlp

    yt_dlp.main()


if __name__ == "__main__":
    _main()
"""

out_blob, ytdlp_whl, ejs_whl = sys.argv[1], sys.argv[2], sys.argv[3]

files = []
for whl in (ytdlp_whl, ejs_whl):
    with zipfile.ZipFile(whl) as zin:
        for name in zin.namelist():
            top = name.split("/")[0]
            if top in ("yt_dlp", "yt_dlp_ejs") and not name.endswith("/"):
                files.append((name, zin.read(name)))

digest = hashlib.sha256()
for name, data in files:
    digest.update(name.encode())
    digest.update(data)
main = MAIN_TEMPLATE.replace("@BUNDLE_ID@", digest.hexdigest()[:16]).encode()

buf = io.BytesIO()
buf.write(b"#!/usr/bin/env python3\n")
with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as out:
    out.writestr("__main__.py", main)
    for name, data in files:
        out.writestr(name, data)

with open(out_blob, "wb") as f:
    f.write(buf.getvalue())
print(len(buf.getvalue()))
