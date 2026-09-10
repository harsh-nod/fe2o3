# A1/A2 Next-Wave Dispatch

Reviewed 2026-09-10 against signed R71
`1c7b1249fa1326b5cc90c2ac19fc8007b5e84763`, present on both remotes at review.
This is the three-agent planning handoff for
[#182](https://github.com/harsh-nod/fe2o3/issues/182), not a new implementation
or acceptance result. All three agents performed read-only audits; no build,
proof run or MI300X workload was started for this checkpoint.

The [full roadmap](runtime-a1-a2-swarm-plan.md) retains historical packet scope
and the property-level acceptance matrix. The [R71 evidence](evidence/local-r71-device-pool-drain-2026-09-10/README.md)
covers the implemented device-cache limits and copy-first drain qualifier.
Those features need hardware acceptance, not duplicate implementation.

## First Implementation Wave

These are three worker slots plus the primary. A lane executes its queue
sequentially; listed follow-ons are not additional concurrent workers.

| Owner | First assignment | File ownership and handoff |
| --- | --- | --- |
| Native: `r66_native_coexistence` | MEM-N1A-NATIVE: coherent host-GTT lifecycle accounting | Own `crates/fe2o3-kfd/src/shared_memory.rs` and new `shared_memory/tests/host_backing.rs`. Use the agreed Resources account API. SCALE-1A fixture design is independent work while waiting for that adapter. |
| Resources: `r66_coexistence_model` | MEM-N1A-COST: private backing account, cost projection and proofs | Own new `crates/fe2o3-kfd/src/shared_memory/host_resource_accounting.rs` and isolated model/proof/tests. Then take GEN-2R charged results as a separate scheduled packet. |
| Admission: `r66_runtime_coexistence` | GEN-2A: owned generated invocation and private admission boundary | Own proposed `crates/fe2o3-host/src/generated_runtime_invocation.rs` and focused tests. Reuse protected application admission and existing packing; agree output/decoder custody with Resources before GEN-2R. |
| Primary | MEM-N1A-FWD, cross-layer proof integration and signed hardware qualification | Own `memory.rs`, `queue_live.rs`, runtime/Context constructors, shared exports and proof registration. Schedule OVL-QUAL-2 and DRN-2A against frozen signed source independently of new implementation. |

Native and Resources have agreed the N1A API shape, but no implementation has
landed. Native edits must not race Primary's constructor/export integration.
All shared-file changes require an explicit ownership handoff.

## Remaining Tickets

| Packet | Lead | Deliverable and acceptance gate |
| --- | --- | --- |
| MEM-N1A-COST/NATIVE/FWD | Resources, Native, Primary | Charge ordinary coherent backing before effects; preserve exact custody across maps, CPU access, queue loans and reuse. Only confirmed complete disposal refunds. Configure immutably in both startup orders. |
| MEM-N1A-QUAL and N2/pool qualification | Primary; Native/Resources review | Signed Linux tests for real admission pressure, both startup orders, retained backing, cache reuse/disposal and cleanup. Fake-backend tests do not fill these cells. |
| GEN-2A | Admission | Move-only owned executable/arguments/decoder preparation; private permit from protected admission only. Reject changed artifact, packing, geometry, device, generation and publication; compile-fail tests prohibit borrowed escapes and duplication. |
| GEN-2R | Resources; Admission hooks | Charged typed decoded-result storage. Reserve before allocation; keep the debit through extraction, observer drop and owner shutdown. No raw uncharged storage escape or result-capacity mismatch. |
| GEN-2B | Admission/Native; Primary integration | Consume the permit once in actual nonblocking publication/retirement. Revalidate at publication, retain all authority through quiescence, and gate decoding on exact completion. Blocking execution joins this path. |
| MEM-N1B and host-pool limits | Resources/Native | Qualify userptr, doubled-VA AQL, executable and control profiles separately; add bounded host cache reuse/eviction without early refund. Distinguish physical backing from reserved VA. |
| MEM-DOM-1 | Resources; Primary construction hooks | Root/device/Context account ownership with bootstrap and terminal headroom reserved in advance. Repeated Context creation and simultaneous quarantine must not reset or exceed aggregate limits. |
| MEM-3A/B | Resources/Native | Account for queue/ring, signal, kernarg/control storage and occupied slots. Adopt exact MEM-TXN-1 members without duplicate backing debits; enforce a progress-safe acquisition order before publication. |
| MEM-4A/B | Resources/Native | Bound retained host executable images and materialized code/control caches. Exact identity and live-operation leases govern eviction; ambiguous unload retains charges. |
| VER-1A/B, then VER-2 | Resources; Primary mutation hooks | Bounded persistent Context mutation journal, conservative invalidation of every mutation path, and private cross-run input leases. Reject foreign, stale, unknown, mixed-generation and replayed versions before issue. |
| MEM-5 closure | Resources; Primary integration | Close aggregate command/result/capture, registry, arena, journal, reply, terminal and quarantine footprints. Include all native and host owners, not just requested allocation bytes. |
| OVL-QUAL-2 and DRN-2A hardware | Primary; Native/Admission review | Independently checked signed coexistence and copy-only outstanding-work drain captures, complete outputs/canaries, exact native identity and owned-process/stage cleanup. Retention alone does not prove physical overlap. |
| SCALE-1A, SCALE-CAP, SCALE-2 | Native | Correctness-qualified short/long fixtures, then genuinely admitted native capacity and measured out-of-order completion/depth. Thousands of host-queued records do not establish thousands of native in-flight operations. |
| DRN-2B and repeated generated graphs | Admission; Primary hardware | After GEN-2B and exact compiler evidence, qualify repeated kernel/copy graphs, active drain, dropped observers, complete typed outputs and cleanup. Keep fixture and production cells separate. |
| PRF-1/2 | Primary; rotating cross-review | Compose production executor transitions with lifecycle, credits, dependency readiness, versions, reuse and drain proofs. Run authenticated positives/negative mutations and integration gates; retain explicit external contracts. |
| SCALE-3 measurements | Native; Primary hardware | Signed matched HIP/HSA/KFD producers and correctness-first captures for latency, throughput, copy bandwidth, CPU use and memory bounds. Device timelines are separately required for physical-overlap claims. |

## Interface Decisions

N1A proposes public `Gfx942HostVisibleBackingBudgetV1::new(bytes, records)` and
an inert usage snapshot. Limits are positive, at most 8 GiB and 256 records.
The byte ceiling is a chosen numerical bound matching the existing envelope,
not a measurement of physical residency from VA usage.

Private account/reservation/charge types bind the exact session, device, VM,
allocation ID, generation and full canonical `HostVisibleCoherentGttV1` layout.
For that ordinary non-userptr profile, the page-padded CPU span equals the
native allocation size: charge it once plus one allocation record. CPU/GPU
views, loans and recycle do not create new backing or refund it.

Ordinary coherent completion/control allocations made during bootstrap are
included by profile. Userptr, executable AQL, kernarg and complete bootstrap
accounting are not implied. Failures and panics before record insertion must
retain/quarantine the charge. Later queue-retake failure cannot resurrect a
debit already returned after confirmed disposal.

GEN-2 must bind the deliberately non-Send/non-Sync
[checked device](../crates/fe2o3-kfd/src/device.rs) inside its owner, not widen
its lifetime or thread-safety. Existing GEN-1
[`try_take()`](../crates/fe2o3-host/src/generated_runtime_arguments.rs)
returns bare `Box<[T]>` and represents decoded data only. GEN-2R therefore needs
a distinct charged result owner whose credit follows the returned storage,
not merely the observer. This is a new production contract, not a defect in
GEN-1's intentionally inert data API.

## Dependencies And Stops

1. N1A cost/native/forwarding land as one integration packet. Broader N1
   profiles, domain ownership, control/code budgets and host pools follow;
   MEM-5 cannot close before all owners are accounted for.
2. GEN-2A and the N1A packet can proceed concurrently. Resources takes GEN-2R
   after N1A-COST; GEN-2B consumes their agreed interfaces. VER-1 can move
   earlier when that worker slot is free.
3. Positive production GEN-2 admission requires compiler/generated-host owners
   to supply exact protected verification and semantic-to-machine refinement
   evidence: finalized artifact/ISA, semantic KIR/final LLVM, ABI/effects,
   authenticated proof inputs, currentness/ledger and publication occurrence.
   The repository does not ship the concrete production refinement backend and
   artifact. The [existing fail-closed boundary](../crates/fe2o3-host/src/worker_v3_verification_admission.rs)
   must remain; caller digests or
   qualification fixtures cannot replace that handoff.
4. SCALE-CAP requires real backing/control/slot admission and correctness-qualified
   fixtures. Native depth and timing follow admission, not a constant increase.
5. Proof composition starts with each implementation packet. Final acceptance
   requires the complete A1/A2 matrix, not the sum of isolated cost proofs.
   Matched performance follows correctness and signed hardware acceptance.

Only Primary runs builds, proof gates, MI300X campaigns, signing and pushes.
Use one bounded campaign at a time on an admitted idle selected GPU, private
staging, exact binary/census evidence and independent cleanup. Foreign work is
untouched; disruptive reset/fault campaigns need a separately agreed window.
Every completed implementation packet is cross-reviewed, validated against its
actual source and pushed to both remotes. A topic-branch push is not a main merge.

A1/A2 remain open. Later A3 multi-GPU, A4/A5 distributed control/data/collectives,
A6 distributed failure qualification and A7 broader production performance need
their own decompositions. Neither this plan nor R71 establishes full HIP/HSA
parity or an unmeasured speedup.
