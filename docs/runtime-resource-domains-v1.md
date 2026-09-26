# Shared Resource Domains V1

This is development of MEM-DOM-1, not complete bounded-memory qualification.
The [shared accounting engine](../crates/fe2o3-resource-accounting/src/domain.rs)
supports a fixed-arena, three-level hierarchy, and Context allocation
admission can attach to it. The subsequent
[rooted N1 adapter](runtime-native-root-admission-v1.md) adds checked-device
parents and root-required native host-backing construction. The explicit
[compound N1/N2 profile](runtime-compound-native-backing-v1.md) adds a fourth
level for separate class leaves below combined session limits. Whole-profile
physical identity/root reuse, remaining native adapters, complete bootstrap and
formal refinement remain open.

## Identity And Admission

`ResourceCreditAccountV1::new_root` owns one mutex and bounded node, record,
free-list and batch-validation arenas. `new_child` consumes a vacant domain
slot, not another allocation or independent ledger. A child has immutable
parentage, capacity and record limit. Unused child capacity is not reserved
against its parent: actual reservations contend for shared ancestor capacity.
`new_root` retains its three-level bound; `new_root_with_class_domains_v1`
explicitly selects a four-level bound. These are generic accounting domains,
not native-device capabilities. A fifth level is rejected without mutation.
Both profiles use the same fixed arenas and bootstrap size calculation.

An account handle carries its root identity and a generation-bearing leaf key.
`shares_ledger_with` requires both the exact root and leaf; `shares_root_with`
does not imply interchangeable leaf accounts. A compact token binds the root,
owner generation and global record slot; that exact record retains the leaf key
and immutable ancestor path. Domain and record generations never
wrap. Domain slots are reusable only after their handles, descendants and
records are gone. Quarantined records prevent reuse.

Scalar and whole-roster admission check every ancestor's complete resource
vector and record limit before one commit under the root mutex. Rejection for
capacity, records or generation exhaustion changes no usage, owner generation
or occupied/free roster. Validation scratch is temporary state, not a debit.
The R75 pure planner shares the production R67 arithmetic bodies and preserves
the prior R70 leaf-first error order. It scans charges once at the leaf, then
checks the accepted aggregate at each parent. Fixed four-entry facts and plans
require no heap allocation or charge-roster conversion; the work is
`O((members + depth) * 19)` instead of rescanning members for every ancestor.
The adapter still validates the actual path and global record-total consistency,
extracts facts under the mutex, validates selected free slots and commits the
staged usage/counts.
Every member remains independently retainable and disposable; release updates
its entire ancestor path. Child and ancestor readings are inclusive projections
of the same global records, not additional physical allocations to sum together.

Refund and final account Drop validate the complete selected leaf-to-root path
before mutation, including the immutable three/four-level profile. A bounded
retirement plan projects the pending record refund or handle decrement, checks
the removable node prefix, parent child-count decrements and both logical and
actual Vec push headroom, then commits without further fallible cleanup. The
root is never retired and free nodes are returned in leaf-to-root order. A live
handle, remaining record or sibling stops node removal. Invalid ancestry above
that stopping node is still rejected by the initial path validation.

The [retirement development evidence](evidence/dev-domain-retirement-2026-09-26/README.md)
records a fault-injected bug fixed at this boundary: previously a late reaping
error could follow a partial refund or node recycle. Detected preflight failure
now preserves the complete selected state before poisoning. This is not a scan
of unrelated arena slots or a new formal hierarchy/native correspondence proof.

Quarantine changes the original record phase without new arena storage. One
existing Arc anchor retains the root and its full bootstrap baseline after
outside handles disappear. Invariant/mutex poison seals the entire root;
ordinary capacity rejection does not. No callback or native operation is used
to admit, cancel, retire or quarantine a credit.

Metadata tables retain only their payload and credit, not a duplicate account
handle. `HostMetadataTableV1::account()` now returns an owned optional handle
recovered from the exact retained record. This is a source-API change from the
previous borrowed return: borrowed consumers use `.as_ref()`, and callers that
already wanted an owned handle remove `.cloned()`. Recovery preserves the leaf
before original table disposal and remains inert on a valid poisoned record.
It adds no heap allocation and keeps the existing 64-byte table-header limit.

## Bootstrap Boundary

`resource_domain_bootstrap_bytes_v1` derives checked Rust payload sizes from
the actual coordinator, domain/record slot types, full free-list capacities and
batch-validation bitmap. `new_root` checks that baseline against
`ControlResidentBytes` before allocating, verifies exact Rust `Vec` capacities,
and keeps the whole arena charged through vacancy, slot reuse and quarantine.
Child construction and credit transitions allocate no new ledger storage.

This is a Rust payload contract, not total heap residency. Arc control headers,
allocator rounding/headers, external account handles, returned batch boxes,
Context registries and arbitrary error/panic/callback payloads are excluded.
The batch output is allocated and boxed before ledger commitment, but is not
charged to this root. A future bounded batch-output adapter remains necessary.
Fallible Vec allocation failures dispose already-created local storage; global
allocator abort is not an in-process recovery guarantee.

The root is not precharged to a higher process account. A caller may create
unrelated roots, and legacy independent accounts remain available. Therefore
this bounds participating supplied charges, not every allocation in a process.
The bounded production profile must require a persistent root and cover its
complete actual bootstrap before a process/global claim is supportable.

## Context Integration

`RuntimeContextV1::configure_allocation_admission_in_domain_v1` creates a private
child branded with that Context's exact device ID. Attachment rejects existing
accounts or live allocations on that device; once installed it cannot be
replaced by another root or by the legacy local-budget method, even after clean
release. Other Context devices remain unconfigured unless explicitly attached.

The existing allocation and roster-preflight paths consume these same child
tokens. Two participating Contexts compete for ancestor requested-byte and
allocation-record limits before backend entry. Definite rejection and confirmed
disposal refund the exact member; uncertain allocation, terminal/panicking
disposal and Context destruction do not reset quarantined parent usage.

The Context-local device brand is not a stable physical-GPU identity. Canonical
root-issued physical-device parents, native session/VM association, root-required
constructors and accounting for work before attachment are still required.
The rooted N1 constructor joins ordinary coherent backing to its typed
root/device hierarchy; compound N1/N2 adds a combined session and class leaves.
Neither shares the Context request leaf nor composes that logical-request
profile into the same typed root. Other backing,
module-image and scaled-table accounts remain separate. A request charge is not
a native-residency measurement.

## Verification Gates

The [development evidence](evidence/dev-resource-domains-2026-09-26/README.md)
separates core tests, actual Context/MockBackend paths and broader regressions.
It does not prove the new hierarchy by reusing R67/R70 proofs. New obligations
include exact ancestry and node reuse, all-ancestor conservation, failure
atomicity, quarantine lifetime, bootstrap correspondence and lock/arena
refinement. Mutations must omit an ancestor or final coordinate, refund
quarantine, reuse a retained node, accept a foreign root or duplicate a debit.
Native cost extraction/disposal and matched HIP/HSA measurements remain separate.

The subsequent [planner evidence](evidence/dev-domain-planner-2026-09-26/README.md)
adds a production-shared Verus proof of the pure reservation planner and scalar
vector arithmetic. The exact bodies prove all active ancestor usage and record
counts, checked owner advancement, capacity bounds, zero unused plan entries
and first-error precedence. A pinned campaign verifies 19 obligations twice and
rejects 15 semantic body mutations; nine controller calibrations also pass.
The Rust adapter's source identity is bound, but its fact extraction, path/key
validity, lock, arena/token commit and retirement are not formally refined by
this proof. Differential model tests and actual adapter snapshots cover those
selected execution boundaries without promoting them to universal theorems.
The paired CPU microbenchmark measures only the pure planner, not account
admission, native execution or HIP/HSA performance. Full MEM-DOM qualification
and the other hierarchy obligations above remain open.

The [immutable arena evidence](evidence/dev-domain-arena-2026-09-26/README.md)
extends refinement to production's exact slice-based lookup, ancestry traversal
and planner-fact extraction. No converted shadow arena or runtime uniqueness
scan is introduced. Success and rejection are complete over arbitrary arenas;
successful paths have exact keys/generations, leaf-to-root parent edges, distinct
occupied slots, bounded depth and unchanged ROOT padding. Fact extraction
preserves every selected planning field and zero inactive entries. A proved
implication derives graph properties from the executable path's exact result
contract, without assuming them.

The authenticated campaign runs both the 19-obligation planner and 21-obligation
arena units for two bracket positives and each of 31 semantic mutations. All
66 unit runs classify correctly, and nine controller calibrations pass. Shared
declaration obligations overlap, so the counts are not additive properties.
Four new Rust test groups include 432,180 old/new traversal comparisons and
selected production rejection snapshots. The broader library run retains the
same four socket failures; no native or matched performance result is added.
Mutable ledger conservation, token/record correspondence, retirement, mutex and
whole admission/commit composition remain separate open proof obligations.
