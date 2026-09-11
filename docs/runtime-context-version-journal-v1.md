# Context Version Journal V1

## Status And Scope

VER-1A.1 contract/inventory against signed R97
`1b53ef417d0f4184e2b4e6024b37271b5f719832`. No Context-wide journal, new
executable model, formal theorem or cross-run lease implementation is claimed.
Existing R65 graph versions remain graph-local history. This journal records
mutation lineage, not initialized contents, native currentness, disposal
authority or successful kernel semantics.

R98 accepts this [contract/inventory only](evidence/local-r98-completion-contract-2026-09-11/README.md).
The next Resources deliverable is VER-1A.2's production-consumed executable
model and property proofs, followed by the bounded Context journal.

VER-1A first provides an explicitly opt-in bounded profile. It must not silently
restrict existing ordinary-runtime behavior. Complete VER-1B hook coverage,
bounded ordered writers and recovery are required before enabling persistent
reuse across the ordinary surface. These are requirements, not optional
optimizations to be dropped when the initial journal passes tests.

## Identity And Capacity

Use complete existing `RuntimeAllocationIdV1` and `RuntimeDeviceIdV1`, including
their private Context generation. Never key authority by `.get()`, a backend
handle, an address or a graph node alone. `context.rs` defines these brands;
`RuntimeContextV1::open` allocates the nonwrapping Context generation.

A synchronous writer uses a privately tagged
`{ context_generation, local: Context::next_id()? }`. An asynchronous writer
uses the exact `RuntimeSubmissionIdV1` already minted by its submit path. Do not
introduce a second identity allocator or replace the original submission ID
when preparing, translating, retrying or observing work. Consumed IDs never
roll back.

Configure separate finite allocation-entry and writer-record capacities before
the first tracked allocation/mutation. Preallocate entries, retained membership
and scratch before admitting effects. Existing Context allocation/submission
maxima are each 1,048,576; the shared credit engine's 65,536-record maximum is a
different quantity, not an implicit journal limit. Allocation admission must
reject before creating an allocation it cannot track.

An initial one-pending-writer-per-allocation profile permits O(A + W) retained
state, with membership stored once per allocation and writer headers retaining
exact membership counts. It is staged opt-in only: ordinary same-stream and
dependency-ordered overlapping writers require bounded ordered chains or an
equivalent nonrestrictive design. That design must specify total membership
capacity, predecessor identity and settlement ordering. It cannot treat an
earlier NoEffect receipt as permission to erase a later writer.

Journal metadata bounds do not establish aggregate memory charging. Bootstrap,
arenas and terminal headroom remain MEM-DOM/MEM-5 obligations. Do not duplicate
allocation credit records or refund resource credits through a lineage change.

## State And Transitions

Each allocation retains a nonwrapping attempt epoch independently of content
lineage. Initial epoch/lineage zero means no journaled writer, not initialized,
readable or native-materialized storage. Partial writes advance whole-allocation
lineage without proving that untouched bytes are initialized.

| Transition | Preconditions and result |
| --- | --- |
| Register | Reserve complete metadata before effects. Bind exact allocation/device/extent. Successful allocation or generated-shell installation commits the entry; ambiguous failure retains its provisional identity rather than creating untracked custody. |
| Begin | Validate the entire canonical destination roster, exact identities, writer uniqueness, capacity and every checked epoch increment. Atomically burn one epoch per member and install `Pending { writer, prior_lineage, attempt_epoch }`. No member remains reusable. |
| Observe pending | Preserve the exact writer and complete roster. Timeouts, observer loss and rejected polling do not roll back admission. |
| Settle success | Require exact conclusive writer success and complete retained membership. Commit each member's admitted attempt epoch as its new lineage, then remove the pending writer record. |
| Settle NoEffect | Require a private, exact attempt-bound definite-no-write receipt. Preserve prior lineage but never roll back attempt epochs or Context IDs. Consume the receipt; it does not authorize replay, reissue or revival of an old lease. |
| Settle unknown | Failure without definite-no-write evidence, partial effects, ambiguous publication, panic or currentness loss cannot restore availability. Retain required membership/custody and deny reuse. |
| Retire | Remove entries only after exact confirmed disposal. Failed disposal retains them. Generated retirement validates and retires the complete shell roster, not a caller-selected subset. |

`NoEffect` is not inferred from an error's spelling.
`RuntimeBackendFailureV1::Rejected` is documented as preceding device-visible
mutation. It is usable only at the corresponding write/submit attempt boundary
and for an adapter whose contract covers every relevant write. A rejected
poll/query does not describe the effects of the already-issued operation.

`BackendCancellationV1::Cancelled` describes withdrawal before publication and
quiescence. A future private cancellation receipt may establish NoEffect only
after proving that this exact attempt incurred no relevant host/cooperative/native
write. Generic `RuntimeCompletionFailureV1::Cancelled`, quiescence, or a public
descriptive enum is not that receipt. Producer/profile correspondence remains
pending.

Unknown recovery must be separately specified, not silently equated with a
successful ordinary write or permanently omitted from parity. Exact exclusive
overwrite/reinitialization and ordered-chain recovery need explicit evidence
and acceptance. NoEffect in an ordered chain preserves the correct predecessor
lineage; it cannot erase later writers or promote an unknown predecessor.

## Private Interface

Proposed module: `crates/fe2o3-runtime/src/context/versions.rs`. Use fixed errors
and private identity-bearing types. Suggested operations are
`prepare_registration`, `commit_registration`, `begin_write`, `settle_success`,
`settle_no_effect`, `settle_unknown`, `retire` and `seal_context`. Model/export
names and proof revision numbers are assigned at implementation.

The Context owns complete pending rosters. A move-only writer ticket identifies
a retained record; it does not own the only copy of membership. Dropping it
cannot cancel the writer, free its metadata or restore availability. Settlement
operates on journal-owned membership, not an externally supplied subset.

All fallible validation, scratch preparation and checked arithmetic precede
mutation. Canonicalize aliases once by full logical allocation identity; distinct
views of one allocation produce one mutation member. Validate cardinality,
uniqueness, writer ownership and all members before committing any member. No
allocation, callback or fallible user code runs during commit. Ordinary preflight
failure leaves journal entries unchanged; consumed Context IDs remain consumed.
Internal invariant failure seals the Context and retains custody.

Install Pending before entering the backend, including operations that return
an error without producing a submission token. An unwind guard retains the
writer/roster and denies reuse on panic. For a call that never returns, the
preinstalled Pending state remains unavailable. Process abort is not successful
settlement. Settle the journal before callbacks or terminal-result publication.

## Mutation And Retirement Inventory

All paths below are under `crates/fe2o3-runtime/src/` at R97. The named functions
are the integration boundaries, not claims that hooks already exist.

| Boundary | Required hook |
| --- | --- |
| `context.rs::allocate`, `release_allocation` | Provisional registration before allocation effects; exact removal after successful disposal only. Preserve requested-byte credit ownership. |
| `context/generated_shells.rs::install_generated_shells_v1`, `retire_generated_shells_v1` | Atomic complete original roster, including unused/read-only buffers. Registration is not initialization or native adoption. |
| `context.rs::write_allocation` | Range validation, writer admission, backend write, exact success/NoEffect/unknown settlement. |
| `prepare_context_copy_v1`, `submit_prepared_copy_v1` | Preserve logical destination/device before translation. Begin with the original submission ID before backend copy effects. |
| `peer_copy` | Preserve destination identity before local variables become backend regions; join both devices to the existing exact submission. |
| `prepare_context_launch_v1`, `submit_prepared_launch_v1` | Retain logical mutation roster alongside translated bindings. Cover ordinary, snapshot, atomic and collective launch wrappers. Caller-declared access alone cannot prove the kernel's write set. |
| `context/graph.rs::prepare_graph_launch_v1`, `prepare_graph_copy_v1`, `submit_graph_action_v1` | Reuse ordinary prepare/submit hooks. Graph reservation or R65 history does not replace the persistent journal. |
| Future generated ISSUE; `context/generated_preparation.rs`, `async_engine/generated_operation` | Use existing logical shells, original submission identity and admitted effects. R73/R80 storage/reply debits remain unchanged. Preparation, reservation and adoption are not successful mutation settlement. |
| `context.rs::transition_submission_status`, `observe_submission_backend`, `completion_backend_result` | One exact settlement before callbacks. Preserve sticky terminal status; rejected polling does not settle NoEffect. |
| `poll`, `wait`, `poll_event`, `wait_event`, `synchronize_stream`; `context/drain.rs::poll_async_drain_v1` | Route observations through the same settlement contract. Events retain the originating submission identity. |
| `context.rs::cancel`, `drain`, `mark_stream_quiescent` | Distinguish exact cancellation receipts from generic cancellation/quiescence. Neither drain completion nor custody release implies successful contents. |
| `flush_with_graph_access_v1` | Deferred publication/progress refers to already-admitted writers. Backend failure cannot create a fresh writer or lose retained membership. |
| `backend_result`, `seal_backend_protocol`, `quarantine_after_async_command_panic_v1` | Terminal/currentness/protocol/unwind paths deny reuse across the affected Context. Malformed returned handles do not undo admission. |
| `destroy_stream`, `release_submission_ref`, `cleanup`, `shutdown` | Settle or retain pending writers before retiring submission/allocation records. No journal removal on rejected or ambiguous cleanup. |
| Reads, drain capture, queries, event/module management | No direct destination mutation. Their terminal errors, cleanup and retained-submission effects still participate in invalidation/retirement. |

`PreparedContextLaunchV1` currently retains backend bindings;
`PreparedContextCopyV1` retains backend regions. Reverse lookup from translated
handles is not a substitute for retaining the original logical roster. Backend
aliases or mutation routes outside Context must be covered, invalidated or
explicitly excluded before private leases are enabled.

## Acceptance And Proof Boundary

VER-1A.2 adds a production-consumed model; VER-1A.3 implements the bounded
journal; VER-1A.4/.5 integrates initial hooks and validates them. VER-1B closes
every mutation family plus ordered writers/recovery; only then may VER-2 issue
cross-run leases. First non-reusing generated ISSUE does not depend on leases,
but must retain its mutation identity/roster now.

Required negatives cover foreign Context/device/allocation/writer, duplicate or
omitted members, first/middle/last capacity or epoch failure, replay, dropped
tickets, late settlement mismatch, NoEffect epoch rollback, rejected polling
misclassification, cancellation without receipt, lineage-zero readiness, failed
retirement and panic before a returned submission. Tests must also preserve
existing same-stream/dependency-ordered writers when that profile is enabled.

R65 supplies per-segment guards and a useful whole-set preflight pattern, not
Context-wide ownership or NoEffect semantics. R67/R70 resource-credit proofs do
not prove journal transactions. New obligations separately establish complete
roster atomicity, nonwrapping identity/epoch evolution, exact writer settlement
and NoEffect lineage preservation. Proving pure transitions does not prove Rust
extraction, complete hook coverage, backend receipt issuance, native currentness
or hardware behavior. Property evidence remains separate from CPU regressions,
live KFD qualification and matched performance.
