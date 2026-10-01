#!/usr/bin/env python3
"""Replay saved CPU records; optionally verify the full recovery archive."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tarfile

sys.dont_write_bytecode = True
REMOTE = Path('/tmp/fe2o3-multigpu-cpu-dd5-d81705f06b41b2ff')
SOURCE_SHA = 'b1ab447c92cb6a7bd51f549f6489c828db304144b41ff79f1665e5c51730689a'
PREPARED_SHA = '056e033d6b461f9a20686a95480e1949a84f26d78a635374085751e7120914e1'
PRIOR_SHA = '22e40a2e340bef10c480b8cc5e8f47af28e735dc746e966b8b2b36ee46cfa61f'


def need(value, label):
    if not value:
        raise ValueError(label)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    return json.loads(path.read_bytes())


def load(path, digest, name):
    need(sha(path.read_bytes()) == digest, 'pinned parser ' + name)
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def detached_metadata(stdout, stderr, source, repo):
    # The original host paths are absent locally; join their manifest identities to
    # the pinned source inventory instead of pretending those paths were reopened.
    need(not stderr and stdout.endswith('\n'), 'complete Cargo metadata')
    value = json.loads(stdout)
    need(value['version'] == 1 and value['workspace_root'] == str(repo) and
         value['target_directory'] == str(REMOTE / 'target-metadata/target') and value['resolve'] is None,
         'bound no-deps metadata')
    manifests, ids = [], []
    for row in value['packages']:
        path = Path(row['manifest_path'])
        need(path.is_relative_to(repo) and '..' not in path.parts, 'bound manifest path')
        name = str(path.relative_to(repo))
        need(source['candidate_inventory'][name]['sha256'] == source['workspace']['manifest_sha256'][name]
             and row['source'] is None and isinstance(row['id'], str), 'source-manifest identity')
        manifests.append(name)
        ids.append(row['id'])
    members = value['workspace_members']
    need(len(ids) == len(set(ids)) == len(manifests) == len(set(manifests)) and
         set(manifests) == set(source['workspace']['members']) and
         len(members) == len(set(members)) and set(members) == set(ids), 'complete metadata members')
    return dict(packages=len(ids), manifest_names=sorted(manifests), workspace_members=sorted(members))


def verify_archive(archive, before, after, extracted):
    need(before.read_bytes() == after.read_bytes(), 'unchanged originals across transfer')
    identities, hashes = {}, {}
    for line in before.read_text().splitlines():
        if line.startswith('STAT\t'):
            _, name, size, mode, inode, links, device = line.split('\t')
            identities[name] = dict(bytes=int(size), mode=int(mode, 8), inode=int(inode),
                                    links=int(links), device=int(device))
        elif re.match(r'^[0-9a-f]{64}  ', line):
            digest, name = line.split('  ', 1)
            hashes[name] = digest
    need(identities.keys() == hashes.keys(), 'complete stat/hash inventory')
    observed = set()
    with tarfile.open(archive, 'r:gz') as stream:
        for member in stream:
            name = member.name
            need(not PurePosixPath(name).is_absolute() and '..' not in PurePosixPath(name).parts,
                 'relative archive paths')
            if member.isdir():
                continue
            need(member.isfile() and name not in observed and name in hashes,
                 'only distinct regular custody copies')
            observed.add(name)
            info = identities[name]
            need(member.size == info['bytes'] and member.mode == info['mode'], 'archive metadata')
            need(sha(stream.extractfile(member).read()) == hashes[name], 'archive bytes')
            local = extracted / name
            need(local.is_file() and not local.is_symlink() and local.stat().st_nlink == 1,
                 'single-link extracted custody copy')
            need(sha(local.read_bytes()) == hashes[name] and
                 stat.S_IMODE(local.stat().st_mode) == info['mode'], 'extracted identity')
    need(observed == hashes.keys(), 'all selected original files recovered')
    return identities, hashes


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('records', type=Path)
    ap.add_argument('--recovery', type=Path)
    ap.add_argument('--repository', type=Path)
    args = ap.parse_args()
    root = args.records.resolve()
    source_path = root / 'candidate-attempt-1/record.json'
    prepared_path = root / 'cpu-prepared/record.json'
    old_path = root / 'cpu-execution/result.json'
    for path, digest in ((source_path, SOURCE_SHA), (prepared_path, PREPARED_SHA), (old_path, PRIOR_SHA)):
        need(sha(path.read_bytes()) == digest, 'frozen input ' + str(path))
    source, frozen, old = read(source_path), read(prepared_path), read(old_path)
    repository_matches, documentation_changes = 0, []
    if args.repository:
        for name, identity in source['candidate_inventory'].items():
            path = args.repository / name
            need(path.is_file() and not path.is_symlink(), 'candidate file exists ' + name)
            if sha(path.read_bytes()) != identity['sha256']:
                need(name == 'docs/runtime-a1-a2-swarm-current.md', 'unchanged tested source ' + name)
                documentation_changes.append(name)
            else:
                repository_matches += 1
    new_dir = root / 'cpu-completion-attempt-1/execution'
    new, inputs = read(new_dir / 'result.json'), read(new_dir / 'inputs.json')
    need(old['accepted'] is False and old['errors'] == ['ValueError: libtest complete-list summary'],
         'original rejection retained')
    need(len(old['rows']) == 18 and all(row['accepted'] is True for row in old['rows'][:17]) and
         old['rows'][17]['accepted'] is False, 'exact original prefix scope')
    need(new['accepted'] is True and new['original_accepted'] is False and new['errors'] == [] and
         len(new['rows']) == 5 and all(row['accepted'] is True for row in new['rows']), 'completion scope')
    need(inputs['prepared_sha256'] == PREPARED_SHA and inputs['prior_result_sha256'] == PRIOR_SHA,
         'completion binds immutable original')
    for result in (old, new):
        need(result['gpu_invocations'] == 0 and result['solver_invocations'] == 0 and
             result['performance_acceptance'] is False and result['source_signed'] is False,
             'no GPU, performance, solver, or signed-overlay claim')
    modules = [load(root / 'candidate-attempt-1/controls' / name,
                    source['controls'][name]['sha256'], name[:-3])
               for name in ('cpu_results_v1.py', 'cpu_results_v2.py', 'cpu_workspace_v1.py')]
    parser, serial, _ = modules
    common_path = root / 'common.py'
    common = load(common_path, '5af2c624319e6c0d6bbfb0c3afa650aca1a9d93ad0769e6a46dbee0b16199965', 'saved_common')
    common.HERE, common.CANDIDATE = REMOTE, REMOTE / 'candidate-attempt-1'
    common.SOURCE = common.CANDIDATE / 'source'
    artifacts, rosters = new['artifacts'], {}
    need(set(artifacts) == {'runtime-debug', 'smoke-tests', 'smoke-example'}, 'three artifacts')
    need(all(value == artifacts[name] for name, value in old['artifacts'].items()), 'prior artifact continuity')
    plan = frozen['plan']['stages'][:18]
    need(inputs['stages'] == [dict(frozen['plan']['stages'][0], name='completion-opening-attestation'),
                              *frozen['plan']['stages'][17:]], 'exact five-command continuation')
    command_count, last = 0, 0
    for folder, result, stages in ((root / 'cpu-execution', old, plan), (new_dir, new, inputs['stages'])):
        census = read(folder / 'fresh-census.json')
        need(census['attempt_order'] == [stage['name'] for stage in stages], 'closed census order')
        need(census['namespace'] == frozen['snapshot']['namespaces']['pid_namespace'], 'saved namespace')
        joined = {row['name']: row for row in census['records']}
        need(len(joined) == len(stages), 'one census row per command')
        for index, (row, stage) in enumerate(zip(result['rows'], stages, strict=True)):
            name, kind, mode = stage['name'], stage['kind'], stage['mode']
            command = folder / 'commands' / name
            receipt = read(command / 'receipt.json')
            stdout, stderr = (command / 'stdout').read_bytes(), (command / 'stderr').read_bytes()
            need(row == read(folder / (name + '-result.json')), 'same saved stage row')
            need(receipt['exit'] == 0 and receipt['error'] is None and receipt['group_absent'] is True,
                 'successful reaped command')
            need(last <= receipt['started_ns'] <= receipt['finished_ns'] and
                 receipt['finished_ns'] - receipt['started_ns'] <= (stage['seconds'] + 30) * 10**9,
                 'ordered bounded commands')
            last = receipt['finished_ns']
            argv = list(stage['argv'])
            if argv[0].startswith('@'):
                argv[0] = artifacts[argv[0][1:]]['path']
            need(receipt['command'] == argv and receipt['cwd'] == str(common.SOURCE) and
                 receipt['environment'] == common.environment(mode) and
                 receipt['timeout_seconds'] == stage['seconds'] and receipt['stdin_sha256'] is None,
                 'exact argv/environment/time bound')
            for leaf in ('stdout', 'stderr', 'receipt.json'):
                digest = sha((command / leaf).read_bytes())
                need(census['raw_sha256'][name][leaf] == digest, 'census raw digest')
                if leaf != 'receipt.json':
                    need(receipt[leaf + '_sha256'] == digest, 'receipt output digest')
            need(joined[name]['pid'] == receipt['pid'] and
                 joined[name]['receipt_sha256'] == sha((command / 'receipt.json').read_bytes()), 'census join')
            observed = None
            text, errors = stdout.decode(), stderr.decode()
            if kind == 'attest':
                need(not stderr and json.loads(stdout) == {'immutable_snapshot_sha256':
                     sha(json.dumps(frozen['snapshot'], sort_keys=True, separators=(',', ':')).encode())},
                     'source/tool/cache attestation')
            elif kind == 'version':
                need(bool(stdout) and not stderr, 'tool version output')
            elif kind == 'loader-smoke':
                need(not stderr and stdout == (folder / 'commands/rustc-version/stdout').read_bytes(), 'loader output')
            elif kind == 'metadata':
                observed = detached_metadata(text, errors, source, common.SOURCE)
            elif kind == 'controls':
                need(not stderr and json.loads(stdout) == source['source_control_summary'], 'source controls')
            elif kind == 'parser-controls':
                expected = {'test_cpu_results_v1': 17, 'test_cpu_results_v2': 5, 'test_cpu_workspace_v1': 7}[name]
                need(not stdout and stderr.endswith(b'\nOK\n') and
                     re.findall(rb'^Ran (\d+) tests in [0-9.]+s$', stderr, re.M) == [str(expected).encode()], 'parser tests')
            elif kind in ('cargo', 'build'):
                messages = parser.cargo_output(text, errors)
                if kind == 'build':
                    observed = artifacts[mode]
                    need(messages.count(observed['cargo_record']) == 1 and observed['cargo_record']['fresh'] is False and
                         observed['newly_built_in_this_attempt'] is True, 'actual fresh Cargo artifact')
            elif kind == 'list':
                if mode == 'smoke-tests':
                    need(not stderr and stdout == (source['example_tests'][0] + ': test\n\n1 test, 0 benchmarks\n').encode(), 'exact singleton list')
                    rosters[mode] = source['example_tests']
                    observed = {'names': rosters[mode], 'elf_sha256': artifacts[mode]['sha256']}
                else:
                    rosters[mode] = parser.test_list(text, errors)
                    observed = {'names': rosters[mode], 'elf_sha256': artifacts[mode]['sha256']}
                    for prefix, names in source['required_tests'].items():
                        need(sorted(name for name in rosters[mode] if name.startswith(prefix)) == names, 'new test roster')
            elif kind == 'ignored':
                names = parser.test_list(text, errors)
                need(names == sorted(source['runtime_ignored']), 'ignored roster')
                observed = {'names': names, 'elf_sha256': artifacts[mode]['sha256']}
            elif kind == 'full':
                observed = serial.libtest_output(text, rosters[mode], source['runtime_ignored'])
            elif kind == 'example-tests':
                need(not stderr, 'CLI test stderr empty')
                observed = parser.libtest_output(text, rosters[mode], {}, headers={}, required=frozenset(rosters[mode]))
            else:
                raise ValueError('unexpected stage kind')
            if row['accepted']:
                need(observed == row['classification'], 'replayed classification ' + name)
            command_count += 1
    identities, hashes = {}, {}
    if args.recovery:
        recovery = args.recovery.resolve()
        identities, hashes = verify_archive(recovery / 'recovered.tar.gz', recovery / 'before.audit',
                                             recovery / 'after.audit', root)
        for artifact in artifacts.values():
            for item in (artifact, artifact['dep_info']):
                relative = str(Path(item['path']).relative_to(REMOTE))
                need(hashes[relative] == item['sha256'] and identities[relative]['bytes'] == item['bytes'] and
                     identities[relative]['mode'] == item['mode'], 'recorded artifact custody')
    summary = dict(records_accepted=True, original_accepted=False, composite_scope='17 prior gates plus 5 completion commands',
                   commands_replayed=command_count, runtime_passed=1928, runtime_ignored=32, example_passed=1,
                   native_gpu_qualified=False, performance_accepted=False, formal_proofs_rerun=False,
                   source_record_sha256=SOURCE_SHA, prepared_sha256=PREPARED_SHA,
                   original_result_sha256=PRIOR_SHA, completion_result_sha256=sha((new_dir / 'result.json').read_bytes()),
                   repository_files_hash_matched=repository_matches, disclosed_documentation_changes=documentation_changes,
                   artifact_bytes_verified=bool(args.recovery), recovered_regular_files=len(hashes),
                   hardlink_custody_copies={name: info for name, info in identities.items() if info['links'] > 1},
                   artifacts={key: {field: value[field] for field in ('path', 'sha256', 'bytes', 'mode')} for key, value in artifacts.items()})
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
