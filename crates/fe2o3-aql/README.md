# fe2o3-aql

`fe2o3-aql` is a `no_std`, dependency-free production contract for the exact
64-bit little-endian AMDHSA kernel-dispatch packet and ROCr user-signal prefix
reviewed for the first gfx942 runtime path.

It provides:

- bounded one-, two-, and three-dimensional dispatch geometry;
- the exact INVALID unpublished 64-byte kernel-dispatch packet layout;
- a linear prepared value that exposes only the invariant system-scoped final
  header after the exact INVALID body;
- checked monotonic single-producer reservation arithmetic and slot wrapping;
- one inert, all-or-nothing reservation of 1 through 256 ordered packet IDs
  with distinct wrap-aware slots;
- a fixed prepared-batch value that drives all INVALID body writes before any
  ordered release-header callback;
- an additive V2 fixed-batch type and reservation transition for 1 through
  8192 packets, with the exact-cardinality packet array heap-owned, while
  retaining the exact V1 256-packet boundary;
- the exact 64-byte, 64-aligned busy-wait completion signal initialized to one,
  plus an exact inert pending-signal byte image;
- pure classification of a completion value already acquired elsewhere;
- typed numeric address observations with the required descriptor, kernarg,
  and signal alignment checks;
- a canonical source and ABI manifest.

This crate is deliberately inert. Numeric addresses do not prove allocation,
mapping, ownership, lifetime, device identity, or accessibility. The crate does
not reserve a ring slot, copy a packet, perform the release atomic publication,
map or store a doorbell, poll with a timeout, create or destroy a queue, or
establish hardware completion. Those operations require the KFD memory and
queue authority layers.

The byte encoder and exact-array initializer produce bytes only. They do not
start a typed Rust object's lifetime, establish atomic storage, initialize an
allocation, or authorize interpreting arbitrary storage as
`AmdBusyCompletionSignalV1`. The pure classifier performs no load and does not
authenticate where its supplied value came from.

The mutable reservation model prevents duplicate slots only within one model
instance. A batch transition validates the complete packet count, observed
read counter, available capacity, and checked next write counter before it
changes either retained counter. Its ordered entries are arithmetic
observations, not native ring leases. A later queue authority layer must own
the only model instance, acquire the read pointer, retain the memory
publication, reserve the actual write counter once, copy every INVALID packet
body, release-publish every paired header, and ring the exact admitted
doorbell under a separately reviewed batch-publication contract.

The V2 maximum occupies 512 KiB of logical ring slots. A ring smaller than the
requested fixed batch is rejected before the reservation model changes; a
later native owner must still admit and own the corresponding ring, completion
signals, one write-counter increment, and one final doorbell publication.

The prepared-batch target preserves body-before-header call order but remains
inert. Its callback trait does not authenticate a target implementation,
perform a release atomic, or prove that indices name the reservation's native
slots. Those joins remain private responsibilities of the queue owner.

GPU writes to the signal value and their visibility to a Rust atomic load are
contracted platform/coherency facts. Rust's language memory model alone does
not prove firmware completion semantics or device/host cache coherence.

The signal event fields are zero. The first profile therefore supports bounded
busy polling only; interrupt-backed waits remain a later, separately admitted
extension.

The pinned legacy `fe2o3-hsa-runtime/native/runtime.c` path is only a reference
for its final release-`u32` operation. It zeroes a packet body and is not
evidence for this crate's required INVALID-body discipline.

The authenticated
[`aql_publication_v1.rs`](../fe2o3-runtime-model/verus/aql_publication_v1.rs)
Verus model proves the corresponding bounded mathematical body-copy,
release-word, frame, pending-signal byte-image, supplied-value classification,
and single-producer counter relations, including concrete non-vacuity witnesses
and five expected-negative mutations. It does not refine
this crate's executable Rust, establish that a publication target performs a
CPU release atomic, authenticate a read pointer, or prove device visibility,
firmware consumption, native slot ownership, completion, liveness, or
performance. The V1 executable batch transition also needs a later dedicated
Verus relation and concrete-refinement join before it can contribute proof
authority to a native batch publisher.
