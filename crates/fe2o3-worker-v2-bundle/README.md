# fe2o3 Worker V2 bundle

This crate defines the bounded canonical `WorkerV2LoadEnvelopeV1` wire. The
envelope owns one existing artifact container, its exact bundle index,
direct-link evidence, canonical device descriptor table, one proof record per
kernel, the raw pre-finalization HSACO, and the canonical inert durable
publication claim.

`WorkerV2EnvelopeInputsV1` is the canonical pre-envelope capsule for independently
supplied direct-link evidence, proof records, and exact raw HSACO bytes. Its digest can be
committed before publication so restart recovery can reopen those exact inputs and reconstruct
the complete envelope. Decoding the capsule does not authenticate any of that evidence; a genuine
proof/compiler authenticator remains absent.

The schema composes existing canonical codecs. It does not reinterpret their
wire formats or recreate authority-bearing values. In particular, an envelope
contains no process-local publication lease, currentness token, HSA executable,
loaded module, or launch token. Decoding and validation grant no load, launch,
proof, compiler-origin, or currentness authority.

`DurablePublishedHsacoClaimV1` preserves the exact publication plan, receipt,
output-directory identity, record identity, artifact identity, and artifact
length. A recovered-admission adapter can use it to reacquire a non-`Clone`
currentness lease, but only after the artifact-transaction crate revalidates
the durable publication under its lock. That adapter must still authenticate
the envelope, reconstruct the typed proof/compiler joins, inspect the finalized
HSACO, and compare its embedded descriptor table before host admission.
