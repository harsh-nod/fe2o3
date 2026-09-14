# R117 Independent Archive Reviews

Two read-only reviews completed on 2026-09-14 with no acceptance blockers.
Neither reviewer modified artifacts or ran project builds, tests, solvers or
GPU jobs.

## Evidence Review

Reviewer: `r115_evidence_review`.

Verified all 372 raw artifact hashes, exact inventory and byte equality to
retained originals; all 155 historical pins, eight freeze-helper pins and seven
manifest-helper pins match. All 32 compiled negatives reach their exact
behavioral assertions and all 32 restorations recover 5,689 source identities.
All 70 campaign chain entries have valid predecessor hashes and clocks; all 38
campaign runs have closed children and cleared owned process groups.

Frozen/restored suites pass 16/15/37/7/9. The closed collector matches the summary.
GNU/musl each pass 2,762 tests with five ignored; 17 source gates, ten auxiliary
checks and 9/31/117 runner/freeze/qualification contracts pass.

## Source And Oracle Review

Reviewer: `r115_contract_review`.

Independently verified all 372 artifact hashes, both pinned original-source
texts, current source map, every compiled behavioral failure and restoration,
28 distinct mutation groups and exact frozen/restored suites. All 52 integrated
runner records have closed children and matching source endpoints. Full GNU/musl
rosters add only the sixteen KFD tests above R116; neither routing guard is
counted as a behavioral negative. Seven isolated runs retain both original
failures and their distinct source cohort.

Source review confirms full-owner retention before validation, exact generation
and state precedence, forward borrowed cleanup, one-shot behavior and no detached
output allocation or conversion. Separate detached data stays with its owner.

## Accepted Boundary

Both reviews accept only local scripted lower detached-control cleanup for map
`c7cbc3c52eb29d3eaf70cfe9fdf964a9f5c092eb439cad26d1706874f51735be`.
The archived summary SHA-256 is
`b253372c1f392bd8a92925b5cdd68a582575c82006d4f2fe91c8e100131390b0`.

Raw UTC remains intact; ordering uses same-boot monotonic observations. Source
checks establish endpoint equality, not continuous immutability. Restoration
relies on exclusive mutation ownership and is not crash-atomic recovery.
Persistent returned-data bridges, data disposal, live model retake/commit, queue
teardown, native execution, formal correspondence, aggregate memory bounds and
HIP/HSA performance parity remain outside acceptance.
