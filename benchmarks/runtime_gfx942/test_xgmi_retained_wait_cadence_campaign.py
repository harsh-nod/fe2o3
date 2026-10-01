#!/usr/bin/env python3
"""Synthetic native36 planning/replay; no native or external commands."""

import copy
from pathlib import Path
import sys
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import test_xgmi_peer_series_campaign as existing
import test_xgmi_retained_wait_cadence as fixture
import xgmi_peer_series_campaign as ordinary
import xgmi_retained_wait_cadence_campaign as campaign


def receipts():
    inputs = existing.inputs()
    plan = campaign.trial_specs(**inputs)
    references = existing.receipts(inputs)
    result = []
    for spec in plan['trials']:
        if spec['backend'] == campaign.BACKEND:
            rows, _expected = fixture.fixture(spec['cadence'], spec['mode'])
            rows[0].update(unique_ids=','.join(plan['unique_ids']), gpu_ids='101,202',
                           forward_engine='2', reverse_engine='3', depth=str(spec['depth']))
            if spec['mode'] == 'profiled':
                for row in rows[1:]:
                    row['observations'] = str(spec['depth'])
            stdout = fixture.encode(rows)
        else:
            stdout = next(row['stdout'] for row in references
                          if row['backend'] == spec['backend'] and row['depth'] == spec['depth'])
        result.append({**copy.deepcopy(spec), 'returncode': 0, 'stdout': stdout, 'stderr': b'',
                       'execution_receipt_sha256': existing.digest(spec['name'])})
    return result


def replay(rows):
    return campaign.replay_campaign(rows, environment_after=existing.environment(), **existing.inputs())


class PlanTests(unittest.TestCase):
    def test_exact_thirty_six_palindrome_and_experimental_baseline(self):
        plan = campaign.trial_specs(**existing.inputs())
        self.assertEqual(len(plan['trials']), 36)
        expected = [('kfd-cadence', 'ordinary-1ms', 'ordinary'), ('kfd-cadence', 'ordinary-1ms', 'profiled'),
                    ('kfd-cadence', 'ceiling-25us', 'ordinary'), ('kfd-cadence', 'ceiling-25us', 'profiled'),
                    ('hsa', None, 'ordinary'), ('hip', None, 'ordinary')]
        expected += expected[::-1]
        self.assertEqual([(row['backend'], row['cadence'], row['mode']) for row in plan['trials']], expected * 3)
        self.assertEqual(len({row['name'] for row in plan['trials']}), 36)
        self.assertEqual(plan['kfd_baseline'], 'separately-named-ordinary-1ms-experiment-method')
        self.assertFalse(plan['unchanged_kfd_series_timed'])
        self.assertEqual(plan['query_binary_role'], 'unchanged-kfd-series-query-only')
        self.assertFalse(plan['performance_acceptance'])

    def test_same_kfd_elf_closed_policy_modes_and_visibility(self):
        for row in campaign.trial_specs(**existing.inputs())['trials']:
            if row['backend'] == campaign.BACKEND:
                self.assertEqual(row['binary'], 'kfd-retained-cadence')
                self.assertEqual(row['arguments'], ['0xb7baafd0fb173d8e', '0x10a254ce4987e716', str(row['depth']),
                    row['cadence'], row['mode'], '--reviewed-mi300x-retained-cadence-experiment'])
                self.assertEqual(row['environment_overrides'], {'HSA_XNACK': '0'})
            self.assertEqual(row['clear_environment'], ordinary._CLEAR_ENVIRONMENT)

    def test_hip_hsa_commands_are_unchanged_subsequence_without_fabricating_old_kfd_rows(self):
        reference = ordinary.trial_specs(**existing.inputs())
        plan = campaign.trial_specs(**existing.inputs())
        old = [row for row in reference['trials'] if row['backend'] != 'kfd']
        new = [row for row in plan['trials'] if row['backend'] != campaign.BACKEND]
        self.assertEqual(len(old), len(new))
        for original, current in zip(old, new, strict=True):
            self.assertEqual({key: value for key, value in original.items() if key != 'name'},
                             {key: value for key, value in current.items() if key not in ('name', 'cadence', 'mode')})
        self.assertFalse(any(row['backend'] == 'kfd' for row in plan['trials']))

    def test_closed_producer_controls_and_reviewed_admission_cannot_drift(self):
        for key, value in (('copy_bytes', 1024), ('warmups', 1), ('samples', 11), ('samples', True)):
            with self.assertRaises(ValueError):
                campaign.trial_specs(**existing.inputs(), **{key: value})
        inputs = existing.inputs()
        inputs['environment_before']['identity']['amdgpu_version'] = 'other'
        with self.assertRaises(ValueError):
            campaign.trial_specs(**inputs)
        inputs = existing.inputs()
        inputs['admission']['visibility']['hip'].reverse()
        with self.assertRaises(ValueError):
            campaign.trial_specs(**inputs)


class ReplayTests(unittest.TestCase):
    def test_twenty_four_ordinary_twelve_profiled_and_both_policy_rosters(self):
        result = replay(receipts())
        self.assertEqual((len(result['trials']), len(result['ordinary']), len(result['profiled'])), (36, 24, 12))
        self.assertEqual(sum(len(row['fields']['samples']) for row in result['profiled']), 240)
        self.assertEqual(sum(len(row['fields']['samples']) for row in result['ordinary'] if row['backend'] == campaign.BACKEND), 240)
        for policy in ('ordinary-1ms', 'ceiling-25us'):
            self.assertEqual(sum(row['cadence'] == policy for row in result['ordinary']), 6)
            self.assertEqual(sum(row['cadence'] == policy for row in result['profiled']), 6)
        for key in ('performance_acceptance', 'device_completion_timing', 'formal_refinement',
                    'engine_matching', 'unchanged_kfd_series_timed'):
            self.assertFalse(result[key])

    def test_missing_reordered_duplicate_or_unknown_rows_fail(self):
        good = receipts()
        bad = [good[:-1], good + [good[0]], [good[1], good[0], *good[2:]]]
        duplicate = copy.deepcopy(good)
        duplicate[1]['execution_receipt_sha256'] = duplicate[0]['execution_receipt_sha256']
        bad.append(duplicate)
        unknown = copy.deepcopy(good)
        unknown[0]['unknown'] = None
        bad.append(unknown)
        for rows in bad:
            with self.assertRaises(ValueError):
                replay(rows)

    def test_policy_mode_and_old_schema_substitution_fail(self):
        good = receipts()
        for source in (1, 2, 3, 4, 5):
            rows = copy.deepcopy(good)
            rows[0]['stdout'] = good[source]['stdout']
            with self.assertRaises(ValueError):
                replay(rows)
        rows = copy.deepcopy(good)
        rows[0]['stdout'] = existing.receipts()[0]['stdout']
        with self.assertRaises(ValueError):
            replay(rows)

    def test_both_endpoint_ids_engines_canaries_and_teardown_are_bound(self):
        changes = ((b'unique_ids=b7baafd0fb173d8e,10a254ce4987e716', b'unique_ids=10a254ce4987e716,b7baafd0fb173d8e'),
                   (b'gpu_ids=101,202', b'gpu_ids=202,101'), (b'forward_engine=2', b'forward_engine=3'),
                   (b'reverse_engine=3', b'reverse_engine=2'), (b'canaries=pass', b'canaries=fail'),
                   (b'teardown=explicit', b'teardown=missing'))
        for before, after in changes:
            rows = receipts()
            self.assertIn(before, rows[0]['stdout'])
            rows[0]['stdout'] = rows[0]['stdout'].replace(before, after)
            with self.assertRaises(ValueError):
                replay(rows)

    def test_command_status_environment_and_output_errors_are_not_reinterpreted(self):
        for key, value in (('returncode', True), ('stderr', b'warning\n'), ('arguments', []),
                           ('binary', 'kfd-series'), ('stdout', b'malformed\n'), ('cadence', 'custom'),
                           ('mode', 'ordinary'), ('environment_overrides', {})):
            rows = receipts()
            rows[1][key] = value
            with self.assertRaises(ValueError):
                replay(rows)


if __name__ == '__main__':
    unittest.main(verbosity=2)
