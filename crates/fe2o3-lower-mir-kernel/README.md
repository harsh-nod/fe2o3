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
subtract, wrapping multiply, bit-and, bit-or, and bit-xor. The V3 production
certificate checker discharges the theorem's operand and destination relation
for a deliberately narrow straight-line, parameter-or-constant-rooted SSA
fragment. A source constant must match its exact effect-free KIR constant
definition. A copied, unprojected `u32` argument local must match its positional
KIR function parameter in the replayed production correspondence. Any other
copied, unprojected source local must map to an earlier certified KIR result in
the same one-block function. The unprojected source destination is then mapped
to the exact KIR binary result. Under that checked relation, the Verus theorem
`fe2o3_mir_kir_u32_element_refines_v1` establishes equal output and an equal
ordered read/read/write trace. The production evidence builder revalidates the
live semantic owner, requires the statement span to contain exactly the
constant definitions and binary operation prescribed by the source operands,
and binds the certificate to the semantic MIR and canonical KIR identities.
Existing V4 correspondence evidence can compose this semantic evidence without
changing the V4 wire format. V3 assigns separate canonical tags to constants,
parameters, and earlier-result locals; V2 evidence cannot be reinterpreted as
V3.

`production_source_mir_kir_composition_v2` V3 evidence independently joins authority-free
source-to-MIR evidence to a replayed live semantic/KIR owner. For every source
certificate it requires the same semantic module, statement, operator, exact
left/right parameter-local identities, and destination local, then retains the
same-session HIR owner/expression and raw-MIR body identities together with the
exact canonical KIR and ordered operand/result SSA identities. The live join
rechecks the exact operation and its non-aliasing result in the canonical KIR.
Its Verus theorem is universal over explicit source-local, semantic-local, and
KIR-SSA valuation maps related by the exact retained local/parameter maps. It
composes separate six-case source-to-MIR and MIR-to-KIR opcode relations and
proves the destination-local and result-SSA state updates agree. Because the
binary operations are effect-free, the observation is only the ordered
operands and result, not a synthetic write or memory-effect trace.

The private production wrapper counts syntactically eligible direct-parameter
`u32` expressions and covered compositions. It reports `Proved` only for a
nonempty exact roster, `Incomplete` when rustc eliminated or reshaped an
eligible expression, and `NotApplicable` only for zero eligible expressions.
The status and both counts remain retained through the production stages. The
public inert constructor does not authenticate rustc provenance. Only
`rustc-codegen-fe2o3` constructs the private wrapper from same-session source
custody, and neither form grants authority. The Rust opcode conversion and
candidate classification are executable checker premises and remain in the
trusted computing base; the Verus theorem proves their explicit relation but
does not cryptographically authenticate those integers.

This claim does not cover moved or projected operands, projected destinations,
multi-block covered functions, calls as scalar operations, pointer/address
equivalence, bounds, aliasing, whole-function control-flow refinement,
concurrent invocation behavior, floating-point behavior, LLVM lowering,
machine code, runtime behavior, or launch correctness. Unsupported input
relations fail closed and produce no evidence. The trusted computing base is
the rustc observation/extraction code, Rust live-owner and canonicalization,
the independent statement-recipe, parameter, composition, and local-to-SSA
checkers, SHA-256 identity binding, the pinned Verus/vstd/Z3 closure, and the
small executable/spec semantics. The proofs contain no axioms, admits, or
external bodies and grant no artifact or execution authority.

## MIR-to-KIR call/CFG refinement V2

`production_mir_kir_cfg_refinement_v2` closes one multi-function control-flow
slice. A unit-returning kernel entry begins by passing its first `u32`
argument to one non-recursive internal helper. The helper implements exactly
`if x == 0 { x } else { C }` as a four-block diamond; both arms flow into one
join parameter and the helper returns that parameter. Separate executable MIR
and KIR machines consume six semantic macro steps and expose the internal
helper/call result plus the ordered call, selected arm, join, and return trace.
The final step reaches the caller-continuation observation boundary; it does
not execute or make a claim about the rest of the kernel. Insufficient fuel
fails closed.

Production evidence replays the live owner and binds the admitted semantic-MIR
and canonical-KIR identities. It checks the caller argument and call-destination
locals, exact entry-call operation span and continuation edge, direct callee and
call result, every helper source/KIR block, switch direction, fallback constant
definition, both edge arguments, join parameter, and helper return. The Verus
proof uses explicit MIR locals, KIR SSA valuations, blocks/PCs, and independent
fuel-consuming step relations. It proves both machines terminate at the
observation boundary for every related `u32` environment and that each paired
macro step preserves the simulation relation. Hostile branch, callee, phi, and
return substitutions fail verification.

This is not general CFG or call refinement. It excludes loops, recursion,
multiple cases/helpers, arithmetic in either arm, caller-visible return values
(the kernel returns unit), panics/unwind, floats, atomics, barriers, tensors,
MFMA, LLVM, runtime, and hardware. Kernel continuation control, memory,
pointers, effects, and consumption of the call result may exist only because
the normal production compiler verifies them separately; they are outside this
theorem. The Rust shape/correspondence checker, canonical owner machinery,
SHA-256, and pinned Verus/vstd/Z3 closure remain in the trusted computing base.
