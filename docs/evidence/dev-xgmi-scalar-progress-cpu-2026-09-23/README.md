# Scalar XGMI Dependency Progress: CPU Checkpoint

Development CPU evidence for the [bounded scalar progress method](../../runtime-xgmi-scalar-progress-v1.md).
This is not native GPU qualification, a new formal refinement, a matched
performance comparison, or A1/A2 acceptance.

## Implementation

One backend call selects at most one FIFO publication attempt, one published
ticket observation or one failed-dependent settlement. Iterative traversal
retains consumers while progressing dependencies; only the requested root's
own retained result can report completion. Successful-prefix, dependency
identity/retention/depth, bounded native-window and publication-prefix checks
precede native effects. Ordered dependencies retain their separate progress
path. Scalar poll/wait and nonwaiting flush contracts are unchanged.

FIFO membership is a flag on the existing active submission, not a separately
allocated set. Admission, wakeup, extraction, restoration, aggregate/ordered
execution, cancellation and settlement maintain it. Waiting settlement avoids
an unrelated FIFO scan. Duplicate wake insertion is idempotent. The dormant
unticketed recursive helper now only restores its owner and returns Pending.

## Qualification

Pinned nightly `2026-04-03`, all features, test optimization level 1, debug
assertions enabled, incremental compilation disabled and two build jobs:

- GNU runtime library: 1,251 passed, zero failed, 20 hardware-only ignores.
- Musl runtime library: 1,251 passed, zero failed, 20 hardware-only ignores.
- Runtime doctests: 46 passed in the two reported groups (4 and 42).
- Formatting and all-target warnings-denied Clippy pass.
- All seven command receipts report status zero and owned process-group absence.
- All 3,805 source/configuration/fixture hashes and the runner hash match before
  and after execution. Current implementation files also match those hashes.

There are 27 added CPU test functions: 26 shared-selector/driver/index tests and
one aggregate-admission flag test. The suite exercises both directions,
multi-window FIFO tails, pending blockers, consumer-only chains, diamonds,
failure precedence/propagation, depth and dependency limits, malformed retained
state, ordered isolation, scripted callback errors/unwinds, saturated FIFO
restoration, partial restoration and cancellation. Counts above are test
functions, not the larger number of loop cases.

The raw command outputs, exact arguments, tool versions and input brackets are
in `records`. `runner.py` preserves the original absolute workspace paths; its
argument chooses a fresh output directory. It is a development runner, not a
portable/hermetic source or toolchain attestation. The archive contains 23 raw
records: seven command triples and two input brackets.

Independent read-only review found no healthy-path defect in the final flag
transitions. The scripted fixtures do not execute production native mapping
rollback, aggregate native execution or ordered Pair/Ticket restoration. Those
paths received source review, not new hardware fault qualification. A scripted
adapter panic before native custody is consumed is not evidence of resumable
native unwind; the existing custody-consuming fail-stop policy remains.

## Retained Development History

The initial `cpu-1` attempt remains at
`/home/harsh/.codex-tmp/fe2o3-xgmi-progress-20260923-LBY7nO5F/cpu-1`.
It was deliberately interrupted during GNU compilation after a standalone Rust
probe and source review identified a healthy restoration-capacity defect in
the discarded HashSet design. Its source bracket is unchanged and its cleanup
receipt reports process-group absence. It is not a passing qualification run.
The fresh `cpu-2` campaign supplies all results above; no earlier result is
substituted into it. The private build target and probe binary were removed
after qualification; raw records, runner and probe source remain local.

## Remaining Gates

Context pending-producer reads remain rejected. Exact producer/route admission,
Context-owned dependency retention, producer-first outcome reconciliation,
consumer-only async integration and native data/canary/cleanup campaigns remain
next. Worker transport and executable/native proof refinement are separate.

No SSH, GPU workload or HIP/HSA benchmark ran for this packet. Native R125,
Admission R118B and Resources R116/V3 remain the broader accepted checkpoints;
#182, A1/A2 and HIP/HSA parity remain open.
