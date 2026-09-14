#!/usr/bin/env python3
"""Validate the data used by the CIP figures against source rows and model laws.

    python3 -B pubsub-node/docs/experiments/check_cells_against_docs.py

Measured values are matched to a named document, section, configuration, row
and column. Rounding follows that source cell's printed precision. Counts must
match exactly. Coverage laws are independently recomputed with analyse_churn;
this checks transcription, not the validity of those models or raw simulations.

Coverage: operating_points, alternatives, coverage_cells, churn_cells,
gated_point and gate_tradeoff (the groups make_cip_figures.py consumes).
Unused historical groups in cells.json are outside this check. See
cip-evidence-checks.md for scope, precision and reproduction instructions.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from decimal import Decimal, ROUND_HALF_UP
from functools import lru_cache
import json
import math
from pathlib import Path
import re
import sys

import analyse_churn
from m4_synthesis_predictions import m3_isolation

HERE = Path(__file__).resolve().parent
MODELS = {f'M{i}' for i in range(1, 6)}


def clean(text):
    text = re.sub(r'<[^>]+>', '', text).replace('*', '').replace('`', '')
    text = text.replace('\u202f', ' ').replace('\xa0', ' ')
    return re.sub(r'(?<=\d)[ ,](?=\d{3}(?:\D|$))', '', text).strip()


def parameters(text):
    """Parameter integers, excluding the M5 resample annotation."""
    return tuple(map(int, re.findall(r'\d+', text.split('resample')[0].split('),')[0])))


def one(items, context):
    items = list(items)
    if len(items) != 1:
        raise ValueError(f'{context}: expected one source row, found {len(items)}')
    return items[0]


def table(documents, name, section, index=0):
    """Read a table within one named section; never search the whole document."""
    if name not in documents:
        raise ValueError(f'UNSOURCED: missing document {name}')
    text = documents[name]
    headings = list(re.finditer(r'^## .+$', text, re.M))
    heading = one((h for h in headings if h.group().startswith(section)), f'{name}/{section}')
    end = next((h.start() for h in headings if h.start() > heading.start()), len(text))
    blocks = re.findall(r'(?:^\|.*\|\s*\n)+', text[heading.end():end] + '\n', re.M)
    if index >= len(blocks):
        raise ValueError(f'UNSOURCED: missing table {index} in {name}/{section}')
    rows = [[clean(c) for c in line.strip().strip('|').split('|')]
            for line in blocks[index].strip().splitlines()]
    if len(rows) < 3 or not all(re.fullmatch(r'[:\- ]+', c) for c in rows[1]):
        raise ValueError(f'invalid source table in {name}/{section}')
    return rows[2:]


def row(rows, label):
    return one((r for r in rows if r[0] == label), label)


def number(text):
    """Read the leading numeric value in a cell, preserving printed precision."""
    text = clean(text).lstrip('≈ ')
    text = re.sub(r'×10([⁻⁰¹²³⁴⁵⁶⁷⁸⁹]+)',
                  lambda m: 'e' + m[1].translate(str.maketrans('⁻⁰¹²³⁴⁵⁶⁷⁸⁹', '-0123456789')), text)
    match = re.match(r'[-+]?\d+(?:\.\d+)?(?:e[-+]?\d+)?', text)
    if not match:
        raise ValueError(f'expected a numeric source cell, got {text!r}')
    return Decimal(match[0])


@dataclass
class Report:
    errors: list[str] = field(default_factory=list)
    checked: int = 0

    def value(self, actual, expected, label, *, exact=False):
        self.checked += 1
        if isinstance(expected, str):
            expected = number(expected)
        actual = Decimal(int(actual)) if isinstance(actual, bool) else Decimal(str(actual))
        expected = Decimal(int(expected)) if isinstance(expected, bool) else Decimal(str(expected))
        valid = actual.is_finite() and expected.is_finite()
        if valid:
            valid = (actual == expected if exact else
                     actual.quantize(expected, rounding=ROUND_HALF_UP) == expected)
        if not valid:
            self.errors.append(f'{label}: {actual} differs from source {expected}')

    def metric(self, entry, key, source, label, *, exact=False):
        if key not in entry:
            raise ValueError(f'{label}: missing metric {key}')
        self.value(entry[key], source, f'{label}/{key}', exact=exact)

    def law(self, entry, key, label):
        self.checked += 1
        model = entry['model']
        value = ungated_law(model, entry.get('N', 20000), entry.get('mu_eff', .2),
                            parameters(entry['params']))
        # Coverage/churn ledgers are printed to eight decimal places. The
        # operating-point probabilities are printed to 3–4 significant digits.
        tolerance = 5e-9 if key == 'law' else abs(value) * 1e-3
        if key not in entry or not math.isclose(entry[key], value, abs_tol=tolerance, rel_tol=0):
            self.errors.append(f'{label}/{key}: {entry.get(key)} differs from recomputed {value}')


@lru_cache(maxsize=None)
def ungated_law(model, n, mu, params):
    return getattr(analyse_churn, f'law_{model.lower()}')(n, mu, *params)


# (model, parameter tuple) -> source document, section, table and value column.
# Selecting these coordinates does not depend on the value being checked.
OPERATING_SOURCES = {
    ('M3', (12, 8)): ('m3-comparison.md', '## 2.', 0, 2),
    ('M4', (8,)): ('m4-comparison.md', '## 2.', 0, 2),
    ('M5', (9, 8)): ('m5-comparison.md', '## 3.', 0, 2),
    ('M1', (24,)): ('m5-comparison.md', '## 3.', 1, 2),
    ('M2', (24,)): ('m2-comparison.md', '## 1.', 0, 2),
    ('M3', (13, 7)): ('m3-comparison.md', '## 5.', 0, 2),
    ('M4', (9,)): ('m4-comparison.md', '## 5.', 0, 2),
}
COST_ROWS = {
    'copies_per_node': 'copies per honest node',
    'hops_full': 'hops, full coverage',
    'hops_mean': 'hops, mean first receipt',
    'msgs_per_publication': 'honest→honest sends per message',
}


def check_operating(report, e, docs, label):
    key = e['model'], parameters(e['params'])
    if key not in OPERATING_SOURCES:
        raise ValueError(f'UNSOURCED configuration {key}')
    name, section, index, column = OPERATING_SOURCES[key]
    source = table(docs, name, section, index)
    for metric, title in COST_ROWS.items():
        report.metric(e, metric, row(source, title)[column], label)
    preferred = key in {('M3', (13, 7)), ('M4', (9,))}
    report.value(e.get('preferred', False), preferred, label + '/preferred', exact=True)
    if preferred:
        links = row(source, 'standing links, mean / max')[column].split('/')
        report.metric(e, 'standing_links', links[0], label)
        report.metric(e, 'standing_links_measured_mean', links[0], label)
        report.metric(e, 'standing_links_max', links[1], label, exact=True)
        report.metric(e, 'churn_budget_pct', row(source, 'churn budget')[column], label)
    else:
        means = table(docs, 'standing-degree.md', '## 1.')
        mean = one((r for r in means if (r[0], parameters(r[1])) == key), str(key))
        report.metric(e, 'standing_links', mean[4], label, exact=True)
        report.metric(e, 'standing_links_measured_mean', mean[3], label)
        maximum = row(table(docs, 'standing-degree.md', '## 2.'), e['model'])
        report.metric(e, 'standing_links_max', maximum[2], label, exact=True)
        budgets = table(docs, 'churn-tolerance.md', '## 4.')
        budget = one((r for r in budgets if (r[0], parameters(r[1])) == key), str(key))
        report.metric(e, 'churn_budget_pct', budget[2], label)
    report.law(e, 'p_bad', label)


def count_pair(report, e, cell, label, good=False):
    counts = re.match(r'(\d+)\s*/\s*(\d+)', clean(cell))
    if not counts:
        raise ValueError(f'{label}: missing source counts {cell!r}')
    count, runs = map(int, counts.groups())
    report.metric(e, 'bad', runs - count if good else count, label, exact=True)
    report.metric(e, 'runs', runs, label, exact=True)


def check_coverage(report, e, docs, label):
    model, n, params = e['model'], e['N'], parameters(e['params'])
    if e['master_seed'] in (821, 822):
        expected = {821: ('M3', 4000, (9, 5), '## 1.'),
                    822: ('M4', 20000, (6,), '## 2.')}
        m, size, picks, section = expected[e['master_seed']]
        if (model, n, params) != (m, size, picks):
            raise ValueError(f'{label}: deep-tail configuration does not match its source')
        r = row(table(docs, 'tail-correction.md', section), 'this run')
        count_pair(report, e, r[1], label)
    elif model == 'M2':
        if params != (16,) or n not in (4000, 20000):
            raise ValueError(f'UNSOURCED M2 coverage configuration {(n, params)}')
        r = row(table(docs, 'm2-comparison.md', '## 2.' if n == 4000 else '## 3.'),
                'this framework')
        count_pair(report, e, r[1], label, good=n == 20000)
    else:
        name = {'M1': 'm5-comparison.md', 'M3': 'm3-comparison.md',
                'M4': 'm4-comparison.md', 'M5': 'm5-comparison.md'}[model]
        rows = table(docs, name, '## 2.' if model == 'M1' else '## 1.')
        if model == 'M4':
            if n != 20000:
                raise ValueError(f'{label}: M4 comparison source is at N=20000')
            r = one((r for r in rows if parameters(r[0]) == params), label)
            count_pair(report, e, r[1], label)
        elif model == 'M3':
            r = one((r for r in rows if (int(r[0]), int(r[1]), int(r[2])) == (n, *params)), label)
            count_pair(report, e, r[3], label)
        else:
            r = one((r for r in rows if int(r[0]) == n and parameters(r[1]) == params
                     and ('resample' in e['params']) == ('runs' in r[1])), label)
            count_pair(report, e, r[2], label)
    report.law(e, 'law', label)


def check_churn(report, e, docs, label):
    key = e['model'], parameters(e['params'])
    preferred = key in {('M3', (13, 7)), ('M4', (9,))}
    if preferred:
        rows = table(docs, 'churn-proposed-points.md', '## 1.', 0 if key[0] == 'M3' else 1)
        r = one((r for r in rows if number(r[0]) == e['churn_pct']), label)
        mu, counts = r[1:3]
        n, at_op = 20000, True
    else:
        at_op = key in OPERATING_SOURCES
        rows = table(docs, 'churn-tolerance.md', '## 2b.' if at_op else '## 1.')
        r = one((r for r in rows if (r[0], parameters(r[1])) == key
                 and number(r[2]) == e['churn_pct']), label)
        mu, counts = r[3:5]
        n = 20000 if at_op else 4000
    report.metric(e, 'N', n, label, exact=True)
    report.metric(e, 'at_operating_point', at_op, label, exact=True)
    report.metric(e, 'mu_eff', mu, label)
    count_pair(report, e, counts, label)
    report.law(e, 'law', label)


def check_gated(report, e, docs, label):
    if (e['model'], parameters(e['params'])) != ('M4', (10, 500, 23)):
        raise ValueError(f'UNSOURCED gated configuration {e.get("params")}')
    source = table(docs, 'm4-synthesis.md', '## 2.', 1)
    r = row(source, 'gated + capped K = 10 (seed 1139)')
    report.metric(e, 'p_bad', r[1], label)
    counts = {'bad': e['measured_bad'], 'runs': e['measured_runs']}
    count_pair(report, counts, r[2], label, good=True)
    # The flooded row's prediction is in the E20 anchor table.
    flooded = table(docs, 'm4-synthesis.md', '## 3.')
    r = one((r for r in flooded if tuple(map(int, r[:5])) == (20000, 10, 500, 23, 4000)),
            'E20 full flooder')
    report.metric(e, 'flooded_p_bad', r[6], label)
    count_pair(report, counts, r[5], label + '/flooded', good=True)


def check_gate(report, gate, docs, label):
    # The source experiment fixes this scenario; the concentration identity
    # budget is an illustrative chart input, not a measured quantity.
    for key, value in [('N', 4000), ('RF', 16)]:
        report.metric(gate, key, value, label, exact=True)
    report.metric(gate, 'law', '0.0088', label)
    if not isinstance(gate['sybils_for_concentration'], int) or gate['sybils_for_concentration'] < 1:
        raise ValueError('gate_tradeoff: concentration budget must be a positive integer')
    rows = table(docs, 'e10-selection-fidelity.md', '## 2.')
    if len({e['B'] for e in gate['cells']}) != len(gate['cells']):
        raise ValueError('gate_tradeoff: duplicate bucket count')
    if len(gate['cells']) != len(rows):
        raise ValueError('gate_tradeoff: missing or extra source cells')
    for e in gate['cells']:
        r = one((r for r in rows if int(r[0]) == e['B']), f'B={e["B"]}')
        item = f'{label}/B={e["B"]}'
        for key, value in [('r', r[1]), ('runs', r[2]), ('bad', r[3])]:
            report.metric(e, key, value, item, exact=key in ('runs', 'bad'))
        lo, hi = r[5].strip('[]').split(',')
        report.metric(e, 'lo', lo, item)
        report.metric(e, 'hi', hi, item)


@lru_cache(maxsize=1)
def m3_gated_downtime():
    """Largest integer honest dropout count meeting the baseline target."""
    def bad(down):
        return -math.expm1(-sum(m3_isolation(20000, 13, 769, 4000 + down, 7, 769)))
    lo, hi = 0, 8000
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if bad(mid) <= 1e-4:
            lo = mid
        else:
            hi = mid
    return lo, bad(lo), bad(hi)


def check_cells(cells, documents):
    report = Report()
    if not isinstance(cells, dict):
        report.errors.append('expected an object containing the evidence groups')
        return report
    # These are the curated datasets described by the figure captions. A new
    # sample requires updating the source mapping and caption together.
    sizes = {'operating_points': 5, 'alternatives': 2,
             'coverage_cells': 25, 'churn_cells': 35}
    try:
        for key, value in [('adversarial_fraction', .2), ('design_target', .0001),
                           ('operating_point_network_size', 20000)]:
            report.metric(cells['parameters'], key, value, 'parameters', exact=True)
    except (KeyError, TypeError, ValueError, ArithmeticError) as exc:
        report.errors.append(f'parameters: {exc}')
    groups = [('operating_points', check_operating), ('alternatives', check_operating),
              ('coverage_cells', check_coverage), ('churn_cells', check_churn),
              ('gated_point', check_gated), ('gate_tradeoff', check_gate)]
    for group, check in groups:
        if group not in cells:
            report.errors.append(f'{group}: missing data group')
            continue
        expected_type = list if group in sizes else dict
        if not isinstance(cells[group], expected_type):
            report.errors.append(f'{group}: expected {expected_type.__name__}')
            continue
        entries = cells[group] if group in sizes else [cells[group]]
        if group in sizes and len(entries) != sizes[group]:
            report.errors.append(f'{group}: expected {sizes[group]} datasets, found {len(entries)}')
        seen = set()
        for i, entry in enumerate(entries):
            if not isinstance(entry, dict):
                report.errors.append(f'{group}/{i}: expected a data record')
                continue
            identity = tuple(str(entry.get(k)) for k in ('model', 'N', 'params', 'master_seed', 'churn_pct'))
            if identity in seen:
                report.errors.append(f'{group}/{i}: duplicate dataset')
            seen.add(identity)
            label = f'{group}/{i}/{entry.get("model", "M2")}/{entry.get("params", "gate ladder")}'
            try:
                if entry.get('model', 'M2') not in MODELS:
                    raise ValueError(f'UNSOURCED model {entry.get("model")}')
                check(report, entry, documents, label)
            except (ValueError, KeyError, TypeError, ArithmeticError, IndexError) as exc:
                report.errors.append(f'{label}: {exc}')
    return report


def main():
    try:
        cells = json.loads((HERE / 'cells.json').read_text(encoding='utf-8'))
        docs = {p.name: p.read_text(encoding='utf-8') for p in HERE.glob('*.md')}
        report = check_cells(cells, docs)
        down, below, above = m3_gated_downtime()
        budget = (Decimal(down) * 100 / 16000).quantize(Decimal('.01'), rounding=ROUND_HALF_UP)
        # Both CIP tables must continue to quote the reproducible result.
        for name in ('README.md', 'design-comparison.md'):
            text = (HERE.parents[2] / 'docs/cip' / name).read_text(encoding='utf-8')
            r = one(re.findall(r'^\| Predicted honest downtime absorbed[^|]*\|([^|]+)\|', text, re.M), name)
            report.value(number(r), budget, f'{name}/M3 gated downtime', exact=True)
        print(f'M3 gated downtime: {down}/16000 = {budget}% (display rounded); '
              f'p_bad={below:.9g}, next dropout={above:.9g}')
        for error in report.errors:
            print(f'MISMATCH  {error}')
        print(f'{report.checked} values checked; {len(report.errors)} errors.')
        return 1 if report.errors else 0
    except (OSError, ValueError, KeyError) as exc:
        print(f'Unable to validate evidence: {exc}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
