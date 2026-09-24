# Directed XGMI Shared Sources

The native diamond witness exposed a production admission gap: root writes A1,
then independent left and right copies read A1. Both depend on root, not on each
other. The old owner gate rejected right because left also retained A1. The
[original native refusal](evidence/dev-xgmi-directed-owner-refused-mi300x-2026-09-24/README.md)
is preserved, including its unsuccessful internal shutdown.

## Admission

The production scalar admission path now delegates its allocation-owner gate
to the directed provenance view. A present provisional root must match the
incoming route, extents, original event handles and resolved producer roster.
Every indexed selected owner must name an existing, matching active record that
uses that allocation. A present directed root must pass the existing retained
route/record check. Duplicate, stale, wrong-allocation and inconsistent roots
are terminal corruption, not retryable contention.

An earlier owner outside the explicit dependency roster is compatible only when
both roots name the allocation as their source with exact `Read` access and the
same home device. Writers still need dependencies. Legacy scalar and ordered
profiles receive no new sharing authority. `ReadWrite` source permission remains
admissible generally, but does not receive this new independent-reader exception.

Only the two selected allocation rosters are scanned, each bounded to 256 owners.
Coverage of the complete active map remains an invariant maintained by admission
and settlement; this helper does not detect arbitrary omitted index entries by
scanning the entire backlog. Detected corruption precedes Busy; a healthy hazard
precedes owner-count Capacity, preserving the original contention precedence.

## Mapping Publication

Admission retains every reader; it does not duplicate native mapping authority.
Scalar publication validates at most the first 63 FIFO entries and selects their
maximal allocation-disjoint prefix. A validated source/source collision ends the
prefix without skipping that entry. The rest of the bounded window is still
checked, so a later corrupt writer cannot be hidden by the healthy collision.
Unproven collisions and write-involving collisions are terminal inconsistencies.

The existing scalar selector can publish left, observe and restore its mappings,
then publish right on a later quantum. No sibling dependency is invented and no
second backend action is added to a single targeted observation. Pairwise work
is bounded to a 63-entry window; provenance validation consumes the existing
bounded dependency rosters. This is not a throughput or wall-clock guarantee.

Strict scalar flush keeps its complete-ready-set contract. Empty, in-flight and
over-63 classifications retain their previous precedence. A smaller disjoint
prefix returns pre-effect Busy before queue construction, mapping extraction or
native publication. It never reports success after publishing only a subset.

Exact aggregate admission similarly recognizes validated sibling readers as
healthy retained custody, validates its existing indexes and custody, then
returns nonterminal Busy for a non-disjoint complete roster. It does not shorten
the requested roster or take a mapping twice. Ordered singleton behavior remains
separate.

## Qualification Boundary

The regression fixtures call the production owner gate and publication selector;
the older directed-driver mock alone does not cover native owner admission.
Scripted mapping occupancy is not native mapping authority. Aggregate fixtures
exercise the actual selection/custody-decision helpers with scripted provenance.
The [CPU packet](evidence/dev-xgmi-shared-source-cpu-2026-09-24/README.md) passes
1,341 runtime tests on each GNU/musl target with twenty hardware-only ignores,
46 doctests, six example tests per target, fourteen Python tests and strict
static checks. All sixteen added regressions pass on each runtime target.
A fresh signed native diamond remains required. No formal refinement, native
fault, aggregate-memory or matched HIP/HSA performance claim follows.
