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
For a composed session this observation includes request, N1 and N2 usage;
it is neither native-only residency nor a logical-allocation count.

## Verification Boundary

The [development evidence](evidence/dev-compound-native-backing-2026-09-26/README.md)
separates generic tests, production adapters with FakeBackend, runtime policy
tests and source-routing checks. It does not fabricate a checked Linux device
or substitute mocks for hardware qualification. Existing single-VM-attempt per
GPU restrictions remain; sibling model sessions do not establish native
same-GPU multi-Context concurrency.

Compound account creation does not reserve the complete VM/queue/bootstrap
allocation roster before effects. It is not an R70 cross-class native creation
transaction. Logical Context request accounts are separate from the native-only
profile; the composed single-device integration is described below.
Executable/kernarg/AQL/userptr profiles, image and scaled-table accounts, complete
bootstrap/terminal headroom, batch output and other metadata still require
integration. Independent roots
and legacy constructors remain available, so this is not a process-global cap.

R67/R70 reuse does not prove exact ancestor traversal, canonical registration,
native layout/disposal correspondence or bootstrap refinement. Those proofs,
signed native replay, broader multi-device execution, physical overlap and
matched HIP/HSA performance remain open. Accepted milestones are unchanged.

## Next Logical Composition

The [exact retained-credit query](evidence/dev-retained-charge-2026-09-26/README.md)
and [typed composed minting](evidence/dev-composed-request-2026-09-26/README.md)
are implemented prerequisites for the logical/native profile.
The latter adds a separate root and budgets with one session and sibling request,
N1 and N2 leaves. Typed request reservations and retained credits preserve the
canonical registry even before any native allocation exists. Clean cancellation
or release remains reclaimable; quarantine preserves registry custody and debit.
The owning batch iterator reuses the generic token array. Native-only extraction
and raw-account/token extraction are unavailable.

The lower KFD constructors
`acquire_shared_gtt_memory_session_with_composed_backing_v1` and
`create_compute_aql_queue_with_composed_backing_v1` now consume the full admission.
They check device identity and observed session health before the VM attempt,
then validate binding/currentness/health again before installing all three
accounts together. Healthy reserved or retained requests are allowed at intake.
No class is installed on failure; an installation panic quarantines the engine
and preserves the original panic. The engine retains the typed request account
alongside native accounts and observes inclusive session quarantine at checked
operation boundaries. An unrelated session retains spare capacity and remains
active. A health snapshot is not atomic exclusion against another request clone
quarantining during an already-started operation.

The [lower-intake evidence](evidence/dev-composed-native-intake-2026-09-26/README.md)
uses FakeBackend and queue-foundation/pool fixtures plus constructor source
checks. It does not qualify Linux intake, actual shutdown, hardware behavior or
formal adapter composition. These constructors charge native backing without
inventing logical requests; they do not enforce a per-allocation witness.
Context now adopts the existing typed request account, since another child
would exceed the four-level hierarchy. The single-device Runtime constructors
and mandatory Context/backend witness transport are implemented as described
below, including ordered composed XGMI intake. Native replay remains open.

New composed device/session budgets must directly describe requested, host and
device byte ceilings plus an explicit combined-record ceiling. Do not reinterpret
existing native-only `max_allocations` as including logical requests. Registry
metadata consumes a generic record but no AllocationRecords units; native cached
and bootstrap backing remains charged independently of logical requests.

All three leaves are minted inside canonical device admission before its
registry entry is published. The composed binding retains the exact typed
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
Generic ledger quarantine retains ancestor pressure without globally poisoning
spare-capacity siblings; native session sealing must be enforced by its consumer.
The mandatory single-device witness interfaces are implemented. Native and
formal composition qualification remain open.

### Single-Device Runtime Integration

The [Runtime witness packet](evidence/dev-runtime-request-witness-2026-09-26/README.md)
qualifies CPU accounting/Context behavior and distinguishes isolated fixtures
from shipped native authority. The following interfaces are implemented:

- `resource_credits.rs`: preserve typed KFD request accounts, reservations and
  retained credits in private variants, including an owning batch iterator.
  Do not extract generic tokens or allocate a second output array after commit.
- `Context::open_configured_v1`: validate a complete backend-device-keyed
  mandatory roster against enumeration before exposing the Context. Reject
  missing, extra and duplicate entries; preserve exact Context device brands
  and full native model admissions. Later local/domain configuration must not
  replace those accounts. Per-device optional hooks alone cannot prove coverage.
- Ordinary allocation: privately construct a non-Clone witness for the exact
  retained credit, account, device and full requested extent. Pass the witness
  by value while Context keeps ownership of its credit. A default backend hook
  must return explicit unsupported status, not invoke a legacy method with the
  witness ignored. Unsupported must not require constructing generic
  `Self::Error`. Preserve current rejection/settlement/quarantine outcomes.
- `native_budget.rs` and all SDMA-first, compute-first and generated-first
  switches: retain the typed request account independently of one-shot native
  admission. Both old KFD allocation entry points must reject a mandatory
  profile before identity advancement, staging or native work, including on a
  backend returned by shutdown. Worker protocols without witness transport
  cannot advertise the profile.
- Generated shells are logical requests. Finish structural preflight, atomically
  reserve their whole request roster, then retain and authenticate exact order/cardinality,
  device/account/extent before committing identities, journal or backend tables.
  A private witness-bound plan must reject unbound materialization. Later DATA
  adoption charges N1/N2, not a second request. Shell-only and adopted retirement
  must each release the original request exactly once after disposal.
The complete roster also rejects two backend keys sharing the exact request
leaf. Witness creation checks the private Context-device brand; backend matching
checks the independently retained exact leaf, full model admission and extent.
The backend keeps its mandatory policy after consuming native admission and
after Context shutdown. Missing or inconsistent native/request policy seals
startup rather than falling back to local accounting.

Generated registration retains the whole request roster before commit. Clean
pre-commit binder/journal rejection refunds it. A panic quarantines retained
custody; this is not atomic rollback under corrupt accounting. A move-only
authenticated plan gates metadata commit. Its stored marker is registration
evidence, not ongoing native-layer authentication of individual Context tokens.
The Context rechecks exact credits before generated adoption/issue/retirement.
Later native DATA backing reuses those logical requests without charging them
again. Session-health checks are snapshots, not concurrent quarantine exclusion.

### Multi-Device Runtime Integration

The [multi-device request packet](evidence/dev-runtime-multi-request-2026-09-26/README.md)
extends the composed profile to `KfdMultiDeviceRuntimeBackendV1`, with production
and semantic-authority constructors. Each input tuple binds a unique device ID,
authority, device budget and session budget. The new composed constructor
validates the complete ID roster and reserves checked-device, child and routing
storage first; every checked device precedes any root session admission. Later
failure drops unused sessions, not canonical device registrations. This is not
a complete bootstrap-allocation preflight.

Composition accepts only all-Legacy or all-Required children. Required account
leaves are distinct and cover the complete immutable device index. Profile
discovery uses persistent bindings, including after partial or complete child
shutdown; it never omits unhealthy or retired children to form a smaller roster.
Witnessed allocation authenticates the selected child's exact binding before
routing storage, ID advancement or child effects. Legacy allocation cannot
bypass the required policy.

Both allocation APIs share a preflighted route transaction. Only a successful
nonzero child allocation commits the outer ID and route. Unsupported, rejection,
generic quiescence, settled-no-owner and terminal outcomes preserve their exact
diagnostics and do not consume an outer ID. A child-effect panic seals both
owners and resumes its original payload. Same-child local handle uniqueness
remains enforced by the concrete child's allocation table, without a linear
scan over live routes. Release removes the route only after child disposal.

CPU fixtures cover actual Context/trait witness routing and typed account/root
custody, not checked-device minting or native constructor execution. Generated
shell APIs remain single-device; native XGMI is a separate backend.

### Native XGMI Runtime Integration

The [XGMI request packet](evidence/dev-runtime-xgmi-request-2026-09-26/README.md)
extends the separate two-endpoint backend with composed-root constructors.
Both root admissions and full device/bundle/request binding checks precede
either VM acquisition. Lower native account installation remains fallible;
the existing second-acquisition fail-stop rule is unchanged. Original endpoint
order is preserved even when device IDs are not sorted.

Required policy retains both distinct typed request leaves independently of
the consumed native admissions. Complete profile discovery rejects an unhealthy
endpoint instead of truncating the roster. Each allocation checks only its
selected endpoint's exact witness before storage reservation, handle advancement
or native effects. Direct witness-free allocation cannot bypass the profile.
Clean shutdown preserves inert Required-profile visibility without permitting
native operations.

Both allocation APIs share the same preflighted record transaction. Native
capacity rejection retains its classification and the existing consumed-handle
behavior. Other lower failures and panics seal the backend; no inferred
settled-no-owner outcome refunds uncertain custody. Successful lease custody
is indexed immediately into a pre-reserved vacant record.

CPU tests execute the production request policy and record transaction, with
typed accounting/Context fixtures through a synthetic backend. They do not
execute the checked-device admission bridge, public successful constructors,
real native trait path, peer mapping, shutdown or kernel/copy engines. Source
wiring checks do not replace those execution gates. No new formal refinement
or matched performance result is claimed.

### Worker Server-Local Request Ownership

The [Worker request-owner packet](evidence/dev-worker-request-owner-2026-09-26/README.md)
adds `RuntimeWorkerRequestOwnerV1` and opt-in `serve_runtime_request_owner_v1`,
`v4` and `v5` entry points. A private real Context validates the complete Required
roster and owns allocation credits. Its existing allocate/release state machine
mints borrowed witnesses locally; no ledger account or native authority is sent
over the wire. A pre-reserved raw-handle index connects wire handles to that
Context's allocation records without adding a post-native allocation step.

Other worker operations remain backend-owned. After complete decoding, every
allocation reference in read/write, ordinary launch, peer copy, async copy,
atomic and collective requests must belong to this owner's index. Enumeration
uses the admitted immutable roster. Existing unscoped serving APIs and V1/V4/V5
wire encodings remain unchanged; V1 retains its immediate-progress trait bound.

Success is rooted before response encoding/writing. I/O or protocol failure,
panic, and terminal responses seal the owner and quarantine all its local
credits without disposal. Release removes its index entry only after Context
confirms disposal, even when the successful response is later lost. Backend
transfer requires empty local records and healthy Required sessions, not zero
aggregate usage from other healthy account holders. Empty shutdown frames do
not imply disposal, native shutdown or quiescence. Directly dispatched streams,
modules and submissions are not inventoried by the private allocation Context.

Generic quiescence still quarantines locally. Allocation-specific settled-no-owner
can refund locally but keeps the same generic Quiescent wire tag; client-side
credit remains conservative. This does not transport a parent Context witness,
join host/worker roots, add replay epochs, or establish Worker V3 compiler authority.
The owning constructor preserves existing Context panic/Drop behavior, while
serving borrows an externally retained owner and preserves it across unwind.

### Remaining Integration Gates

- Composed XGMI and multi-device checked construction, partial-admission cleanup,
  selected-endpoint forwarding and native startup/shutdown require hardware
  qualification. CPU policy and structural checks are not native acceptance.
- Worker proxies still do not transport borrowed request witnesses. The explicit
  request-owner servers support a worker-local Required ledger, not transparent
  parent-profile transport. Old unwrapped servers still fail closed. Deployment,
  native child-process failure replay, cross-process authority and Worker V3
  compiler/application refinement remain open.
- Real checked-device constructor replay, all three native startup orders,
  generated DATA adoption/disposal, pool reuse, failure injection and shutdown
  still require native qualification. CPU fixtures cannot establish these.
- Production Context/backend/ledger composition refinement, concurrent sealing,
  complete memory closure and matched HIP/HSA performance remain unproved.
