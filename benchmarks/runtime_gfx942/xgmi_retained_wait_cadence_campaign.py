#!/usr/bin/env python3
"""Pure native36 cadence planning/replay; no launcher or admission authority."""

import copy
import hashlib

import xgmi_peer_series_campaign as ordinary
import xgmi_retained_wait_cadence as cadence


BACKEND = 'kfd-cadence'
BINARY = 'kfd-retained-cadence'
HALF = ((BACKEND, 'ordinary-1ms', 'ordinary'), (BACKEND, 'ordinary-1ms', 'profiled'),
        (BACKEND, 'ceiling-25us', 'ordinary'), (BACKEND, 'ceiling-25us', 'profiled'),
        ('hsa', None, 'ordinary'), ('hip', None, 'ordinary'))
ORDER = HALF + HALF[::-1]


def trial_specs(**inputs):
    reference = ordinary.trial_specs(**inputs)
    if (reference['copy_bytes'], reference['warmups'], reference['samples']) != (1048576, 2, 10):
        raise ValueError('cadence experiment requires the exact closed size/warmup/sample controls')
    trials = []
    for depth in ordinary.DEPTHS:
        block = [row for row in reference['trials'] if row['depth'] == depth]
        if len(block) != 6 or tuple(row['backend'] for row in block) != ordinary.ORDER:
            raise ValueError('unchanged reference planner controls required')
        for position, (backend, policy, mode) in enumerate(ORDER, 1):
            template = next(row for row in block if row['backend'] == ('kfd' if backend == BACKEND else backend))
            trial = copy.deepcopy(template)
            trial.update(name=f'd{depth:02d}-t{position:02d}-{backend}', backend=backend, cadence=policy, mode=mode)
            if backend == BACKEND:
                trial.update(binary=BINARY, arguments=[*('0x' + uid for uid in reference['unique_ids']), str(depth),
                    policy, mode, '--reviewed-mi300x-retained-cadence-experiment'])
            trials.append(trial)
    return {
        **{key: value for key, value in reference.items() if key not in ('schema', 'trials')},
        'schema': 'fe2o3.xgmi-retained-wait-cadence-plan.v1',
        'reference_plan_sha256': ordinary._json_digest(reference),
        'trials': trials, 'ordinary_trials': 24, 'profiled_trials': 12,
        'kfd_baseline': 'separately-named-ordinary-1ms-experiment-method',
        'unchanged_kfd_series_timed': False, 'query_binary_role': 'unchanged-kfd-series-query-only',
        'diagnostic_authority': 'none', 'profiled_timing_scope': 'instrumented-host-only',
        'profiled_completion_offsets': 'host-observed-not-device-timestamps',
        'instrumentation_perturbs_timing_and_readiness': True, 'performance_acceptance': False,
    }


def validate_trial(plan, spec, record):
    ordinary._roster(record, (*spec, 'returncode', 'stdout', 'stderr', 'execution_receipt_sha256'), 'cadence experiment receipt')
    for key, expected in spec.items():
        if record[key] != expected or type(record[key]) is not type(expected):
            raise ValueError('trial command/order mismatch: ' + key)
    if type(record['returncode']) is not int or record['returncode'] != 0 or \
            type(record['stdout']) is not bytes or type(record['stderr']) is not bytes or record['stderr']:
        raise ValueError('unsuccessful or malformed trial output')
    ordinary._digest(record['execution_receipt_sha256'])
    if spec['backend'] == BACKEND:
        return cadence.parse(record['stdout'], {
            'unique_ids': plan['unique_ids'], 'gpu_ids': plan['kfd_gpu_ids'], 'engines': plan['kfd_engines'],
            'depth': spec['depth'], 'cadence': spec['cadence'], 'mode': spec['mode'],
        })
    if spec['backend'] not in ('hip', 'hsa') or spec['cadence'] is not None or spec['mode'] != 'ordinary':
        raise ValueError('only unchanged HIP/HSA rows are external ordinary references')
    return ordinary.parse_result(record['stdout'], backend=spec['backend'],
        unique_ids=['0x' + uid for uid in plan['unique_ids']], copy_bytes=plan['copy_bytes'],
        depth=spec['depth'], warmups=plan['warmups'], samples=plan['samples'])


def replay_campaign(records, *, environment_after, **inputs):
    plan = trial_specs(**inputs)
    ordinary.validate_environment(environment_after)
    if type(records) is not list or len(records) != 36:
        raise ValueError('exact complete ordered 36-trial experiment required')
    seen, profiled, ordinary_rows, joins = set(), [], [], []
    for spec, record in zip(plan['trials'], records, strict=True):
        parsed = validate_trial(plan, spec, record)
        digest = record['execution_receipt_sha256']
        if digest in seen:
            raise ValueError('duplicate execution receipt across policies and modes')
        seen.add(digest)
        row = {'name': spec['name'], 'backend': spec['backend'], 'mode': spec['mode'], 'cadence': spec['cadence'],
               'depth': spec['depth'], 'stdout_sha256': hashlib.sha256(record['stdout']).hexdigest(),
               'execution_receipt_sha256': digest}
        joins.append(row)
        (profiled if spec['mode'] == 'profiled' else ordinary_rows).append({**row, 'fields': parsed})
    if len(ordinary_rows) != 24 or len(profiled) != 12:
        raise ValueError('exact ordinary/profiled separation')
    return {
        'schema': 'fe2o3.xgmi-retained-wait-cadence-consistency-replay.v1',
        'claim': 'input-and-receipt-consistency-only', 'plan_sha256': ordinary._json_digest(plan),
        'ordinary': ordinary_rows, 'profiled': profiled, 'trials': joins,
        'kfd_baseline': plan['kfd_baseline'], 'unchanged_kfd_series_timed': False,
        'diagnostic_authority': 'none', 'instrumentation_perturbs_timing_and_readiness': True,
        'performance_acceptance': False, 'device_completion_timing': False,
        'formal_refinement': False, 'engine_matching': False,
    }
