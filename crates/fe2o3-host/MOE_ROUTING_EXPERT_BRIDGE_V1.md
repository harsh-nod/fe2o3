# Exact MoE routing-to-expert bridge V1

## Scope

This bridge covers only `T8/E4/K2/C4`, with 16 token-major routes and the
`u32::MAX` drop sentinel. It closes one host-side consistency gap: expert
preparation can no longer combine an independently supplied offset array with
an unrelated inverse-routing device view.

The input is still an untrusted host observation. The bridge does not claim
that the routing kernel ran, that its output completed, or that device bytes
were read back. It grants no compiler, artifact, HSA copy, load, dispatch, or
GPU authority.

## Linear boundary

1. `MoeRoutingOutputCandidateV1` contains caller-supplied router-shaped bytes.
   It is data, not evidence.
2. `check_host_observed_moe_routing_output_v1` validates the complete fixed
   relation and returns an opaque, non-`Clone` checked witness.
3. `upload_checked_moe_routing_expert_bridge_v1` consumes that witness and
   synchronously uploads both its offsets and inverse arrays.
4. The returned bridge retains immutable views of both exact device regions.
   `GeneratedMoeExpertV1HostAdapterV1::prepare` consumes this single bridge and
   has no separate inverse-routing argument.
5. Expert preparation still terminates at `deny_moe_expert_execution_v1`.

Safe callers cannot splice checked arrays, replay a checked witness, or mutate
either uploaded destination while the retained bridge is alive.

## Checked relation

The checker validates:

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
GPU routing execution.
