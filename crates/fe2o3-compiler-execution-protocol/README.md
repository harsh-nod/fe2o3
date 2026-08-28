# fe2o3 compiler execution protocol

This crate owns the canonical, inert compiler-execution issuer policy,
expected-client launch manifest, attestation, receipt-carriage, and bounded
service packet records. The launch manifest binds an exact PID/UID/GID tuple to
an exact policy identity without granting process or signing authority. The
crate contains no process launcher, signer, durable ledger, compiler, artifact
publisher, loader, or GPU execution authority.

`fe2o3-runtime-protocol` re-exports these records for compatibility with its
existing load-envelope API. New compiler-execution components should depend on
this crate directly.
