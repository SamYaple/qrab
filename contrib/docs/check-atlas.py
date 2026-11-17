#!/usr/bin/env python3
"""Check atlas links, crate coverage and rendered/export provenance."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[2]
ATLAS = ROOT / "docs/architecture"


def heading_ids(text: str) -> set[str]:
    ids, counts = set(), {}
    in_code = False
    for line in text.splitlines():
        if line.startswith("```"):
            in_code = not in_code
        if in_code or not re.match(r"^#{1,6} ", line):
            continue
        label = re.sub(r"^#+ +", "", line).lower()
        label = re.sub(r"[^\w\- ]", "", label).replace(" ", "-")
        n = counts.get(label, 0)
        counts[label] = n + 1
        ids.add(f"{label}-{n}" if n else label)
    return ids


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exports", action="store_true", help="also validate the reading/presentation export manifest")
    args = parser.parse_args()
    documents = list(ATLAS.glob("*.md"))
    links = 0
    for document in documents:
        for target in re.findall(r"\]\(([^)]+)\)", document.read_text()):
            if re.match(r"[a-z]+://", target) or target.startswith("mailto:"):
                continue
            target = target.strip("<>")
            path, _, fragment = target.partition("#")
            resolved = (document.parent / unquote(path)).resolve() if path else document
            if not resolved.is_relative_to(ROOT) or not resolved.exists():
                raise ValueError(f"{document.name}: missing/unsafe target {target}")
            if fragment and resolved.suffix == ".md" and unquote(fragment) not in heading_ids(resolved.read_text()):
                raise ValueError(f"{document.name}: unknown heading {target}")
            links += 1
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]
    code_map = (ATLAS / "code-map.md").read_text()
    for member in workspace["members"]:
        if f"../../{member}/" not in code_map:
            raise ValueError(f"crate missing from code map: {member}")
    count = len(workspace["members"])
    defaults = len(workspace["default-members"])
    if f"workspace has {count} crates" not in code_map:
        raise ValueError("code-map crate count is stale")
    for original in json.loads((ATLAS / "upstream/provenance.json").read_text()):
        path = ATLAS / "upstream" / original["file"]
        if hashlib.sha256(path.read_bytes()).hexdigest() != original["sha256"]:
            raise ValueError(f"modified attributed upstream image: {original['file']}")
    # The prose spells the default count; source coverage is checked through
    # membership and every crate's link rather than a duplicate package list.
    subprocess.run([sys.executable, str(ROOT / "contrib/docs/render-atlas.py"), "--check"], check=True)
    if args.exports:
        manifest = json.loads((ATLAS / "exports/manifest.json").read_text())
        for kind in ["inputs", "outputs"]:
            for relative, expected in manifest[kind].items():
                path = (ROOT / relative).resolve()
                if not path.is_relative_to(ROOT):
                    raise ValueError(f"unsafe export path: {relative}")
                actual = hashlib.sha256(path.read_bytes()).hexdigest()
                if actual != expected:
                    raise ValueError(f"stale export {kind}: {relative}")
    print(f"Validated {len(documents)} atlas documents, {links} local links and {count} crates ({defaults} default).")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError) as error:
        sys.exit(str(error))
