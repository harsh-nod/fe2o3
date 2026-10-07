#!/usr/bin/env python3
"""Verify exact shared logical-window arithmetic; no adapter/DMA refinement."""
import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import signal
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path('crates/fe2o3-runtime-model/verus')
K = Path('crates/fe2o3-kfd/src')
SUPPORT = V / 'compute_xgmi_packet_plan_check.py'
SUPPORT_SHA = '61074f6cdbfde1df24bff3d1e56e04398f2850dd00518c233d847f13a29e0db4'
support_path = ROOT / SUPPORT
if (support_path.is_symlink() or not support_path.is_file()
        or hashlib.sha256(support_path.read_bytes()).hexdigest() != SUPPORT_SHA):
    raise ValueError('pinned packet-plan controller support')
spec = importlib.util.spec_from_file_location('window_support', support_path)
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)
need, digest, save, strict_json = support.need, support.digest, support.save, support.strict_json
BODY = K / 'sdma/compute_xgmi_window_body.rs'
RUST = K / 'sdma/compute_xgmi_window.rs'
PROOF = V / 'compute_xgmi_window_v1.rs'
TEST = V / 'compute_xgmi_window_test.py'
FOCI = {
    'bounds': ('compute_xgmi_window_bounds_v1', 'gfx942_compute_xgmi_window_bounds_v1'),
    'packet': ('compute_xgmi_window_packet_v1', 'gfx942_compute_xgmi_window_packet_v1'),
}
VERIFIER = support.VERIFIER
SCOPE = ('Two actual shared arithmetic bodies and their bounded composition only. '
         'Offsets refer to complete logical allocations; packet planning is a separate theorem. '
         'No proof of owner authenticity, initialization, physical GPU-base addressing, mapping, '
         'currentness, ticket ordering, restoration, DMA, runtime, concurrency, hardware or performance.')


def validate_sources(sources):
    for path, expected in {SUPPORT: SUPPORT_SHA, **support.PINS}.items():
        need(digest(sources[str(path)]) == expected, 'pinned support: ' + str(path))
    rust, body, proof = (sources[str(path)].decode('ascii') for path in (RUST, BODY, PROOF))
    need(re.findall(r'include!\("([^"]+)"\);', rust) == [BODY.name], 'actual Rust include')
    need(re.findall(r'include!\("([^"]+)"\);', proof)
         == ['../../fe2o3-kfd/src/sdma/' + BODY.name], 'actual proof include')
    need('include!' not in body, 'closed shared arithmetic')
    need(not re.search(r'\b(?:assume|admit)\s*\(|external_body|external_fn_specification|\baxiom\b',
                       body + proof), 'no proof trust escape')
    need(re.findall(r'macro_rules!\s+(\w+)', body) == [item[1] for item in FOCI.values()],
         'exact two shared macros')
    compact = re.sub(r'\s+', '', rust.split('#[cfg(test)]', 1)[0])
    need('pubstructGfx942ComputeXgmiCopyWindowV1{source_logical_bytes:u64,destination_logical_bytes:u64,'
         'source_offset:u64,destination_offset:u64,plan:Gfx942ComputeXgmiPacketPlanV1,}' in compact,
         'immutable original logical extents and offsets')
    need('pubstructGfx942ComputeXgmiCopyPacketV1{pubsource_offset:u64,pubdestination_offset:u64,pubbytes:u32,}' in compact,
         'exact packet projection schema')
    constructor = ('pubfnnew(source_logical_bytes:u64,destination_logical_bytes:u64,source_offset:u64,'
                   'destination_offset:u64,bytes:u64,)->Option<Self>{'
                   'if!gfx942_compute_xgmi_window_bounds_v1!(source_logical_bytes,destination_logical_bytes,'
                   'source_offset,destination_offset,bytes){returnNone;}Some(Self{source_logical_bytes,'
                   'destination_logical_bytes,source_offset,destination_offset,'
                   'plan:Gfx942ComputeXgmiPacketPlanV1::new(bytes)?,})}')
    packet = ('pubfnpacket(&self,index:usize)->Option<Gfx942ComputeXgmiCopyPacketV1>{'
              'letpacket=self.plan.packet(index)?;let(source_offset,destination_offset)='
              'gfx942_compute_xgmi_window_packet_v1!(self.source_offset,self.destination_offset,'
              'self.bytes(),packet.offset,packet.bytes)?;Some(Gfx942ComputeXgmiCopyPacketV1{'
              'source_offset,destination_offset,bytes:packet.bytes,})}')
    need(compact.count(constructor) == 1, 'actual complete constructor forwarding')
    need(compact.count(packet) == 1, 'actual complete packet forwarding')
    need('pubfnbytes(&self)->u64{self.plan.total_bytes()}' in compact,
         'actual admitted transfer length')
    for family, (_, macro) in FOCI.items():
        need(compact.count(macro + '!') == 1 and proof.count(macro + '!') == 1,
             'single production/proof invocation: ' + family)
    # These unchanged sources bind the existing relative packet provider, not a new proof of it.
    for path, declaration in support.CONSTANTS.items():
        need(sources[str(path)].decode('ascii').count(declaration) == 1, 'packet constant provider')
    plan = sources[str(support.RUST)].decode('ascii')
    need('compute_xgmi_packet_count_body_v1!' in plan and 'compute_xgmi_packet_at_body_v1!' in plan,
         'existing relative packet provider remains shared')


def source_snapshot():
    paths = [BODY, RUST, PROOF, TEST, Path(__file__).relative_to(ROOT), SUPPORT,
             support.RUST, support.BODY, support.PROOF, *support.PINS, *support.CONSTANTS]
    sources = {}
    for path in paths:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             'ordinary exact source')
        sources[str(path)] = selected.read_bytes()
    validate_sources(sources)
    return sources


def mutations(body):
    cases = {}
    for name, before, after, family in (
        ('accept-zero-window', 'bytes > 0', 'true', 'bounds'),
        ('omit-source-window-end', '&& bytes <= source_len - source_offset', '', 'bounds'),
        ('omit-destination-window-end', '&& bytes <= destination_len - destination_offset', '', 'bounds'),
        ('deny-exact-source-end', 'bytes <= source_len - source_offset', 'bytes < source_len - source_offset', 'bounds'),
        ('deny-exact-destination-end', 'bytes <= destination_len - destination_offset', 'bytes < destination_len - destination_offset', 'bounds'),
        ('accept-zero-packet', 'packet_bytes == 0', 'false', 'packet'),
        ('omit-relative-end', '|| packet_bytes > bytes - relative', '', 'packet'),
        ('substitute-source-base', 'Some((source, destination))', 'Some((destination, destination))', 'packet'),
        ('substitute-destination-base', 'Some((source, destination))', 'Some((source, source))', 'packet'),
        ('omit-source-packet-end', 'packet_bytes > u64::MAX - source', 'false', 'packet'),
        ('omit-destination-packet-end', 'packet_bytes > u64::MAX - destination', 'false', 'packet'),
    ):
        need(body.count(before) == 1, 'unique actual body mutation: ' + name)
        changed = body.replace(before, after)
        need(changed != body, 'nonvacuous mutation')
        cases[name] = (changed, family)
    need(len(cases) == len(set(cases.values())) == 11, 'exact distinct logical mutation roster')
    return cases


def classify(status, stdout, stderr, proof, family=None):
    try:
        data = strict_json(stdout)
        diagnostics = [strict_json(line) for line in stderr.splitlines() if line]
        need(data.get('verus') == VERIFIER, 'exact verifier identity')
        result = data['verification-results']
        need(result.get('encountered-vir-error') is False, 'no translation error')
        errors = [row for row in diagnostics if row.get('level') == 'error']
        need(all(row.get('level') in {'error', 'note'} for row in diagnostics), 'no hidden warnings')
        if family is None:
            return (status == 0 and result.get('encountered-error') is False
                    and result.get('errors') == 0 and result.get('verified') == 3
                    and result.get('success') is True
                    and result.get('is-verifying-entire-crate') is True and not diagnostics)
        focus, macro = FOCI[family]
        need(status == 1 and result.get('encountered-error') is True
             and result.get('errors') == 1 and result.get('verified') == 0
             and result.get('is-verifying-entire-crate') is False, 'one selected logical failure')
        logical = [row for row in errors if row.get('message') == 'postcondition not satisfied']
        abort = [row for row in errors if row.get('message') == 'aborting due to 1 previous error'
                 and row.get('spans') == []]
        need(len(logical) == len(abort) == 1 and len(errors) == 2, 'exact logical failure and abort summary')
        notes = {
            'verifying root module (selected functions)',
            'function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function',
        }
        need(all(row.get('message') in notes for row in diagnostics if row.get('level') == 'note'),
             'exact selected-function notes')
        lines = proof.read_text().splitlines()
        first = next(i + 1 for i, line in enumerate(lines) if line.startswith('fn ' + focus + '('))
        call = next(i + 1 for i, line in enumerate(lines) if line.strip().startswith(macro + '!('))
        body = (proof.parent / '../../fe2o3-kfd/src/sdma/compute_xgmi_window_body.rs').resolve()
        definition = next(i + 1 for i, line in enumerate(body.read_text().splitlines())
                          if line.startswith('macro_rules! ' + macro + ' {'))
        spans = logical[0].get('spans', [])
        contract = any(span.get('is_primary') is True
                       and Path(span.get('file_name', '')).resolve() == proof
                       and first < span.get('line_start', 0) < call for span in spans)
        expansion = any(
            Path(span.get('file_name', '')).resolve() == body
            and span.get('expansion', {}).get('macro_decl_name') == macro + '!'
            and Path(span['expansion']['span']['file_name']).resolve() == proof
            and span['expansion']['span']['line_start'] == call
            and Path(span['expansion']['def_site_span']['file_name']).resolve() == body
            and span['expansion']['def_site_span']['line_start'] == definition
            for span in spans if isinstance(span.get('expansion'), dict))
        return contract and expansion
    except (ValueError, KeyError, TypeError, StopIteration, OSError):
        return False


def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, 'use python3 -I -B')
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--positive-only', action='store_true', help='development only, not full acceptance')
    args = parser.parse_args()
    out, verus = args.output, args.verus
    need(out.is_absolute() and out.resolve() == out and not out.exists()
         and not out.is_relative_to(ROOT), 'fresh output outside repository')
    need(verus.is_absolute() and verus.resolve() == verus and verus.is_file(), 'canonical verifier')
    before = source_snapshot()
    owner = types.ModuleType('window_process_owner')
    owner.__file__ = str(ROOT / support.OWNER)
    sys.modules[owner.__name__] = owner
    exec(compile(before[str(support.OWNER)], owner.__file__, 'exec'), owner.__dict__)
    out.mkdir(parents=True)
    (out / 'tmp').mkdir()
    for path, data in before.items():
        target = out / 'inputs' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    save(out / 'source-before.json', {path: digest(data) for path, data in before.items()})
    home = Path.home().resolve()
    cargo_home = Path(os.environ.get('CARGO_HOME', str(home / '.cargo'))).resolve()
    rustup_home = Path(os.environ.get('RUSTUP_HOME', str(home / '.rustup'))).resolve()
    env = {'HOME': str(home), 'PATH': str(cargo_home / 'bin') + ':/usr/bin:/bin',
           'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8', 'RUSTUP_HOME': str(rustup_home),
           'CARGO_HOME': str(cargo_home), 'VERUS_Z3_PATH': str(verus.parent / 'z3'),
           'TMPDIR': str(out / 'tmp')}
    save(out / 'environment.json', env)
    rows = []
    handlers = {number: signal.getsignal(number) for number in owner.SIGNALS}
    for number in handlers:
        signal.signal(number, owner.interrupted)
    closure_command = ['/bin/sh', str(ROOT / support.CLOSURE), str(verus.parent), str(ROOT / support.MANIFEST)]

    def run(name, command, accept):
        need(source_snapshot() == before, 'source continuity before ' + name)
        status, stdout, stderr = owner.run_owned(command, 130, out / name, env)
        receipt = strict_json((out / name / 'record.json').read_text())
        accepted = bool(receipt.get('group_absent') is True and accept(status, stdout, stderr))
        rows.append({'name': name, 'status': status, 'accepted': accepted})
        print(name + ': ' + ('PASS' if accepted else 'FAIL'), flush=True)
        need(source_snapshot() == before, 'source continuity after ' + name)
        need(accepted, 'rejected stage ' + name)

    def closure(status, stdout, stderr):
        return status == 0 and stdout == 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n' and not stderr

    def proof_run(name, body, family=None):
        staged = out / (name + '-source')
        for path in (BODY, PROOF):
            target = staged / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body.encode('ascii') if path == BODY else before[str(path)])
        source = staged / PROOF
        command = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '120',
                   str(verus), '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating',
                   '--output-json', '--error-format=json', '--no-report-long-running',
                   '--num-threads', '1', '--multiple-errors', '1',
                   *(['--verify-function', '*' + FOCI[family][0], '--verify-root'] if family else []), str(source)]
        run(name, command, lambda status, stdout, stderr: classify(status, stdout, stderr, source, family))

    error = None
    unchanged = False
    try:
        run('controls', [sys.executable, '-I', '-B', str(ROOT / TEST)],
            lambda status, stdout, stderr: status == 0 and not stdout
            and re.findall(r'^Ran (\d+) tests in [0-9.]+s$', stderr, re.M) == ['7']
            and stderr.endswith('\nOK\n'))
        run('release-before', closure_command, closure)
        original = before[str(BODY)].decode('ascii')
        proof_run('positive-before', original)
        if not args.positive_only:
            for name, (body, family) in mutations(original).items():
                proof_run('negative-' + name, body, family)
            proof_run('positive-after', original)
    except BaseException as failure:
        error = type(failure).__name__ + ': ' + str(failure)
    finally:
        try:
            run('release-after', closure_command, closure)
            after = source_snapshot()
            save(out / 'source-after.json', {path: digest(data) for path, data in after.items()})
            unchanged = after == before
        except BaseException as failure:
            error = (error or '') + '; closing: ' + type(failure).__name__ + ': ' + str(failure)
        for number, handler in handlers.items():
            signal.signal(number, handler)
    expected = 4 if args.positive_only else 16
    result = dict(accepted=error is None and unchanged and len(rows) == expected and all(row['accepted'] for row in rows),
                  full_campaign=not args.positive_only, scope=SCOPE, stages=rows, error=error,
                  source_unchanged=unchanged, whole_adapter_verified=False)
    save(out / 'result.json', result)
    print(support.json.dumps(result, indent=2))
    return 0 if result['accepted'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
