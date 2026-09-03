#!/usr/bin/env python3
"""Check cells.json against the comparison documents it was transcribed from.

    python3 check_cells_against_docs.py

cells.json is the single source the CIP's figures are generated from, but it is
written by hand from the per-design comparison documents rather than emitted by
the experiments tool. That makes the figures only as good as the transcription,
which is a silent failure mode: a mistyped digit produces a plausible figure.

This script closes the loop the cheap way. For every operating point it looks up
the measured quantities in that design's comparison document and reports any
value that does not appear there. It is not a substitute for the tool emitting
cells.json directly - it cannot catch a value that is wrong in both places - but
it does catch transcription slips, which is the failure the note warns about.

Configurations with no comparison document are reported as UNSOURCED rather than
passed over, since an unverifiable number should be visible as such.
"""
from __future__ import annotations

import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
CELLS = json.loads((HERE / "cells.json").read_text())

DOCS = {
    "M1": "m5-comparison.md",      # M1 is documented inside the M5 write-up
    "M2": "m2-comparison.md",
    "M3": "m3-comparison.md",
    "M4": "m4-comparison.md",
    "M5": "m5-comparison.md",
}

# The quantities worth checking: the ones the CIP's tables and figures read.
CHECKED = ("copies_per_node", "hops_full", "hops_mean", "msgs_per_publication")


def normalise(text: str) -> str:
    """Collapse thousands separators so 188 751, 188,751 and 188751 all match."""
    text = text.replace(" ", " ").replace(" ", " ")
    return re.sub(r"(?<=\d)[  ,](?=\d\d\d\b)", "", text)


def appears(value: float, haystack: str) -> bool:
    """Is this measurement present, at any of the precisions a doc might use?"""
    candidates = set()
    for dp in (0, 1, 2):
        candidates.add(f"{value:.{dp}f}")
    if float(value).is_integer():
        candidates.add(str(int(value)))
    return any(c in haystack for c in candidates)


def main() -> int:
    bad, unsourced, checked = [], [], 0
    groups = [("operating_points", CELLS.get("operating_points", ())),
              ("alternatives", CELLS.get("alternatives", ()))]

    for group, entries in groups:
        for e in entries:
            model, params = e["model"], e.get("params", "?")
            doc = HERE / DOCS.get(model, "")
            label = f"{group}/{model} ({params})"
            if not doc.exists():
                unsourced.append(f"{label}: no comparison document")
                continue
            text = normalise(doc.read_text())
            # A configuration is only sourced if the document mentions it.
            if params.replace(" ", "") not in text.replace(" ", ""):
                unsourced.append(f"{label}: {doc.name} does not mention this configuration")
                continue
            for key in CHECKED:
                if key not in e:
                    continue
                checked += 1
                if not appears(e[key], text):
                    bad.append(f"{label}: {key} = {e[key]} not found in {doc.name}")

    for line in unsourced:
        print(f"UNSOURCED  {line}")
    for line in bad:
        print(f"MISMATCH   {line}")

    print(f"\n{checked} values checked, {len(bad)} mismatched, "
          f"{len(unsourced)} configurations unsourced.")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
