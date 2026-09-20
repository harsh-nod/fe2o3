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

The initial inspection returns `Unbound(operand)`: a ranked `Argument(0)` does
not by itself identify the original Rust output argument or its length.
Checked-view construction now retains an optional, fixed-size extent proposal
inline in the existing access row. It records the original source argument,
view, index and extent operands; no separate correspondence map is added.

`rederive_output_extent_v1` checks that proposal against the exact conditional
binding, unique dynamic global-X index and supported uses of the extent. Missing
provenance, conflicting interpretations, unrelated arithmetic/view uses and
unsupported operations reject. The result retains the exact canonical
`SliceLength` value and address domain. A matching guard-to-write edge is not a
dominance or complete CFG proof.

The separate `check_ranked_coverage_v1` query rederives that extent and checks
the borrowed, constructor-validated ranked recipe. It walks both truth values
of the exact global-X/length predicate to normal returns: the true case must
execute exactly the selected write, and the false case none. Exact literal
conditions may exclude an edge; unknown conditions, feasible traps or cycles,
extra effects, arithmetic/load expressions and block arguments refuse. Even
unreachable blocks must remain inside the supported operation fragment.

The constructor's definition, scope and edge checks are reused. There is no
copied graph or heap scratch: each deterministic case takes at most the recipe's
block count, and literal lookup scans are charged to the same inherited ledger.
The result keeps the original owner, candidate and address-domain premise.
It does not prove the stored value, source translation, ownership, the actual
length/launch values or `N <= G`.

The ranked candidate and its proposal are still inert input. These queries do
not authenticate translation or establish reference-value equivalence. Existing
source replay validates base occurrence coordinates, not the new extent meaning.
Legacy base-field reconstruction leaves the proposal absent; no old wire or
digest version acquires new fields implicitly. Authoritative replay and a
coordinated conditional-evidence successor remain required.

The reference contract's output ordinal is normalized into logical
kernel-argument coordinates, excluding the CPU point-coordinate prefix; it is
not a raw CPU parameter number. The inspection retains this field but does not
validate the reference's logical ABI relation. No proof counter is incremented.

## Scoped CPU Reference Join

The backend-private `with_conditional_reference_output_v1` composes these
queries against the actual pending ranked owner. Its synchronous callback
borrows checked source translation, canonical and ranked coverage, the selected
ownership occurrence and the original authenticated reference binding. No graph
or argument map is copied. The callback cannot retain the locally derived facts
or convert them into a clean compilation stage.

The initial reference fragment is deliberately narrow: one complete `u32`
output argument, one leading `usize` point coordinate, and one unconditional
constant output store in a normally returning CPU block. The checker compares
the original signature preimage, derived relations, CPU effect IR and selected
write, source-root identities and bound proof subjects. It also checks the
contract's reference and GPU coordinates, domains, preconditions, values and
numeric models in one charged operand scan. Both values must be the same checked
CPU `u32` constant, both coordinates unsigned-64 point axis zero, and both
domains and preconditions true under exact bitvector semantics. The exact GPU
write must be value-bearing and agree with the selected view, index and value;
a value-less access or a matching write at another occurrence cannot substitute.
The existing source replay must match exactly one memory effect and one value.
Those counts constrain the fragment; they are not a value-equivalence proof.
Signature
replay is separate from the effect-IR hash: changing the signature without
changing that hash must not preserve acceptance. Unsupported shapes refuse.

The shared interpretation is output `O`, its checked length `N`, and global-X
index `i`. Under the retained premises, canonical coverage and ranked coverage
each select one same-value store followed by normal return when `i < N`, and
no store followed by normal return otherwise. The CPU point function is lifted
only over `i < N`; its own store remains unconditional. Ranked views do not
contain physical pointer or stride fields. Interpreting the store address as
`base(O) + 4*i` comes from the checked canonical address relation, not comparison
against invented ranked byte-address metadata. Concrete packed arguments,
launch values and memory validity remain unjoined at this observer boundary.

For the fill example, the four checked positions are:

| Position | Output argument |
| --- | --- |
| Rust source | 0 |
| Adjusted FnAbi | 0, complete slice pair |
| Physical KIR entry | 0, global slice |
| Raw CPU reference | 1, following the point coordinate |

The reference contract stores the logical source ordinal, not the raw CPU
ordinal. The source root supplies kernel identity even when a transparent
Result wrapper selects a different body. Equal metadata is not independent
producer custody: the production observer borrows the same per-root roster
used to prepare the proof request, without reconstructing it.

The test-only post-bind observer retains the actual imported receipts and
protected-runtime lease while running the join. It still returns `Incomplete`.
Every residual check, the legacy trace finding, selected coverage blocker and
mandatory bounds diagnostic remain unchanged. Unconditional `TotalView`
credit remains zero. The live work ledger is inherited; existing lowerer
allocation and unwind boundaries are not upgraded to whole-process bounds.

This is not conditional proof discharge or launch authority. The host must
still establish `N <= G`, unit inactive axes, admitted memory bindings and the
retained address-representation domain, including when `N == 0`. Aggregate
proof generation, exact optimized-graph replay, machine refinement and safe
host admission remain separate integration requirements.

## Existing Host Binding

The descriptor already identifies each slice-length ABI component. The
generated packing plan can resolve the base address and length together from
the exact packed argument owner, checking plan/kernel identity, generated ABI
field ordinal, unique pointer/length component locations, widths, memory role,
and packing-buffer bounds. That field ordinal is not automatically the Rust
source argument number or physical KIR slot. It does not infer a coverage
requirement from a kernel name or assume that every slice is an output.

A compiler-private join now checks the conditional binding against the complete
ordered typed-root roster using existing semantic ownership, type and ABI
validators. For the supported flat profile it retains the exact descriptor
argument position, using the certified physical slot only for KIR type checking.
Same-typed fields are selected by source correspondence, not by shape. Ignored,
expanded tuple and context-forwarding profiles remain unsupported. This is
descriptive agreement: caller-supplied matching roots do not acquire
same-transaction custody. Production integration must use the transaction's own
retained owner and typed roots.

The resolver returns inert borrowed data, not pointer validity, allocation
admission or a discharged coverage condition. A host-side `N <= G` comparison
without a bound conditional theorem would not repair an unconditional compiler
proof claim.

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

The private conditional ownership analysis now records available residual checks
without replacing the existing V1 result. Its shared preparation preserves V1
diagnostic order and early exits. The new result separately retains a mandatory
bounds failure even when the legacy dynamic-trace exit masks it, checks exact
effect-domain sites without requiring a complete trace, and runs finite
unselected ownership checks when their prerequisites are available. Each row
distinguishes checked, rejected, inapplicable and blocked obligations. Selected
coverage always remains blocked on canonical coverage and source replay.

Selections identify actual operations and views, not source authority. Context
and operation identity do not supply mutation-epoch or transaction custody;
the existing immutable-function analysis-manager contract still applies. All
new work, retained rows and diagnostic/name storage use that caller's metered
manager. Its conservative monotonic resource domain is distinct from the
canonical query ledger. There is no clean-report conversion, coverage-counter
credit or production admission path in this diagnostic result. The acyclic
typed conditional discharge and independent replay of the reference relation
remain integration requirements. The scoped backend join does not change
these diagnostic rows into proved obligations.

Required integration negatives include `G=64, N=65`, omitted or swapped argument
bindings, cross-kernel and stale-graph substitution, unsupported dimensions,
and bypassed host checks. `G=64, N=64` and guarded tails with `N < G` must pass
only after every other applicable requirement is satisfied.

Compiler source-observation tests stop at the real pre-ranked canonical owner
or at reference-proof request preparation. The latter borrows the actual owner
and prepared ranked request in the same phase ledger, then returns an explicit
test-only `Incomplete` error before opening a proof runtime. It cannot yield a
clean ranked program, signed receipt, handoff or artifact.
The source tests also exercise the full descriptor, extent and CFG-query budget
boundaries; independent test measurements are not replacement production ledgers.
Two same-typed output parameters in retained Result wrappers test exact argument
selection without claiming that the untouched output is fully written.
Finite simulator checks consume its exact bytes, compare output and canary
bytes, and repeat each execution. The fill probe includes lengths 0, 1, 63, 64
and 65 at a 64-invocation launch; the last case must leave one element unwritten.
Retained wrappers use their source-derived 256-invocation launch and lengths
0, 1, 255, 256 and 257, preserving the same boundary check.
These tests do not bypass ranked checks, import proof receipts, produce GPU
artifacts, or execute on a GPU. Neither these observations nor the
analysis/packing unit tests qualify a tutorial kernel end to end.

The separate post-bind source parent does require the real protected proof
runtime and imported receipts. It exercises the scoped CPU reference join on
both target profiles, tampered-reference refusals and inherited-budget limits,
but always stops before a clean output stage. Runtime absence is a refusal,
not an alternative positive result. Those compiler tests also do not execute
on a GPU or qualify a tutorial kernel end to end.

Separate inert operand tests reject changed GPU values, widths, signedness,
numerical models, non-leaf/duplicate definitions and selected-write operands.
Their matching subjects are only a request, never a fabricated signed proof:
even the otherwise matching fixture must refuse. Some malformed graphs cannot
reach this observer because earlier source/proof checks reject them. These
tests therefore make no claim of a real-prover semantic counterexample or an
admitted canonical/ranked mismatch. Actual protected positives and the original
reference-input negatives remain separate tests.
