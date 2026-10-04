# Assembly-authoring contract and status refresh — 2026-09-22

Status: **phase17's narrow public API is accepted and published; phase18 has
passed its bounded compiler, native and browser gates, not broad milestone signoff**.
Publication identities and remote-main verification are recorded in the issue handoff.
This additive refresh preserves the historical
[September 20 review](assembly-authoring-contract-review-20260920.md), the
[integration contract](assembly-authoring-integration-contract-v1.md), and
the original exits in [#280](https://github.com/harsh-nod/fe2o3/issues/280),
[#281](https://github.com/harsh-nod/fe2o3/issues/281), and
[#282](https://github.com/harsh-nod/fe2o3/issues/282). It creates no schema,
policy identifier, compiler authority, resource quota or acceptance waiver.

## Accepted public baseline and current scope

The [phase17 primary integration decision](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5770232406)
accepted the bounded Linux in-process source-promotion API and normal dependency
placement. It did not attribute approval to other owners or close M0/U0/U2.
The published baselines are compiler
[`431fa35b`](https://github.com/harsh-nod/fe2o3/commit/431fa35b6795e5170ae74858da8b2416002a8717),
mirror [`e9d98e62`](https://github.com/powderluv/fe2o3/commit/e9d98e62254b095b29da50e1904a83ab1a6d9c79),
and site [`cdd6b7d9`](https://github.com/harsh-nod/fe2o3-kernels/commit/cdd6b7d95631743a88a665c57d6e0e61d6aeadcd).
Statements in older reviews that this exact public API is still unapproved are
historical, not the current decision. Broader contract decisions remain open.

The [phase18 compiler scope](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5779836118)
and [site integration addendum](https://github.com/harsh-nod/fe2o3/issues/281#issuecomment-5780554571)
bound the present work. Genuine current HIR/semantic/KIR ownership, exact target
and launch checks, retained-source custody, and fresh frontend admission remain
mandatory. Observation JSON is not compiler authority. No private recipe is
ungated; no production worker/finalizer/runtime/KFD path is changed.

## Phase18 observations at this checkpoint

The [dated evidence record](evidence/authoring-source-values-20260922.md) retains
the exact input, tool, report and command domains. The following are distinct
runs, not a synthesized source-to-protected-artifact receipt.

| Observation | Actual result | Qualification boundary |
| --- | --- | --- |
| Live-prefix promotion, final r5 | Normal public consumer seed; three fresh default/edit/repeat source compilations; 90 whole-kernel CPU simulations; 17 public-API controls comprising one publication and 16 exact refusals | One live prefix survives into independently checked output/canaries. The eight-prefix control qualifies publication, not eight live values through native execution. |
| Public-seeded source negatives | Actual normal-consumer seed, 30 CPU simulations and four fresh invalid-resource/boundary refusals | Existing test-only fresh resource/boundary observers consume candidates descended from the external normal public seed; no protected/ranked proof checks. The negative children do not call the public publication API. |
| Public-seeded debugger joins | Two fresh source/capture variants, 60 CPU simulations and three stale source/catalog/capture join refusals | Existing test-only fresh callback observers consume descendants of the external normal public seed. Actual simulator captures, not public pipeline/publication calls, a public V17 source-map transport, physical-register lifetime proof or protected admission. |
| Source Variable V2 / resource capture | Three forward/reverse/repeat stops; 27 successful excerpt pairs, 34 full raw pairs including four refusal controls; 12 variables and six limit-2 pages per stop; 24 memory bytes observed per stop | Only `a` and `b` have captured values; the other ten variables are explicitly `not_represented`. Output and canary bytes remain unchanged. No write attribution or inferred source-to-SSA map. |
| Fresh prefix native qualification | `prefix-native-report-join-r2` passed strict joining of four low/high-register O0/O3 native cases, two positive and 21 negative controls, the existing 90 simulator runs, and 101 exact selected file pins totaling 810,739,619 stable bytes | Fresh normal-worker/native observations with validated reports, not merely command exit codes. No hardware execution, lifetime proof, protected proof or toolchain-closure attestation. |
| Companion-site validation | Lint, type checking, production build, 870 Vitest tests in 61 files and 21 lab controls passed | 136 Playwright tests passed with retries disabled, zero skips/flakes; final documentation-only lint/typecheck/unit/build/evidence rerun also passed. Synthetic controls are not actual debugger capture; the 56-lesson/306-tab compiler curriculum stays pending. |

The source-variable pages preserve the actual frame-1 refinement of the same
stop, separately from its unframed control/stack/memory anchor. Legacy
occurrence 1 is not a dynamic helper activation. Each stop has 12 distinct
variables: 36 retained variable rows and 18 pages across the three stops, not
36 distinct variables. Checkpoint SSA counts are 4/3/4; names are not guessed
from those ordinals. Exact raw request/response lines remain unchanged.

Prefix eligibility is at most eight preceding immutable simple `u32` lets,
each a direct AND/OR/XOR of original immutable formals. There are no aliases,
dependencies on earlier locals, shadowing, calls, macros or side effects.
The sole selected bitselect keeps its direct-formal boundary and exact three
contiguous entry-block operations. Only its initializer is replaced; original
and prefix bytes are preserved. Optimization can erase required operator/span
attribution, including common-subexpression collisions: such sources refuse,
without weakening the join or claiming every syntactically eligible prefix
will publish. The generated region remains constrained LLVM inline assembly
inside the ordinary kernel, not a bypass of source or canonical-IR checking.

## Original milestone exits: 3 qualified, 15 still open

M1, V1 and U1 remain the three qualified original exits. Phase18 advances
bounded acceptance cases but closes no additional milestone. In particular,
U2 asks for one supported region, not arbitrary lifting or additional targets;
its remaining acceptance must not be expanded merely because M2/M4 are broader.

| Milestone(s) | Exact remaining acceptance or dependency |
| --- | --- |
| M0 / V0 / U0 | Named compiler, schedule, simulator/debugger, LLVM/finalizer and tutorial owners still need to accept the complete declared profiles, whole-body/source/resource/observation boundaries, representative fixtures and budget policy. The narrow phase17 decision does not settle all contracts. |
| M1 / V1 / U1 | Qualified bounded real-source instruction, recorded-memory and lowered-region inspection exits; no new closure is claimed. |
| M2 | Complete bounded whole-kernel and mixed-region qualification still needs the declared helper/specialization, labels/branches, ABI and resource contracts. Source-fed body and one-region promotion evidence do not establish every whole-kernel obligation. |
| M3 | Authored global/buffer/LDS, waits/barriers and admitted atomic/wave operations need source-produced semantics, bounds/init/race/convergence/missing-wait negatives, descriptor checks and applicable hardware cells; ordinary Rust memory captures are not that ISA coverage. |
| M4 | Executable exact-gfx950 and matrix/low-precision profiles need numerical-policy, layout/resource and target-qualified positive/negative evidence; catalog inventory is not implementation. |
| M5 / V3 | Exact source/expansion-to-final-artifact and allocator lineage, portable supported source-map/capture handoff, per-variant resource bindings and allocator replay remain incomplete; logical/planned values are not observed physical assignments. |
| V2 | Source values now supplement real retained SSA/memory stops. Repeated helper activations, genuine allocation reuse/generation and the combined source/SSA/reverse/fault/watch workflow still require the corresponding #215/#216 capture/query ownership; no name-derived correspondence is added. |
| V4 | The target-bound bank model still needs supported live hardware/session adapters and exact target/stop evidence. Assumed layout or base residue does not establish native transactions, conflicts or timing. |
| U2 | Finish reviewed acceptance/publication of the one supported-region chain and applicable old-proof/analysis invalidation at the admitted production boundaries. Strict fresh native validation now passes; stale captures and fresh simulation do not prove protected proof invalidation. No general extraction, reversible decompilation or extra target requirement is added. |
| U3 | #134 D4 / #271 must provide current-source custody into the existing schedule model, fixed production policy and checked continuation: two legal retained schedules, real edited-source replay/rebind/refusals and fresh records. A private local-order codec or diagnostic JSON cannot supply this authority. |
| M6 / V5 / U4 | Complete small-kernel and tiled workflows, exact compiler/site pins, clean-checkout compiler/query/source-roundtrip and UI/browser gates, measured budgets, shared lesson publication and applicable proof/runtime/hardware evidence remain required. #272's production proof/ABI/generated-host closure and #275's integrated tiled path are separate dependencies, not supplied by this batch. |

## Parallel work and remaining gates

Compiler promotion/refusal work, native observation review, and the read-only
site importer/tutorial can progress in separate leaves. Their results join only
through measured exact inputs; root owns shared integration and qualification.
Recipe policy, source-proof/generated-host closure and tiled integration need
their existing owners, not an alternate pipeline or arbitrary pass list.
Final regressions passed independently: canonical 1,710 and mirror 1,709 tests,
with 109 pre-existing ignored tests each, plus all four actual source ladders.
Strict validation of the four fresh native observations and 136 browser tests
passed. Independent compiler and site reviews found no release blocker.
Strict Clippy still fails with 31 diagnostics on both baseline and current:
30 identical code/message/source records and one narrowed unused-field record.
No added diagnostic is present; this is not a green strict-lint or hosted-CI claim.
Final DCO/hygiene and ordinary non-force publication are separately recorded
in the issue handoff. No hardware run is claimed.

The phase18 task envelope is retained ROOT at most **80 GiB**, free disk at least
40 GiB, available RAM at least 64 GiB, two Cargo jobs and incremental compilation
off, with existing qualification/site locks and bounded fresh outputs. These
are this task's recorded supervision guards, not new compiler semantic budgets,
accepted performance SLOs or standing permission to widen limits. The historical
September 20 document's 20 GiB envelope belongs to that earlier task. Existing
candidates, targets and receipts remain retained; no cleanup authority follows.

## Separate owner handoffs

The 17:49 UTC #275 readback remains blocked on the #271 owner's composed-tile
Phase1a/b plus same-graph memory-interface/allocation handoff; there is no
disjoint bypass. The signed excerpt-only packet
`d79d730ba5cb70cd0b138f45de35a014dc2ab49a` is not on main and has zero
qualified production exits. Its pending in-memory binding adjustment is not a
manifest change. See the [excerpt handoff](https://github.com/harsh-nod/fe2o3/issues/275#issuecomment-5781086978);
the integration owner's blocked-status readback identifies comment `5781268947`.
The separately reported #272 handoff remains at `92928f`.
Neither packet is counted as phase18 protected-production completion.
