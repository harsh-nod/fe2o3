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

## Kernel context lowering

Semantic MIR V15 `KernelContextIssue` is lowered only through the explicit
`ProductionKernelContextLoweringInputV1` API. The lowerer derives the root
symbol, function identity, and kernel-contract identity from the admitted
semantic owner, requires one nonzero authenticated input for each context-
bearing root, and emits canonical KIR V12. Legacy entry points fail closed
instead of erasing the logical zero-sized value.

The production compiler adapter must authenticate the physical root, logical
helper, nominal marker, and unique issuance record from the frontend sidecar;
derive the compilation-unit, marker, target-brand, launch-brand, and issuance
identities; and pass them to `try_lower_with_kernel_contexts`,
`try_lower_after_ranked_checks_with_kernel_context`, or the ranked-roster API.
The lowerer intentionally does not derive these identities from source names.
