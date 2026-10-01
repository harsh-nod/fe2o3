#!/usr/bin/env python3
"""Descriptive native36 comparison composed from the existing strict replay."""

from collections import Counter
import math
import statistics

import xgmi_retained_host_diagnostic as diagnostic
import xgmi_retained_wait_cadence_campaign as campaign

POLICIES = ('ordinary-1ms', 'ceiling-25us')
CELLS = (*POLICIES, 'hsa', 'hip')
DIRECTIONS = ('forward', 'reverse')
METRICS = ('elapsed_ns', *diagnostic.TIMES, *diagnostic.COUNTERS, *diagnostic.CPU)


def distribution(values):
    present = sorted(value for value in values if value is not None)
    return {'samples': len(values), 'available': len(present), 'missing': len(values) - len(present),
            'min': present[0] if present else None, 'max': present[-1] if present else None,
            'median': statistics.median(present) if present else None,
            'p95_nearest_rank': present[math.ceil(0.95 * len(present)) - 1] if present else None,
            'mean': statistics.mean(present) if present else None}


def _identity(row):
    return {key: row[key] for key in ('name', 'stdout_sha256', 'execution_receipt_sha256')}


def compare(records, *, environment_after, **inputs):
    # The planner checks every command, receipt, policy, mode and raw sample.
    replay = campaign.replay_campaign(records, environment_after=environment_after, **inputs)
    ordinary, profiled, all_profiled = [], [], []
    for depth in campaign.ordinary.DEPTHS:
        for direction in DIRECTIONS:
            cells = {}
            for label in CELLS:
                rows = [row for row in replay['ordinary'] if row['depth'] == depth
                        and (row['cadence'] if row['backend'] == campaign.BACKEND else row['backend']) == label]
                diagnostic.need(len(rows) == 2, 'two ordinary invocations per policy/backend/depth/direction')
                invocations = []
                for row in rows:
                    fields = row['fields']
                    if row['backend'] == campaign.BACKEND:
                        values = sorted(sample['elapsed_ns'] for sample in fields['samples']
                                        if sample['direction'] == direction)
                        diagnostic.need(len(values) == 10, 'ten validated ordinary KFD samples')
                        p50 = values[4]
                        source = 'nearest-rank-fifth-of-ten-raw-samples'
                    else:
                        p50 = int(fields[direction + '_p50_ns'])
                        source = 'validated-producer-reported-p50'
                    invocations.append({**_identity(row), 'p50_ns': p50, 'p50_source': source})
                cells[label] = {'invocations': invocations,
                    'mean_of_two_invocation_p50_ns': statistics.mean(row['p50_ns'] for row in invocations)}
            means = {key: cell['mean_of_two_invocation_p50_ns'] for key, cell in cells.items()}
            ratios = {'ceiling-25us_over_ordinary-1ms': means['ceiling-25us'] / means['ordinary-1ms']}
            ratios.update({policy + '_over_' + backend: means[policy] / means[backend]
                           for policy in POLICIES for backend in ('hsa', 'hip')})
            ordinary.append({'depth': depth, 'direction': direction, 'cells': cells, 'latency_ratios': ratios})

            for policy in POLICIES:
                rows = [row for row in replay['profiled'] if row['depth'] == depth and row['cadence'] == policy]
                diagnostic.need(len(rows) == 2, 'two profiled invocations per policy/depth/direction')
                samples, invocations = [], []
                for row in rows:
                    fields = row['fields']
                    current = [sample for sample in fields['samples'] if sample['direction'] == direction]
                    diagnostic.need(len(current) == 10, 'ten validated profiled samples per invocation/direction')
                    samples.extend(current)
                    invocations.append({**_identity(row),
                        'elapsed_ns': distribution([sample['elapsed_ns'] for sample in current]),
                        'scope_entry_ns': int(fields['summary'][direction + '_entry_ns']),
                        'scope_finish_ns': int(fields['summary'][direction + '_finish_ns'])})
                diagnostic.need(len(samples) == 20, 'twenty instrumented samples per policy/depth/direction')
                profiled.append({'cadence': policy, 'depth': depth, 'direction': direction,
                    'timing': 'instrumented-host-only', 'invocations': invocations,
                    'distributions': {key: distribution([sample[key] for sample in samples]) for key in METRICS},
                    'counter_histograms': {key: dict(sorted(Counter(str(sample[key]) for sample in samples).items()))
                        for key in (*diagnostic.COUNTERS, 'voluntary_switches', 'involuntary_switches')},
                    'cpu_status_counts': dict(sorted(Counter(sample['cpu_status'] for sample in samples).items())),
                    'counters_status_counts': dict(sorted(Counter(sample['counters_status'] for sample in samples).items()))})
                all_profiled.extend(samples)
    return {'schema': 'fe2o3.xgmi-retained-wait-cadence-descriptive-comparison.v1',
        'plan_sha256': replay['plan_sha256'], 'trials': replay['trials'],
        'ordinary_trial_count': 24, 'profiled_trial_count': 12,
        'ordinary': ordinary, 'profiled': profiled, 'profiled_sample_count': len(all_profiled),
        'profiled_missing_by_field': {key: sum(sample[key] is None for sample in all_profiled) for key in METRICS},
        'kfd_baseline': replay['kfd_baseline'], 'unchanged_kfd_series_timed': False,
        'conventions': {
            'ordinary': 'Mean of two invocation p50s per policy/backend/depth/direction, not a pooled median.',
            'kfd_p50': 'Nearest rank: fifth sorted value of ten raw samples, not the mean of fifth and sixth.',
            'ratios': 'Numerator latency divided by denominator latency in the same depth/direction cell.',
            'profiled': 'Separate distributions of twenty instrumented samples per policy/depth/direction.',
            'median': 'Mean of the two central available values for even diagnostic sample counts.',
            'missing': 'Missing values remain None, are counted explicitly, and are never replaced with zero.',
            'phase_intervals': 'Independent distributions are not summed; thread CPU brackets scan only.',
            'sleep': 'Requested sleep and wall-minus-thread-CPU are not measured avoidable latency.',
        },
        'instrumentation_perturbs_timing_and_readiness': True,
        'performance_acceptance': False, 'aggregate_parity_assertion': False,
        'aggregate_speedup_assertion': False, 'device_completion_timing': False,
        'formal_refinement': False, 'engine_matching': False, 'exclusive_reservation': False}
