# Assembly authoring implementation status

This is a working-implementation handoff for [#280](https://github.com/harsh-nod/fe2o3/issues/280),
[#281](https://github.com/harsh-nod/fe2o3/issues/281), and
[#282](https://github.com/harsh-nod/fe2o3/issues/282), inspected on 2026-09-17.
The integration base is `db3122b46a17a47b48cb0bb39d63aed7c1c7f280` **plus
uncommitted implementation changes**. It is not a release receipt or milestone
signoff. Original scope, including both initial architectures and whole-kernel,
resource, scheduling, and curriculum requirements, remains unchanged.

[First-slice commands and evidence boundaries](assembly-authoring-first-slice.md)
describe reproduction. The implementation below makes useful progress; no
umbrella milestone is declared complete by this document or by test counts.

## Exercised evidence and implementation anchors

| ID | Delivered portion and reproducible checks |
| --- | --- |
| E1: instruction/source contract | Shared [gfx942 validator](../crates/fe2o3-kernel-ir/src/gfx942_inline_assembly_v1.rs), [validator tests](../crates/fe2o3-kernel-ir/tests/gfx942_inline_assembly_v1.rs), and [simulator tests](../crates/fe2o3-kir-sim/tests/gfx942_inline_assembly_v1.rs). The real frontend admits six u32 VGPR markers through [V30 marker admission](../crates/rustc-codegen-fe2o3/src/production_inline_assembly_v30.rs), [exact occurrence custody](../crates/rustc-codegen-fe2o3/src/production_inline_source_occurrences_v30.rs), its [negative tests](../crates/rustc-codegen-fe2o3/src/production_inline_source_occurrences_v30_tests.rs), and [lowering tests](../crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/gfx942_inline_v30_tests.rs). The existing backend validator also describes SGPR move/i32 carriers; that is not additional source/simulator coverage. |
| E2: source compilation and CPU cases | Actual [typed-source fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30/src/lib.rs) and [source smoke](../scripts/assembly-authoring-v30-smoke.mjs): all six markers, seven distinct occurrence references, four words of 469 versus edited-constant 725, unchanged canaries. [Generated-helper roundtrip](../scripts/assembly-source-roundtrip-smoke.mjs): fresh source exports, retained helper call, 469 versus intentional OR-to-AND 0. [Ordinary bitwise fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/ordinary-bitwise-promotion-v1/src/lib.rs) and [promotion smoke](../scripts/ordinary-bitwise-promotion-smoke.mjs): ordinary Rust, unchanged generated helper, edited helper produce 469 / 469 / 0 with fresh identities and unchanged originals/canaries. These are independent differential cases, not universal equivalence proofs. |
| E3: inspection and explicit candidates | [Immutable authoring queries](../crates/fe2o3-source-isa-observation/src/multilevel_authoring_v1.rs) consume actual Bundle V6/KIR V11, with [selection/materialization tests](../crates/fe2o3-source-isa-observation/src/multilevel_authoring_v1_tests.rs). [Private source-edit proposals](../crates/fe2o3-source-isa-observation/src/source_edit_v1.rs), [conflict tests](../crates/fe2o3-source-isa-observation/src/source_edit_v1_tests.rs), [candidate CLI tests](../crates/fe2o3-source-isa-observation/tests/authoring_cli/source_candidate.rs), and [FD tests](../crates/fe2o3-source-isa-observation/src/bin/fe2o3_author/candidate_fs_tests.rs) cover exact bytes, stale selectors, invalid paths/spans, symlinks, collisions, bounds, retained-parent semantics, and post-publication errors. Candidate creation never replaces the original or compiles source. |
| E4: LLVM-text observation | [Complete-module example](../crates/fe2o3-amdgcn-model/examples/inspect_bundle_v6_llvm.rs), including six focused tests, and [actual-source LLVM smoke](../scripts/assembly-source-llvm-inspection-smoke.mjs) passed for baseline, unchanged helper, and edited helper. They retain templates, VGPR constraints, helper definitions/calls, and separate original/target-bound identities; malformed bundles reject without output. This is pre-ranked LLVM text, not independent final instruction decoding, an HSACO, or protected finalization. |
| E5: stopped resource observations | [Debugger projection](../crates/fe2o3-kir-debugger/src/resource_projection_v1.rs), [protocol](../crates/fe2o3-debug-protocol/src/resource_queries_v1.rs), and [CLI queries](../crates/fe2o3-debug-cli/src/resource_queries_v1.rs), each with sibling tests. [Real source resource smoke](../scripts/resource-query-v6-smoke.mjs) checks the first-write watchpoint, bytes/initialization, allocation/access paging, reverse-prewrite/no-future-access, and stale revision/token rejection. The actual resource is a global u32 output allocation, not LDS. [Scale harness](../scripts/resource-query-scale.mjs) and [tests](../scripts/resource-query-scale.test.mjs) measure 4/64/128 CPU invocations; 256 is explicitly skipped by the guard. |
| E6: companion site | In `fe2o3-kernels`: `ResourceMemoryView`, `ResourceAccessView`, `resource-memory-controller.ts`, their focused tests and capture-isolation tests; `SourceVariantComparison.tsx`, `source-variant-comparison*.ts`, and three `source-variant-*` test leaves. The immutable actual example `examples/ordinary_bitwise_promotion_v1.json` retains exact sources, generated helper, operation responses, CPU bytes, and hashes; its 14 focused tests pass. The exporter build is explicitly WIP with no retrospective compiler/release pin. Measured browser decode/projection and bounded viewport tests distinguish actual one-row pages from synthetic layout-scale input. Global site/publication gates remain separate. |

E2's actual captured successes were `assembly-v30-smoke-r1`,
`assembly-source-roundtrip-r2`, and `ordinary-bitwise-promotion-r1`; E4 was
`assembly-source-llvm-inspection-r2`. Their receipts scope results to those
working-build runs. Retaining their bytes later does not authenticate compiler
execution or retroactively qualify a committed release.

## #280: direct assembly

| Milestone | Delivered portion | Required remaining work |
| --- | --- | --- |
| M0: contracts and ownership | E1/E3 freeze a bounded marker, observation, and diagnostic materialization profile with disjoint owners. | Agree complete source/region/resource/ABI contracts, catalog and two-target coverage matrix, generation budgets, and pinned-worker inline-region versus whole-body prototype with all owners. |
| M1: real-source instruction slice | E1/E2 exercise source, canonical instructions, CPU results and negative operands; E4 observes actual LLVM templates/constraints. | Final encoding inspection and applicable production correspondence/admission remain unqualified. LLVM text is not final-code evidence. |
| M2: complete basic assembly kernels | Actual generated helpers and explicit concrete source variants re-enter the frontend (E2/E3). | Ordered regions, physical register ownership, labels/branches, full ABI metadata, broader helpers/specialization, whole bounded assembly kernels, and intended machine-contract roundtrips. Six independent SSA expressions do not implement an exact physical-register region. |
| M3: memory and synchronization | Existing ordinary-KIR checks remain in force around the marker slice. | Authored ISA memory/LDS/atomic/wave operations, address provenance, wait/scoreboard hazards, barrier/convergence/race obligations, descriptors, and applicable hardware checks. Ordinary surrounding loads/stores do not satisfy this milestone. |
| M4: matrix and second architecture | No new matrix or gfx950 authoring qualification in this slice. | Reviewed matrix/low-precision semantics/layouts, exact gfx950 profile/catalog/lowering and independently qualified target cases. gfx942 integer success does not narrow the original target matrix. |
| M5: debug and resource integration | Exact per-variant source references, logical instructions and stopped CPU resource observations (E2/E5/E6). | Allocator trace/lifetime replay, expansion and cross-stage transformation lineage, final-machine mappings and exact artifact association; ambiguous or absent mappings must stay unavailable. |
| M6: production and curriculum | Reproduction scripts, negative tests, draft lessons and independent small integer cases (E1–E6). | Full preceding interfaces, clean-checkout release gates, generated host, protected proof/artifact admission, target-matched hardware claims, complete planned lessons and publication coverage. |

## #281: resource visualization

| Milestone | Delivered portion | Required remaining work |
| --- | --- | --- |
| V0: contract and inventory | Exact stopped-state/query bindings, bounded pages, availability labels and measured CPU/browser budgets (E5/E6). | Complete backend/capture capability matrix and shared compiler/physical-resource handoff; these are not inferred from a rendered page. |
| V1: recorded memory visualization | Real source-produced global allocation/access pages, bytes/initialization, captured-page logical-scope filtering (E5/E6). | Actual source-produced workgroup/LDS example, broader allocation/access overlays and supported event/lane navigation. Producer generation 0 is not allocation-lifetime reconstruction. |
| V2: debugger integration | First-write watchpoint, forward/reverse stopped queries, no-future-access and stale cursor/token checks; separate interactive raw-KIR capture remains available. | Same-source loops/helpers/reuse lifecycle qualification, faults, linked source/SSA navigation and UI watchpoint/breakpoint editing/live query transport. The generated-helper CPU smoke is not a resource-history test. |
| V3: compiler resource views | Exact original/edited source comparisons and independently retained logical observations (E2/E6). | Compiler allocation lifetimes, plan-versus-final resources, physical registers and checked cross-level lineage. Call rows currently lack a callee target; the viewer does not infer call edges or cross-variant SSA correspondence. |
| V4: target analysis and live adapters | No new hardware-session or bank-conflict model qualification. | Target-bound bank/transaction models, independent references and actual same-stop hardware adapters; unavailable physical state remains unavailable. |
| V5: tutorials and qualification | Draft tutorials, source/variant-bound observations, focused UI tests and separated actual/synthetic scale measurements. | All planned lessons, complete linked-selection/accessibility/scale coverage, import/export/service documentation, aggregate desktop/mobile and evidence gates, and publication. Browser responsiveness is not GPU performance or capture-overhead qualification. |

## #282: multi-level authoring

| Milestone | Delivered portion | Required remaining work |
| --- | --- | --- |
| U0: editing and ownership contract | Bounded exact selectors, stale-source checks, private proposals, explicit new-file publication and no snapshot-resume authority (E3). | Complete per-level capability matrix, semantic extraction/insertion ownership, specialization/resource boundaries, recipe semantics and shared acceptance/budget agreement. A source-map file identity is not a source-byte hash. |
| U1: inspect and select | Actual source-produced V6/KIR V11 operations, contiguous single-block regions, live-ins/outs, scalar detail/source references and immutable comparison (E2/E3/E6). | Genuinely available additional stage/transform lineage and resource contracts, checked cross-level navigation, remaining stale/ambiguous cases. Missing stages are not synthesized. |
| U2: promote, edit and recompile | Supported u32 bitwise/typed-marker drafts, actual ordinary-source promotion, fresh unchanged/edited helper exports, CPU cases, and LLVM-text observation (E2–E4). | General supported source-boundary integration, applicable exact instruction/resource contract preservation and final-code inspection, invalid-resource/hidden-clobber cases, fresh final analyses/admission. EOF helper insertion alone is not semantic source replacement or proof of valid Rust. |
| U3: replayable schedule recipes | Existing fixed-policy optimizer and non-executable schedule shell are prerequisites, not delivered recipes. | Two legal schedules of one source algorithm, compiler-owned applicability/anchors, deterministic checked replay, source-edit success and stale/ambiguous rejection, explicit rebind, fresh transformation/analysis records. CPU interleaving schedules are not compiler recipes. |
| U4: end-to-end qualification | Exercised small-kernel source cases, exact retained comparisons, scripts and draft lessons. | Qualified U1–U3 including tiled compute, original target/operation matrix, final artifact/resource comparisons, clean-source full gates, inspection/materialization/recipe cost measurements and normal compiler/site pinning. Resource-query timing alone does not satisfy all authoring budgets. |

## Concrete dependencies and next owner handoffs

1. **Source-output correspondence and protected production — #271 owners.** On
   the inspected integration base,
   [`normalize_kir_expression_inner_v1`](../crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1.rs)
   handles modeled ordinary expressions but has no `InlineAssembly` relation;
   [`GpuSemanticExpressionResolverV2` / `exact_reaching_assignment_v1`](../crates/rustc-codegen-fe2o3/src/production_ranked_projection_v1.rs)
   cannot resolve call-produced scalar values through an exact callable roster.
   The [private-array output census](../crates/fe2o3-lower-mir-kernel/src/production_source_output_private_array_census_v1.rs)
   also refuses assembly. Agree a closed, independently checked source/KIR
   relation and call-result transport with these owners; do not replace authored
   instructions with ordinary arithmetic to evade the boundary.
   [`execute_protected_reproducible_first_build_worker_v3`](../crates/fe2o3-hsaco-finalize/src/first_build_worker_v3.rs)
   requires a consumed compiler V3 handoff and exact publication receipt/closure;
   a diagnostic V6 bundle or LLVM string cannot supply them.

   These local gaps do **not** mean that protected proof execution is unavailable
   everywhere. The separate [safe-launch owner finding](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5722361530)
   describes the missing complete backend composition; a
   [later actual protected-proof result](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5722799642)
   passes ordinary functional proof and then refuses dynamic-launch total-output
   ownership. That requires a symbolic TotalView coverage theorem and an
   authenticated extent premise, not weaker ownership checking. Those owner
   changes/results are not claimed as merged or qualified by this authoring slice.

2. **Checked scheduling — #134/#271/#272 owners.** The
   [#134 acceptance addendum](https://github.com/harsh-nod/fe2o3/issues/134#issuecomment-5720288650)
   assigns U3 to checked schedule parameters within the fixed policy, not
   caller-selected passes. [`dialect-schedule`](../crates/dialect-schedule/src/lib.rs)
   supplies bounded `PlanOp`/`PlanType`/`ParametersAttr`, not applicability or
   production materialization. Existing
   [`optimize_checked_canonical_kernel_ir_policy3_v1`](../crates/fe2o3-kernel-opt/src/checked_optimization_policy3_v1.rs)
   and [published relation replay](../crates/fe2o3-kernel-opt/src/checked_optimization_policy3_receipt_v1.rs)
   must retain their real execution-versus-relation distinction. An inert recipe
   DTO would not close the gap.

   The [exact pre-owner handoff](https://github.com/harsh-nod/fe2o3/issues/272#issuecomment-5720236118)
   requires authenticated callback/capture/exit regions and immutable schedule
   bindings before both canonicalizations in
   [`try_materialize_origins_inner_v1`](../crates/fe2o3-lower-mir-kernel/src/production_pre_ranked_v1.rs).
   Reuse private
   [`with_checked_call_site_v1`](../crates/fe2o3-lower-mir-kernel/src/production_call_assembly_v1.rs)
   under its owner; the later V12-inventory call view would be circular here.
   Independently check versioned CFG/scope/distribution expansion and cumulative
   budgets before minting custody. `ScopeEnd` is not a GPU barrier.

3. **Qualification and publication — #275 plus production/runtime owners.** The
   [complete-output gate status](https://github.com/harsh-nod/fe2o3/issues/275#issuecomment-5720631827)
   explicitly leaves M0 partial, M1–M6 incomplete and qualified SIMT/tile pairs at
   zero in that checkpoint. Complete-output CPU comparisons do not close its
   source/epoch producer, numerical, generated-host, proof or hardware gates.
   Keep companion lessons draft until their exact compiler/site evidence and
   ordinary publication requirements are met; do not retrofit WIP observations
   into release attestations.

## Work that can proceed in parallel

The hard boundaries above do not block every lane. With disjoint owner approval:

- Extend real-source negative/differential cases and bounded query/candidate
  tests without inventing proof authority; measure inspection/materialization
  independently of resource-query performance.
- Add actual admitted source resource cases for LDS, loops/helpers, faults and
  reuse where the existing backend can provide them; keep any specific producer
  gap explicit. Improve captured-view navigation/accessibility independently of
  live hardware and physical-register support.
- Develop the pinned ISA catalog/coverage inventory, exact-region ABI/resource
  contract and worker emission prototype under #280 owners. gfx950/matrix work
  can proceed once its target contract is frozen, without narrowing M0–M6.
- Design the two-schedule acceptance cases and stale/rebind negatives alongside
  the checked pre-owner scheduling implementation. Do not add a second executable
  graph, alternate pass language, snapshot-to-production path or finalizer.

Each lane still needs its own reviewed contract, bounded implementation and
actual qualification; difficulty is not a blocker and partial delivery is not
completion.
