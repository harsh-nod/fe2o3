# Functional Refinement Receipt V2

## Authority boundary

Production admission accepts only `SafeReferenceMirToKernelMir`. A source hash
may be retained as identity data, but it does not prove source-to-MIR
correspondence. The imported proof grants exact `FunctionalRefinement` evidence
for the bound MIR subjects and normalized obligation/effect transcript. It does
not grant lowering, source-to-ISA, artifact, load, launch, runtime, or hardware
authority.

Legacy declarative proof identities are not part of the production API.
Functional-refinement evidence enters only through an authenticated V2 receipt;
callers cannot construct an unsupported compatibility operation inside the
ranked graph.

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
equivalence or target-instruction conformance. The interpretation is a universally
quantified `spec_fn(int, int, int, int) -> int` parameter, not an assumed function
body. Each aggregate replay passes the same interpretation into every applicable
effect lemma. Integer-only formulas do not carry that parameter. This uses
[Verus spec closures](https://verus-lang.github.io/verus/guide/spec_closures.html)
and keeps `--no-cheating` enabled; the former unconditional `uninterp spec fn`
declaration was rejected by that production flag even for integer-only proofs.

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

The provisioning script accepts an optional `SYSTEM_LIB_DIRECTORY` after the
ordinary `audit-source` or `provision` arguments. This supports recovering the
exact pinned libraries after a host package upgrade without replacing shared
system libraries. The directory supplies all eight libraries using their
`system-lib/` manifest basenames; each must be a regular, single-link file with
the exact pinned size and SHA-256. There is no fallback to host libraries when
a staged file is absent or different. The installed closure is independently
audited after copying, and the system ELF interpreter and its path chain must
still match the existing pins. A different loader requires a compatible host,
not a source-directory override.

For this manifest, the excluded rustup provenance matches the official 1.29.0
`x86_64-unknown-linux-gnu/rustup-init` archive. The pinned zlib bytes match
Ubuntu's `zlib1g_1.3.dfsg-3.1ubuntu2.1_amd64.deb`; extracting that package into a
private staging directory does not install it. These version labels aid
recovery only: the manifest byte pins, not package names, decide admission.
Source audit success does not establish proof execution or artifact authority.

The ignored `pinned_functional_refinement_runtime_executes_a_real_verus_proof`
test exercises the retained process controller using the exact manifest bytes.
Set `FE2O3_FUNCTIONAL_REFINEMENT_TEST_RUNTIME_ROOT` to a complete test closure
and run the selected test with `--ignored`. Its private, test-only filesystem
policy permits a user-owned closure; that policy is absent from production
builds and cannot create a production runtime lease or proof receipt. The
smoke test passed on mi300x on 2026-09-09. Production admission still requires
the root-owned installation and all normal path, inventory, and content checks.
Two additional ignored tests passed through the same retained controller:
`retained_runtime_checks_generated_equivalence_and_rejects_operator_mutations`
checks integer and IEEE operator-congruence formulas and requires wrong-operator
mutations to fail assertions, not runtime setup;
`retained_runtime_executes_generated_ieee_aggregate_formula` checks forwarding
of the universal interpretation through a generated aggregate proof. None of
these tests signs a receipt or establishes tutorial compilation or GPU execution.
Runtime errors preserve the underlying diagnostic and error chain, including
the failing object or protection check.

The additional ignored
`production_runtime_imports_generated_proofs_and_rejects_wrong_operators` test
requires the real root-protected installation. It opens the normal production
lease, executes and locally imports integer and IEEE operator-congruence
proofs within a 60-second per-proof deadline, and rejects wrong operators at
verification rather than during setup. All four cases passed on mi350 in
24.85 seconds on 2026-09-09. The exact runtime was installed only inside a
private mount namespace; host libraries and manifest pins were unchanged.
Bounded controller polling now starts at 50 microseconds after progress and
backs off to the existing 2-millisecond maximum, retaining all deadline,
process-tree, mapping, output, and cleanup checks. Real vecadd extraction also
passes local reference-proof import, then rejects dynamic-launch ownership.
Neither result establishes final-graph equivalence or GPU qualification.

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
