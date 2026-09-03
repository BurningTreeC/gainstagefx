#!/usr/bin/env python3
"""Regenerate THIRD-PARTY-NOTICES.md from the crates GainStageFx actually links.

Walks the dependency graph from the plugin's own package, following normal and
build dependencies but not dev-dependencies, collects each crate's licence and
copyright notices from its vendored source, and writes them out grouped by
licence.

    python3 tools/third-party-notices.py
"""

import json
import re
import subprocess
import sys
from pathlib import Path

PROJECT = Path(__file__).resolve().parent.parent
OUTPUT = PROJECT / "THIRD-PARTY-NOTICES.md"
TARGET = "x86_64-unknown-linux-gnu"

# Licence files are found by name; these are the ones crates.io crates use.
LICENCE_FILES = re.compile(r"^(LICEN[SC]E|COPYING|NOTICE|UNLICENSE)", re.IGNORECASE)
# Which licence a file called LICENSE-MIT and friends holds.
SUFFIX_TO_ID = {
    "MIT": "MIT",
    "APACHE": "Apache-2.0",
    "APACHE2": "Apache-2.0",
    "ZLIB": "Zlib",
    "BSD": "BSD-3-Clause",
    "ISC": "ISC",
    "MPL": "MPL-2.0",
    "GPL": "GPL-3.0",
    "0BSD": "0BSD",
    "CC0": "CC0-1.0",
    "UNICODE": "Unicode-3.0",
    "BLUEOAK": "BlueOak-1.0.0",
}
# A real copyright notice names a year and a holder. Licence bodies are full of
# lines that mention copyright without being one, so those are filtered out.
# Where a crate offers a choice of licence, this is the order this
# distribution takes them in. Every entry is compatible with the GPL, which the
# plugin as a whole is under.
PREFERENCE = [
    "MIT",
    "Apache-2.0",
    "ISC",
    "Zlib",
    "BSD-3-Clause",
    "BSD-2-Clause",
    "0BSD",
    "CC0-1.0",
    "Unicode-3.0",
    "MPL-2.0",
]

# Some crates bundle assets under a licence of their own. The fonts inside
# nih_plug_assets are the reason this matters here: the crate is ISC, but the
# Noto Sans files it embeds into the binary are under the SIL Open Font
# License, which has to travel with them.
CONTENT_IDS = [
    ("SIL OPEN FONT LICENSE", "OFL-1.1"),
    ("Mozilla Public License", "MPL-2.0"),
    ("GNU GENERAL PUBLIC LICENSE", "GPL-3.0"),
    ("GNU LESSER GENERAL PUBLIC", "LGPL"),
]

COPYRIGHT_LINE = re.compile(r"^\s*copyright\b.*$", re.IGNORECASE | re.MULTILINE)
NOT_A_NOTICE = re.compile(
    r"\[yyyy\]|\[year\]|<year>|\{year\}|copyright license|copyright notice"
    r"|copyright holder|copyright and permission|copyright \(c\) <|copyright owner",
    re.IGNORECASE,
)
HAS_YEAR = re.compile(r"(19|20)\d{2}")


def metadata():
    raw = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET],
        cwd=PROJECT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return json.loads(raw)


def plugin_name():
    """The name of the package that becomes the plugin, from its own manifest."""
    for line in (PROJECT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        match = re.match(r'\s*name\s*=\s*"([^"]+)"', line)
        if match:
            return match.group(1)
    sys.exit("no [package] name in Cargo.toml")


def linked_packages(meta):
    """Every package reachable from the plugin, excluding dev-dependencies."""
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    # The plugin package, and only that.
    #
    # Not every workspace member: `xtask` is a build tool that is run here and
    # never shipped, and listing what it links would claim the distribution
    # contains code it does not.
    name = plugin_name()
    roots = [p["id"] for p in meta["packages"] if p["name"] == name]
    if not roots:
        sys.exit(f"no package called {name} in the workspace")

    seen, stack = set(), list(roots)
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dep in nodes[current]["deps"]:
            if any(kind["kind"] in (None, "build") for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    for root in roots:
        seen.discard(root)
    return [packages[i] for i in seen]


def read(path):
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None


def licence_texts(package):
    """The licence files a crate ships, as {relative path: text}.

    Subdirectories are searched as well, because crates that bundle assets keep
    the asset's licence next to it. Crates that are members of a workspace
    often ship no licence of their own, so the workspace root is searched too.
    """
    source = Path(package["manifest_path"]).parent
    texts = {}
    if not source.is_dir():
        return texts

    for path in sorted(source.rglob("*")):
        if path.is_dir() or not LICENCE_FILES.match(path.name):
            continue
        relative = path.relative_to(source)
        if len(relative.parts) > 3:
            continue
        # A subdirectory with its own Cargo.toml is a different package, and
        # cargo lists it separately if we actually depend on it. Its licence is
        # not ours to report, which is what keeps nih-plug's own GPL licensed
        # example plugins out of this file.
        if any(
            (source.joinpath(*relative.parts[:depth]) / "Cargo.toml").is_file()
            for depth in range(1, len(relative.parts))
        ):
            continue
        text = read(path)
        if text:
            texts[str(relative)] = text

    if not texts:
        # A workspace member inherits the licence sitting at the workspace root.
        for parent in list(source.parents)[:3]:
            for path in sorted(parent.glob("*")):
                if path.is_file() and LICENCE_FILES.match(path.name):
                    text = read(path)
                    if text:
                        texts[path.name] = text
            if texts:
                break
    return texts


def bundled(package, texts):
    """Licences a crate carries that its own licence field does not mention.

    Compared by family rather than by exact id, so a crate offering
    `Apache-2.0 OR GPL-2.0-only` is not reported as bundling a surprise GPL.
    """
    declared = (package.get("license") or "").upper().replace("V3", "-3").replace("V2", "-2")
    found = []
    for text in texts.values():
        head = text[:400].lower()
        for marker, licence_id in CONTENT_IDS:
            family = licence_id.split("-")[0].upper()
            if marker.lower() in head and family not in declared:
                if licence_id not in found:
                    found.append(licence_id)
    return found


def copyrights(texts):
    """Copyright lines, which permissive licences require us to reproduce."""
    lines = []
    for text in texts.values():
        for match in COPYRIGHT_LINE.finditer(text):
            line = " ".join(match.group(0).split()).rstrip(".")
            if len(line) > 120 or NOT_A_NOTICE.search(line) or not HAS_YEAR.search(line):
                continue
            if line not in lines:
                lines.append(line)
    return lines


def normalise(expression):
    """crates.io has old style `MIT/Apache-2.0` alongside SPDX `OR`."""
    return " OR ".join(part.strip() for part in re.split(r"\s*(?:/|\bOR\b)\s*", expression))


def elect(expression):
    """The licences this distribution takes for a crate.

    A crate offering `MIT OR Apache-2.0` lets the user pick one, so only the
    picked one has to be reproduced. Terms joined by AND all apply.
    """
    taken = []
    for term in re.split(r"\s+AND\s+", expression):
        options = [option.strip("() ") for option in re.split(r"\s+OR\s+", term.strip("() "))]
        chosen = next((p for p in PREFERENCE if p in options), options[0])
        taken.append(chosen)
    return taken


def canonical_bodies(packages):
    """One full text per licence, taken from a crate that ships it."""
    bodies = {}
    for package in packages:
        expression = package.get("license") or ""
        for name, text in licence_texts(package).items():
            stem = name.upper().replace("LICENCE", "LICENSE")
            suffix = stem.split("-", 1)[1].split(".")[0] if "-" in stem else None
            licence_id = SUFFIX_TO_ID.get(suffix)
            if licence_id is None:
                for marker, sniffed in CONTENT_IDS:
                    if marker.lower() in text[:400].lower():
                        licence_id = sniffed
                        break
            if licence_id is None and " " not in expression and expression:
                licence_id = expression
            if licence_id and licence_id not in bodies and len(text) > 400:
                bodies[licence_id] = text.strip()
    return bodies


def main():
    meta = metadata()
    packages = sorted(
        linked_packages(meta), key=lambda p: (p["name"].lower(), p["version"])
    )

    groups = {}
    for package in packages:
        groups.setdefault(normalise(package.get("license") or "unspecified"), []).append(package)
    elected = {expression: elect(expression) for expression in groups}

    out = [
        "# Third party notices",
        "",
        "The GainStageFx plugin is distributed under the GNU General Public",
        "License version 3 or later, whose text is in `LICENSE`. It links the",
        f"{len(packages)} crates listed below, whose own licences and copyright notices",
        "are reproduced here as those licences require.",
        "",
        "Where a crate offers a choice of licence, the one this distribution",
        "takes is named alongside it, and that is the text reproduced below.",
        "",
        "Regenerate this file with `python3 tools/third-party-notices.py`.",
        "",
        "## Crates",
        "",
    ]

    for expression in sorted(groups):
        taken = elected[expression]
        heading = expression
        if taken != [expression]:
            heading += f" \u2014 taken as {' AND '.join(taken)}"
        out.append(f"### {heading}")
        out.append("")
        for package in groups[expression]:
            repository = package.get("repository") or ""
            link = f" <{repository}>" if repository else ""
            texts = licence_texts(package)
            extra = bundled(package, texts)
            note = f" \u2014 bundles assets under {', '.join(extra)}" if extra else ""
            out.append(f"- **{package['name']}** {package['version']}{link}{note}")
            for line in copyrights(texts):
                out.append(f"  - {line}")
        out.append("")

    bodies = canonical_bodies(packages)
    needed_now = {licence for taken in elected.values() for licence in taken}
    for package in packages:
        needed_now.update(bundled(package, licence_texts(package)))
    out.append("## License texts")
    out.append("")
    out.append("The GPLv3, which covers both this plugin and the `vst3-sys` crate,")
    out.append("is in `LICENSE` rather than repeated here.")
    out.append("")
    # The GPL text lives in LICENSE, so it is not repeated here.
    skip = {"GPLv3", "GPL-3.0", "GPL-3.0-or-later"}
    for licence_id in sorted(
        body for body in bodies if body in needed_now and body not in skip
    ):
        out.append(f"### {licence_id}")
        out.append("")
        out.append("```")
        out.append(bodies[licence_id])
        out.append("```")
        out.append("")

    needed = {licence for taken in elected.values() for licence in taken}
    for package in packages:
        needed.update(bundled(package, licence_texts(package)))
    in_license_file = {"GPLv3", "GPL-3.0", "GPL-3.0-or-later"}
    missing = sorted(
        licence
        for licence in needed
        if licence not in bodies and licence not in in_license_file
    )

    OUTPUT.write_text("\n".join(out), encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(PROJECT)}: {len(packages)} crates, "
          f"{len(groups)} licence expressions, {len(bodies)} licence texts")
    if missing:
        print("no text found for:", ", ".join(missing))
    return 0


if __name__ == "__main__":
    sys.exit(main())
