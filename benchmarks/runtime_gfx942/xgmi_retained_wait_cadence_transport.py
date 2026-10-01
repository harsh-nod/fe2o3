#!/usr/bin/env python3
"""Reviewed transport primitives with a separate native36 cadence entrypoint."""

import argparse
import os
from pathlib import Path
import runpy
import shlex
import signal
import sys
import time
import types

SCRIPT = Path(__file__).resolve()
RELATIVE = 'benchmarks/runtime_gfx942/xgmi_retained_wait_cadence_transport.py'
NATIVE_RELATIVE = 'benchmarks/runtime_gfx942/xgmi_retained_wait_cadence_native.py'
PLANNER_RELATIVE = 'benchmarks/runtime_gfx942/xgmi_retained_wait_cadence_campaign.py'
PARSER_RELATIVE = 'benchmarks/runtime_gfx942/xgmi_retained_wait_cadence.py'
EXTENSION_PATHS = (RELATIVE, NATIVE_RELATIVE, PLANNER_RELATIVE, PARSER_RELATIVE)


def helpers():
    if not sys.flags.isolated or not sys.dont_write_bytecode:
        raise RuntimeError('use python3 -I -B')
    sys.path.insert(0, str(SCRIPT.parent))
    import xgmi_peer_series_transport as transport
    import xgmi_retained_wait_cadence_native as experiment
    import xgmi_retained_wait_cadence_campaign as planner
    native = transport.native_module()
    hot, ordinary, _ = native.load_helpers()
    transport.need(SCRIPT.parent == native.HERE, 'integrated signed repository required; external draft cannot execute')
    return transport, native, hot, ordinary, experiment, planner


def extended_binding(root, binding, transport):
    transport.verify_source(root, binding)
    for relative in EXTENSION_PATHS:
        key = 'source/' + relative
        transport.need(key in binding['files'] and relative in binding['selected_objects'], 'signed extension source object')
        transport.need(transport.inventory_row(root / key) == binding['files'][key], 'exact extension source bytes and mode')


def public_command(owned, marker, binding):
    result = [str(owned / 'source' / NATIVE_RELATIVE), '--campaign', '--commit', marker['commit'],
              '--output', str(owned / 'campaign-1')]
    for row in binding['devices']:
        result += ['--device', str(row['physical_index']), row['unique_id'], row['pci_bdf']]
    return result


def remote_run(marker, transport, native, hot):
    os.umask(0o077)
    owned = transport.owned_path(marker)
    binding = transport.read_binding(owned, marker)
    extended_binding(owned, binding, transport)
    transport.need(SCRIPT == owned / 'source' / RELATIVE, 'bound diagnostic remote entrypoint')
    namespace = os.readlink('/proc/self/ns/pid')
    previous = {sig: signal.getsignal(sig) for sig in hot.B.MANAGED}
    prior_argv = sys.argv
    failures, closed, census_sha = [], False, None
    transport.save(owned / 'launch-started.json', {'marker': marker, 'pid': os.getpid(),
        'namespace': namespace, 'started_ns': time.time_ns()})
    command = public_command(owned, marker, binding)
    try:
        for sig in previous:
            signal.signal(sig, hot.B.interrupted)
        with transport.shared_lock() as identity:
            transport.save(owned / 'lock.json', identity)
            transport.save(owned / 'resources-before.json', transport.resource_observation(owned))
            with transport.resource_guard(owned, marker, hot) as monitor:
                try:
                    sys.argv = command
                    runpy.run_path(command[0], run_name='__main__')
                except BaseException as error:
                    failures.append(repr(error))
            if monitor['exit'] != 0 or monitor['error'] is not None or monitor['group_absent'] is not True:
                failures.append('resource monitor did not complete without a guard failure')
            try:
                census_sha = transport.replay_fresh_closure(owned, native, hot, namespace)
                closed = True
            except BaseException as error:
                failures.append('fresh closure: ' + repr(error))
            extended_binding(owned, binding, transport)
            transport.save(owned / 'resources-after.json', transport.resource_observation(owned))
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            transport.save(owned / 'launch.json', {'marker': marker, 'public_argv': command, 'pid': os.getpid(),
                'namespace': namespace, 'finished_ns': time.time_ns(), 'failures': failures,
                'fresh_closure': closed, 'fresh_census_sha256': census_sha if closed else None,
                'original_foreground_execution_returned': True, 'experiment': 'retained-wait-cadence-36'})
        finally:
            sys.argv = prior_argv
            for sig, handler in previous.items():
                signal.signal(sig, handler)
    return 1 if failures else 0


def independent_replay(readback, marker, binding, transport, hot, ordinary, experiment, planner):
    launch = transport.parse_json((readback / 'launch.json').read_bytes())
    transport.need(launch['public_argv'] == public_command(Path(marker['path']), marker, binding)
                   and launch['experiment'] == 'retained-wait-cadence-36', 'exact diagnostic public entrypoint')
    # This normal API validates all fresh raw joins and invokes the distinct planner.
    result = transport.independent_replay(readback, marker, planner)
    if not result['accepted']:
        return result
    transport.need(result['trials'] == 36, 'exact diagnostic thirty-six-trial replay')
    folder = readback / 'campaign-1'
    load = lambda name: transport.parse_json((folder / (name + '.json')).read_bytes())
    finished = load('finished')
    transport.need(finished['commit'] == marker['commit'] and finished['diagnostic_authority'] == 'none'
                   and finished['instrumentation_perturbs_timing_and_readiness'] is True
                   and all(finished[key] is False for key in ('exclusive_reservation', 'performance_acceptance',
                           'formal_refinement', 'engine_matching')), 'explicit diagnostic scope, not performance acceptance')
    expected_source = {name: binding['files']['source/' + name]['sha256'] for name in binding['selected_objects']}
    transport.need(load('source-before') == expected_source and load('source-after') == expected_source,
                   'opening and closing source inventories match exact signed selected objects')
    transport.need(load('tools-before') == load('tools-after'), 'independent closing tools continuity')
    before, after = load('binaries-before'), load('binaries-after')
    transport.need(set(before) == set(experiment.BINARIES) and before == after, 'four-ELF before/after roster')
    transport.need({key: transport.sha(folder / 'binaries' / name) for key, name in experiment.BINARIES.items()} == before,
                   'collected four-ELF bytes')
    remote = Path(marker['path'])
    binary_root = remote / 'campaign-1/binaries'
    source_root = remote / 'source'
    native_paths = types.SimpleNamespace(HERE=source_root / 'benchmarks/runtime_gfx942')
    device = binding['devices'][0]
    observed_hosts = {}
    for cadence, labels in ((False, ('host-before', 'host-after')), (True, ('host-cadence-before', 'host-cadence-after'))):
        for label, edge in zip(labels, ('start', 'end'), strict=True):
            value = load(label)
            experiment.validate_host(value, ordinary, before, device, cadence, hot)
            expected = experiment.host_command(native_paths, binary_root, device, label, edge, cadence)[1]
            receipt = transport.parse_json((folder / 'commands' / label / 'receipt.json').read_bytes())
            transport.need(receipt['command'] == expected and receipt['cwd'] == str(source_root)
                           and receipt['exit'] == 0 and receipt['error'] is None and receipt['group_absent'] is True,
                           'independent actual fourth-ELF dependency command join')
            transport.need(transport.parse_json((folder / 'commands' / label / 'stdout').read_bytes()) == value
                           and not (folder / 'commands' / label / 'stderr').read_bytes(), 'host raw observation join')
            observed_hosts[label] = value
        transport.need(observed_hosts[labels[0]]['continuity'] == observed_hosts[labels[1]]['continuity'],
                       'per-ELF loader/host continuity')
    transport.need(observed_hosts['host-before']['environment'] == observed_hosts['host-cadence-before']['environment']
                   and observed_hosts['host-after']['environment'] == observed_hosts['host-cadence-after']['environment'],
                   'ordinary/cadence independent host environment join')
    boundary = load('cadence-admission-boundary')
    expected_boundary = {'query_binary': str(binary_root / experiment.BINARIES['kfd']),
        'query_binary_sha256': before['kfd'], 'cadence_binary_sha256': before['kfd-cadence'],
        'cadence_binary_query_performed': False,
        'cadence_uid_gpu_engine_join': 'strict-cadence-output-versus-current-query-inputs',
        'query_only_mode_added': False, 'performance_acceptance': False}
    transport.need(boundary == expected_boundary and all(type(boundary[key]) is type(value)
        for key, value in expected_boundary.items()), 'explicit cadence no-query boundary')
    artifacts = load('fresh-kfd-artifacts')
    build_receipt = transport.parse_json((folder / 'commands/build-kfd-pair/receipt.json').read_bytes())
    expected_environment = hot.environment(remote / 'campaign-1/work')
    expected_environment['CARGO_BUILD_JOBS'] = '2'
    transport.need(build_receipt['command'] == experiment.kfd_build_spec()[1]
                   and build_receipt['cwd'] == str(source_root) and build_receipt['environment'] == expected_environment
                   and build_receipt['exit'] == 0 and build_receipt['error'] is None and build_receipt['group_absent'] is True,
                   'actual fresh paired build command and closed environment')
    raw_cargo = (folder / 'commands/build-kfd-pair/stdout').read_bytes()
    rows = [transport.parse_json(line) for line in raw_cargo.splitlines()]
    transport.need(rows and rows[-1] == {'reason': 'build-finished', 'success': True}
                   and sum(row.get('reason') == 'build-finished' for row in rows) == 1, 'fresh paired Cargo completion')
    transport.need(set(artifacts) == set(experiment.KFD_TARGETS), 'exact two fresh KFD producer records')
    for backend, target in experiment.KFD_TARGETS.items():
        row = artifacts[backend]['cargo_record']
        selected = [item for item in rows if item.get('reason') == 'compiler-artifact'
                    and item.get('target', {}).get('name') == target and item.get('executable') is not None]
        path = str(remote / 'campaign-1/work/target/release/examples' / target)
        transport.need(selected == [row] and row.get('fresh') is False and row.get('profile', {}).get('test') is False
                       and row.get('features') == ['default', 'hardware-diagnostic', 'live-validation']
                       and row['target'].get('kind') == ['example'] and row.get('executable') == path
                       and row.get('manifest_path') == str(source_root / 'crates/fe2o3-kfd/Cargo.toml')
                       and artifacts[backend]['path'] == path and artifacts[backend]['sha256'] == before[backend],
                       'fresh fourth ELF Cargo/raw/copied binary join')
    return {**result, 'four_elf_dependency_identity': True, 'ordinary_trials': 24, 'profiled_trials': 12,
            'cadence_binary_query_performed': False, 'performance_acceptance': False}


def execute(args, transport, native, hot, ordinary, experiment, planner):
    os.umask(0o077)
    prepared = args.prepared
    transport.need(prepared.is_absolute() and prepared.resolve(strict=True) == prepared, 'canonical prepared payload')
    marker = transport.parse_json((prepared / 'owner.json').read_bytes())
    binding = transport.read_binding(prepared, marker)
    extended_binding(prepared, binding, transport)
    transport.need(SCRIPT == prepared / 'source' / RELATIVE, 'execute authenticated relocated extension')
    transport.need(transport.sha(prepared / 'source.tar.gz') == binding['payload_sha256'], 'unchanged local payload')
    rec = native.FreshRecorder(prepared / 'remote-commands', prepared, hot)
    # Receive/collection/cleanup keep the reviewed standalone bootstrap unchanged.
    script = (prepared / 'source' / transport.TRANSPORT_RELATIVE).read_bytes()
    payload = (prepared / 'source.tar.gz').read_bytes()
    binding_raw = (prepared / 'binding.json').read_bytes()
    wire = script + f'{len(binding_raw):016x}'.encode('ascii') + binding_raw + payload
    previous = {sig: signal.getsignal(sig) for sig in hot.B.MANAGED}
    failures, run_known_terminal = [], False
    try:
        for sig in previous:
            signal.signal(sig, hot.B.interrupted)
        rec.run('receive', transport.bootstrap_command(script, 'receive', marker), 600, stdin=wire)
        command = transport.SSH + [shlex.join(['/usr/bin/python3', '-I', '-B',
            marker['path'] + '/source/' + RELATIVE, 'remote-run', transport.encoded(marker).decode('ascii').strip()])]
        try:
            rec.run('remote-run', command, 7200)
        except BaseException as error:
            failures.append(repr(error))
        receipt = transport.parse_json((rec.output / 'remote-run/receipt.json').read_bytes())
        run_known_terminal = receipt['exit'] in (0, 1) and receipt['error'] is None and receipt['group_absent'] is True
        transport.need(run_known_terminal, 'original remote terminal uncertain; retain private scratch')
        folder = rec.run('collect', transport.bootstrap_command(script, 'collect', marker), 180, stdin=script)
        collection = transport.parse_json((folder / 'stdout').read_bytes())
        transport.need(collection['marker'] == marker, 'collected owner join')
        folder = rec.run('pull', transport.bootstrap_command(script, 'pull', marker), 180, stdin=script)
        archive = folder / 'stdout'
        transport.need(transport.sha(archive) == collection['archive_sha256']
                       and archive.stat().st_size == collection['archive_bytes'], 'raw archive readback')
        destination = prepared / 'readback'
        destination.mkdir(mode=0o700)
        transport.validate_archive(archive, collection['files'], destination)
        replay = independent_replay(destination, marker, binding, transport, hot, ordinary, experiment, planner)
        transport.save(prepared / 'independent-replay.json', replay)
        rec.run('cleanup', transport.bootstrap_command(script, 'cleanup', marker, [collection['archive_sha256']]), 180, stdin=script)
        rec.run('absence', transport.bootstrap_command(script, 'absence', marker), 30, stdin=script)
        transport.need(replay['accepted'], 'native diagnostic campaign did not qualify')
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            transport.save(prepared / 'transport-census.json', native.fresh_census(rec, hot, os.readlink('/proc/self/ns/pid')))
            transport.save(prepared / 'transport-finished.json', {'marker': marker, 'failures': failures,
                'original_remote_terminal': run_known_terminal, 'accepted': not failures,
                'experiment': 'retained-wait-cadence-36'})
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
    transport.need(not failures, 'diagnostic transport/campaign failed: ' + repr(failures))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_subparsers(dest='mode', required=True)
    prepare = modes.add_parser('prepare')
    prepare.add_argument('--commit', required=True)
    prepare.add_argument('--output', type=Path, required=True)
    prepare.add_argument('--device', nargs=3, action='append', required=True)
    run = modes.add_parser('execute')
    run.add_argument('--prepared', type=Path, required=True)
    remote = modes.add_parser('remote-run')
    remote.add_argument('marker')
    args = parser.parse_args()
    transport, native, hot, ordinary, experiment, planner = helpers()
    if args.mode == 'prepare':
        parsed = native.parse_arguments(['--campaign', '--commit', args.commit, '--output', str(args.output),
            *(item for device in args.device for item in ('--device', *device))])
        transport.prepare(parsed)
        marker = transport.parse_json((args.output / 'owner.json').read_bytes())
        extended_binding(args.output, transport.read_binding(args.output, marker), transport)
    elif args.mode == 'execute':
        execute(args, transport, native, hot, ordinary, experiment, planner)
    else:
        raise SystemExit(remote_run(transport.parse_json(args.marker), transport, native, hot))


if __name__ == '__main__':
    main()
