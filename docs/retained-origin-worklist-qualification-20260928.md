# Retained exact-origin worklist component — 2026-09-28

The private origin-propagation component now retains its FIFO queue, head and
local work counter through success, failure and unwinding. Origins and edges
remain caller-owned: a future combined owner must keep those tables alive with
this queue through checked postflight and destroy them before credit refund.
This scratch component grants no source or checkpoint authority.

The unchanged original algorithm is preserved after an explicit 32-work-unit
prefix and fifteen additive typed accounting rows. Initial local order and
supplied successor order are unchanged; no sorting or different traversal is
introduced. Original partial updates, errors, census limits and debit order
remain intact. Malformed successor indices retain the original panic behavior;
this component does not make malformed input admissible.

All 11 new controls passed, covering independent original data/debits, FIFO order,
cycles and duplicates, conflicts, inconsistent lengths, exact/one-short budgets,
every short work limit, callback success/error/panic, retry and ledger mismatch,
non-static generic values, and malformed-successor partial custody. Full
qualification passed 356 model and 3,233 backend tests (197 ignored), with
backend and extractor builds.

Full regression receipt:
`3b78019f35bfc8fa5ffc70469b80e27e87de06380a9e533c0688dbf37118dea5`.

Independent source review:
`ac4ca3bff75271bfee8329bad09ae49fcf30fe3361dd76a908ef7b914749ead9`.

These are logical resource policies, not native stack, allocator or RSS
measurements. No new genuine-source hook or GPU execution was run for this
component. Retained carrier/provenance integration, authenticated continuation,
argument writers, joint bounds and production admission remain open.
Broad accepted milestone exits remain M1/V1/V2/U1/U2/U3 (6/18).
