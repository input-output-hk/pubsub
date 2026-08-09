"""Fold one E12 flooding cell's per-node detail into a small summary JSON,
then delete the raw detail files (regenerable from the cell's config and
master seed with --per-node-detail).

The summary carries everything e12-flooding-mitigation.md reads per cell:
the attacker-held-slots histogram over honest victims, honest-slot and
total-slot histograms, the per-victim starvation histogram, refusal totals
split by the refused dialer's class, the per-run accounting-identity
verdict (detail sums == the run row's rejected_over_capacity;
acceptor-issued == dialer-refused per class), and the cell's good /
full-coverage aggregates.

Usage: python3 summarise_flooding_cell.py <cell-dir>
    (writes <parent>/summaries/<cell>.json)"""
import json, glob, sys, os
from collections import Counter

cell = sys.argv[1].rstrip('/')
runs = {}
with open(f'{cell}/runs.jsonl') as f:
    for line in f:
        row = json.loads(line)
        runs[row['run']] = row

adv_slots = Counter(); honest_slots = Counter(); total_slots = Counter()
starved = Counter()      # per-victim dials_refused (honest dialers only)
refused_h = refused_a = 0
identity_ok = True
detail_files = sorted(glob.glob(f'{cell}/run-*-detail.jsonl'))
for path in detail_files:
    rec = runs[int(path.split('run-')[1].split('-')[0])]
    s_ref = 0; i_h = i_a = 0; d_h = d_a = 0
    for line in open(path):
        row = json.loads(line)
        s_ref += row['dials_refused']
        i_h += row['refusals_issued_honest']; i_a += row['refusals_issued_adversarial']
        if row['class'] == 'honest':
            d_h += row['dials_refused']
            adv_slots[row['downstream_adversarial']] += 1
            honest_slots[row['downstream_honest']] += 1
            total_slots[row['downstream_honest'] + row['downstream_adversarial']] += 1
            starved[row['dials_refused']] += 1
        else:
            d_a += row['dials_refused']
    if s_ref != rec['rejected_over_capacity'] or i_h != d_h or i_a != d_a:
        identity_ok = False
    refused_h += d_h; refused_a += d_a

n = sum(adv_slots.values())
agg = json.load(open(f'{cell}/aggregates.json'))['experiments'][0]
summary = {
    'cell': os.path.basename(cell),
    'runs': len(detail_files),
    'victim_rows': n,
    'adv_slots_mean': sum(k*v for k, v in adv_slots.items()) / n,
    'adv_slots_hist': dict(sorted(adv_slots.items())),
    'honest_slots_mean': sum(k*v for k, v in honest_slots.items()) / n,
    'total_slots_hist': dict(sorted(total_slots.items())),
    'starved_hist': dict(sorted(starved.items())),
    'refused_honest': refused_h,
    'refused_adversarial': refused_a,
    'identity_ok': identity_ok,
    'good': agg['good'],
    'full_coverage': agg['full_coverage'],
}
out = f'{os.path.dirname(cell)}/summaries/{os.path.basename(cell)}.json'
with open(out, 'w') as f:
    json.dump(summary, f, indent=1)
for path in detail_files:
    os.remove(path)
print(f"{summary['cell']}: adv_slots_mean={summary['adv_slots_mean']:.3f} "
      f"refused_h={refused_h} refused_a={refused_a} identity={'OK' if identity_ok else 'VIOLATED'}")
