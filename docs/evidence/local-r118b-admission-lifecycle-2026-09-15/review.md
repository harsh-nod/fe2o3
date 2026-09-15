# R118B Independent Archive Reviews

Two read-only reviews completed on 2026-09-15 UTC with no acceptance blockers.
Neither reviewer changed artifacts or ran project builds, tests, solvers or GPU
jobs. Primary owns implementation, execution and publication.

## Evidence Review

Reviewer: `r115_evidence_review`.

Verified exact membership and all 1,432 raw artifact hashes against retained
originals, including 468 mutation/restoration artifacts pinned during closed
prefix reviews. All 935 historical pins, 23 helpers plus launcher and 862 frozen
validation inputs match. All 78 compiled negative executions reach their exact
named assertions, restore the full source map and close their owned processes.

Fresh GNU/musl each pass 2,780 tests with five ignored. Seventeen source gates,
ten auxiliary checks and frozen/restored suites 9/4/5/1/15/14 pass. Contract
counts are core 48, prior history 45, corrected history 27, runner 9, freeze 57,
lifecycle 22 and qualification 167. The closed collector transcript matches the
summary exactly; its record SHA-256 is
`0413025b80914e55f62cc3e419366f23da78793f2c2d39938f24b1ae2672f150`.

## Source And Oracle Review

Reviewer: `r115_contract_review`.

Independently checked all 11 source changes and 18 added tests, the 5,692-file
source inventory, eight original-source buffers, helper and environment pins,
all 1,432 archived hashes, 78 compiled failures, 74 distinct maps, exact
restorations, restored suites and collector equality. GNU/musl preserve the
R117 executable rosters plus exactly eighteen runtime tests. Production
semantics, public APIs, features and dependencies are unchanged.

C1 checks identity and actual backend routing; C2 checks exposed descriptor
identity; C3 checks reply and retained-owner lifecycle. Corrected C3 observation
snapshots release mutex guards before assertions. Campaign case 73 reaches the
normal assertion at `completion_tests.rs:338:13`, with one named failed test,
Cargo exit 101 and no abort or destructor double-panic.

## Launcher Observation

The root launcher session exited zero after reporting all campaign checks as
passing. Its terminal output also contained two Node circular-import warnings
for `assertVacant` and `outputNames`. The launcher invokes its lexical `launch`
before assigning exports; baseline inspection imports the test/prepare modules,
which capture those unavailable exports in unused bindings. Neither preparation
nor the dormant test closures executes in that process. Executed guards use
lexical functions, and the separately executed 167-contract process imports
fully exported helpers. Review found no acceptance blocker in this path.
Preserve the frozen helpers; a future helper version should assign exports
before invoking main. The original terminal output remains in the tool session,
not an invented archived raw launcher transcript.

## Accepted Boundary

Both reviews accept local host/test qualification for source map
`3aa1ab32e9e6e6ffd0474ec9055d3c29b02bf53e45129406617e50f0b3c15126`.
The archived summary SHA-256 is
`a5d8da25ec95c0d4ef330d2ae4c3f2e5894e2226d09499737f3842f474c57b55`.
The 78 executions comprise 75 production executions, one combined-defense
execution and two explicitly labeled helper-calibration executions.

Stopped R118 remains unaccepted. Its aborted case 73 is not retroactively
qualified, and the preliminary corrected regression is outside the new 78.
The nested `corrected_history.full_campaign_accepted: false` preserves that
validator's earlier prequalification observation; current campaign evidence is
the complete mutation/restoration chain and closed collector, not that field.

Clock ordering uses same-boot monotonic observations while preserving raw UTC.
Source checks establish endpoint equality, not continuous immutability, and
restoration is not crash-atomic recovery. Generated native ISSUE/COMPLETE,
production journal integration, Worker/compiler authority, formal correspondence,
aggregate memory bounds and HIP/HSA performance parity remain unqualified.
