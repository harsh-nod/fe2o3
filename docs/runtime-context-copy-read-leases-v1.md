# Context Copy-Source Read Leases V1

Development toward A1/A2 in #182. Accepted checkpoints remain Native R125
CPU/test, Admission R118B C1-C3 and Resources R116/V3. This does not complete
V6, A1/A2, formal runtime composition or HIP/HSA parity.

## Contract

The opt-in Context journal now retains the original logical source of built-in
same-device, graph and peer copies. A prepared copy preserves both the source ID
and exact allocation record; submission revalidates them before minting an ID or
entering the backend. Recycling a backend allocation handle cannot revive an old
prepared source. This identity recheck also applies without the optional journal.

Each journal-backed copy acquires one bounded source lease after destination
Begin and before backend entry. It binds the original submission ID, allocation
reference, device, full extent, requested byte range, attempt epoch and content
lineage. A fresh nonwrapping incarnation distinguishes repeated acquisitions,
including the same consumer and unchanged source in a recycled lease slot.
The submission marker and independent reader root retain the exact reference;
the [typed-input extension](runtime-context-kernel-read-leases-v1.md) generalizes
the original writer-attached copy reference to a separate batched input root.

Multiple readers may overlap. While any reader remains, host writes, writable
launch bindings, copy destinations and allocation retirement reject for that
whole allocation before backend entry. Source admission rejects Pending or
Unknown writers. Stream ordering and event dependencies do not override these
conservative exclusions. Partial ranges do not permit disjoint concurrent writes.

## Settlement

| Observation | Source lease | Destination writer |
| --- | --- | --- |
| Initial Rejected | Release | NoEffect |
| Initial Quiescent | Release | Unknown |
| Initial Terminal, panic, zero/duplicate handle | Retain and quarantine | Unknown and quarantine |
| Pending, rejected observation, TooLate | Retain | Retain |
| Exact Succeeded | Release before status/callback | Success |
| Exact Failed or quiescence without result | Release before status/callback | Unknown |
| Exact cancellation before publication | Release | NoEffect |
| Metadata release, observer drop, timeout alone | No release evidence | No settlement evidence |

The shared terminal transition covers poll, wait, event wait, drain, stream
synchronization and conclusive stream destruction. One consumer's quiescence
releases only its own lease. Repeated observations cannot release a subsequently
reused slot. Missing or substituted markers seal the Context without publishing
completion. Terminal quarantine includes source allocation credits, charged once
even when several consumers share the source. Normal writer settlement and
Unknown destination disposal require source custody to have ended first.

Generated whole-shell retirement checks readers both before native DATA/submission
retirement and at final shell disposal. Issued Stop can defer on a live reader;
completion and adoption cleanup retain their conservative terminal error boundary.
Generated shell IDs are currently private, so these checks defend internal
composition, not a newly supported public generated-source sharing workflow.

## Bounds And Evidence

`ContextReadLeasedJournalV1` owns the existing writer journal and exposes only
immutable inspection of it. Every mutation or retirement path capable of changing
a leased allocation is guarded. Constructor-reserved lease slots, free slots and
per-allocation reader counts are preserved without acquisition/release allocation.
Canonical batches cost O(k) in touched requests; each built-in copy uses k=1.
Runtime reader capacity equals writer capacity by constructor policy, but its
occupancy is independent; read-only typed launches need no writer slot.
This is bounded metadata, not aggregate native residency.
Ordinary prepared graph actions store their larger identity snapshots behind
one box, allocated during preparation, so generated and join nodes need not
carry that payload inline. Non-graph copies need no additional box.

The tests use opt-in deferred copies: bytes move at modeled success/quiescence,
not at initial submission. Unpublished cancellation discards the queued copy
without touching destination bytes. These are CPU backend-contract tests, not
native GPU or protected completion evidence. Qualification is recorded in the
[copy-source archive](evidence/dev-v6-copy-read-leases-2026-09-17/README.md).

The original issuance-only proof describes the unchanged inner journal. It does
not prove this owning wrapper's reader invariants or Rust/native composition.
Quiescence evidence in the executable model is an inert premise; Context consumes
the backend's classification contract rather than manufacturing machine evidence.

## Remaining Work

Leases protect ownership and version stability, not initialized bytes. Lineage
zero is allowed; even nonzero lineage does not prove that every byte was written.
The typed-input extension now adds ordinary pure-Read launch custody. General
kernel effect authority, available-input authority, backend aliases, ordered
writers, cross-run versions, content reuse, aggregate pools/residency and native
high-depth/overlap/fault qualification remain open. Production Worker V3 and
machine refinement, reader-composition proofs and matched HIP/HSA measurements
remain separate acceptance requirements. No new native speedup is claimed.
