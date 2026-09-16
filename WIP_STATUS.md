# Issue 271 Actual-Collector Address Correspondence

Reviewable WIP, not default activation or tutorial qualification. Do not merge
this historical tree wholesale over current main. M5/M7 remain active; no
acceptance milestone is newly complete.

## Exact Source

- Host: XSJHARMENON01.
- Checkout: /home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913.
- Checkout HEAD: 10b190b8267c0c4011b4ef6bbb52680a3c44b391.
- Source: 5,069 files, manifest SHA256
  4781066c8495229411633d0ade83450c1734468002d20afa1b35f64ef6abe8c0.
- Previous published WIP: 3d524469941d8a3725219015aee222fff7ca3044,
  source 624245e3cbbed8c58419fc05ba46ca46a811cfd52ee938adfcd2d9b661a5b960.

The source checkout HEAD/index remain unchanged. This status is outside the
source manifest; snapshot construction verifies every source Git blob.

## New Work

An explicit diagnostic selector, FE2O3_EXTRACT_COLLECTED_ADDRESSES_V1=1, now
checks the full Expression/R1 prefix from the original materialized semantic
program (N), followed by D/P/R2 against each actual optimized root (O). This
reuses the private physical-address relation from the previous WIP. The
original references must be genuinely empty; they are never cleared or
substituted. Nonempty references retain their existing refusal.

The shared prefix preserves source admission and resource checks. The existing
source-custody wrapper still requires functional Some and aggregate coherence;
the diagnostic retains functional None and aggregate absence. The diagnostic
does not grant source-proof, artifact, launch, or default-route authority.
Exact root/access counts and a completion line are emitted only after the
bounded writer and all outer postflight checks succeed.

Eight source paths change from the previous WIP, all under
crates/rustc-codegen-fe2o3:

- src/production_ranked_projection_v1/checked_output_source_join_v1.rs
- src/production_checked_output_pipeline_v1.rs
- src/production_rustc_driver_v1.rs
- src/lib.rs
- src/bin/fe2o3-rustc-extract.rs
- tests/reference_binding_v1.rs
- src/production_ranked_projection_v1/checked_output_source_join_v1_tests.rs
- src/production_ranked_projection_v1/checked_output_local_relations_v1_tests.rs

The last path changes only a formatting-sensitive source audit: it narrows the
audit to its wrapper and normalizes whitespace while retaining the assertion
that source gates run before continuation. No production check is removed.

## Qualification

The exact corrected source passed 1,304 ordinary compiler tests:

- 401 lowerer library tests, 17.75s.
- 883 backend library tests, 289.24s.
- 20 extractor tests, 0.04s.

All source/helper guards passed. The integration harness also built
successfully with guards intact. Logs in the diagnostics checkout:

- v257-clean-v344-collected-r2-lowerer.ijovI1nm
- v257-clean-v344-collected-r2-audit-corrected.6I8wfSiv
- v257-clean-v344-collected-r2-integration-build.durJ0TKr

The earlier source passed 882 backend tests and failed the formatting-sensitive
audit described above; extractor tests did not run on that failed attempt.
The corrected-source results above supersede it, without treating it as a pass.

All seven explicitly enabled actual-collector integration tests passed on this
exact source, followed by a retained direct positive transcript. The complete
run finished in 1,769.55 seconds within its unchanged 1,800-second limit.
Fresh binaries, source/tool/config guards, complete bounded logs, child reaping
and owned scratch cleanup all passed. ROOT independently checked the eight
cases' scratch paths and observed process IDs were absent afterward.

The fresh rustc-collected two-root getter programs passed the N-only full
Expression/R1 and actual-O D/P/R2 checks on both gfx942 and gfx950. The three-root
case passed, as did original nonempty-reference refusal, unsafe/ABI admission
refusals, both-profile reference-shape checks and changed-reference identity.
The saved direct gfx942 transcript reports exactly two checked roots and two
global accesses, functional None and absent aggregate, followed by the unique
postflight completion line. Logs: v344-collected-r2-actual.It7lNDJZ.

This is actual collection and relation testing, not GPU execution, source-proof,
artifact, launch, optional-reference proof or all-tutorial qualification. The
run's qualification flag remains false by design; no production gate changes.

## Remaining Boundaries

Separate one-short tests at the private 32/24 charge boundaries and hostile
captured-event objects remain gaps. The inherited physical relation still
refuses differing N/O function ordinals. No arbitrary helper, wrapper, phi,
reordered-function, source-functional Some or default-activation claim follows.

Approved Verus runtime qualification remains deferred, not passed. Native/LLVM/
ABI, simulation, hardware, all-tutorial qualification, legacy retirement and
final production integration remain pending. The tutorial website evidence
pin has not advanced.

Both main branches separately contain d5424f5bf00b7531e11380a86d5d2cb71603807c,
including the race-name accounting fix and tested Pliron dependency update.
That batch removes the scalar-attention storage-budget refusal but exposes a
later missing-ranked-effect correspondence failure. Pipelined attention still
refuses a multiply-defined scalar. Main tests cannot qualify this historical
snapshot, or vice versa. CPU and issue 272 WIP branches are unchanged.

Main's generic CI run 35107039115 and formal-contract run 35107039082 both
completed successfully. Hardware and tutorial qualification remain separate.
