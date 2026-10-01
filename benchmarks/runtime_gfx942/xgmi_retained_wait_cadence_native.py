#!/usr/bin/env python3
"""Separate native36 cadence experiment; no parity or timing authority."""

import base64
import hashlib
import os
from pathlib import Path
import resource
import shutil
import signal
import stat
import sys
import time

HERE = Path(__file__).resolve().parent
KFD_TARGETS = {
    'kfd': 'kfd-sdma-xgmi-peer-benchmark',
    'kfd-cadence': 'kfd-xgmi-retained-cadence-experiment',
}
BINARIES = {'kfd': 'kfd-series', 'hsa': 'hsa-series', 'hip': 'hip-series',
            'kfd-cadence': 'kfd-retained-cadence'}


def load_helpers():
    if not sys.flags.isolated or not sys.dont_write_bytecode:
        raise RuntimeError('use python3 -I -B')
    sys.path.insert(0, str(HERE))
    import xgmi_peer_series_native as native
    import xgmi_retained_wait_cadence_campaign as planner
    hot, ordinary, observations = native.load_helpers()
    return native, hot, ordinary, observations, planner


def kfd_build_spec():
    return ('build-kfd-pair', ['cargo', 'build', '--frozen', '--release', '--jobs', '2',
            '-p', 'fe2o3-kfd', '--features', 'live-validation,hardware-diagnostic',
            '--example', KFD_TARGETS['kfd'], '--example', KFD_TARGETS['kfd-cadence'],
            '--message-format=json'], 1200)


def build_specs(native, binaries):
    _, hip, hsa = native.build_specs(binaries)
    return [kfd_build_spec(), hip, hsa]


def fresh_kfd_artifacts(raw, work, source, hot):
    hot.need(raw.endswith(b'\n'), 'complete fresh Cargo artifact stream')
    rows = [hot.parse_json(line) for line in raw.splitlines()]
    hot.need(rows and rows[-1] == {'reason': 'build-finished', 'success': True}, 'successful fresh paired build')
    hot.need(sum(row.get('reason') == 'build-finished' for row in rows) == 1, 'one paired build completion')
    result = {}
    for backend, target in KFD_TARGETS.items():
        selected = [row for row in rows if row.get('reason') == 'compiler-artifact'
                    and row.get('target', {}).get('name') == target and row.get('executable') is not None]
        hot.need(len(selected) == 1, 'one artifact for each KFD producer')
        row = selected[0]
        hot.need(row.get('fresh') is False and row.get('profile', {}).get('test') is False
                 and row['target'].get('kind') == ['example']
                 and row.get('features') == ['default', 'hardware-diagnostic', 'live-validation']
                 and row.get('manifest_path') == str(source / 'crates/fe2o3-kfd/Cargo.toml'), 'fresh KFD example target')
        path = Path(row['executable'])
        hot.need(path == work / 'target/release/examples' / target and path.resolve(strict=True) == path,
                 'canonical fresh target executable')
        info = path.stat()
        hot.need(stat.S_ISREG(info.st_mode) and info.st_mode & 0o111, 'ordinary executable artifact')
        with path.open('rb') as stream:
            hot.need(stream.read(4) == b'\x7fELF', 'KFD ELF artifact')
        result[backend] = {'path': str(path), 'sha256': hot.sha(path), 'cargo_record': row}
    return result


def run_trials(plan, records, invoke, validate, postflight):
    for spec in plan['trials']:
        failure = None
        try:
            record = invoke(spec)
            validate(plan, spec, record)
            records.append(record)
        except BaseException as error:
            failure = error
        failure = postflight(spec['name'], failure)
        if failure is not None:
            raise failure


def host_command(native, binaries, device, label, edge, cadence):
    argv = ['/usr/bin/python3', '-I', '-B', str(native.HERE / 'xgmi_peer_series_host.py'),
            '--gpu-index', str(device['physical_index']), '--observation-edge', edge]
    for role in ('kfd', 'hsa', 'hip'):
        key = 'kfd-cadence' if role == 'kfd' and cadence else role
        argv += ['--' + role + '-binary', str(binaries / BINARIES[key])]
    return label, argv, 180


def validate_host(value, ordinary, expected, device, cadence, hot):
    hot.need(value['schema'] == 'fe2o3.xgmi-peer-series-host-observation.v1', 'host observation schema')
    ordinary.validate_environment(value['environment'])
    context = value['context']
    hot.need(context['unique_id'] == '0x' + device['unique_id'] and context['pci_bdf'] == device['pci_bdf'],
             'host collector physical join')
    for role in ('kfd', 'hsa', 'hip'):
        key = 'kfd-cadence' if role == 'kfd' and cadence else role
        hot.need(context[role + '_binary_sha256'] == expected[key], 'independently collected ELF identity: ' + key)


def run_campaign(args, native, hot, ordinary, observations, planner):
    os.umask(0o077)
    output = native.fresh_output(args.output)
    work, binaries = output / 'work', output / 'binaries'
    work.mkdir(mode=0o700)
    binaries.mkdir(mode=0o700)
    (work / 'tmp').mkdir(mode=0o700)
    environment = hot.environment(work)
    environment['CARGO_BUILD_JOBS'] = '2'
    rec = native.FreshRecorder(output / 'commands', native.ROOT, hot)
    namespace = os.readlink('/proc/self/ns/pid')
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    previous = {sig: signal.getsignal(sig) for sig in hot.B.MANAGED}
    failures, records = [], []
    source_before = tools_before = binaries_before = admission = incarnation = plan = inputs = None
    hosts_before, hosts_after = {}, {}

    def save(name, value):
        hot.B.write_json(output / (name + '.json'), value)

    def binary_snapshot():
        return {backend: hot.sha(binaries / name) for backend, name in BINARIES.items()}

    def observe(label):
        raw, rows, errors = [], [], []
        for device in args.devices:
            index, bdf, uid = device['physical_index'], device['pci_bdf'], '0x' + device['unique_id']
            try:
                folder = rec.run(label + '-gpu' + str(index), hot.observe_spec(label, index, bdf, uid), 100, env=environment)
                hot.need(not (folder / 'stderr').read_bytes(), 'empty physical observer stderr')
                data = (folder / 'stdout').read_bytes()
                parsed = hot.parse_endpoint(data, index, bdf, uid)
                raw.append(hashlib.sha256(data).hexdigest())
                rows.append({'physical_index': parsed['gpu_index'], 'pci_bdf': parsed['pci_bdf'],
                             'unique_id': parsed['unique_id'][2:]})
            except BaseException as error:
                errors.append(error)
        if errors:
            raise errors[0]
        return rows, observations.digest(raw)

    def query(label, physical):
        raw = {}
        for backend, command, env in native.query_specs(binaries, args.devices, environment, ordinary._CLEAR_ENVIRONMENT):
            folder = rec.run(label + '-' + backend, command, 60, env=env)
            hot.need(not (folder / 'stderr').read_bytes(), 'empty API query stderr')
            raw[backend] = (folder / 'stdout').read_bytes()
        current = observations.join_admission(physical[0], raw, inventory_sha256=physical[1])
        started = time.monotonic_ns()
        time.sleep(native.QUERY_SETTLE_SECONDS)
        save(label + '-settle', {'schema': 'fe2o3.native-query-settle.v1',
            'closed_query_stages': [label + '-' + backend for backend in ('kfd', 'hsa', 'hip')],
            'fixed_seconds': native.QUERY_SETTLE_SECONDS, 'started_monotonic_ns': started,
            'finished_monotonic_ns': time.monotonic_ns(), 'outside_timed_workloads': True,
            'polling': False, 'idle_admission': False, 'exclusive_reservation': False})
        return current

    def same_admission(current, current_incarnation):
        hot.need(current_incarnation == incarnation, 'unchanged topology incarnation')
        hot.need({key: value for key, value in current.items() if key != 'evidence_sha256'} ==
                 {key: value for key, value in admission.items() if key != 'evidence_sha256'},
                 'unchanged independently observed pair/routes/visibility')

    def host(label, edge, cadence):
        name, argv, seconds = host_command(native, binaries, args.devices[0], label, edge, cadence)
        folder = rec.run(name, argv, seconds, env=environment)
        hot.need(not (folder / 'stderr').read_bytes(), 'empty host identity stderr')
        value = hot.parse_json((folder / 'stdout').read_bytes())
        validate_host(value, ordinary, binary_snapshot(), args.devices[0], cadence, hot)
        return value

    def invoke(spec):
        name = spec['name']
        hot.need(binary_snapshot() == binaries_before, 'unchanged four trial ELFs')
        same_admission(*query(name + '-query', observe(name + '-query-before')))
        observe(name + '-before')
        env = {key: value for key, value in environment.items() if key not in spec['clear_environment']}
        env.update(spec['environment_overrides'])
        folder = rec.run(name, [str(binaries / spec['binary']), *spec['arguments']], 180, env=env)
        return {**spec, 'returncode': 0, 'stdout': (folder / 'stdout').read_bytes(),
                'stderr': (folder / 'stderr').read_bytes(), 'execution_receipt_sha256': hot.sha(folder / 'receipt.json')}

    try:
        for sig in previous:
            signal.signal(sig, hot.B.interrupted)
        source_before = native.source_snapshot(rec, 'source-before', args.commit, environment, hot)
        save('source-before', source_before)
        tools_before = native.tool_snapshot(rec, 'tools-before', environment, hot)
        save('tools-before', tools_before)
        for name, command, seconds in build_specs(native, binaries):
            folder = rec.run(name, command, seconds, env=environment)
            if name == 'build-kfd-pair':
                artifacts = fresh_kfd_artifacts((folder / 'stdout').read_bytes(), work, native.ROOT, hot)
                save('fresh-kfd-artifacts', artifacts)
                for backend, artifact in artifacts.items():
                    shutil.copy2(artifact['path'], binaries / BINARIES[backend])
                    hot.need(hot.sha(binaries / BINARIES[backend]) == artifact['sha256'], 'copied fresh KFD ELF bytes')
        binaries_before = binary_snapshot()
        save('binaries-before', binaries_before)
        for cadence, label in ((False, 'host-before'), (True, 'host-cadence-before')):
            hosts_before[cadence] = host(label, 'start', cadence)
            save(label, hosts_before[cadence])
        hot.need(hosts_before[False]['environment'] == hosts_before[True]['environment'], 'same independent reviewed host environment')
        admission, incarnation = query('admission', observe('admission-before'))
        hot.need(incarnation['boot_id'] == hosts_before[False]['context']['boot_id'] == hosts_before[True]['context']['boot_id'],
                 'both ELF dependency observations and topology share boot identity')
        save('admission', admission)
        save('incarnation', incarnation)
        save('cadence-admission-boundary', {'query_binary': str(binaries / BINARIES['kfd']),
            'query_binary_sha256': binaries_before['kfd'], 'cadence_binary_sha256': binaries_before['kfd-cadence'],
            'cadence_binary_query_performed': False,
            'cadence_uid_gpu_engine_join': 'strict-cadence-output-versus-current-query-inputs',
            'query_only_mode_added': False, 'performance_acceptance': False})
        inputs = {'physical_indices': [row['physical_index'] for row in args.devices],
                  'unique_ids': [row['unique_id'] for row in args.devices], 'admission': admission,
                  'environment_before': hosts_before[False]['environment'], **native.CONTROLS}
        plan = planner.trial_specs(**inputs)
        save('plan', plan)
        run_trials(plan, records, invoke, planner.validate_trial,
                   lambda name, failure: hot.B.settled_postflight(observe, name, failure))
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            try:
                if binaries_before is None:
                    save('admission-after-status', {'performed': False, 'status': 'not-performed',
                        'reason': 'native-binaries-not-built', 'native_acceptance': False})
                    raise RuntimeError('not performed: native binaries were not built')
                same_admission(*query('admission-after', observe('admission-after-before')))
            except BaseException as error:
                failures.append('admission-after: ' + repr(error))
            failure = hot.B.settled_postflight(observe, 'campaign-close', None)
            if failure is not None:
                failures.append('closing idle observation: ' + repr(failure))
            for cadence, label in ((False, 'host-after'), (True, 'host-cadence-after')):
                try:
                    value = host(label, 'end', cadence)
                    save(label, value)
                    hosts_after[cadence] = value
                    hot.need(cadence in hosts_before and value['continuity'] == hosts_before[cadence]['continuity'],
                             'unchanged independently collected host/loader identity for each KFD ELF')
                except BaseException as error:
                    failures.append(label + ': ' + repr(error))
            for label, action, expected in (
                ('tools-after', lambda: native.tool_snapshot(rec, 'tools-after', environment, hot), tools_before),
                ('source-after', lambda: native.source_snapshot(rec, 'source-after', args.commit, environment, hot), source_before),
                ('binaries-after', binary_snapshot, binaries_before),
            ):
                try:
                    value = action()
                    save(label, value)
                    hot.need(expected is not None and value == expected, 'closing identity: ' + label)
                except BaseException as error:
                    failures.append(label + ': ' + repr(error))
            try:
                save('fresh-census', native.fresh_census(rec, hot, namespace))
            except BaseException as error:
                failures.append('fresh census: ' + repr(error))
            try:
                serialized = [{**{key: value for key, value in row.items() if key not in ('stdout', 'stderr')},
                               **{stream + '_base64': base64.b64encode(row[stream]).decode('ascii')
                                  for stream in ('stdout', 'stderr')}} for row in records]
                save('records', serialized)
                if not failures:
                    save('replay', planner.replay_campaign(records, environment_after=hosts_after[False]['environment'], **inputs))
            except BaseException as error:
                failures.append('replay: ' + repr(error))
            try:
                native.fresh_census(rec, hot, namespace)
                shutil.rmtree(work)
            except BaseException as error:
                failures.append('private scratch cleanup: ' + repr(error))
            save('finished', {'commit': args.commit, 'failures': failures, 'native_execution': not failures,
                'exclusive_reservation': False, 'performance_acceptance': False, 'formal_refinement': False,
                'engine_matching': False, 'diagnostic_authority': 'none', 'instrumentation_perturbs_timing_and_readiness': True})
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
    hot.need(not failures, 'native diagnostic campaign failed: ' + repr(failures))


def main():
    native, hot, ordinary, observations, planner = load_helpers()
    hot.need(HERE == native.HERE and HERE.resolve(strict=True) == HERE,
             'integrated repository entrypoint required; external draft cannot execute')
    run_campaign(native.parse_arguments(), native, hot, ordinary, observations, planner)


if __name__ == '__main__':
    main()
