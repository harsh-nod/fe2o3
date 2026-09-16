# Issue 271 Guarded Formal Analysis Checkpoint

Reviewable WIP, not production activation or tutorial qualification. M5 exact
optimized-program verification and M7 production integration/legacy retirement
remain active. M8 is unqualified; no acceptance milestone is newly complete.
Do not merge this historical tree wholesale over current main.

## Exact Source

- Host: XSJHARMENON01.
- Checkout: /home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913.
- Checkout HEAD: 10b190b8267c0c4011b4ef6bbb52680a3c44b391.
- Source: 5,073 files, manifest SHA256
  e511a34ba9f7abe70a11ee41775e5df74d7099aee1f28acd522c7645b735729c.
- Previous published WIP: 54c34f322654b1f455ca2e092bbd658d434718d3.

Snapshot construction verifies every source Git blob and preserves the source
checkout HEAD/index. This status is outside the source manifest. The new batch
changes ten source paths, including two new files, plus this status.

## Implemented

The formal-memory engine now recognizes a bounded exact conditional slice-access
recipe: rank-one GlobalX index, the same formal slice's length/data, index less
than length, and zero selected for an inactive offset before GEP. Each actual
access needs the matching explicit predicate or a checked unique true incoming
edge dominating that use. Conditional recipes are not unconditional affine facts.
Private index overwrites and conflicting pointer joins do not retain stale facts.

The analysis retains the active domain [0, launch_extent) intersected with
index < slice_length, symbolic bounds, whole-formal-allocation alias regions and
all required conflict pairs. It does not invent a launch-sized minimum slice
length or discharge Rust slice validity, alignment, lifetime or exclusivity.
Byte-only bound evaluation cannot discharge the new symbolic obligation.
Legacy receipt formats explicitly refuse representations they cannot encode.

New guarded scans, queries, sorting, capacities and pair work are bounded and
charged. The new workspace/report storage ceiling is not a claim about whole
engine RSS or the accounting of every historical allocation.

The existing five positive final-consumer tests now pass unchanged. The original
guarded-read integration fixture also stays unchanged; its obsolete refusal
oracle now requires Complete while checking both retained accesses, symbolic
bounds, alias obligations and refusal by the legacy receipt encoder. Arbitrary
guard and wrong-predicate negative coverage remains intact.

## Exact Qualification

On the final source above, with all source/helper guards passing:

- 804 kernel-IR tests passed across 41 suites, zero failed, one pre-existing
  ignored full-block-count stress test. Includes all 16 new guarded unit cases.
  Logs: v257-clean-v358-guarded-formal-final-kernel-ir.32lFBefV.
- 426 lowerer library tests passed, zero failed or ignored, 9.54 seconds.
- 896 backend library tests passed, zero failed or ignored, 166.40 seconds.
  Consumer logs: v257-clean-v358-guarded-formal-final-consumers.5fy2EluY.

Total: 2,126 passing tests. Changed-region pinned formatting was applied; this is
not a full-workspace formatting result. No new extractor/binary, actual collector,
tutorial, simulator, hardware, website or Verus qualification is claimed.

Earlier failures remain recorded rather than relabeled: the initial new-test
Type::U32 compile failure, then 452 passing tests and the obsolete guarded-read
oracle failure. These were corrected using the existing Type::Scalar API and the
reviewed fixture-preserving oracle change. Before the final two hostile tests,
426 lowerer and 896 backend tests also passed on their recorded predecessor
sources. The earlier published WIP's five genuine formal failures are now fixed.

## Remaining Integration

Production artifact lineage and lowerer evidence still invoke legacy formal
receipt encoders, which cannot encode the new symbolic rows. A read-only audit
also identified a legacy admission roster that assumes every guarded read still
has a ranked-proof refusal reason; mixed newly supported and older ranked-only
reads require explicit compatibility work. Neither gate is weakened here, and
passing in-memory consumers is not proof of artifact publication compatibility.

Broader source rules, managed metadata custody, runtime/target binding, optional
reference refinement, production activation and legacy removal remain open.
The separately published initializer/local-frame WIP is a75d1ab228767280cb1d985dfd5cf59027862e52.
Its genuine helper positive still fails HelperEffectsUnavailable. Retained
helper obligations and downstream consumers are a separate unqualified draft.

Both public main refs were independently observed at
9f8ffda4df3647d0ed7c424f16a6ff0111a32a69. Neither this historical canonical tree
nor the initializer WIP is composed with that newer execution-KIR/MIR work.
Preserve those concurrent changes; all test claims here are source-specific.

M8 all-tutorial compilation, simulation, target-matched hardware and website
qualification remain open. Approved Verus runtime qualification is deferred,
not passed. No SSH/GPU work or shared-machine cleanup occurred in this batch.
Separate issue 272 WIP was not modified by this checkpoint.
