# Ordinary Coherent Host Backing V1

## Scope

MEM-N1A adds optional session-local admission for **ordinary non-userptr
`HostVisibleCoherentGttV1` allocations only**. The isolated adapter and model are
part of the R72 implementation packet, along with native lifetime hooks and
both runtime startup paths. The
[R72 local gate record](evidence/local-r72-host-backing-2026-09-10/README.md)
retains the actual results. Signed hardware acceptance remains separate; this
document is not an execution or performance certificate.

`Gfx942HostVisibleBackingBudgetV1::new(bytes, records)` accepts positive limits
of at most 8 GiB and 256 allocations. The byte maximum is a chosen numerical
ceiling matching the existing session envelope, not a general VA-residency
claim. One allocation remains bounded by the canonical 2-GiB CPU span limit.
An unconfigured account preserves the existing native profile guards.

The account covers every allocation of the exact ordinary coherent profile,
including ordinary coherent queue/bootstrap objects. It is not restricted to
application payloads. Consequently a small budget can reject native setup before
the first payload is allocated. AQL, userptr, kernarg and executable profiles,
additional anonymous queue shadows, MMIO mappings, metadata and session bootstrap
outside this profile remain unaccounted by this particular ceiling.

## Cost Projection

The native owner supplies its actual private `SharedGttAllocationLayoutV1`.
The adapter compares the complete layout, including profile and flags, with
`profile_layout::<HostVisibleCoherentGttV1>(requested_bytes)`. Checking only the
profile enum would not exclude all userptr control layouts.

For this exact ordinary profile, the page-rounded `cpu_mapping_bytes` equals
the size passed to the native allocation operation. CPU and GPU views refer to
that same backing. The R72 projection checks all of the following:

- Positive requested size and a CPU span covering it.
- Page alignment, less than one page of padding, and at most 2 GiB per object.
- GPU VA extent equal to the ordinary CPU span.
- Exactly `ResidentHostAllocationBytes = cpu_mapping_bytes` and
  `AllocationRecords = 1`; the other seventeen R67 coordinates remain zero.

No caller-reported cost is accepted. The GPU VA extent is a consistency input,
not an additional host or device byte debit. Existing `retained_gpu_va_bytes`
checks remain separate VA-capacity guards. Requested logical credits, N2 device
backing and runtime shadow copies are distinct quantities; none is evidence
that this native backing has been disposed.

## Private Ownership API

`shared_memory/host_resource_accounting.rs` contains the adapter:

| Value | Operations And Boundary |
| --- | --- |
| `Gfx942HostVisibleBackingBudgetV1` | Immutable positive byte/record limits with read-only getters; not authority |
| `Gfx942HostVisibleBackingUsageV1` | Inert used bytes/records and reserved/retained/quarantined/poisoned observations |
| `HostBackingAccountV1` | Native-parent-private construction, exact domain matching, reserve and usage, reusing `fe2o3-resource-accounting` |
| `HostBackingReservationV1` | Move-only unissued reservation; Drop cancels; `retain()` enters retained ownership |
| `HostBackingChargeV1` | Move-only native-record charge; inert exact matching and consuming `release_after_disposal()`; Drop quarantines |

The account binds the actual nonzero session ID, device generation and VM. The
native generic GTT path can read the account's private device/VM coordinates,
which originate only from actual-session configuration; each operation supplies
the engine's session ID independently. Tokens additionally bind allocation ID,
native generation and the complete canonical layout. Exact account identity is
also checked using the private `Arc` domain, so an equal-valued replacement
account cannot release another account's charge.

The adapter does not add a duplicate allocation registry. The native record table
continues to own allocation uniqueness, current phase, mappings and handles.
Native and pool generations are distinct: ordinary native incarnation identity
does not change merely because a buffer is recycled or its logical extent is
resized. A native phase/view/ownership transition must retain the same charge.

## Native Handoff

Configuration must bind the actual fresh session before the first covered native
attempt and queue certification. Restoring model ownership or successfully
releasing all buffers must not reopen it. Runtime constructor forwarding must
occur before native setup allocations, in both startup orders; a late Context
setter cannot retroactively qualify already-created backing.

For a covered allocation, validate the complete cost and reserve its vector and
owner record before `reserve_va` or any allocation/mapping operation. Ordinary
pure rejection or failure before native entry cancels only the unissued
reservation. Retain immediately before the first native effect. After that
point, failures and unwind before insertion into `SharedAllocationRecord` still
preserve the complete charge through the shared ledger's quarantine anchor.

After insertion, the exact native record owns the retained charge through CPU
and GPU mapping, host writes, initialization, model loans, queue ownership,
pool checkout/recycle and native teardown. Before disposal, independently check
the record's actual identity/layout and charge match. Full ordinary disposal
requires the existing successful CPU unmap, backing free, VA release, closing
currentness and checked retained-byte accounting before consuming the charge.
Partial disposal or an uncertain result must retain it and prohibit continued
native use. A dropped retained token alone never authorizes a refund.

Confirmed disposal is the refund boundary, not queue-facade success. A later
queue-model retake failure may terminalize the facade after backing has already
been disposed and its debit returned. That later failure must not resurrect an
already-disposed charge. Conversely a lower occupancy count, observer drop,
logical release, drain or session teardown does not prove backing disposal.

The account's `poisoned` and `quarantined_records` observations must both feed
the native owner's fail-closed phase checks. Retained-credit Drop can quarantine
without setting the ledger's separate corruption-poison bit. Unwind containment
must cover every covered native transition, not just allocate/free. No new native
cleanup or retry is implied by catching and resuming the original panic.

## Evidence And Remaining Work

The isolated adapter supplies nine focused tests: budget/domain rejection,
page-padding and unissued rollback, byte/record failure atomicity, full
profile/layout exclusions, exact incarnation/account matching, independent
member disposal, retained ownership after account-owner drop and invalid-refund
quarantine. Four model tests cover boundary extents, page-rounding relationships,
all vector coordinates and R67 reserve/release arithmetic.

Eighteen native fixtures cover first-effect ordering, allocation/disposal error
and panic boundaries, pre-record retention, original callback panic identity,
mapped closing-currentness panic, nineteen signal-access ingress cases, exact
token/account substitution, both foundation-loan orders and buffer generation/
logical-extent reuse. Bootstrap tests use the production completion-arena size
and ring/control/ordinary-arena profile order, including independent byte and
record saturation. Failed-retake tests distinguish an explicitly dropped live
token from backing already conclusively disposed and refunded.

These fixtures execute actual private accounts, records, model ownership and
move-only buffer transitions with a fake backend. They do not construct Linux
queues, initialize the signal ABI or execute the Linux pool facade. The retake
test explicitly composes a rejection with the existing facade's quarantine
policy rather than claiming to exercise that facade. Eight runtime tests cover
default/immutable configuration, all six host/device/pool configuration orders,
closed history, terminal non-recovery and source forwarding. Actual Linux
constructor failure, retained terminal-memory usage and signed hardware
saturation/reuse remain separate acceptance gates.

Configured ordinary-host access panics quarantine and resume the original
payload without another fallible currentness check. Nonpanic paths retain
their existing checks; unconfigured and excluded profiles retain their original
bodies. This packet does not widen the existing contracts for arbitrary queue
facade observations or Linux outer-retake panics.

`r72_host_visible_backing_credits_v1.rs` is a property-specific executable
projection companion with reviewed Rust correspondence. Rust's fixed-divisor
`is_multiple_of(4096)` corresponds to the companion's remainder check. The
proof covers acceptance of the bounded ordinary span, exact host-byte/record
coordinates, zero other coordinates and algebraic reserve/release conservation.
Nine targeted negative sources remove positive-size, size-bound, logical-to-
backing, single-backing, record-count, coverage, minimal-padding, alignment or
ordinary-view constraints. The authenticated runner must confirm the actual
positive count and each named expected failure.

Neither R72 nor R67 proves actual profile/layout extraction, native outcomes,
private record/token correspondence, mutex/arena ownership or the whole executor.
Other GTT profiles require MEM-N1B. Complete queue/executable plans and exact
adoption of pre-reserved member credits require MEM-3/4; low-level allocations
must not independently charge those already-reserved backing bytes again.
Parent domains, cross-account transactions, account-arena/bootstrap bytes,
global limits and aggregate quarantine remain MEM-DOM-1/MEM-5 obligations.
