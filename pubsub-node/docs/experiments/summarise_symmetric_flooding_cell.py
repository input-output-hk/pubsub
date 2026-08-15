"""Fold one symmetric-flooding cell's per-node detail into a summary JSON,
then delete the raw detail files (regenerable from the cell's config and
master seed with --per-node-detail).

The summary carries what the symmetric-flooding report reads per cell:
per-honest-victim route means and histograms (own-only / mutual /
admitted x linked-peer class — ADR 0042's decomposition), the Sybil-edge
histogram split into the cap-blind own-pick floor (mutual_s; own_only_s
would be a Sybil that never dialed back, impossible for a flooder) and
the admitted route, refusal totals with their crossing subsets, and the
cell's good / full-coverage aggregates.

Identities checked per run (detail kept on any violation):
  - route partition: own_only + mutual + admitted == downstream, per row
    and class;
  - refusals: detail dials_refused sums to the run row's
    rejected_over_capacity, and acceptor-issued == dialer-refused per
    class; crossing subsets never exceed their class counts.

Usage: python3 summarise_symmetric_flooding_cell.py <cell-dir>
    (writes <parent>/summaries/<cell>.json)"""
import json, glob, sys, os
from collections import Counter

cell = sys.argv[1].rstrip('/')
runs = {}
with open(f'{cell}/runs.jsonl') as f:
    for line in f:
        row = json.loads(line)
        runs[row['run']] = row

ROUTES = ['edges_own_only_honest', 'edges_own_only_adversarial',
          'edges_mutual_honest', 'edges_mutual_adversarial',
          'edges_admitted_honest', 'edges_admitted_adversarial']
route_sums = Counter()          # per-victim means, honest rows only
sybil_edges = Counter(); sybil_admitted = Counter(); sybil_floor = Counter()
honest_edges = Counter(); total_edges = Counter()
starved = Counter()             # per-victim dials_refused (honest rows)
refused_h = refused_a = 0
crossing_h = crossing_a = 0
identity_ok = True
detail_files = sorted(glob.glob(f'{cell}/run-*-detail.jsonl'))
for path in detail_files:
    rec = runs[int(path.split('run-')[1].split('-')[0])]
    s_ref = 0; i_h = i_a = 0; d_h = d_a = 0
    for line in open(path):
        row = json.loads(line)
        if row['publish'] != 0:
            continue
        # Route partition identity, per class.
        if (row['edges_own_only_honest'] + row['edges_mutual_honest']
                + row['edges_admitted_honest'] != row['downstream_honest']
                or row['edges_own_only_adversarial']
                + row['edges_mutual_adversarial']
                + row['edges_admitted_adversarial']
                != row['downstream_adversarial']):
            identity_ok = False
        if (row['refusals_issued_crossing_honest'] > row['refusals_issued_honest']
                or row['refusals_issued_crossing_adversarial']
                > row['refusals_issued_adversarial']):
            identity_ok = False
        s_ref += row['dials_refused']
        i_h += row['refusals_issued_honest']
        i_a += row['refusals_issued_adversarial']
        crossing_h += row['refusals_issued_crossing_honest']
        crossing_a += row['refusals_issued_crossing_adversarial']
        if row['class'] == 'honest':
            d_h += row['dials_refused']
            for f in ROUTES:
                route_sums[f] += row[f]
            adv = row['downstream_adversarial']
            sybil_edges[adv] += 1
            sybil_floor[row['edges_mutual_adversarial']] += 1
            sybil_admitted[row['edges_admitted_adversarial']] += 1
            honest_edges[row['downstream_honest']] += 1
            total_edges[row['downstream_honest'] + adv] += 1
            starved[row['dials_refused']] += 1
        else:
            d_a += row['dials_refused']
    if s_ref != rec['rejected_over_capacity'] or i_h != d_h or i_a != d_a:
        identity_ok = False
    refused_h += d_h; refused_a += d_a

n = sum(sybil_edges.values())
agg = json.load(open(f'{cell}/aggregates.json'))['experiments'][0]
summary = {
    'cell': os.path.basename(cell),
    'runs': len(detail_files),
    'victim_rows': n,
    'routes_mean': {f: route_sums[f] / n for f in ROUTES},
    'sybil_edges_mean': sum(k * v for k, v in sybil_edges.items()) / n,
    'sybil_edges_hist': dict(sorted(sybil_edges.items())),
    'sybil_floor_hist': dict(sorted(sybil_floor.items())),
    'sybil_admitted_hist': dict(sorted(sybil_admitted.items())),
    'honest_edges_mean': sum(k * v for k, v in honest_edges.items()) / n,
    'total_edges_hist': dict(sorted(total_edges.items())),
    'starved_hist': dict(sorted(starved.items())),
    'refused_honest': refused_h,
    'refused_adversarial': refused_a,
    'refused_crossing_honest': crossing_h,
    'refused_crossing_adversarial': crossing_a,
    'identity_ok': identity_ok,
    'good': agg['good'],
    'full_coverage': agg['full_coverage'],
}
os.makedirs(f'{os.path.dirname(cell)}/summaries', exist_ok=True)
out = f'{os.path.dirname(cell)}/summaries/{os.path.basename(cell)}.json'
with open(out, 'w') as f:
    json.dump(summary, f, indent=1)
if identity_ok:
    for path in detail_files:
        os.remove(path)
else:
    print(f"{summary['cell']}: identity VIOLATED - detail files kept for inspection")
r = summary['routes_mean']
print(f"{summary['cell']}: sybil_edges_mean={summary['sybil_edges_mean']:.3f} "
      f"(floor {r['edges_mutual_adversarial']:.3f} + admitted "
      f"{r['edges_admitted_adversarial']:.3f} + own_only "
      f"{r['edges_own_only_adversarial']:.3f}) refused_h={refused_h} "
      f"crossing_h={crossing_h} identity={'OK' if identity_ok else 'VIOLATED'}")
