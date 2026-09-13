# Pristine Control Cleanup Custody V1

Status: R114 locally accepted for the bounded CPU/test scope below, with
[retained evidence](evidence/local-r114-pristine-control-cleanup-2026-09-13/README.md).
The prior runtime checkpoint is R113 `d85d6d7dc4f3b065dce9dc6505eac065c48cb0fa`;
the qualification/publication parent is planning commit
`fb5e19002e5a451aaf27f4bc16fc78c00b9163b3`.
See the [next swarm dispatch](runtime-swarm-dispatch-r114.md) and
[remaining packets](runtime-swarm-next-packets.md).

## Boundary

N4-R1 covers code and kernarg cleanup during pristine ordinary-dispatch abort.
It does not close ordinary/returning control release, data cleanup, all live
detach/destruction routes, generated adoption, native qualification or proofs.
Existing public consuming memory APIs retain their signatures; their other
failure-custody gaps remain assigned to the subsequent N4 packets.

The implementation uses the existing native memory engine, original queue
foundation and projections. There is no second memory policy, queue engine,
authority allocator or per-control heap allocation. CPU test fixtures invoke
the shared production cleanup adapter over scripted native leaves. Injected
live settlement is not successful concrete Linux/KFD composition.

## Ownership

`PristineDispatchAbortV1` retains every data input, storage identity, vector and
continuation throughout cleanup. It installs each removed actual control token
in `active_control` before invoking the borrowed cleanup callback. Kernarg is
first, then code in reverse order. The control is removed only after confirmed
Complete; `Ok` without Complete rejects without discarding it.

`ControlCleanupCustodyV1` is one-shot and owns exactly one of:

- the original mapped kernarg/code token;
- the corresponding unmapped token after successful lower unmap and closing
  currentness;
- a nonextractable native-disposal receipt after confirmed native disposal.

Started cleanup cannot be retried. No method extracts or reconstructs usable
authority after it starts. Failure preserves the active control, untouched
controls and all data/continuation custody. A confirmed disposal followed by a
currentness or model-commit failure retains a receipt, not releasable authority.

## Ordering

The cleanup adapter preserves two distinct revision preflights, each for one
transition. Unmap preflight/evidence precede native GPU unmap, projection and
model commit. A separate release preflight/evidence and release projection
precede native disposal and release commit. One available revision therefore
allows unmap to commit before release rejects; requiring two up front would
change existing behavior.

Ordinary native release preserves CPU-unmap, native free, then VA-release order.
USERPTR's existing free-before-CPU-unmap order remains in the shared borrowed
lower core. Record attempted calls before entry, and returned outcomes before
later validation. Keep malformed GPU-unmap progress precedence and do not retag
after a failed closing currentness check.

Mark confirmed native disposal before final currentness/accounting. On a later
error or panic, convert the original typed token to a nonextractable receipt.
Quarantine the entered cleanup failure and preserve the original panic payload.
Untouched owners cannot acquire a refund or reusable state from that failure.

## Live Transport

The existing `terminal_abort` retains the abort owner outside the model loan.
Settlement records whether the complete parent needs terminal retention before
returning or resuming a panic. The direct public API calls a private finishing
helper that performs retention before exposing the result. Its production sink
is the existing permanent-retention path; tests observe the same helper through
an injected sink.

The selected-lane facade ORs the settled transport bit into the existing pending
bit. It does not extract the parent while an auxiliary lane is swapped in.
The common lane wrapper first restores every lane, then retains the complete
parent and leaves the caller an inert shell. A poisoned retry cannot clear an
earlier pending transfer. Healthy opening/preflight rejection does not transfer.

When cleanup and retake both panic, retain the secondary envelope without
running its payload destructor, then resume the original cleanup panic. This
matches the existing model-loan policy and avoids a second destructor panic
replacing or aborting delivery of the primary panic.

## Validation Contract

Positive coverage includes all first/middle/last control positions, configured
and unconfigured lower budgets, native errors/panics and malformed prefixes,
projection/commit failures, all 18 cleanup currentness boundaries and revision
exhaustion. Compare exact original-model state, typed active custody, native
records, progress, retained VA, backing usage and one-shot retry snapshots.

Live tests capture the abort root before cleanup. Compare its exact control
order, completed native prefix, untouched shared/device records, data metadata,
storage and continuation before and after parent transfer. The touched active
record's full partial-native-state oracle belongs to the lower matrix; the live
record auditor supplements it, rather than replacing it. Cover primary, first
auxiliary and a later occupied slot behind a vacancy, not a second constructed
auxiliary beyond the admitted lane limit.

Direct finishing tests cover returned errors, original/crossed panics, successful
detachment, preflight/opening rejection and previously terminal retries. Source
routing guards supplement these dynamic tests but do not qualify native success.

GNU and musl each pass 2,712 tests with five ignored across 48 libtest harnesses
and the unchanged harnessless benchmark. All 17 source gates, ten auxiliary
checks, frozen/restored 37/7/9 focused suites and 15 compiled negatives pass.
Nine runner, 30 freeze and 80 qualification-contract tests pass. Both independent
archive reviews and the closed collector verify 254 raw artifacts and exact
restoration of all 5,683 source identities.

The two full runs are separately recorded pre-freeze prerequisites, checked by
matching source endpoints rather than relabeled as later campaign executions.
Retained failures and R113-parent preliminary records keep their original
identities. The archive notes the unwrapped formatter attempt without saved
inventories; it is not reconstructed as evidence. Formal refinement, native GPU
execution, total retained-memory closure and performance remain independent gates.
