# fe2o3-lower-mir-kernel

`fe2o3-lower-mir-kernel` owns a bounded in-memory detached lowering service
from the feature-gated `mir` dialect shell to target-neutral
`kernel.algorithm_root` operations. One verified kernel algorithm root is
materialized for each supported MIR function, in source order.

The accepted source is deliberately narrow. A source must be one verified
`mir.module`; its direct children must be `mir.func` operations; and every CFG
block may contain only its canonical `mir.block` marker followed by
`mir.return`. All traversal is bounded before recursive Pliron verification.
Any unsupported operation, malformed structure, exhausted source bound,
unsupported rank, or exhausted rewrite bound is a terminal typed error. The
service has no fallback path and never reports a result after failure.

Successful results retain the source operation pointer and a pointer-independent
observation of module identity, function identity and ordinal, argument type
references, canonical block identifiers, and admitted MIR operation order.
This evidence lets a later exact bridge check that it is consuming the same
in-memory source. It is not a durable MIR identity, equivalence proof, artifact
identity, or authorization. Result validation rechecks both the live source
evidence and every emitted kernel operation. Results and registration markers
are bound to a private, context-owned `fe2o3-pliron` identity anchor, so moving
public auxiliary-data markers cannot transfer them to another context.

The result accessors expose contextless Pliron `Ptr` values only for internal
pipeline integration. Those values are Pliron-TCB handles, not portable or
self-authenticating references; callers must validate the result against its
owning context before using them and must never dereference them in another
context. Validation reports erased source and output handles as typed errors
instead of allowing Pliron traversal panics to escape.

The crate does not choose a GPU or physical target and contains no AMDGCN,
COMGR, `pliron-llvm`, compiler, linker, artifact publication, loader, launcher,
tuning, proof-authority, runtime, filesystem, process-execution, or unsafe-code
surface in its own source. Pinned Pliron remains part of the memory-safety
trusted computing base.

This crate deliberately does not implement Pliron's `Pass` trait. The service
materializes detached operations outside the source root, which is not a legal
in-tree pass rewrite. Callers invoke `run_checked` and retrieve the explicit
detached bundle from the service result.

## MIR-to-KIR scalar refinement V1

`production_mir_kir_scalar_refinement_v1` is the first operationally checked
semantic slice. It gives distinct executable MIR and KIR semantics for one
selected `u32` element and the closed operations wrapping add, wrapping
subtract, wrapping multiply, bit-and, bit-or, and bit-xor. The V2 production
certificate checker discharges the theorem's operand and destination relation
for a deliberately narrow straight-line, constant-rooted SSA fragment. A
source constant must match its exact effect-free KIR constant definition. A
copied, unprojected source local must map to an earlier certified KIR result in
the same one-block function. The unprojected source destination is then mapped
to the exact KIR binary result. Under that checked relation, the Verus theorem
`fe2o3_mir_kir_u32_element_refines_v1` establishes equal output and an equal
ordered read/read/write trace. The production evidence builder revalidates the
live semantic owner, requires the statement span to contain exactly the
constant definitions and binary operation prescribed by the source operands,
and binds the certificate to the semantic MIR and canonical KIR identities.
Existing V4 correspondence evidence can compose this semantic evidence without
changing the V4 wire format.

This claim does not cover parameter-rooted values, moved or projected operands,
projected destinations, multi-block functions, pointer/address equivalence,
bounds, aliasing, whole-function control-flow refinement, concurrent invocation
behavior, floating-point behavior, LLVM lowering, machine code, runtime
behavior, or launch correctness. Unsupported input relations fail closed and
produce no evidence. The trusted computing base is the Rust live-owner and
canonicalization, the independent statement-recipe and local-to-SSA checker,
SHA-256 identity binding, the pinned Verus/vstd/Z3 closure, and the small
executable/spec semantics. The proof contains no axioms, admits, or external
bodies and grants no artifact or execution authority.
