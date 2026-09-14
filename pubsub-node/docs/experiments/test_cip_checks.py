"""Regression checks for the CIP's evidence validation (standard library only)."""
import contextlib
import copy
import io
import shutil
import sys
import tempfile
from unittest.mock import patch
import json
from pathlib import Path
import unittest

import check_cells_against_docs as cells_check
import check_cip_bucket_table as buckets
import make_cip_figures as figures

HERE = Path(__file__).resolve().parent


class EvidenceChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cells = json.loads((HERE / 'cells.json').read_text(encoding='utf-8'))
        cls.docs = {p.name: p.read_text(encoding='utf-8') for p in HERE.glob('*.md')}

    def test_committed_evidence(self):
        self.assertEqual(cells_check.check_cells(self.cells, self.docs).errors, [])

    def test_transcription_errors(self):
        changes = [
            ('operating_points', 0, 'copies_per_node', 9.99),
            ('operating_points', 1, 'hops_full', 6.49),
            ('operating_points', 2, 'copies_per_node', 13.99),
            ('alternatives', 0, 'hops_mean', 3.49),
            ('operating_points', 0, 'msgs_per_publication', 987654321),
            ('alternatives', 0, 'copies_per_node', 9.6),  # old configuration's value
            ('alternatives', 1, 'standing_links', 16),
            ('coverage_cells', 0, 'bad', 209),  # formal MC column, not ours
            ('coverage_cells', 0, 'runs', 601),
            ('coverage_cells', 0, 'law', 0.5),
            ('churn_cells', 0, 'bad', 31),  # zero-churn row, not the 2% row
            ('churn_cells', 0, 'mu_eff', 0.24),
            ('operating_points', 0, 'churn_budget_pct', 2.17),
        ]
        for group, index, key, value in changes:
            with self.subTest(group=group, index=index, key=key):
                changed = copy.deepcopy(self.cells)
                changed[group][index][key] = value
                self.assertTrue(cells_check.check_cells(changed, self.docs).errors)

    def test_gate_and_gated_reference(self):
        for mutate in [
            lambda d: d['gated_point'].update(p_bad=.9),
            lambda d: d['gated_point'].update(flooded_p_bad=.9),
            lambda d: d['gated_point'].update(measured_runs=800),
            lambda d: d['gate_tradeoff']['cells'][0].update(bad=78),
        ]:
            changed = copy.deepcopy(self.cells)
            mutate(changed)
            self.assertTrue(cells_check.check_cells(changed, self.docs).errors)

    def test_unknown_model_and_missing_source_are_errors(self):
        changed = copy.deepcopy(self.cells)
        changed['operating_points'][0]['model'] = 'M6'
        self.assertTrue(cells_check.check_cells(changed, self.docs).errors)
        docs = dict(self.docs)
        del docs['m3-comparison.md']
        self.assertTrue(cells_check.check_cells(self.cells, docs).errors)

    def test_source_changes_and_ambiguity_are_detected(self):
        for replacement in ['| copies per honest node | **9.60** | 10.49 |',
                            '| copies per honest node | **9.60** | 10.40 |\n'
                            '| copies per honest node | **9.60** | 10.40 |']:
            docs = dict(self.docs)
            docs['m3-comparison.md'] = docs['m3-comparison.md'].replace(
                '| copies per honest node | **9.60** | 10.40 |', replacement)
            self.assertTrue(cells_check.check_cells(self.cells, docs).errors)

    def test_missing_metric_is_not_silently_skipped(self):
        changed = copy.deepcopy(self.cells)
        del changed['alternatives'][0]['hops_full']
        self.assertTrue(cells_check.check_cells(changed, self.docs).errors)

    def test_missing_or_duplicate_datasets_fail(self):
        for group in ('operating_points', 'alternatives', 'coverage_cells', 'churn_cells'):
            for duplicate in (False, True):
                changed = copy.deepcopy(self.cells)
                if duplicate:
                    changed[group][0] = copy.deepcopy(changed[group][1])
                else:
                    changed[group].pop()
                with self.subTest(group=group, duplicate=duplicate):
                    self.assertTrue(cells_check.check_cells(changed, self.docs).errors)

    def test_malformed_group_fails_without_crashing(self):
        for value in (None, {}, [42]):
            changed = copy.deepcopy(self.cells)
            changed['coverage_cells'] = value
            self.assertTrue(cells_check.check_cells(changed, self.docs).errors)

    def test_m3_gated_downtime_is_derived(self):
        count, below, above = cells_check.m3_gated_downtime()
        self.assertEqual(count, 252)
        self.assertLessEqual(below, 1e-4)
        self.assertGreater(above, 1e-4)


class BucketTableChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cip = buckets.CIP.read_text(encoding='utf-8')
        cls.rows = buckets.read_rows(cls.cip)

    def test_table_14_matches_calculations(self):
        self.assertEqual(buckets.check_loss_table(self.cip, self.rows), [])

    def test_table_14_mutations_fail(self):
        original = '| 80 | 3 | 2 | 1.50× |'
        for replacement in ['| 80 | 4 | 2 | 1.50× |',
                            '| 80 | 3 | 2 | 1.51× |', '', original + '\n' + original]:
            with self.subTest(replacement=replacement):
                self.assertTrue(buckets.check_loss_table(
                    self.cip.replace(original, replacement), self.rows))


class FigureInventoryChecks(unittest.TestCase):
    def test_missing_manual_and_untracked_figures_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'cip'
            shutil.copytree(figures.OUT.parent, root)
            for missing_manual in (True, False):
                if missing_manual:
                    (root / 'images/joining.svg').unlink()
                else:
                    shutil.copyfile(figures.OUT / 'joining.svg', root / 'images/joining.svg')
                    with (root / 'README.md').open('a', encoding='utf-8') as out:
                        out.write('\n![Untracked](images/untracked.svg)\n')
                with self.subTest(missing_manual=missing_manual):
                    with patch.object(figures, 'OUT', root / 'images'), \
                         patch.object(sys, 'argv', ['make_cip_figures.py', '--check']), \
                         contextlib.redirect_stdout(io.StringIO()), \
                         contextlib.redirect_stderr(io.StringIO()):
                        self.assertEqual(figures.main(), 1)


if __name__ == '__main__':
    unittest.main()
