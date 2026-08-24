# Functional Refinement Receipt V2

## Authority boundary

Production admission accepts only `SafeReferenceMirToKernelMir`. A source hash
may be retained as identity data, but it does not prove source-to-MIR
correspondence. The imported proof grants exact `FunctionalRefinement` evidence
for the bound MIR subjects and normalized obligation/effect transcript. It does
not grant lowering, source-to-ISA, artifact, load, launch, runtime, or hardware
authority.

`ProductionReferenceProofV1::declare_exact` is compatibility data. The V1
production path materializes it as unsupported evidence, so a declarative
`Proved` value cannot make the semantic pipeline clean.

Construction is acyclic: the initial ranked graph contains an unbound request
with subjects but no receipt. The typed producer executes that exact request,
then `bind_functional_refinement_request_v2` consumes the kernel and replaces
only the addressed operation. Production compilation rejects any unbound
request.

## Producer and import

`fe2o3-verifier` owns the authoritative producer. It takes a validated
`ProductionRankedKernelV1`, exact operation location, and compiler-provided MIR
subjects. It walks the ranked semantic DAG and internally emits the Verus
program. There is no caller-provided Verus source in the public producer API.

The versioned, length-delimited transcript binds:

- safe-reference kind, identity, source/MIR hashes, kernel subject/MIR hashes;
- function and exact block/operation location;
- transitive scalar/index formula definitions;
- ranked view shape, dynamic extents, memory space, allocation origin, and
  no-alias class;
- the unique correlated write, including atomic ordering/scope;
- the exact ownership contract; and
- the normalized domain, precondition, and value formula pairs.

The distributed path returns an unsigned canonical receipt to a configured
signer. The local path creates an ephemeral compiler-owned Ed25519 trust root,
executes the same generated proof, signs and strictly imports it in-process, and
returns the imported proof plus the matching production trust policy.

The strict importer rejects noncanonical wire data, forged signatures,
non-`Proved` results, wrong signer/toolchain/boundary, stale subject or formula
hashes, replay, and resource-limit violations. Production Pliron admission also
recomputes the transcript from the current recipe and rejects missing,
duplicate, unused, or mismatched proofs. Pliron obligation, subject, model, and
evidence IDs are independently domain-separated and checked for zero/collision.

## Execution requirement

The producer requires the retained no-follow runtime closure under
`/opt/fe2o3/verus-runtime-v2/<version>`. A loose Verus installation is not
silently substituted. On the MI300X validation host that retained closure was
absent, so the production builder failed closed. The pinned loose Verus
distribution was exercised separately: the generated commutative-add proof
reported `1 verified, 0 errors`, while changing one add to multiply reported a
failed postcondition and `0 verified, 1 errors`.

## Remaining boundaries

The compiler frontend must provide current safe-reference and kernel MIR hashes.
This layer authenticates and consumes those identities; it does not establish
Rust source-to-MIR correctness. Later compiler stages must preserve the imported
evidence lineage without treating it as ISA or artifact proof.
