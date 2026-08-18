# fe2o3-service-model

`fe2o3-service-model` is the executable-free P0 model for persistent GPU
services described by issue #135. It contains bounded identity inputs, abstract
states, legal transition relations, global invariant checking, and independent
property classifications.

This crate does **not** implement a queue, scheduler, host service, GPU kernel,
compiler lowering, AMD memory operation, proof, or runtime integration. A
validated value is descriptive model data only. It grants no proof, artifact,
load, launch, execution, progress, storage-release, or performance authority.

The crate has no Pliron, HSA, Verus, host-runtime, or platform dependency. The
canonical preimages are deterministic and bounded. Their digest algorithm is
intentionally not selected here: the canonical identity contract owned by
issue #134 must perform that operation and return a typed opaque commitment.
