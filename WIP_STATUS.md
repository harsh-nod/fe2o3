# Issue 271 Initializer And Retained Local-Frame WIP

Review snapshot, not main-ready or milestone-complete. Source basis:
`17f786cca73585a6ca39554f3d25595a3d9eb828`. Current tested source manifest:
`364f99f12df6f1ae8b1b84420c0c468bb523caa9f98e3671ea6ea1f99778aa3e`
(5,130 files). Publication preserves the source checkout HEAD/index and verifies
all snapshot source blobs against that manifest.

## Changes

Eleven source/test paths differ from the basis. The opt-in local-frame chain
analysis retains selected physical control, simultaneous scalar edge bindings,
allocation/access rows and latest preceding Store information. The original
single-block entry is preserved. Unresolved predicates, unsupported effects,
malformed sinks and incomplete initialization fail closed.

The retained-helper owner now preserves all four row families, exact per-function
ranges and function-local allocation indexes under its original ledger and
cached storage receipt. All output capacities are reserved before core analysis;
actual six-vector capacities and enlarged header/ranges are accounted. Copying
checks exact module/function, ordinal, starts, lengths and prepaid capacity.
Scoped queries preserve the original incoming floor and cannot release borrowed
control/substitution slices outside the callback.

Private-array output occurrence keys include the initializer component from the
sealed source effect. Distinct elements at one statement cannot collide.
Ordinary queries still require the original source index relation; whole
initializers through that facade and unsupported Read proofs keep their exact
refusal. The retained-read test uses a genuine initializer, indexed write and
read, verifies ten source/N effects (nine Stores and one Load), then runs the
normal seven-pass optimizer. Existing promoted cases are unchanged.

This snapshot does not admit effectful source helpers, grant artifact/runtime
authority, or introduce an initializer/Load source-to-optimized-value theorem.
New retained-chain positives are verified physical KIR component tests, not
source-helper admission. Genuine raw-empty helpers still return None.

## Exact Qualification

All results below refer to the exact current `364f99f1` source, with
source/tool/config guards passing:

- 22 focused retained-helper tests passed, including ten new chain tests.
- Full lowerer library: **400 passed / 1 failed**. Focused22 are included in400.
- All five lowerer integration suites: **135 passed**.
- Full backend library: **682 passed**.
- Three lowerer compile-fail documents passed for their intended private
  construction and E0521 scoped-borrow escape errors. The new test covers both
  control and edge-binding slice getters.
- All 822 KIR tests across 41 suites passed, with one existing ignored stress test.
- All four core compile-fail documents passed for their intended errors.
- Workspace formatting passed.

The sole lowerer failure remains the unchanged positive
`private_array_initializer_uses_exact_sealed_helper_instance_and_promoted_absence`:
its fixture fails with `HelperEffectsUnavailable { function: 1 }` before sealing.
It is unfinished integration work, not a newly passing case. It has not been
skipped, rewritten as a negative or enabled by relaxing admission. Independent
static review confirms the gate/call route is unchanged; no separate fresh
clean-baseline execution is claimed.

Earlier `29617043` source had 390 passed / the same one failed in the library,
135 integration and 682 backend passes, and formatting passed. The ten-test
increase is the new retained-chain child. Earlier `d05fb3e3` source passed822 KIR
tests (one existing ignore) and four core documents. Its first output-only run
had a test-fixture failure: a read-only array was promoted. The later genuine
retained-read fixture corrected that assumption without changing promotion.

ROOT logs on XSJHARMENON01 under
`/home/harsh/work/fe2o3-issue271-diagnostics-20260910`:

- `v362-helper-v365-retained-chain-focused.c58l1Xab`
- `v362-helper-v365-retained-chain-docs.r0ooYdab`
- `v362-helper-v365-retained-chain-scoped-docs.1Fe0u09u`
- `v362-helper-v365-retained-chain-full-lowerer.8FAphrHD`
- `v362-helper-v365-retained-chain-integrations.nfOIaYLP`
- `v362-helper-v365-retained-chain-backend.944JGd9J`
- `v362-helper-v365-retained-chain-kir-all.lCbqnCHd`
- `v362-helper-v365-retained-chain-kir-docs.FaqOHPG6`
- `v362-helper-v365-retained-chain-format.mDBDeKXQ`

The independently reviewed four-path retained packet is
`v364-retained-chain-review1.patch`, SHA256
`2738d79b6292f201d6ae94be0854f93e0c4d3c5a3efad15d14d9fa9d627ae615`.
ROOT and a separate reviewer verified its 41 pins and exact 4-path/25-hunk replay.
All applied postimages matched. Component schedules are derived from the code,
including371/370 and705/704 copy boundaries; tests passed without calibrating
limits from a successful run.

## Main And Remaining Work

Both mains separately contain `6ae096ca717c762ad00d5d1b3ca80d4db5e0628c`:
the main-compatible local-frame producer plus literal initializer tracking and
production ranked constructor/attachment integration. The composed initializer
batch passed 687 backend and 528 lowerer tests, formatting and publication policy
checks. Main preserves its newer Execution V15, semantic MIR V29, shared-slice
and RustCall boundaries. Do not merge this historical WIP tree wholesale over
main. A current-main retained-owner/floor/scoped-call port is separately drafted.

Still open: genuine source/call/initializer/Load/latest-Store correspondence,
normal N/B/O transport, nonempty source-helper admission, per-call frame and
target semantics, final formal/artifact policy, and tutorial acceptance.
A separate main query is being drafted to reuse checked typed value transport
for each actual optimized initializer Store; this snapshot does not include it.

M5/M7 remain active. M8 and whole-compiler verification remain unqualified.
Approved Verus runtime qualification is deferred, not passed. No full-repository,
tutorial, hardware or runtime qualification is claimed.
