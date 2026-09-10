# Single-Account Resource Batch Admission

MEM-TXN-1 extends the existing shared credit engine. It is a prerequisite for
compound native-resource admission, not an implementation of MEM-3, hierarchical
budgets, physical cost extraction or global quarantine closure. Existing scalar
admission and token disposal semantics remain unchanged.

## Admission API

`ResourceCreditAccountV1::reserve_batch(&[ResourceVectorV1])` returns a boxed
slice of independent `ResourceReservationV1` members. The roster must be
nonempty and contain at most 65,536 members, also bounded by the account's
existing owner-record arena. Every member consumes one record even if its
nineteen-dimensional cost vector is zero. Empty/oversized rosters use the
existing `InvalidRecordCapacity` error; the public error enum is unchanged.

The method allocates its returned boxed slice with empty private token slots
before locking for admission. It also preallocates a bitset for validating the
selected native-independent ledger slots. Under the account mutex it:

1. Runs the production `r70_resource_batch_reserve_v1` preflight over every
   member vector, available record count and complete nonwrapping owner interval.
2. Checks aggregate record counters and every selected free slot for exact
   vacancy, bounds and uniqueness.
3. Installs all member records/tokens and the complete new usage/owner state.

The commit performs no allocation, callback or fallible operation. Output
boxing, including any shrink, occurs before mutation. Error paths drop the
mutex guard before destroying the preallocated empty owners. Ordinary capacity,
record exhaustion and generation exhaustion preserve usage, records, free-slot
order and the next owner. Corrupt zero-owner, counter or free-slot state instead
poisons the account without issuing any member or advancing the owner interval.
That distinction matches the existing scalar engine's fail-closed behavior.

For `m` members and `r` account records, runtime work is
`O(19 * m + ceil(r / 64))`; slot checking is not quadratic. Output storage is
`m * size_of::<ResourceReservationV1>()`; temporary bitset storage is
`8 * ceil(r / 64)` bytes before allocator overhead. The existing account arena,
output allocation, bitset, allocator rounding and metadata are not charged by
the caller's vectors. These are explicitly bounded local implementation costs,
not measured aggregate MEM-5 residency or a performance result.

## Member Ownership

Returned members are the existing move-only reservation type, not a new debit
family. A caller may consume the boxed slice into a vector/iterator without
duplicating its members. Dropping an unissued member cancels only that member.
Retaining it transitions its exact record to retained custody. Definite
rejection or established disposal returns only that member's charge; abandoning
a retained member quarantines it while independent siblings may be disposed.
Scalar and batch callers share one mutex, one usage vector, one free-slot arena
and one nonwrapping owner counter.

Native adapters must retain every potentially affected member before their
first effectful operation. They must store each token privately with its actual
resource and establish disposal for that resource before refund. The low-level
credit API does not establish native effects or supply a disposal certificate.
It does not divide an already-issued charge, move custody between accounts or
provide an atomic transaction over parent/device/Context accounts.

The next compound-native consumer must supply a complete actual-layout roster
and establish construction/unwind/disposal correspondence. Landing this shared
primitive alone does not close that consumer's acceptance gate.

## Native Forwarding Handoff

MEM-2A-FWD separately forwards optional immutable N2 limits to the actual fresh
native session. Configuration must precede first N2 materialization in
compute-first startup, and queue transfer/certification in SDMA-first startup.
Existing constructors without configuration preserve their defaults. An early
runtime configuration interface must reject any logical/native resource history;
it must not become a late native setter or weaken session freshness.

The exact session/device/VM account survives ownership transfer, loan/retake,
pool checkout/recycle and uncertain disposal. A logical Context release may
return requested-allocation credits while backing enters the native free pool;
the N2 resident debit must remain. Successful physical backing and VA disposal
permits its refund. Device N2 accounting does not cover host GTT pools,
queue/control bootstrap, root budgets or aggregate quarantine. MEM-N1 supplies
host backing, MEM-3 supplies control/slot integration, and MEM-DOM-1 supplies
the still-missing parent/bootstrap domain contract.

## Evidence And Proof Scope

Fourteen new shared-core tests cover whole-roster state snapshots, final-member
and final-dimension rejection, arithmetic overflow, record/generation exhaustion,
internal corruption poisoning, independent disposal, retained-member quarantine,
account identity and concurrent scalar/batch callers. Six model tests include
a wider-integer boundary oracle. One compile-fail example rejects cloning an
individual returned member. Test outcomes belong to the current-source release
record, not this implementation inventory.

The R70 Verus source projects classified model errors to rejection and proves
the executable checked-vector step and complete roster scan against bounded
prefix sums and owner intervals. Supporting properties cover all required
records, distinct member owners, late-member rejection and exact-member refund
algebra. Nine named negative fixtures mutate the roster bound, final member,
last dimension, aggregate overflow, record count, owner interval, owner
assignment or refunded sibling.

The Rust/Verus correspondence is reviewed, not mechanically linked. The proof
does not establish mutex linearization, bitset/arena extraction, Box allocation
success, actual token issuance, native cost/disposal observations, the constructor
handoff, parent accounting or whole-executor refinement. R67 remains the
separate record-transition primitive. The authenticated runner must confirm
all positive obligations and exact named negative failures before publication.

The [R70 release record](evidence/local-r70-batch-forwarding-2026-09-10/README.md)
retains the passing current-source GNU/musl, lint, dependency and full
authenticated proof gates, alongside earlier failed attempts. It does not
qualify a compound native consumer or report a performance measurement.
