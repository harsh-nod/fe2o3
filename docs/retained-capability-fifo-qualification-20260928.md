# Retained original-order capability FIFO — 2026-09-28

A private, inert capability-propagation component now retains its entry table,
reachability flags, transfer scratch, FIFO queue, successor scratch and visit
trace in one pending owner. The queue preserves duplicate visits and original
entry order. It is not a topological traversal or a coalescing worklist.

Original semantic work charges remain separate from the new resource ledger.
Every raw edge is charged before deduplication; successors are sorted and all
merge charges occur before successor mutation. Existing-value changes enqueue
a successor even when no key is added. Original clone/state/aggregate limits
and transfer/error order are preserved within the supported bounded shape.

All fallible growth is prepaid and remains attached on error or panic.
Completed data requires the original lexical function and budget-ledger
binding with no sticky denial. Occupied retries do not move data or add debits.
The enclosing owner destroys retained vectors before refunding accepted
credits; the component itself never refunds or creates a replacement budget.

The component adds 32 wrapper-work units and 31 conservative typed accounting
rows. Independent review corrected an edge visitor row to use the actual Copy
edge and block-ID value types, rather than a borrowed edge. This is logical
source-owned accounting, not a native-stack, allocator-capacity or RSS bound.

Qualification passed all 11 new controls, 356 model tests, 3,326 backend tests
(197 ignored), 227 serial authority/capability tests, nine approval-policy
integration tests and backend/extractor builds. Controls cover full original
data and FIFO traces, duplicate joins, nonnumeric entry, original semantic
limits, exact/one-short resource limits, every insufficient work prefix,
partial state on callback error/panic, occupied retry and wrong-source/ledger
refusal. The trace oracle is a reversible trace-only adaptation of the original
HashMap/VecDeque algorithm; positive results also match the untouched original
API. Expected states are not derived from the candidate.

Full regression receipt:
`7edc2a419cd60b0410a54fe3abc392375188663666533dc0499dca5ee5a0ea56`.
Independent review:
`365635c25e7300854902bb5ff2550b0645900393f2badc8abb30bea12568e2df`.

This checkpoint is a single generic pass, not the complete capability driver.
Its synthetic consumer does not perform an authentic nominal query. Arbitrary
transfer captures, heap owners and callback behavior require accounting and
custody in the future closed caller. The empty pipeline payload map represents
only the original initial pass; owner discovery, actual payload analysis,
repeated propagation, final source-order replay and the genuine outer join
remain separate work.

No ordinary compilation route, native debugger capture or GPU execution is
enabled by this component. Earlier constants, argument writers, joint bounds,
genuine nonempty Fixed coverage and production admission remain open.
Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).
