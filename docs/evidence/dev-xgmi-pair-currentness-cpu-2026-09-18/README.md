# XGMI Pair Currentness CPU Qualification

All 21 recorded commands passed with no owned process group remaining at command
closure. The selected source inventory was unchanged across the run. GNU and
musl each passed 148 KFD tests, 30 runtime tests, and four benchmark-control tests,
with identical test membership across targets. Feature-off benchmark controls
passed four tests. The unsafe-source policy passed five tests with its one
explicit maintenance test ignored. Strict all-features/all-targets Clippy,
no-default compilation, formatting, and diff checks passed.

This packet covers scoped CPU implementation/regression tests, not native
execution, driver fault injection, formal refinement, or performance acceptance.
The 13 new top-level tests exercise the shared full-observation driver and paired
terminal-state guard, including callback errors/panics, both directions,
identity/route/snapshot mismatches, exact short-circuiting, and inert reentry.
These are scripted CPU observations, not native ioctl failure injection.

The fixed 21-command driver reuses the SHA-pinned recorder and command plan.
It includes the preceding retirement/SDMA regressions, all currentness tests,
pair-session guards, device admission tests, and topology parser/route tests.
Test concurrency is limited to two CPU threads. Test builds use optimization
level one with debug assertions and overflow checks enabled.

Run tools are pinned before execution. The post-run verifier separately binds
source, exact commands, statuses, stream digests, chronology, and process-group
closure. Recorded execution paths stay fixed while `--live` compares the current
checkout's selected file map, including untracked compiler inputs. This run used
base `dccf5cb85ab79c31e493f0efe4e9b7ac76fd1770` plus the recorded implementation
changes. Its 5,572-file source snapshot SHA-256 is
`87462da0047e92771bdb69f20c4db1eec8cd89e60ee839b89074090562f2f872`.
The inventory binds selected compiler inputs, not arbitrary host state.

The design and changed observation boundary are documented in
`docs/runtime-xgmi-pair-currentness-v1.md`. That descriptive document is outside
the frozen compiler-input selector and was finalized while qualification ran.
The post-run verifier separately pins its exact digest, including its proposed
matched baseline/candidate protocol. That protocol has not been executed or
accepted by this CPU packet.

The verifier passed 14 calibration tests, including portable verification from
a relocated checkout and rejection of a changed external design document.
The final `SHA256SUMS` seals the complete archive, including the post-run
verifier, calibration tests, and this report. Raw command output is retained
without whitespace normalization.

Acceptance:

```sh
python3 -I docs/evidence/dev-xgmi-pair-currentness-cpu-2026-09-18/verify.py --live
python3 -I docs/evidence/dev-xgmi-pair-currentness-cpu-2026-09-18/test_verify.py
```

Earlier sealed packets are unchanged and do not qualify these new source bytes.
