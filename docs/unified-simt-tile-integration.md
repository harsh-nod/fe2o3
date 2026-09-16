# ADR: Unified SIMT/Tile Production Integration

**Status: Proposed.** This is the missing integration review artifact for
[#275 M0](https://github.com/harsh-nod/fe2o3/issues/275), not M0 approval,
a schema allocation, executable tile support, or proof/publication/launch authority.

## Scope and ownership

Ordinary Rust kernels must combine SIMT work and structured tiles through the
existing production pipeline. The first execution tranche is authenticated context
entry, scoped workgroup derivation, masked u32 load, fragment extraction, and
ordinary parts returned to the logical root for scalar computation and checked
stores. Reduction follows separately; load/parts alone do not complete M1.
The staged source contract remains [generative tile source provider](generative-tile-source-provider.md).
The existing tile distribution and schedule dialects are not executable integration.

#275 owns integration and acceptance; #272 owns capability and call transport;
#271 owns the shared graph and analysis contracts; #134 owns scheduling/lowering;
#182 owns runtime lifecycle. This proposal creates no implementation lease.
No historical capability WIP is imported wholesale.

## One executable graph

Authenticate source through the existing semantic MIR/SSA. Retain executable
structured operations in the sole canonical KIR graph until schedule selection,
then lower them on that graph to existing guarded memory, scalar and collective
operations. Do not maintain a competing executable tile or verification program.
Replay must bind the semantic contract, selected schedule, exact transformed
graph and source lineage. Unsupported transformations or exhausted checks reject.

## Context and authority

Carry sealed producer evidence into semantic ownership: physical root, logical
helper, nominal kernel, target/launch brands, and source-to-optimized issuance flow.
Four DefIds, matching layouts, names or geometry alone cannot grant authority.
An authenticated entry plan may select the proven logical helper as the kernel
body, seed only its leading logical context, and identity-forward every physical
argument. The host supplies no context kernarg; ordinary wrapper inference is unchanged.

Retain original issuer and helper-call occurrences in a private move-only compiler
receipt through `AuthenticatedCollectedKernelClosureV1`, then consume it before
semantic identity inventory/preflight. Independently authenticate optimized flow;
original and optimized block/local ordinals need not agree. A logical `ContextIssue`
represents the one physical issuance, not a second issuer. Preserve physical export
and launch identities. Source spans are diagnostics, not authority keys.

Preserve invariant lifetimes, brand and epoch through borrowing, callbacks, SSA
edges and calls. `&mut KernelContext` is a real reference, and `WorkgroupCapability`
is not an ignored ZST. Workgroup derivation binds exact producer and call/SSA
occurrences; equal nominal types do not make separate issuances interchangeable.
`load_masked`, `into_fragment` and `into_parts` remain distinct operations.
Proposed `WorkgroupDerive` exclusively acquires the actual mutable context borrow.
Proposed `ScopeEnd` in that same canonical CFG closes the exact scope/epoch on every
admitted callback exit before another derive. It invalidates branded descendants
and releases the borrow, but is not a memory barrier. Ordinary returned values
survive. Reject overlapping scopes, escaping authority, stale epochs, capability
joins and active-scope backedges. Ordinary joins require matching lifecycle states.
Known terminal traps need scope end before the trap; unsupported unwind/drop/tail
call and other exceptional exits reject. Shared tile receiver borrows remain short.

Choose checked generic callback materialization into the sole graph, retaining
caller/callee, argument/result and original call-path/access lineage. Account for
every moved operation and preserve each load's original effect position. The
current earlier effectful-helper refusal needs owner-approved staging before this
transformation can run; disabling that refusal is not the staging solution.

`into_parts` consumes one exact fragment and yields E u32 values, then E bool
masks. Component j maps to `Field(0)/ArrayElement(j)` or
`Field(1)/ArrayElement(j)` respectively. Reuse the bounded aggregate SSA binder to
reconstruct `([u32; E], [bool; E])`; this alone needs no canonical tuple type.
Check producer/component/consumption lineage beyond matching types and counts.
Same-typed swaps, partial/double consumption and substituted producers reject.
Parts confer no authority. New context/workgroup/tile/fragment roles must be
explicitly non-storable; casts, constants and ordinary memory cannot reconstruct
them. Compiler-order effects must prohibit invalid duplication/CSE even when the
operation has no physical memory effect.

## Algorithm, schedule and arithmetic

The algorithm defines logical contributions, masks and numerical behavior.
Schedule choice is not part of source carrier types. For `L` lanes and `E`
components per lane, the existing distribution contracts are:

- Blocked: logical offset = `lane * E + component`.
- Striped: logical offset = `component * L + lane`.

Each schedule must establish a bijection over the same logical contribution set.
Initial carrier bounds remain `1 <= L <= 256`, `1 <= E <= 125`. Selection is
immutable and bound to owner correspondence and replay; substitution rejects.
Per-lane results may differ. Arbitrary lane-observing code is not automatically
equivalent across schedules, even when both mappings are legal.

Selection is an external immutable compilation input, not a source carrier
parameter or mutable environment lookup. Seal it against semantic/SSA identity,
complete root/launch roster, authenticated target, exact structured canonical input
and covered tile occurrences before scalarization. Retain selection and transform
policy in owner correspondence/replay. Structured operations enter the graph
before applying the checked schedule. Missing, duplicate, foreign or stale bindings
reject, even if substituted schedules happen to produce equal outputs. Existing
coordinate queries and fixed scalar/CFG policies do not authorize this new tile
transition. Its policy and replay contract require #271/#134 approval. Test each
distribution against its own independent lane oracle.

Use the declared machine-integer width for addresses. Checked base-plus-offset
overflow or an out-of-bounds index produces zero with an inactive mask and no read.
Preserve the load's original effect position; fragment extraction cannot move it.
Require uniform arrival, input identity and base, initial `[L, 1, 1]` geometry, and
checked allocation/extent/index/guard correspondence. Affine conclusions require
justified bounds/no-wrap facts or exact supported modular reasoning.

## First mixed reduction

Proposed contract: wrapping-u32 transform followed by modulo-2^32 workgroup sum.
Each logical contribution is `mask ? transform(value) : 0`; transforming an
inactive zero must not accidentally contribute a nonzero value. The same source
and contribution set must work under both distributions, including partial tiles.
Reduction initially requires exact `[L, 1, 1]` geometry and power-of-two `L <= 256`.
Other lane counts remain distribution contracts, not admitted reductions.

Propose compiler-private scoped scratch branded to workgroup issuance and the
reduction occurrence, with exact capacity and checked synchronization phases.
Reuse the existing target-neutral reduction recipe only after authenticating its
LDS effects and barriers. Scratch addresses, views and reuse tokens cannot escape;
any reuse needs an explicit checked completion barrier. This restricted operation
does not require a public scratch/epoch-reuse API, but internal epochs still need
#272 approval and verification. No stale view may cross an invalidating transition.
Do not bridge through legacy `WorkgroupCollectives::current()` or unbranded `DynamicLds`.
Final stores remain in the logical root and require existing checked output authority.

## Call and effect dependencies

Consume a released, reviewed lowerer-owned canonical call/source adapter over
existing `with_checked_call_v1` and checked `physical(slot)` views. Reuse
`RankedProjectionSourceV1::owner()`, the exact inventory, root/function-qualified
coordinates and one cumulative work/storage ledger. Retain complete call rosters,
repeated occurrences, argument substitution, source paths and result correspondence.
Released checkpoint 219 supplies that adapter at
`6a7b6dcecce4d9fa68801d82ccb87bbca67142c2`; it is integration evidence, not launch
authority. Its callback-budget extension is deferred until needed.

The adapter alone admits neither shared-slice reads nor LDS writes/barriers.
Read admission still requires exact allocation/view/extent/index/guard transport
and complete ranked/formal coverage. Reduction additionally requires owner-approved
same-graph scoped materialization or complete call-qualified LDS-write/barrier
coverage. Removing helper-purity or proof checks is not an implementation strategy.

## Decisions required before implementation

The scope, callback and schedule choices above are proposed integration contracts,
not allocated operations or implemented execution. Private entry-receipt custody
does not authorize logical-root selection, a new role, or a new accepted program.

- #271/#272: coordinated semantic-MIR and canonical-KIR operation/type,
  verification and encoding contracts. Their version spaces are independent;
  exact allocations remain owner decisions, not one shared numeric version.
- #272: sealed producer ownership, logical-root selection, capability borrowing,
  epoch and convergence rules.
- #271/#134/#272: immutable schedule representation, transformation/source lineage,
  replay and bounded resource accounting on the shared graph.
- #272/#134: scoped scratch policy and complete reduction-effect handling.
- #275: consume exact released adapter/read dependencies and freeze their source,
  simulator, verifier, generated-host and target-specific acceptance commands.

## Qualification matrix

| Boundary | Required positive evidence | Required negative evidence |
| --- | --- | --- |
| Source authority | Genuine entry, callback, generic cross-crate helper | Forged/reused issuer, substituted root/helper/brand, escaped capability |
| Structured transport | Load/fragment/parts with exact SSA/call lineage | Role erosion, wrong epoch, reconstructed authority, stale occurrence |
| Addressing | Empty inputs, tails, maximal base; overflow means no read | Wrong carrier, index, allocation or guard |
| Scheduling | Same source, both schedules, complete contribution coverage | Substitution, duplicate/missing logical elements |
| Reduction | Wrapping transform/sum, multiple groups, bounded scratch | Divergence, nonuniform base, invalid geometry, stale scratch, conflicting store |
| Verification | Roundtrip, exact final-graph correspondence, bounded replay | Missing effects/barriers, foreign owner, insufficient resources |
| CPU execution | Independent Rust oracle, canaries, both target models | No fabricated GPU state or performance prediction |
| Host/GPU | Generated safe host, admitted artifact, direct KFD | Context kernarg, wrong target/launch, unsupported artifact |

M1 requires ordinary-source execution through the production compiler and existing
simulator, not hand-built KIR alone. M2 additionally requires equivalent mixed-kernel
results under both schedules, target-matched direct-KFD hardware qualification,
and an exact-source tutorial separating CPU and GPU evidence. gfx950 simulation
on MI300X is not gfx950 hardware qualification. Later families extend this contract
through their owners; the initial u32 family does not qualify the whole curriculum.
