# fe2o3 Worker V2 bundle

This crate defines the bounded canonical `WorkerV2LoadEnvelopeV1` wire. The
envelope owns one existing artifact container, its exact bundle index,
direct-link evidence, canonical device descriptor table, one proof record per
kernel, the raw pre-finalization HSACO, and an inert projection of the durable
publication claim.

The schema composes existing canonical codecs. It does not reinterpret their
wire formats or recreate authority-bearing values. In particular, an envelope
contains no process-local publication lease, currentness token, HSA executable,
loaded module, or launch token. Decoding and validation grant no load, launch,
proof, compiler-origin, or currentness authority.

`BackendPublicationReceiptProjectionV1` preserves every public receipt field,
but deliberately cannot be converted back into
`BackendPublicationReceiptV1`. The current artifact-transaction API also has no
read-only operation that reacquires a fresh lease from this inert claim. A
future recovered-admission adapter must authenticate the envelope, reacquire
and validate current publication state, reconstruct the typed proof/compiler
joins, inspect the finalized HSACO, and compare its embedded descriptor table
before host admission.
