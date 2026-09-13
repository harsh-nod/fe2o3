# Context Version Membership: V2 Contract

This is the frozen executable-model contract after R108 writer issuance.
R113 locally accepts its membership/Begin implementation above R112, with
[retained evidence](evidence/local-r113-context-version-membership-2026-09-13/README.md).
This is model acceptance, not formal or production acceptance; the production
runtime has no V2 consumer yet.

## Scope And Identity

Extend `runtime-model/src/context_version_journal.rs`, retaining all eleven V1
test names and their existing registration/lookup/abort semantics. The patch
implements allocation enrollment, exact lookup and complete whole-roster Begin.
Settlement, retirement, Context hooks, ordered writers and input reuse remain
V3 and later work. Model values are inert projections, not runtime authority.

Allocation keys contain the complete existing Context generation and local ID.
Device keys separately contain their complete Context generation and local ID.
Slot references also carry the complete key; slots, addresses and backend
handles are not identities. No second allocator or allocation watermark is
introduced. Enrollment replay rejects a duplicate enrolled allocation key;
there is no retirement/re-enrollment API in this patch.

V2 descriptors name complete allocation byte extents, not input-view ranges.
Canonical order is strictly increasing full allocation key. Device and extent
are exact validation fields, not extra ordering coordinates that allow aliases
to become distinct members. Alias canonicalization remains a separately bounded
O(n log n) preparation step, outside Begin and outside this patch.

## Storage And Transitions

Preallocate W writer entries/free slots and A allocation entries/free slots,
A membership entries/free slots and A planning scratch entries. Construction
cost is O(A + W). The initial profile admits one pending writer per allocation;
it is not a permanent restriction on ordinary ordered-write compatibility.

Enrollment creates epoch/lineage zero with no pending member. Zero means no
journaled writer, not initialized or readable bytes. It validates Context,
allocation ID, device Context/ID and positive extent, then duplicate identity,
available capacity and the chosen vacant slot, before mutation. A bounded O(A)
duplicate scan is permitted for enrollment and must not appear inside Begin.

Begin validates in this order:

1. Exact Reserved writer reference, including kind, then the Reserved-only
   counter needed for the transition. Newer registrations do not invalidate an
   older reservation; registration watermark is not a Begin eligibility test.
2. Canonical roster length within A and strictly increasing full allocation
   keys. Duplicate or unsorted keys reject before member preparation.
3. Each allocation's exact reference, exact device and extent, availability,
   and checked attempt-epoch increment, in roster order.
4. Sufficient membership and scratch capacity, and eligibility of all selected
   vacant member and scratch slots.

All these checks precede writes to scratch, arenas, free stacks and counters.
First/middle/last failure preserves their complete semantic state and storage
pointers/capacities. Test instrumentation counters are observations, not journal
state. Materialize the plan only after full validation and commit under the
same exclusive borrow, without allocation, callback, backend entry or remaining
fallible work. Clear the used scratch cells before returning.

Commit changes Reserved to Pending, decrements Reserved count exactly once,
retains W capacity and installs the complete immutable membership chain with
allocation backlinks. Each member binds exact writer/allocation references,
prior lineage, admitted epoch and next link. Begin burns one epoch per member;
it does not advance content lineage.

**Empty Begin succeeds** as a zero-member Pending writer. It consumes no member
capacity and changes no allocation epoch, but retains the writer slot. It cannot
subsequently be aborted as Reserved. Dropping a Copy reference releases nothing.

**Epoch MAX-1 to MAX succeeds.** Incrementing MAX rejects atomically. This is
deliberately distinct from Context/local IDs, for which MAX is unissuable.

## Invariants And Cost

Private transitions preserve unique enrolled keys, free/occupied partitions,
unique member ownership, exact cardinalities/backlinks and acyclic chains.
O(k) touched validation relies on these global preservation invariants; it does
not claim to detect arbitrary corruption of unrelated private arena storage.
In particular, free-slot uniqueness is a maintained invariant, not an O(A)
scan hidden inside every Begin.

Keep a full test-only auditor and an independent map/set reference model.
Count fixed-k work while increasing unrelated A/W. Preserve V1's indexed-access
counts of 4/1/3 for register/lookup/abort, and narrow its no-loop guard to those
operations and indexed helpers. Begin has separate O(k) traversal checks.
Constructor and full-auditor costs must not be reported as issuance costs.

Tests cover empty Pending, older reservations, stale/copied references,
cross-Context/key/device/extent substitutions, duplicate/unsorted rosters,
pending overlap, exact capacities, final epoch and first/middle/last rejection.
Boundary tests may seed valid epoch states; they are not evidence of executing
2^64 writes. Full invariants and every arena/scratch/free-stack snapshot must
be checked independently of the production transition.

No tests here authenticate the supplied existing IDs, establish that a roster
contains every kernel write, or prove production commit correspondence.
Authenticated proofs and actual Context integration remain separate gates.

## R113 Local Acceptance

Seventeen source gates and ten auxiliary checks pass. GNU/musl each pass 2,695
tests with five ignored across 48 libtest harnesses; the existing harnessless
benchmark is accounted separately. Seventeen compiled behavioral negatives
reject at exact named oracles, followed by restored journal/membership suites
with 23/12 passing. All 5,680 source identities are restored. The closed collector
and independent review verify the exact 344-artifact archive.

Nine runner and nine freeze tests pass. The corrected collector passes 52
contract tests, including exact benchmark target/schema accounting; original
preparation rejections and earlier helper identities are retained. This does
not qualify settlement, authenticated proofs, native execution, aggregate
memory, performance or production Context integration.

## Historical Preliminary Checks

The isolated candidate adds twelve membership tests while retaining all eleven
issuance tests. The 23-test journal suite and full GNU model suite (744 passing,
two ignored) pass, as do all-feature/all-target and production-only strict
Clippy. These are preliminary checks, not immutable packet qualification.

The integrated candidate additionally tests full-key ordering across Context
generations and direct Busy rejection with spare membership capacity. Its final
journal suite passes 23 tests and both strict lint configurations pass. The
preliminary 744-pass model run predates the final Busy-only test enhancement;
the accepted frozen full-workspace campaign validates the final source. The
isolated source and its preliminary artifacts remain separate historical evidence.

The membership differential model enumerates 29,282 four-action traces with
independent maps/sets and exact chain comparisons. Direct tests cover identity
edges, combined error precedence, selected private-slot corruption, final epochs,
full rejection snapshots and fixed-k indexed work. Structural source guards
cover Begin and its allocation/free/plan helpers; they are not a formal
complexity proof or allocator instrumentation. Constructor allocation-failure
injection and production correspondence remain outside this evidence.

The first runner attempt failed during Git inventory collection before spawning
the formatter. Its prior runner version and a labeled retrospective observation
are retained; raising the inventory output buffer allowed the formatter and
subsequent checks to run. No test deadline or runtime source changed for that
correction. The sparse-worktree runner distinguishes materialized source hashes
from unmaterialized Git index entries; it does not claim a full-workspace build.
