# Compiler Execution Subject V1

## Status

The sole production rustc backend derives one canonical compiler-execution
subject immediately after publishing its strict semantic V3 compiler-module
handoff. This closes the canonical-subject milestone in
[issue #218](https://github.com/harsh-nod/fe2o3/issues/218). It does not close
the `CompilerExecutionProvenance` authority obligation.

The authoritative implementation is
`fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1`.

## Purpose

The protected attestation issuer and the runtime verifier need one exact,
bounded byte sequence that names the same compiler occurrence. Signing a
caller-selected hash, one compiler executable, or only the final module would
leave substitution gaps. Copying the complete semantic handoff into every
attestation record would instead duplicate a payload whose configured maximum
is large.

Subject V1 is a fixed 690-byte canonical association. It retains complete
compiler pins and compact native identities for the large canonical records.
The producer derives it only from a validated durable V3 publication receipt
and its exact strict handoff. A consumer independently reconstructs the same
bytes from the strictly consumed handoff and transaction binding.

## Bound Axes

The schema binds these fields in fixed order:

1. Durable build-attempt generation, session, and source/configuration build
   invocation.
2. Closed V3 transaction slot and exact transaction identity.
3. Domain-separated digest of the complete canonical V3 rustc invocation.
4. All six `CompilerClosureV2` pins, its Cargo transition protocol, and its
   independently rederived closure identity.
5. Rustc semantic identity-inventory receipt identity and byte length.
6. Rustc semantic preflight-plan receipt identity and byte length.
7. Complete semantic-capsule identity and byte length.
8. Final compiler-module commitment receipt identity and byte length.
9. Native V2 compiler-module handoff identity and byte length.
10. V3 capsule/module pair-binding identity and byte length.
11. Exact outer semantic V3 handoff identity and byte length.
12. A terminal domain-separated subject identity over every preceding byte.

The semantic capsule already retains the exact V3 invocation, inventory,
preflight, semantic MIR, middle-end evidence, Kernel IR, refinement and memory
evidence, target records, and final module commitment. Subject V1 does not
duplicate those potentially large bytes. The runtime must retain the exact
handoff and recompute the subject before accepting an attestation.

## Canonical Encoding

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | Magic `F2O3CES1` |
| 8 | 2 | Version `1`, little endian |
| 10 | 2 | Zero flags |
| 12 | 8 | Exact total length `690` |
| 20 | 4 | Zero reserved word |
| 24 | 56 | Build attempt |
| 80 | 8 | V3 slot plus seven zero reserved bytes |
| 88 | 32 | V3 transaction identity |
| 120 | 32 | V3 rustc invocation digest |
| 152 | 226 | Complete compiler closure |
| 378 | 280 | Seven ordered identity/length bindings |
| 658 | 32 | Terminal subject identity |

The terminal identity preimage is the V1 domain, the little-endian length of
the 658-byte prefix, and that exact prefix. Decoding accepts exactly 690 bytes,
rejects unknown versions, flags and slots, requires every reserved byte to be
zero, reconstructs the build attempt and compiler closure, rejects zero
identities and lengths, verifies the terminal identity, and then requires an
exact canonical re-encoding.

## Production Placement

The single production path now performs these transitions:

```text
FinishedProtectedRustcInvocationV3
  -> semantic lineage and strict outer V3 handoff
  -> repeat live protected-rustc admission
  -> durable strict V3 publication
  -> InertCompilerExecutionSubjectV1::from_publication
```

The artifact transaction also supports reconstruction from
`ConsumedCompilerModuleHandoffV3`. Publication and consumption therefore
produce byte-identical subjects while retaining their independently validated
transaction occurrence.

## Security Boundary

The subject is intentionally inert and cloneable. Its codec proves canonical
association only. It does not prove that rustc or the backend executed, does
not establish freshness, does not authenticate a producer, and grants no
compiler, publication, load, or launch authority. Its API reports these limits
directly and reports that protected execution attestation remains required.

The following work remains before `CompilerExecutionProvenance` can close:

1. Invoke a caller-pinned protected issuer while live rustc custody and the
   canonical subject are both present.
2. Bind a fresh challenge and exact issuer/runtime policy to the signed receipt.
3. Carry the receipt through V3 publication, crash recovery, load-envelope
   custody, and the application verification request in a new explicit wire
   schema.
4. Reconstruct the subject at the consumer, verify freshness and rollback
   state, and join the authenticated receipt to proof and exact HSACO evidence.

No COMGR path is involved. LLVM module linking and code generation remain on
the pinned upstream LLVM API path.

## Test Evidence

The implementation has tests for:

- exact publication-to-consumption reconstruction;
- all 690 individual canonical byte positions;
- independently resealed invalid headers, slots, reservations, attempts,
  closure pins, identities, and lengths;
- one-axis semantic substitutions changing the subject identity;
- strict handoff replacement rejection; and
- explicit no-authority behavior.

The affected package suites, doc tests, strict artifact-transaction Clippy,
workspace dependency policy, parity matrix, and parity dashboard are required
before publication to `main`.
