# Original fixed-query driver qualification — 2026-09-27

The test-only independent original fixed-proof oracle now executes a bounded
query schedule and retains partial or nonempty query payloads through postflight.
The production compilation route remains unchanged.

## Implementation and checks

A fresh original constructor/query run supplies the reference result. The retained
run uses the same original query implementation, with explicit ownership through
return, error and same-Box checkpoint panic. Within each run, source/resource
postflight precedes payload destruction and refund to that run's original Budget.
Separate comparison runs do not establish a shared authentic Budget.

The closed schedule admits at most 32 query ordinals. Empty schedules do not
prepare a strict proof; the first query constructs once and later queries reuse
its caches. Cap refusals and occupied retirement slots are terminal or inert as
specified. Both comparison paths pay the same 21 added typed frame rows and
original constructor prefix before comparing the original query debit suffix.
Each reached positive suffix debit is discovered from the original execution;
one-short controls check refusal order and first-denial history.

All 13 controls passed. They cover entry and single-definition inputs, repeated
queries, nonempty success followed by refusal, exact unsigned width/extent data,
comparison and dominance failures, source identity, late definition rejection,
occupied slots, bounded cut recording and same-Box post-query panic retention.
Full regression passed 356 model and 3,033 backend tests (189 ignored), plus
backend/extractor build and diff checks.

The initial fixture incorrectly expected an argument overwritten by another
argument to reach the rich callback. Diagnostics showed source preparation
correctly refused it first. That case now independently asserts the early
refusal, unchanged Budget identity and correct refund. A separate temporary-local
late-definition case reaches the query and asserts its exact original rejection.
All diagnostic instrumentation was removed before qualification.

Regression receipt:
`e0631416e4fd17e3a6dd681fc66a2075cf0dde028c6ead22a6433e063f1edb34`.
Independent query-driver source review:
`c2622bd430041d84276062adf59c659f8afb453c0ec617fe61402f4a63e32f13`.
Corrected formatted test:
`0cc77be49ab441d33da994c5b7fd8e90d682e646aec8c7b2e14fb2c80a94aa2f`.

## Remaining boundary

These are closed synthetic query schedules, not an authentic source-classifier
connector or production routing. Current snapshots expose allocation identities,
lengths and capacities, not complete nonempty cache contents. Exact bounded
cache/checked-row DATA comparison and allocation-preservation controls are next,
followed by sequential original/candidate comparison under an authentic source
and Budget. Arbitrary meter unwind and allocator overcapacity are not covered.
Logical charges are not physical stack or RSS measurements.

Option-first prelude, later writers, joint bounds driver, mandatory verification
and production routing remain separate work. No debugger or public capture
activation and no global compiler pin change.
Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.
