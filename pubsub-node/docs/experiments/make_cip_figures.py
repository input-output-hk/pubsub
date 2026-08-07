#!/usr/bin/env python3
"""Generate the CIP evidence figures from cells.json.

    python3 make_cip_figures.py            # regenerate ../../../docs/cip/images/*.svg
    python3 make_cip_figures.py --check    # verify committed SVGs are up to date

Emits plain SVG using presentation attributes only: GitHub's markdown sanitiser
strips <style> blocks and scripts, so nothing here may depend on them. Each
figure carries an opaque light plot surface so it renders identically under
GitHub's light and dark themes.
"""
from __future__ import annotations

import argparse
import json
import math
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
DATA = HERE / "cells.json"
OUT = HERE.parents[2] / "docs" / "cip" / "images"

# Categorical hues, checked for colour-vision separation as an ordered set.
SERIES = {
    "M1": "#2a78d6",
    "M2": "#eb6834",
    "M3": "#1baf7a",
    "M4": "#eda100",
    "M5": "#e87ba4",
}
SURFACE = "#fcfcfb"
INK = "#1a1a19"
INK_SOFT = "#52514e"
GRID = "#e2e0d8"
RULE = "#b9b6ab"

SUPERS = str.maketrans("-0123456789", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹")


def esc(s: str) -> str:
    return (str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))


def decade(v: float) -> str:
    return "10" + str(int(round(math.log10(v)))).translate(SUPERS)


def wilson(k: int, n: int, z: float = 1.959963985) -> tuple[float, float]:
    if n == 0:
        return 0.0, 1.0
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = (z / d) * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))
    return max(0.0, c - h), min(1.0, c + h)


def text(x, y, s, size=11.5, fill=INK_SOFT, anchor="start", weight=None, style=None):
    a = f'x="{x:.1f}" y="{y:.1f}" font-size="{size}" fill="{fill}"'
    a += ' font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif"'
    if anchor != "start":
        a += f' text-anchor="{anchor}"'
    if weight:
        a += f' font-weight="{weight}"'
    if style:
        a += f' font-style="{style}"'
    return f"<text {a}>{esc(s)}</text>"


def line(x1, y1, x2, y2, stroke=GRID, w=1.0, cap=None, dash=None, opacity=None):
    a = f'x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" stroke="{stroke}" stroke-width="{w}"'
    if cap:
        a += f' stroke-linecap="{cap}"'
    if dash:
        a += f' stroke-dasharray="{dash}"'
    if opacity is not None:
        a += f' opacity="{opacity}"'
    return f"<line {a}/>"


def circle(cx, cy, r, fill, stroke=None, w=2.0, opacity=None):
    a = f'cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="{fill}"'
    if stroke:
        a += f' stroke="{stroke}" stroke-width="{w}"'
    if opacity is not None:
        a += f' opacity="{opacity}"'
    return f"<circle {a}/>"


def frame(w: int, h: int, body: list[str], title: str, desc: str) -> str:
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" '
        f'height="{h}" role="img" aria-labelledby="t d">\n'
        f"<title id=\"t\">{esc(title)}</title>\n<desc id=\"d\">{esc(desc)}</desc>\n"
        f'<rect width="{w}" height="{h}" rx="8" fill="{SURFACE}"/>\n'
        + "\n".join(body)
        + "\n</svg>\n"
    )


# ------------------------------------------------------------------ figure 1
def fig_validation(cells) -> str:
    W, H = 860, 500
    ml, mr, mt, mb = 86, 26, 22, 76
    pw, ph = W - ml - mr, H - mt - mb
    lo, hi = 2e-3, 1.15
    lg = math.log10

    def X(v):
        return ml + (lg(max(v, lo)) - lg(lo)) / (lg(hi) - lg(lo)) * pw

    def Y(v):
        return mt + ph - (lg(max(v, lo)) - lg(lo)) / (lg(hi) - lg(lo)) * ph

    b = []
    for e in range(-3, 1):
        v = 10.0**e
        if not (lo <= v <= hi):
            continue
        b.append(line(X(v), mt, X(v), mt + ph))
        b.append(line(ml, Y(v), ml + pw, Y(v)))
        b.append(text(X(v), mt + ph + 19, decade(v), anchor="middle"))
        b.append(text(ml - 11, Y(v) + 4, decade(v), anchor="end"))

    b.append(line(X(lo), Y(lo), X(hi), Y(hi), RULE, 1.8, cap="round"))


    for c in cells:
        x, p = X(c["law"]), c["bad"] / c["runs"]
        wl, wh = wilson(c["bad"], c["runs"])
        b.append(line(x, Y(max(wl, lo)), x, Y(wh), SERIES[c["model"]], 2.0,
                      cap="round", opacity=0.45))
    for c in cells:
        b.append(circle(X(c["law"]), Y(c["bad"] / c["runs"]), 4.4,
                        SERIES[c["model"]], SURFACE, 1.6))

    b.append(text(ml + pw / 2, H - 34, "P(bad) predicted by the coverage law",
                  12.5, INK, "middle", "600"))
    b.append(text(ml + pw / 2, H - 18,
                  "log scale, each gridline ×10 — left: almost never fails · "
                  "right: fails most epochs", 11, INK_SOFT, "middle"))
    b.append(f'<text x="0" y="0" transform="translate(22,{mt + ph / 2:.1f}) rotate(-90)" '
             f'text-anchor="middle" font-size="12.5" font-weight="600" fill="{INK}" '
             f'font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif">'
             f'P(bad) measured</text>')
    b.append(f'<text x="0" y="0" transform="translate(37,{mt + ph / 2:.1f}) rotate(-90)" '
             f'text-anchor="middle" font-size="11" fill="{INK_SOFT}" '
             f'font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif">'
             f'bar = Wilson 95% interval</text>')

    lx = ml + 14
    b.append(text(lx, mt + 20, "one point = one tested configuration:", 11.5, INK_SOFT))
    b.append(text(lx, mt + 38, "grey line = the law matched measurement exactly",
                  11, "#8a887e"))
    for i, (m, col) in enumerate(SERIES.items()):
        cx = lx + 218 + i * 52
        b.append(circle(cx, mt + 16, 4.4, col, SURFACE, 1.6))
        b.append(text(cx + 9, mt + 20, m, 11.5, INK_SOFT))

    return frame(W, H, b, "Measured against predicted epoch failure probability",
                 "Each point is one experiment configuration. Horizontal position is the "
                 "probability predicted by the closed-form coverage law, vertical position "
                 "the fraction of sampled topologies that actually failed. Bars are Wilson "
                 "95% intervals. Points lie on the diagonal across three decades.")


# ------------------------------------------------------------------ figure 2
def fig_cost_state(ops) -> str:
    W, H = 860, 430
    ml, mr, mt, mb = 96, 40, 30, 80
    pw, ph = W - ml - mr, H - mt - mb
    x0, x1, y0, y1 = 10, 52, 8, 21

    def X(v):
        return ml + (v - x0) / (x1 - x0) * pw

    def Y(v):
        return mt + ph - (v - y0) / (y1 - y0) * ph

    b = []
    for v in range(10, 51, 10):
        b.append(line(X(v), mt, X(v), mt + ph))
        b.append(text(X(v), mt + ph + 19, v, anchor="middle"))
    for v in range(8, 21, 4):
        b.append(line(ml, Y(v), ml + pw, Y(v)))
        b.append(text(ml - 11, Y(v) + 4, v, anchor="end"))

    front = sorted([o for o in ops if o["model"] in ("M3", "M4")],
                   key=lambda o: o["standing_links"])
    b.append(line(X(front[0]["standing_links"]), Y(front[0]["copies_per_node"]),
                  X(front[1]["standing_links"]), Y(front[1]["copies_per_node"]),
                  "#1e8f5e", 1.8, cap="round", opacity=0.55))
    mx = (X(front[0]["standing_links"]) + X(front[1]["standing_links"])) / 2
    my = (Y(front[0]["copies_per_node"]) + Y(front[1]["copies_per_node"])) / 2
    b.append(text(mx, my - 11, "nothing beats both", 11, "#1e8f5e", "middle", "600"))

    seen: dict[tuple[float, float], list] = {}
    for o in ops:
        seen.setdefault((o["standing_links"], o["copies_per_node"]), []).append(o)

    for (sl, cp), group in seen.items():
        r = 4 + (max(g["hops_full"] for g in group) - 4.5) * 5.5
        if len(group) > 1:
            b.append(circle(X(sl), Y(cp), r + 3.5, SERIES[group[0]["model"]], SURFACE, 1.6))
        b.append(circle(X(sl), Y(cp), r, SERIES[group[-1]["model"]], SURFACE, 1.6))
        label = "  /  ".join(f"{g['model']} · {g['params']}" for g in group)
        b.append(text(X(sl), Y(cp) - r - 10, label, 12, INK, "middle", "600"))

    b.append(text(ml + 6, mt + ph - 8, "↙ cheaper on both axes = better",
                  11, INK_SOFT, style="italic"))
    # marker size carries a third axis, so it needs its own key
    b.append(text(ml + 14, mt + 20, "marker size = hops to reach the last subscriber",
                  10.5, "#8a887e"))
    for dx, hops in ((22, 4.8), (86, 5.9)):
        r = 4 + (hops - 4.5) * 5.5
        b.append(circle(ml + 14 + dx, mt + 44, r, "#b9b6ab", SURFACE, 1.6))
        b.append(text(ml + 14 + dx + r + 6, mt + 48, f"{hops}", 10.5, "#8a887e"))

    b.append(text(ml + pw / 2, H - 36,
                  "State cost — connections each node holds open all epoch",
                  12.5, INK, "middle", "600"))
    b.append(text(ml + pw / 2, H - 20, "right = more connection slots and churn surface",
                  11, INK_SOFT, "middle"))
    b.append(f'<text x="0" y="0" transform="translate(26,{mt + ph / 2:.1f}) rotate(-90)" '
             f'text-anchor="middle" font-size="12.5" font-weight="600" fill="{INK}" '
             f'font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif">'
             f'Bandwidth cost — copies per honest node</text>')
    b.append(f'<text x="0" y="0" transform="translate(41,{mt + ph / 2:.1f}) rotate(-90)" '
             f'text-anchor="middle" font-size="11" fill="{INK_SOFT}" '
             f'font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif">'
             f'up = more traffic</text>')

    return frame(W, H, b, "Bandwidth cost against state cost at equal safety",
                 "Each design tuned to the same failure target, so points differ only in "
                 "cost. Horizontal axis is standing connections per node, vertical axis is "
                 "message copies per honest node; both are costs, so lower-left is better. "
                 "M3 and M4 are jointly non-dominated.")


# ------------------------------------------------------------------ figure 3
def fig_tradeoffs(ops) -> str:
    """Four-axis trade-off, as small multiples rather than one overlay.

    On all four axes only M1 is dominated (by M5); M2, M3, M4 and M5 are each
    best at something. Four filled polygons in the models' own hues cannot be
    told apart reliably — magenta against orange falls under the normal-vision
    separation floor — so each model gets its own panel in its own colour, with
    the other three drawn behind it in grey. Colours therefore stay identical to
    the other figures, and comparison is against a fixed backdrop.

    Each axis is scored best/this, so the best design on an axis reaches the
    outer ring and a larger shape is better.
    """
    import math as _m
    W, H, R = 900, 700, 92
    by = {o["model"]: o for o in ops}
    SHOWN = ["M3", "M4", "M5", "M2"]

    AXES = [
        ("Bandwidth economy", lambda o: o["copies_per_node"], True),
        ("Connection economy", lambda o: o["standing_links"], True),
        ("Speed", lambda o: o["hops_full"], True),
        ("Churn tolerance", lambda o: o["churn_budget_pct"], False),
    ]
    best = [(min if low else max)(get(by[m]) for m in SHOWN)
            for _, get, low in AXES]

    def score(m, i):
        _, get, low = AXES[i]
        v = get(by[m])
        return (best[i] / v) if low else (v / best[i])

    ang = [-_m.pi / 2, 0.0, _m.pi / 2, _m.pi]

    def poly(cxx, cyy, m):
        pts = []
        for i in range(4):
            s = score(m, i)
            pts.append((cxx + R * s * _m.cos(ang[i]), cyy + R * s * _m.sin(ang[i])))
        return " ".join(f"{x:.1f},{y:.1f}" for x, y in pts)

    b = [text(38, 30, "All four axes are oriented so that outward is better: further "
              "out means less bandwidth used, fewer connections held, fewer hops to the "
              "last subscriber,", 11, INK_SOFT, style="italic"),
         text(38, 46, "and more honest downtime absorbed. Each is scored against the "
              "best of the four. Grey shapes are the other three designs, for "
              "comparison; the churn axis is", 11, INK_SOFT, style="italic"),
         text(38, 62, "predicted from the coverage laws rather than measured.",
              11, INK_SOFT, style="italic")]

    for k, m in enumerate(SHOWN):
        cxx = 215 + (k % 2) * 430
        cyy = 246 + (k // 2) * 310
        for ring in (0.33, 0.66, 1.0):
            pts = " ".join(
                f"{cxx + R * ring * _m.cos(a):.1f},{cyy + R * ring * _m.sin(a):.1f}"
                for a in ang)
            b.append(f'<polygon points="{pts}" fill="none" stroke="{GRID}" stroke-width="1"/>')
        for a in ang:
            b.append(line(cxx, cyy, cxx + R * _m.cos(a), cyy + R * _m.sin(a), GRID, 1))
        for other in SHOWN:
            if other == m:
                continue
            b.append(f'<polygon points="{poly(cxx, cyy, other)}" fill="none" '
                     f'stroke="#d5d3ca" stroke-width="1.4" stroke-linejoin="round"/>')
        col = SERIES[m]
        b.append(f'<polygon points="{poly(cxx, cyy, m)}" fill="{col}" fill-opacity="0.20" '
                 f'stroke="{col}" stroke-width="2.4" stroke-linejoin="round"/>')
        for i in range(4):
            s = score(m, i)
            b.append(circle(cxx + R * s * _m.cos(ang[i]), cyy + R * s * _m.sin(ang[i]),
                            4, col, SURFACE, 1.6))
        for i, (name, get, _low) in enumerate(AXES):
            x = cxx + (R + 16) * _m.cos(ang[i])
            y = cyy + (R + 16) * _m.sin(ang[i])
            anchor = "middle" if i in (0, 2) else ("start" if i == 1 else "end")
            dy = -8 if i == 0 else (16 if i == 2 else 4)
            b.append(text(x, y + dy, name, 10.5, "#8a887e", anchor))
        cap = {"M3": "M3 · RF=12, s=8", "M4": "M4 · RF=8",
               "M5": "M5 · (9, 8)", "M2": "M2 · RF=24"}[m]
        note = {"M3": "least bandwidth", "M4": "fewest connections",
                "M5": "most churn-tolerant", "M2": "fastest to full coverage"}[m]
        b.append(text(cxx, cyy - R - 62, cap, 13, col, "middle", "650"))
        b.append(text(cxx, cyy - R - 46, note, 10.5, "#8a887e", "middle"))

    return frame(W, H, b, "Four-way trade-off across the non-dominated designs",
                 "Four small radar charts, one per design, on the axes bandwidth, "
                 "connections, latency and churn tolerance. Each design is best on "
                 "exactly one axis: M3 bandwidth, M4 connections, M5 churn tolerance, "
                 "M2 latency. M3 shows the most uneven shape and M4 the most even. "
                 "The churn axis is predicted rather than measured.")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true",
                    help="fail if the committed SVGs differ from freshly generated ones")
    args = ap.parse_args()

    d = json.loads(DATA.read_text())
    figs = {
        "coverage-validation.svg": fig_validation(d["coverage_cells"]),
        "cost-vs-state.svg": fig_cost_state(d["operating_points"]),
        "tradeoff-radar.svg": fig_tradeoffs(d["operating_points"]),
    }

    rc = 0
    OUT.mkdir(parents=True, exist_ok=True)
    for name, svg in figs.items():
        path = OUT / name
        if args.check:
            if not path.exists() or path.read_text() != svg:
                print(f"stale: {path}", file=sys.stderr)
                rc = 1
        else:
            path.write_text(svg)
            print(f"wrote {path}")
    if args.check and rc == 0:
        print("figures up to date")
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
