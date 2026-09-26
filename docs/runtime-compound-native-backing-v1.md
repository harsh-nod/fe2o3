# Compound Native Backing V1

This MEM-DOM development composes ordinary coherent host backing (N1) and
device-local backing (N2). It is not complete MEM-5 closure, native
qualification, a hierarchy proof or HIP/HSA parity. The
[N1-only API](runtime-native-root-admission-v1.md) remains a separate profile.

## Accounts And Limits

`Gfx942NativeBackingRootV1` owns one canonical checked-device UID/PCI registry
and one fixed-arena coordinator. Its hierarchy is root, canonical device,
native session, then separate N1 and N2 class leaves. The explicit
`new_root_with_class_domains_v1` accounting profile permits these four levels;
the existing generic `new_root` still permits only three.

`Gfx942NativeBackingDeviceBudgetV1` sets independent host and device backing
byte ceilings and a combined allocation-record ceiling. The session budget
contains both existing typed class budgets and a mandatory combined record
ceiling. Each allocation consumes one global record; its class byte coordinate
and record count are reflected at every ancestor atomically. Class limits and usage
remain authoritative; sharing a session does not merge their observations.
Unused child capacity is not reserved against ancestors.

The existing class envelopes are unchanged: N1 permits at most 8 GiB and 256
records, N2 at most 192 GiB and 128 records. Larger aggregate budgets do not
enlarge either native class. Root, device, session and class usage are inclusive
views of the same charges, not additive physical allocations.

Domain capacity must include root, every registered device, every retained
session and both class leaves. One device/session needs five nodes; two sessions
on one device need eight; two devices with one session each need nine. The
constructor requires at least `max_devices + 4` slots, not capacity for every
possible concurrent session. Registry metadata consumes one generic record
but zero `AllocationRecords` units. `bootstrap_bytes_v1` covers the generic
arena and registry slot payload, not Arc headers, wrappers or allocator overhead.

## Admission And Native Custody

Only a retained `CheckedGfx942XnackMinusDevice` can mint a public admission.
The registry reuses an immutable parent for the same UID/PCI across model
generations and rejects conflicting aliases or changed limits. The move-only
admission binds the exact checked generation, session and both class budgets.
New session/class creation completes before a new device parent is published.
Ordinary rejected admission releases unused children; accounting poison
conservatively retains domain state. Successful canonical registrations remain
permanent for the root's lifetime. UID/PCI/limit conflicts do not themselves
poison the root. Registry-mutex poison prevents new registration; generic
coordinator poison seals accounting transitions across issued accounts.

Native intake checks binding before VM identity/attempt creation. Adapter
installation checks both pristine-configuration guards, constructs both local
adapters, checks native currentness, then installs both without a fallible step
between assignments. Neither partial installation nor local fallback is allowed.
Accounting admission does not replace opener-PID/currentness, launch or disposal
authority.

Actual native allocators reserve their existing exact padded backing cost
before currentness/VA/native effects. The original debit survives mappings,
queue foundation transfer/loan/retake and pool retagging. Either class's uncertain
disposal closes the shared session's allocation paths. A retained N2 charge now
anchors the same typed registry as N1 before generic credit quarantine; failed
identity refund, accounting error and unwind do not disarm that anchor. Only
confirmed disposal refunds, and final clean owner destruction releases the root.

## Runtime Routes

Single-device runtime constructors
`open_default_with_native_backing_root_v1` and
`from_checked_device_with_native_backing_root_v1` require a typed root and
device/session budgets. SDMA-first, ordinary compute-first and generated adoption
consume the same compound admission. Rooted policy remains explicit after token
consumption; missing or mismatched admission is terminal, not a request to use
legacy independent accounts. Both local budget replacements reject.

The XGMI constructors `open_default_with_native_backing_root_v1` and
`from_checked_pair_with_native_backing_root_v1` use the same root with ordered
device/session budget arrays. Topology preflight precedes admission, and both
endpoint admissions complete before either VM acquisition. Second-admission
failure drops the first unused session/classes; it does not remove an already
registered canonical device. First-VM failure drops the unused second admission.
The existing abort boundary for failure/panic after the first VM is acquired
is unchanged.

`native_backing_usage_v1` reports the retained session's inclusive usage through
queue, primary teardown or terminal memory ownership. XGMI returns observations
in original endpoint argument order. Snapshots grant no execution or refund
authority. Legacy constructors retain their existing behavior.

## Verification Boundary

The [development evidence](evidence/dev-compound-native-backing-2026-09-26/README.md)
separates generic tests, production adapters with FakeBackend, runtime policy
tests and source-routing checks. It does not fabricate a checked Linux device
or substitute mocks for hardware qualification. Existing single-VM-attempt per
GPU restrictions remain; sibling model sessions do not establish native
same-GPU multi-Context concurrency.

Compound account creation does not reserve the complete VM/queue/bootstrap
allocation roster before effects. It is not an R70 cross-class native creation
transaction. Logical Context request accounts, executable/kernarg/AQL/userptr
profiles, image and scaled-table accounts, complete bootstrap/terminal headroom,
batch output and other metadata still require integration. Independent roots
and legacy constructors remain available, so this is not a process-global cap.

R67/R70 reuse does not prove exact ancestor traversal, canonical registration,
native layout/disposal correspondence or bootstrap refinement. Those proofs,
signed native replay, broader multi-device execution, physical overlap and
matched HIP/HSA performance remain open. Accepted milestones are unchanged.

## Next Logical Composition

The [exact retained-credit query](evidence/dev-retained-charge-2026-09-26/README.md)
is an implemented prerequisite, not installation of a logical/native profile.
The next explicit profile must use one session with sibling request, N1 and N2
leaves. Context must adopt the already-minted request leaf; creating another
child would exceed the four-level hierarchy.

New composed device/session budgets must directly describe requested, host and
device byte ceilings plus an explicit combined-record ceiling. Do not reinterpret
existing native-only `max_allocations` as including logical requests. Registry
metadata consumes a generic record but no AllocationRecords units; native cached
and bootstrap backing remains charged independently of logical requests.

All three leaves must be minted inside canonical device admission before its
registry entry is published. The composed binding must retain the exact typed
root/registry, session and request leaf, including cold request quarantine before
any native backing exists. A generic accounting-root anchor alone does not retain
that typed registry. Unused admission must remain cleanly reclaimable.

Context installation after exact device enumeration and a borrowed allocation
witness must be mandatory for this profile. Direct backend calls and backends
returned by shutdown must not bypass it. Ordinary allocation and generated
rosters both need coverage, followed by every startup order and ordered XGMI
endpoint admission. Internal queue/bootstrap backing must not invent logical
request charges. Capacity failures before effects remain distinct from binding,
generation or accounting-invariant failures; ambiguity must not refund custody.
These interfaces and native qualification are not implemented by the query alone.
