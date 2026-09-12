# Live Coherent Insertion Custody V1

## Status

R110 / N3-L2 is locally accepted above R109
`a9e1c73bb9b752610e58be9cd96861962e140787`, with
[retained evidence](evidence/local-r110-live-coherent-insertion-2026-09-12/README.md).
All seventeen source gates and ten auxiliary checks pass. GNU/musl each pass
2,638 tests with five ignored across 48 harnesses. Frozen/restored coherent
insertion, device insertion, coherent initializer, borrowed and model-loan
suites pass 19/19, 14/14, 10/10, 8/8 and 4/4. Ten compiled negatives reject at
their exact behavioral assertions; all 5,675 source identities are restored.
The final collector and its closed transcript pass. Native execution,
authenticated formal correspondence and performance are not qualified here.

See [the swarm assignments](runtime-swarm-next-packets.md) for the remaining
Native, Admission and Resources packets. This packet does not close A1/A2,
issue #182, or HIP/HSA parity.

## Ownership Boundary

The initialized coherent-memory path shares the bounded insertion settlement in
`crates/fe2o3-kfd/src/queue_live/data_insertion.rs` with initialized device memory.
The type-specific root determines preparation, completed identity, extraction,
and failure retention. The common sequence owns validation, reservation, loan
entry, retake, ledger commit, and terminal classification.

`CoherentInitializationCustodyV1` contains a one-shot preparation flag and the
actual optional initialized host-visible owner. It neither copies nor owns the
input source and allocates no separate storage. Owned public entrypoints retain
their input box through the synchronous borrowed preparation. Borrowed
entrypoints do not extend the source lifetime beyond that call.

Preparation invokes the existing R106 coherent initializer. There is no second
allocation/copy/map implementation. A successful mapped result moves into the
root before the memory-model loan can be returned. The root stays outside the
loan and remains populated until the ordered identity ledger and count commit.

## Ordering

1. Validate terminal/detached state and the identity ledger, then
   completion-owner readiness and releasability using the existing session checks.
2. Reject the seventeenth entry before choosing an insertion index or reserving
   metadata storage. An explicit index overrides any remembered hole.
   Replacement requires a previously remembered hole; it cannot append by
   default.
3. Reserve the full bounded identity-vector capacity before loan entry and any
   allocation/copy/map effect.
4. Enter the original session's memory-model loan, then invoke the existing
   coherent initializer. Empty-source validation remains inside this boundary.
5. Retake the original model. On an ordinary operation error, a retake error has
   existing precedence. An operation panic retains its first payload even if
   retake also fails or panics.
6. Borrow the completed identity, insert it at the selected ordinal, clear the
   remembered hole, and commit the count. Only then extract the data owner.

No kernel publication, device completion, command submission, or data adoption
occurs in this operation.

## Failure Custody

Before loan entry, ordinary validation/reservation rejection does not newly
poison or transport the parent. A panic or an entered failure terminalizes
the parent. Ordinary entered errors do not independently poison the global
dispatch gate; panic classification retains the existing global behavior.

Any admitted lower prefix remains in R106's existing pending-allocation or
typed-transition custody; pre-effect rejection may retain no native owner.
An empty outer coherent root must not modify,
replace, reconstruct, or dispose that custody.

If initialization succeeded but retake or commit entry fails, the outer root
moves the actual mapped token to the original engine's terminal transition.
The terminal stage is `LiveInsertion`, with output-only custody and default
native progress: this outer retention step performs no native operation.
It does not revalidate currentness, retake the model, or acquire a second owner.
Defensive occupied-slot handling preserves the earlier terminal owner rather
than replacing it. The engine remains quarantined.

Direct session wrappers retain the terminal parent before exposing an error or
resuming a panic. Selected-lane facades only accumulate sticky transport intent;
the facade's existing envelope restores the selected lane before transporting
the complete original parent. A swallowed error or later clean rejection cannot
clear earlier transport intent.

## Test Boundaries

Constructed-engine tests use the existing primary/auxiliary construction,
genuine model loans, scripted native backend, resource accounts, and retained
data owners. They are not live Linux/KFD execution. A later-slot case relocates
one real auxiliary behind a vacancy; it does not construct a second auxiliary
beyond the current two-compute-lane limit.

The accepted matrix covers successful ordered insertion, source identity,
retake failures, commit-entry panic, validation/capacity ordering, real released
holes, allocation/currentness/copy/map prefixes, projection failures, and
terminal retry. Shared-memory root tests cover one-shot preparation, one-time
extraction, empty retention, and preservation of an earlier real lower failure.
Concrete facade tests separately exercise actual public entrypoints with the
missing-engine boundary and parent restoration/transport.

Test auditors compare exact native records, pending/terminal metadata, retained
bytes, account charges, source contents, and ordered ledger identities. The
model oracle applies independent lifecycle transitions to the original model:
checkpoint, reserve, allocate, begin-map, and observe-map. It selects the active
foundation from actual ownership phase, including a still-open failed loan.
Allocation-stage projection failures preserve the committed released-entry
checkpoint; later failures preserve the allocation-only model unless mapping
also committed.

The nineteen new functions comprise eighteen dynamic tests and one source-routing
guard. Eight negatives target the new root/shared insertion behavior; two target
the reused model-loan error/panic precedence. Dynamic commit-entry custody and
source-guarded complete ledger ordering are distinct evidence. Stable vector
capacity is not global allocation instrumentation. The constructed composition
uses configured accounts; lower initializer/root tests retain both account modes.

All fifteen preliminary attempts, including six rejected runs, remain retained.
The full campaign's outer deadline increased from 1,800 to 7,200 seconds to fit
the larger serialized matrices; individual gates retain their 1,800-second
deadline and four test threads. No unplanned source, runner or test-limit
changes were made. Evidence-clock contract tests and exact helper
pins bind the mutation/restoration sequence without rewriting timestamps.

These checks validate the named CPU and source-wiring boundaries only.
Authenticated Rust/model correspondence, configured native campaigns,
multi-device behavior and matched HIP/HSA performance remain separate work.
