#!/usr/bin/env python3
"""Write Markdown release notes for the commits between two refs.

Usage: release_notes.py <previous-tag-or-empty> <head-ref> <repo> <assets-dir>

Commits are grouped by what a user notices (new features, fixes, performance,
platform work); commits that only touch tests, CI or docs go into a collapsed
"Development" section. Merged pull requests are linked. The download table
lists every asset found in <assets-dir> with its size.
"""

import os
import re
import subprocess
import sys

DEV_PATHS = re.compile(
    r"^(src/test/|\.github/|\.pre-commit|\.markdownlint|.*\.md$"
    r"|tools/release_notes\.py$)"
)
GROUPS = [
    (
        "New",
        re.compile(
            r"\b(add|adds|added|new|support|supports|introduce|feat"
            r"|enable|implement)\b",
            re.I,
        ),
    ),
    (
        "Fixes",
        re.compile(
            r"\b(fix|fixes|fixed|crash|bug|restore|repair|correct|prevent"
            r"|avoid|broken|regression)\b",
            re.I,
        ),
    ),
    (
        "Performance and size",
        re.compile(
            r"\b(perf|faster|speed|optimi[sz]\w*|smaller|size|lto|ipo|webp|opus|latency|cache|compression)\b",
            re.I,
        ),
    ),
]
PLATFORMS = [
    ("Android", re.compile(r"\bandroid\b", re.I)),
    ("iOS", re.compile(r"\bios\b|\bipa\b|iphone|ipad", re.I)),
    ("macOS", re.compile(r"\bmacos\b|\bmac\b|\bdmg\b", re.I)),
    ("Windows", re.compile(r"\bwindows\b|\bmsi\b|\bwin(32|64)?\b", re.I)),
    ("Linux", re.compile(r"\blinux\b|appimage|\bdeb\b|ubuntu|flatpak", re.I)),
]
ASSETS = [
    (".apk", "Android (arm64)"),
    (".ipa", "iOS / iPadOS (arm64, unsigned)"),
    ("arm64.msi", "Windows (ARM64)"),
    (".msi", "Windows (x64)"),
    ("arm64.dmg", "macOS (Apple Silicon)"),
    (".dmg", "macOS (Intel)"),
    (".deb", "Ubuntu / Debian (x86_64)"),
    (".AppImage", "Linux (x86_64)"),
]


def git(*args):
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout


def size(path):
    n = os.path.getsize(path)
    return f"{n / 1048576:.0f} MB" if n >= 1048576 else f"{n / 1024:.0f} KB"


def is_own_pr(repo, num, title):
    """True when PR <num> of <repo> has this title (else it came with an upstream merge)."""
    try:
        own = subprocess.run(
            [
                "gh",
                "pr",
                "view",
                num,
                "-R",
                repo,
                "--json",
                "title",
                "-q",
                ".title",
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return own.returncode == 0 and own.stdout.strip() == title


def main():
    prev, head, repo, assets_dir = sys.argv[1:5]
    rng = f"{prev}..{head}" if prev else head
    log = git(
        "log", "--no-merges", "--format=%x1e%H%x1f%s%x1f%b", "--name-only", rng
    )
    pr_titles = {}
    for m in re.finditer(
        r"Merge pull request #(\d+) from \S+\n\n(.+)",
        git("log", "--merges", "--format=%s%n%n%b", rng),
    ):
        pr_titles[m.group(1)] = m.group(2).strip()

    groups = {name: [] for name, _ in GROUPS}
    groups["Other changes"] = []
    dev = []
    platforms = {name: 0 for name, _ in PLATFORMS}
    for entry in log.split("\x1e")[1:]:
        sha, subject, rest = entry.split("\x1f", 2)
        files = [f for f in rest.split("\n") if f and not f.startswith(" ")][
            -200:
        ]
        files = [f for f in files if "/" in f or "." in f]
        line = f"- {subject.strip()} ([{sha[:10]}](https://github.com/{repo}/commit/{sha}))"
        text = subject + " " + " ".join(files)
        for name, rx in PLATFORMS:
            if rx.search(text):
                platforms[name] += 1
        if files and all(DEV_PATHS.match(f) for f in files):
            dev.append(line)
            continue
        for name, rx in GROUPS:
            if rx.search(subject):
                groups[name].append(line)
                break
        else:
            groups["Other changes"].append(line)

    total = sum(len(v) for v in groups.values()) + len(dev)
    out = []
    if prev:
        out.append(
            f"{total} changes since [{prev}](https://github.com/{repo}/releases/tag/{prev}) "
            f"([full diff](https://github.com/{repo}/compare/{prev}...{head}))."
        )
    touched = [name for name, count in platforms.items() if count]
    if touched:
        out.append(f"Platforms with specific changes: {', '.join(touched)}.")
    out.append("")

    if pr_titles:
        out.append("## Pull requests")
        for num, title in sorted(pr_titles.items(), key=lambda x: int(x[0])):
            own = is_own_pr(repo, num, title)
            ref = (
                f"#{num}"
                if own
                else f"[mixxxdj/mixxx#{num}](https://github.com/mixxxdj/mixxx/pull/{num})"
            )
            out.append(
                f"- {title} ({ref}){'' if own else ', from upstream Mixxx'}"
            )
        out.append("")
    for name, lines in groups.items():
        if lines:
            out += [f"## {name}", *lines, ""]
    if dev:
        out += [
            "<details>",
            f"<summary>Development: tests, CI and docs ({len(dev)})</summary>",
            "",
            *dev,
            "",
            "</details>",
            "",
        ]

    files = sorted(os.listdir(assets_dir)) if os.path.isdir(assets_dir) else []
    if files:
        out += [
            "## Downloads",
            "",
            "| Platform | File | Size |",
            "| --- | --- | --- |",
        ]
        used = set()
        for suffix, label in ASSETS:
            for f in files:
                if f.endswith(suffix) and f not in used:
                    used.add(f)
                    out.append(
                        f"| {label} | `{f}` | {size(os.path.join(assets_dir, f))} |"
                    )
        out.append("")
        if any(f.endswith(".ipa") for f in files):
            out += [
                "The iOS build is unsigned: install it with [AltStore](https://altstore.io) or "
                "[Sideloadly](https://sideloadly.io), which sign it with your Apple ID.",
                "",
            ]
    print("\n".join(out))


if __name__ == "__main__":
    main()
