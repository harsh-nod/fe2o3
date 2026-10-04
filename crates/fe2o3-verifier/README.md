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
- control-flow and static-view proof obligations;
- multi-kernel proof association and replay protection;
- persistent freshness ledgers;
- canonical proof capsules and verifier invocation plans.

Workload-specific mathematical models, CPU references, and Verus examples live
with their examples. They are not exported as production verifier authority and
must enter the same generic refinement interface as any other kernel.

The retained Verus runtime closure is described by
`verus/pins/FUNCTIONAL_REFINEMENT_RUNTIME_V1.manifest`. The installation helper
is `scripts/functional-refinement-verus-runtime-v1.sh` at the workspace root.
That closure authenticates bounded tool execution; it grants no GPU artifact,
lowering, load, or launch authority.

Run the crate suite with:
