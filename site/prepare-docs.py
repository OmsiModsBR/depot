#!/usr/bin/env python3
"""Prepare the Markdown served by the Pages site (run by .github/workflows/pages.yml).

The files in docs/ keep the full inherited history. The site is the canonical,
current description of Project Depot, so what is published leaves out everything
about builds and features that Depot does not compile or support today: LAN and
multiplayer play, the dedicated server, Android, macOS, Linux and ARM64 builds.

usage: prepare-docs.py SITE_DOCS_DIR
Only the pages in PUBLISHED are kept; sections, sentences, list items, table rows
and code blocks that mention an unsupported platform or network feature are dropped,
and links to pages that are not published become plain text.
"""
import posixpath
import re
import shutil
import sys
from pathlib import Path

PUBLISHED = ["USER_GUIDE", "MODDING", "PBR", "BUILDING", "FORMATS", "ARCHITECTURE", "ROUTES", "VERSIONING"]

OUT = re.compile(
    r"\bLAN\b|other players|lan-(?:host|join|name|store|world)|OMSI_(?:NO_)?LAN|OMSI_(?:NO_BRIDGE|NO_VAR_SYNC|DEBUG_VAR_SYNC)"
    r"|multi-?player|dedicated server|\bSERVER\.md|server package|session code|UPnP|Hamachi|ZeroTier|Tailscale"
    r"|GreenTeaSpeak|NativeActivity|\\bJNI\\b|\bandroid\b|\bAPK\b|\bmacOS\b|\bmac\b|\bApple\b|\bLinux\b|\bARM64\b|\barm64\b|aarch64"
    r"|build-(?:macos|linux|android|server)|\.icns|\bWine\b|\bwayland\b|\bxcode\b|\bopenomsi-server\b",
    re.I,
)

# Sections dropped whole: only headings that are entirely about an unsupported topic.
SECTION_OUT = re.compile(r"^(?:LAN play|Android|Dedicated server)\b", re.I)

FENCE = re.compile(r"^\s*```")
HEADING = re.compile(r"^(#{1,6})\s+(.*)$")
LIST = re.compile(r"^\s*(?:[*+-]|\d+[.)])\s+")
SPLIT = re.compile(r"(?<=[.!?])(\s+)(?=[A-Z*`(\[])")


def split_sections(lines):
    """Drop headings whose text matches OUT, with their whole section."""
    out, skip_level, in_fence = [], None, False
    for ln in lines:
        if FENCE.match(ln):
            in_fence = not in_fence
        m = None if in_fence else HEADING.match(ln)
        if m:
            level = len(m.group(1))
            if skip_level is not None and level <= skip_level:
                skip_level = None
            if skip_level is None and SECTION_OUT.search(m.group(2)):
                skip_level = level
        if skip_level is None:
            out.append(ln)
    return out


def units(lines):
    """Yield (kind, [lines]) for fenced code, table rows, headings, list items, paragraphs, blanks."""
    def plain_cont(s):
        return s.strip() and not LIST.match(s) and not s.lstrip().startswith("|") \
            and not HEADING.match(s) and not FENCE.match(s)

    i, n = 0, len(lines)
    while i < n:
        ln = lines[i]
        if FENCE.match(ln):
            j = i + 1
            while j < n and not FENCE.match(lines[j]):
                j += 1
            yield "code", lines[i : j + 1]
            i = j + 1
        elif not ln.strip():
            yield "blank", [ln]
            i += 1
        elif HEADING.match(ln):
            yield "heading", [ln]
            i += 1
        elif ln.lstrip().startswith("|"):
            yield "table", [ln]
            i += 1
        elif LIST.match(ln):
            j = i + 1
            while j < n and plain_cont(lines[j]) and lines[j].startswith((" ", "\t")):
                j += 1
            yield "list", lines[i:j]
            i = j
        else:
            j = i + 1
            while j < n and plain_cont(lines[j]):
                j += 1
            yield "para", lines[i:j]
            i = j


def trim_sentences(block):
    """Remove the sentences of a paragraph or list item that match OUT."""
    text = "\n".join(block)
    if not OUT.search(text):
        return block
    marker = LIST.match(block[0])
    prefix = marker.group(0) if marker else ""
    parts = SPLIT.split(text[len(prefix):])
    res = ""
    dropped = False
    for k in range(0, len(parts), 2):
        sent = parts[k]
        sep = parts[k + 1] if k + 1 < len(parts) else ""
        if OUT.search(sent):
            # Keep the clauses of a long "a; b; c" sentence that are about supported things.
            clauses = re.split(r"(?<=;)(\s+)", sent)
            kept = [(clauses[i], clauses[i + 1] if i + 1 < len(clauses) else "")
                    for i in range(0, len(clauses), 2) if not OUT.search(clauses[i])]
            if kept and len(clauses) > 1:
                sent = "".join(c + w for c, w in kept).rstrip()
                if not sent.endswith((".", "!", "?", ")", "*", "`")):
                    sent = sent.rstrip(";,") + "."
                res += sent + sep
            dropped = True
            continue
        if dropped and re.fullmatch(r"\*\*\([^)]*\)\*\*\.?", sent.strip()):
            continue  # an orphaned tag such as "**(done)**" after a dropped sentence
        dropped = False
        res += sent + sep
    res = res.strip()
    return (prefix + res).split("\n") if res else []


# Fixes for sentences that mix supported and unsupported items (regex, applied first).
LITERAL = [
    (r"They are obstacles for the AI like the player's bus\. A session is found by.*?opens the chat line\.", ""),
    (r", name tags over the other players' buses; the HUD's\s+LAN block is one line; the others see the driver profile's name\. Settings `chat`,\s+`tooltips`,",
     ". Settings `tooltips`,"),
    (r"a Roblox-like chat \(V shows/hides,\s+`/` or a click types, rustrict filter, 200 lines, wheel scrolls\), the name", "the name"),
    (r"\* \*\*Online\*\* - `ui\.rs`", "* **Interface** - `ui.rs`"),
    (r"shadows, online interface,", "shadows, interface,"),
    (r", other players' buses\.", "."),
    (r"`log`, `join`, `settings`", "`log`, `settings`"),
    (r"controls, controllers, multiplayer that lasts, phones, more builds", "controls, controllers"),
    (r"`OMSI_DEBUG_LAN`, ", ""),
    (r", a peer,", ","),
    (r", install mods and play over LAN\.", " and install mods."),
    (r"\*\*Voice chat through GreenTeaSpeak\*\*.*?greenteaspeak-plugin/README\.md`\)\.", ""),
    (r", on the computer and on Android alike", ""),
    (r"scripting, gameplay or multiplayer functionality", "scripting or gameplay functionality"),
    (r"The only published packages for this release are Windows x64 game/launcher and\s+dedicated server, named `depot-0\.2\.17-windows-x64\.zip` and\s+`depot-0\.2\.17-server-windows-x64\.zip`\.",
     "The only published package for this release is the Windows x64 game/launcher, `depot-0.2.17-windows-x64.zip`. The current release, 0.2.52, publishes the same kind of package."),
    (r"2\. builds Windows x64 and ARM64.*?builds;", "2. builds Windows x64 (MSVC);"),
    (r"3\. creates the release `v<version>` \(tag on that commit\) with the archives.*?from the commits\.",
     "3. creates the release `v<version>` (tag on that commit) with the Windows x64 archive and release notes generated from the commits."),
    (r"\* \*\*(?:macOS|Linux|Android)\*\*.*?(?=\n\*|\n\n)", ""),
    (r";\s+with `--server server\.cfg` it is the dedicated server\.", "."),
    (r"`depot\.icns` \(standard margin, rounded\) goes into the macOS\s+bundle, `depot-256\.png` is the window icon on Windows and Linux, and the Android launcher icons are\s+in `android/res/mipmap-\*`\.",
     "and `depot-256.png` is the window icon."),
    (r", \*\*V\*\* the chat line in a\s+LAN session\.", "."),
    (r", \*LAN play: host / join\*,", ","),
    (r" and, for a LAN\s+session, the code to copy, who is playing and the chat\.", "."),
    (r"\(Apple M3/M4 and newer, ", "("),
    (r"the other\s+players of a LAN session or server as purple arrows with their names \(on the city map too\), ", ""),
    (r"; beside `openOMSI\.app` on macOS", ""),
    (r"scripted objects, LAN\)", "scripted objects)"),
]


def clean(text, published):
    for pat, new in LITERAL:
        text = re.sub(pat, new, text, flags=re.S)
    out = []
    for kind, block in units(split_sections(text.split("\n"))):
        if kind in ("heading", "blank"):
            out.extend(block)
        elif kind == "code":
            if not any(OUT.search(b) for b in block):
                out.extend(block)
        elif kind == "table":
            if not OUT.search(block[0]):
                out.extend(block)
        else:
            out.extend(trim_sentences(block))
    text = "\n".join(out)

    def link(m):
        label, target = m.group(1), m.group(2)
        path = target.split("#")[0]
        if re.match(r"^[a-z]+:", target) or target.startswith("#") or not path:
            return m.group(0)
        if path.endswith(".md") and "/" not in path and path[:-3] in published:
            return m.group(0)
        if OUT.search(path):
            return label
        repo = posixpath.normpath(posixpath.join("docs", path))
        return f"[{label}](https://github.com/OmsiModsBR/depot/blob/main/{repo}{target[len(path):]})"

    text = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", link, text)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.rstrip() + "\n"


def main():
    docs = Path(sys.argv[1])
    for p in sorted(docs.iterdir()):
        if p.is_dir():
            shutil.rmtree(p)
        elif p.suffix != ".md" or p.stem not in PUBLISHED:
            p.unlink()
    for name in PUBLISHED:
        p = docs / f"{name}.md"
        p.write_text(clean(p.read_text(encoding="utf-8"), PUBLISHED), encoding="utf-8")
        print(f"prepared {p}")


if __name__ == "__main__":
    main()
