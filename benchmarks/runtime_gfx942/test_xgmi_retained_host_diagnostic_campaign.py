#!/usr/bin/env python3
"""Synthetic planning/replay only; no native jobs or external commands."""

import copy
import importlib.util
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'benchmarks/runtime_gfx942'))
import test_xgmi_peer_series_campaign as existing
import test_xgmi_retained_host_diagnostic as profile_fixture
import xgmi_peer_series_campaign as ordinary

SPEC = importlib.util.spec_from_file_location('retained_host_diagnostic_campaign_test', Path(__file__).with_name('xgmi_retained_host_diagnostic_campaign.py'))
campaign = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(campaign)


def receipts():
    inputs = existing.inputs()
    plan = campaign.trial_specs(**inputs)
    normal = {row['name']: row for row in existing.receipts(inputs)}
    result = []
    for spec in plan['trials']:
        if spec['backend'] != campaign.BACKEND:
            result.append(normal[spec['name']])
            continue
        rows = profile_fixture.fixture()
        rows[0].update(unique_ids=','.join(plan['unique_ids']), gpu_ids='101,202',
                       forward_engine='2', reverse_engine='3', depth=str(spec['depth']))
        for row in rows[1:]:
            row['observations'] = str(spec['depth'])
        result.append({**copy.deepcopy(spec), 'returncode': 0,
                       'stdout': profile_fixture.encode(rows), 'stderr': b'',
                       'execution_receipt_sha256': existing.digest(spec['name'])})
    return result


class PlanTests(unittest.TestCase):
    def test_twenty_four_trials_preserve_every_ordinary_spec_as_exact_subsequence(self):
        plan = campaign.trial_specs(**existing.inputs())
        baseline = ordinary.trial_specs(**existing.inputs())
        self.assertEqual(len(plan['trials']), 24)
        self.assertEqual([row['backend'] for row in plan['trials']], list(campaign.ORDER) * 3)
        self.assertEqual([row for row in plan['trials'] if row['backend'] != campaign.BACKEND], baseline['trials'])
        self.assertEqual(plan['ordinary_plan_sha256'], ordinary._json_digest(baseline))
        self.assertEqual(len({row['name'] for row in plan['trials']}), 24)
        self.assertFalse(plan['performance_acceptance'])
        self.assertTrue(plan['instrumentation_perturbs_timing_and_readiness'])

    def test_profiled_commands_are_distinct_with_same_visibility_scrubbing(self):
        for row in campaign.trial_specs(**existing.inputs())['trials']:
            if row['backend'] == campaign.BACKEND:
                self.assertEqual(row['binary'], 'kfd-retained-host-diagnostic')
                self.assertEqual(row['arguments'], ['0xb7baafd0fb173d8e', '0x10a254ce4987e716', str(row['depth']),
                                                    '--reviewed-mi300x-retained-host-diagnostic'])
                self.assertEqual(row['clear_environment'], ordinary._CLEAR_ENVIRONMENT)
                self.assertEqual(row['environment_overrides'], {'HSA_XNACK': '0'})

    def test_diagnostic_controls_cannot_drift_from_closed_producer(self):
        for key, value in (('copy_bytes', 1024), ('warmups', 1), ('samples', 11), ('samples', True)):
            with self.assertRaises(ValueError):
                campaign.trial_specs(**existing.inputs(), **{key: value})

    def test_unreviewed_environment_or_mismatched_visibility_remains_rejected(self):
        inputs = existing.inputs()
        inputs['environment_before']['identity']['amdgpu_version'] = 'other'
        with self.assertRaises(ValueError):
            campaign.trial_specs(**inputs)
        inputs = existing.inputs()
        inputs['admission']['visibility']['hip'].reverse()
        with self.assertRaises(ValueError):
            campaign.trial_specs(**inputs)


class ReplayTests(unittest.TestCase):
    def test_replay_keeps_eighteen_ordinary_and_six_profiled_outputs_separate(self):
        result = campaign.replay_campaign(receipts(), environment_after=existing.environment(), **existing.inputs())
        self.assertEqual(result['ordinary'], existing.replay(existing.receipts()))
        self.assertEqual(len(result['profiled']), 6)
        self.assertEqual(sum(len(row['fields']['samples']) for row in result['profiled']), 120)
        for key in ('performance_acceptance', 'device_completion_timing', 'formal_refinement', 'engine_matching'):
            self.assertFalse(result[key])

    def test_missing_reordered_duplicate_and_substituted_results_fail(self):
        good = receipts()
        bad = [good[:-1], good + [good[0]], [good[1], good[0], *good[2:]]]
        duplicate = copy.deepcopy(good)
        duplicate[1]['execution_receipt_sha256'] = duplicate[0]['execution_receipt_sha256']
        bad.append(duplicate)
        substitute = copy.deepcopy(good)
        substitute[1]['stdout'] = substitute[0]['stdout']
        bad.append(substitute)
        for rows in bad:
            with self.assertRaises(ValueError):
                campaign.replay_campaign(rows, environment_after=existing.environment(), **existing.inputs())

    def test_command_identity_errors_and_stderr_are_never_reinterpreted_as_diagnostic_success(self):
        for key, value in (('returncode', True), ('stderr', b'warning\n'), ('arguments', []),
                           ('binary', 'kfd-series'), ('stdout', b'malformed\n')):
            rows = receipts()
            rows[1][key] = value
            with self.assertRaises(ValueError):
                campaign.replay_campaign(rows, environment_after=existing.environment(), **existing.inputs())


if __name__ == '__main__':
    unittest.main(verbosity=2)
