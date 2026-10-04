# One-region U2 acceptance ledger — 2026-09-23

Subsequent qualification: the [actual-source proof freshness ledger](source-proof-freshness-u2-20260923.md) records the later protected-proof test and bounded U2 acceptance. This earlier run and its pending-at-capture statements remain unchanged below.

The final fresh build, full backend regression and actual-source launch-analysis
freshness test passed. The backend suite reports **1,711 passed, 112 ignored,
zero failed**; the separately selected ignored actual-source ladder reports
**one passed**. Ignored tests are not credited as passes. Publication/readback
is a separate final integration step, not implied by these receipts.

This advances the original #282 U2 **one supported region** exit; it does not
require completion of all #280 M2, and does not by itself close U2. The supported
path remains the [bounded Rust bitselect promotion](../source-bitselect-promotion-v1.md)
and the [source-promotion lab](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/source-promotion-lab-v1.md).
LLVM IR is not bypassed: the authored expression becomes constrained inline
assembly inside the ordinary kernel.

## Original five acceptance obligations

| U2 obligation | Bounded evidence and remaining boundary |
| --- | --- |
| 1. Materialize one supported lowered region as ordinary typed Rust; replace it explicitly or create a named concrete variant. | The normal public promotion API creates a distinct candidate file for the genuine typed-HIR/semantic/KIR-selected initializer, preserving the original and surrounding source. The new run again uses the normal-dependency publisher, not a compiler snapshot or object import. |
| 2. Re-admit generated source through rustc/the normal frontend and reproduce unedited semantics and the applicable machine contract. | Historical default/edit/repeat ladders reparse actual Rust into fresh V32 semantic / V17 KIR owners. This run obtains two genuine current owners and verifies the exact XOR/AND/XOR program, source operands and declared low/high register plans against the independent whole-kernel oracle. Final emitted code remains separately qualified below. |
| 3. Edit a real instruction/resource choice, rebuild source through the fixed pipeline, and establish fresh applicable evidence. | This run changes scratch/output/inputs from `4,5,[0,1,2]` to `32,33,[34,35,36]`; actual source and semantic identities change while source launch geometry agrees. Fresh current analysis succeeds after the old analysis is rejected. The separate XOR-to-OR lab changes the computation and has its own oracle/source/native chain. |
| 4. Demonstrate stale source, unsupported extraction, hidden-clobber/invalid-resource rejection and proof/capture invalidation. | Historical source/refusal ladders cover stale bytes, unsupported/ambiguous extraction, overlapping/out-of-range roles and live-in/program mismatches; genuine old catalog/capture identities are rejected. This run adds rejection of a genuine old source-launch roster before a new V17 owner can be returned. These bounded role checks are not general native clobber-lifetime proof. Protected-proof reuse/invalidation remains **not qualified by this V17 run**; the separate actual-source stale-proof qualifier is pending. Capture/query rejection is not a substitute, and no result from that separate qualifier is imported here. |
| 5. Compare actual results and final emitted instructions/resources with an independent reference and declared contract. | This run compares complete simulator backing/initialization/canaries with host bitselect, but emits no LLVM/native artifact. Existing native checks inspect exact authored operands and descriptor capacity for their own pinned source/LLVM chains. They are historical, separately qualified observations—not native qualification of this new build. |

## Do not merge distinct source/native chains

- **Prefix-free register edit:** the older [machine qualification](../source-candidate-machine-qualification-v1.md)
  records a private prototype's original/default/edited/repeat source chain and
  separate edited-LLVM O0/O3 native inspection. Later normal public roundtrips
  are recorded separately in [source-authoring evidence](authoring-source-values-20260922.md).
  Neither is relabeled as this new run.
- **Live prefix:** that same dated source-authoring evidence records a normal
  public seed, default/edit/repeat and the different oracle
  `bitselect(a,b,mask) ^ (a | mask)`. Its low/high O0/O3 native join consumes
  the exact prefix LLVM. Matching edited/repeat LLVM bytes do not invent an
  additional repeat-native process.
- **XOR-to-OR:** [instruction-edit evidence](authoring-helper-instruction-20260922.md)
  and its [lab](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/source-promotion-instruction-edit-lab-v1.md)
  use `b | (a & mask)`, three fresh exports/90 simulations and four
  default/edited O0/O3 full-payload observations. It is not an equivalent
  register-only edit, the prefix fixture, or this build.
- **Bounded repeat:** [repeat-native evidence](authoring-repeat-native-20260923.md)
  joins separate one/two/fifteen/repeat15 LLVM inputs to four actual native calls
  per fork. Phase24 revalidated historical Phase22 source/CPU results; it did
  not rerun those source compilations or simulations.

## New actual source-analysis observation

The tested worktree is base `0fa063fd17921d24f955479bb4736d0a8beb1cf2`
plus the test-only increment, not that commit alone. Its unchanged before/after
census is 6,688 files / 101,442,416 bytes, SHA-256
`18d41d38ead4d039b1a460429f06af78d3367bbe34fa823a71db6234f10fb125`.
Selected tool/input arrays also remained unchanged. The fresh target is
`target-milestones-phase28-u2-launch-freshness-r2`; earlier targets and
receipts were preserved rather than overwritten.

The six test-only source postimages are byte-identical in the compiler and
mirror candidates. The actual build/regression/source execution recorded here
ran on the compiler candidate only; matching mirror source is not an
independently executed mirror qualification. Three existing files add
test-only module joins and three leaves contain the test/helper implementation.
No production admission, proof consumer, finalizer or ABI code was changed.

One normal publisher process creates the seed. Three fresh candidate callbacks
then run **default-current → edited-stale → edited-current**. Only the genuine
move-only, source-only old launch roster crosses sessions as a negative input;
no old SSA, executable owner, snapshot, proof or successful continuation does.

The unchanged `ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget`
returns precisely `Lowering::Unsupported` with detail
`ordered program requires one exact V32 gfx942 source and launch root`.
It accepts eight additional work units after a seven-unit prefix: total 15.
The same ledger preserves its 448-byte incoming logical floor/peak, then the
caller releases that floor to zero. No new executable owner is returned.

The two current-binding positives each execute **15 scenarios twice**:
five input triples × lengths 0/1/65 × two deterministic replays. Across two
register plans this is **60 runs, not 60 distinct scenarios**. They check
`(a & mask) | (b & !mask)`, every output/backing and initialization byte,
both canaries, unchanged requests and replay equality—not merely successful
compilation. Each positive executes 33,940 simulator steps. The retained source
fixture contains ten files / 7,543 bytes. These observations do not prove
arbitrary inputs, race freedom, native lifetimes or GPU behavior.

All paths below are relative to the operator-retained mi350 task root
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`; they are not public downloads.

| Evidence | Bytes | SHA-256 |
| --- | ---: | --- |
| `logs/phase28-resume-r2-compiler-u2-launch-freshness-build-r3/receipt.json` | 24184 | `e6805c0f9825493e04410165ed144a9cd22e11d6891cbb9e37488d1e5ab766ff` |
| `logs/phase28-resume-r2-compiler-u2-final-regressions-actual-r1/receipt.json` | 31235 | `83598dc361f34b5b5d4acfbf610a2f47d920715ea4819e60cea28920930fec70` |
| `logs/phase28-u2-source-launch-freshness-r2/launch-analysis-observation.json` | 36317 | `65917c23cda2ffb369bc232a30ff57a267ac8b2f23887e4fb5f3107ae521535c` |

Both final outer receipts report exit zero, no error/reason/signal, completed
stream drainage, unchanged input/tool/source ledgers and no authority or
hardware qualification. The combined command runs the full backend libtest
binary with two test threads, the normal headless no-argument refusal smoke,
then the exact actual-source ladder with one test thread. Its stdout is
264,761 bytes, SHA-256
`115300016d2864cc403cdc5c9f9fb39e7306754e781fa1ca48e6de2ec9cffa75`.
The full suite took 179.83 seconds and the selected actual ladder 15.31 seconds.
Normal target-feature and internal-feature warnings remain in the retained
stderr; this receipt does **not** run or claim strict Clippy.

### Retained failures and corrections

| Earlier evidence | Result | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `logs/phase28-resume-r2-compiler-u2-launch-freshness-build-r1/receipt.json` | Failed: JSON macro recursion | 13669 | `584c8acd28fb415161b068be5ff6891d375495c2c9a2502e1558545fcf7623f6` |
| `logs/phase28-resume-r2-compiler-u2-launch-freshness-build-r2/receipt.json` | Passed after macro correction | 22423 | `26072850da0022935c416906f461759f304fa5c70df2caf4223f5a610256ddb3` |
| `logs/phase28-resume-r2-compiler-u2-launch-freshness-actual-r1/receipt.json` | Earlier actual ladder passed | 27960 | `a83918303930fd8cdd19b98b33f50b024ffe273ba6dcc5fc46b824c1b3aaf966` |
| `logs/phase28-resume-r2-compiler-u2-backend-regressions-r1/receipt.json` | Failed: one structural census assertion | 19730 | `b4032d77fcafe01b050290540e69fb52c07b5d0d006b7c95409c4901f51ad599` |

The first correction splits test-report construction into smaller objects
without changing the flat schema or raising the crate recursion limit.
The first full backend run then reported 1,710 passed, one failed and 112
ignored: the source-structure test counted the new test-only launch-roster
constructor alongside the single production constructor (two instead of one).
The helper was moved unchanged into its own cfg(test) child leaf,
`ordered_program_launch_freshness_v17_tests.rs`. The existing census assertion
was not relaxed and the production constructor was not changed. The final
fresh build and full suite above pass that exact census test. Earlier actual
success is retained history, not retroactively labeled as the final build.

### Byte-identical compiler/mirror test sources

These are the qualified build postimages. Final formatting afterward only moved
the test-only launch-analysis module declaration into rustfmt order; its body,
selector and behavior were unchanged. Both repository postimages still match.
The final formatting check passed before publication.

Paths in this table are relative to `crates/rustc-codegen-fe2o3/src/`.

| Source | Bytes | SHA-256 |
| --- | ---: | --- |
| `production_pipeline.rs` | 192974 | `06762c388a61bb637b01d4eda638fc9c3ba209f18df0cbe2457ec921b60b85c9` |
| `production_pipeline/ordered_program_diagnostic_v32.rs` | 5283 | `8e90cbbbc19ba77c71e66228745b6b142fd379375ccfd4fdd2fc8b921df0eb7f` |
| `production_pipeline/ordered_program_launch_freshness_v17_tests.rs` | 1998 | `a36ce80f694019e6741f4d91d84db46bf8a539f0f599df82249ef1725b6682f9` |
| `production_pipeline/source_candidate_launch_analysis_v17_tests.rs` | 16743 | `6b8d71546e163f4f5229758e13b401d30e6f6df49fc2d471f12800f736f630cc` |
| `production_rustc_driver_v1/source_bitselect_candidate_machine_driver_v1_tests.rs` | 10656 | `6ad88be8b665c95b3ec309ec0938b85783555488d4b99c21c8ff65e46abadd1b` |
| `production_rustc_driver_v1/source_candidate_launch_analysis_driver_v17_tests.rs` | 20705 | `7ffd8060f66818184353b65b844726d2c379b72504bd30a492339d2a7722c074` |

## Reproduce this precise slice

Use the pinned nightly, offline locked dependencies, normal publisher and
matching measured backend test executable. The positive publisher is
`fe2o3-source-bitselect-promote-once`; the similarly named
`fe2o3-source-bitselect-headless-consumer` only performs a no-argument refusal smoke.

With absolute pinned paths assigned to the variables below, the exact parent
selector/environment is:

```sh
env RUSTC="$U2_RUSTC" LD_LIBRARY_PATH="$U2_LIBRARY_PATH" \
  FE2O3_TEST_SOURCE_HEADLESS_CONSUMER="$U2_PROMOTE_ONCE" \
  FE2O3_TEST_SOURCE_CANDIDATE_LAUNCH_ANALYSIS_OUTPUT="$U2_FRESH_OUTPUT" \
  "$U2_BACKEND_TEST" \
  --exact production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::launch_analysis::actual_source_candidate_launch_analysis_ladder \
  --ignored --nocapture --test-threads=1
```

These variables are required inputs, not discovery/fallback commands.
The output and corresponding repository-relative source subtree must both be
fresh; retain failed runs. Use the [lab's matching-library and process-isolation
rules](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/source-promotion-lab-v1.md).
Do not invoke an unprepared child selector or credit an empty/ignored test filter.
This run needs no native observer or GPU dispatch.

The reports explicitly retain `functional_proof=false`,
`proof_invalidation_qualified=false`, `production_resume=false`,
`hardware_observed=false` and no source-authentication or artifact/launch
authority. Applicable protected proof/runtime boundaries need their own evidence
and review. The separate actual-source stale-proof qualifier is still pending
in this ledger; this diagnostic V17 chain is not represented as producing or
invalidating protected proof. No protected admission/finalizer/ABI implementation
changes, complete U2 closure or milestone signoff are inferred from this result.
