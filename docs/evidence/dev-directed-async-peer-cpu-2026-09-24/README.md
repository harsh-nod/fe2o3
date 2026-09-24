# Directed Async Peer Copies: CPU Checkpoint

Development evidence for the
[directed async adapter](../../runtime-directed-async-peer-v1.md), above signed
Context checkpoint `cb70d00a00d3436313f878e57c348fc569313d8c`.
This is not native, formal-refinement, performance or milestone acceptance.

## Implementation

The two public directed-copy methods reuse the existing factory, registry,
control, snapshot, reply and operation lifecycle. A private compile-time policy
selects exact Context directed progress and no automatic operation flush
identity. Ordinary operations retain their existing poll/flush policy. The
adapter adds no backend Send requirement or native owner to the driver.

The shared driver now preserves a rejected backend observation that has sealed
Context, instead of retrying it and replacing the diagnostic with EngineStopped.
Registry terminal handling still retains the driver and Context custody.

One directed progress advance performs at most one backend action, not one
action for an entire stream or scheduler tick. Explicit stream registrations,
ordinary operations, event observers, graph work and drain remain independent.
No early-submission/event API or general queued graph composition is added.

## Qualification

Pinned nightly `2026-04-03`, all features, test optimization level 1, debug
assertions enabled, incremental compilation disabled and two build jobs:

- GNU runtime library: 1,325 passed, zero failed, twenty hardware-only ignores.
- Musl runtime library: 1,325 passed, zero failed, twenty hardware-only ignores.
- Runtime doctests: 46 passed, in groups of 4 and 42.
- Formatting and all-target warnings-denied Clippy pass.
- Seven serial command receipts report status zero and owned process-group absence.
- All 3,818 input hashes are unchanged across the run and match the final source;
  an independent Git-based discovery matches the captured input roster.
- Runner SHA256: `bb7d2d953ef0a2b0dd3ce478405822e0bc27f7a20ff1ce444d2a5325010adefc`.

The archive retains 23 exact raw records: seven command triples and two input
brackets. Cargo's final blank lines and interleaved libtest trailing spaces are
preserved rather than normalized. The exact runner is included.

Twenty-one new test functions cover the real Context/driver/registry paths:
consumer-only reconciliation with released events, a shared-producer diamond,
exact returned consumer identity, cancellation, timeout, observer Drop, Stop,
failure/quiescence, terminal and panic custody, retained-success contradictions,
and conservative discarded producer results. Tests exercise snapshot/reply/
channel/registry limits, owner-side alias/stale-event rejection, reentrancy and
refunds before and after backend entry. Both methods join the existing duplicate
and oversized-dependency preflight test.

Scheduling tests cover ordinary same/different-stream operations in both
insertion orders, independent standalone/paired progress registration in both
command orders, multiple directed drivers and local retirement after ordinary
stream synchronization. Public engine coverage includes caller-driven ownership
with a journal, non-Send background ownership without a journal, and the existing
Send-capable Context-returning engine with a journal. These are scripted CPU
backends, not GPU execution or physical data-copy oracles.

Two independent read-only source reviews found no remaining blocker. The review
identified the sealed-rejection diagnostic issue and required explicit scheduling
and ownership scope. Review does not constitute formal proof.

## Retained History And Cleanup

Private scratch root:
`/home/harsh/.codex-tmp/fe2o3-directed-async-20260924-1eUnOXRz`.

- `focused-1` failed compilation on a test's struct-style use of a tuple enum
  variant (`E0559`, command status 101).
- `focused-2` failed compilation on two test registration futures passed to a
  command-future helper (`E0308`, command status 101).
- The test helpers and oracles were corrected, including owner reentrancy
  identity and a valid exhausted reply budget.
- `focused-3` passed 37 snapshot/directed tests on the preliminary source.
- One final retained-producer Pending/Failed/Quiescent regression was added and
  an unnecessary non-Drop handle disposal removed.
- `cpu-1` is the final complete successful campaign archived here.

Failed attempts and their input brackets remain in the private scratch. The
runner imports the owned-process helper, not the historical owner proof theorem.
These are development source checks, not hermetic compiler/toolchain attestations.

After qualification, `cargo clean` reported removing 1,341 files and 662.2 MiB
from only this task's private target. Target absence was separately checked; raw
logs and the runner remain. No SSH/GPU operation, shared cache cleanup or matched
HIP/HSA benchmark ran in this packet.

## Remaining Gates

The contract records the next native four-copy dependency-diamond witness and
the existing hardened runner to reuse. Native chains, complete payload/canary
checks, faults, cleanup, executable refinement, aggregate memory and matched
performance remain open. Historical model proofs keep their original source
identities and do not automatically prove the changed driver integration.

Native R125, Admission R118B C1/C2/C3 and Resources R116/V3 remain the broader
accepted checkpoints. This packet does not close A1/A2, #182 or HIP/HSA parity.
