# fe2o3-verifier

`fe2o3-verifier` contains workload-neutral proof planning, authenticated tool
execution, bounded runtime custody, and compiler-issued refinement receipts.

Its production-facing functional-refinement path binds the exact Rust source,
MIR and ranked PLIRON owners, generated Verus input, toolchain closure, process
occurrence, and imported result. Validation is fail closed and resource
bounded. A successful structural or execution record is not by itself proof
that compiler lowering, generated machine code, loading, or launch is sound.

The crate also provides generic records for:

- authenticated executable and proof bindings;
- authority-free admission of exact static capability obligations and results;
- control-flow and static-view proof obligations;
- multi-kernel proof association and replay protection;
- persistent freshness ledgers;
- canonical proof capsules and verifier invocation plans.

Workload-specific mathematical models, CPU references, and Verus examples live
with their examples. They are not exported as production verifier authority and
must enter the same generic refinement interface as any other kernel.

Static capability admission rejects non-final outcomes, stale graph epochs,
obligation omission, receipt substitution, and kernel/root/target/launch
mismatches. Its move-only result remains inert. The sealed Worker V3 verifier
must still reacquire protected policy, consume existing owned proof and target
lineage, authenticate refinement producers, and perform any later authority
transition.

The production source-owner join is explicit and fail closed.
`ValidatedCompilerProofInputsV5` composes the frozen V4 source owner with a
separate exact KIR V12 receipt, graph epoch, complete execution subject,
compiler policy, capability association, and source/machine refinement receipt
identities. The sealed Worker V3 path accepts capability evidence only through
this native move-only V5 owner and retains the joined owner through machine
refinement and application preparation. V8 remains the exact frozen V4 capsule
input and cannot authorize V12 by decoding, identity comparison, or projection.

The remaining integration dependency is producer-side: the compiler pipeline
must emit the V5 association and its separate canonical KIR V12 receipt beside
the frozen V3/V4 artifacts. This crate validates and retains those values but
does not synthesize them.

The retained Verus runtime closure is described by
`verus/pins/FUNCTIONAL_REFINEMENT_RUNTIME_V1.manifest`. The installation helper
is `scripts/functional-refinement-verus-runtime-v1.sh` at the workspace root.
That closure authenticates bounded tool execution; it grants no GPU artifact,
lowering, load, or launch authority.

Run the crate suite with:
