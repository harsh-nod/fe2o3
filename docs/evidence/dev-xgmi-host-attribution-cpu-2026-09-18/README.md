# XGMI Host Attribution CPU Qualification

All 21 recorded CPU qualification commands passed, with unchanged source before
and after the run. The isolated authoritative verifier and all 13 post-run
calibrations passed. This packet is for the opt-in lower/runtime XGMI host-stage
diagnostic, not native execution, formal refinement, or performance acceptance.
The earlier settled MI300X run used source `31ebb8fc` and did not exercise this
instrumentation.

## Scope

The fixed 21-command run passed 34 selected KFD tests, 25 runtime tests, and four
benchmark tests on each of GNU and musl. The four benchmark controls also passed
with the diagnostic feature disabled. Strict all-features all-targets Clippy,
all-targets no-default-feature compilation, formatting, whitespace, and
source-before/source-after equality passed. The unsafe-source suite passed five
tests and intentionally ignored one maintenance-only test. Every recorded command
exited successfully and its owned process group was absent at closure.

The source inventory is based on `8b1ab89f48dd9dfedefef2c4b9c344e97e2c1e65` plus
the diagnostic implementation, frozen before the first test. Its 5,564 files
include both new diagnostic modules. The exact snapshot SHA-256 is
`c25a9d1fb3046510f06034b414be379bcaa702a1167ef044f68308e8749cff3a`.
This is a selected compiler-input inventory, not a claim of hermetic host state.
The descriptive `docs/runtime-xgmi-copy-diagnostics-v1.md` is separately digest
bound by acceptance; it was not an input to, or part of, the CPU source-continuity
check. The subsequent signed commit contains the qualified code bytes.

CPU tests cover timers, bounded recorder state, an actual recorder-state unwind,
source-wiring order guards, lifecycle gates, and benchmark controls. They do not
instantiate live KFD sessions or inject native driver faults. The source-order
test is a wiring guard, not a Rust/native refinement proof.

## Acceptance

Use the isolated authoritative verifier, not the preserved launch-time draft:

```sh
python3 -I docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/accept.py --live
python3 -I docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/test_accept.py
```

`accept.py` authenticates all repository helpers before loading them, pins the
exact source snapshot and GNU/musl test identities, checks every exact command,
receipt, chronological boundary, output digest and successful test closure, and
requires the complete file/directory roster and seal. Thirteen independently run,
reproducible post-run calibrations (outside the 21 recorded CPU commands)
exercise substitution, order, type, roster, source, symlink, missing/extra file,
duplicate-key and seal failures on disposable copies. They never mutate the
recorded run. `--live` compares current code bytes, ignoring later commit identity.

The pre-run `qualify.py`, `verify.py`, and `tools.json` remain byte-for-byte frozen.
Review found the original `verify.py` draft insufficiently strict; it is retained
only to preserve the recorded launch-tool binding and is not the acceptance
entry point. `accept.py` supersedes it without rewriting any raw record or source
snapshot. Acceptance does not rerun benchmarks or grant parity/profiling authority.
