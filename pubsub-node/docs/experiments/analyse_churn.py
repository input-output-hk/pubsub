#!/usr/bin/env python3
"""Compare an E13 churn sweep against each model's coverage law read at the
shifted adversarial fraction.

    python3 analyse_churn.py /tmp/e13            # markdown table to stdout
    python3 analyse_churn.py /tmp/e13 --json     # machine-readable

The claim under test is that honest downtime with per-epoch probability p is
indistinguishable from adversarial behaviour, so that P(bad) at churn p equals
the model's own law evaluated at mu + p(1-mu). Each sweep point is therefore
scored against that prediction, not against a separate churn model.
"""
from __future__ import annotations

import json
import math
import pathlib
import sys
from math import comb, exp, sqrt

MU = 0.2


def hyper(k: float, n_minus_1: int, r: int) -> float:
    """C(k,r)/C(N-1,r) — every one of r picks lands on an adversary."""
    k = int(round(k))
    if r == 0:
        return 1.0
    if r > k:
        return 0.0
    return comb(k, r) / comb(int(n_minus_1), r)


def law_m1(N, mu, F):
    k = mu * N
    H = N - k
    return 1 - exp(-H * ((1 - F / (N - 1)) ** (H - 1) + hyper(k, N - 1, F)))


def law_m2(N, mu, RF):
    k = mu * N
    H = N - k
    rho = 0.9
    for _ in range(3000):
        rho = 1 - exp(-RF * (1 - mu) * rho)
    u = 0.0
    for _ in range(8000):
        u = (mu + (1 - mu) * u) ** RF
    return 1 - exp(-H * ((1 - rho) + u))


def law_m3(N, mu, RF, s):
    k = mu * N
    H = N - k
    return 1 - exp(-H * (hyper(k, N - 1, RF)
                         + (1 - RF / (N - 1)) ** (H - 1) * hyper(k, N - 1, s - 1)))


def law_m4(N, mu, RF):
    k = mu * N
    H = N - k
    return 1 - exp(-H * hyper(k, N - 1, RF) * (1 - RF / (N - 1)) ** (H - 1))


def law_m5(N, mu, ki, ko):
    k = mu * N
    H = N - k
    return 1 - exp(-H * (hyper(k, N - 1, ki) * (1 - ko / (N - 1)) ** (H - 1)
                         + hyper(k, N - 1, ko) * (1 - ki / (N - 1)) ** (H - 1)))


def law_for(model: str, N: int, mu: float, honest: dict) -> float:
    """Read the model's coverage law at the given adversarial fraction."""
    relay = honest.get("pick_count")
    pub = (honest.get("publisher") or {}).get("pick_count")
    if model == "m1":
        return law_m1(N, mu, pub)
    if model == "m2":
        return law_m2(N, mu, relay)
    if model == "m3":
        return law_m3(N, mu, relay, pub + 1)
    if model == "m4":
        return law_m4(N, mu, relay)
    if model == "m5":
        return law_m5(N, mu, relay, pub)
    raise ValueError(model)


def wilson(k: int, n: int, z: float = 1.959963985) -> tuple[float, float]:
    if n == 0:
        return 0.0, 1.0
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = (z / d) * sqrt(p * (1 - p) / n + z * z / (4 * n * n))
    return max(0.0, c - h), min(1.0, c + h)


def z_vs(k: int, n: int, p0: float) -> float | None:
    if not 0 < p0 < 1:
        return None
    return (k / n - p0) / sqrt(p0 * (1 - p0) / n)


def params_label(model: str, honest: dict) -> str:
    relay = honest.get("pick_count")
    pub = (honest.get("publisher") or {}).get("pick_count")
    if model == "m1":
        return f"F={pub}"
    if model in ("m2", "m4"):
        return f"RF={relay}"
    if model == "m3":
        return f"RF={relay}, s={pub + 1}"
    return f"({relay}, {pub})"


def read_sweep(d: pathlib.Path) -> list[dict]:
    man = json.loads((d / "manifest.json").read_text())
    agg = json.loads((d / "aggregates.json").read_text())["experiments"]
    out = []
    for spec, a in zip(man["experiments"], agg):
        N = spec["size"]
        adv = spec["adversarial"]
        H = N - adv
        down = spec.get("churn_count", 0) or 0
        p = down / H
        mu_eff = MU + p * (1 - MU)
        good, runs = a["good"]["count"], a["good"]["runs"]
        bad = runs - good
        law = law_for(spec["model"], N, mu_eff, spec["honest_strategies"])
        lo, hi = wilson(bad, runs)
        out.append(dict(
            model=spec["model"].upper(), N=N, params=params_label(
                spec["model"], spec["honest_strategies"]),
            churn=p, down=down, mu_eff=mu_eff, bad=bad, runs=runs,
            measured=bad / runs, lo=lo, hi=hi, law=law,
            in_ci=lo <= law <= hi, z=z_vs(bad, runs, law),
            tool_commit=man.get("tool_commit"), master_seed=man.get("master_seed"),
        ))
    return out


def main() -> int:
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/e13")
    rows: list[dict] = []
    for d in sorted(root.iterdir()):
        if (d / "aggregates.json").exists():
            rows.extend(read_sweep(d))
    if not rows:
        print(f"no sweep output under {root}", file=sys.stderr)
        return 1

    if "--json" in sys.argv:
        print(json.dumps(rows, indent=2))
        return 0

    print("| model | params | churn | μ_eff | bad / runs | measured | Wilson 95% | "
          "law at μ_eff | in CI | z |")
    print("|---|---|---:|---:|---:|---:|---|---:|:--:|---:|")
    for r in rows:
        z = "—" if r["z"] is None else f"{r['z']:+.2f}"
        print(f"| {r['model']} | {r['params']} | {r['churn'] * 100:.0f}% | "
              f"{r['mu_eff']:.3f} | {r['bad']} / {r['runs']} | {r['measured']:.4f} | "
              f"[{r['lo']:.4f}, {r['hi']:.4f}] | {r['law']:.4f} | "
              f"{'yes' if r['in_ci'] else '**no**'} | {z} |")

    zs = [r["z"] for r in rows if r["z"] is not None]
    n = len(zs)
    mean = sum(zs) / n
    sd = sqrt(sum((x - mean) ** 2 for x in zs) / (n - 1)) if n > 1 else float("nan")
    print(f"\nCells: {n}. Law inside the Wilson interval in "
          f"{sum(1 for r in rows if r['in_ci'])} of {len(rows)}.")
    print(f"Mean z = {mean:+.3f}, Stouffer z = {mean * sqrt(n):+.2f}, sd(z) = {sd:.2f}.")

    churned = [r["z"] for r in rows if r["churn"] > 0 and r["z"] is not None]
    if churned:
        m2_ = sum(churned) / len(churned)
        print(f"Churned cells only ({len(churned)}): mean z = {m2_:+.3f}, "
              f"Stouffer z = {m2_ * sqrt(len(churned)):+.2f}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
