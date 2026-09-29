# Retained Shared place operations qualification — 2026-09-29

This private checkpoint extends the retained alias-state owner with original
place/path matching, deinitialization and write-place transformations. It does
not activate the ordinary analysis route or construct a source/SSA association.

## Implemented retention

The owner keeps the borrowed place, decoded path, removed holder vector and
cursor, current alias, remaining drain, and output vector attached through
failure. Successful retirement and insertion retain their original positions.
The deinitialization path reinserts an existing empty holder; the write path
does not insert an empty result. Retained zero-length buffers keep their actual
capacity when an early return or refusal occurs.

All four operations and the capacity helper refuse terminal entry before doing
more work or releasing payloads. The original physical Budget and owned-credit
counter remain coupled, with the alias owner's single first-failure slot.
There is no alternate ledger, mutable-budget escape, refund operation, or
generic continuation callback.

Original work and mutation order are preserved. Typed ownership envelopes are
additive. Output growth reserves requested storage before attached allocation,
then observes and admits actual excess capacity before population. Allocation
or excess-admission failure leaves the actual allocation attached; this is not
a claim that a failed excess reservation funded that capacity. Native B-tree
allocation, process RSS and recoverable host OOM are not proven bounds.

## Qualification

Root reviewed the complete implementation, controls and donor transformations.
Independent source review accepted the four original donor mappings, five
ownership corrections, unchanged primitive bodies and terminal guards.
Formatting was twice idempotent.

The broad CPU regression passed on base
`5c8c057609cc5260dc47551fff343caf088aa42d`:

- 17 new place controls within 1,722 pliron tests;
- 370 model and 3,390 backend tests (197 backend tests remained ignored);
- 270 authority/capability tests (four ignored) and 20 policy/runtime controls;
- 385 broker, 250 coordinator and 242 protected-spawn tests (20, four and seven
  separately gated tests remained ignored);
- backend/extractor builds and whitespace checks.

Controls compare original result data and work prefixes at every work cutoff,
storage refusal points and partial semantic errors. They cover retained
current/unvisited/removed owners, early None and dereference paths, empty-vector
capacity, source identity and terminal continuation. Deterministic allocator
OOM and excess-capacity injection are not claimed.

Full regression receipt SHA-256: `22210a2e962843b16936d803181b099ea0fd882ca88af5add68656ee66b11d06`.
No ignored native/GPU mode was silently counted as executed.

## Remaining integration

Read observation and read-place production, rvalue/install and statement
processing, the complete original analyze-observed order, actual SharedReads
source/SSA/promotion association, the canonical-facts backend bridge and the
original Shared-first whole-root join remain necessary.

The source lifetime is not a function-membership proof. These private
components do not grant compiler or native authority, activate argument
writers, qualify genuine nonempty Fixed analysis or provide physical debugger
capture. Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).
