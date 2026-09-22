#!/usr/bin/env python3
"""Offline replay of source identities, exact solver diagnostics and CPU receipts."""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
PACKET = Path('docs/evidence/dev-producer-query-2026-09-22')


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args])


def manifest(packet):
    rows = {}
    for line in (packet / 'SHA256SUMS').read_text().splitlines():
        need(re.fullmatch(r'[0-9a-f]{64}  [^\n]+', line), 'manifest row shape')
        digest, path = line.split('  ', 1)
        need(path not in rows and not Path(path).is_absolute() and '..' not in Path(path).parts, 'manifest path')
        rows[path] = digest
    files = {str(p.relative_to(packet)): p for p in packet.rglob('*') if p.is_file() and p != packet / 'SHA256SUMS'}
    need(not any(p.is_symlink() for p in packet.rglob('*')), 'no artifact symlinks')
    need(set(rows) == set(files), 'exact manifest roster')
    need(all(sha(files[p].read_bytes()) == h for p, h in rows.items()), 'artifact hashes')
    return set(rows)


def terminal(row, status, command):
    need(set(row) == {'command', 'started_ns', 'process_group', 'finished_ns', 'status', 'group_absent'}, 'terminal receipt fields')
    need(type(row['status']) is int and row['status'] == status and row['group_absent'] is True, 'normal terminal solver receipt')
    need(row['command'] == command, 'exact command and scope')
    need(all(type(row[k]) is int and row[k] > 0 for k in ('started_ns', 'finished_ns', 'process_group')), 'receipt integer fields')
    need(row['started_ns'] <= row['finished_ns'], 'receipt time order')


def test_results(text):
    results = []
    for line in text.splitlines():
        if line.startswith('test result:'):
            match = re.fullmatch(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in \d+\.\d+s', line)
            need(match is not None, 'exact successful terminal test result')
            results.append(tuple(map(int, match.groups())))
    return results


def render_results(packet):
    source = (packet / 'SOURCE').read_text().strip()
    performance = read_module(packet / 'performance.py', 'query_render_performance')
    lines = [
        "# Qualified Actual Producer-Read Queries",
        "",
        "- Verus: two whole-root runs, each 546 verified and zero errors at default solver limits.",
        "- Twenty-nine controls: twenty-five executable mutations, one raw-domain contract sensitivity, one projection and two witness sensitivities.",
        "- CPU: 908 unit tests and 27 doctests passed; fifteen tests ignored in the normal run.",
        "- Formatting, all-target Clippy with warnings denied and a release test build passed.",
        "- One selected release benchmark passed, producing 1,456 rows over 104 case combinations.",
        "- No native GPU execution or HIP/HSA measurements were run.",
        "",
        "Actual writer lookup and producer status, validation, retained inspection, lookup and",
        "public status now share production bodies. Raw contracts are unconditional: no custody,",
        "count/storage shape, capacity or successful-admission premise is introduced.",
        "Four paired harnesses independently execute actual and historical queries, preserving",
        "exact results and error precedence on represented owners and mapped inputs.",
        "",
        "Successful public status eliminates a redundant traversal: three versus six journal",
        "accesses for Pending/Unknown, and one versus two for Success/NoEffect. Other operations",
        "and rejected queries retain their original counts. These counters measure journal",
        "allocation/backlink/writer access, not every reservation or vector operation.",
        "",
        "Synthetic raw witnesses run all four query operations for Pending, Unknown, Success",
        "and NoEffect while irrelevant free/count/incarnation storage is malformed. Pending",
        "and Unknown writer metadata may contain invalid heads and maximal counts; read-only",
        "lookup does not validate retained chains. Resolved witnesses use either an out-of-range",
        "old producer slot or the same slot occupied by an unrelated Reserved writer. A stale",
        "reservation reference rejects before a corrupt stored device; the exact reference",
        "exposes that device error. These are not acquisition/settlement reachability proofs.",
        "",
        "CPU fixtures use actual lifecycle setup and retain a second reservation, compare",
        "complete snapshots, storage identities, exact results and journal access counts",
        "against frozen pre-change execution. They isolate error order, range boundaries,",
        "full identities, nonzero prior lineage, resolved writer reuse and device-generation",
        "mismatch. Writer leaf tests preserve Reserved/Pending/Unknown and exact counts,",
        "including zero and maximal identifiers/counts, without chain-admission semantics.",
        "",
        "The 546 obligations include inherited proofs. Historical query declarations are an",
        "exact authenticated projection into one type universe, not a duplicate custody root.",
        "Reviewed public/private adapters and unchanged baseline dependencies are source-bound.",
        "Production's typed journal receiver uses immutable Deref; proof execution explicitly",
        "projects the journal field. That unchanged trait adapter is authenticated, not newly",
        "proved as a trait implementation. Physical storage, allocator/unwind behavior,",
        "construction and lifecycle reachability, producer admission/release and remaining",
        "wrappers remain open. Native pending-consumer admission stays closed; Gate 1 and full",
        "HIP/HSA parity remain unproved.",
        "",
        "## Instrumented CPU Comparison",
        "",
        "Ratios are shared/frozen medians over seven alternating rounds. Lower is faster.",
        "These are non-exclusive CPU regression observations with test-only journal counters,",
        "per-call timer, result mapping and dispatch overhead inside the measurement. Counter",
        "reset, assertions and storage checks are outside timing. The same immutable owner",
        "needs no structural restoration. Instrumentation may affect compiler optimization;",
        "these are not uninstrumented production latency, GPU comparisons or an",
        "orders-of-magnitude claim. All short/error paths and regressions are retained.",
        ""
]
    lines[2:2] = ['Source: ' + source + '.', '']
    lines.extend([performance.table((packet / 'cargo/performance/stdout').read_text()), ''])
    return '\n'.join(lines)


def audit(repo, packet):
    actual_roster = manifest(packet)
    source = (packet / 'SOURCE').read_text().strip()
    need(re.fullmatch('[0-9a-f]{40}', source), 'source commit')
    need(not git(repo, 'diff', '--name-only', '9d2ec2b98819daf14b2a0bc72681f95a3cc571a9', source, '--',
                 'crates/fe2o3-runtime'), 'native runtime unchanged')
    static = ['check.py', 'cargo-checks.py', 'audit.py', 'negative-expectations.json', 'README.md', 'performance.py']
    for name in static:
        need((packet / name).read_bytes() == git(repo, 'show', source + ':' + str(PACKET / name)), 'source-bound packet file: ' + name)
    need(Path(__file__).read_bytes() == (packet / 'audit.py').read_bytes(), 'running source-bound auditor')
    with tempfile.TemporaryDirectory(prefix='fe2o3-ordering-audit-') as temporary:
        tree = Path(temporary)
        for name in static:
            path = tree / PACKET / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((packet / name).read_bytes())
        check = read_module(tree / PACKET / 'check.py', 'ordering_offline_check')
        cpu = read_module(tree / PACKET / 'cargo-checks.py', 'ordering_offline_cpu')
        performance = read_module(tree / PACKET / 'performance.py', 'guards_offline_performance')
        # Discover the complete CPU source roster from Git, not from the receipt.
        src = git(repo, 'ls-tree', '-rz', '--name-only', source, '--', str(check.CRATE / 'src')).split(b'\0')
        for raw in src:
            if raw:
                path = Path(raw.decode())
                target = tree / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(git(repo, 'show', source + ':' + str(path)))
        for path in check.inputs(tree):
            target = tree / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(git(repo, 'show', source + ':' + str(path)))
        for path, digest in check.PINS.items():
            need(sha((tree / path).read_bytes()) == digest, 'leaf/source pin')
        performance.authenticate(repo, tree)
        base = check.load(tree, check.BASE, 'ordering_offline_recorder')
        parse = base.unique_json
        wanted_inputs = check.snapshot(tree, False)
        for directory in ['proof', 'cargo']:
            for phase in ['before', 'after']:
                need(check.same(parse((packet / directory / ('inputs-' + phase + '.json')).read_text()), wanted_inputs), 'exact source input bracket')
        meta = parse((packet / 'proof/source.json').read_text())
        need(set(meta) == {'commit', 'probe', 'verus'} and meta['commit'] == source and meta['probe'] is False, 'qualified source metadata; probes forbidden')
        verus = Path(meta['verus'])
        need(verus.is_absolute() and verus.name == 'verus', 'recorded tool path')
        expected = parse((tree / PACKET / 'negative-expectations.json').read_text())
        roster = set(static) | {'SOURCE', 'RESULTS.md', 'proof/inputs-before.json', 'proof/inputs-after.json', 'proof/source.json',
                                'cargo/inputs-before.json', 'cargo/inputs-after.json'}
        policy = check.load(tree, check.POLICY, 'ordering_offline_policy')
        recorded_root = None
        proof_start = None
        proof_finish = 0
        for name, mutation in [('positive-before', None), *[(m[0], m) for m in check.MUTATIONS], ('positive-after', None)]:
            folder = packet / 'proof' / name
            receipt = parse((folder / 'solver/record.json').read_text())
            command = receipt['command']
            recorded_folder = Path(command[-1]).parents[len(check.ROOT.parts) - 1]
            need(recorded_folder.name == name, 'recorded case path')
            if recorded_root is None:
                recorded_root = recorded_folder.parent
            need(recorded_folder.parent == recorded_root, 'single campaign root')
            terminal(receipt, 1 if mutation else 0, check.command(verus, recorded_folder / check.ROOT, mutation))
            if proof_start is None:
                proof_start = receipt['started_ns']
            need(proof_finish <= receipt['started_ns'], 'serialized bracketed proof cases')
            proof_finish = receipt['finished_ns']
            for path in check.SOURCES:
                wanted = (tree / path).read_bytes()
                if mutation and path == mutation[1]:
                    wanted = check.mutated(wanted, mutation)
                need((folder / path).read_bytes() == wanted, 'exact replayed shared source')
                roster.add(str(Path('proof') / name / path))
            for path, projection, count in check.PROJECTIONS:
                shared = (folder / path).read_text()
                need(shared.count('macro_rules!') == count, 'exact shared macro roster')
                stripped = shared.replace('macro_rules!', '').encode()
                need((folder / projection).read_bytes() == stripped, 'exact policy projection')
                policy.scan(folder / projection)
                roster.add('proof/' + name + '/' + projection)
            for proof in check.POLICY_SOURCES:
                policy.scan(folder / proof)
            check.check_result(base, receipt['status'], (folder / 'solver/stdout.log').read_text(),
                               (folder / 'solver/stderr.log').read_text(), recorded_folder, mutation, expected)
            roster.update('proof/' + name + '/solver/' + f for f in ('record.json', 'stdout.log', 'stderr.log'))
        original_repo = None
        for phase in ['before', 'after']:
            folder = packet / 'proof' / ('closure-' + phase)
            row = parse((folder / 'record.json').read_text())
            command = row['command']
            command_repo = Path(command[1]).parents[len(check.CLOSURE.parts) - 1]
            if original_repo is None:
                original_repo = command_repo
            need(command_repo == original_repo, 'single recorded source directory')
            terminal(row, 0, ['/bin/sh', str(original_repo / check.CLOSURE), str(verus.parent), str(original_repo / check.MANIFEST)])
            if phase == 'before':
                need(row['finished_ns'] <= proof_start, 'opening closure bracket')
            else:
                need(proof_finish <= row['started_ns'], 'closing closure bracket')
            proof_finish = max(proof_finish, row['finished_ns'])
            need((folder / 'stdout.log').read_text() == 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'closure transcript')
            need(not (folder / 'stderr.log').read_bytes(), 'clean closure diagnostic output')
            roster.update('proof/closure-' + phase + '/' + f for f in ('record.json', 'stdout.log', 'stderr.log'))
        environment = None
        previous_finish = proof_finish
        for name, command in cpu.commands():
            folder = packet / 'cargo' / name
            row = parse((folder / 'receipt.json').read_text())
            need(set(row) == {'command', 'cwd', 'started_ns', 'timeout_seconds', 'pid', 'exit', 'error',
                             'group_absent', 'environment', 'stdin_sha256', 'finished_ns', 'stdout_sha256', 'stderr_sha256'}, 'CPU receipt field roster')
            need(row['command'] == command and row['exit'] == 0 and type(row['exit']) is int
                 and row['error'] is None and row['group_absent'] is True and type(row['timeout_seconds']) is int
                 and row['timeout_seconds'] == 300, 'normal CPU receipt')
            need(row['cwd'] == str(original_repo) and row['stdin_sha256'] is None, 'CPU source directory and stdin')
            need(all(type(row[k]) is int and row[k] > 0 for k in ('pid', 'started_ns', 'finished_ns'))
                 and row['started_ns'] <= row['finished_ns'], 'CPU process/time fields')
            need(previous_finish <= row['started_ns'], 'serialized CPU campaign')
            previous_finish = row['finished_ns']
            if environment is None:
                environment = row['environment']
                target = Path(environment['CARGO_TARGET_DIR'])
                need(target.is_absolute() and target.name == 'target' and target.parent.name.startswith('fe2o3-producer-query-'), 'owned CPU target')
                need(environment == {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
                                     'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_TARGET_DIR': str(target)}, 'minimal CPU environment')
            need(row['environment'] == environment, 'stable CPU environment')
            for stream in ('stdout', 'stderr'):
                need(sha((folder / stream).read_bytes()) == row[stream + '_sha256'], 'CPU transcript digest')
            out = (folder / 'stdout').read_text()
            if name == 'test':
                need(test_results(out) == [(908, 0, 15, 0, 0), (27, 0, 0, 0, 0)], 'complete unit/doctest result roster')
            if name == 'performance':
                need(test_results(out) == [(1, 0, 0, 0, 922)], 'exact release producer-query benchmark result')
                performance.parse(out)
            roster.update('cargo/' + name + '/' + f for f in ('receipt.json', 'stdout', 'stderr'))
        need(actual_roster == roster, 'exact standalone packet roster')
        need((packet / 'RESULTS.md').read_text() == render_results(packet), 'replayed qualification summary')
        return check, base, expected


def selftest(repo, packet):
    check, base, expected = audit(repo, packet)
    rejected = 0
    for name, mutation in [('positive-before', None), *[(m[0], m) for m in check.MUTATIONS]]:
        folder = packet / 'proof' / name / 'solver'
        receipt = base.unique_json((folder / 'record.json').read_text())
        case = Path(receipt['command'][-1]).parents[len(check.ROOT.parts) - 1]
        stdout, stderr = (folder / 'stdout.log').read_text(), (folder / 'stderr.log').read_text()
        report = base.unique_json(stdout)
        for field, value in [('errors', 0 if mutation else 1), ('verified', True), ('encountered-vir-error', True), ('is-verifying-entire-crate', False if not mutation else True)]:
            changed = copy.deepcopy(report)
            changed['verification-results'][field] = value
            try:
                check.check_result(base, receipt['status'], json.dumps(changed), stderr, case, mutation, expected)
            except ValueError:
                rejected += 1
            else:
                raise ValueError('accepted altered solver result')
        if mutation:
            rows = [base.unique_json(line) for line in stderr.splitlines()]
            check.check_result(base, receipt['status'], stdout,
                               '\n'.join(json.dumps(row) for row in reversed(rows)), case, mutation, expected)
            diagnostic = next(row for row in rows if row['spans'])
            missing = list(rows)
            missing.remove(diagnostic)
            for changed in (missing, [*rows, diagnostic]):
                try:
                    check.check_result(base, receipt['status'], stdout,
                                       '\n'.join(json.dumps(row) for row in changed), case, mutation, expected)
                except ValueError:
                    rejected += 1
                else:
                    raise ValueError('accepted missing or duplicated diagnostic')
            for field, value in [('message', 'unrelated error'), ('is_primary', 1), ('byte_start', 0)]:
                changed = copy.deepcopy(rows)
                target = next(row for row in changed if row['spans'])
                if field == 'message':
                    target[field] = value
                else:
                    target['spans'][0][field] = value
                try:
                    check.check_result(base, receipt['status'], stdout, '\n'.join(json.dumps(r) for r in changed), case, mutation, expected)
                except ValueError:
                    rejected += 1
                else:
                    raise ValueError('accepted altered diagnostic')
    print('PASS: rejected', rejected, 'altered solver results/diagnostics')
    print('PASS: accepted', len(check.MUTATIONS), 'top-level diagnostic-order reversals')
    performance = read_module(packet / 'performance.py', 'guards_selftest_performance')
    performance.selftest((packet / 'cargo/performance/stdout').read_text())
    modifications = [
        ('benchmark', 'cargo/performance/stdout', None),
        ('owner', 'proof/inputs-after.json', None),
        ('declaration', 'proof/positive-before/crates/fe2o3-runtime-model/src/context_read_leases/declarations.rs', b'// changed owner\n'),
        ('source', 'proof/positive-before/' + str(check.BODY), b'// changed source\n'),
        ('extra', 'proof/positive-before/unexpected.rs', b'// extra staged source\n'),
        ('adapter', 'proof/inputs-before.json', None),
        ('probe', 'proof/source.json', None),
        ('receipt', 'cargo/test/receipt.json', None),
        ('overlap', 'cargo/host/receipt.json', None),
        ('proof-overlap', 'proof/positive-after/solver/record.json', None),
        ('closure-before', 'proof/closure-before/record.json', None),
        ('closure-after', 'proof/closure-after/record.json', None),
        ('bridge', 'proof/positive-before/' + str(check.BRIDGE), b'// changed bridge\n'),
        ('witness', 'proof/positive-before/' + str(check.LIVE), b'// changed witness\n'),
    ]
    for name, relative, addition in modifications:
        with tempfile.TemporaryDirectory(prefix='fe2o3-ordering-negative-') as temporary:
            candidate = Path(temporary) / 'packet'
            shutil.copytree(packet, candidate)
            target = candidate / relative
            if addition is not None:
                target.write_bytes((target.read_bytes() if target.exists() else b'') + addition)
            elif name == 'adapter':
                row = base.unique_json(target.read_text())
                key = next(key for key in row if key.endswith('/context_producer_reads/query.rs'))
                row[key] = '0' * 64
                target.write_text(json.dumps(row))
            elif name == 'owner':
                row = base.unique_json(target.read_text())
                key = 'crates/fe2o3-runtime-model/src/context_producer_reads.rs'
                row[key] = '0' * 64
                target.write_text(json.dumps(row))
            elif name == 'benchmark':
                rows = target.read_text().splitlines(keepends=True)
                index = next(i for i, line in enumerate(rows) if 'producer_query,' in line)
                del rows[index]
                target.write_text(''.join(rows))
                receipt_path = candidate / 'cargo/performance/receipt.json'
                row = base.unique_json(receipt_path.read_text())
                row['stdout_sha256'] = sha(target.read_bytes())
                receipt_path.write_text(json.dumps(row))
            elif name == 'probe':
                row = base.unique_json(target.read_text())
                row['probe'] = True
                target.write_text(json.dumps(row))
            elif name == 'receipt':
                row = base.unique_json(target.read_text())
                row['exit'] = False
                target.write_text(json.dumps(row))
            elif name == 'overlap':
                row = base.unique_json(target.read_text())
                row['started_ns'] = 1
                target.write_text(json.dumps(row))
            elif name in ('proof-overlap', 'closure-after'):
                row = base.unique_json(target.read_text())
                row['started_ns'] = 1
                target.write_text(json.dumps(row))
            elif name == 'closure-before':
                row = base.unique_json(target.read_text())
                row['finished_ns'] = 2 ** 63 - 1
                target.write_text(json.dumps(row))
            else:
                raise ValueError('unhandled packet mutation')
            files = sorted(p for p in candidate.rglob('*') if p.is_file() and p != candidate / 'SHA256SUMS')
            (candidate / 'SHA256SUMS').write_text(''.join(sha(p.read_bytes()) + '  ' + str(p.relative_to(candidate)) + '\n' for p in files))
            try:
                audit(repo, candidate)
            except ValueError:
                pass
            else:
                raise ValueError('accepted rehashed artifact mutation: ' + name)
    print('PASS: rejected', len(modifications), 'rehashed packet mutations')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--packet', type=Path, required=True)
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args()
    if args.selftest:
        selftest(args.repo.resolve(), args.packet.resolve())
    else:
        audit(args.repo.resolve(), args.packet.resolve())
        print('PASS: standalone actual producer query proof and instrumented CPU packet reconstructed from Git objects')


if __name__ == '__main__':
    main()

