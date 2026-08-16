# Exact MoE routing-to-expert bridge V1 and V2

## Scope

This bridge covers only `T8/E4/K2/C4`, with 16 token-major routes and the
`u32::MAX` drop sentinel. It closes one host-side consistency gap: expert
preparation can no longer combine an independently supplied offset array with
an unrelated inverse-routing device view.

The input is still an untrusted host observation. The bridge does not claim
that the routing kernel ran, that its output completed, or that device bytes
were read back. It grants no compiler, artifact, HSA copy, load, dispatch, or
GPU authority.

## Checked boundary

1. `MoeRoutingOutputCandidateV1` contains caller-supplied router-shaped bytes.
   It is data, not evidence.
2. `check_host_observed_moe_routing_output_v1` validates the complete fixed
   internal relation and returns an opaque, non-`Clone` checked witness.
3. `upload_checked_moe_routing_expert_bridge_v1` consumes that witness and
   synchronously uploads both its offsets and inverse arrays.
4. The returned bridge retains immutable views of both exact device regions.
   `GeneratedMoeExpertV1HostAdapterV1::prepare` consumes this single bridge and
   has no separate inverse-routing argument.
5. Expert preparation still terminates at `deny_moe_expert_execution_v1`.

Safe callers cannot splice arrays within a checked witness or mutate either
uploaded destination while the retained bridge is alive. This is not replay or
freshness protection: callers can reconstruct and recheck an equivalent public
candidate. A future authority-bearing bridge must obtain freshness from an
opaque router-completion/readback receipt.

## Checked relation

Conditioned on the caller-supplied top-2 expert IDs, the checker validates:

- two distinct in-range experts for each of eight tokens;
- exact requested counts and `min(requested, 4)` admitted counts;
- capacity-bounded admitted counts;
- zero-based monotone offsets equal to the exclusive admitted-count scan;
- the exact compact-plan relation for four 256-element expert tiles and one
  256-element compact output;
- stable-prefix route slots, including dropped-route sentinels;
- uniqueness and bounds of every accepted slot;
- exact accepted permutation prefix and sentinel tail;
- route/slot/permutation/inverse round trips.

The payload SHA-256 is domain-separated and canonically commits all seven
arrays with field names and lengths. It does not include pointers, context,
stream, allocation, or region identities. Those operational identities are
recorded as separate bridge fields and diagnostics.

## Failure and trust boundary

Validation completes before either upload begins. Both pinned host sources are
allocated before copying. A failure of the second synchronous HIP copy may
leave the first destination updated, but no bridge witness is issued. The
function therefore provides a checked retained handoff, not transactional
device-memory rollback.

External mutation through unsafe native APIs remains outside the safe Rust
contract. A future end-to-end bridge must add authenticated router completion
and typed device-to-host observation before these bytes can be attributed to a
GPU routing execution. It must also bind top-2 selection to logits and tie
policy, and connect route weights and packed activation construction.

## Completed V2 boundary

V1 remains source-compatible and denial-only. It continues to accept a checked
caller observation through `GeneratedMoeExpertV1HostAdapterV1`; it has not been
reclassified as completed GPU evidence.

The production-shaped completed contract is versioned separately as V2:

1. `MoeRoutingOutputCandidateV2` owns the complete untrusted routing shape. It
   converts through the unchanged public V1 candidate constructor and checker;
   V2 neither accesses nor widens any V1 field.
2. `MoeRoutingExpertBatchIdentityV2` is an opaque, move-only request identity.
   It commits routing-request identity, logits identity, exact token
   activations, caller route-weight policy, and model/expert-weight artifact
   identity.
3. `MoeRoutingCompletionReadbackProvenanceV2` retains exact typed dispatch and
   readback `ContextIdentity` and `StreamIdentity` values, dispatch identity,
   completion event identity, readback operation and event identities, an
   explicit completion-before-readback ordering identity, exact profile, the
   complete seven-field observation mask, and routing-payload digest.
4. `check_completed_moe_routing_readback_v2` consumes that provenance and
   rejects context, stream, profile, event, ordering, batch, payload, partial
   observation, and transcript substitutions before V1 routing consistency is
   considered.
5. `bind_completed_moe_routing_expert_inputs_v2` requires the token activation
   and route-weight policy digests from the shared batch and verifies the exact
   zero-padded expert-major activation layout. Route weights remain caller
   policy inputs; they are not authenticated router outputs.
6. `GeneratedMoeExpertV2HostAdapterV2` lives in its own V2 module and accepts
   only the completed V2 bridge and
   `MoeExpertWeightArtifactBindingV2`. It cannot accept a raw expert-weight
   device view, preventing unrelated weights from being attached after routing.

There is no public constructor or production issuer for the V2 batch,
provenance, or expert-weight binding. There is also no feature-gated issuer:
Cargo feature unification, including `hardware-test-hooks`, cannot expose a
production-shaped MoE token. Positive transcript logic is exercised only by
private `cfg(test)` fact models, which have no conversion into production
provenance. Safe production code therefore fails closed before it can construct
a completed V2 adapter.

## V2 identity limits

V2 requires exact typed context and stream equality at readback and upload.
Every typed runtime identity is also covered by one domain-separated,
process-local V2 transcript. Because `ContextIdentity` and `StreamIdentity`
hide their sequence fields, the transcript uses their Rust `Hash`
implementations with a pinned-toolchain encoder. This is not stable or durable
serialization across compiler or standard-library versions and must not be
persisted or compared across process restarts.

Move-only ownership prevents safe reuse of one issued value. It is not global
replay protection. A future production issuer must consume one unique router
completion/readback lifecycle and maintain anti-replay state; reminting and
process restart remain unresolved until that issuer exists.

V2 proves no routing semantics, logits-to-top-2 correctness, compiler
refinement, expert GEMM/combine behavior, general memory safety, or race
freedom. It grants no compiler, finalizer, artifact, copy, load, dispatch, or
expert execution authority. The gfx942 hardware test remains limited to the
genuine V1 host upload/readback and denial path; there is no claimed completed
hardware path without a real router dispatch and authenticated readback.
