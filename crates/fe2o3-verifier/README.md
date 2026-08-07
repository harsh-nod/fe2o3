# fe2o3-verifier

`fe2o3-verifier` defines bounded records and process execution at the
verifier-driver boundary. It normalizes proof configuration, requested
properties, trusted items, tool identities, policy, canonical request bytes,
process output, and strict recorder results.

The crate does not invoke a shell. `CommandSpec` keeps the program and each
argument separate, and the bounded executor launches only the planned recorder
with an empty environment, null stdin, and fixed working directory. Tests use a
fixture recorder and do not require Verus or a solver installation.

`execute_authenticated_verus` is the measured Linux path. It reads Verus, the
solver, and the evidence recorder exactly once into anonymous executable
snapshots, computes SHA-256 over those exact bytes, compares each measurement to
the trusted policy, and seals each snapshot against write, growth, and shrink
before execution. The canonical proof request is also sealed. The result file
is anonymous and is sealed immediately after the recorder exits.

The authenticated recorder receives a fresh 256-bit challenge and canonical
invocation, policy, request, Verus, solver, and recorder digests. Its strict
`FE2O3-VERUS-AUTH-RESULT-V1` envelope must echo every binding before embedding a
canonical `FE2O3-VERIFIER-RESULT-V1` payload. A stale result therefore fails the
challenge check, while policy, input, or executable substitution fails its
specific digest check. The returned transcript binds the exact bounded stdout,
stderr, and result bytes and their SHA-256 digests.

`bind_authenticated_proof_executable_v1` is the fail-closed artifact bridge. It
recomputes the retained invocation, policy, request, payload, and complete
authenticated-envelope identities; reparses the exact sealed result; matches
an independent `ProofMatchPolicy`; and constructs the exact
`ProofExecutableBindingV1`. The resulting identity commits the fresh challenge,
all three measured executable snapshots, stdout/stderr/result transcripts,
compiler and source semantics, finalized HSACO digest, target and code-object
version, ABI, launch contract, effects, and proof policy.

The bridge consumes its challenge and transcript in an
`AuthenticatedExecutionFreshnessV1` ledger only after every check succeeds.
Failed attempts leave freshness available for a corrected policy. Reusing the
same evidence in one ledger is rejected.

## Trust boundary

- `VerifierPolicy` is the explicit local trust anchor. The authenticated API
  accepts no separate caller claim for measured tools; it derives executable
  digests from the sealed bytes and requires exact policy matches. Names,
  versions, configuration digests, model, axiom policy, and timeout ceiling are
  committed by canonical policy bytes.
- A `Proved` result is evidence, not authority to load or launch a kernel. The
  artifact finalizer must reconstruct and match target, configuration, model,
  invocation, tool, property, and trusted-item identities.
- The parser accepts the recorder envelope, not unstructured Verus output. A
  reviewed recorder must translate Verus and solver outcomes, inventory trusted
  escapes, and emit the envelope only after both tools terminate. The caller
  must also supply the recorder's process termination; only exit code zero can
  produce a parsed result.
- Correlation IDs prevent accidental request mixups. Authenticated executions
  additionally use an OS-generated challenge to reject stale result replay.
- `AuthenticatedVerusExecutionEvidenceV1` has private construction and exposes
  descriptive measurements and transcript bytes only. It has no runtime,
  module-load, kernel-launch, or compiler-refinement capability.
- `AuthenticatedProofExecutableBindingV1` is also evidence only. The legacy
  conversion and artifact-binding paths remain explicitly descriptive and
  cannot acquire authority by supplying unmeasured identities.

## Current limitations

There is no reviewed production Verus recorder, signature or remote-attestation
scheme, dynamic-library closure measurement, compiler-refinement proof, or GPU
runtime authority. Authentication is local and relative to the supplied trusted
policy. The sealed execution path currently requires Linux `memfd_create`,
`fcntl` seals, and `/proc/self/fd`.

The freshness ledger is process-local. A production admission service must
persist consumed challenge and transcript identities transactionally across
restart before this evidence can participate in a runtime-authority decision.

A timeout kills and reaps the direct recorder child, but does not yet establish
a process group or forcibly terminate arbitrary descendants. The existing
legacy `build_invocation_plan` and `execute_recorder` APIs still accept
caller-supplied tool identities and intentionally cannot construct
`AuthenticatedVerusExecutionEvidenceV1`.
