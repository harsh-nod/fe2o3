# Complete bounded original-query contents — 2026-09-27

Test-only original-query controls now compare complete admitted checked rows and
cache entries, including keys, Boolean values, insertion order, row boundaries
and initialized tails. Equal counts or allocation addresses alone are not enough.
The production compilation route remains unchanged.

## Qualified implementation

The fixed observation accepts at most 32 checked rows and 128 total checked
entries, and at most 128 entries in each strict cache. Legacy storage, oversized
contents and owned-side cases refuse explicitly before copying. Two matching
refusals never establish DATA equality. An unavailable original session after
constructor failure is explicitly uncovered, not reconstructed.

The reference uses the original constructor and query implementation independently
of the retained execution. Live, retired and final postflight observations retain
the same owned allocations. Original errors, cuts and same-Box panic payloads
survive through postflight; owned credits are refunded only after destruction.
Both paths pay the same state-independent observation prefix, with 34 typed
accounting rows and checked arithmetic. These source-logical charges do not
measure physical stack usage, allocator capacity or RSS.

All 21 new controls passed: four cache controls, four content controls and thirteen
query/lifecycle controls. They discriminate changed keys, values, ordering,
checked coordinates and extents; exercise capacity and one-short refusals; and
retain nonempty query state after later refusal or checkpoint panic.
Full regression passed 356 model and 3,054 backend tests (189 ignored), plus
backend/extractor build and diff checks.

Regression receipt:
`25cc97d2c25a6ad46be34683bc608d85bc5a82c0b2e3b1d86620d0ffcb797e06`.
Independent source review:
`4565376380c1b2f107ef9e4286cac3c0a97901302fe18bfc43a38b7a24f07c2b`.
Root formatted source selection:
`2971fabca32a75b931497c6b4cfed7b69a716637676c65b70e0b6bb99232f0f0`.

## Remaining boundary

These are bounded component schedules with independent comparison budgets,
not the authentic same-source original/candidate connector under one Budget.
That connector must preserve original-first chronology, separately retained
owners, complete DATA and pre-paid observation work. Real fixed-query zero-cache
contents are currently empty; seeded storage controls do not prove real proof
queries. Constructor-partial DATA, arbitrary meter unwind and allocator-abort
recovery are not claimed.

Option-first prelude, later writers, joint bounds driver, mandatory verification
and production routing remain separate work. No debugger/public capture
activation or global compiler pin change.
Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.
