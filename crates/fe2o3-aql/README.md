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
- the exact 64-byte, 64-aligned busy-wait completion signal initialized to one;
- typed numeric address observations with the required descriptor, kernarg,
  and signal alignment checks;
- a canonical source and ABI manifest.

This crate is deliberately inert. Numeric addresses do not prove allocation,
mapping, ownership, lifetime, device identity, or accessibility. The crate does
not reserve a ring slot, copy a packet, perform the release atomic publication,
map or store a doorbell, poll with a timeout, create or destroy a queue, or
establish hardware completion. Those operations require the KFD memory and
queue authority layers.

The mutable reservation model prevents duplicate slots only within one model
instance. It is not a native ring lease. A later queue authority layer must own
the only instance, acquire the read pointer, retain the memory publication,
copy the INVALID packet body, combine its already-copied setup halfword with
the invariant final header in one release `u32` publication,
advance the write pointer, and ring the exact admitted doorbell in that order.

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
release-word, frame, and single-producer counter relations, including concrete
non-vacuity witnesses and five expected-negative mutations. It does not refine
this crate's executable Rust, establish that a publication target performs a
CPU release atomic, authenticate a read pointer, or prove device visibility,
firmware consumption, native slot ownership, completion, liveness, or
performance.
