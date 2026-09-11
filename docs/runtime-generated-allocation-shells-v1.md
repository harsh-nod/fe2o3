# Generated Allocation Shells

R84 implements DATA-SHELL after the signed
[R83 unpublished lifecycle](runtime-unpublished-operation-lifecycle-v1.md).
It registers the complete original generated allocation roster and transfers
inert packet custody into the existing backend. It does not allocate native
storage, bind a lane, publish a kernel or produce typed completion.

## Source Ownership

`PreparedGfx942PersistentDispatchV1::packet()` keeps its existing public borrowed
interface. Consuming `into_generated_storage_v1()` moves its data and packet into
`GeneratedGfx942PersistentStorageV1`. The generated representation exposes only
immutable descriptions; its optional packet is private and may be transferred
once into a runtime-owned destination. Conversion does not allocate, encode or
copy buffers, shrink capacities or replace the payload with a dummy recipe.

Common data retains the original executable image, complete buffer vectors,
policies, fixups, digest and timeout. The projection now also owns an opaque
`Arc<()>` identity created once during projection construction. R80's descriptive
roster retains that same identity. Matching uses pointer identity plus all roster
coordinates, not equality of byte lengths or artifact digests alone. A different
but byte-identical source cannot replace the reserved source. This metadata
identity is neither a native handle nor Worker authority.

The mutable source view simultaneously borrows generated storage, the exact
HSACO and the existing authority. It reuses the original artifact, authority and
currentness checks. Legacy carrier implementations retain a default `None`
mutable view. There is no new unsafe authorizer or public control extraction.
The Context-bound carrier remains owner-local, including its decoder.

## Registration

The private Context entry point first checks the exact R83 hold, held stream's
device and absence of an existing generated registration. These pure failures
precede carrier callbacks and native currentness. Exact Context/backend and
native-admission checks, source validation and the closing retained-device check
must succeed before the metadata transaction starts.

The transaction reuses existing Context allocation IDs, backend handle IDs,
allocation maps, backend-handle membership and optional allocation admission.
It includes all original ordinals, including unused and read-only buffers.
It does not construct ordinary encoded `Arc<[u8]>` shadows or debit their
ordinary staged-byte budget.

1. Validate the bounded roster, checked identity ranges, descriptor lengths,
   stream/device binding and all handle collisions.
2. Pre-reserve Context and backend map capacity and the complete logical credit
   roster through the existing single-account batch operation.
3. Commit Context records and individual retained credits, then mark the held
   stream with the generated owner key.
4. Root the backend owner, insert its generated allocation entries, and move the
   inert packet directly into that owner. There is no callback, native effect or
   fallible allocation between these preflighted commit steps.

Ordinary preflight rejection leaves IDs, records, source control and credits
unchanged. Successful map-capacity growth may remain after a later rejection;
allocator high-water accounting is not established here. Successful registration
consumes monotonic IDs, which retirement never rewinds.

## Allocation Table

One backend allocation table has explicit ordinary and generated variants.
Generated entries contain only device, kind, alignment, full length, owner key
and original ordinal. They contain no byte snapshot or native allocation lease.
Ordinary lookup/iteration cannot fabricate a byte record for a generated entry;
ordinary removal cannot consume one. Duplicate insertion fails before replacing
an existing owner.

Ordinary records keep their existing inline layout. The explicit large-variant
lint exception avoids adding a heap allocation to their preflighted insertion
path. A generated slot consequently occupies the same inline table slot size;
the absence of an encoded-byte shadow does not imply zero metadata overhead.

Read, write, free, compute submission and same-device copy reject generated
targets before effects. Counts and emptiness include both variants, so existing
configuration/shutdown checks cannot silently ignore generated records. Native
coexistence and copy-drain qualification reject these unaccounted generated
rosters instead of claiming their ordinary-only observations are complete.

## Retirement

Metadata-only retirement requires the exact live hold and complete
Context/backend join. Before any disposal it validates the stored plan, bounded
count/tail, checked handle range, canonical ordinals/descriptors, unique logical
IDs, every Context and backend entry, and the absence of extra backend entries
for that owner. Corrupting a late member cannot cause a partially removed roster.

Retirement destroys inert packet custody and generated backend entries first,
then removes Context records and returns their logical allocation credits. The
hold remains installed until separately released. Release of a hold with a
generated registration still attached fails; replayed or foreign holds cannot
dispose or refund the current owner. Original encoded data, staged readback and
the private decoder remain with their existing carrier, not the backend shells.

## Bounds And Accounting

The full roster remains limited to sixteen entries. Existing Context allocation
and stream bounds constrain generated metadata; checked ID arithmetic rejects
exhaustion. Optional Context accounting reserves requested bytes and one record
per original buffer atomically. It does not reserve R80's readback or eventual
completion reply a second time.

The allocation table stores one entry per allocation. Metadata disposal scans
the table to detect extra owner entries and compares at most sixteen logical
IDs pairwise: O(table capacity + roster size squared), without another index.
This is an unpublished-retirement path, not a measured completion hot path.
It is not evidence of native-depth performance or an aggregate memory ceiling.

The new identity allocation, table capacities and other host metadata remain
within the explicit MEM-5 accounting exclusions. Logical requested bytes do not
represent padded native backing, queue/signal/kernarg storage, cache residency,
executable GTT, allocator overhead or aggregate quarantine.

## Evidence Boundary

Runtime tests exercise the actual inner metadata transaction with model-only
identity and synthetic source-authority fixtures. They cover substitution,
one-shot transfer, preflight and credit failures, ID exhaustion, ordinary API
isolation, corrupted retained rosters, exact retirement and no mock native
bootstrap. Positive tests do not bypass the outer entry point's genuine device
requirement; they make no claim about protected construction or Linux currentness.

Host tests use actual R73 charged storage and R80 readback owners. They check
unchanged full source/readback pointers, unused read-only storage, observer-drop
retention and payload-before-refund ordering on drop and unwind. These are not
the real charged carrier passing through successful protected construction or
runtime-native control transfer. Type tests keep control private and one-shot.

The [local evidence](evidence/local-r84-generated-shells-2026-09-11/README.md)
reports exact source gates separately from proof, Linux and performance cells.
No new Verus theorem or executable adapter refinement is claimed. Production
generated preparation still installs no adoption hooks; there is no public
generated submission or completion API in this packet.

## Native Handoff

The next lower-layer prerequisite is a bounded in-place dispatch-build owner
that retains unvisited original inputs, converted initialization authorities,
packet and code/kernarg prefixes. Existing consuming preparation wrappers do not
return complete typed failure custody. Initial/auxiliary constructors must then
retain bootstrap resources and successful insertion results before closing
currentness retake. Reuse the existing planner, allocator and queue constructors;
do not create a second native path. A CREATE_QUEUE-attempted failure remains
terminal unless an existing exact transition establishes otherwise; it cannot
manufacture a usable lane or pristine abort continuation.

DATA-ADOPT must replace inert shell custody with a distinct rooted native phase
before the first allocation, mapping or bind effect. Metadata-only retirement
must already be disabled, and backend Drop/terminal custody must include every
native prefix and lane. Reuse R82 pristine abort and R83 hold/drain ownership.
ISSUE, COMPLETE, public API and generated graph/drain integration follow. Those
transitions require independent tests, refinement and relevant Linux acceptance;
DATA-SHELL does not complete A1/A2, #182 or HIP/HSA parity.
