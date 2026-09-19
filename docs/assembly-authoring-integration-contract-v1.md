# Assembly authoring integration contract: owner-review proposal v1

Status: **proposed, not accepted**. Tracking: [#280](https://github.com/harsh-nod/fe2o3/issues/280)
M0, [#281](https://github.com/harsh-nod/fe2o3/issues/281) V0 and
[#282](https://github.com/harsh-nod/fe2o3/issues/282) U0. Assessment base:
compiler `70b3fe0057e18e16eaa568301d2743b1ca6c252a`; published site
`55bd4500af7308c96ea6c2158ffb31a2a4e5b236`. Historical captures keep their own
exact source, build, compiler and site identities. This document allocates no
wire identifier, source path, worker lease, hardware slot or authority.

The proposal preserves every original exit and both initial architectures.
Pending whole-body, gfx950, memory/synchronization, matrix, physical-resource,
source-promotion, recipe and curriculum work is not removed. Contract acceptance
does not complete later implementation milestones. Tests or silence cannot
substitute for the explicit reviewer decisions at the end.

## 1. Architecture and unresolved emission gate

One ordinary Rust source/dependency closure is the executable input. Authenticated
frontend operations enter existing semantic MIR/SSA, canonical KIR, checked
transformation/analysis and target lowering. Existing compiler custody, proof,
worker, finalizer, artifact and direct-KFD admission remain authoritative.

| Source mode | Proposed mechanism / existing evidence | Required boundary |
| --- | --- | --- |
| Compiler-allocated typed instruction | Existing structured operation to constrained LLVM inline assembly | SSA operands; complete constraints/effects; no promise of fixed registers or whole-body order |
| Mixed exact ordered region | One structured region to one constrained multi-instruction LLVM unit; bounded gfx942 experiments and source qualification exist | Preserve authored internal order/encodings/roles; separately describe permitted compiler boundary moves and surrounding code |
| Fully author-owned kernel body | Prefer compiler-generated assembly through the existing worker's integrated assembler/MC; module assembly is only a candidate transport | Unresolved: current module-assembly-only body fails the LLVM-function export gate; no supported whole-body transport is selected by this document |

The last mode would bypass LLVM SSA instruction selection/allocation/scheduling
for that body, not typed source/KIR checking, assembler/object/link machinery or
normal final admission. Unchecked strings, arbitrary objects, dummy definitions
to evade export checks, or a second build/launch path are not acceptable.

M0 emission acceptance must include the existing pinned-worker positive inline
and negative whole-body experiment, then an owner-reviewed working whole-body
mechanism decision/prototype: canonical input and deterministic emitted bytes,
real export/launch/ABI metadata, debug association, normal worker handoff and
independent final decoding. An unresolved transport cannot be marked supported.
General whole kernels and branches remain M2 implementation, not an additional
M0 completion claim. See [the prototype](ordered-inline-region-prototype-v1.md).

Proposed compile-time generation rule: ordinary typed Rust macros/constant
construction may expand only to the admitted bounded structured instruction
sequence. Independently revalidate results in the authenticated frontend; macro
checks are not custody. Retain expansion/source occurrences and charge cumulative
growth/work/storage before expansion. No separate scripting frontend, unchecked
runtime-generated assembly or producer-selected proof bypass is introduced.

## 2. Target, stage and incorrect-behavior coverage

Table meanings: **existing** identifies the stated narrow implementation/evidence;
**pending** is required work, not an implicit supported cell. Proof of a scalar
relation, native decoding and protected artifact admission are separate stages.
The catalog is metadata, never an executable legality/effect specification.

| Exact target / authored family | Catalog and source/canonical admission | CPU / analyses | LLVM / final-machine evidence | Proof, production and hardware |
| --- | --- | --- | --- | --- |
| gfx942:xnack-/wave64, six u32 SSA markers | Existing six-family overlay; standalone MIR34 plus explicitly versioned historical captures | Existing wrapping/bitwise model and scoped source cases | Existing inline lowering; historical LLVM observations retain their pins | Scoped source-reference proof evidence exists; complete ownership/artifact/GPU qualification not supplied by it |
| Same target, fixed XOR/ADD pair | Existing MIR31/KIR16, five distinct VGPR roles | Existing atomic-region logical execution and refusals | Existing O0/O3 actual-source pair decoding/register/descriptor checks | Diagnostic path; no production-resume or GPU authority |
| Same target, 1-16 integer program | Existing MIR32/KIR17; mov/add/sub/and/or/xor | Existing definite initialization, 36 source CPU cases and logical debugger capture | Existing 12 actual-source native compilation cases / 80 authored sites | Complete protected ownership/final admission and GPU qualification pending |
| Same target, general regions/helpers/control flow/whole bodies | Broader source/ABI/resource contract and admission pending | Branch/loop/lifetime and complete semantics pending | Whole-body transport and general final-code checks pending | Pending |
| Same target, authored memory/LDS/atomic/wave/synchronization | Catalog metadata exists; authored operation/effect/provenance overlay pending | Pending waits/scoreboard, races, convergence and scope models; surrounding ordinary Rust checks already exist | Authored lowering, encoding and descriptor qualification pending | Pending; ordinary memory captures are not authored-ISA qualification |
| Same target, matrix/low precision | Metadata exists; exact operation/layout/numerical profile pending | Independent numerical/layout semantics pending | Matrix instruction/resource correspondence pending | Target-matched qualification pending |
| gfx950, six integer markers and ordered regions | CDNA4 metadata exists; exact features/wave/launch profile and executable overlay pending | Pending; no inheritance from gfx942 | Pending | Pending |
| gfx950, whole bodies/helpers/branches | Metadata is not admission; source/ABI contract pending | Pending | Working transport and final decoding pending | Pending |
| gfx950, memory/LDS/atomic/wave/synchronization | Metadata exists; exact operation/effect/capability profile pending | Pending target hazard/scope/convergence models | Pending | Pending |
| gfx950, matrix/low precision | Metadata exists; encoding/layout/numerical profile pending | Pending independent references | Pending | Pending |

Owner review must freeze the exact gfx950 feature/wave/launch profile; bare
catalog selection `gfx950` does not do that. Per-family modifiers, literal forms,
tuple widths/alignment, AGPR/SGPR/VGPR/special-state roles and architecture
availability need reviewed overlays. Retain the pinned upstream archive, schema,
input/output/overlay hashes and attribution in [the existing catalog](amd-isa-spec-catalog-v1.md).

| Incorrect behavior | Required detector / acceptance negative | Current limitation |
| --- | --- | --- |
| Wrong target/opcode/form/operand/register tuple/immediate | Frontend + canonical operand/effect overlay; malformed/unsupported source and decoded-word controls | Current executable coverage is the declared integer subset, not every catalog entry |
| Uninitialized register, overlapping live range, stale handle, hidden helper scratch | Definite assignment plus physical ownership/liveness at entry/exit and CFG joins | Closed five-role profile is checked; general lifetime/loop/helper analysis pending |
| Undeclared EXEC/VCC/SCC/M0 or ABI mutation | Implicit use/def, save/restore and boundary contract; hidden-clobber negatives | Current region reads EXEC only; no general special-state support inferred |
| Bad pointer relationship, bounds/alignment/permissions, tail mask, uninitialized LDS | Existing memory/capability checks extended to exact instruction semantics; full-output/init/canary and failing-access cases | A scalar address or successful ordinary store does not prove authored memory safety |
| Missing wait, pending destination reuse, memory ordering error | Target pending-operation/scoreboard model with exact order, independent negative and applicable hardware check | Immediate-completion sequential simulation cannot detect unmodeled asynchronous hazards |
| Divergent barrier, invalid collective participation, race/atomic scope | Scope/convergence/effect analysis plus admitted simulator schedules | Wait counter is not a barrier; one schedule is not universal race freedom |
| Wrong matrix mapping, precision, overflow/rounding or numerical drift | Exact per-target lane/layout/numerical policy and independent reference | Integer wrapping must not silently replace checked Rust arithmetic; matrix policy pending |
| Optimizer changes authored order/registers/encoding; ABI/descriptor mismatch | Independent final decoding + typed subject/operand/order/relocation/descriptor reconciliation | Permitted prologue/epilogue/boundary moves are separate; no whole-file byte-identity promise |
| Stale source/map/recipe/capture, equal-valued SSA substitution | Exact immutable identity/custody, independent joins, stale/ambiguous refusal tests | Equal values, names, paths or source lines are not identity |

## 3. Existing identities, schemas and admission boundaries

Reuse these owners and version-qualified contracts; this proposal introduces no
competing trace, compiler graph, bundle or materialization authority format.

| Existing contract | Retained boundary / coordination rule |
| --- | --- |
| MIR30/intrinsic87 | Published saturation grammar; not former scalar-authoring MIR30 bytes |
| MIR31/intrinsic88, terminal133; KIR16/op38 | Frozen ordered-pair profile, including its historical scalar87 grammar |
| MIR32/intrinsic89, terminal134; KIR17/op39 | Frozen bounded-program profile; not a numeric upgrade of every other feature |
| MIR33/intrinsic90, terminals135-137 | Peer Wave64 ownership; preserve unchanged |
| MIR34/intrinsic91, terminals138-143 | Coordinated standalone scalar marker allocation; preserve version-qualified tails |
| BundleV6/KIR11 + SourceMapV2 | Existing immutable authoring observation/selection/materialization input; not compiler resume |
| `fe2o3-multilevel-authoring-observation-v1` | Existing level/action matrix and exact selectors; no caller-created trusted operation handle |
| DebugProtocol snapshot/cursor and resource request/response V1 | Existing stopped state and bounded allocation/access projections; decimal byte extents, exact revisions and opaque tokens |
| SemanticTrace/Query, source/ISA catalog and #215 production adapters | Reuse each admitted version's actual identities/capabilities; do not relabel frozen older envelopes for newer KIR |
| Diagnostic census and private retained browser projections | Correlate measured source/capture content; no producer authentication, replacement custody or executable authority |

Compatible decoding is explicit, never a version-number maximum. Current private
diagnostic KIR16/17 consumers are not new simulation-bundle or protected-artifact
routes. Every future schema/terminal/handshake change requires its owner's exact
allocation and compatibility controls. Keep source, KIR, map, request, target,
policy, transformation, artifact and capture identities separate where supplied;
missing identities remain unavailable, not computed from similarly named data.

## 4. Source promotion, resources, ABI and recipes

The existing four-level/seven-action `AuthoringCapabilityV1` matrix is the starting
point. Proposed publication expands its documentation by target/operation family;
it does not declare unavailable stages implemented.

| Level | Read/select today | Materialize/edit/re-admit boundary |
| --- | --- | --- |
| Ordinary structured Rust | Existing source locations and retained diagnostic attribution | External source edit; semantic ownership must come from compiler binding, not location overlap |
| Scheduled/tile Rust | Unavailable in this authoring snapshot | Pending owner-defined schedule/materialization contract; no recovered generic source |
| Canonical SIMT V11 observation | Bounded immutable operation pages and contiguous single-block regions | Supported u32 bitwise/typed-marker helper drafts; explicit source integration and fresh frontend compilation |
| Exact physical-register region/program | Narrow source/diagnostic profiles above and declared plans | No general allocator/lifetime/whole-body materializer; closed graph planner is not checked source replacement |
| MIR, separate neutral/target KIR, LLVM and final ISA | Only actual retained stage views/associations; identity alone is not a readable body | Read-only unless that stage has a separately admitted structured materializer; no arbitrary optimized-LLVM pasteback |

Simulation/inspection are separate existing admitted interfaces, not consequences
of editing permission. For every row and target/family, reviewers must adopt all
seven explicit action states and unavailable reasons rather than infer support.

Proposed source ownership contract:

1. Bind exact original file bytes/revision, compiled source/dependency closure,
   kernel/instance/specialization, selected stage/owner and target/policy. Preserve
   original UTF-8 offsets versus normalized compiler offsets and expansion origin.
2. The compiler establishes an unambiguous typed source expression/statement
   boundary and SSA live-in/out binding. Preserve all same-span occurrences;
   a constant and OR attributed to `low | 256` are not one owned operation.
3. Extract only an admitted complete region: types, aliases, control entry/exits,
   memory/provenance/convergence/trap effects, implicit state, helper closure and
   resource ownership. Reject unsupported escape, mutable alias, hidden clobber,
   ambiguity and unproved distribution/scope changes before producing a candidate.
4. Materialize actual typed Rust, with explicit inputs/results and deterministic
   names. Concrete monomorphizations retain their exact types/const arguments;
   do not claim generic recovery or edit every specialization implicitly.
5. The first editing action creates a reviewed candidate, not an in-place rewrite.
   Revalidate source bytes, selection/proposal and retained descriptors; publish
   to an explicitly selected new path without replacement. Existing safe I/O is
   not a compare-and-swap proof for a changing source tree or semantic insertion.
6. Fresh source compilation recomputes identities, analyses, source maps, proofs,
   descriptors and applicable final checks. Failure cannot present a baseline
   artifact/capture as the edited result. Source restoration is not assembly
   decompilation; serialized edited intermediates have no production-resume right.

Initial extraction proposal is a bounded direct-root u32 expression/initializer,
with exact compiler-bound parameters and no escaping intermediate. The active
ordinary bit-select feasibility uses the normal **V8 owner**, not the separate
V11 observation API. Its proposed diagnostic candidate enters MIR32/KIR17 only
after fresh compilation. Working feasibility/candidate tests must have separate
receipts; neither U1 attribution nor candidate I/O establishes production U2.

Resource/ABI contract to review with #272/#280/#145/#146: compiler-allocated SSA
and physical reservations are different. Reserve every live-in/out, scratch,
tuple and implicit clobber against surrounding code; define entry/exit and
branch/loop join obligations. Handles may not escape or survive release/reuse
without a producer-owned generation/lifetime. Helpers declare all scratch/state.
LDS uses workgroup/epoch-branded allocation identity, extents/alignment/init and
layout facts; absent physical bases cannot be inferred. Independently reconcile
target/wave/workgroup, kernarg layout, SGPR/VGPR/AGPR use, static/dynamic LDS,
private segment, stack/calls, symbols/termination and permitted relocations with
the emitted descriptor. The experimental fixture ABI is not a universal ABI.

Recipe proposal: extend #134 D4's existing non-executable schedule parameters,
not caller-selected pass lists or a second transform engine. Bind source/kernel/
instance, target, exact fixed policy/version, semantic anchors, applicability,
resource/numerical preconditions and bounded parameters. Resolve under #271/#272
custody before the affected canonicalizations; use a separately reviewed fixed
composition, never relabel an altered Policy3/Policy6/other policy identity.
Existing checked local ordering on V12 is a reusable implementation primitive,
not a persistent recipe. Its invocation-local ordinals cannot survive edits.
Use `(a ^ b) & (c | d)` to propose two distinct ready orders; the bit-select
dependency chain does not supply that test. A real source change must exercise
valid replay, stale/ambiguous rejection and explicit rebind with fresh evidence.

## 5. Shared capture, selection and producer/consumer contract

Reuse [#215 architecture](debugger-profiler-architecture-v1.md). Record declared,
proved, observed (named producer), inferred (retained rule/inputs), or unavailable
per fact. Zero/inactive differs from not captured/unsupported/stale/truncated.

| Producer / existing input | Available consumer facts | Explicit missing facts / owner |
| --- | --- | --- |
| Ordinary source BundleV6/KIR11 debugger | Global allocation/access pages, stopped bytes/init, logical scope and exact source-bound snapshot | Per-access source/frame/occurrence and lifecycle depend on actual producer; #216/#215 |
| Ordinary LDS BundleV5, single/two workgroups | Real logical allocations/ranges, accesses, reverse-restored inventory and checkpoints | Generation0 is not release/reuse, physical LDS base or bank behavior; #216/#272 |
| Diagnostic KIR16/17 | Declared instruction/register plan; selected lane0 whole-operation inputs/results and reverse/repeat | Scratch values, instruction microsteps, physical registers/EXEC/lifetimes unavailable; #280/#216 |
| Compiler snapshot/source census/source-ISA producer | Exact admitted stage facts, source attribution and final associations where produced | Attribution is not semantic edit ownership; missing MIR/LLVM/ISA bodies/mappings stay unavailable; #271/#280 |
| Native descriptor/decoded instruction observation | Exact case-specific static resource capacity and authored instruction facts | Not same-stop register values or an automatic join to CPU capture; #145/#146/#280 |
| Admitted KFD/ROCgdb/profiler/ATT adapters | Only each admitted backend's version-qualified facts and exact origin | No generic GPU register/value/dispatch/source identity inference; #215/#137/#182 |
| Site bounded retained adapters | Display checked original responses and both variant identities, without mutation | No live query, load, resume, attach, instrumentation or recapture authority |

Selection is atomic: capture/run/configuration plus artifact/KIR/map/stage where
present, target/policy/variant, event cursor and state revision, workgroup/wave/
lane, exact operation occurrence and call activation where produced. Source line,
timestamp, equal address, depth or static KIR site is not a dynamic identity.
Never manufacture absent axes. Current runtime identity extensions need their
own versioned producer/query acceptance and real loop/helper/fault fixtures.

The consumer independently binds selected request/range/allocation/generation
and complete anchor; changing input/selection invalidates in-flight and retained
responses. Reverse/repeated cursors keep distinct revisions. Memory is exact
checkpoint bytes plus initialization, not values reconstructed from accesses.
Use allocation-relative half-open byte ranges and lossless integers; preserve
per-lane accesses before aggregation. Access overlays never change checkpoint
time or paint another generation/workgroup. Movement arrows require real dataflow.
Keep partial/empty-with-continuation/truncated distinct from complete absence.

Compiler mutation belongs to the explicit authoring service, never cursor moves,
hover or import. Compiler transformation history, CPU replay and GPU time remain
separate. New physical/lifetime data is an owner-produced extension, not a UI
allocator. Frozen envelopes and shared selection/service roots need explicit
owner handoff before parallel edits.

## 6. Reusable acceptance fixtures and shared inventory proposal

These are proposed roster entries, not new inventory IDs or qualification labels.
Use existing `config/tutorial-kernel-manifest-v1.json`, #275 SIMT/tile obligations
and site source/evidence gates. Assembly/promoted variants are additional; the
integrator and tutorial owners must agree exact entries and source/target pins.

| Reuse / proposed addition | Required acceptance and current boundary |
| --- | --- |
| `assembly-authoring-v30` fixture and existing source/roundtrip scripts | Six operations, base/edited constants, helper re-export, complete independent outputs/init/canaries; historical name does not change current standalone wire version |
| `ordinary-bitwise-promotion-v1` fixture | Ordinary OR, unchanged generated helper and deliberate OR-to-AND candidate; source immutable, fresh identities, stale/unsupported controls |
| `ordered_region_v31.rs` / `ordered_program_v32.rs` extraction fixtures | Used/unused result, exact operand/role/order/encoding, 1/3/16 steps, independent input oracles; source/CLI/decoded mutation refusals |
| Existing actual LDS `workgroup_reduce_u32` source/captures | Single/two-workgroup bytes/init/accesses and reverse isolation; no lifecycle claim |
| Source census + immutable navigation | Same-span constant/OR ambiguity, exact selected boundary, unavailable stages, stale/capture replacement; no source ownership inference |
| Proposed parameter-only bit-select candidate | Actual typed source binding, alias/ambiguous/normalized-offset refusals, create-new preservation, fresh-source semantic/machine contract checks |
| Proposed two-schedule ordinary expression | Two distinct legal schedules, source edit, valid replay/stale ambiguity/rebind; existing model-only local-order cases are controls, not source evidence |
| Required tiled-compute and gfx950 counterparts | Freeze concrete source/target/numerical/profile owners; pending dependencies remain required, not replaced by scalar or conceptual lessons |
| Required authored memory/sync/whole-body/matrix cases | Add independent missing-wait, divergence/race, resource lifetime/clobber, ABI/encoding and numerical negatives with their admitted producers |

Retain original/promoted/edited complete source closure, exact features/arguments,
expected successful/refused stage, independent oracle, full output/init/canaries,
raw diagnostic and final-code evidence where applicable. A preflight refusal is
not an execution fault; synthetic parser/layout controls are not real captures.
Record compiler and site commits separately. No FE2O3_PIN, lesson maturity,
curriculum route or GPU-qualified status changes through this proposal.

## 7. Baselines, proposed budgets and cancellation

Measured resource baselines are in the site's `docs/resource-memory-windows.md`:
mi350-2 EPYC 9534, 128 logical CPUs/Linux 5.15.160+, Node 22.22.3,
Chromium 151.0.7922.34, 1280x800, warmed development Vite/no StrictMode.
Query measurements: 3 warmups/20 sweeps; browser: 5 warmups/30 samples.
No generation/materialization/recipe budget measurement is claimed here.

| Retained measured sample | Observed p95 / memory |
| --- | --- |
| Actual 4/64/128 invocation page roundtrip | 0.581/0.664/0.652 ms |
| Corresponding complete query sweeps | 0.715/6.404/12.691 ms; whole debugger peak RSS 16.10/20.67/27.27 MiB |
| Actual allocation/access pages | Guard 0.4 ms; mount+commit+synchronous-layout 4.2/5.0 ms |
| Synthetic 256-row/64-rendered-row layout | Guard 4.6 ms; mount/layout 26.0 ms; not execution evidence |

Those timings predate current navigation/overlays; they exclude async paint/GPU
work and are not new release, browser-memory, import or reverse-interaction
measurements. Source/native/debugger correctness receipts are not timing samples.

The following are **proposed acceptance ceilings**, not implemented defaults or
passed gates. Owner review must approve/revise them and freeze exact fixture,
source/tool/browser/hardware identities and measurement scripts before closure.

| Workload / measured boundary | Proposed budget | Baseline status |
| --- | --- | --- |
| Offline pinned two-architecture catalog regeneration, inputs already present | p95 wall 30 s; whole-process peak RSS 512 MiB | Unmeasured under this policy |
| Closed 1/3/16-step generation+validation; separately maximum supported source generation | p95 warm 250 ms/stage; incremental retained storage 64 MiB | Unmeasured; report cold process/source compilation separately |
| Immutable inspection/selection and bounded source materialization | p95 warm 250 ms/action; incremental retained storage 64 MiB; existing source/report ceilings unchanged | Correctness exists; timing/storage baseline required |
| Proposed persistent recipe resolve/apply/replay at initial 64-op profile | p95 warm 500 ms/action; incremental retained storage 128 MiB | Source recipe implementation/baseline pending |
| Resource query page / complete named sweep | p95 25 ms/250 ms | Existing advisory baseline above; changed profiles require new measurement |
| Retained display import ≤2 MiB, including parse/hash/guard | p95 250 ms; browser heap delta 128 MiB | Unmeasured end-to-end; narrower adapter caps still apply |
| Presentation guard / actual page render / synthetic stress render | p95 25 ms/100 ms/250 ms | Historical diagnostic baselines above, current UI requalification pending |
| Current selection/page/reverse-checkpoint action through committed display | p95 100 ms; ≤64 rendered access rows and existing bounded memory viewport | Functional bounds exist; current timing baseline pending |
| Cancel superseded display work / cancel bounded authoring request | ≤100 ms invalidation / ≤1 s acknowledgement at cooperative boundary | Cancellation measurement/contract acceptance pending |

Use 5 warmups/30 retained measured repetitions for new latency distributions;
retain every failure/outlier and separately report cold/warm cache cost, input
size, encoded output, logical retained bytes and process RSS/browser heap. Do not
substitute logical ledger charges for RSS. Cache keys bind source/instance/target/
policy/schema and generator/overlay identities; edits invalidate affected caches.
Bound recursion/growth, scans, outputs and deadlines before allocation/work.

Existing safety ceilings remain independently binding: authoring pages 64,
regions 64 operations/256 values, emitted source 64 KiB, reports 256 KiB; resource
pages/scans 256; each importer retains its own byte ceiling. Proposed budgets do
not enlarge them. Cancellation leaves no success receipt or partially published
candidate; supervise only owned child groups, preserve original/existing files,
and label any atomic publication completed before cancellation explicitly.
Browser stale-result suppression is not proof that computation was cancelled.
Compiler/native/GPU subprocess budgets require their owners' separately named
fixtures and existing supervision; no timing ceiling relaxes custody or checks.

## 8. Review, ownership and milestone acceptance checklist

Each acceptance record must name this document's exact commit/digest, reviewer
role/thread, decision, supported/pending cells, exact dependency base, owned and
shared paths, lease/expiry if starting implementation, fixtures, schema allocation
and measured budget receipt. A draft, acknowledgement of non-overlap, expired
lease or absence of objections is not technical acceptance.

| Review owner | Required decision; relevant existing thread |
| --- | --- |
| #134 / #271 | Source levels, complete custody and fixed policy/recipe applicability; [roadmap addendum](https://github.com/harsh-nod/fe2o3/issues/134#issuecomment-5720288650) is not detailed contract approval |
| #272 / #280 | Source generation, typed extraction/specialization, memory/scope/ABI/resource contract and exact target/operation overlay |
| #145 / #146 and LLVM/finalizer owners | Working inline/whole-body mechanism, typed transaction/export/debug metadata and independently decoded final contract |
| #216 / #215 | Simulator coverage, availability, dynamic selection identity and producer/query/schema handoff; no competing trace or invented physical state |
| #275 / site#3 | Exact shared positive/negative source/capture roster, SIMT/tile/additional-assembly inventory, tutorial and budget profile; [existing curriculum handoff](https://github.com/harsh-nod/fe2o3-kernels/issues/3#issuecomment-5720288788) remains the baseline |
| Integrator + independent reviewer | Exact allocation/base compatibility, disjoint write sets, independent acceptance/mutation matrix, measured budgets and publication boundaries |

Known narrow coordination: [MIR34 allocation](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5733660887)
is explicitly ownership-only; [additive phase11 acknowledgement](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5737886663)
preserves peer collector/pipeline hooks. Neither approves this entire proposal.

- [ ] M0: reviewed architecture/source-generation/exact-region/whole-body prototype
  decision; source extraction/materialization/reimport and ABI/resource contracts;
  complete operation/target/bug matrix; shared observations; exact base/allocations
  and disjoint writes; representative source positives/refusals; agreed measured
  generation/validation/cache/inspection budgets; every required owner review.
- [ ] V0: accepted capture/backend/capability inventory and resource projection;
  atomic selection/provenance/producer handoff; real captures and missing data
  identified with exact versions; fixture/tutorial ownership; measured load,
  memory, interaction/paging/reverse budgets and relevant owner agreement.
- [ ] U0: accepted per-level/action/target/family matrix; source ownership,
  extraction/specialization/materialization/stale-conflict contract; fixed-policy
  recipes and identity/evidence invalidation; bounded snapshots and mutation
  separation; independent positive/negative fixtures and disjoint writes;
  authoring budgets; source promotion first, no raw production resume.

Parallel work after the relevant acceptance: catalog/overlay leaves, source and
materializer leaves, semantics/hazard tests, worker prototype, read-only resource
adapters, recipe feasibility, tutorial drafts and independent review can proceed
on disjoint paths. The integrator alone coordinates shared codecs/IDs, collectors,
canonical/session roots, registries, manifests/lockfiles, finalizer, site routes/
selection/pins, serialized aggregate builds and publication. No lane acquires
another owner's path or executable authority from this checklist.
