# Explicit Peer-Copy Batch CPU Qualification

Status: CPU qualification passed all 21 recorded stages. The GNU and musl
rosters match exactly: 168 KFD tests, 250 runtime/Context tests and 6 benchmark
tests per target. All 6 feature-off benchmark tests and all 11 verifier
calibration tests passed. The final archive is sealed and replayable.

The implementation contract is described in
[`runtime-xgmi-peer-batch-v1.md`](../../runtime-xgmi-peer-batch-v1.md).
The optional aggregate API preserves ordinary submission identities and wait
behavior, retains the whole batch on timeout, restores native mappings before
Success, and closes the full currentness scope before exposing completion.

The 21-stage recipe includes strict all-feature Clippy, no-default-feature
compilation, matching GNU/musl test rosters, all Context regression tests, lower
XGMI and scheduling tests, feature-on/off benchmark CLI tests, unsafe-source
policy, formatting, diff checks and identical before/after source snapshots.
Execution uses two build jobs and two test threads with optimized test code,
debug assertions and overflow checks enabled.

Recorded `binding.json` pins the historical source base, complete source
snapshot, exact test roster hashes, tool identities and command count. The
snapshot's file map is the native build prerequisite; the later signed source
commit may differ from the historical base without changing those file bytes.
`--live` compares the current verifier checkout's selected source files.
The before/after snapshots contain the same 5,578 source files and have SHA-256
`d5f89cfb37c79fd1c3501fe3947aa5cd08561a79dec99b6a70ab82ab4307baee`.

This is a selected regression cohort, not a full-library pass. An earlier,
separate all-feature GNU library run was stopped during unrelated construction
fault matrices and is not counted as passing evidence here.

```sh
python3 -I -B docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/qualify.py
python3 -I -B docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/verify.py --prepare-binding
python3 -I -B docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/test_verify.py
python3 -I -B docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/verify.py --seal
python3 -I -B docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/verify.py --live
```

This is CPU qualification, not native driver failure injection, machine-code
refinement, complete HIP/HSA parity or a performance acceptance result. The
native aggregate panic policy is process abort, not resumable unwind recovery.
