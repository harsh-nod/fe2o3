#!/usr/bin/env python3
"""Check the pinned production planner sources and their shared Verus execution."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

MODEL = Path('crates/fe2o3-runtime-model')
ROOT = MODEL / 'verus/r75_resource_domain_v1.rs'
VECTOR = MODEL / 'src/resource_vector_bodies.rs'
BODY = MODEL / 'src/r75_resource_domain_bodies.rs'
CLOSURE = Path('examples/row_softmax_v1/verify-verus-closure.sh')
TOOLS = MODEL / 'verus/pins/VERUS_CLOSURE_MANIFEST'
MANIFEST = MODEL / 'verus/pins/R75_DOMAIN_SOURCES_SHA256'
MANIFEST_SHA = 'b8a02242c33aef316f6c0af54490a7cd26e84657e60f294a806c269990d47250'
TOOL_SHA = 'd97501a883931d1d173b1bf4b6cf4d973f16d105dbcb468e177b52b2331612d2'
CLOSURE_SHA = 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c'
TOOLS_SHA = 'f06883e4ce463bcb9a3c8f911064ac85054c7822dc331db1a79f75f9e8878b01'
PROOF_FILES = frozenset((ROOT, VECTOR, BODY,
    MODEL / 'src/resource_vector_declarations.rs',
    MODEL / 'src/r75_resource_domain_declarations.rs'))
SOURCES = PROOF_FILES | frozenset((MODEL / 'src/r67_resource_credits.rs',
    MODEL / 'src/r70_resource_batch.rs', MODEL / 'src/r75_resource_domain.rs',
    MODEL / 'src/lib.rs', MODEL / 'Cargo.toml',
    Path('crates/fe2o3-resource-accounting/src/domain.rs'),
    Path('crates/fe2o3-resource-accounting/Cargo.toml'),
    Path('Cargo.toml'), Path('Cargo.lock')))
INTERRUPTS = frozenset((signal.SIGINT, signal.SIGTERM, signal.SIGHUP))


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def ordinary(path):
    need(path.is_absolute(), 'absolute path')
    for part in (path, *path.parents):
        need(not part.is_symlink(), 'no symlink: ' + str(part))
    need(path.is_file(), 'ordinary file: ' + str(path))
    return path.read_bytes()


def source_gate(manifest, captured):
    need(sha(manifest) == MANIFEST_SHA, 'reviewed source manifest')
    pins = {}
    for line in manifest.decode('ascii').splitlines():
        match = re.fullmatch(r'([0-9a-f]{64})  ([A-Za-z0-9_./-]+)', line)
        need(match is not None, 'canonical source manifest row')
        path = Path(match[2])
        need(path not in pins, 'duplicate source manifest row')
        pins[path] = match[1]
    need(set(pins) == SOURCES == set(captured), 'complete exact source roster')
    for path, digest in pins.items():
        need(sha(captured[path]) == digest, 'reviewed source: ' + str(path))
    # Exact reviewed proof bytes fix ghost-only annotations. Rust must erase all
    # hooks, not merely compile another invocation of the same macro.
    production = captured[MODEL / 'src/r75_resource_domain.rs'].decode()
    invocation = 'resource_domain_path_body_v1!(resource_domain_rust_expr,facts,depth,profile,charges,owner,plan,total,level,next,[],[],[],[],[],[],[],[])'
    need(production.count('resource_domain_path_body_v1!') == 1
         and invocation in re.sub(r'\s+', '', production), 'one hook-free production path body')


def replace(data, before, after):
    before, after = before.encode(), after.encode()
    need(data.count(before) == 1, 'unique mutation target: ' + before.decode())
    return data.replace(before, after)


def mutations(captured):
    roster = [
        ('omit-last-ancestor', BODY, 'while $level < $depth', 'while $level + 1 < $depth'),
        ('reverse-ancestors', BODY, 'let fact = $facts[$level];', 'let fact = $facts[$depth - 1 - $level];'),
        ('omit-last-member', BODY, 'while $member < $charges.len()', 'while $member < $charges.len() - 1'),
        ('omit-retained-count', BODY, '$facts.counts[0].checked_add($facts.counts[1])', '$facts.counts[0].checked_add(0)'),
        ('omit-quarantine-count', BODY, 'occupied.checked_add($facts.counts[2])', 'occupied.checked_add(0)'),
        ('wrong-record-error', BODY, 'return Err(R75ResourceDomainErrorV1::RecordCapacity)', 'return Err(R75ResourceDomainErrorV1::Capacity)'),
        ('wrong-header-error', BODY, 'if $owner == 0 { return Err(R75ResourceDomainErrorV1::Invariant); }', 'if $owner == 0 { return Err(R75ResourceDomainErrorV1::GenerationExhausted); }'),
        ('wrong-summary', BODY, 'r67_resource_release_v1(next, fact.used)', 'r67_resource_release_v1(next, next)'),
        ('zero-parent-charge', BODY, 'r67_resource_reserve_v1(fact.used, $total, fact.capacity)', 'r67_resource_reserve_v1(fact.used, R67ResourceVectorV1::ZERO, fact.capacity)'),
        ('skip-owner-advance', BODY, '$plan.next_owner = next_owner;', '$plan.next_owner = $owner;'),
        ('skip-reserved-advance', BODY, '$plan.next_reserved[$level] = reserved;', '$plan.next_reserved[$level] = fact.counts[0];'),
        ('skip-used-commit', BODY, '$plan.next_used[$level] = $next;', '$plan.next_used[$level] = fact.used;'),
        ('wrong-reserve-coordinate', VECTOR, '$used.counts[$index].checked_add($charge.counts[$index])', '$used.counts[$index].checked_add($charge.counts[0])'),
        ('wrong-release-coordinate', VECTOR, '$used.counts[$index].checked_sub($charge.counts[$index])', '$used.counts[$index].checked_sub($charge.counts[0])'),
    ]
    result = [(name, path, replace(captured[path], before, after)) for name, path, before, after in roster]
    # Both vector operations must cover the final coordinate.
    text = captured[VECTOR]
    target = b'while $index < R67_RESOURCE_DIMENSIONS_V1'
    need(text.count(target) == 2, 'both vector loops')
    result.append(('omit-final-coordinate', VECTOR, text.replace(target, b'while $index + 1 < R67_RESOURCE_DIMENSIONS_V1')))
    need(len({name for name, _, _ in result}) == 15, 'distinct mutation names')
    need(len({(path, data) for _, path, data in result}) == 15, 'distinct mutation bodies')
    return result


def classify(code, log, negative=False):
    summaries = re.findall(rb'^verification results:: (\d+) verified, (\d+) errors$', log, re.M)
    if not negative:
        need(code == 0 and summaries == [(b'19', b'0')], 'exact positive proof result')
        need(not re.search(rb'^error', log, re.M), 'no positive errors')
    else:
        need(code == 1 and len(summaries) == 1 and int(summaries[0][1]) > 0, 'logical negative, not compiler error/timeout')
        need(sum(map(int, summaries[0])) == 19, 'complete negative obligation roster')
        need(re.search(rb'^error: (postcondition not satisfied|precondition not satisfied|invariant not satisfied|assertion failed)', log, re.M), 'named verification rejection')
        errors = re.findall(rb'^error[^\n]*', log, re.M)
        allowed = rb'error: (postcondition not satisfied|precondition not satisfied|invariant not satisfied.*|assertion failed|aborting due to \d+ previous errors?)'
        need(all(re.fullmatch(allowed, error) for error in errors), 'no unrelated negative errors')
        need(not re.search(rb'timed out|resource limit|internal error|panicked|out of memory', log, re.I), 'no verifier infrastructure failure')


def execute(command, cwd, env, output, name):
    (output / (name + '.command.json')).write_text(json.dumps(command) + '\n')
    # Single-threaded POSIX runner: defer handled signals until ownership of the
    # new group is installed. The child restores the caller's mask before exec.
    previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, INTERRUPTS)
    process = None
    timeout = False
    try:
        process = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, start_new_session=True,
            preexec_fn=lambda: signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask))
        (output / (name + '.pid.json')).write_text(json.dumps({'pid': process.pid, 'pgid': process.pid}) + '\n')
        signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
        log = process.communicate(timeout=120)[0]
    except subprocess.TimeoutExpired:
        timeout = True
        log = stop(process)
    except BaseException:
        if process is not None:
            log = stop(process)
            (output / (name + '.log')).write_bytes(log)
            (output / (name + '.exit.json')).write_text(json.dumps({'exit': process.returncode, 'interrupted': True}) + '\n')
        raise
    finally:
        try:
            if process is not None:
                stop(process)
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
    (output / (name + '.log')).write_bytes(log)
    (output / (name + '.exit.json')).write_text(json.dumps({'exit': process.returncode, 'timeout': timeout}) + '\n')
    need(not timeout, 'timed out: ' + name)
    return process.returncode, log


def stop(process):
    previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, INTERRUPTS)
    try:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            log = process.communicate(timeout=5)[0]
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            log = process.communicate(timeout=5)[0]
        deadline = time.monotonic() + 5
        while True:
            # Reap only adopted descendants of this owned process group.
            try:
                while os.waitpid(-process.pid, os.WNOHANG)[0] != 0:
                    pass
            except ChildProcessError:
                pass
            try:
                os.killpg(process.pid, 0)
            except ProcessLookupError:
                return log
            need(time.monotonic() < deadline, 'owned process group still exists')
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            time.sleep(0.02)
    finally:
        signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)


def install_handlers():
    need(sys.platform == 'linux', 'Linux qualification runner required')
    libc = ctypes.CDLL(None, use_errno=True)
    need(libc.prctl(36, 1, 0, 0, 0) == 0, 'PR_SET_CHILD_SUBREAPER')
    def interrupted(signum, _frame):
        raise KeyboardInterrupt('interrupted by signal ' + str(signum))
    for signum in INTERRUPTS:
        signal.signal(signum, interrupted)


def stage(output, name, captured, changed=None):
    root = output / ('sources-' + name)
    root.mkdir()
    for path in sorted(PROOF_FILES):
        destination = root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        data = changed[1] if changed and path == changed[0] else captured[path]
        destination.write_bytes(data)
    return root


def main():
    install_handlers()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    output = args.output.absolute()
    need(output.resolve() == output and not output.exists(), 'fresh ordinary output directory')
    output.mkdir()
    captured = {path: ordinary(repo / path) for path in SOURCES}
    manifest = ordinary(repo / MANIFEST)
    source_gate(manifest, captured)
    (output / 'source-manifest.sha256').write_bytes(manifest)
    # Capture adapter/wrapper bytes as well as the executed include closure.
    for path, data in captured.items():
        destination = output / 'captured' / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    verus = args.verus.absolute()
    need(sha(ordinary(verus)) == TOOL_SHA, 'pinned Verus binary')
    closure, tool_manifest = ordinary(repo / CLOSURE), ordinary(repo / TOOLS)
    need(sha(closure) == CLOSURE_SHA and sha(tool_manifest) == TOOLS_SHA, 'pinned tool closure authority')
    checker = output / 'verify-verus-closure.sh'
    checker.write_bytes(closure)
    tool_pin = output / 'verus-closure-manifest'
    tool_pin.write_bytes(tool_manifest)
    home = str(Path.home())
    env = {'PATH': home + '/.cargo/bin:/usr/bin:/bin', 'HOME': home,
        'RUSTUP_HOME': home + '/.rustup', 'CARGO_HOME': home + '/.cargo',
        'LC_ALL': 'C', 'VERUS_Z3_PATH': str(verus.parent / 'z3')}
    closure_command = ['/bin/sh', str(checker), str(verus.parent), str(tool_pin)]
    closure_env = {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}
    code, _ = execute(closure_command, repo, closure_env, output, 'closure-before')
    need(code == 0, 'opening tool closure')
    code, log = execute([str(verus), '--version'], repo, env, output, 'version')
    need(code == 0 and b'0.2026.08.09.92f466f' in log, 'pinned Verus version')
    cases = [('positive-before', None)] + [(name, (path, data)) for name, path, data in mutations(captured)] + [('positive-after', None)]
    for name, changed in cases:
        root = stage(output, name, captured, changed)
        command = [str(verus), '--crate-type', 'lib', '--no-cheating', '--num-threads', '2', '--triggers-mode', 'silent', str(root / ROOT)]
        code, log = execute(command, root, env, output, name)
        classify(code, log, negative=changed is not None)
    code, _ = execute(closure_command, repo, closure_env, output, 'closure-after')
    need(code == 0 and sha(ordinary(verus)) == TOOL_SHA, 'closing tool closure')
    source_gate(ordinary(repo / MANIFEST), {path: ordinary(repo / path) for path in SOURCES})
    (output / 'result.json').write_text(json.dumps({'status': 'passed', 'obligations_per_positive': 19,
        'positive_runs': 2, 'logical_mutations': 15, 'source_manifest_sha256': MANIFEST_SHA,
        'scope': 'shared pure planner and vector bodies; adapter source identity, not arena/lock/native refinement'}, indent=2) + '\n')
    print('R75_SHARED_DOMAIN_PLANNER_OK obligations=19 positives=2 mutations=15')


if __name__ == '__main__':
    main()
