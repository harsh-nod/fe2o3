# Device Cached-Free Policy V1

## Scope

MEM-2B adds an opt-in device-only cached-free ceiling, not another resident-memory
account. `Gfx942DevicePoolLimitsV1::new(bytes, buffers)` accepts `0..=192 GiB` and
`0..=128` buffers. Either zero limit disables device caching. An unconfigured
`None` retains the legacy policy; zero is not a synonym for `None`.

The immutable native configuration is installed before SDMA activity/history.
Runtime forwarding must preserve its earlier backend configuration boundary and
the actual queue/session owner. Configuration cannot authorize a foreign queue,
replace an existing policy, or reopen a terminal native session.

`Gfx942DevicePoolUsageV1` reports configured limits and the sum/count of cached
device allocations only. It is inert data, not a token or disposal witness.
Host-visible GTT buffers, checked-out device buffers, and native allocations
retained after ambiguous disposal are not in that cache observation. Consequently
an occupancy decrease is not evidence that resident memory or N2 credits fell.

## Cost And Ownership

The native adapter scans the complete mixed cache roster, bounded by 256 GTT plus
128 device allocation records. It validates queue ownership, nonzero pool
generations and nonempty valid logical buffer extents, and projects every device
entry. Host entries consume no device-cache byte or record capacity; this is not
an audit of their native GTT allocation records or a host-residency ceiling.

For each device entry and incoming device buffer, the native session projection
validates the exact mapped lease, device/VM and live native allocation record,
including its generation and canonical layout. An optional N2 account must agree
with the record's retained charge; an entirely unconfigured `(None, None)` pair
is also valid. A mixed or mismatched account/charge pair is rejected. Empty cache
observations still require the concrete session/domain validation.

The projected cost is `Gfx942DeviceMemoryLayoutV1::backing_bytes()`, not logical
payload bytes and not the SDMA facade's `physical_bytes()` accessor, which can
represent the lease's requested extent. Native record extraction is the source
of the cost; no public caller-supplied cost or success boolean is accepted.

The policy checks the bounded device roster's domain, nonzero allocation/native
and pool generations, positive bounded costs, and distinct allocation IDs.
Different generations never make the same allocation ID a distinct cache entry.
The roster must already fit both configured limits. An invalid or over-limit
existing roster is an error, not an ordinary capacity miss.

## Decisions And Disposal

For a valid, distinct incoming device buffer:

- `Cache` requires the existing padded-byte sum plus incoming backing bytes and
  the existing device count plus one to fit both ceilings.
- `Dispose` is the ordinary capacity-miss result, including either zero ceiling.
  It requests the existing native release path; it is not a disposal authority.
- Invalid candidate identity/layout/shape or aliasing is rejected before cache
  mutation or native release. Invalid existing state remains fail-closed.

The policy never increments/decrements a second byte counter and never moves or
releases an N2 credit. Cache insertion, checkout, and logical reuse preserve the
same native allocation and its retained N2 debit. A pool generation change does
not replace the native allocation generation. Actual N2 refund still requires
the existing native disposal sequence, VA disposal and closing currentness check.
Failure, uncertainty or unwind before confirmed native disposal must retain the
charge and terminal custody even when the buffer no longer appears in the
cached-free roster. A later queue-model retake failure can still terminalize the
queue after full disposal and closing currentness have refunded the native debit;
it must not resurrect a debit for backing already disposed.

There is no implicit eviction-on-allocation-failure or new retry policy. A cache
miss still uses the native allocation path and optional N2 admission. Existing
explicit trim/release paths remain responsible for disposal. Host recycling and
the unconfigured legacy path do not acquire a device policy by inference.

## Implementation And Evidence

- `crates/fe2o3-kfd/src/sdma/pool_policy.rs`: immutable public values, concrete
  native projection wrappers, complete fixed-capacity roster extraction, fixed
  private errors and cache/dispose mapping. No heap allocation or native syscall
  occurs in the policy itself.
- `crates/fe2o3-runtime-model/src/r71_device_pool.rs`: production-used bounded
  domain/identity/extent checks, checked prefix sums and exact capacity decision.
- `crates/fe2o3-runtime-model/verus/r71_device_pool_v1.rs`: executable companion
  scans with reviewed Rust correspondence, whole-roster policy acceptance and
  returned sum/count, overflow exclusion under closed bounds, exact cache versus
  disposal condition, stale-ID alias rejection, and zero-limit behavior.
- Ten targeted negative sources remove logical-to-backing distinction, final
  record coverage, generation-insensitive identity, domain or pool-generation
  guards, byte/count limits, zero-disable semantics, roster bounds, or bounded
  checked arithmetic. Each must fail its named obligation under the authenticated
  runner. These mutations are policy counterexamples, not native vulnerability
  reproductions or physical-memory execution proofs.

The source tests include six model tests and five policy-adapter tests. Synthetic
adapter projections exercise roster mechanics only. Shared-memory fake-engine
tests cover actual private record/account/charge projection and disposal failure
paths; queue/runtime forwarding tests belong to those respective owners. The
primary integration gate records actual test/proof results separately. This
document does not treat unrun sources, model inputs or test fixtures as evidence
of Linux/GPU execution.

With `h <= 256` host and `d <= 128` device cached entries, the policy uses bounded
stack storage for at most 128 projections. Including lookup among `n <= 128`
native device records, a usage call performs `O(h + d^2 + d*n)` work. A recycle
call projects one additional candidate and repeats the model roster check for
fixed-error classification: `O(h + d^2 + (d+1)*n)`. Its two roster-alias scans plus
candidate scan perform at most `d^2 <= 16384` ID comparisons; up to 129 native
lookups each have a bounded fallback scan of 128 records. The indexed native
lookup ordinarily avoids that fallback. Configured N2 phase observations also
take an account mutex per projection; this is not a lock-free path. No throughput
or parity claim follows from these bounds.

R71 does not verify native extraction, locks, cache mutation, actual physical
nonaliasing, queue authority, N2 retained-charge ownership, native disposal, GPU
execution or the entire runtime. It does not add parent/device hierarchy,
cross-account transactions, GTT admission, executable/control budgets, pool
metadata/bootstrap accounting, global ceilings or aggregate quarantine closure.
Those MEM-N1/MEM-DOM/MEM-3/4/5 obligations remain separate.
