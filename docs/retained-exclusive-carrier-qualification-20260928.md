# Retained ExclusiveOwner carrier component — 2026-09-28

The private carrier-analysis component retains the origin table, all three scan
arrays, nested copy edges, borrowed-receiver pairs and exact-origin FIFO queue.
Previously completed arrays and partial allocations remain attached when a
later operation fails. The original return APIs remain unchanged; this component
does not yet provide a genuine authenticated continuation or production route.

Whole original carrier and scan-initialization algorithms are preserved with
explicit ownership adaptations. Borrow pairs are visited without consuming their
owning vector. The original ABI no-exclusive-owner path, definition-shape errors,
copy/move filtering, receiver rules, alias/write/escape exclusions, local census,
projection-spine charges and propagation order remain intact.

The component adds a 32-work-unit prefix and 22 logical accounting rows.
If propagation is reached, the separately qualified retained FIFO adds its own
prefix and frame. The accounting explicitly covers both caller vector references
and actual coerced slice references. Existing frame policies remain separately
charged; these are not native stack, allocator or RSS measurements.

All 12 new controls passed, including independent original data/debit comparison,
nonexclusive ABIs, mutations and aliases, receiver positions, missing callables,
exact local census and resource boundaries, partial scan initialization,
success/error/panic custody, detached inputs and wrong ledgers, and long
projection/copy graphs. Full qualification passed 356 model and 3,245 backend
tests (197 ignored), plus backend and extractor builds.

Full regression receipt:
`8bb0690261e027e1f9eae076674e565eaeeca4e28c2622a983e5f80b4ee51d6c`.

Independent source review:
`71564b717d1f6cb82d15eb6dec33aef35b24e289e565233bc0e0d62e28fbb8a0`.

The completed view has lexical source/ledger consistency checks, not durable
source authority. A later factory must retain the original immutable source
loans and all pending owners across checked postflight, destroy them, then refund
accepted credits. No new genuine-source hook or GPU execution was run here.
Local provenance, allocation/capability preparation, genuine continuation,
argument writers, joint bounds and production admission remain open.
Broad accepted milestone exits remain M1/V1/V2/U1/U2/U3 (6/18).
