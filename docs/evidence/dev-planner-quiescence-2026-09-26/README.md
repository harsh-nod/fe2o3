# Completion Planner and Stream Quiescence Development

Date: 2026-09-26. This is development evidence, not A1/A2 acceptance,
complete Context refinement, native qualification or HIP/HSA parity.

## Source and Scope

The runtime fix is signed commit `cf9dba20dfada19cae343bc819e1036471ef4a9b`.
`mark_stream_quiescent` now sorts its already-collected submission IDs before
settlement. Previously HashMap iteration randomized callbacks and the committed
prefix before a local error, contrary to the documented cleanup order. Sorting
is per stream; no cross-stream callback ordering is claimed. Submission and
polling paths are unchanged.

Two regression groups exercise 64 fresh scripted Contexts, with 16 submissions
per Context. They cover explicit stream destruction and cleanup, journal and
nonjournal operation, successful/rejected/quiescent backend returns, retry,
exact-once ascending callbacks, and a writer-adapter error at the middle ID.
The latter preserves only the lower-ID callback/status prefix, all writer
references, every retained resource count and the original quiescent backend
diagnostic. The Context remains terminal and later cleanup makes no backend
calls. Unknown output retains writer custody; this is not successful completion.

The planner campaign source is signed commit
`f0746982ff9f9233225ac4dd235be7fb49194b0b`. The executable planner and all seven
projection/proof files are unchanged from the earlier 53-check development
root. The teardown sort is not inside that formal boundary.

## Qualification

`run-v7` completes all 35 phases: opening, relocated and closing full positives
each report 53 verified / 0 errors; all 25 named mutations produce clean logical
rejections. These are three measurements of the same 53 checks, not 159 distinct
theorems. Source snapshots match exactly at opening and closing. Both verifier
closure measurements match 190 files / 129,019,839 bytes. All 119 subprocess
records across the retained attempts and supplementary checks confirm their
owned process groups are absent.

Controller calibration covers eight groups; inherited negative-classifier and
source-correspondence calibration cover twelve and eight groups respectively.
The original guard-form early-yield candidate remains unqualified as described
below; it is not included among the 25 successful final candidates.

CPU receipts:

| Lane | Observed result |
| --- | --- |
| Unsorted production baseline with new ordering tests | Both tests fail at the intended callback order/prefix comparison |
| Sorted production, focused tests | 2 passed |
| Unfiltered all-feature runtime library | 1,525 passed, 3 failed, 28 ignored |
| Strict all-feature/all-target runtime Clippy | Passed |
| No-default-feature runtime check | Passed |
| Workspace formatting check | Passed |

The three broad-suite failures are the unchanged `authorized_execution` socket
tests: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. Each reports
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317` in this
environment. They are not skipped or counted as passes. The full suite includes
the existing exact-fuel, retained-prefix and 303-operation graph regressions.
Counts overlap; this is not a fully passing broad CPU lane.

## Campaign Boundary

The controller is
`crates/fe2o3-runtime-model/verus/check-completion-reconciliation-campaign.py`.
It binds 6,052 measured inputs to signed Git blobs, authenticates its inherited
helpers, checks the pinned 190-file Verus distribution before and after, checks
syntax correspondence to the original production body, and replays the complete
proof from an exact eight-file relocation. Every mutant changes one file while
the other seven remain byte-identical. Source continuity is checked around
each owned subprocess; a failed phase prevents a completed `results.json`.

The candidate roster has 21 planner mutations plus four inherited controls:

- Ordinary/custody/peer/producer validation omissions.
- Wrong observed backend or selected ID; wrong descent, parent push or pop.
- Invalid cache reuse after descent/pop; skipping a producer or crossing Pending.
- Promoting quiescent dependencies, Pending/NoEffect input or failed physical
  observations to Success; ignoring a settlement error.
- Wrong Local status, shortened initial fuel and an explicit early-yield return.
- Skipping leaf settlement, promoting Unknown input, omitting failure quarantine
  and rejecting valid custody. The last two change projection adapters, not the
  production body. Thus 23 candidates change the shared production body and two
  change the finite projection.

The controller requires exact pinned result schemas and logical diagnostics
with authenticated primary source paths. It rejects warnings, nested
diagnostics, malformed containers/counters/spans, duplicate JSON keys,
timeouts, parser/type failures and resource failures, even when accompanied by
a logical failure. Exact enumeration notes and include-path normalization are
handled separately; no arbitrary diagnostic text is discarded.

All positive and negative commands use `--multiple-errors 0`: request at most
one counterexample per function, rather than additional searches for errors.
This does not increase the default SMT resource budget. The command retains
`--no-cheating`, four verifier threads, a 120-second timeout, a five-second kill
grace and a 130-second owned-process bound. These checks are not exhaustive
counterexample enumeration. A validation/cache mutation may reject because it
violates proof instrumentation; that alone is not a reachable real-Context bug.

## Retained Attempts

The [evidence archive](receipts.tar.xz) preserves per-command records,
stdout/stderr, source/tool measurements, source snapshots, CPU results and
network checks. `run-v7/results.json` contains the completed campaign.
The archive was compared file-by-file against all 1,073 retained files:
26,021,416 uncompressed bytes, 564,232 archive bytes, with exact content hashes.
Archive SHA-256:
`f2e446d0f3fab2682883ad664ba2cfd068c8d7da36ddd5b72aa1afec312227d4`.
Earlier failed attempts are not retroactively marked qualified:

| Attempt | Outcome |
| --- | --- |
| `run` | Symlinked verifier directory failed closure discovery before solver execution |
| `run-v2` | Full positives passed; `omit-ordinary` mixed an assertion failure with an rlimit error and was rejected |
| `first-error-probe` | Diagnostic-only first-error run rejected `omit-ordinary` without rlimit |
| `run-v3` | 53/0 solver result; old positive classifier rejected enumeration notes |
| `run-v4` | 53/0 solver result; positive classifier rejected the absolute include path containing `../..` |
| `run-v5` | Full positives and seven negatives passed; a genuine `loop invariant not satisfied` diagnostic was not yet recognized |
| `run-v6` | Full positives and twenty negatives passed; guard-form `early-yield` mixed a postcondition failure with rlimit and was rejected |
| `early-return-probe` | Diagnostic-only equivalent early-return encoding produced a clean postcondition failure |
| `run-v7` | All 35 phases passed with exact closing source and verifier measurements |

The final `early-yield-return` candidate keeps the original loop guard and
returns before decrement when one iteration remains. Like the rejected
`remaining > 1` guard candidate, it performs 512 iterations and returns the
same requested status; earlier exits and state changes are unchanged. This is
a distinct executable encoding of the same early-yield fault. The original
guard encoding remains unqualified. Neither the contract nor resource limits
were relaxed to accept it.

## Reproduction and Remaining Work

From the signed campaign source, use a new absolute output directory outside
the checkout and the canonical pinned release executable, not a wrapper or
symlinked release directory:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/test-completion-reconciliation-campaign.py
python3 -I -B crates/fe2o3-runtime-model/verus/check-completion-reconciliation-campaign.py \
  --output /absolute/new/campaign-output \
  --verus /absolute/canonical/pinned-release/verus
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo test --offline \
  -p fe2o3-runtime --all-features --lib quiescence_order_tests
```

The model still uses a finite Vec table, not the actual Context HashMaps,
complete custody metadata, real journal ownership/effects, callbacks or
quarantine implementation. Synthetic settlement prefixes and root/input
observations do not prove those adapters. The named mutation coverage does not
close protected admission or constitute complete production refinement.

MI300X access still failed hostname resolution; no remote job or file was
created. No GPU timing, copy bandwidth, physical overlap or HIP/HSA performance
result is introduced. A1/A2, native/generated execution, multi-device and
distributed qualification, and performance/release gates remain open.
