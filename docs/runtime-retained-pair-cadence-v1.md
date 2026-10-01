# Retained XGMI Wait Cadence Experiment

This opt-in `hardware-diagnostic` experiment compares the existing 1 ms sleep
ceiling with a 25 microsecond ceiling. Ordinary runtime entrypoints keep their
existing policy. The 64-spin/16-yield prefix, single deadline, operational checks,
retained custody and final retirement behavior are unchanged.

`Gfx942XgmiRetainedWaitCadenceV1` selects `Ordinary1ms` or `Ceiling25us` for the
explicit retained-scope methods `wait_batch_for_cadence_experiment_v1` and
`wait_batch_for_cadence_diagnostic_v1`. The former disables timing observation;
the latter reports it. A requested sleep ceiling does not bound OS scheduling
latency. The separate producer and strict parser use a distinct experiment
schema; their results must not be relabeled as the ordinary benchmark schema.

## Qualification

Signed candidate `3527956b671b30b96ec34a20866584f0cee507bf` contains the seventeen
reviewed source paths. Its fresh CPU campaign passes all nineteen stages:
1,831 no-default and 1,837 all-feature KFD tests, no failures/ignores/filtering,
both non-test library checks, both strict all-target Clippy modes, source controls
and the example build. All original/retained executables and rejected attempts
are preserved. The CPU result SHA-256 is
`1c154c51a032c34d78b79ae6637772f9489a9c1bc242af854d3253e39b5269f7`.

The separately signed selector campaign passes a full relocated 2/0 proof,
three full 1/1 actual-body mutations with exact one/two/two diagnostic counts,
and both tool-release brackets. All six owned process groups close. The result
SHA-256 is `5b8a1feefe929a41e5b4825708c594349e8ae261c0c21f424706344cd6d3b9d9`.
The three-input proof includes a derived Clone obligation; it proves ceiling
selection, not cursor construction, scheduling, syscalls, hardware behavior or
performance. Macro diagnostic anchors establish source association, not branch
execution witnesses. Earlier diagnostic captures are not qualified negatives.

The raw CPU/proof packets are retained locally under
`/home/harsh/.codex-tmp/fe2o3-cadence-qualification-records-20261001-revision-3/`
and `/home/harsh/.codex-tmp/fe2o3-cadence-selector-qualification-20261001/`.
They are not yet a public, self-contained replay package. Integration preserves
the exact candidate paths and all four existing affected proof closures; only
the four wider source-inventory hashes are rebound.
All 24 local source-workflow commands pass on the integrated worktree, including
the two new cadence source/parser suites. This is not a new Rust or GPU run.

## Remaining Work

Matched MI300X measurements must compare the experiment's ordinary control,
short ceiling, HSA and HIP, separating ordinary from instrumented timing. Shared
host availability is not an exclusive reservation. No default change, speedup,
HIP/HSA parity or A7 milestone exit is justified by the CPU/selector evidence.
