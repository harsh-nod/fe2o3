# Issue 271 Captured SSA Index Regressions

Reviewable WIP, not default activation or tutorial qualification. Do not merge
this historical tree wholesale over current main. M5/M7 remain active; no
acceptance milestone is newly complete.

## Exact Source

- Host: XSJHARMENON01.
- Checkout: /home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913.
- Checkout HEAD: 10b190b8267c0c4011b4ef6bbb52680a3c44b391.
- Source: 5,069 files, manifest SHA256
  6c11fe39d96cd6b4d61d01201273fe6cae5c2550bed8f6a1abf00370eafb7700.
- Previous published WIP: 2993fde04fdb46b08b6315a5bcc8b3eec0a3e95b,
  source 4781066c8495229411633d0ade83450c1734468002d20afa1b35f64ef6abe8c0.

The source checkout HEAD/index remain unchanged. This status is outside the
source manifest; snapshot construction verifies every source Git blob.

## New Work

Only two lowerer paths change from the previous WIP:

- production_semantic_kir_v1.rs: expose an existing admitted scalar factory
  to sibling tests with pub(super), inside its existing cfg(test) module.
- production_source_output_functional_address_v1.rs: three cfg(test)
  regressions exercising genuine captured SSA rows and their event index.

The fixture adds an admitted self-copy to the existing source factory and
captures occurrences through the real semantic SSA owner. It derives SSA
values from those captured events rather than constructing private owners or
fabricating SSA values. Tests cover absent keys, out-of-range and wrong real
rows, wrong local/use/definition identities, and callback errors, exact panic
payloads, reentry and restoration of existing storage floors. Production
algorithms, budgets, APIs, admission and proof gates are unchanged.

## Current Qualification

The exact current source passed all three focused regressions and then all
404 lowerer library tests, with zero failures or ignores. Source/helper
checks passed before and after both runs. Diagnostics:

- v257-clean-v346-captured-index-focused.sgnw2l5T
- v257-clean-v346-captured-index-lowerer.JSRQm3ot

Backend, extractor and actual-collector tests were not rerun on this test-only
successor. Their preceding source-specific results below are retained as
history, not claimed as fresh execution on this manifest.

## Previous Actual-Collector Checkpoint

The parent source 4781066c passed 401 lowerer, 883 backend and 20 extractor
tests (1,304 total) and an integration harness build, all with source/helper
checks. All seven explicitly enabled actual-collector integration tests then
passed, followed by a retained positive transcript, in 1,769.55 seconds under
the unchanged 1,800-second limit. Logs: v344-collected-r2-actual.It7lNDJZ.

Real rustc collection covered two-root getter programs on gfx942 and gfx950,
a three-root roster, original nonempty-reference refusal, unsafe/ABI admission
refusals, reference-shape checks and changed-reference identity. The direct
gfx942 transcript reports two checked roots and two global accesses, functional
None and aggregate absence. Complete logs, source/tool/configuration checks,
reaping and owned scratch cleanup passed, with independent absence checks.

That diagnostic checks the original materialized N through full Expression/R1
and each actual optimized O through D/P/R2. It preserves original references
and grants no source-proof, functional, aggregate, artifact, launch, default
route or all-tutorial authority. The original Some/aggregate gates remain.

## Remaining Work

Private prepare/Store one-short charge-boundary tests remain gaps. These new
scalar captured-row tests do not establish GetMut/R1/D/P or private charge-site
coverage. The inherited physical relation still refuses differing N/O function
ordinals; arbitrary helpers, phi values and reordered functions are not covered.

An independently checked mandatory source-to-N semantic relation and exact
all-root composition are being implemented separately. No optional-reference
absence, importer replay or matched count replaces that relation. The draft is
not included here; existing proof, default, lineage and aggregate gates remain.

Both main branches separately contain 17701889a8f4e325ca8468b58a3cc26080509306,
including the tested race-name accounting/Pliron changes and the new bounded
numeric pipelined-scalar refusal diagnostic. Its 648 backend tests and final
repository checks passed; hosted CI is running, not yet qualified here.
Parent d5424f5 had complete green generic and formal-contract CI.

Actual main-source attention compilation still fails: pipelined attention has
a multiply-defined semantic scalar; scalar attention clears the old resource
refusal but fails a later missing-ranked-effect correspondence join. A complete
earlier-stage ranked IR dump is retained, not counted as final compilation.

Approved Verus runtime qualification remains deferred, not passed. Full
production integration, legacy retirement, tutorial compilation, simulator
comparison, target-matched hardware and website qualification remain open.
The tutorial evidence pin, CPU WIP and issue 272 WIP are unchanged.
