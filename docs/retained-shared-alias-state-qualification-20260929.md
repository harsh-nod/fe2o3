# Retained Shared alias-state qualification — 2026-09-29

This checkpoint adds a private, inert retained owner/session for nine existing
Shared alias-state primitives. It does not replace the ordinary analysis route,
construct source-driven aliases, admit a semantic/SSA owner, or grant compiler,
native or GPU authority.

## Implemented custody

The owner retains candidate/holder/active state, the current alias, the remaining
drain, and a removed holder vector with its cursor. Partial failure keeps those
allocations attached through outer postflight. Successful alias retirement stays
at its original successful position. Every primitive now refuses non-active
entry before further work, mutation or payload release; an ignored error cannot
resume the failed session.

The session borrows the original physical Budget and owned-credit counter
together. It binds exact source/types, physical ledger/counter identity, coupled
growth, held floors, sticky denials and the first resource error. There is no new
ledger, mutable-budget loan, replacement policy or refund operation. The outer
caller must drop retained owners before releasing their original credits.

Original primitive work and mutation order are preserved, with an explicit
32-unit entry and additive typed frame/mutation envelopes. These are logical
source-policy charges, not complete native B-tree node allocation, stack or RSS
bounds, and not a recoverable host-OOM guarantee.

## Qualification

Root reviewed the full implementation, controls and mapped callees, checked all
nine reversible donor transformations, the unchanged parent prefix, eight exact
fixture copies, two snapshot copies and both immutable correction inverses.
Independent review found and corrected continuation-after-error behavior and
four omitted result carriers before execution. Formatting was twice idempotent.

The broad CPU regression passed on base
`c7b1a12f62368c7b3b222dad5fc7cf6ce4bdf537`:

- 16 new alias-state controls within 1,705 pliron tests;
- 370 model and 3,390 backend tests (197 backend tests remained ignored);
- 270 authority/capability tests (four ignored) and 20 policy/runtime controls;
- 385 broker, 250 coordinator and 242 protected-spawn tests (20, four and seven
  separately gated tests remained ignored);
- backend/extractor builds and whitespace checks.

The controls compare complete original DATA and original work/refusal prefixes,
including every work cut, storage cuts, partial original semantic errors,
retained current/unvisited/removed owners, physical identity substitution,
sticky first errors, unwind, cap fallback and one-shot behavior. The continuation
test covers both an original error and a resource denial with one work unit still
available, then tries every primitive without allowing any state or work change.

Full regression receipt SHA-256: `6c7985292952602e72e786dc1fb5d291c06584dac93e4244c0884b0bd53bb0d6`.
No separately ignored native/GPU mode was silently counted as executed.

## Remaining integration

Private test seeds are inert DATA, not a production source/SSA construction.
Retained place/read/write transformations, rvalue/install and statement-driven
producers, complete original analyze-observed/liveness/filter order, actual
SharedReads source/SSA/promotion association, the canonical-facts backend bridge
and the original Shared-first whole-root join remain necessary.

This component does not activate argument writers, prove genuine nonempty Fixed
analysis, admit the production route, or provide physical debugger capture.
Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18); the other exits stay open.
