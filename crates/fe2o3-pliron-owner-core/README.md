# fe2o3-pliron-owner-core

`fe2o3-pliron-owner-core` is the lower-level ownership boundary shared by the
fe2o3 Pliron session shell and admitted dialect adapters. It owns opaque,
process-local context identities and the bounded typed capability used to
register dialect entities.

The crate depends only on `std` and the workspace-pinned Pliron revision. It
does not own a session, expose raw Pliron pointers, execute passes, construct
operations for callers, or grant artifact, proof, publication, or runtime
authority. Dialect registration hooks receive only
`DialectRegistrationService`; its context, namespace, action counter, and
construction remain private. The full `fe2o3-pliron` session shell publicly
re-exports this API for compatibility.
