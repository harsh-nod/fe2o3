#!/usr/bin/env python3
"""Pure 24-trial diagnostic planning/replay; no launcher or native admission."""

import copy
import hashlib

import xgmi_peer_series_campaign as ordinary
import xgmi_retained_host_diagnostic as diagnostic


BACKEND = 'kfd-profiled'
BINARY = 'kfd-retained-host-diagnostic'
ORDER = ('kfd', BACKEND, 'hsa', 'hip', 'hip', 'hsa', BACKEND, 'kfd')


def trial_specs(**inputs):
    baseline = ordinary.trial_specs(**inputs)
    if (baseline['copy_bytes'], baseline['warmups'], baseline['samples']) != (1048576, 2, 10):
        raise ValueError('diagnostic experiment requires the exact closed size/warmup/sample controls')
    trials = []
    for depth in ordinary.DEPTHS:
        block = [row for row in baseline['trials'] if row['depth'] == depth]
        if len(block) != 6 or tuple(row['backend'] for row in block) != ordinary.ORDER:
            raise ValueError('unchanged ordinary six-trial block required')
        def profiled(position):
            return {
                'name': f'd{depth:02d}-profiled-{position:02d}', 'backend': BACKEND,
                'depth': depth, 'binary': BINARY,
                'arguments': [*('0x' + uid for uid in baseline['unique_ids']), str(depth),
                              '--reviewed-mi300x-retained-host-diagnostic'],
                'clear_environment': list(block[0]['clear_environment']),
                'environment_overrides': copy.deepcopy(block[0]['environment_overrides']),
            }
        trials.extend((block[0], profiled(1), *block[1:5], profiled(2), block[5]))
    return {
        **{key: value for key, value in baseline.items() if key not in ('schema', 'trials')},
        'schema': 'fe2o3.xgmi-retained-host-diagnostic-plan.v1',
        'ordinary_plan_sha256': ordinary._json_digest(baseline),
        'ordinary_subsequence_names': [row['name'] for row in baseline['trials']],
        'trials': trials,
        'diagnostic_authority': 'none',
        'diagnostic_timing_scope': 'instrumented-host-only',
        'diagnostic_completion_offsets': 'host-observed-not-device-timestamps',
        'instrumentation_perturbs_timing_and_readiness': True,
        'performance_acceptance': False,
    }


def validate_trial(plan, spec, record):
    ordinary._roster(record, (*spec, 'returncode', 'stdout', 'stderr', 'execution_receipt_sha256'), 'diagnostic experiment receipt')
    for key, expected in spec.items():
        if record[key] != expected or type(record[key]) is not type(expected):
            raise ValueError('trial command/order mismatch: ' + key)
    if type(record['returncode']) is not int or record['returncode'] != 0 or \
            type(record['stdout']) is not bytes or type(record['stderr']) is not bytes or record['stderr']:
        raise ValueError('unsuccessful or malformed trial output')
    ordinary._digest(record['execution_receipt_sha256'])
    if spec['backend'] == BACKEND:
        return diagnostic.parse(record['stdout'], {
            'unique_ids': plan['unique_ids'], 'gpu_ids': plan['kfd_gpu_ids'],
            'engines': plan['kfd_engines'], 'depth': spec['depth'],
        })
    options = {'kfd_gpu_ids': plan['kfd_gpu_ids'], 'kfd_engines': plan['kfd_engines']} \
        if spec['backend'] == 'kfd' else {}
    return ordinary.parse_result(record['stdout'], backend=spec['backend'],
        unique_ids=['0x' + uid for uid in plan['unique_ids']], copy_bytes=plan['copy_bytes'],
        depth=spec['depth'], warmups=plan['warmups'], samples=plan['samples'], **options)


def replay_campaign(records, *, environment_after, **inputs):
    plan = trial_specs(**inputs)
    ordinary.validate_environment(environment_after)
    if type(records) is not list or len(records) != 24:
        raise ValueError('exact complete ordered 24-trial experiment required')
    seen, profiled, ordinary_records, joins = set(), [], [], []
    for spec, record in zip(plan['trials'], records, strict=True):
        parsed = validate_trial(plan, spec, record)
        digest = record['execution_receipt_sha256']
        if digest in seen:
            raise ValueError('duplicate execution receipt across ordinary and diagnostic trials')
        seen.add(digest)
        joins.append({'name': spec['name'], 'kind': 'profiled' if spec['backend'] == BACKEND else 'ordinary',
                      'stdout_sha256': hashlib.sha256(record['stdout']).hexdigest(),
                      'execution_receipt_sha256': digest})
        if spec['backend'] == BACKEND:
            profiled.append({'name': spec['name'], 'fields': parsed,
                             'stdout_sha256': hashlib.sha256(record['stdout']).hexdigest(),
                             'execution_receipt_sha256': digest})
        else:
            ordinary_records.append(record)
    baseline = ordinary.replay_campaign(ordinary_records, environment_after=environment_after, **inputs)
    return {
        'schema': 'fe2o3.xgmi-retained-host-diagnostic-consistency-replay.v1',
        'claim': 'input-and-receipt-consistency-only', 'plan_sha256': ordinary._json_digest(plan),
        'ordinary': baseline, 'profiled': profiled, 'trials': joins,
        'diagnostic_authority': 'none', 'instrumentation_perturbs_timing_and_readiness': True,
        'performance_acceptance': False, 'device_completion_timing': False,
        'formal_refinement': False, 'engine_matching': False,
    }
