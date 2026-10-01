#!/usr/bin/env python3
"""Offline native24 replay; never probes historical processes or connects to SSH."""
import hashlib
import json
from pathlib import Path
import shlex
import sys
import types

BASE = Path('/home/harsh/.codex-tmp')
PACKET = BASE / 'fe2o3-xgmi-retained-host-diagnostic-native24-20260930-attempt-1'
OUTPUT = BASE / 'fe2o3-native24-root-readback-20261001.json'
RELATIVE = 'benchmarks/runtime_gfx942/xgmi_retained_host_diagnostic_transport.py'

def need(value, message):
    if not value:
        raise ValueError(message)

def raw(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path, 'ordinary canonical readback')
    return path.read_bytes()

def sha(path):
    return hashlib.sha256(raw(path)).hexdigest()

need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, 'python -I -B required')
path = PACKET / 'source' / RELATIVE
need(sha(path) == '4bc99ac319ffd8a3a56efb37cb63a6e4769e41c3ccc610f6d2f3a2b3014f527f', 'reviewed signed diagnostic transport')
module = types.ModuleType('root_native24_replay')
module.__file__ = str(path)
exec(compile(raw(path), str(path), 'exec'), module.__dict__)
transport, native, hot, ordinary, experiment, planner = module.helpers()
read = lambda p: transport.parse_json(raw(p))
marker = read(PACKET / 'owner.json')
preflight = read(BASE / 'fe2o3-native24-preflight-20261001.json')
need(preflight['marker'] == marker and preflight['source_archive_and_signature_match'] is True,
     'independent signed preflight ownership join')
need(marker == {'path': '/home/harsh/fe2o3-xgmi-series-20260930.0b26c1f6eb613ac2',
                'commit': 'a26dbebb57f948439a2e813c7a17c1142014ba1e',
                'binding_sha256': 'b4d89265c8861e3bcf39517e4c96cd2a7885e0753f688051451082eb0bf38e82'},
     'exact reviewed owner')
binding = transport.read_binding(PACKET, marker)
module.extended_binding(PACKET, binding, transport)
need(sha(PACKET / 'source.tar.gz') == preflight['payload_sha256'] == binding['payload_sha256'], 'unchanged signed payload')
collection = read(PACKET / 'remote-commands/collect/stdout')
archive = PACKET / 'remote-commands/pull/stdout'
need(collection['marker'] == marker and sha(archive) == collection['archive_sha256'] ==
     '1375a5d7dd8968bd905a0a1ce74add0b3a2c38bf5336f44f63dc28a0592f2227'
     and archive.stat().st_size == collection['archive_bytes'], 'exact collected raw archive')
transport.validate_archive(archive, collection['files'])
readback = PACKET / 'readback'
need({str(p.relative_to(readback)) for p in readback.rglob('*') if p.is_file()} == set(collection['files']),
     'complete extracted collection roster')
for name, row in collection['files'].items():
    need(transport.inventory_row(readback / name) == row, 'archive-to-readback byte/mode join')
replay = module.independent_replay(readback, marker, binding, transport, hot, ordinary, experiment, planner)
need(replay == read(PACKET / 'independent-replay.json') and replay['accepted'] is True
     and replay['trials'] == 24 and replay['ordinary_trials'] == 18 and replay['profiled_trials'] == 6
     and replay['four_elf_dependency_identity'] is True and replay['performance_acceptance'] is False,
     'complete exact independent diagnostic/ordinary replay')
script = raw(PACKET / 'source' / transport.TRANSPORT_RELATIVE)
binding_raw = raw(PACKET / 'binding.json')
wire = script + f'{len(binding_raw):016x}'.encode('ascii') + binding_raw + raw(PACKET / 'source.tar.gz')
remote_command = transport.SSH + [shlex.join(['/usr/bin/python3', '-I', '-B',
    marker['path'] + '/source/' + RELATIVE, 'remote-run', transport.encoded(marker).decode('ascii').strip()])]
phases = [('receive', transport.bootstrap_command(script, 'receive', marker), 600, wire),
          ('remote-run', remote_command, 7200, None),
          ('collect', transport.bootstrap_command(script, 'collect', marker), 180, script),
          ('pull', transport.bootstrap_command(script, 'pull', marker), 180, script),
          ('cleanup', transport.bootstrap_command(script, 'cleanup', marker, [collection['archive_sha256']]), 180, script),
          ('absence', transport.bootstrap_command(script, 'absence', marker), 30, script)]
census = read(PACKET / 'transport-census.json')
need(census['attempt_order'] == [row[0] for row in phases]
     and set(census['raw_sha256']) == {row[0] for row in phases}, 'complete ordered transport ownership')
receipts = []
previous_end = 0
for name, argv, timeout, stdin in phases:
    folder = PACKET / 'remote-commands' / name
    record = read(folder / 'receipt.json')
    pins = {part: sha(folder / part) for part in ('receipt.json', 'stdout', 'stderr')}
    need(pins == census['raw_sha256'][name] and record['command'] == argv and record['cwd'] == str(PACKET)
         and record['timeout_seconds'] == timeout and record['environment'] is None
         and record['stdin_sha256'] == (None if stdin is None else hashlib.sha256(stdin).hexdigest())
         and record['exit'] == 0 and type(record['exit']) is int and record['error'] is None
         and record['group_absent'] is True and record['started_ns'] >= previous_end
         and record['finished_ns'] >= record['started_ns']
         and record['stdout_sha256'] == pins['stdout'] and record['stderr_sha256'] == pins['stderr'],
         'exact closed command/stream/stdin/deadline/time joins: ' + name)
    need(not raw(folder / 'stderr'), 'silent successful transport stderr')
    receipts.append({'name': name, 'pid': record['pid'], 'receipt_sha256': pins['receipt.json']})
    previous_end = record['finished_ns']
need(census['records'] == sorted(receipts, key=lambda r: r['name']) and len({r['pid'] for r in receipts}) == 6,
     'six distinct original closed owners, without current PID probes')
need(read(PACKET / 'remote-commands/cleanup/stdout') == {'removed': marker['path'], 'archive_sha256': collection['archive_sha256']}
     and read(PACKET / 'remote-commands/absence/stdout') == {'path_absent': marker['path']}, 'exact marked cleanup and absence')
finished = read(PACKET / 'transport-finished.json')
need(finished == {'accepted': True, 'experiment': 'retained-host-diagnostic-24', 'failures': [],
                  'marker': marker, 'original_remote_terminal': True}, 'authoritative original terminal success')
native_census = read(readback / 'campaign-1/fresh-census.json')
result = {'marker': marker, 'independent_replay': replay, 'archive_files': len(collection['files']),
          'archive_bytes': archive.stat().st_size, 'archive_sha256': sha(archive),
          'transport_stages': receipts, 'native_stages': len(native_census['records']),
          'owned_directory_removed_and_absence_recorded': True, 'new_network_or_gpu_execution': False,
          'historical_pid_probes': False, 'performance_acceptance': False,
          'scope': 'Offline signed-source, all raw archive members, native replay and original transport/cleanup receipts.'}
with OUTPUT.open('x') as stream:
    stream.write(json.dumps(result, sort_keys=True, indent=2) + '\n')
print(json.dumps({'audit_sha256': sha(OUTPUT), 'archive_files': result['archive_files'],
                  'native_stages': result['native_stages'], 'transport_stages': len(receipts), 'trials': replay['trials']}))
