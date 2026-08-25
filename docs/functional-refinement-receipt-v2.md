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
- the complete canonical ranked graph, including every operation, CFG
  terminator, branch argument/control dependency, and execution layout;
- ranked view shape, dynamic extents, memory space, allocation origin, and
  no-alias class;
- the exact GPU write block/operation and its semantic RHS, including atomic
  ordering/scope;
- the reference output argument/block/statement;
- GPU/reference coordinate, domain, precondition, and value formula pairs; and
- the exact ownership contract present in that graph.

The receipt attests the graph and formula proof. It does not attest a cached
analysis result. V2 compilation reruns the mandatory effect and hierarchical
ownership analyses on that exact graph; evidence is retained only when those
freshly computed reports are clean. ExactView is not shorthand for dynamic
whole-buffer coverage: runtime-only dynamic ownership remains Incomplete unless
the graph contains enough static or dominating facts to prove it.

For Boolean and 8/16/32/64-bit integer expressions, the generated Verus program
interprets the closed scalar language: wrapping and statically discharged
checked arithmetic, signed/unsigned division and remainder, bitwise operators,
signed/unsigned shifts and comparisons, selects, and Rust integer casts. The
proof therefore establishes equality of the interpreted MIR bitvector values,
not merely equality of operator tags. Operation definedness is checked before
receipt admission. Floating-point expressions still use a separately tagged
uninterpreted operator-congruence model; their receipt proves typed MIR
operator identity under the declared rounding/exception policy, not IEEE value
equivalence or target-instruction conformance.

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

The producer requires FunctionalRefinementVerusRuntimeLeaseV1 over a retained
no-follow root under /opt/fe2o3/verus-runtime-v2/<version>. Its exact manifest
contains only the pinned rust_verify, Z3, Rust toolchain/target files, system
libraries, and empty directory. The legacy reviewed workload proof tree is
absent from both this manifest and the generated proof child's inherited
descriptors. A loose Verus installation or the general-GEMM proof closure is not
silently substituted.

## Remaining boundaries

The compiler frontend derives one reference output location/formula and one GPU
write location/formula from same-session monomorphized MIR projections. It
rejects ambiguous definitions, unsupported unchecked operations, loads, calls,
loops, multiple bindings/writes, and expression chains beyond the fixed depth
budget. This layer binds and checks that compiler-owned projection; it does not
establish Rust source-to-MIR correctness or prove the projection algorithm
itself.

The current formula generator is intentionally bounded and acyclic. Each proved
effect establishes partial correctness for that one effect. Total output
refinement additionally requires a non-vacuous clean total-view ownership
result, an effect proof for every observable output write, and a serialized
record that retains those exact pass summaries. Finite folds, bounded
recurrences, and permutation gathers now have dedicated generic PLIRON
contracts, but the rustc frontend does not yet synthesize them from arbitrary
source loops, reads, reductions, MFMA sequences, or multiple output effects.
Those frontend and lowering refinements remain required before complete
workload semantics can be claimed. Later compiler stages must preserve the
imported evidence lineage without treating it as source-to-ISA, artifact,
load, launch, runtime, or hardware proof.
