# Lifecycle capture and CPU console qualification — 2026-09-19

These are two distinct qualified implementation slices. The lifecycle owner
remains a private test integration; the console uses the existing public JSONL
debugger protocol and does not expose that owner. Neither closes #281 V2.

## Same-execution lifecycle observation

The additive simulator entry
`AdmittedSimulationModuleV1::simulate_debugged_scheduled_with_sinks` pairs the
existing event and debug sinks in one execution. Existing preflight, default
execution, single-sink behavior and hard event-sink errors remain unchanged.

The private debugger owner retains the actual allocation event ledger and
ObservedSession together. Lifecycle transitions are indexed against accepted
debug records: a boundary applies before its indexed record, and terminal
boundary N is not attributed to N−1. Selection borrows this owner, refuses
foreign capture/cursor identity, and joins bytes/initialization only to the
same checkpoint. Invocation and workgroup scope are retained. Prefix gaps and
execution failure do not claim successful terminal state.

Global allocations are preexisting-live. Actual workgroup allocations supply
create/release events; generation and frame activation remain NotRepresented.
Monotone, non-recycled allocation IDs do not demonstrate reuse or generations.
The retained ledger caps at 65,536 rows, 16 MiB capacity and 16 million work
units, with one upfront reservation; these are logical bounds, not process RSS.

Actual ordinary-source lifecycle r2 passed four contextual and four legacy
runs with six create/release pairs, output/init/canary and reverse checks.
The exporter produced ordinary production KIR8 → same-module simulation KIR10
Bundle V5. Exact bundle identity:
`ddc3f2f2662807125a71c48a8e696ffe031ccf19fadd7735634801f7ef502d1e`.
Raw file SHA-256 (55,988 bytes), a separate identity domain:
`2b49a52494ba200b4740064605dd9136454f3ebd3372a304d603c7afb7ada3bf`.

Focused retention r9 passed 57 tests with two ignored; full debugger r4 passed
263; affected debugger regression r5 passed 353 with five ignored. These
overlapping suites are not added as unique tests. Source-format r9 and
source-policy r6 passed. Rust/Cargo/crate-README census for these gates:
3,800 files, 73,897,817 bytes, SHA-256
`98262b4b8633bcadfe05eb04877f7803d45b067d287a179c952aad192adf58a7`.

## Public CPU console

The [console](../../scripts/debug-console/README.md) adds bounded state,
forward/reverse/continue, source/stack/memory and exact-site break/watch
commands around one selected `fe2o3-debug sim` process. It introduces no wire
version, Resource V1 interface, compiler mutation, server or GPU route.

All 37 pure unit controls passed after review fixes for aliased terminal
descriptor restoration, exact one-item mutation acknowledgements, movement
direction, and terminate cursor preservation. Seven separate harness controls
also passed; synthetic controls are not actual debugger qualification.

Actual console r1 passed on the fresh lifecycle r2 source Bundle V5, wave32,
using the existing reductionProfile request. This is not the private lifecycle
test's request and does not exercise its private API. Four actual sessions
entered 27 commands and received 32 responses, including discovery and EOF
termination:

- Commands/quit: real source resolution, one stack frame/two captured values,
  forward/reverse/repeat, memory, breakpoint and watchpoint add/list/remove.
- EOF: actual protocol termination, exit 0.
- SIGINT: exit 130 and the expected interrupt diagnostic.
- Shared-FD PTY quit: the parent's aliased terminal remained blocking after
  exit, matching its original setting. This is a scripted PTY, not a human
  interactive or PTY-EOF qualification.

Each observed debugger process exited and was reaped. Reverse/repeat moved
event/revision (1,1) → (0,2) → (1,3), preserving repeated values and bytes.
Source attribution was compiler_bundle_bound at bytes [16083,16137); this
does not authenticate source ownership. Typed snapshot not_captured remained
visible at entry, reverse-to-entry and the watchpoint stop.

An actual exact write watchpoint stopped at event 15132. The next checkpoint
15133 exposed the first initialized little-endian word 128 (`80000000`),
independently expected from 64×2. This is not full-output/canary qualification.
Breakpoint registration/list/removal passed, but no breakpoint stop was
tested: the listed hit_count 1 counts through the already-current cursor.
No post-hit watchpoint counter was queried.

Worker command time was 3,305 ms; outer gate 5,854 ms. These are observations,
not approved latency budgets. Inputs, source ancestry, selected executables
and 13 retained session artifacts were checked; this is not complete build/
runtime-closure attestation. Process/fd cleanup on successful sessions was
observed. Failure cleanup cannot guarantee escaped-descendant closure; the
task harness's aggregate artifact check occurs after each bounded file read.

## Retained receipts and source identity

Both actual gates measured this unchanged full compiler snapshot at parent
HEAD `123363584d329bb4d6aa2d528e0515d5eea04361` plus the implementation:
5,982 files, 91,658,600 bytes, SHA-256
`b6462e977e63d9c7e5b9b827897cddca5ad7eb78a7d07978a07fc668929dce66`.
This documentation and later commits are not relabeled as that tested census.

Under `/home/harmenon/fe2o3-authoring-280-282.FEW3gj/`:

- `rebuildable-cache-phase10.fpFy5o/secondary/phase11-lifecycle-source-r2/receipt.json`:
  45,052 bytes, SHA-256
  `a878a4639338b8d066af4d4f7b6c093efdef5ac414b63718d03c63b0cc94f94e`.
- `rebuildable-cache-phase10.fpFy5o/secondary/phase11-console-qualification-r1/receipt.json`:
  64,503 bytes, SHA-256
  `0b1e2e69390f8670c5145eebf791c9e0c0c7676443bad0fd6829ec9da9998958`.
- Pure console gate: `logs/phase11-debug-console-r2.json` (37 passed).

Guards retained the 20 GiB combined cache/output cap, 40 GiB disk reserve and
64 GiB available RAM; source builds used two jobs. These checks are not OS
quotas. No allocation reuse, physical-register capture, GPU execution, public
origin/lifecycle query, source authentication or production-resume completion
is claimed. Compiler main publication remains separately subject to the
unchanged DCO check and the inherited unsigned main-commit disposition.
The paired-sink method is a normal public simulator addition, not test-only;
its #216 review request remains pending and no owner acceptance is inferred.
