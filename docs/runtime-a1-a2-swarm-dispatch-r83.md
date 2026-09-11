# A1/A2 Swarm Dispatch After Local R83

Dispatch established by planning-only commit `32c1beff`, 2026-09-10, and updated
after the R83 local release gates. This is the current assignment overlay for
[next-wave dispatch](runtime-a1-a2-next-wave.md) and the
[historical roadmap](runtime-a1-a2-swarm-plan.md). It supersedes their current
assignment rows, not their packet-specific evidence or historical contracts.
The immediate scope is remaining A1/A2 work in
[#182](https://github.com/harsh-nod/fe2o3/issues/182), observed open with issue
update `2026-09-10T10:50:51Z`. Later A3-A7 milestones remain separate.

## Checkpoint And Ownership

The signed pre-R83 implementation baseline is R82
`5184428b7bb9bbb0e9cc30c6929d3c77ee60cbdb`, on both topic remotes.
R83 implements private activation, nonflushing active custody, stream holds and
explicit drain/shutdown retirement. Its
[local evidence](evidence/local-r83-unpublished-lifecycle-2026-09-10/README.md)
records all seventeen frozen-source gates, 2,363 runtime tests per GNU/musl
target, fifteen new lifecycle tests and the resolved callback type-complexity
lint. The original planning-only checkpoint did not publish or qualify R83;
the later source/evidence packet supplies this local acceptance.

Production generated preparation installs no adoption hooks. Activation is a
private boundary, not a public API. Native DATA adoption, publication and typed
completion are still missing. R83's scripted lifecycle tests do not establish
actual charged-carrier integration, Linux execution or executable refinement.

Three read-only workers completed the source audit below. Their review turns
are complete; the follow-on queues are assignments, not unattended jobs.
Primary owns all edits, integration, conflict resolution, tests, proofs,
hardware scheduling and signed publication. There are three worker slots plus
Primary, not one worker per ticket. Shared Context/backend edits are serialized.

## First Parallel Packets

| Worker | First bounded assignment | Deliverable and exit gate |
| --- | --- | --- |
| Native: `r66_native_coexistence` | DATA-SHELL, the nonexecuting prerequisite to DATA-ADOPT | Closed one-shot packet transfer and genuine generated allocation records without ordinary encoded `Arc<[u8]>` snapshots. Preserve the full original ordinal roster, pointers, authority and decoder custody. Reject substitution/capacity failures before effects. |
| Admission: `r66_runtime_coexistence` | COMPLETE-ORACLE, independent of native adoption | Exact invocation/submission/generation and full-roster validation contract; failure and disposal-order fixtures covering malformed late output, decoder panic, observer loss and shutdown. No native completion authority from fixtures. |
| Resources: `r66_coexistence_model` | VER-1A, independent Context mutation journal | Bounded nonwrapping whole-allocation `Available/Pending/Unknown` transitions, atomic multi-destination admission, exact writer completion and a complete mutation-site inventory. Cross-run leases stay disabled. |
| Primary | DATA-SHELL/DATA-ADOPT integration on accepted local R83 | Preserve R80 reservations and R83 custody. Compose reviewed interfaces; do not install adoption callbacks before complete native custody exists. Run current-source gates and retain separate proof/Linux/performance results. |

Native and Admission first agree on the exact packet/hold/native-key boundary.
Admission can develop COMPLETE-ORACLE while Native designs DATA-SHELL; Resources
does not depend on either. This avoids assigning both workers the same shared
implementation or recreating R83's existing phase/hold machinery.

## Generated Execution Queue

Paths below are relative to `crates/fe2o3-runtime/src/` unless otherwise stated.
New modules are proposed, not existing implementation claims.

| Packet and lead | Dependency / module boundary | Acceptance before advancing |
| --- | --- | --- |
| DATA-SHELL: Native | After R83 interface freeze. `persistent_projection.rs`, `generated_source.rs`, `kfd_backend.rs`, `kfd_backend/compute_state.rs`; host `generated_runtime_invocation.rs` coordinated by Primary. | Consuming transfer without cloning or exposing packet/decoder authority; complete unused/read-only buffers and original ordinals; exact Context/native IDs and bounded records. No native effects or dummy replacement payload. |
| DATA-ADOPT: Native + Admission | DATA-SHELL, R80 reservations, R82 abort and R83 lifecycle. Proposed `kfd_backend/generated_adoption.rs`; narrow Context, `compute_dispatch.rs` and existing adoption hooks. | Root complete partial native prefixes and lane ownership before every effect; borrowed initialization, nonpublishing bind and explicit abort/disposal. Exercise initial/auxiliary/reused lanes, all allocation/map/bind/retake failures, panic, empty-prefix Stop and drain. Linux bind/abort/reuse remains a separate gate. |
| COMPLETE-ORACLE: Admission; Resources review | Independent now. Proposed completion contract plus focused host charged-storage and runtime preparation fixtures. | Freeze one-reply semantics, exact completion identity, full-roster failures, original-storage disposal and charge lifetime before ISSUE. Reuse R73/R80 tests rather than duplicating them. |
| ISSUE: Admission + Native | Composed DATA-ADOPT and COMPLETE-ORACLE. Generated driver, Context registration, `compute_state.rs` and `compute_dispatch.rs`. | A private linear permit binds exact resources/submission and survives actual flush/retry. Reject substitution/replay; distinguish definite nonpublication from unknown publication. No broad backend authorization flag or per-invocation backend replacement. |
| COMPLETE: Admission; Resources + Native review | ISSUE. Generated driver, host invocation/results/readback and backend readback hooks. | Fill existing R80 destinations, validate every buffer, destroy original encoded storage before committing externally droppable typed outputs, then resolve once. Test stale/partial results, currentness loss, decoder panic, retained results and credits after shutdown. Never reserve readback or the completion reply twice. |
| API: Admission; Primary integration | ISSUE + COMPLETE. Runtime handle, generated host interfaces and narrow macro integration. | One executor-neutral typed future; blocking launch joins the same engine. Test completion/poll races, latest-waker replacement, reentrancy, reply/queue pressure, equivalent outcomes and ownership compile failures. No blocking executor hidden in a command callback. |
| GRAPH/DRAIN: Admission | API; cross-run reuse also needs VER-1B/VER-2. `async_engine/graph`, drain/capture and owned shutdown. | Repeated generated graphs, exact dependencies/results, accepted-prefix drain, dropped observers and failure retention. Preserve graph/standalone exclusion. Production hardware acceptance additionally needs exact protected compiler evidence. |

## Resources And Proof Queue

| Packet and lead | Dependency / module boundary | Acceptance before advancing |
| --- | --- | --- |
| VER-1A: Resources | Independent now. Proposed `context/versions.rs`, isolated model/proof/tests; Primary owns shared exports. | Bounded capacity and version exhaustion, whole-allocation invalidation, multiple destinations, exact writer completion and unknown outcomes. Existing graph-local versions are history, not cross-run authority. |
| VER-1B, then VER-2: Resources + Primary | VER-1A. Context host-write/copy/peer/launch/release/currentness hooks; proposed graph input leases. | Every mutation path invalidates or rejects before issuing private cross-run leases. Test operations outside graphs, overlapping writers, reuse, foreign Contexts, replay and unknown publication. Precise kernel effects require compiler admission. |
| MEM-DOM-1A/B: Resources | Independent domain/headroom design; integrate `fe2o3-resource-accounting` and Context/session construction. | One bounded root, exact children, charged account arenas and reserved terminal headroom. Repeated Contexts and simultaneous quarantine cannot reset ceilings; parent exhaustion is failure-atomic. |
| MEM-N1B, then MEM-3: Resources + Native | Existing R70/R72 primitives; global claims also require domains. Native shared-memory accounting, queue/dispatch and slot/arena hooks. | First kernarg/executable backing, then AQL/userptr/remaining control profiles. Distinguish physical backing, aliases and VA; no duplicate debit. Cover every construction/disposal failure and progress-safe resource acquisition. |
| MEM-4A/B: Resources | Host-image ceiling can start independently; native residency needs backing/control integration. Proposed backend residency module. | Bound retained host images and materialized code caches; exact identity/live leases constrain eviction. Uncertain unload retains charges. Executable GTT is not VRAM. |
| PRF + MEM-5: Primary; Resources review | Incremental with every packet, not deferred until the end. Models, adapter checks, proof inventory and retained evidence. | First target R83 one-shot lifecycle/R82 cleanup correspondence. Ultimately account for registry, commands/captures, replies/results, arenas, journals and aggregate quarantine, with executable-transition refinement or explicit remaining boundaries. |

The retained authenticated R73 proof checkpoint reports 62 positive sources,
1,374 obligations and 686 negative mutations. It does not prove later adapter
changes. Inventory checks are not solver reruns. See the
[R73 evidence](evidence/local-r73-charged-results-2026-09-10/README.md).

## Independent Qualification Queue

Native rotates into these packets after its current handoff; Primary alone
schedules hardware. They need no new protected production compiler handoff
unless explicitly stated.

| Packet | Implementation versus qualification | Exit gate |
| --- | --- | --- |
| SCALE-1A-FIXTURE | New bounded fixture sources/manifests/oracles/checker tests under `benchmarks/runtime_gfx942`; then sequential Linux correctness. | Freeze ABI/effects, geometry, source/object/toolchain and complete output/padding oracles. Mutated coordinates reject. Short/long labels are not measured durations. |
| MEM-QUAL-HARNESS | New example and signed runner/checker exercising existing N1/N2 and both cache policies in both startup orders. | Bootstrap consumption, padded byte/record pressure, zero caching, retained reuse and exact disposal; complete data and native identities. Existing logical-budget examples do not qualify native budgets. |
| R76/R78/R82 qualification | Native primitives are implemented; dedicated retained-scope, rebound-initialization and same-queue abort/rebind harness cells remain to be added. | Genuine retained device before/after lazy bootstrap, repeated complete outputs, exact queue/generation and conclusive cleanup. One cell cannot substitute for another. Positive generated construction separately needs compiler evidence. |
| OVL-QUAL-2 / DRN-2A | Existing coexistence and copy-drain campaigns, not duplicate implementations. | Frozen signed source, complete outputs/canaries, retained native identity, bounded staging and independently confirmed owned-process/file cleanup. These captures alone do not establish physical overlap. |
| SCALE-CAP -> SCALE-2 | New bounded native profile/admission before larger depth qualification. | Native backing/control/slot limits before increasing capacity. Distinguish queued host records from actually published/retained native work and measure out-of-order completion. |
| DRN-2B / generated drain | Fixture compute drain follows sequential fixture correctness; production graph/drain follows generated integration and exact compiler evidence. | Outstanding compute, dropped observers, complete outputs and cleanup. Fixture results do not fill production-generated cells. |
| SCALE-3 | Matched KFD/HSA/HIP producers and signed correctness-first performance campaigns remain open. | Predeclared workloads/thresholds, identical work, full-output checks, CPU/memory/tail metrics and independent device timelines for physical overlap. No parity or speedup claim from API shape or CPU tests. |

No SSH, GPU workload, solver or runtime test was started for the original
planning-only refresh; subsequent R83 runtime tests are recorded separately
above. Future shared-MI300X campaigns require an idle admitted GPU, bounded
private staging, one owned campaign at a time and cleanup of only owned resources.
Disruptive fault tests require an agreed isolated window.

## Ordering And Closure

```text
local R83 -> DATA-SHELL -> DATA-ADOPT -> ISSUE -> COMPLETE -> API -> GRAPH/DRAIN
                                  ^           ^
                         R80 + R82 custody     |
COMPLETE-ORACLE -------------------------------+
VER-1A -> all VER-1B mutation hooks -> VER-2 cross-run input leases
domains + backing/control/residency -> MEM-5 aggregate closure
each implemented packet -> CPU + adapter/proof + relevant Linux acceptance
```

A1/A2 closure also requires native depth, repeated graph/version/resource
qualification and matched measurements, not only completion of the generated
API. Keep `Proved`, `Checked`, `Validated`, `Contracted` and `Unsupported` results
property-specific. Protected compiler evidence is an external handoff for
production-generated acceptance, not a reason to stop independent local work.

After A1/A2, the same slots rotate through the remaining
[issue milestones](https://github.com/harsh-nod/fe2o3/issues/182):

| Milestone | Lead and boundary |
| --- | --- |
| A3: local multi-GPU | Native, with Resources/Admission: placement, shards/replicas, compute/peer ownership and group failure/drain. Existing two-device copy support is insufficient. |
| A4: distributed control | Admission: authenticated membership, artifact/plan exchange, publication receipts and terminal run records. |
| A5: distributed data/collectives | Native + Resources: bounded transfers, versions/credits and separately qualified collective plans. |
| A6: fault qualification | Admission + Primary: participant/network/device failures without unsafe replay, early release or false completion. |
| A7: production performance | Native + Primary: predeclared single-device, multi-GPU and two-host performance gates with direct-KFD dependency/symbol audits. |

This dispatch does not close A1/A2, #182, full HIP/HSA parity or any performance
target. Local R83 acceptance covers its private lifecycle only, not the queued
native adoption, completion, proof, hardware or performance work.
