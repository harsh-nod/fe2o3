# A1/A2 Next-Wave Dispatch

Reviewed 2026-09-10 against signed R72
`474b40a73ecd1e16d8b6d0e6ef0e4edb9ee9c7cc` and the uncommitted R73 working tree.
Three read-only agents reconciled the remaining work with source and the current
open [#182](https://github.com/harsh-nod/fe2o3/issues/182). This document assigns
bounded analysis/review queues, not unattended implementation jobs. Primary
owns edits, integration, tests, hardware scheduling and publication. A queued
ticket is not new implementation or an accepted proof/hardware result.

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

## R73 Working-Tree Checkpoint

GEN-2R charged storage and the generated-argument packing route now exist in the
working tree. The latest focused host run passes 28 tests: 17 charged-path
tests and 11 legacy tests. The authenticated Verus run also completed with 62
positive sources, 1,374 obligations and 686 rejected mutations, including R73's
seven obligations and eight mutations. Full source gates, evidence retention
and signed dual-remote implementation release remain pending. These local
results describe the uncommitted tree, not this documentation commit or a
completed GEN-2 execution path.

The draft uses a separate charged result state, reuses the caller's typed seed,
reserves the full roster before encoding, and publishes outputs together only
after complete validation/decoding and returned-buffer disposal. Each output
retains its full conservative encoded-plus-typed debit through disposal;
read-only returned storage has its own debit. Its cost/shape proofs do not prove
the mutex/Box adapters, native completion or whole-executor refinement.

GEN-2A must retain the private decoder with invocation custody. In particular,
read-result credits cannot be detached from possibly live returned storage.
The public charged arguments are data only: native publication, Context
generations, runtime-owner shutdown and authenticated compiler execution remain
GEN-2B/integration acceptance, not consequences of the storage tests.

Primary finishes R73's current-source gates before releasing it. Resources can
now design ordinary host-cache limits without reimplementing GEN-2R. Admission
prepares GEN-2A against the draft interface; integration waits for its freeze.
Native's bounded fixture work is independent of both.

## Current Swarm Dispatch

These are three read-only worker slots plus the primary. A lane supplies a
source-grounded design, test/proof obligations and an independent review for
one bounded packet at a time. Primary implements the agreed packet. Follow-ons
are sequential queues, not additional concurrent workers or delegated edits.

| Lead | Next bounded assignment | Module scope and primary handoff |
| --- | --- | --- |
| Native: `r66_native_coexistence` | SCALE-1A-FIXTURE: bounded short/long artifacts and full-output oracles | Specify new runtime fixture directories, isolated `qualification_gfx942_scale_v1.rs`, mutation tests and sequential correctness checker. Primary owns artifact builds, shared admission hooks and edits. No GEN-2 prerequisite. |
| Resources: `r66_coexistence_model` | MEM-2B-HOST: ordinary coherent host-cache bounds; review R73 release | Specify isolated `sdma/host_pool_policy.rs`, exact padded-cost/identity projection and policy/model tests. Preserve R72 backing debit across reuse; zero cache limit disables caching, default stays unchanged. Coordinate native hooks before integration. |
| Admission: `r66_runtime_coexistence` | GEN-2A: owned nonexecuting invocation preparation | Specify `fe2o3-host/src/generated_runtime_invocation.rs` and unit/compile-fail fixtures. Reuse protected admission and packing helpers after GEN-2R interface freeze. Context admission and native publication remain GEN-2B. |
| Primary | R73 release gate, shared integration, proof composition and qualification | Own every edit, integration/build/proof run, evidence record and signed dual push. Schedule existing OVL-QUAL-2 and DRN-2A against frozen signed source; native-budget pressure first needs MEM-QUAL-HARNESS. |

Admission and Resources agree the permit/decoder/result boundary before code
integration. Native's fixture work does not mint production generated-launch
authority. Primary serializes shared-file edits; each worker reviews another
lane's contract before the packet's local gates.

After this wave, Native takes the sequential fixture qualifier and
MEM-QUAL-HARNESS, then ordinary host-cache and broader native budget hooks.
Resources takes MEM-N1B, MEM-DOM-1, MEM-3/4, VER-1/2 and MEM-5. Admission takes
GEN-2B, then generated graph/drain qualification. Waiting for hardware must not
block CPU harness work. Version-journal mechanics or host-image accounting may
move earlier when the Resources slot is free.

## Execution Order

| Stage | Ready work | Gate before advancing |
| --- | --- | --- |
| 0: finish current packet | Primary validates/releases R73; workers prepare the three contracts above | Current-source tests/lints, generated compile-fail fixtures, authenticated property proofs and explicit remaining boundaries |
| 1: independent local packets | Host-cache policy/hooks, GEN-2A, short/long fixtures and native-budget harness | Freeze cross-lane interfaces; complete deterministic rejection, ownership, failure and oracle tests |
| 2: compose ownership | Broader native profiles, domains, control/code budgets, versions and GEN-2B | No duplicate backing debit, unbounded child accounts, stale input authority or detached decoder; preserve progress headroom |
| 3: qualify A1/A2 | Out-of-order native depth, repeated generated graphs, active drain, memory pressure and overlap | Admitted workloads, exact compiler evidence for production generated cells, signed full-output captures and cleanup; existing copy/overlap campaigns may run earlier |
| 4: measure and close | Matched KFD/HSA/HIP producers, executable proof composition and exit audit | Predeclared workloads/thresholds, resource/CPU/tail metrics, separate device-timeline evidence for physical overlap; every A1/A2 acceptance cell resolved |

These stages express prerequisites, not rigid barriers between independent
lanes. Missing compiler artifacts do not block host-cache, copy-version or
fixture implementation. A busy selected GPU does not block local proof/tests.

## Remaining Tickets

| Packet | Lead | Deliverable and acceptance gate |
| --- | --- | --- |
| MEM-QUAL-HARNESS | Native; Resources review, Primary runner integration | New example/checker exercises actual optional N1/N2 budgets and device-cache limits in both startup orders, with pressure, retained backing, reuse and disposal. Existing R66/drain examples only configure logical requested-byte limits; rerunning them cannot qualify native budgets. |
| MEM-N1A-QUAL and N2/pool qualification | Primary after MEM-QUAL-HARNESS | Signed Linux admission-pressure captures, exact native usage/identities, complete data and cleanup. Host-cache cells follow that policy's implementation. Fake-backend tests do not fill these cells. |
| GEN-2A | Admission | Move-only nonexecuting executable/arguments/decoder preparation and private protected permit. Reject changed artifact, packing, geometry, device and publication; compile-fail tests prohibit borrowed escapes and duplication. Context allocation generations/admission belong to GEN-2B. |
| GEN-2R release/integration | Primary; Resources/Admission review | Working-tree storage implementation exists; complete release gates and retain evidence. Test runtime-owner integration separately from ordinary host-thread/observer disposal. No raw storage escape, partial-result publication or capacity mismatch. |
| GEN-2B | Admission/Native; Primary integration | Consume the permit once in actual nonblocking publication/retirement. Revalidate at publication, retain all authority through quiescence, and gate decoding on exact completion. Blocking execution joins this path. |
| MEM-2B-HOST | Resources policy; Native hooks | Bound ordinary coherent host-cache reuse/eviction using R72 N1A. Test padded byte/record limits, zero caching, generation reuse and failed trim; retain charges for incompletely disposed backing, but never resurrect confirmed-disposed charges. Broader N1B is not a prerequisite for this narrow profile. |
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

### First-Packet Acceptance

- Host cache: padded bytes and record ceilings, zero caching, pressure eviction,
  stale/foreign token rejection and generation-safe reuse. Failed trim retains
  charges for incomplete/uncertain disposal; it does not resurrect charges for
  backing already confirmed disposed. Broader N1B is not required for this profile.
- GEN-2A: changed artifact/packing/geometry/effects/device/publication reject;
  compile-fail tests prohibit borrowed escape, duplicate permits and decoder
  extraction. Preserve the checked device's owner-local `!Send`/`!Sync` contract.
- Fixtures: freeze bounded geometry, ABI/effects, complete ReadWrite footprint,
  source/object/toolchain identity and independent output/padding oracles;
  mutate coordinates to test rejection. Short/long labels are not measurements.
- Native-budget harness: exercise actual N1/N2/cache configuration in both
  startup orders, bootstrap consumption and padded byte/record exhaustion.
  Pre-effect budget rejection leaves usage unchanged; post-native failure or
  ambiguity retains/quarantines the debit. Test charged reuse and confirmed
  disposal. Existing R66 and DRN-2A captures cannot substitute for this harness.

### Later Issue Milestones

These remain separate from closing A1/A2. The same three slots rotate into
these queues after local contracts stabilize; no extra workers are implied.

| Milestone | Lead and deliverable | Exit evidence |
| --- | --- | --- |
| A3: local multi-GPU | Native, with Resources/Admission: unified compute/peer ownership, topology/currentness placement, shards/replicas and group drain | All admitted GPUs execute a sharded workload; exact versions, stale-generation rejection and partial-failure isolation. Existing exact-two-device copy-only XGMI is insufficient |
| A4: distributed control | Admission, with Primary protocol integration: authenticated sessions, membership epochs, artifact negotiation, receipts and drain | Two hosts reject substitutions and classify interrupted publication without duplicate issue or treating timeout as quiescence |
| A5: data and collectives | Native transport plus Resources versions/credits: bounded reference host-staged transfers and deterministic collective plans | Two-host complete canaries, exact membership/version receipts and bounded network/staging use; broadcast, reduce-scatter, all-gather and all-reduce qualified separately |
| A6: failure qualification | Admission; rotating cross-review and Primary fault campaigns | Boundary-by-boundary participant/network/device/collective faults; no unsafe replay, early disposal or false complete outputs. Disruptive tests need an agreed isolated window |
| A7: production performance | Native measurements; Primary evidence/release | Precommitted single-device, eight-GPU and two-host thresholds, tail latency, CPU/memory bounds, recovery costs and direct-KFD dependency/symbol audits |

Broad device Rust and production atomics/collectives also require their exact
compiler semantic-to-machine contracts and native qualification. Existing
host-observed profiling is not missing wholesale; trusted device timelines and
cross-collector attribution remain separate work.

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
returns bare `Box<[T]>` and represents decoded data only. The GEN-2R draft adds
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
2. GEN-2A design and GEN-2R validation can proceed concurrently with the agreed
   result contract; integration requires the interface freeze. Their local
   rejection, ownership and proof tests need no compiler deployment. GEN-2B
   consumes these interfaces; positive production
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

A1/A2 remain open. The later A3-A7 table assigns milestone leads and exit gates;
those milestones still need packet-level designs and their own test environments.
Neither this plan nor R72 establishes full HIP/HSA
parity or an unmeasured speedup.
