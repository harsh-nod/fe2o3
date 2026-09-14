# R116 Independent Archive Reviews

Two read-only reviews completed on 2026-09-14 with no acceptance blockers.
Neither reviewer changed artifacts or ran project builds, tests or GPU jobs.

## Evidence Review

Reviewer: `r115_evidence_review`.

Verified exact coverage and hashes for all 378 raw artifacts, including byte
equality to retained originals and all 183 pinned historical artifacts. Manifest
and helper pins match. All 29 compiled negatives reach their exact behavioral
oracles; every restoration recovers all 5,688 source identities. All 63 campaign
chain entries have matching predecessor hashes and monotonic ordering; all 34
campaign runs record closed children and cleared owned process groups.

Restored suites pass 19/42/12/2, and the closed collector transcript matches the
summary. Reviewed GNU/musl 2,746/5 results, source/auxiliary gates and 9/31/122
contract suites remain byte-identical in the archive.

## Model And Oracle Review

Reviewer: `v3_settlement_handoff`.

Independently verified all 378 artifact hashes, current source identities, all
29 actual behavioral failures and exact restorations, restored suites, full-run
rosters and closed collector equality. The three review-added mutations fail at
their pinned free-stack and Unknown-conflict assertions. No structural source
guard is counted as a compiled behavioral negative.

The production review found bounded touched-chain validation before scratch or
commit, correct Success/NoEffect lineage behavior, sticky revalidated Unknown,
and no new production Context consumer or native authority. The public enum
extensions can break external exhaustive matches; the contract records this
compatibility limit.

## Accepted Boundary

Both reviews accept only local executable settlement and counted-work behavior
for source map
`54692ff83a852b8bdb3f7bd156cdd33bfea5a2b5cd21789665bb75ada9872517`.
The archived summary SHA-256 is
`87ff18ea291a130451c38cb27cb1aee70cbdd87fdb2486d936d264053b3caa3d`.

Raw UTC remains intact; ordering uses same-boot monotonic observations. Source
checks establish endpoint equality, not continuous immutability. Restoration
relies on exclusive mutation ownership and is not crash-atomic recovery.
Production Context integration, authenticated receipts, formal correspondence,
recovery/reuse, native execution, aggregate memory and HIP/HSA performance
parity remain outside acceptance.
