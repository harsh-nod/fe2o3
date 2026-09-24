# Native XGMI Backing-Budget Witness

Status: [native qualification passes on MI300X GPUs 5/6](evidence/dev-xgmi-backing-budget-native-mi300x-2026-09-24/README.md).
Both pressure/retry paths, both copies, all 36,875 checked bytes, final zero
accounts and separate owned cleanup pass. The preceding
[CPU qualification](evidence/dev-xgmi-backing-budget-witness-cpu-2026-09-24/README.md)
passes five example tests per GNU/musl target, twenty Python tests without skips,
formatting, strict Clippy and unchanged-input checks. Native faults, aggregate
memory, refinement and performance remain outside this result.

The example `gfx942-runtime-xgmi-backing-budget-smoke` exercises the
[endpoint backing budgets](runtime-xgmi-backing-budgets-v1.md) through the public
Context and native two-device XGMI backend. It adds no production API. Native
execution requires its separate source-bound campaign; source presence and CPU
tests alone do not qualify hardware, formal refinement or HIP/HSA performance.

## Exact Workload

The ordered endpoint limits deliberately differ:

| Account | Endpoint 0 bytes / records | Endpoint 1 bytes / records |
| --- | --- | --- |
| Device backing (N2) | 8,192 / 3 | 24,576 / 2 |
| Ordinary coherent GTT (N1) | 4,096 / 1 | 8,192 / 2 |
| Context requested allocations | 1,048,576 / 16 | 1,048,576 / 16 |

The version journal admits sixteen allocations and sixteen writer records.
Three 4,097-byte device allocations have homes `[0, 1, 1]`. Each consumes 8,192
padded backing bytes. An additional one-byte allocation on each endpoint must
return exact backend Capacity, leaving a live Context and unchanged backing,
request-credit and journal snapshots. Endpoint 0 has record headroom but lacks
bytes; endpoint 1 has byte headroom but lacks records. These dimensions are
inferred from exact arithmetic, not distinguished by the common Capacity tag.

One owner on each endpoint is released. The original one-byte request must then
succeed with a fresh Context allocation identity, round-trip `0x5a`, and release.
A new 4,097-byte owner restores the original layout. Context identities, not
native handles or addresses, are checked for non-reuse.

Two dependency-free directed copies use destination-owned streams:

| Copy | Source offset | Destination offset | Bytes |
| --- | --- | --- | --- |
| Allocation 0 -> 1 | 32 | 17 | 1,024 |
| Allocation 2 -> 0 | 64 | 2,048 | 1,024 |

The first progress action for each sole ready root must return Pending after
publication. Its retained-mapping snapshot checks independently supplied endpoint
IDs, exact budgets, device charges, coherent charges and healthy custody. After
completion the same device charges must remain. Peer mappings must not double
charge N2. N1 progresses from `[0, 0]` to `[4096, 0]` to `[4096, 4096]` as each
source direction creates its cached completion word. This is host-side retained
publication evidence, not measurement of physical engine timing or overlap.

All three complete buffers are checked after pressure and after each copy.
Expected bytes come from original seeds, never earlier readback. Including the
two single-byte retries, the oracle checks exactly 36,875 bytes. It covers
payload, guards and unchanged sources, not allocation padding outside the
requested 4,097-byte range.

## Cleanup And Campaign

After stream and allocation release, requested and device credits must be zero.
Logical Context shutdown must preserve both cached coherent completion charges;
native shutdown must reduce both accounts to zero without poison or quarantine.
Every workload Result failure still attempts explicit cleanup. Cleanup failure
cannot replace the original error; uncertain native owners are retained until
process teardown, never forced free by the witness.

One absolute 60-second deadline starts at workload entry and is checked before
copy publication and each progress action. Construction, synchronous setup,
readback and shutdown remain bounded by the outer 300-second process limit,
not by an assertion of preemptible native calls. Timeout is not quiescence.

`benchmarks/runtime_gfx942/xgmi_backing_budget_campaign.py` builds a signed clean
checkpoint as a default-feature release musl example. It binds fixed controls,
the complete local source inventory and exact executable. The strict parser
requires a single exact receipt with ordered endpoint identities and explicit
false reservation, performance, refinement and aggregate-bound claims.

Local helpers are digest-checked before import; a remote bootstrap verifies the
marker-bound binding and native program bytes before executing the entrypoint.
Git checks use an absolute executable, scrubbed environment and disabled replace
refs, and require ancestry from the CPU-qualified budget implementation with no
production-source changes beyond this example. The parsed `runtime_cleanup`
result describes Context/native teardown only. Remote `owned_cleanup` comes
separately from the controller's collection and absence checks.

The sibling campaign reuses pinned observers and owned-process controls without
changing historical campaigns. Both GPUs must pass fresh preflight plus fixed
two-second settled and twenty-second delayed observations. Failed observations
are retained, never replaced by later idle samples. Byte-exact collection must
precede marker-owned cleanup and independent path/process absence checks.

The campaign does not inject native faults, exhaust physical device memory,
qualify general multi-GPU compute, or establish aggregate process memory bounds.
Even a passing native witness leaves those gates, A1/A2 and full parity open.
