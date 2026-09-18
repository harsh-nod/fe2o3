# Conditional Output Coverage

This work contributes to #272 through the canonical compiler pipeline in #271.
It is not an alternative proof, compiler, artifact, or launch path.

## The Missing Relation

Consider the ordinary Rust kernel:

```rust
let index = thread::index_1d();
if let Some(element) = output.get_mut(index) {
    *element = 17;
}
```

Let `G` be the number of logical global-X invocations and `N` the output slice
length. This kernel writes `[0, min(G, N))`. A workgroup-size requirement of
`[64, 1, 1]` does not establish `N <= G`: a valid 64-invocation launch with a
65-element output leaves the last element unwritten.

Pointwise agreement with a Rust reference does not establish total output
coverage. Bounds safety also does not establish that a store executes.

The intended total-coverage theorem is conditional on the exact runtime slice
length being at most the logical global-X extent. It additionally needs a
one-dimensional dispatch, valid index/address representation, admitted memory
bindings, and preservation through the exact final graph and machine path.
Checking a runtime value is not a Rust compile-time error.

## Implemented Analysis Boundary

`derive_conditional_total_view_from_verified_v1` operates on a borrowed,
verified canonical `Module`. It does not create another executable graph or
substitute a sampled launch size for a dynamic extent.

The initial supported fragment contains a single output store with:

- a direct global-X index and an exact comparison with that output's length;
- an address based on that output and the index, or the guarded index selected
  against zero;
- one ordinary, nonvolatile global store;
- an acyclic CFG containing understood operations and normal completion for
  both valid-index invocations and the tail.

The analysis tracks two abstract cases: the bounds predicate is true or false.
Each reachable block carries possible write counts: zero, one, or at least two.
Branch joins take the union of possibilities, not the sum of mutually exclusive
paths. At every feasible normal exit, the valid-index case must have exactly one
write and the tail case must have none. Unknown conditions explore both edges.
Boolean switches are interpreted by their case values, not case ordering.

Address formation has its own retained domain. `GuardedOutput` means that
the direct index is evaluated only for valid output indices, or that the tail
selects offset zero. `GlobalLaunch` means a direct address can also be formed
on the tail, as in the current ordinary-Rust fill lowering. Plain canonical
GEP does not assert `inbounds`; forming an out-of-view address is not a memory
access. However, representability is still an explicit prerequisite. For a
nonzero logical extent `G`, the latter domain requires checked representation
of `base + (G - 1) * element_bytes`, even when `N` is zero. The former requires
the admitted output-span arithmetic and zero-offset formation. Neither is
established by the control-flow analysis itself.

Calls, loops, unsupported operations, unproved reads, and unsupported control
flow do not establish coverage. A store guarded by a bounds predicate alone
is insufficient: bypasses, extra conditions, abnormal exits, and additional
writes are separately checked. `Unsupported` is not a counterexample proving
that the source program is wrong. Resource exhaustion is a separate error.

Work and scratch use the caller's verification budget. Returned facts borrow
the exact module and contain output, predicate, address and store coordinates.
They are structural analysis results, not authenticated production custody,
functional-value proofs, runtime checks, or permission to launch.

## Source and Ranked Correspondence

`ProductionPreRankedKirOwnerV1::bind_conditional_output_v1` binds derived facts
to the exact retained canonical graph, kernel entry, source root and whole
source argument. Equal bytes in a different graph owner do not qualify. It uses
the existing checked argument view, including original source ordinals,
adjusted FnAbi ordinals and physical KIR slots. Those three coordinates need
not be equal: arguments may be erased or expanded into physical components.
Transparent Result wrappers must preserve argument order and types.

The borrowed binding can inspect a ranked candidate through
`inspect_ranked_output_v1`. This joins the exact store's owner-qualified source
span and access ordinal to a unique ranked write, matching effect contract and
global view. It rejects missing or ambiguous correspondence rather than falling
back to allocation identity. Sparse block IDs are resolved by identity, not
vector position. These bounded queries share the caller's work/storage ledger
and retain the canonical address-formation premise.

The ranked candidate is still inert input. This inspection does not authenticate
the translation or prove index, value, predicate or extent equivalence. In
particular, its dynamic extent is returned as `Unbound(operand)`: a ranked
`Argument(0)` does not by itself identify the original Rust output argument or
its length. The reference contract's output ordinal is normalized into logical
kernel-argument coordinates, excluding the CPU point-coordinate prefix; it is
not a raw CPU parameter number. The inspection retains this field but does not
validate the reference's logical ABI relation. No proof counter is incremented.

## Existing Host Binding

The descriptor already identifies each slice-length ABI component. The
generated packing plan can resolve the base address and length together from
the exact packed argument owner, checking plan/kernel identity, generated ABI
field ordinal, unique pointer/length component locations, widths, memory role,
and packing-buffer bounds. That field ordinal is not automatically the Rust
source argument number or physical KIR slot. It does not infer a coverage requirement
from a kernel name or assume that every slice is an output.

The resolver returns inert borrowed data, not pointer validity, allocation
admission or a discharged coverage condition. A host-side `N <= G` comparison without a bound conditional theorem
would not repair an unconditional compiler proof claim.

## Remaining Integration

No conditional result may increment the existing unconditional `TotalView`
proved count. The remaining compiler-to-host connection must:

1. Reconcile the structural coverage premise with the source reference and
   semantic contract, retaining the conditional requirement in aggregate
   proof generation and independent replay.
2. Bind the exact kernel root, canonical subject, source argument map and
   derivation policy through existing #209/#213 custody. Recompute or replay
   affected coverage on the exact optimized graph.
3. Connect native checked-output proof custody to protected publication and
   host admission explicitly. The existing V4/KIR-V8 capsule branch cannot
   silently stand in for that native subject.
4. Resolve the certified output length from the actual packed arguments and
   check it against the actual logical grid before runtime preparation or
   GPU effects. Reject nonunit inactive grid/workgroup axes and invalid
   representation or memory bindings, including the retained address-formation
   domain. Target/machine refinement must separately relate the logical launch
   to physical execution, including inactive lanes.
5. Retain all existing target, artifact, context, borrow and completion checks.

No descriptor wire-format extension is required solely to locate the slice
length. Avoid adding a caller-authored predicate list or independent authority
flag. Deterministic rederivation still requires the admitted exact subject.

Required integration negatives include `G=64, N=65`, omitted or swapped argument
bindings, cross-kernel and stale-graph substitution, unsupported dimensions,
and bypassed host checks. `G=64, N=64` and guarded tails with `N < G` must pass
only after every other applicable requirement is satisfied.

Compiler source-observation tests stop at the real pre-ranked canonical owner
or at reference-proof request preparation. The latter borrows the actual owner
and prepared ranked request in the same phase ledger, then returns an explicit
test-only `Incomplete` error before opening a proof runtime. It cannot yield a
clean ranked program, signed receipt, handoff or artifact.
Finite simulator checks consume its exact bytes, compare output and canary
bytes, and repeat each execution. The fill probe includes lengths 0, 1, 63, 64
and 65 at a 64-invocation launch; the last case must leave one element unwritten.
These tests do not bypass ranked checks, import proof receipts, produce GPU
artifacts, or execute on a GPU. Neither these observations nor the
analysis/packing unit tests qualify a tutorial kernel end to end.
