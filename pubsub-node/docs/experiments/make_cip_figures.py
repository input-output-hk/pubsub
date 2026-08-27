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
    "M1": "#2a78d6",   # blue
    "M2": "#4a3aa7",   # violet
    "M3": "#008300",   # green
    "M4": "#eda100",   # yellow
    "M5": "#e87ba4",   # magenta
}
SURFACE = "#fcfcfb"
INK = "#1a1a19"
INK_SOFT = "#52514e"
GRID = "#e2e0d8"
RULE = "#b9b6ab"

# Reserved strip at the foot of a stamped figure, holding the conditions line
# and nothing else.
STAMP_BAND = 26

SUPERS = str.maketrans("-0123456789", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹")


def esc(s: str) -> str:
    return (str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))


def runs(s: str) -> str:
    """Label markup, rendering `_{...}` as a subscript.

    The prose writes its quantities with subscripts - p_bad, N_T, h_full - and a
    figure that spells them differently reads as a different quantity. GitHub
    strips <style>, so the shift is a presentation attribute on a tspan, and the
    run after a subscript carries the opposite dy to restore the baseline.
    """
    s = str(s)
    parts, i = [], 0
    while (j := s.find("_{", i)) >= 0:
        k = s.index("}", j)
        parts.append(("lit", s[i:j]))
        parts.append(("sub", s[j + 2:k]))
        i = k + 1
    parts.append(("lit", s[i:]))

    out, shifted = [], False
    for kind, txt in parts:
        if kind == "sub":
            out.append(f'<tspan font-size="0.72em" dy="0.26em">{esc(txt)}</tspan>')
            shifted = True
        elif txt:
            if shifted:
                # a run opening a tspan loses its leading space to whitespace
                # collapsing, and "N_T− 1" is a different expression from "N_T − 1".
                # Belt and braces: the non-breaking space survives a renderer that
                # ignores xml:space, and xml:space one that collapses the nbsp.
                txt = " " + txt[1:] if txt.startswith(" ") else txt
                out.append(f'<tspan dy="-0.26em">{esc(txt)}</tspan>')
            else:
                out.append(esc(txt))
            shifted = False
    return "".join(out)


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
    if "_{" in str(s):
        a += ' xml:space="preserve"'
    return f"<text {a}>{runs(s)}</text>"


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


def rect(x, y, w, h, fill="none", stroke=GRID, sw=1.2, rx=6, dash=None):
    a = (f'x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" rx="{rx}" '
         f'fill="{fill}" stroke="{stroke}" stroke-width="{sw}"')
    if dash:
        a += f' stroke-dasharray="{dash}"'
    return f"<rect {a}/>"


def arrow(x1, y1, x2, y2, stroke=RULE, w=1.6, head=7.0):
    """A line with an explicit triangular head.

    SVG <marker> definitions do not survive GitHub's markdown sanitiser, so the
    head is drawn as an ordinary polygon rather than referenced by marker-end.
    """
    dx, dy = x2 - x1, y2 - y1
    ln = math.hypot(dx, dy) or 1.0
    ux, uy = dx / ln, dy / ln
    bx, by = x2 - ux * head, y2 - uy * head
    px, py = -uy * head * 0.5, ux * head * 0.5
    return (line(x1, y1, bx, by, stroke, w, cap="round") + "\n"
            + f'<polygon points="{x2:.1f},{y2:.1f} {bx + px:.1f},{by + py:.1f} '
              f'{bx - px:.1f},{by - py:.1f}" fill="{stroke}"/>')


def frame(w: int, h: int, body: list[str], title: str, desc: str,
          conditions: str | None = None) -> str:
    """Wrap a figure body, stamping the conditions it was measured under.

    The stamp goes inside the image rather than in the markdown caption
    because figures get screenshotted into slides and pasted into chat, where
    a caption does not follow them. Every number in a figure is one slice of
    a parameter space, and the slice should travel with the picture.

    A stamped figure is drawn STAMP_BAND taller than the body asks for, and
    the stamp sits in that band alone. Placing it inside the body's own last
    line let a long axis title or legend note run into it from the left,
    which cost the reader exactly the numbers the stamp exists to carry.
    """
    stamp = ""
    fh = h
    if conditions:
        fh = h + STAMP_BAND
        stamp = text(w - 14, fh - 11, conditions, 9.5, "#8a887e", "end") + "\n"
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {fh}" width="{w}" '
        f'height="{fh}" role="img" aria-labelledby="t d">\n'
        f"<title id=\"t\">{esc(title)}</title>\n<desc id=\"d\">{esc(desc)}</desc>\n"
        f'<rect width="{w}" height="{fh}" rx="8" fill="{SURFACE}"/>\n'
        + "\n".join(body) + "\n" + stamp
        + "</svg>\n"
    )


# ------------------------------------------------------------------ figure 1
def fig_architecture() -> str:
    """The Specification's opening map: what the parts are and what flows between them.

    A structural diagram, so its content is literal rather than drawn from
    cells.json. It stays in this script anyway, so that `--check` keeps every
    figure in the CIP under one gate and the palette cannot drift.

    Three bands, read downward, because that is the order the protocol runs in:
    the chain supplies inputs, every node turns them into the same link set
    independently, and messages then travel over those links.

    Deliberately an overview: the boxes carry names only. What each registry
    holds, what the gate computes and what a link costs are all stated
    normatively within a page or two of this figure, so repeating them here
    only competes with the Specification for the reader's attention. What the
    figure keeps is what running prose cannot show at a glance: the three
    inputs, the order they are consumed in, and where the public derivation
    stops and the node's private draw begins.
    """
    W, H = 860, 492
    b = []
    verifiable = SERIES["M2"]
    private = "#1e8f5e"

    def band(y, h, n, title):
        # numbered, so the Specification's subsections can name which layer they are
        b.append(rect(38, y, W - 76, h, SURFACE, GRID, 1.2, rx=8))
        b.append(circle(66, y + 19, 11, INK, INK, 0))
        b.append(text(66, y + 23, str(n), 11.5, SURFACE, "middle", "700"))
        b.append(text(84, y + 24, title, 12.5, INK, weight="600"))

    def box(x, y, w, h, head, stroke=RULE, head_fill=INK):
        b.append(rect(x, y, w, h, SURFACE, stroke, 1.4))
        b.append(text(x + w / 2, y + h / 2 + 4, head, 11.5, head_fill, "middle", "600"))

    band(38, 96, 1, "On the Cardano chain")
    box(60, 80, 236, 38, "Node registry", verifiable)
    box(312, 80, 236, 38, "Topic registry", verifiable)
    box(564, 80, 236, 38, "Randomness beacon", verifiable)

    for x, lab in ((178, "membership at the cutoff"), (430, "publisher keys"),
                   (682, "epoch randomness  η")):
        b.append(arrow(x, 134, x, 170, RULE, 1.6))
        b.append(text(x + 10, 156, lab, 10, "#8a887e"))

    band(178, 118, 2, "In every node, from those inputs alone")
    stages = [(60, "Registered peers", verifiable), (254, "Verifiable gate", verifiable),
              (448, "Pick", private), (642, "Link set", INK_SOFT)]
    for x, head, col in stages:
        box(x, 220, 158, 38, head, col, col if col != INK_SOFT else INK)
    for x0 in (218, 412, 606):
        b.append(arrow(x0 + 2, 239, x0 + 34, 239, RULE, 1.6))
    b.append(text(60, 282, "Recomputable by anyone holding the chain", 10, verifiable,
                  weight="600"))
    b.append(text(304, 282, "→", 10, "#8a887e"))
    b.append(text(324, 282, "the node's own draw, and not required to be checkable",
                  10, private))

    b.append(arrow(430, 296, 430, 332, RULE, 1.6))
    b.append(text(440, 318, "one signed handshake per link", 10, "#8a887e"))

    band(340, 116, 3, "Over those links, until the epoch ends")
    b.append(text(430, 390, "signed once by the publisher, verified by every recipient",
                  10, "#8a887e", "middle"))
    for x, lab in ((130, "publisher"), (320, "relay"), (540, "relay"), (770, "subscriber")):
        b.append(circle(x, 416, 13, SURFACE, INK_SOFT, 1.8))
        b.append(text(x, 442, lab, 10.5, INK_SOFT, "middle"))
    for x0, x1 in ((143, 307), (553, 757)):
        b.append(arrow(x0, 416, x1, 416, RULE, 1.6))
    # the run between the first and last relay is any number of hops, not one:
    # a dashed span with an ellipsis, so the figure stops implying a fixed depth
    b.append(line(333, 416, 527, 416, RULE, 1.6, dash="5 5"))
    b.append(circle(430, 416, 13, SURFACE, SURFACE, 0))
    b.append(text(430, 421, "\u22ef", 17, RULE, "middle"))
    b.append(text(430, 442, "any number of relays", 10.5, INK_SOFT, "middle"))

    return frame(W, H, b, "The protocol at a glance",
                 "Three numbered bands read downward. Band 1, the Cardano chain, holds a node registry, a "
                 "topic registry and a per-epoch randomness beacon, contributing "
                 "membership, publisher keys and the epoch randomness respectively. Every "
                 "node turns those public inputs into its registered peers on a topic, "
                 "applies the verifiable gate, picks from the survivors with its own "
                 "private randomness, and holds the resulting links for the epoch; the "
                 "steps up to the gate are recomputable by anyone holding the chain and "
                 "the pick is not. Messages then travel over those links from publisher "
                 "through any number of relays to subscribers, signed once end to end.")


# ------------------------------------------------------------------ models
def _model_layer_row(b, y, picked, fill, quietfill=None):
    """A row of the topic's peers with the linked subset filled, fig-3 idiom."""
    x0, x1, n = 300, 812, 16
    step = (x1 - x0) / (n - 1)
    for i in range(n):
        x = x0 + i * step
        if picked and i in picked:
            b.append(circle(x, y, 5.2, fill, SURFACE, 1.5))
        elif quietfill and i in quietfill:
            b.append(circle(x, y, 5.2, "#8a887e", SURFACE, 1.5))
        else:
            b.append(circle(x, y, 4.6, SURFACE, GRID, 1.5))

def _model_scaffold(b, up_title, up_sub, dn_title, dn_sub):
    quiet = "#8a887e"
    b.append(text(38, 62, up_title, 12.5, INK, weight="600"))
    b.append(text(38, 80, up_sub, 10.5, quiet))
    b.append(line(38, 116, 822, 116, GRID, 1.2, dash="5 6"))
    b.append(text(38, 170, "Current node", 12.5, INK, weight="600"))
    b.append(circle(556, 165, 11, SURFACE, INK, 2.2))
    b.append(line(38, 214, 822, 214, GRID, 1.2, dash="5 6"))
    b.append(text(38, 262, dn_title, 12.5, INK, weight="600"))
    b.append(text(38, 280, dn_sub, 10.5, quiet))

def _model_link(b, i, up, col, dashed=False, both=False):
    """One link between the current node and peer i of a layer row."""
    x0, step = 300, (812 - 300) / 15
    x = x0 + i * step
    cx, cy = 556, 165
    if up:
        p1 = (x + (cx - x) * 0.06, 82); p2 = (cx + (x - cx) * 0.08, cy - 18)
    else:
        p1 = (cx + (x - cx) * 0.08, cy + 18); p2 = (x - (x - cx) * 0.06, 262)
    if dashed:
        b.append(line(p1[0], p1[1], p2[0], p2[1], col, 1.5, dash="4 5"))
        dx, dy = p2[0] - p1[0], p2[1] - p1[1]
        ln = math.hypot(dx, dy)
        b.append(arrow(p2[0] - dx / ln * 8, p2[1] - dy / ln * 8, p2[0], p2[1], col, 1.5))
    else:
        b.append(arrow(p1[0], p1[1], p2[0], p2[1], col, 1.6))
        if both:
            b.append(arrow(p2[0], p2[1], p1[0], p1[1], col, 1.6))


def fig_model_m1() -> str:
    """One node's links under M1, seen from the node itself.

    Three layers separated by dashed dividers, read top to bottom: the peers
    whose draws included this node, the node itself, and the peers it drew.
    Each outer layer is a row of the topic's peers in the idiom of the
    derivation figure, with the linked subset filled, so the rows read as a
    chosen few among the eligible many. Text is the layer titles and
    subtitles alone; the prose beside the figure carries everything else.
    """
    W, H = 860, 330
    b = []
    quiet = "#8a887e"
    col = SERIES["M1"]
    x0, x1 = 300, 812
    n = 16
    step = (x1 - x0) / (n - 1)
    cx, cy = (x0 + x1) / 2, 165

    def row(y, picked, fill):
        for i in range(n):
            x = x0 + i * step
            if i in picked:
                b.append(circle(x, y, 5.2, fill, SURFACE, 1.5))
            else:
                b.append(circle(x, y, 4.6, SURFACE, GRID, 1.5))

    UP = [3, 8, 13]
    DN = [1, 5, 10, 14]

    b.append(text(38, 62, "Upstream", 12.5, INK, weight="600"))
    b.append(text(38, 80, "they initiate: nodes whose draw included this one", 10.5, quiet))
    row(72, UP, quiet)
    for i in UP:
        x = x0 + i * step
        b.append(arrow(x + (cx - x) * 0.06, 82, cx + (x - cx) * 0.08, cy - 18, quiet, 1.4))

    b.append(line(38, 116, W - 38, 116, GRID, 1.2, dash="5 6"))
    b.append(text(38, 170, "Current node", 12.5, INK, weight="600"))
    b.append(circle(cx, cy, 11, SURFACE, INK, 2.2))
    b.append(line(38, 214, W - 38, 214, GRID, 1.2, dash="5 6"))

    b.append(text(38, 262, "Downstream", 12.5, INK, weight="600"))
    b.append(text(38, 280, "this node initiates: the F targets it drew", 10.5, quiet))
    row(272, DN, col)
    for i in DN:
        x = x0 + i * step
        b.append(arrow(cx + (x - cx) * 0.08, cy + 18, x - (x - cx) * 0.06, 262, col, 1.6))

    return frame(W, H, b, "One node's links under M1",
                 "Three layers separated by dashed lines. The top row holds the topic's "
                 "peers with three filled: nodes whose own draws included this one, "
                 "linking down to the current node. The bottom row holds the same peers "
                 "with four filled: the F targets the current node drew, linked from it. "
                 "Unfilled circles are eligible peers not drawn on either side.")


def fig_model_m2() -> str:
    """One node's links under M2: the M1 figure with the colour flipped."""
    W, H = 860, 330
    b = []
    col = SERIES["M2"]
    _model_scaffold(b, "Upstream", "this node initiates: the RF forwarders it drew",
                    "Downstream", "they initiate: nodes whose draw included this one")
    _model_layer_row(b, 72, [2, 6, 10, 14], col)
    for i in [2, 6, 10, 14]:
        _model_link(b, i, True, col)
    _model_layer_row(b, 272, None, None, quietfill=[4, 9, 12])
    for i in [4, 9, 12]:
        _model_link(b, i, False, "#8a887e")
    return frame(W, H, b, "One node's links under M2",
                 "The same three layers as the M1 figure with the colours exchanged. The "
                 "top row's filled peers are the RF forwarders the current node drew, "
                 "links it initiated. The bottom row's filled peers drew this node as a "
                 "forwarder; they initiated those links. Messages still flow downward.")


def fig_model_m3() -> str:
    """One node's links under M3: M2's relay links plus dashed seeding links."""
    W, H = 860, 330
    b = []
    col = SERIES["M3"]
    _model_scaffold(b, "Upstream", "this node initiates: the RF relay forwarders it drew",
                    "Downstream",
                    "dashed: its seeding links · grey: their relay draws")
    _model_layer_row(b, 72, [2, 6, 10, 14], col)
    for i in [2, 6, 10, 14]:
        _model_link(b, i, True, col)
    _model_layer_row(b, 272, [1, 8], col, quietfill=[5, 12])
    for i in [1, 8]:
        _model_link(b, i, False, col, dashed=True)
    for i in [5, 12]:
        _model_link(b, i, False, "#8a887e")
    return frame(W, H, b, "One node's links under M3",
                 "M2's layers with one addition. The top row holds the RF relay "
                 "forwarders the current node drew. The bottom row holds, dashed and in "
                 "the design's colour, the s minus 1 seeding links the node initiates to "
                 "hand out its own publications, beside the grey relay draws made by "
                 "others. Messages flow downward on every link.")


def fig_model_m5() -> str:
    """One node's links under M5: both layers are the node's own draws."""
    W, H = 860, 330
    b = []
    col = SERIES["M5"]
    _model_scaffold(b, "Upstream",
                    "its k_in senders drawn · grey: their outbound draws",
                    "Downstream",
                    "its k_out receivers drawn · grey: their inbound draws")
    _model_layer_row(b, 72, [2, 7, 12, 14], col, quietfill=[4, 9])
    for i in [2, 7, 12, 14]:
        _model_link(b, i, True, col)
    for i in [4, 9]:
        _model_link(b, i, True, "#8a887e")
    _model_layer_row(b, 272, [3, 8, 13], col, quietfill=[6, 11])
    for i in [3, 8, 13]:
        _model_link(b, i, False, col)
    for i in [6, 11]:
        _model_link(b, i, False, "#8a887e")
    return frame(W, H, b, "One node's links under M5",
                 "Both outer layers now hold links in the design's colour: the current "
                 "node draws its k_in senders above and its k_out receivers below, "
                 "tuning the two counts separately. Grey links are the same two draws "
                 "made by other nodes that happened to land on this one. Messages flow "
                 "downward on every link.")


def fig_model_m4() -> str:
    """One node's links under M4: direction gone, only who opened a link remains."""
    W, H = 860, 330
    b = []
    col = SERIES["M4"]
    _model_scaffold(b, "Its picks",
                    "this node initiated these; messages flow both ways",
                    "Their picks",
                    "opened by others toward it; messages flow both ways")
    _model_layer_row(b, 72, [2, 6, 10, 14], col)
    for i in [2, 6, 10, 14]:
        _model_link(b, i, True, col, both=True)
    _model_layer_row(b, 272, None, None, quietfill=[4, 9, 13])
    for i in [4, 9, 13]:
        _model_link(b, i, False, "#8a887e", both=True)
    return frame(W, H, b, "One node's links under M4",
                 "The upstream and downstream layers are gone; what remains is who "
                 "opened each link. The top row holds the RF peers the current node "
                 "drew, the bottom row the peers whose draws landed on it, and every "
                 "arrow points both ways because a link carries messages in both "
                 "directions whichever end opened it.")


# ------------------------------------------------------------------ handshake
def fig_handshake() -> str:
    """The handshake as a sequence: one request, an ordered evaluation, one reply.

    A structural diagram like fig_architecture, so its content is literal.
    The seven checks are numbered to match the Specification's list, because
    the order is normative: it decides what a refusal reveals to a prober.
    What the figure carries that the list cannot is the shape of the exchange
    -- that five of the seven exits are silent, and only the last two ever put
    a message back on the wire.
    """
    W, H = 860, 470
    b = []
    ok = "#1e8f5e"
    no = SERIES["M5"]
    quiet = "#8a887e"
    DX, AX = 120, 610

    for x, lab in ((DX, "Dialler"), (AX, "Acceptor")):
        b.append(rect(x - 80, 34, 160, 36, SURFACE, RULE, 1.4))
        b.append(text(x, 57, lab, 12.5, INK, "middle", "600"))
        b.append(line(x, 70, x, H - 22, GRID, 1.2, dash="4 5"))

    b.append(arrow(DX + 2, 102, AX - 4, 102, INK, 1.6))
    b.append(text((DX + AX) / 2, 94, "Request   topic T, link kind, epoch e, signed",
                  10.5, INK_SOFT, "middle"))

    rows = [("Kind", "dropped, no reply", quiet),
            ("Signature", "dropped, no reply", quiet),
            ("Epoch", "dropped, no reply", quiet),
            ("Membership", "dropped, no reply", quiet),
            ("Already held", "Accepted again, idempotent", ok),
            ("Gate", "dropped, no reply", quiet),
            ("Cap", "crossing always completes", ok)]
    b.append(rect(410, 122, 400, 228, SURFACE, RULE, 1.4, rx=8))
    b.append(text(430, 142, "Evaluated in this order", 11, INK, weight="600"))
    for i, (name, exit_, col) in enumerate(rows):
        y = 168 + i * 26
        b.append(circle(434, y - 4, 8.5, INK, INK, 0))
        b.append(text(434, y, str(i + 1), 9.5, SURFACE, "middle", "700"))
        b.append(text(450, y, name, 11, INK_SOFT, weight="600"))
        b.append(text(794, y, exit_, 9.5, col, "end"))

    b.append(arrow(AX - 4, 386, DX + 2, 386, ok, 1.6))
    b.append(text((DX + AX) / 2, 378, "Accepted   the link stands for this epoch",
                  10.5, ok, "middle", "600"))
    b.append(arrow(AX - 4, 424, DX + 2, 424, no, 1.6))
    b.append(text((DX + AX) / 2, 416, "Rejected   the admissions budget is spent",
                  10.5, no, "middle", "600"))
    b.append(text(DX - 80, 452,
                  "A dialler that is rejected does not retry that peer this epoch.",
                  9.5, quiet))

    return frame(W, H, b, "Establishing one link",
                 "A sequence diagram in two lanes. The dialler sends one signed Request "
                 "to the acceptor. The acceptor evaluates seven checks in a fixed order: "
                 "kind, signature, epoch, membership, already held, gate, cap. Failing "
                 "the first four or the gate is dropped without a reply. An already held "
                 "link is accepted again. At the cap a crossing still completes. The "
                 "acceptor replies Accepted, or Rejected once its admissions budget is "
                 "spent.")


# ------------------------------------------------------------------ figure 2
def fig_derivation() -> str:
    """One node's links for one epoch: three rows of markers over the same peers.

    Structural, like Figure 1, and deliberately only the selection. The headroom
    arithmetic and the acceptor's checks were boxed text inside the drawing,
    which is markdown's job, not SVG's - they live in the prose around it now.
    The counts are a miniature at exactly the sizing rule the Specification
    fixes: 32 registered peers, B = 4, so 8 eligible, k = 4 picked, r = 2.

    The rows are numbered so the Specification's subsections can name which one
    they describe, and the two arrows carry B and r because those quantities are
    what the transitions between the rows are.
    """
    W, H = 860, 272
    b = []
    verifiable = SERIES["M2"]
    private = "#1e8f5e"

    n, rf, buckets = 32, 4, 4
    eligible = [i for i in range(n) if i % buckets == 1]       # 8 of the 32
    picks = eligible[:: len(eligible) // rf][:rf]              # 4 of the 8

    x0, x1 = 300, 812
    step = (x1 - x0) / (n - 1)

    rows = [
        (76, "Registered peers", f"N_{{T}} \u2212 1 = {n}"),
        (150, "Eligible peers", f"\u2248 (N_{{T}} \u2212 1)/B = {len(eligible)}"),
        (224, "Picks", f"k = {rf}"),
    ]
    # the two transitions are where B and r live, so the arrows carry them
    steps = [f"gate, B = {buckets}", f"headroom r = {n / (buckets * rf):g}"]
    for k, (y, head, count) in enumerate(rows):
        col = private if k == 2 else (verifiable if k == 1 else INK_SOFT)
        b.append(circle(48, y - 8, 11, INK))
        b.append(text(48, y - 4, str(k + 1), 11.5, SURFACE, "middle", "700"))
        b.append(text(66, y - 4, head, 12.5, INK, weight="600"))
        b.append(text(66, y + 16, count, 10.5, col, weight="600"))
        for i in range(n):
            cx = x0 + i * step
            if k == 0:
                b.append(circle(cx, y, 4.6, SURFACE, RULE, 1.5))
            elif k == 1:
                b.append(circle(cx, y, 4.6, verifiable if i in eligible else SURFACE,
                                SURFACE if i in eligible else GRID, 1.5))
            else:
                if i in picks:
                    b.append(circle(cx, y, 4.6, private, SURFACE, 1.5))
                elif i in eligible:
                    b.append(circle(cx, y, 4.6, SURFACE, verifiable, 1.5))
                else:
                    b.append(circle(cx, y, 4.6, SURFACE, GRID, 1.5))
        if k < 2:
            xm = x0 + (x1 - x0) / 2
            b.append(arrow(xm, y + 20, xm, y + 50, RULE, 1.4))
            b.append(text(xm + 12, y + 40, steps[k], 10, "#8a887e"))

    return frame(W, H, b, "Deriving one node's links for one epoch",
                 "Three rows of markers over the same peers. The first row is every peer "
                 "registered on the topic at the epoch's registration cutoff. The second "
                 "marks those for which the verifiable gate holds, roughly one in B of "
                 "them. The third marks the k the node actually picks from that eligible "
                 "set, drawn with its own randomness. The first two rows are publicly "
                 "recomputable; the third is the node's own draw and is private.")


# ------------------------------------------------------------------ figure 4
def fig_validation(cells, churn=()) -> str:
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

    # A point is not expected to sit on the diagonal: a finite sample scatters
    # around the rate it is drawn from. The band is that scatter at two standard
    # errors for a 4 000-draw sample, which is both the median and the most
    # common size here. Each point's own bar is exact for its own sample; the
    # band only gives the eye a scale, and points from larger samples should sit
    # well inside it while the few small ones may fall outside.
    BAND_N = 4000
    steps = 90
    upper, lower = [], []
    for i in range(steps + 1):
        v = 10 ** (lg(lo) + (lg(hi) - lg(lo)) * i / steps)
        se = math.sqrt(max(v * (1 - v), 0.0) / BAND_N)
        upper.append((X(v), Y(min(v + 2 * se, hi))))
        lower.append((X(v), Y(max(v - 2 * se, lo))))
    pts = " ".join(f"{x:.1f},{y:.1f}" for x, y in upper + lower[::-1])
    b.append(f'<polygon points="{pts}" fill="{RULE}" fill-opacity="0.20" stroke="none"/>')

    b.append(line(X(lo), Y(lo), X(hi), Y(hi), RULE, 1.8, cap="round"))

    everything = list(cells) + [c for c in churn if lo <= c["law"] <= hi]
    for c in everything:
        x = X(c["law"])
        wl, wh = wilson(c["bad"], c["runs"])
        b.append(line(x, Y(max(wl, lo)), x, Y(wh), SERIES[c["model"]], 2.0,
                      cap="round", opacity=0.45))
    # hollow marks the cells run under churn, so the two claims stay separable
    for c in everything:
        churned = c.get("churn_pct", 0) > 0
        b.append(circle(X(c["law"]), Y(c["bad"] / c["runs"]), 4.4,
                        SURFACE if churned else SERIES[c["model"]],
                        SERIES[c["model"]] if churned else SURFACE, 1.8 if churned else 1.6))

    b.append(text(ml + pw / 2, H - 34, "p_{bad} predicted by the coverage law",
                  12.5, INK, "middle", "600"))
    b.append(text(ml + pw / 2, H - 18,
                  "log scale, each gridline ×10; left: almost never fails · "
                  "right: fails most epochs", 11, INK_SOFT, "middle"))
    b.append(f'<text x="0" y="0" transform="translate(22,{mt + ph / 2:.1f}) rotate(-90)" '
             f'text-anchor="middle" font-size="12.5" font-weight="600" fill="{INK}" '
             f'font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif" '
             f'xml:space="preserve">{runs("p_{bad} measured")}</text>')
    b.append(f'<text x="0" y="0" transform="translate(37,{mt + ph / 2:.1f}) rotate(-90)" '
             f'text-anchor="middle" font-size="11" fill="{INK_SOFT}" '
             f'font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif">'
             f'bar = the range consistent with that count</text>')

    lx = ml + 14
    b.append(text(lx, mt + 20, "one point = one tested configuration:", 11.5, INK_SOFT))
    b.append(text(lx, mt + 38, "grey line = law matched measurement exactly · "
                  "band = scatter expected of a 4 000-draw sample", 11, "#8a887e"))
    b.append(text(lx, mt + 54, "hollow = measured under honest downtime",
                  11, "#8a887e"))
    for i, (m, col) in enumerate(SERIES.items()):
        cx = lx + 218 + i * 52
        b.append(circle(cx, mt + 16, 4.4, col, SURFACE, 1.6))
        b.append(text(cx + 9, mt + 20, m, 11.5, INK_SOFT))

    return frame(W, H, b, "Measured against predicted epoch failure probability",
                 "Each point is one experiment configuration. Horizontal position is the "
                 "probability predicted by the closed-form coverage law, vertical position "
                 "the fraction of sampled topologies that actually failed. Each bar is the "
                 "range of true rates consistent with that count, and the shaded band is "
                 "the scatter a 4 000-draw sample is expected to show. Points lie along "
                 "the diagonal across the whole range.",
                 conditions="N = 4 000 and 20 000 · μ = 0.2")


# ------------------------------------------------------------------ figure 6
def fig_tradeoffs(ops, alternatives=()) -> str:
    """Single overlaid radar: the four designs that are each best at something.

    Overlay rather than small multiples, because side-by-side comparison is the
    whole point of the form. That caps the series count: four filled polygons in
    the models' own hues cannot be separated (magenta against orange falls under
    the normal-vision floor), while M3/M4/M5 in theirs clears every gate. M5's
    aqua-magenta pair sits in the colour-vision warn band, so each polygon also
    carries a direct label at the vertex where it reaches the outer ring — which
    doubles as naming what that design is best at.

    M2 and M1 are left out and covered in prose: M1 is dominated outright, and
    M2 reaches the frontier only on a latency margin the evidence calls
    non-discriminating.

    Each axis is scored best/this, so the best design reaches the outer ring and
    every axis reads outward-is-better.
    """
    import math as _m
    W, H = 860, 616
    cx, cy, R = 430, 282, 162
    by = {o["model"]: o for o in ops}
    # where a design has a preferred alternative to its published point, plot the
    # one this proposal actually recommends
    for a in alternatives:
        if a.get("preferred"):
            by[a["model"]] = a
    # M5 and M1 are dominated once each design is allowed its best known
    # parameters; M2 stays as the conservative reference. The dominated pair is
    # still drawn, muted, so the reader can see that they are inside the others
    # everywhere rather than having to take the claim on trust.
    #
    # They are muted by hue and by a faint fill rather than by a dash pattern:
    # the churn axis is dashed to mark that it is law-derived, and a dashed
    # series against a dashed axis makes the reader guess which meaning applies.
    # Dashing therefore says one thing in this figure. The two neutrals separate
    # the pair by lightness alone, which is what keeps them out of the four
    # categorical hues; M1 lies inside M5 on three axes and on top of it on the
    # fourth, so the nesting itself distinguishes them.
    SHOWN = ["M3", "M4", "M2"]
    MUTED = [("M5", "#8a887e"), ("M1", "#bcb9ae")]

    AXES = [
        ("Bandwidth economy", "copies per honest node c", lambda o: o["copies_per_node"], True),
        ("Connection economy", "standing links per node d", lambda o: o["standing_links"], True),
        ("Speed", "hops to full coverage h_{full}", lambda o: o["hops_full"], True),
        ("Churn tolerance", "churn budget p_{max}", lambda o: o["churn_budget_pct"], False),
    ]
    # the design leading each axis is whichever of the shown set is best on it,
    # rather than a fixed assignment that goes stale when the set changes
    OWNER = [min(SHOWN, key=lambda m: get(by[m])) if low
             else max(SHOWN, key=lambda m: get(by[m]))
             for _, _, get, low in AXES]
    best = [(min if low else max)(get(by[m]) for m in SHOWN)
            for _, _, get, low in AXES]

    def score(m, i):
        _, _, get, low = AXES[i]
        v = get(by[m])
        return (best[i] / v) if low else (v / best[i])

    ang = [-_m.pi / 2, 0.0, _m.pi / 2, _m.pi]

    b = []

    for ring in (0.25, 0.5, 0.75, 1.0):
        pts = " ".join(f"{cx + R * ring * _m.cos(a):.1f},{cy + R * ring * _m.sin(a):.1f}"
                       for a in ang)
        b.append(f'<polygon points="{pts}" fill="none" stroke="{GRID}" stroke-width="1"/>')
    for i, a in enumerate(ang):
        dash = "4 4" if i == 3 else None          # law-derived, not sampled
        b.append(line(cx, cy, cx + R * _m.cos(a), cy + R * _m.sin(a),
                      "#c4c2b9" if i == 3 else GRID, 1, dash=dash))

    # the dominated pair first, so the contending designs draw over them
    for m, col in MUTED:
        pts = " ".join(
            f"{cx + R * score(m, i) * _m.cos(ang[i]):.1f},"
            f"{cy + R * score(m, i) * _m.sin(ang[i]):.1f}" for i in range(4))
        b.append(f'<polygon points="{pts}" fill="{col}" fill-opacity="0.07" stroke="{col}" '
                 f'stroke-width="1.4" stroke-linejoin="round"/>')

    b.append(text(38, 30, "dominated on all four axes, drawn for reference:",
                  10.5, "#8a887e"))
    for k, (m, col) in enumerate(MUTED):
        y = 48 + k * 18
        b.append(f'<rect x="38" y="{y - 12:.1f}" width="28" height="11" rx="2" '
                 f'fill="{col}" fill-opacity="0.07" stroke="{col}" stroke-width="1.4"/>')
        b.append(text(73, y, f"{m} · {by[m]['params']}", 10.5, "#8a887e"))

    # p_bad is not a spoke: churn tolerance is the same coverage law read as
    # margin, so plotting both would double-count. But the three designs differ
    # by more than an order of magnitude in it, and a normalised radar hides
    # that, so the rate each shape is drawn at is stated outright.
    def _sci(v):
        mant, exp = f"{v:.1e}".split("e")
        return f"{mant} × 10{str(int(exp)).translate(SUPERS)}"
    pb = " · ".join(f"{m} {_sci(by[m]['p_bad'])}" for m in SHOWN)
    b.append(text(38, 96, "epoch failure probability these costs are read at:",
                  10.5, "#8a887e"))
    b.append(text(38, 112, pb, 10.5, INK_SOFT, weight="600"))

    for m in SHOWN:
        col = SERIES[m]
        pts = " ".join(
            f"{cx + R * score(m, i) * _m.cos(ang[i]):.1f},"
            f"{cy + R * score(m, i) * _m.sin(ang[i]):.1f}" for i in range(4))
        b.append(f'<polygon points="{pts}" fill="{col}" fill-opacity="0.13" stroke="{col}" '
                 f'stroke-width="2.4" stroke-linejoin="round"/>')
        for i in range(4):
            s = score(m, i)
            b.append(circle(cx + R * s * _m.cos(ang[i]), cy + R * s * _m.sin(ang[i]),
                            4.2, col, SURFACE, 1.6))

    CAP = {m: f"{m} · {by[m]['params']}" for m in SHOWN}
    for i, (name, unit, get, _low) in enumerate(AXES):
        owner = OWNER[i]
        vx, vy = cx + R * _m.cos(ang[i]), cy + R * _m.sin(ang[i])
        vals = " · ".join(f"{m} {get(by[m])}" for m in SHOWN)
        if i == 0:
            if owner:
                b.append(text(vx, vy - 22, CAP[owner], 12.5, SERIES[owner], "middle", "650"))
            b.append(text(vx, vy - 58, name, 12.5, INK, "middle", "600"))
            b.append(text(vx, vy - 43, unit, 10.5, "#8a887e", "middle"))
        elif i == 2:
            if owner:
                b.append(text(vx, vy + 26, CAP[owner], 12.5, SERIES[owner], "middle", "650"))
            b.append(text(vx, vy + 48, name, 12.5, INK, "middle", "600"))
            b.append(text(vx, vy + 63, unit, 10.5, "#8a887e", "middle"))
            b.append(text(vx, vy + 77, vals, 10.5, "#8a887e", "middle"))
            continue
        else:
            anchor = "start" if i == 1 else "end"
            dx = 20 if i == 1 else -20
            if owner:
                b.append(text(vx + dx, vy - 22, CAP[owner], 12.5, SERIES[owner], anchor, "650"))
            b.append(text(vx + dx, vy - 4, name, 12.5, INK, anchor, "600"))
            b.append(text(vx + dx, vy + 11, unit, 10.5, "#8a887e", anchor))
            b.append(text(vx + dx, vy + 25, vals, 10.5, "#8a887e", anchor))
            # the one dashed thing in the figure, named where it is drawn
            if i == 3:
                b.append(text(vx + dx, vy + 41, "dashed axis: read off the law, "
                              "not sampled", 10, "#8a887e", anchor))
            continue
        b.append(text(vx, vy - 72, vals, 10.5, "#8a887e", "middle"))

    # the four quantities are named at their own axes, so the orientation note
    # does not enumerate them again - it ran off the canvas when it did
    b.append(text(38, 556, "Every axis is oriented so that outward is better.",
                  11, INK_SOFT, style="italic"))
    b.append(text(38, 572, "Each design is scored against the best of the three and "
                  "labelled at the axis where it leads.", 11, INK_SOFT, style="italic"))
    b.append(text(38, 588, "Three axes are measured directly; the dashed one, churn "
                  "tolerance, is read off the coverage law, whose behaviour under churn "
                  "was measured separately.", 11, INK_SOFT, style="italic"))
    b.append(text(38, 604, "Radial position is the ratio to the best value on that axis, "
                  "so half-way out is half as good; the centre is zero downtime, or "
                  "unbounded cost.", 11, INK_SOFT, style="italic"))

    return frame(W, H, b, "Four-way trade-off across the surviving designs",
                 "One radar chart overlaying M2, M3 and M4 on four axes: bandwidth "
                 "economy, connection economy, speed and churn tolerance, all oriented "
                 "outward-is-better. Each is labelled at the axis where it reaches the "
                 "outer ring. M4 reaches it on connection economy and on churn tolerance, "
                 "and is the most even shape; M3 reaches it on bandwidth alone and sits "
                 "under a third of the way out on churn tolerance; M2 reaches it on speed "
                 "and is innermost on the other three. M5 and M1 are drawn as muted grey "
                 "shapes with a faint fill and a solid outline, M1 nested inside M5: each "
                 "lies inside a contending design on every axis, which is what being "
                 "dominated looks like. The churn axis is the only dashed line in the "
                 "figure, marking that it is read off the coverage law rather than "
                 "sampled.",
                 conditions="N = 20 000 · μ = 0.2 · δ = 10⁻⁴")


# ------------------------------------------------------------------ figure 7
def fig_extrapolation(cells, ops, alternatives=()) -> str:
    """Where the measured configurations sit relative to the proposed ones.

    Substantiates the first entry in "Limits of this evidence": sampling can
    only resolve failure rates it can observe, so every configuration that was
    measured fails far more often than any configuration that is proposed. Solid
    marks are counted outcomes; hollow marks are law predictions.

    The hollow mark is the configuration this proposal names, which for M3 and
    M4 is the preferred split rather than the published one - the same point the
    trade-off radar and the two-candidate tables carry. Reading the published
    split here would label a configuration "proposed" that the text argues
    against.
    """
    W, H = 860, 460
    ml, mr, mt, mb = 104, 34, 88, 108
    pw, ph = W - ml - mr, H - mt - mb
    lo, hi = 1e-5, 1.4
    lg = math.log10

    def X(v):
        return ml + (lg(max(v, lo)) - lg(lo)) / (lg(hi) - lg(lo)) * pw

    by = {o["model"]: o for o in ops}
    by.update({a["model"]: a for a in alternatives if a.get("preferred")})
    order = [o["model"] for o in sorted(by.values(), key=lambda o: o["copies_per_node"])]
    rows = {m: [c["bad"] / c["runs"] for c in cells if c["model"] == m] for m in order}
    step = ph / len(order)

    b = []
    # A per-epoch probability is readable as a frequency without assuming any
    # epoch duration, which is still an open question: 1e-4 is one bad epoch in
    # ten thousand, whatever an epoch turns out to be.
    for e in range(-5, 1):
        v = 10.0 ** e
        b.append(line(X(v), mt - 8, X(v), mt + ph, GRID, 1))
        b.append(text(X(v), mt + ph + 20, decade(v), anchor="middle"))
        rate = "every epoch" if e == 0 else f"1 in {10 ** -e:,}"
        b.append(text(X(v), mt + ph + 34, rate, 9.5, "#8a887e", "middle"))

    xt = X(1e-4)
    b.append(line(xt, mt - 22, xt, mt + ph, "#52514e", 1.4))
    b.append(text(xt, mt - 42, "design target", 11, INK, "middle", "600"))
    b.append(text(xt, mt - 29, "δ = 10⁻⁴, one bad epoch in ten thousand", 9.5,
                  "#8a887e", "middle"))
    b.append(text(xt, mt - 17, "the rate a configuration is sized to meet", 9.5,
                  "#8a887e", "middle"))

    for k, m in enumerate(order):
        y = mt + step * (k + 0.5)
        col = SERIES[m]
        ps = rows[m]
        opv = by[m]["p_bad"]
        b.append(text(ml - 14, y + 4, f"{m} · {by[m]['params']}", 11.5, INK, "end", "600"))
        b.append(line(X(opv), y, X(min(ps)), y, "#b9b6ab", 1.3, dash="4 4"))
        b.append(line(X(min(ps)), y, X(max(ps)), y, col, 3.4, cap="round", opacity=0.32))
        for v in ps:
            b.append(circle(X(v), y, 4.2, col, SURFACE, 1.5))
        b.append(circle(X(opv), y, 5.4, SURFACE, col, 2.2))
        # A ratio of two per-epoch probabilities, so epoch length cancels.
        # Stated as a factor rather than in decades, which reads as a duration
        # in a document whose central quantity is measured per epoch.
        gap = min(ps) / opv
        b.append(text((X(opv) + X(min(ps))) / 2, y - 11, f"{gap:.0f}\u00d7 rarer",
                      9.5, "#8a887e", "middle"))

    b.append(text(ml + pw / 2, H - 44, "p_{bad}: chance an epoch's wiring fails",
                  12.5, INK, "middle", "600"))
    b.append(text(ml + pw / 2, H - 29, "log scale, each gridline ×10",
                  11, INK_SOFT, "middle"))

    ly = H - 12
    b.append(circle(ml + 6, ly - 4, 4.2, INK_SOFT, SURFACE, 1.5))
    b.append(text(ml + 16, ly, "a configuration that was measured", 11, INK_SOFT))
    b.append(circle(ml + 232, ly - 4, 5.4, SURFACE, INK_SOFT, 2.2))
    b.append(text(ml + 243, ly, "the configuration this proposal uses: predicted by "
                  "the law, too rare to sample", 11, INK_SOFT))

    return frame(W, H, b, "Measured configurations against proposed ones",
                 "For each design, the failure rates of the configurations that were "
                 "measured, and the far lower rate of the configuration actually "
                 "proposed. The two are separated by about two orders of magnitude, "
                 "spanned by the coverage laws rather than by measurement.",
                 conditions="N = 20 000 · μ = 0.2 · δ = 10⁻⁴")


# ------------------------------------------------------------------ figure 8
def fig_gate_tradeoff(g) -> str:
    """The bucket count's two opposing costs, on one shared axis.

    Two stacked panels rather than two y-scales on one plot: the quantities are
    a probability and a slot count, and putting them on a shared vertical axis
    would invent a comparison that does not exist. They share the horizontal
    axis, which is what makes the optimum legible.
    """
    W, H = 860, 560
    ml, mr = 96, 40
    pw = W - ml - mr
    top, ph1 = 76, 190          # coverage panel
    bot, ph2 = 342, 128         # concentration panel
    cells = g["cells"]
    lg = math.log10
    x0, x1 = 8.0, 620.0

    def X(v):
        return ml + (lg(v) - lg(x0)) / (lg(x1) - lg(x0)) * pw

    lo1, hi1 = 4e-3, 1.4
    def Y1(v):
        return top + ph1 - (lg(max(v, lo1)) - lg(lo1)) / (lg(hi1) - lg(lo1)) * ph1
    def Y2(v):
        return bot + ph2 - (v / 22.0) * ph2

    b = []
    rec = next(c for c in cells if c.get("recommended"))
    # the region where the gate leaves enough eligible peers for the pick count
    b.append(f'<rect x="{ml:.1f}" y="{top:.1f}" width="{X(rec["B"]) - ml:.1f}" '
             f'height="{ph1 + (bot - top - ph1) + ph2:.1f}" fill="#1e8f5e" opacity="0.045"/>')

    for c in cells:
        b.append(line(X(c["B"]), top, X(c["B"]), bot + ph2, GRID, 1))
        b.append(text(X(c["B"]), bot + ph2 + 19, c["B"], 10.5, INK_SOFT, "middle"))
        b.append(text(X(c["B"]), bot + ph2 + 32, f"r={c['r']:g}", 9.5, "#8a887e", "middle"))
    for v in (1e-2, 1e-1, 1.0):
        b.append(line(ml, Y1(v), ml + pw, Y1(v), GRID, 1))
        b.append(text(ml - 10, Y1(v) + 4, decade(v), 10.5, INK_SOFT, "end"))
    for v in (0, 5, 10, 20):
        b.append(line(ml, Y2(v), ml + pw, Y2(v), GRID, 1))
        b.append(text(ml - 10, Y2(v) + 4, v, 10.5, INK_SOFT, "end"))

    # after the gridlines, so they are not painted over
    b.append(text(ml + 8, top + 16, "selection headroom r \u2265 2", 11, "#1e8f5e", weight="600"))
    b.append(text(ml + 8, top + 30, "the gate still leaves each node at least twice the "
                  "eligible peers it must pick from", 9.5, "#8a887e"))

    lawy = Y1(g["law"])
    b.append(line(ml, lawy, ml + pw, lawy, "#52514e", 1.4, dash="5 4"))
    b.append(text(ml + pw - 4, lawy - 7, "coverage law, ungated", 10.5, INK, "end"))

    pts = [(X(c["B"]), Y1(c["bad"] / c["runs"])) for c in cells]
    b.append(f'<path d="{" ".join(("M" if i == 0 else "L") + f"{x:.1f} {y:.1f}" for i, (x, y) in enumerate(pts))}" '
             f'fill="none" stroke="{SERIES["M2"]}" stroke-width="2.2" stroke-linejoin="round"/>')
    for c, (x, y) in zip(cells, pts):
        b.append(line(x, Y1(c["lo"]), x, Y1(c["hi"]), SERIES["M2"], 2.0, cap="round", opacity=0.45))
        b.append(circle(x, y, 4.4, SERIES["M2"], SURFACE, 1.6))

    cpts = [(X(c["B"]), Y2(g["sybils_for_concentration"] / c["B"])) for c in cells]
    b.append(f'<path d="{" ".join(("M" if i == 0 else "L") + f"{x:.1f} {y:.1f}" for i, (x, y) in enumerate(cpts))}" '
             f'fill="none" stroke="{SERIES["M4"]}" stroke-width="2.2" stroke-linejoin="round"/>')
    # filled where E12 measured the concentration, hollow where K/B is predicted only
    for c, (x, y) in zip(cells, cpts):
        if c.get("concentration_measured"):
            b.append(circle(x, y, 4.4, SERIES["M4"], SURFACE, 1.6))
        else:
            b.append(circle(x, y, 4.0, SURFACE, SERIES["M4"], 1.8))

    b.append(circle(X(rec["B"]), Y1(rec["bad"] / rec["runs"]), 8.5, "none", "#1e8f5e", 2.0))
    b.append(circle(X(rec["B"]), Y2(g["sybils_for_concentration"] / rec["B"]), 8.5, "none", "#1e8f5e", 2.0))
    b.append(text(X(rec["B"]) + 14, Y2(g["sybils_for_concentration"] / rec["B"]) + 4,
                  f"B = {rec['B']}, recommended", 12, "#1e8f5e", weight="650"))

    b.append(text(ml, top - 34, "What the gate costs in coverage", 12.5, INK, weight="600"))
    b.append(text(ml, top - 19, "p_{bad} measured, with Wilson 95 % intervals \u00b7 log scale "
                  "\u00b7 lower is better", 10.5, "#8a887e"))
    b.append(text(ml, bot - 20, "What the gate buys against a flooder", 12.5, INK, weight="600"))
    b.append(text(ml, bot - 5, "slots one victim gives an attacker holding 5 % of the network "
                  "\u00b7 lower is better", 10.5, "#8a887e"))
    b.append(text(ml + pw - 4, bot + 12, "filled = measured \u00b7 hollow = predicted A/B",
                  9.5, "#8a887e", "end"))
    b.append(text(ml + pw / 2, H - 42, "Bucket count B: how many groups the gate splits the population into", 12.5, INK, "middle", "600"))
    b.append(text(ml + pw / 2, H - 27,
                  "right = a narrower gate: fewer eligible peers per node, and the attacker's "
                  "pressure divided further", 11, INK_SOFT, "middle"))
    b.append(text(38, H - 8, "Both panels share the horizontal axis. The largest B leaving "
                  "headroom for the pick count is coverage-exact and dilutes the attacker "
                  "most.", 11, INK_SOFT, style="italic"))

    return frame(W, H, b, "The bucket count trade-off",
                 "Two stacked panels sharing a bucket-count axis, where selection headroom r "
                 "is the number of peers the gate leaves a node eligible to link to, "
                 "divided by the number it must pick. Coverage stays on the "
                 "ungated law while selection headroom is at least 2, then rises fivefold at "
                 "headroom 1 and collapses below it. Attacker concentration falls as the "
                 "reciprocal of the bucket count throughout, measured at four of the seven "
                 "bucket counts and predicted at the rest. The largest bucket count "
                 "retaining headroom is best on both.",
                 conditions="N = 4 000 · μ = 0.2 · pick count 16")


# ------------------------------------------------------------------ figure 9


def fig_bucket_bounds(g) -> str:
    """What each sizing rule for the bucket count actually delivers.

    The Specification bounds the bucket count three ways and takes the smallest.
    Drawing the three bounds themselves is unilluminating — they are all very
    nearly proportional to the topic size, so on log axes they are parallel
    lines a reader cannot separate. What the proposal actually claims is about
    consequences, so that is what this plots: the failure probability each rule
    arrives at. The headroom floor alone was the rule earlier drafts carried,
    and above a few thousand participants it stops meeting the target at all.
    """
    W, H = 860, 456
    ml, mr = 92, 190
    top, ph = 46, 300
    pw = W - ml - mr
    lg = math.log10
    curves = g["curves"]
    marks = g["marks"]
    delta = g["delta"]
    x0, x1 = 1000.0, 50000.0
    y0, y1 = 1e-5, 1e-1

    def X(v):
        return ml + (lg(v) - lg(x0)) / (lg(x1) - lg(x0)) * pw

    def Y(v):
        v = min(max(v, y0), y1)
        return top + ph - (lg(v) - lg(y0)) / (lg(y1) - lg(y0)) * ph

    b = []
    # the region that misses the target, named once rather than per curve
    b.append(f'<rect x="{ml}" y="{top}" width="{pw}" height="{Y(delta) - top:.1f}" '
             f'fill="#b23b3b" fill-opacity="0.05"/>')

    for e in range(-5, 0):
        v = 10.0 ** e
        b.append(line(ml, Y(v), ml + pw, Y(v), GRID))
        b.append(text(ml - 10, Y(v) + 4, decade(v), 10.5, INK_SOFT, "end"))
    for v in (1000, 2000, 5000, 10000, 20000, 50000):
        b.append(line(X(v), top, X(v), top + ph, GRID))
        b.append(text(X(v), top + ph + 18, f"{v:,}".replace(",", "\u2009"), 10.5,
                      INK_SOFT, "middle"))

    # after the gridlines, for the same reason
    b.append(text(ml + 10, top + 16, "misses the target", 10.5, "#b23b3b"))
    b.append(line(ml, top + ph, ml + pw, top + ph, RULE, 1.2))
    b.append(text(ml + pw / 2, top + ph + 40, "participants on the topic, N_{T}",
                  12.5, INK, "middle", "600"))
    b.append(text(ml + pw / 2, top + ph + 56,
                  "log scale, each gridline \u00d710", 11, INK_SOFT, "middle"))
    b.append(f'<text x="0" y="0" transform="translate(24,{top + ph / 2:.1f}) rotate(-90)" '
             f'text-anchor="middle" font-size="12.5" font-weight="600" fill="{INK}" '
             f'font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif" '
             f'xml:space="preserve">{runs("p_{bad} per epoch")}</text>')
    b.append(f'<text x="0" y="0" transform="translate(39,{top + ph / 2:.1f}) rotate(-90)" '
             f'text-anchor="middle" font-size="11" fill="{INK_SOFT}" '
             f'font-family="-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif">'
             f'lower is better</text>')

    # the target itself
    b.append(line(ml, Y(delta), ml + pw, Y(delta), "#b23b3b", 1.6, dash="5 4"))

    def path(key, colour, width, dash=None):
        pts = " ".join(f"{'M' if i == 0 else 'L'}{X(c['n']):.1f} {Y(c[key]):.1f}"
                       for i, c in enumerate(curves))
        da = f' stroke-dasharray="{dash}"' if dash else ""
        return (f'<path d="{pts}" fill="none" stroke="{colour}" '
                f'stroke-width="{width}"{da} stroke-linejoin="round"/>')

    b.append(path("pbad_headroom_k9", SERIES["M1"], 1.6, "5 4"))
    b.append(path("pbad_headroom_k10", SERIES["M1"], 2.2))
    b.append(path("pbad_rule_k9", SERIES["M5"], 1.6, "5 4"))
    b.append(path("pbad_rule_k10", SERIES["M5"], 2.2))

    n0 = g["reference_n"]
    m9 = marks["k9"]
    b.append(line(X(n0), top, X(n0), top + ph, RULE, 1.0, dash="3 3"))
    b.append(circle(X(n0), Y(m9["headroom_pbad"]), 4.5, SERIES["M1"], SURFACE, 2.0))
    b.append(circle(X(n0), Y(m9["chosen_pbad"]), 4.5, SERIES["M5"], SURFACE, 2.0))

    lx = ml + pw + 18
    ly = top + 10
    def key(colour, label, sub):
        nonlocal ly
        b.append(f'<line x1="{lx}" y1="{ly}" x2="{lx + 20}" y2="{ly}" '
                 f'stroke="{colour}" stroke-width="2.2"/>')
        b.append(text(lx + 26, ly + 4, label, 10.5, INK))
        ly += 15
        b.append(text(lx, ly + 4, sub, 9.5, INK_SOFT))
        ly += 26

    key(SERIES["M5"], "the rule", "smallest of the three bounds")
    key(SERIES["M1"], "headroom alone", "the retired one-line rule")
    b.append(f'<line x1="{lx}" y1="{ly}" x2="{lx + 20}" y2="{ly}" '
             f'stroke="#b23b3b" stroke-width="1.6" stroke-dasharray="5 4"/>')
    b.append(text(lx + 26, ly + 4, "target \u03b4", 10.5, INK))
    ly += 24
    b.append(text(lx, ly + 4, "solid: k = 10 (specified)", 9.5, INK_SOFT))
    b.append(text(lx, ly + 17, "dashed: k = 9", 9.5, INK_SOFT))

    # decade() would round this to 10^-2; the proposal quotes the value itself
    def sci(v):
        e = int(math.floor(math.log10(v)))
        return f"{v / 10 ** e:.1f}\u2009\u00d7\u200910{str(e).translate(SUPERS)}"

    b.append(text(X(n0) - 12, Y(m9["headroom_pbad"]) - 11,
                  f"{sci(m9['headroom_pbad'])} at k = 9", 10.5, SERIES["M1"], "end"))
    b.append(text(X(n0) - 12, Y(m9["headroom_pbad"]) + 3,
                  "ninety-two times the target", 9.5, INK_SOFT, "end"))

    return frame(
        W, H, b,
        "What each bucket-count rule delivers, against topic size",
        "The failure probability reached by two ways of sizing the bucket "
        "count. Taking the smallest of the three bounds the Specification "
        "states holds the failure probability at the target across the whole "
        "range. Reading only the selection-headroom floor, as earlier drafts "
        "did, misses the target above a few thousand participants, and at "
        "twenty thousand participants with nine picks it misses it by roughly "
        "two orders of magnitude.",
        conditions=f"\u03bc = {g['mu']}, \u03b4 = {decade(g['delta'])}, gated M4 laws",
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true",
                    help="fail if the committed SVGs differ from freshly generated ones")
    args = ap.parse_args()

    d = json.loads(DATA.read_text())
    figs = {
        # Structural diagrams: no cells.json data behind them, but kept here so
        # that --check covers every figure the CIP carries.
        "architecture.svg": fig_architecture(),
        "derivation.svg": fig_derivation(),
        "coverage-validation.svg": fig_validation(
            d["coverage_cells"], d.get("churn_cells", ())),
        "gate-tradeoff.svg": fig_gate_tradeoff(d["gate_tradeoff"]),
        "bucket-bounds.svg": fig_bucket_bounds(d["bucket_bounds"]),
        "tradeoff-radar.svg": fig_tradeoffs(
            d["operating_points"], d.get("alternatives", ())),
        "model-m1.svg": fig_model_m1(),
        "model-m2.svg": fig_model_m2(),
        "model-m3.svg": fig_model_m3(),
        "model-m5.svg": fig_model_m5(),
        "model-m4.svg": fig_model_m4(),
        "handshake.svg": fig_handshake(),
        "measured-vs-proposed.svg": fig_extrapolation(
            d["coverage_cells"], d["operating_points"], d.get("alternatives", ())),
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
