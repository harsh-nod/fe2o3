#!/usr/bin/env python3
"""Synthetic descriptive comparison controls; no native or external commands."""

import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_xgmi_retained_wait_cadence_campaign as fixture
import xgmi_retained_host_diagnostic as diagnostic
import xgmi_retained_wait_cadence_comparison as comparison


def compare(rows):
    return comparison.compare(rows, environment_after=fixture.existing.environment(), **fixture.existing.inputs())


def change_samples(row, change):
    fields = [diagnostic.row(line) for line in row['stdout'].decode('ascii').splitlines()]
    for sample in fields[1:]:
        change(sample)
    row['stdout'] = fixture.fixture.encode(fields)


class Comparison(unittest.TestCase):
    def test_complete_cells_invocations_and_diagnostic_partitions(self):
        result = compare(fixture.receipts())
        self.assertEqual((len(result['trials']), result['ordinary_trial_count'], result['profiled_trial_count']), (36, 24, 12))
        self.assertEqual((len(result['ordinary']), len(result['profiled']), result['profiled_sample_count']), (6, 12, 240))
        for row in result['ordinary']:
            self.assertEqual(set(row['cells']), set(comparison.CELLS))
            self.assertTrue(all(len(cell['invocations']) == 2 for cell in row['cells'].values()))
        for row in result['profiled']:
            self.assertEqual(len(row['invocations']), 2)
            self.assertEqual(row['timing'], 'instrumented-host-only')
            self.assertEqual(set(row['distributions']), set(comparison.METRICS))
            self.assertTrue(all(value['samples'] == value['available'] == 20 and value['missing'] == 0
                                for value in row['distributions'].values()))
        self.assertTrue(all(count == 0 for count in result['profiled_missing_by_field'].values()))

    def test_nearest_rank_then_mean_of_two_not_pooled_or_central_average(self):
        rows = fixture.receipts()
        for policy, scale, offset in (('ordinary-1ms', 10, 100), ('ceiling-25us', 2, 20)):
            selected = [row for row in rows if row['depth'] == 1 and row['cadence'] == policy and row['mode'] == 'ordinary']
            for invocation, row in enumerate(selected):
                def edit(sample):
                    if sample['direction'] == 'forward':
                        sample['elapsed_ns'] = str((int(sample['index']) + 1) * scale + invocation * offset)
                change_samples(row, edit)
        result = compare(rows)
        cell = next(row for row in result['ordinary'] if row['depth'] == 1 and row['direction'] == 'forward')
        self.assertEqual([row['p50_ns'] for row in cell['cells']['ordinary-1ms']['invocations']], [50, 150])
        self.assertEqual([row['p50_ns'] for row in cell['cells']['ceiling-25us']['invocations']], [10, 30])
        self.assertEqual(cell['cells']['ordinary-1ms']['mean_of_two_invocation_p50_ns'], 100)
        self.assertEqual(cell['cells']['ceiling-25us']['mean_of_two_invocation_p50_ns'], 20)
        self.assertEqual(cell['latency_ratios']['ceiling-25us_over_ordinary-1ms'], 0.2)
        for backend in ('hsa', 'hip'):
            expected = int(fixture.existing.record(backend, 1)['forward_p50_ns'])
            self.assertEqual(cell['cells'][backend]['mean_of_two_invocation_p50_ns'], expected)
            self.assertEqual(cell['latency_ratios']['ordinary-1ms_over_' + backend], 100 / expected)
            self.assertEqual(cell['latency_ratios']['ceiling-25us_over_' + backend], 20 / expected)

    def test_missing_or_extra_trial_fails(self):
        rows = fixture.receipts()
        for altered in (rows[:-1], rows + [rows[0]]):
            with self.assertRaises(ValueError):
                compare(altered)

    def test_duplicate_order_or_receipt_fails(self):
        rows = fixture.receipts()
        duplicate = copy.deepcopy(rows)
        duplicate[-1] = duplicate[0]
        bad_receipt = copy.deepcopy(rows)
        bad_receipt[1]['execution_receipt_sha256'] = bad_receipt[0]['execution_receipt_sha256']
        for altered in (duplicate, bad_receipt, [rows[1], rows[0], *rows[2:]]):
            with self.assertRaises(ValueError):
                compare(altered)

    def test_profiled_as_ordinary_rejected_not_reclassified(self):
        rows = fixture.receipts()
        rows[0]['stdout'] = rows[1]['stdout']
        with self.assertRaises(ValueError):
            compare(rows)
        rows = fixture.receipts()
        rows[1]['mode'] = 'ordinary'
        with self.assertRaises(ValueError):
            compare(rows)

    def test_wrong_policy_rejected_not_pooled(self):
        rows = fixture.receipts()
        rows[0]['stdout'] = rows[2]['stdout']
        with self.assertRaises(ValueError):
            compare(rows)

    def test_missing_diagnostics_counted_without_zero_substitution(self):
        rows = fixture.receipts()
        selected = next(row for row in rows if row['depth'] == 1 and row['cadence'] == 'ordinary-1ms' and row['mode'] == 'profiled')
        def edit(sample):
            if sample['direction'] == 'forward' and sample['index'] == '0':
                sample.update(cpu_status='unavailable', counters_status='invalid', prepare_ns='none')
                for key in (*diagnostic.CPU, *diagnostic.COUNTERS):
                    sample[key] = 'none'
        change_samples(selected, edit)
        result = compare(rows)
        cell = next(row for row in result['profiled'] if row['depth'] == 1 and row['direction'] == 'forward'
                    and row['cadence'] == 'ordinary-1ms')
        for key in (*diagnostic.CPU, *diagnostic.COUNTERS, 'prepare_ns'):
            self.assertEqual(cell['distributions'][key]['missing'], 1)
            self.assertEqual(cell['distributions'][key]['available'], 19)
            self.assertEqual(result['profiled_missing_by_field'][key], 1)
        self.assertEqual(cell['cpu_status_counts'], {'available': 19, 'unavailable': 1})
        self.assertEqual(cell['counter_histograms']['spins']['None'], 1)

    def test_no_parity_or_scheduler_authority_and_explicit_experiment_baseline(self):
        result = compare(fixture.receipts())
        self.assertEqual(result['kfd_baseline'], 'separately-named-ordinary-1ms-experiment-method')
        for key in ('performance_acceptance', 'aggregate_parity_assertion', 'aggregate_speedup_assertion',
                    'formal_refinement', 'engine_matching', 'device_completion_timing',
                    'exclusive_reservation', 'unchanged_kfd_series_timed'):
            self.assertIs(result[key], False)

    def test_distribution_missing_and_nearest_rank_conventions(self):
        result = comparison.distribution([None, 9, 1, 3, 5])
        self.assertEqual(result, {'samples': 5, 'available': 4, 'missing': 1,
            'min': 1, 'max': 9, 'median': 4, 'p95_nearest_rank': 9, 'mean': 4.5})
        self.assertEqual(comparison.distribution([None, None])['missing'], 2)
        self.assertIsNone(comparison.distribution([None, None])['median'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
