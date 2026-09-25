#!/usr/bin/env python3
"""Unsigned development calibration; this is not a qualification campaign."""
import json
from pathlib import Path
import runpy
import signal

repo = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
output = Path(__file__).resolve().parent / 'mutation-development'
output.mkdir()
c = runpy.run_path(str(repo / 'crates/fe2o3-runtime-model/verus/check-completion-journal-effects.py'))
loaded = c['load'](repo)
control, owner, old, lifecycle, deps, *_, paths = loaded
base, scalar, legacy = (deps[key] for key in ('base', 'scalar', 'legacy'))
candidate = {p: old.ordinary(repo, p) for p in paths}
historical_body = old.git(repo, 'show', c['CONTROL_BASELINE'] + ':' + str(c['BODY']))
changes = c['control_mutations'](candidate, historical_body, control) | c['effect_mutations'](candidate)
verus = Path('/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus')
env = dict(HOME='/home/harsh', PATH='/home/harsh/.cargo/bin:/usr/bin:/bin',
           CARGO_HOME='/home/harsh/.cargo', RUSTUP_HOME='/home/harsh/.rustup',
           VERUS_Z3_PATH=str(verus.parent / 'z3'), TMPDIR=str(output))
for number in base.SIGNALS:
    signal.signal(number, base.interrupted)
owner.materialize(output / 'baseline', candidate)
(output / 'inputs.json').write_text(json.dumps({str(p): c['sha'](data) for p, data in sorted(candidate.items())}, indent=2) + '\n')
stage = output / 'sources-development'
for name, (edits, _, _) in changes.items():
    owner.materialize(stage, candidate | edits)
    c['scan'](stage, deps)
    inventory = old.stage_map(stage)
    command = c['proof_command'](loaded, verus, stage, name, changes)
    status, stdout, stderr = base.run_owned(command, int(command[4]) + 10, output / name, env)
    row = base.unique_json((output / name / 'record.json').read_text())
    old.stage_gate(stage, inventory, row, base)
    observed = legacy.normalized(base, stdout, stderr, stage)
    scalar.check_verifier(observed['verus'])
    lifecycle.check_negative(scalar, name, status, observed)
    print(name, 'logical rejection', flush=True)
print('PASS: 30 unsigned development mutations; authenticated campaign still required', flush=True)
