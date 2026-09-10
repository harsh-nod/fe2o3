# A1/A2 Next-Wave Dispatch

Reviewed 2026-09-10 against the locally validated R72 source, based on signed
`ed01d85acdb37be01840cef79a38a16278c288da` (R71 plus the previous dispatch audit).
Three agents reconciled the remaining A1/A2 work with source and
[#182](https://github.com/harsh-nod/fe2o3/issues/182). This dispatch audit is
read-only: the next tickets below are assigned implementation work, not new
implementation, proof results or unattended background jobs. R72's separate
implementation and validation are recorded below.

The [full roadmap](runtime-a1-a2-swarm-plan.md) retains historical packet scope
and the property-level acceptance matrix. The [R71 evidence](evidence/local-r71-device-pool-drain-2026-09-10/README.md)
covers the implemented device-cache limits and copy-first drain qualifier.
Those features need hardware acceptance, not duplicate implementation.

## R72 Follow-On

MEM-N1A-COST/NATIVE/FWD are now implemented and pass the
[R72 local/proof gates](evidence/local-r72-host-backing-2026-09-10/README.md):
2,219 runtime tests per GNU/musl target, 39 added tests, and an authenticated
61-source/1,367-obligation/678-negative proof run. The new proofs cover five
cost-projection properties, not native disposal or whole-executor refinement.
Hardware acceptance remains open; the selected MI300X was busy at the read-only
check, so no stage or workload was started.

The next Resources slot takes GEN-2R charged results, then ordinary host-cache
limits, broader N1 profiles and MEM-DOM-1. Admission takes GEN-2A owned
invocation/decoder custody; Native independently prepares SCALE-1A fixtures.
Primary retains shared integration, proof composition and hardware scheduling.

## Current Swarm Dispatch

These are three worker slots plus the primary. A lane executes its queue
sequentially; listed follow-ons are not additional concurrent workers.

| Owner | Next bounded assignment | File ownership and handoff |
| --- | --- | --- |
| Native: `r66_native_coexistence` | SCALE-1A-FIXTURE: isolated bounded short/long artifacts and complete-output oracles | Own new runtime fixture directories, isolated qualification module and tests; then sequential correctness example/checker. Primary owns shared backend admission hooks, exports and artifact-build execution. No GEN-2 or broader-budget prerequisite for fixture preparation. |
| Resources: `r66_coexistence_model` | GEN-2R: charged typed-result storage | Own proposed `crates/fe2o3-host/src/generated_runtime_results.rs` and isolated ownership/model tests. Use the agreed peak-reservation contract below; no production bytes enter the legacy result state. |
| Admission: `r66_runtime_coexistence` | GEN-2A: owned nonexecuting invocation preparation | Own proposed `crates/fe2o3-host/src/generated_runtime_invocation.rs` and unit/compile-fail fixtures. Reuse protected admission and packing helpers by explicit shared-file handoff. Context admission and native publication are GEN-2B. |
| Primary | Shared GEN-2 integration, proof composition and release/qualification | Own shared generated arguments, exports, dependency/macro hooks, Context/owner hooks and proof registration. Schedule existing OVL-QUAL-2 and DRN-2A against frozen signed source; native-budget pressure first needs MEM-QUAL-HARNESS. |

Admission and Resources agree the permit/decoder/result boundary before code
integration. Native's fixture work is independent and does not mint production
generated-launch authority. All shared-file changes require an explicit
ownership handoff. Rotate cross-review before each packet's local gates.

After this wave, Native takes MEM-QUAL-HARNESS and ordinary host-cache hooks;
Resources takes host-cache policy, MEM-N1B, MEM-DOM-1, MEM-3/4, VER-1/2 and
MEM-5. Admission takes GEN-2B, then generated graph/drain qualification.
Native capacity and measurements follow admitted workloads and actual resource
budgets. Version-journal work may move earlier when the Resources slot is free.

## Remaining Tickets

| Packet | Lead | Deliverable and acceptance gate |
| --- | --- | --- |
| MEM-QUAL-HARNESS | Native; Resources review, Primary runner integration | New example/checker exercises actual optional N1/N2 budgets and device-cache limits in both startup orders, with pressure, retained backing, reuse and disposal. Existing R66/drain examples only configure logical requested-byte limits; rerunning them cannot qualify native budgets. |
| MEM-N1A-QUAL and N2/pool qualification | Primary after MEM-QUAL-HARNESS | Signed Linux admission-pressure captures, exact native usage/identities, complete data and cleanup. Host-cache cells follow that policy's implementation. Fake-backend tests do not fill these cells. |
| GEN-2A | Admission | Move-only nonexecuting executable/arguments/decoder preparation and private protected permit. Reject changed artifact, packing, geometry, device and publication; compile-fail tests prohibit borrowed escapes and duplication. Context allocation generations/admission belong to GEN-2B. |
| GEN-2R | Resources; Admission hooks | Reserve the complete encoded-plus-typed output peak before internal allocation. Keep each output's full conservative debit through extraction, observer drop and owner shutdown; no raw storage escape, partial-result publication or capacity mismatch. |
| GEN-2B | Admission/Native; Primary integration | Consume the permit once in actual nonblocking publication/retirement. Revalidate at publication, retain all authority through quiescence, and gate decoding on exact completion. Blocking execution joins this path. |
| MEM-2B-HOST | Resources policy; Native hooks | Bound ordinary coherent host-cache reuse/eviction using R72 N1A. Test padded byte/record limits, zero caching, generation reuse and failed trim; preserve the existing backing debit. Broader N1B is not a prerequisite for this narrow profile. |
| MEM-N1B | Resources/Native | Qualify userptr, doubled-VA AQL, executable, kernarg and control profiles separately. Distinguish physical backing, aliases and reserved VA; test exact lifetime/disposal and avoid double charging. |
| MEM-DOM-1 | Resources; Primary construction hooks | Root/device/Context account ownership with bootstrap and terminal headroom reserved in advance. Repeated Context creation and simultaneous quarantine must not reset or exceed aggregate limits. |
| MEM-3A/B | Resources/Native | Account for queue/ring, signal, kernarg/control storage and occupied slots. Adopt exact MEM-TXN-1 members without duplicate backing debits; enforce a progress-safe acquisition order before publication. |
| MEM-4A/B | Resources/Native | Bound retained host executable images and materialized code/control caches. Exact identity and live-operation leases govern eviction; ambiguous unload retains charges. |
| VER-1A/B, then VER-2 | Resources; Primary mutation hooks | Bounded persistent Context mutation journal, conservative invalidation of every mutation path, and private cross-run input leases. Reject foreign, stale, unknown, mixed-generation and replayed versions before issue. |
| MEM-5 closure | Resources; Primary integration | Close aggregate command/result/capture, registry, arena, journal, reply, terminal and quarantine footprints. Include all native and host owners, not just requested allocation bytes. |
| OVL-QUAL-2 and DRN-2A hardware | Primary; Native/Admission review | Independently checked signed coexistence and copy-only outstanding-work drain captures, complete outputs/canaries, exact native identity and owned-process/stage cleanup. Retention alone does not prove physical overlap. |
| SCALE-1A-FIXTURE / QUAL | Native; Primary builds/hardware | Freeze bounded geometry, ABI/effects, source/object/toolchain and complete-output oracles. CPU policy tests precede signed sequential Linux correctness. Intended short/long work classes are not measured durations. |
| SCALE-CAP, then SCALE-2 | Native | Real backing/control/slot admission precedes any larger native profile; preserve existing defaults. Measure native publication/retention, unresolved work and retirement separately from host queue depth. Thousands of queued records do not establish thousands native in flight. |
| DRN-2B and repeated generated graphs | Admission; Primary hardware | After GEN-2B and exact per-kernel compiler evidence, qualify repeated kernel/copy graphs, active drain, dropped observers, complete typed outputs and cleanup. Cross-run input reuse also needs VER-1/2. Keep fixture and production cells separate. |
| PRF-1/2 | Primary; rotating cross-review | Compose production executor transitions with lifecycle, credits, dependency readiness, versions, reuse and drain proofs. Run authenticated positives/negative mutations and integration gates; retain explicit external contracts. |
| SCALE-3 measurements | Native; Primary hardware | Signed matched HIP/HSA/KFD producers and correctness-first captures for latency, throughput, copy bandwidth, CPU use and memory bounds. Device timelines are separately required for physical-overlap claims. |

## Interface Decisions

N1A provides public `Gfx942HostVisibleBackingBudgetV1::new(bytes, records)` and
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
accounting are not implied. Failures and panics after the first native effect
but before record insertion must retain/quarantine the charge. Pre-effect
rejection cancels only the unissued reservation. Later queue-retake failure cannot resurrect a
debit already returned after confirmed disposal.

GEN-2 must bind the deliberately non-Send/non-Sync
[checked device](../crates/fe2o3-kfd/src/device.rs) inside its owner, not widen
its lifetime or thread-safety. Existing GEN-1
[`try_take()`](../crates/fe2o3-host/src/generated_runtime_arguments.rs)
returns bare `Box<[T]>` and represents decoded data only. GEN-2R therefore needs
a distinct charged result owner whose credit follows the returned storage,
not merely the observer. The legacy decoded-result state must never receive
production bytes, even behind a wrapper. This is a new production contract, not
a defect in GEN-1's intentionally inert data API.

The agreed GEN-2R contract reserves one R70 batch member per output for its
encoded-plus-typed peak and retains that entire conservative reservation until
the charged typed result is disposed. It does not require partial splitting of
an issued debit or claim that reserved peak equals current physical residency.
Preallocate private destinations, validate the complete scalar/shape/count and
capacity roster, decode all outputs, then publish results together. Expose
borrowed slices, not `Clone` or raw `Box`/`Vec` extraction. Test late-output
failure, decoder mismatch/panic, exhaustion, dropped observers and shutdown.
Caller seed storage is charged on transfer, not retroactively bounded before
the caller allocated it. Storage ownership alone grants no completion authority.

GEN-2A consumes that interface in an owner-local prepared invocation. It is
nonexecuting; no checked-device lifetime widening, unsafe thread-safety claim or
caller-provided authorizer is permitted. Primary integrates Context allocation
generations and operation admission in GEN-2B with Native's existing publication
and retirement mechanisms, not a second queue implementation.

## Dependencies And Stops

1. N1A cost/native/forwarding is locally implemented as one integration packet.
   Ordinary host-cache limits can follow immediately; broader N1 profiles,
   domain ownership and control/code budgets remain separate prerequisites;
   MEM-5 cannot close before all owners are accounted for.
2. GEN-2A and GEN-2R can now proceed concurrently in separate lanes with the
   agreed result contract. Their local rejection, ownership and proof tests need
   no compiler deployment. GEN-2B consumes these interfaces; positive production
   execution also needs the next handoff. VER-1 may move earlier when its slot
   is free.
3. Positive production GEN-2 admission requires compiler/generated-host owners
   to supply exact protected verification and semantic-to-machine refinement
   evidence: finalized artifact/ISA, semantic KIR/final LLVM, ABI/effects,
   authenticated proof inputs, currentness/ledger and publication occurrence.
   The concrete production protected-verifier/refinement backend and exact
   artifact handoff remain absent from the repository; test adapters do not
   supply them. The [existing fail-closed boundary](../crates/fe2o3-host/src/worker_v3_verification_admission.rs)
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
their own decompositions. Neither this plan nor R72 establishes full HIP/HSA
parity or an unmeasured speedup.
