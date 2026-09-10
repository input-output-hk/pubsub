#!/usr/bin/env python3
"""Check the CIP's candidate bucket table against its baseline coverage law.

    python3 -B pubsub-node/docs/experiments/check_cip_bucket_table.py

Run from any directory with Python 3; no third-party packages are needed.
The table profile uses mu = 0.2, delta = 1e-4 and nearest-integer S = round(N/5).
Every population in each closed row is checked. The final, open row is checked
only through --max-population (default 20,000), never extrapolated to infinity.

The finite binomial calculation follows gated_symmetric_predictions.py, using
the same mean +/- 12 standard deviations (plus two entries) summation window.
It estimates isolated nodes, not all disconnected components, and excludes
admission refusals and downtime. Passing this check is not simulation evidence
or a proof that a deployment meets its delivery target.
"""

import argparse
from decimal import Decimal, ROUND_HALF_UP
from functools import lru_cache
import math
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[3]
CIP = ROOT / "docs/cip/README.md"
DELTA = 1e-4


def adversarial_count(n):
    # Exact nearest-integer rounding of N/5; ties cannot occur.
    return (n + 2) // 5


def binomial_terms(n, p):
    if p == 1:
        return [(n, 1.0)]
    if n == 0:
        return [(0, 1.0)]
    mean = n * p
    sd = math.sqrt(mean * (1 - p))
    lo = max(0, int(mean - 12 * sd) - 2)
    hi = min(n, int(mean + 12 * sd) + 2)
    log_n_factorial = math.lgamma(n + 1)
    return [
        (j, math.exp(log_n_factorial - math.lgamma(j + 1)
                     - math.lgamma(n - j + 1)
                     + j * math.log(p) + (n - j) * math.log1p(-p)))
        for j in range(lo, hi + 1)
    ]


@lru_cache(maxsize=100_000)
def adversarial_picks(h, a, k):
    if h == 0:
        return 1.0
    if a < k:
        return 0.0
    return math.comb(a, k) / math.comb(a + h, k)


def baseline_failure(n, k, b, s=None):
    """Evaluate the CIP's I and 1-exp(-H*I), with an explicit integer S."""
    s = adversarial_count(n) if s is None else s
    honest = n - s
    if b == 1:
        picks = min(k, n - 1)
        isolation = (adversarial_picks(honest - 1, s, picks)
                     * (1 - picks / (n - 1)) ** (honest - 1))
    else:
        gate = 1 / b
        m = math.fsum(prob * min(k, j + 1) / (j + 1)
                      for j, prob in binomial_terms(n - 2, gate))
        m = min(1.0, max(0.0, m))  # Roundoff at the all-peers-picked limit.
        adversaries = [(a, prob) for a, prob in binomial_terms(s, gate) if a >= k]
        terms = []
        for h, prob in binomial_terms(honest - 1, gate):
            avoid = (1.0 if h == 0 else math.fsum(
                weight * adversarial_picks(h, a, k) for a, weight in adversaries))
            terms.append(prob * (1 - m) ** h * avoid)
        isolation = math.fsum(terms)
    return -math.expm1(-honest * isolation)


def arithmetic_ceilings(n, k):
    pool = math.floor((n - 1) * 0.8 / math.log(0.8 * n / DELTA))
    headroom = (n - 1) // (2 * k)
    return pool, headroom


def combined_ceiling(n, k=10):
    """Largest integer B satisfying all three candidate ceilings."""
    lo, hi = 1, min(arithmetic_ceilings(n, k))
    if hi < 1 or baseline_failure(n, k, 1) > DELTA:
        return 0
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if baseline_failure(n, k, mid) <= DELTA:
            lo = mid
        else:
            hi = mid - 1
    return lo


def format_loss(ceiling, b):
    return (Decimal(ceiling) / b).quantize(Decimal('0.01'), rounding=ROUND_HALF_UP)


def read_rows():
    section = CIP.read_text().split('<a name="table-2" id="table-2"></a>', 1)[1]
    section = section.split('</div>', 1)[0]
    rows = []
    for match in re.finditer(
            r'^\| ([\d,]+)(?: – ([\d,]+)| and above) \| (\d+) \| .*? \| (\d+) \|$',
            section, re.M):
        start, end, b, k = match.groups()
        rows.append((int(start.replace(',', '')),
                     int(end.replace(',', '')) if end else None, int(b), int(k)))
    if not rows or rows[0][0] != 2 or rows[-1][1] is not None:
        raise ValueError('Expected contiguous table rows from 2 through an open final row')
    for row, following in zip(rows, rows[1:]):
        if row[1] is None or row[1] + 1 != following[0]:
            raise ValueError('Table rows overlap or leave a gap')
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--max-population', type=int, default=20_000)
    args = parser.parse_args()
    rows = read_rows()
    if args.max_population < rows[-1][0]:
        parser.error('--max-population must reach the final row')

    # Published E20 ladder and the CIP's B=512 baseline estimate, evaluated
    # independently of the historical ledger's global N=4000, K=16 defaults.
    anchors = [(20_000, 9, 250, 1.41353972e-5),
               (20_000, 9, 500, 3.61089941e-5),
               (20_000, 9, 740, 1.03997526e-4),
               (20_000, 10, 500, 5.05346247e-6),
               (20_000, 10, 512, 5.36731434e-6)]
    for n, k, b, expected in anchors:
        actual = baseline_failure(n, k, b)
        if not math.isclose(actual, expected, rel_tol=1e-7):
            raise AssertionError(f'Reference mismatch at {(n, k, b)}: {actual}')
    print('Reference calculations: 5/5 agree. Profile: mu=0.2, delta=1e-4, S=round(N/5).',
          flush=True)

    failures = 0
    checked = 0
    print('B | k | Checked populations | Worst baseline p_bad | At N | Failures', flush=True)
    for start, end, b, k in rows:
        stop = args.max_population if end is None else end
        worst = (-1.0, start)
        failed = []
        for n in range(start, stop + 1):
            p = baseline_failure(n, k, b)
            checked += 1
            worst = max(worst, (p, n))
            pool, headroom = arithmetic_ceilings(n, k)
            if p > DELTA or (b > 1 and b > min(pool, headroom)):
                failed.append(n)
            elif (end is not None and 2 * b <= min(pool, headroom)
                  and baseline_failure(n, k, 2 * b) <= DELTA):
                # The appendix also claims that closed rows give up less than
                # a factor of two relative to the combined ceiling.
                failed.append(n)
        failures += len(failed)
        print(f'{b} | {k} | {start}-{stop} | {worst[0]:.9g} | {worst[1]} | {len(failed)}',
              flush=True)
        if failed:
            print(f'  First failing populations: {failed[:5]}', flush=True)

    print('\nTop-of-row ceilings (all three constraints):', flush=True)
    for _, end, b, k in rows:
        if end is not None and b > 1:
            ceiling = combined_ceiling(end, k)
            print(f'N={end}: ceiling={ceiling}, B={b}, loss={format_loss(ceiling, b)}x',
                  flush=True)
    print('\nAdditional sizing arithmetic (not simulation):', flush=True)
    for n, b in [(3000, 128), (4000, 128), (20_000, 512),
                 (24_968, 512), (24_969, 512), (40_000, 512), (100_000, 512)]:
        ceiling = combined_ceiling(n)
        print(f'N={n}: ceiling={ceiling}, B={b}, loss={format_loss(ceiling, b)}x', flush=True)

    print(f'\nChecked {checked} populations; {failures} failures.', flush=True)
    raise SystemExit(1 if failures else 0)


if __name__ == '__main__':
    main()
