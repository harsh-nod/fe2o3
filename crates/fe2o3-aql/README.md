# fe2o3-aql

`fe2o3-aql` is a `no_std`, dependency-free production contract for the exact
64-bit little-endian AMDHSA kernel-dispatch packet and ROCr user-signal prefix
reviewed for the first gfx942 runtime path.

It provides:

- bounded one-, two-, and three-dimensional dispatch geometry;
- the exact unpublished 64-byte kernel-dispatch packet layout;
- the system-scoped header/setup publication word;
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

GPU writes to the signal value and their visibility to a Rust atomic load are
contracted platform/coherency facts. Rust's language memory model alone does
not prove firmware completion semantics or device/host cache coherence.

The signal event fields are zero. The first profile therefore supports bounded
busy polling only; interrupt-backed waits remain a later, separately admitted
extension.
