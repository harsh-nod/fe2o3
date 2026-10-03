# Whole Conditional Fill Program

This checkpoint checks the exact source, neutral KIR and replayed target KIR of
the first ordinary application profile, `fill_write_only`. It does not complete
application admission, prove ISA interpretation, grant GPU authority or establish
HIP/HSA parity.

Parent commit: `2eb5a1e0e5cb6c3a19ca66938e3947c268f7d5c7`.

## Implementation

`CheckedConditionalFillProgramV1` borrows the exact conditional proof and target
lineage owners. Their neutral bytes and outer proof-receipt digest/length must
match. The checker requires the actual V9 singleton u32 WriteOnly output profile,
dynamic one-dimensional launch and workgroup/subgroup size 64. It recognizes both
the entire neutral module and entire replayed target module independently;
optimizer replay alone is not treated as semantic preservation.

Every accepted KIR value receives an exact denotation relative to the output and
global X index. The only store writes low32(index), under index < length, through
the output base plus the selected safe index. A different safe-but-incomplete
predicate is not accepted. SSA types, definitions, order and result arity are
checked. Exactly one store and a complete acyclic unconditional path to Return
are required. Additional effects, helpers, unreachable blocks and block-argument
plumbing reject. The bounded maps support sparse SSA IDs without allocating by
the largest ID; each module admits at most 64 blocks and 64 operations.

Source recognition uses the existing checked kernel-body selection, then checks
the exact parameter correspondence, Unit-returning body and three intrinsic calls:
ThreadIndex1d, ThreadIndexGet, and the non-disjoint Thread write for Index1d/u32.
Borrow origin, integer truncation, receiver, witness and stored value are tracked
through plain local assignments. Every source statement and terminator span is
checked against the expected KIR operation denotations, and all KIR operations
must be consumed exactly once. Unsupported branches, calls, projections and
effects reject. The selected body is bounded to 64 blocks and 128 locals.

Post-borrow-check MIR can spell the terminal non-Copy ThreadIndex argument as
Copy. Only the write-index consumer accepts that spelling, and it always consumes
the local. Generic owner copying still rejects. Shared witness borrows retain the
referent generation; moves, storage death/revival and reassignment cannot revive
an older borrow. A bare Unit Return is valid without a materialized return value;
non-Unit source returns remain outside this profile.

The new owner is move-only, does not outlive its compiler owners, and exposes no
launch transition. Existing unconditional import and protected Worker constructors
are unchanged. No proof/theorem body was changed by this checkpoint. The new
recognizers have implementation and regression evidence, not a newly claimed
whole-adapter formal proof.

## Genuine Fixture

`crates/fe2o3-verifier/src/conditional_fill_program_v1/fill.handoff` is a 32616-byte
canonical V3 handoff captured from genuine protected Rust extraction, not a
synthetic program. Its SHA256 is
`778098953929152bc6ffe7a1251052a04db63fa12c9a449694bb4078747d2b1a`.
The test-only opt-in capture uses `create_new`, so it never overwrites a prior
capture. This fixture retains embedded-key signature evidence; neither the key
nor fixture is elevated to trusted compiler provenance or launch authority.

Default CPU tests decode the exact handoff, import its conditional proof, replay
its target lineage and check both whole programs. Private source tests mutate
copies of its selected body and correspondence denotations. These mutants test
the recognizer, not forged successful protected compiler executions. The fresh
ignored compiler pair separately exercises genuine production extraction and
the existing changed-reference rejection.

## Qualification

The final affected CPU suites pass 428 tests:

| Suite | Passed | Default ignores |
| --- | ---: | ---: |
| Verifier library | 156 | 7 |
| Verifier ownership/authority doctests | 36 | 0 |
| Compiler proof binding V4 | 25 | 0 |
| Host library | 211 | 1 |

The ten new verifier tests include genuine handoff replay, 30 basic KIR mutants,
six intrinsic/cast/alignment substitutions, exact operation/block bounds,
unsupported block parameters/arguments, 12 source call/control/value/borrow
mutations, missing/extra span operations, terminal Copy/Move consumption, stale
borrow generations, non-Unit returns and wrong exports.

Strict verifier/compiler library Clippy passes with `--no-deps -- -D warnings`.
Targeted rustfmt and whitespace checks pass. The archived `run-checks.sh`
records the locked/offline commands, pinned nightly and checked test profile.

The fresh genuine protected compiler pair passes 2/2 tests in 89.54 seconds.
The positive performs extraction, conditional V9 import, target replay, actual
typed packed-coverage checks and the new complete source/KIR recognition. The
changed-reference negative still rejects and emits no accepted handoff.

The pinned runtime manifest remains
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
Root-owned provisioning uses private mount, PID and network namespaces and drops
to UID/GID 1000 without capabilities for the controller. `/opt` and `/tmp` are
namespace-local; prerequisites are SHA256 checked. No GPU ran in this
qualification. The MI300X inspection was read-only; no shared remote files,
queues or allocations were created or deleted.

The genuine fixture was captured during successful compiler extraction before
the initial source recognizer rejected the terminal witness's Copy spelling.
After the narrow consuming-operand fix, both the captured fixture and the fresh
compiler positive pass. Development compiler-diagnostic failures, source Copy
rejections and the Clippy correction are retained separately from accepted logs.
The code-only patch relative to the parent has SHA256
`e73b504002e0855de29c0a42e0ffb0563b212de5637deab01f17ff6614cd23d8`.
The [qualification archive](qualification.tar.xz) has SHA256
`64199480d98228cbd60cd16691ce705e89f3847fb544e3e2c8e196a32c1428e0`.
Its listing and extracted code-patch digest were checked before scratch cleanup.

## Remaining Critical Path

Join this whole-program profile to the authenticated analyzer's exact HSACO and
existing complete wave executor. Prove dispatch-wide full-wave initialization,
unique coverage, output bytes and termination using the shared executable model.
Retain the universal machine contract in a distinct conditional Worker profile,
then discharge actual packed coverage at the consuming prepared-dispatch/device/
patched-storage transition. Memory backing and completion visibility remain
explicit obligations; no detached digest or inspection receipt may substitute.

Then qualify genuine admitted fill -> tracked upload -> native XGMI -> complete
guarded readback for N=64/65/4097 in both selected-device directions. Existing
native multi-GPU mechanics and the synthetic completed-value staging witness do
not substitute for that application path.
