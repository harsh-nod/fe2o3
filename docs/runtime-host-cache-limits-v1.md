# Ordinary Host Cache Limits

R81 implements MEM-2B-HOST on the existing ordinary coherent GTT SDMA pool.
It does not change default behavior or extend the admitted native profiles.

## Configuration

`Gfx942HostPoolLimitsV1::new(bytes, records)` accepts cached-free padded bytes
through 8 GiB and records through 256. Either zero disables caching, not native
allocation. These bounds are independent of the optional R72 host-backing
budget and R71 device-cache limits. They do not establish aggregate process,
parent-account, executable, userptr, control or checked-out memory bounds.

`KfdRuntimeBackendV1::configure_host_pool_limits_v1` is immutable and precedes
logical/native resource creation. Both startup paths retain the new queue before
forwarding the limits. Native `configure_sdma_host_pool_v1` may follow compute
queue certification but must precede every SDMA resource attempt. Host and device
cache policies share the existing irreversible SDMA activity latch, including
failed enable, allocation, recycle, checkout, trim and release attempts.

`host_pool_usage_v1` and native `sdma_host_pool_usage_v1` are inert observations.
`None` means unavailable or unconfigured, not zero native usage or a refund.
Configured terminal or missing-owner observations fail closed. Observation does
not check live device currentness, drive progress, or authorize disposal.

## Retained Storage

The native projection checks the exact session, allocation ID, generation,
canonical ordinary layout and mapped phase. CPU mapping, GPU handle and VA
reservation must remain present; attempted free, userptr and quarantined records
are rejected. The actual retained session supplies device/VM identity. Even an
empty roster validates the configured account's session/device/VM binding.
Optional N1 account and charge must both be absent or exactly match the record.

Occupancy uses the native record's page-padded `cpu_mapping_bytes`, not the
buffer's logical or SDMA `physical_bytes` extent. For example, a 4097-byte Host
allocation on a 4096-byte page counts as 8192 cached backing bytes and one record.

The complete Host roster is scanned with fixed-capacity stack metadata. IDs
must be distinct regardless of pool/native generation. Mixed-roster shape and
host/device record counts are bounded; device costs are excluded from Host
occupancy. Exact Device record validation remains its own configured policy.
The duplicate scan is O(H^2), bounded at 256 Host records, with no heap allocation
or native calls. Sums are checked. Invalid existing rosters or candidates are
errors, not cache misses or permission to dispose a substituted token.

Cache admission leaves the existing N1 debit unchanged. Checkout reduces idle
occupancy, not native residency. Recycle advances the existing pool generation
and preserves best-fit-by-kind/size/alignment checkout. There is no new account,
background eviction, or allocation retry policy.

## Disposal

Cache pressure passes the incoming idle buffer to `release_sdma_buffer`.
Successful disposal is successful recycle, not a recovered rejection. The
existing unmap/free/release and queue-loan/retake envelope owns native effects.
The charge is released only after confirmed backing disposal. Errors and panics
retain uncertain backing and terminalize the existing owner. Later retake
failure cannot resurrect a debit already refunded after confirmed disposal.
Checkout and trim validate configured Host and Device rosters before mutation.

## Acceptance

The [local evidence](evidence/local-r81-host-cache-2026-09-10/README.md) separates
production policy/fake-native-record tests from source-only Linux forwarding
checks. Tests cover padding, ceilings, aliases, exact account/token substitutions,
all configuration orders, failed history, reuse, both foundation-loan orders,
partial disposal, native/currentness errors and panics, and failed retake.

R72's cost projection and existing credit types are reused without changes.
No new Verus theorem or whole-cache-adapter refinement is claimed. Host roster
scans, native extraction, mutation and disposal composition still need proof
acceptance. Live Linux pressure/reuse/disposal in both startup orders requires
the separate MEM-QUAL-HARNESS. No HIP/HSA performance claim follows from CPU tests.
