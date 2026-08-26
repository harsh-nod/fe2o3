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

`CompilerTransactionRecorderV1` is the bounded gfx942 alpha/zeta compiler
recorder. It measures exact source-tree contents, canonical rustc V2 invocation
bytes, rustc/backend binaries and configurations, backend invocation bytes,
the fixed `gfx942:xnack-`/wave64/COV6/`amd-wave` target profile, alpha/zeta
semantic layout witnesses, Kernel IR, Worker request/response bytes, raw/final
HSACO, descriptor source/final bytes, and the artifact container. The rustc
descriptor and Worker request must use the exact canonical AMD target spelling;
bare `gfx942`, `xnack+`, extra or unknown features, and noncanonical feature
order are rejected.

The recorder typed-decodes every post-IR boundary. A successful Worker response
must bind the exact request and define exactly `alpha` and `zeta`; its output
must equal the raw HSACO. That HSACO must contain a zero-digest COV6 descriptor
table for the fixed target and kernel set. Final bytes must equal deterministic
HSACO finalization, the supplied descriptor bytes must equal the embedded raw
and finalized tables, and the canonical `ArtifactContainerV1` must own exactly
that native payload under the same target and capabilities. Mixed responses,
payloads, descriptors, finalizations, and containers therefore fail before the
transaction can seal.

Each ordered stage consumes a transaction-local checkpoint. Reordered,
repeated, stale, or cross-transaction checkpoints fail before recorder state
changes. The recorder keeps its fixed-profile measurement separate from the
shared `TargetIdentityV1`. At the final-artifact stage, the recorder derives a
descriptive target identity from the validated `ArtifactContainerV1` with the
same canonical manifest-target derivation used by artifact publication. It
does not accept a caller-supplied target identity. The derived identity grants
no publication, currentness, load, launch, or other authority.

`SealedCompilerTransactionV1` retains those measurements and the existing
`CompilerTransactionEvidenceCapsuleV2` in a strict canonical wire. Decoding
reconstructs the complete checkpoint chain and checks duplicated capsule
measurements. `from_bytes_for_identity` is the boundary for rejecting an
otherwise valid stale or substituted record. The caller-provided freshness
binding is coordination input, not proof of currentness.

The schema composes existing canonical codecs. It does not reinterpret their
wire formats or recreate authority-bearing values. In particular, an envelope
contains no process-local publication lease, currentness token, HSA executable,
loaded module, or launch token. Decoding and validation grant no load, launch,
proof, compiler-origin, publication, or currentness authority. A sealed compiler
transaction is an integrity-bound inert record, not a safe-launch credential.

The execution-receipt API is reserved for a future capability and has no issuer
today. In particular, the recorder provides no replay-resistant freshness, no
observation that the measured source and rustc/backend invocation produced the
recorded compiler module, and no real Worker-generated COV6 hardware or
load/dispatch evidence. Its current guarantees are structural consistency and
canonical content binding only.

The application handoff ACK is likewise only bounded liveness and possession
evidence. The child receives its challenge, commitment, and ACK descriptor and
can reproduce the canonical bytes; ACK validation therefore proves no recovery
provenance and grants no host, publication, load, or launch authority.

Production Worker V3 records live in `fe2o3-runtime-protocol`, outside this
qualification crate. Production consumers and cross-version tests depend on
that crate directly. This crate does not re-export the production protocol and
cannot act as a second production route.

`DurablePublishedHsacoClaimV1` preserves the exact publication plan, receipt,
output-directory identity, record identity, artifact identity, and artifact
length. A recovered-admission adapter can use it to reacquire a non-`Clone`
currentness lease, but only after the artifact-transaction crate revalidates
the durable publication under its lock. That adapter must still authenticate
the envelope, reconstruct the typed proof/compiler joins, inspect the finalized
HSACO, and compare its embedded descriptor table before host admission.
