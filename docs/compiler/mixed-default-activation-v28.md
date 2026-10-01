# Mixed Compiler Default Activation

The mixed compiler is not yet the sole default pipeline. The source-owned
continuations are production APIs, but a prepared input, generated proof, or
successful optimization does not authorize publication or execution. This page
distinguishes implemented stage wiring from qualified default activation.

## Current Integration

The default target route in
[`production_pipeline.rs`](../../crates/rustc-codegen-fe2o3/src/production_pipeline.rs)
still selects the `Current` semantic importer. Its existing ranked, formal-memory
and publication continuation is not the nominal mixed pipeline. A failed mixed
transaction does not authorize retry through that older route.

The explicit source-owned Worker route in
[`production_pipeline_source_mixed_worker_v28.rs`](../../crates/rustc-codegen-fe2o3/src/production_pipeline_source_mixed_worker_v28.rs)
now requires this sequence:

```text
collector-authenticated Rust closure and compiler bindings
  -> nominal V35 semantic import and original semantic SSA
  -> canonical mixed KIR with original ABI, target and launch custody
  -> fixed Policy11 scalar fixed point
  -> checked V18 LICM with captured source layout limits
  -> checked cross-block store-consensus forwarding
  -> fresh native/source completion for the final forwarding output
  -> typed original-source / Policy11 / LICM / forwarding composition
  -> target lowering of that exact final graph
  -> original-root descriptor construction
  -> inert Worker input
```

Policy11 repeats these five passes, in order, through a complete unchanged
round: select-same-value canonicalization, integer-neutral worklist
canonicalization, local pure CSE, dominance-aware pure CSE, and DCE. It has a
32-round limit. Its execution record binds the complete pass history, graph
epochs and resource accounting; it is not an arbitrary caller-selected policy.
The implementation is in
[`fixed_policy_v3.rs`](../../crates/fe2o3-pliron/src/fixed_policy_v3.rs) and
[`fixed_mixed_fixedpoint_v18.rs`](../../crates/fe2o3-pliron/src/fixed_mixed_fixedpoint_v18.rs).

The forwarding transaction consumes the actual LICM output and retains a
distinct final owner, including for a legal no-op. Its independent replay
reconstructs all-path store consensus from the actual operations and CFG.
Native completion and the typed source proof remain separate obligations on
that output. Removing a load requires validity and totality at its original
operation gap, not just removal from an effect projection. See
[`production_source_mixed_store_consensus_v46.rs`](../../crates/fe2o3-lower-mir-kernel/src/production_source_mixed_store_consensus_v46.rs).

## Exact Program And Proof Boundary

`prepare_typed_source_tail_v50` composes the original source, scalar prefix,
motion and forwarding stages. Its subject retains four graph identities:
original canonical KIR, Policy11 output, LICM output, and final forwarding
output. It also binds original semantic MIR/SSA and the complete prefix
execution witness. A matching digest alone does not replace the actual owner
or its source provenance.

The V50 request, execution receipt and finalizer lineage are distinct from
the older V29/V36 proof interfaces. Final lineage includes the semantic MIR,
source SSA, four graphs, prefix execution, generated proof source and executed
receipt identities. The final capsule must name the forwarding graph. A
pre-motion or pre-forwarding proof cannot be relabeled as final evidence.

Prepared source text is not an executed proof. The Worker publication path must
still consume the admitted proof runtime, authenticated execution evidence,
protected compiler execution and strict finalizer checks. Inert content wrappers
do not grant Worker, publication, load or launch authority. KFD execution is not
a substitute for source-refinement proof execution, and LLVM remains a separate
downstream trust boundary.

## Remaining Activation Gates

1. **General source admission.** Complete original memory, borrow, move, call,
   enum and control-flow correspondence across the supported Rust subset.
   Unsupported forms must retain typed failures, not select a historical path.
2. **Composed qualification.** Execute the source, optimizer and final-native
   regression suites together. Check positive rewrites, legal no-ops, hostile
   substitutions, exact resource boundaries, selected errors and panic cleanup.
   Component tests alone do not qualify the composed pipeline.
3. **Proof execution.** Parse and discharge the generated concrete typed
   obligations with the admitted proof runtime. Source generation, structural
   replay and a signed result for a different subject do not satisfy this gate.
4. **Publication and runtime custody.** Qualify the final target, original ABI,
   descriptors, compiler execution and finalizer joins end to end. The runtime
   must discharge retained bounds, alias, access, initialization and launch
   conditions against the actual arguments.
5. **Default activation and retirement.** Switch the sole default only after
   the consuming joins and replacement tests pass. Remove redundant production
   projections and selectors without allowing an unoptimized or older-policy
   fallback. Historical schema names may remain for compatibility.
6. **Tutorial and target qualification.** Compile every compiler-produced
   manifest entry from ordinary Rust, compare supported simulator/reference
   results, and execute target-matched hardware gates where available. Parser
   tests, checked-in KIR, source-name dispatch and compile-only results do not
   replace these obligations.

## Evidence Classification

The actual-rustc parent tests exercise source construction, control flow,
references, enums, loops and publication custody on both target profiles. Their
presence is a test obligation, not evidence that they pass. Qualification must
record the exact source revision, test selection, binaries, toolchain and
terminal results. A failed positive admission test is not a successful
fail-closed test.

The milestone tracker is [issue #271](https://github.com/harsh-nod/fe2o3/issues/271).
Reviewed source changes, WIP publication, generated proof text, executed proofs,
default activation and tutorial/hardware qualification are separate states.
Advance the tutorial compiler pin only after the qualified commit is identical
on both public compiler main branches.
