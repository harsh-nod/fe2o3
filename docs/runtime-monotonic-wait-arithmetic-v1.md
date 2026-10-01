# Monotonic Wait Arithmetic V1

The native wait cursor keeps its `Duration`/`Instant` representation, constructors,
deadline ordering and configured cadence. Four small shared numeric helpers
implement saturating attempt increments, the existing 64-spin/16-yield prefix,
sleep-minimum selection and canonical seconds/nanoseconds backoff.

The sleep adapter returns an existing `Duration`. Backoff uses carry, saturation
at `(u64::MAX, 999_999_999)`, and lexicographic ceiling selection; it does not
reconstruct production durations through `u128` division. Canonical input and
output constraints are explicit. This avoids a proposed lowering risk, but is
not a measured performance improvement.

## Qualification

Signed candidate `a7d110af1ff14bf71a82799c6224abc7894149e9` passes four complete
two-input Verus positives at 7/0 and all 17 calibrated actual-body negatives at
6/1, with an equivalent control, relocated positive, closing positive and release
brackets. All four metadata and 23 proof command groups close. No selected-root
or selected-function filter, relaxed proof policy or local std-assumption bridge
is used. The full result includes derived obligations; it is not seven separate
runtime APIs.

The exact native source passes 1,841 no-default and 1,847 all-feature KFD library
tests and strict Clippy. A separate source-byte bridge ties all 6,398 selected
CPU inputs to the signed candidate. The CPU SSH transport failed after complete
remote execution; recovered raw/ELF/census evidence was independently accepted
without relabeling that original transport as successful.

See the [recorded proof packet](evidence/dev-monotonic-wait-arithmetic-2026-10-01/README.md)
for raw outputs, exact fixtures, portable offline diagnostic inspection and
explicit external replay prerequisites.

## Boundary

The theorem covers the actual shared numeric bodies under canonical input
constraints. It does not prove `Duration`/`Instant` conversion, lazy evaluation,
constructors, the entire cursor, the observation loop, OS scheduling or hardware.
Native differential tests support those adapter behaviors but do not prove them.
The absent-None mutant is valid for the broader helper contract; the native
adapter bypasses that helper for None. Diagnostic macro spans establish source
association, not branch-execution witnesses.

The default cadence, error precedence, currentness checks and custody boundaries
are unchanged. No performance acceptance or milestone exit follows from this
component. A matched native measurement of the changed arithmetic is still
needed before claiming equivalent or improved execution cost.
