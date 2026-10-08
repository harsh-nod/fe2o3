#!/usr/bin/env python3
"""Signed-source cached-prefix replay; conditional key laws, not full Context or native."""
import argparse
import hashlib
import types
import os
from pathlib import Path
import shutil
import signal
import sys
import time

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[2]
CONTROLLER = BASE / 'check-compute-pipeline-publication.py'
CONTROLLER_SHA = '1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e'
SUPPORT = BASE / 'pins/CONTEXT_CACHED_POLL_SUPPORT_V1.json'
SUPPORT_PIN = BASE / 'pins/CONTEXT_CACHED_POLL_SUPPORT_V1_SHA256'


def need(value, message):
    if not value:
        raise ValueError(message)


def load(name, path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path, 'ordinary helper source')
    raw = path.read_bytes()
    module = types.ModuleType(name)
    module.__file__ = str(path)
    sys.modules[name] = module
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


def support_check(source):
    return source.support_check()


def resources(out):
    fields = {line.split(':')[0]: line.split(':')[1].strip().split()
              for line in Path('/proc/meminfo').read_text().splitlines()}
    need(fields['MemAvailable'][1] == 'kB' and int(fields['MemAvailable'][0]) * 1024 >= 16 * 1024**3,
         'original 16 GiB available-memory admission floor')
    total = 0
    for path in out.rglob('*'):
        need(not path.is_symlink(), 'ordinary owned evidence tree')
        if path.is_file():
            total += path.stat().st_size
    need(total <= 1024**3, 'at most 1 GiB owned campaign outputs')


def proof_command(verus, root, source):
    return ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '120', str(verus),
            '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
            '--error-format=json', '--num-threads', '4', str(root / source.PROOF)]


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, 'python3 -I -B required')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output
    need(out.is_absolute() and out.resolve() == out and not out.exists(), 'fresh canonical owned output')
    need(not out.is_relative_to(ROOT) and not ROOT.is_relative_to(out), 'output outside repository')
    need(args.verus.is_absolute() and args.verus.resolve() == args.verus and args.verus.is_file(), 'canonical verifier')
    source = load('cached_sources', BASE / 'check-context-cached-poll-v1.py')
    source.audit(source.snapshot())
    support_before = support_check(source)
    mutation = load('cached_mutations', BASE / 'context-cached-poll-mutations-v1.py')
    diagnostics = load('cached_diagnostics', BASE / 'context-cached-poll-diagnostics-v1.py')
    policy = diagnostics.parse(source.ordinary(BASE / 'pins/CONTEXT_CACHED_POLL_DIAGNOSTICS_V1.json')[0].decode())
    diagnostics.policy_check(policy)
    source.PROOF_FILES = (source.BODY, source.PROOF)
    selector = BASE / 'pins/CONTEXT_CACHED_POLL_TOOLCHAIN.toml'
    selector_raw, selector_mode = source.ordinary(selector)
    need(source.record(selector_raw, selector_mode) == policy['positive']['inputs']['rust-toolchain.toml'],
         'exact separately signed calibration selector; repository nightly unchanged')
    cases = mutation.mutations((ROOT / source.BODY).read_text())
    need(set(policy['cases']) == set(cases), 'exact calibrated mutation roster')
    need(hashlib.sha256(source.ordinary(CONTROLLER)[0]).hexdigest() == CONTROLLER_SHA, 'unchanged inherited controller')
    base = load('cached_base', CONTROLLER)
    classifier = base.inherited()
    leaf = classifier.inherited()
    prior = leaf.load('cached_signed_sources', ROOT / leaf.PRIOR, leaf.PRIOR_SHA)
    prior.INPUTS = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'crates', 'examples', 'tests',
                    str(classifier.LEAF), str(classifier.LEAF.with_name('test-run.py')), str(leaf.PRIOR),
                    'scripts/qualify-runtime-production-proofs.sh', 'scripts/tests/runtime-production-proof-pipeline.py',
                    '.github/workflows/runtime-model-verus.yml']
    os.chdir(ROOT)
    prior.clean_source()
    tools = prior.source_tools()
    owner = leaf.load('cached_process_owner', ROOT / prior.CONTROLLER, prior.CONTROLLER_SHA)
    for number in owner.SIGNALS:
        signal.signal(number, owner.interrupted)
    out.mkdir(mode=0o700)
    before = dict(commit=prior.git('rev-parse', 'HEAD').decode().strip(), inputs=prior.snapshot())
    prior.save(out / 'source-before.json', before)
    prior.save(out / 'support-before.json', support_before)
    signer = out / 'allowed-signers'
    signer.write_text(prior.SIGNER)
    env = dict(os.environ, VERUS_Z3_PATH=str(args.verus.parent / 'z3'), TMPDIR=str(out))
    prior.save(out / 'environment.json', {key: env.get(key) for key in
               ('PATH', 'RUSTUP_TOOLCHAIN', 'LD_LIBRARY_PATH', 'VERUS_Z3_PATH', 'TMPDIR')})
    results = {}

    def current():
        return dict(commit=prior.git('rev-parse', 'HEAD').decode().strip(), inputs=prior.snapshot())

    def phase(name, command, accept, environment=env, cwd=ROOT):
        resources(out)
        need(before == current() and support_before == support_check(source), 'source continuity before ' + name)
        started = time.monotonic()
        previous = Path.cwd()
        try:
            os.chdir(cwd)
            status, stdout, stderr = owner.run_owned(command, 130, out / name, environment)
        finally:
            os.chdir(previous)
        passed = accept(status, stdout, stderr)
        results[name] = dict(passed=passed, status=status, cwd=str(cwd), elapsed_seconds=time.monotonic() - started)
        prior.save(out / (name + '.json'), results[name])
        print(name + (': PASS' if passed else ': FAIL'), flush=True)
        need(before == current() and support_before == support_check(source), 'source continuity after ' + name)
        resources(out)
        need(passed, 'failed phase: ' + name)

    def exact(text):
        return lambda status, stdout, stderr: status == 0 and stdout == text and not stderr

    def classify(root, name='positive'):
        return lambda s, o, e: diagnostics.classify(s, o, e, root, policy, name)

    tools.authenticate(tools.GIT)
    prior.signature_tool()
    phase('source-signature', [str(tools.GIT), '--no-replace-objects', '--no-pager', '-c', 'gpg.format=ssh',
          '-c', 'gpg.ssh.program=/usr/bin/ssh-keygen', '-c', 'gpg.ssh.allowedSignersFile=' + str(signer),
          'verify-commit', before['commit']], lambda s, o, e: s == 0 and not o and e == prior.SIGNATURE,
          tools.TOOL_ENV)
    prior.signature_tool()
    prior.save(out / 'signed-inputs.json', leaf.bind_signed_blobs(prior, before))
    phase('source-and-classifier-controls', [sys.executable, '-I', '-B', str(BASE / 'test-context-cached-poll-diagnostics-v1.py')],
          exact('PASS: cached-prefix source and diagnostic controls\n'))
    closure = ['/bin/sh', str(ROOT / 'examples/row_softmax_v1/verify-verus-closure.sh'), str(args.verus.parent),
               str(BASE / 'pins/VERUS_CLOSURE_MANIFEST')]
    closure_ok = exact('PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n')
    phase('closure-before', closure, closure_ok)
    nominal = out / 'nominal-source'
    expected = {str(path): before['inputs'][str(path)] for path in source.PROOF_FILES}
    expected['rust-toolchain.toml'] = before['inputs'][str(selector.relative_to(ROOT))]
    for path in source.PROOF_FILES:
        destination = nominal / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, destination)
        destination.chmod(policy['positive']['inputs'][str(path)]['mode'])
    shutil.copyfile(selector, nominal / 'rust-toolchain.toml')
    (nominal / 'rust-toolchain.toml').chmod(selector_mode)
    need(leaf.tree(nominal) == expected, 'exact signed nominal proof and explicit selector')
    prior.save(out / 'nominal-inputs.json', expected)
    phase('proof-before', proof_command(args.verus, nominal, source), classify(nominal), cwd=nominal)
    relocated = out / 'relocated-source'
    shutil.copytree(nominal, relocated)
    need(leaf.tree(relocated) == expected, 'exact signed executable proof relocation')
    prior.save(out / 'relocated-inputs.json', expected)
    phase('relocated-proof', proof_command(args.verus, relocated, source), classify(relocated), cwd=relocated)
    for name, row in cases.items():
        mutated = out / (name + '-source')
        shutil.copytree(relocated, mutated)
        (mutated / source.BODY).write_text(row['text'])
        measured = leaf.tree(mutated)
        need(measured.keys() == expected.keys() and
             {path for path in expected if measured[path] != expected[path]} == {str(source.BODY)},
             'one actual executable mutation; no changed theorem or selection')
        prior.save(out / (name + '-mutation.json'), dict(macro=row['macro'], before=row['before'],
                   after=row['after'], inputs=measured, whole_root=True))
        phase(name, proof_command(args.verus, mutated, source), classify(mutated, name), cwd=mutated)
        need(leaf.tree(mutated) == measured, 'mutant continuity')
    need(leaf.tree(nominal) == expected, 'unchanged original nominal proof before final positive')
    phase('proof-after', proof_command(args.verus, nominal, source), classify(nominal), cwd=nominal)
    phase('closure-after', closure, closure_ok)
    prior.clean_source()
    need(before == current() and leaf.tree(nominal) == leaf.tree(relocated) == expected, 'closing exact source continuity')
    source.audit(source.snapshot())
    need(support_before == support_check(source), 'closing support continuity')
    prior.save(out / 'source-after.json', current())
    prior.save(out / 'results.json', results)
    prior.save(out / 'scope.json', dict(scope=diagnostics.SCOPE, key_law_premises=2, trusted_vstd=True,
               positives=3, logical_negatives=16, verification_obligations=21,
               full_context=False, native=False, commit=before['commit']))


if __name__ == '__main__':
    main()
