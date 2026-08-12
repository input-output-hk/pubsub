#!/usr/bin/env python3
"""Check that every element a page's script looks up actually exists in its markup.

A JS syntax check passes happily on `$('.gone').addEventListener(...)`; the page
then dies on load. This catches the case that a refactor removed an element some
other code still reaches for, by resolving every id and class selector the
scripts use against the ids and classes present in the HTML.

Selectors that are only ever used under a null guard are reported separately
rather than as failures.
"""
from __future__ import annotations

import pathlib
import re
import sys


def check(path: pathlib.Path) -> int:
    s = path.read_text(encoding="utf-8")
    markup = re.sub(r"<script>.*?</script>", "", s, flags=re.S)

    ids = set(re.findall(r'\bid="([^"]+)"', markup))
    classes: set[str] = set()
    for c in re.findall(r'\bclass="([^"]+)"', markup):
        classes.update(c.split())

    scripts = "\n".join(re.findall(r"<script>(.*?)</script>", s, re.S))
    # Class names the scripts add themselves are legitimate targets too.
    for m in re.finditer(r"""class=\\?["']([^"'\\]+)""", scripts):
        classes.update(m.group(1).split())
    for m in re.finditer(r"classList\.(?:add|toggle)\(\s*['\"]([^'\"]+)", scripts):
        classes.add(m.group(1))
    for m in re.finditer(r"""className\s*=\s*['"]([^'"]+)""", scripts):
        classes.update(m.group(1).split())

    bad, guarded = [], []
    pattern = re.compile(r"""\$\(\s*['"]([#.])([A-Za-z0-9_-]+)['"]\s*\)""")
    for m in pattern.finditer(scripts):
        kind, name = m.group(1), m.group(2)
        present = name in (ids if kind == "#" else classes)
        if present:
            continue
        # Is the very next use a null check?
        after = scripts[m.end():m.end() + 90]
        line_start = scripts.rfind("\n", 0, m.start())
        before = scripts[line_start:m.start()]
        if re.search(r"\bif\s*\(", before) or re.match(r"\s*(?:\?\.|\|\||&&)", after):
            guarded.append(kind + name)
            continue
        # Assigned to a name that is null-checked later?
        var = re.search(r"(?:const|let|var)\s+(\w+)\s*=\s*$", before)
        if var and re.search(rf"\bif\s*\(\s*{var.group(1)}\s*\)", scripts):
            guarded.append(f"{kind}{name} (via {var.group(1)})")
            continue
        bad.append(kind + name)

    rel = path.as_posix()
    for sel in sorted(set(guarded)):
        print(f"  guarded   {rel}  {sel}")
    for sel in sorted(set(bad)):
        print(f"  MISSING   {rel}  {sel}")
    return len(set(bad))


if __name__ == "__main__":
    targets = [pathlib.Path(a) for a in sys.argv[1:]] or sorted(
        pathlib.Path("web").rglob("*.html"))
    total = sum(check(t) for t in targets)
    print(f"\n{len(targets)} page(s) checked, {total} missing selector(s).")
    sys.exit(1 if total else 0)
