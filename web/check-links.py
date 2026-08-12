#!/usr/bin/env python3
"""Resolve every local reference on every page against the filesystem.

A page moved one directory deeper keeps working right up until an asset path
resolves one level short, and the only symptom is a 404 in a console nobody has
open. This walks href, src and url() on each page, resolves them relative to
that page, and reports the ones with nothing behind them.

External links, anchors, mailto and data URIs are out of scope.
"""
from __future__ import annotations

import pathlib
import re
import sys
import urllib.parse

ROOT = pathlib.Path("web")
REF = re.compile(r"""(?:href|src)\s*=\s*["']([^"']+)["']|url\(\s*['"]?([^'")]+)""")


def check(page: pathlib.Path) -> list[str]:
    s = page.read_text(encoding="utf-8")
    bad = []
    for m in REF.finditer(s):
        raw = (m.group(1) or m.group(2) or "").strip()
        if not raw or raw.startswith(("#", "http://", "https://", "data:", "mailto:", "//")):
            continue
        target = urllib.parse.unquote(raw.split("#")[0].split("?")[0])
        if not target:
            continue
        # Site-absolute paths resolve from the web root, everything else from the page.
        base = ROOT if target.startswith("/") else page.parent
        resolved = (base / target.lstrip("/")).resolve()
        if resolved.is_dir():
            resolved = resolved / "index.html"
        if not resolved.exists():
            bad.append(f"{raw}  ->  {resolved.relative_to(pathlib.Path.cwd())}")
    return bad


def main() -> int:
    pages = sorted(ROOT.rglob("*.html"))
    total = 0
    for page in pages:
        bad = check(page)
        if bad:
            print(f"\n{page}")
            for b in sorted(set(bad)):
                print(f"  MISSING  {b}")
            total += len(set(bad))
    print(f"\n{len(pages)} page(s) checked, {total} broken local reference(s).")
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
