# fe2o3 compiler execution protocol

This crate owns the canonical, inert compiler-execution issuer policy,
attestation, receipt-carriage, and bounded service packet records. It contains
no process launcher, signer, durable ledger, compiler, artifact publisher,
loader, or GPU execution authority.

`fe2o3-runtime-protocol` re-exports these records for compatibility with its
existing load-envelope API. New compiler-execution components should depend on
this crate directly.
