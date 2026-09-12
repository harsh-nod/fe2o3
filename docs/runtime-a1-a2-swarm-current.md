# Current Runtime Swarm Work Orders

Execution refresh: 2026-09-12. Scope: finish A1/A2, then advance the remaining
[issue #182 milestones](https://github.com/harsh-nod/fe2o3/issues/182).
The issue was checked through the GitHub API and remains open; its reported
`updatedAt` is `2026-09-12T10:51:18Z`.

This document supersedes the immediate assignment rows in the
[detailed dispatch](runtime-a1-a2-swarm-dispatch-r83.md) and
[next-wave roadmap](runtime-a1-a2-next-wave.md), not their historical evidence.

## Renewed Swarm Dispatch

R104 locally accepts [ordinary live-rebind custody](runtime-ordinary-rebind-custody-v1.md),
with [retained evidence](evidence/local-r104-ordinary-rebind-custody-2026-09-12/README.md),
above signed planning parent `5cdeedd8290fac0bf01ca53b01cc12e828a2e20e` and
accepted R103 `61a1479348ec3b744a8881108e059f540f324364`.
All seventeen source gates, ten auxiliary checks and six compiled behavioral
negatives pass. GNU/musl each pass 2,565 tests with five ignored;
frozen/restored rebind and construction suites pass 10/10 and 69/69.
All 5,662 non-documentation source identities are unchanged and exactly restored.
This is CPU/shared-sequence acceptance, not original-engine composition,
live KFD, new solver refinement or matched performance. N2 pristine rebind is
the next Native implementation packet.

The renewed user request is split across three read-only review workers below.
Primary owns implementation, integration, tests, proof registration and signed
publication. The first three packets are independent; shared module wiring and
all builds remain serialized. Each worker returns a bounded source/test
handoff, not an unreviewed concurrent edit to Context or the queue owner.
All three renewed source-grounded review handoffs are complete. Workers made
no edits and ran no builds or hardware jobs. The implementation packets below
are queued work, not unattended background implementation jobs.

| Worker | First packet | Concrete exit requirement |
| --- | --- | --- |
| Native: `native_replacement_handoff` | N2: pristine rebind in `queue_live/rebind.rs`, `queue_live/pristine_abort.rs` and the lower pristine forwarder | Extend the existing root with continuation/preparation custody before the loan; preserve exact next-generation provenance and entered-pristine process poisoning. Reuse R104 settlement and facade transport. Opening failure retains the unconsumed continuation; consumed authority cannot become retry authority. |
| Admission: `submission_identity_handoff` | C1: CO-2A in new `context/tests/submission_identity_tests.rs` | Exercise all eight existing ingresses with exact-coordinate substitution, genuine cached completion, backend-ID reuse, destroyed-stream semantics and rejection precedence. Rejection must leave supplied handles, retained owners and callback state unchanged and precede backend entry. Primary owns module wiring and the missing test-backend cancel counter. |
| Resources: `r102_evidence_review` | V1: VER-1A.2a in new `runtime-model/src/context_version_journal.rs` | Use existing Context IDs, bounded Reserved slots and the frozen activation/watermark policy; test out-of-order reservations, replay, slot reuse and capacity/overflow rejection. Primary owns exports. This is model-only, not a production journal. |
| Primary | I1: integrate accepted packets and maintain qualification gates | Review shared changes, retain failed attempts, run exact-source checks, and publish accepted work to both repositories. Keep local source, model/proof, native and performance acceptance separate. |

### Ordered Backlogs

These labels are short dispatch aliases for the detailed contracts below, not
new mechanisms or proof claims. Within-lane order is the worker's planned
sequence; it is not a prerequisite where the packets are explicitly independent.

| Lane | Ordered packets after the first assignment | Source boundary / acceptance |
| --- | --- | --- |
| Native | N3: data insertion/replacement | Existing `fixed_dispatch.rs` and lower initializers. Retain incomplete prefixes and returned owners; reserve identity capacity before effects and commit metadata only after retake. |
| Native | N4a: detach/release -> N4b: auxiliary/full returning destroy | Existing dispatch release and queue teardown paths. Retain untouched data, controls and the original parent through cleanup failure. Do not resurrect disposed authority; permit slot reuse only after confirmed full disposal. |
| Native | N5: DATA-ADOPT -> joint I2: ISSUE | Existing generated preparation, shell and backend owners. Adoption binds original bytes without publication; ISSUE binds one logical submission and one permit to real resources without retrying uncertain publication. |
| Admission | C2: CO-2B descriptor identity; C3: CO-3A reply/custody gaps | Both are independently ready now using existing authorization validators and R80/R83 lifecycle fixtures. No second validator, reply, decoder or dummy native completion adapter; neither waits for N5. |
| Admission | Joint I2: ISSUE -> C4: CO-4/COMPLETE -> C5: generated typed-output future/API -> C6: generated GRAPH/DRAIN | Extend existing ordinary async futures and graph/drain support. Actual publication/completion identity, complete readback and closing currentness precede retained decoding/readiness. Exercise wake races, bounded admission, observer loss, cancellation and accepted-prefix drain. |
| Resources | V2: .2b membership -> V3: .2c settlement/cost -> V4: .2d authenticated proofs -> V5: .3 production journal | Whole-roster atomic transitions, exact member/free-slot invariants and O(k) touched work. Prove shared definitions, then separately qualify the actual Context commit; an unused hook is not integration. |
| Resources | V6: .4/.5 initial hooks -> V7: complete VER-1B mutation coverage, ordered writers and Unknown recovery -> V8: VER-2 cross-run leases | Every write family must invalidate before effects and settle before callbacks. Do not enable reuse with only host-write/copy coverage or a single-writer staging profile. |
| Resources with Native | M1: aggregate domains/headroom; M2: native backing/control/slot admission; M3: host/native cache residency; M4: total retained-memory bound | Reuse existing accounts, budgets and caches. Domain design and host-image ceiling are independently ready; native residency and compound pre-effect admission depend on backing/control integration. Include terminal retention without double charging. |
| Primary with all lanes | Q1: incremental adapter correspondence; Q2: timing/depth/overlap/fault qualification; Q3: matched HIP/HSA performance | Keep CPU/source, authenticated formal, live KFD and performance acceptance separate. Investigate R103's unaccepted default-concurrency musl watchdog failures without weakening deadlines. |

R104 supplies N1's named local acceptance. The remaining critical integration
path is **N2-N4 -> N5 DATA-ADOPT -> I2 ISSUE ->
C4 COMPLETE -> C5 generated API -> C6 GRAPH/DRAIN**. I2 also needs C1/C2 and the preissue
C3 contract/oracle, but not evidence of its own publication receipt in advance.
The first non-reusing ISSUE needs a specified mutation hook, not completed
cross-run leases. Cross-run reuse does require V7/V8. Model proofs and adapter
correspondence proceed incrementally, not only at the final hardware gate.

Accepted N1 includes the validation call-chain guard, process-wide poison
classification checks and callback-panic/retention ordering after restoration.
Its ten focused test functions contain 112 dynamic scenarios and one textual
routing guard, not 112 successful binds. Actual original engine/account/platform
composition is not established by its separate preparation and engine-free
facade fixtures. The consuming pristine helper remains N2 work. Neither an
injected closure nor a passing ownership snapshot establishes native execution.

C1/C2/C3's proposed test files and V1's model remain absent at this refresh.
Existing Context identity validators, ordinary typed async launches and ordinary
graph/drain paths should be reused. Generated production preparation still
supplies no adoption hooks, and R65's graph-local versions are not a Context
journal or cross-run input lease. These are implementation boundaries, not
reasons to duplicate the existing runtime.

### Later Swarm Rotations

| Milestone | Lead / required outcome |
| --- | --- |
| A3: unified local multi-GPU | Native with Resources/Admission: admitted topology, sharding/replicas, peer or staged transfers, group drain and partial-failure isolation. Existing copy-only XGMI is not unified compute. |
| A4: distributed control | Admission with Primary: authenticated membership epochs, exact receipts and two-host execution without duplicate publication. |
| A5: distributed data and collectives | Native with Resources: bounded transfers and separately qualified broadcast, reduce-scatter, all-gather and all-reduce. |
| A6: failure campaigns | Primary with Admission: device, participant, network and collective faults without unsafe replay, premature release or false completion. |
| A7: performance and release qualification | Primary with Native: matched complete-output HIP/HSA comparisons, device timelines, memory/CPU/tail metrics and dependency/symbol closure. No blanket speedup claim. |

Compiler/device-language and machine-refined atomics/collectives, Worker V3
authority, protected kernels, installation/deployment and debugger work retain
their separate owning issues listed under [Later Milestones](#later-milestones).
Closing A1/A2 does not close them or all of issue #182. MI300X work is scheduled
by Primary using task-owned resources and cleanup; disruptive tests require an
isolated window. No hardware work is launched by this dispatch refresh.

## Accepted Checkpoints

R104 locally accepts **NATIVE-2C ordinary live-rebind custody** above signed
planning parent `5cdeedd8290fac0bf01ca53b01cc12e828a2e20e`, with
[retained evidence](evidence/local-r104-ordinary-rebind-custody-2026-09-12/README.md).
Seventeen source gates, ten auxiliary checks and six compiled behavioral
negatives pass. GNU/musl each pass 2,565 tests with five ignored;
frozen/restored rebind and construction suites pass 10/10 and 69/69.
All 5,662 source identities match, including exact restoration after each
mutation. Nine dynamic tests cover 112 scenarios; a tenth guards source routing.
Original-engine composition, later auxiliary vector slots, pristine prefixes,
native execution, formal correspondence and performance remain open.
Next: **N2 pristine rebind + CO-2A Context identity + VER-1A.2a issuance**.

R103 locally accepts **NATIVE-2C replacement-input custody** above signed R102
`50c4eb075013fde0a984a003c0b5b90eae562847`, with
[retained evidence](evidence/local-r103-replacement-input-custody-2026-09-12/README.md).
Frozen/restored construction suites each pass 69; all seventeen source gates,
ten auxiliary checks and six compiled behavioral negatives pass. GNU/musl each
pass 2,555 tests with five ignored, using four Rust test-harness threads and four
Cargo build jobs for the fresh campaign. All 5,659 source hashes match. The
earlier musl campaign's two watchdog failures remain unaccepted; their cause is
not proved. This is CPU/shared-sequence acceptance, not new formal, live KFD or
performance qualification. At R103, the next Native packet was **live-lane
restoration and rebind custody**;
**CO-2A Context identity** and **VER-1A.2a issuance** are independent.

R102 locally accepts **NATIVE-2B.5B-3C**, the named roster/destination-slot
matrix, above signed R101 `77ce1196f2867e79eb450b5a9ba5924ed13152fa`.
The [R102 record](evidence/local-r102-auxiliary-roster-slots-2026-09-12/README.md)
contains seventeen source gates, ten auxiliary checks and six compiled
behavioral negatives. GNU/musl each pass 2,546 tests with five ignored;
frozen/restored construction suites each pass 60. All 5,658 source hashes
match. Four files contain test-fixture/helper changes only. Two interrupted
construction attempts remain unaccepted. This completes the planned local .5B
matrix, not native/formal qualification. No production mechanism, solver, live
KFD or performance acceptance is added. At R102, next was **2C replacement input + CO-2A +
VER-1A.2a**.

R101 locally accepts **NATIVE-2B.5B-3B**, the named recovery/currentness matrix,
above signed R100 `4424f4607d8a64677556b32713a74b1ca5c6557a`.
The [R101 record](evidence/local-r101-auxiliary-recovery-currentness-2026-09-11/README.md)
contains seventeen source gates, ten auxiliary checks and four compiled
behavioral negatives. GNU/musl each pass 2,544 tests with five ignored;
frozen/restored construction suites each pass 58. All 5,657 source identities
match. Only two test-fixture files change, with no production, solver, live KFD
or performance acceptance. At R101, next was **.5B-3C + CO-2A + VER-1A.2a**.

R99 locally accepts **NATIVE-2B.5B-2**, the named auxiliary CPU/local Linux
platform composition, above signed R98
`7506596f805af49e432aaa4ef66ec9a586ca4734`. The
[R99 record](evidence/local-r99-auxiliary-local-platform-2026-09-11/README.md)
contains seventeen source gates, ten auxiliary checks and five compiled
behavioral negatives. GNU/musl each pass 2,540 tests with five ignored; all
5,655 non-documentation source identities match. No new production mechanism,
solver, live KFD or performance acceptance is added. At R99, next was:
**.5B-3A/B/C + CO-2A + VER-1A.2**.

R100 locally accepts **NATIVE-2B.5B-3A**, the admitted CREATE outcome matrix,
above signed R99 `e6ac41c7fe61fbbb3e9a7003a8e9fd9a8a0c97a4`.
The [R100 record](evidence/local-r100-auxiliary-create-outcomes-2026-09-11/README.md)
contains seventeen source gates, ten auxiliary checks and four compiled
behavioral negatives. GNU/musl each pass 2,542 tests with five ignored;
frozen/restored construction suites each pass 56. All 5,656 source identities
match. Four test-fixture files change, with no production, solver, live KFD or
performance acceptance. At R100, next was **.5B-3B/3C + CO-2A + VER-1A.2a**.

R98 locally accepts **CO-1**, the production-used completion classifier and six
focused CPU tests, plus **VER-1A.1 contract/inventory only**, above signed R97
`1b53ef417d0f4184e2b4e6024b37271b5f719832`. The
[R98 record](evidence/local-r98-completion-contract-2026-09-11/README.md)
contains seventeen successful source gates, eight auxiliary checks and four
compiled behavioral negatives. GNU/musl each pass 2,537 tests with five ignored;
all 5,653 non-documentation source identities match. No solver, live KFD or
performance acceptance is added. At R98, next was **.5B-2 + CO-2A + VER-1A.2**.

R97 locally accepts **NATIVE-2B.5B-1**, production-used outer settlement and the
named CPU/fake-native prefix matrix, above signed planning checkpoint
`82c8cd854bc6cb8b300a4f5a6b9a2467827dcc6f` and signed R96
`369f99835cfb2af5df9fda45cc828d462ef6b156`.
The [R97 record](evidence/local-r97-auxiliary-outer-settlement-2026-09-11/README.md)
contains seventeen successful source gates, twelve auxiliary checks and four
compiled behavioral negatives. GNU/musl each pass 2,531 tests with five ignored;
all 5,651 non-documentation source identities are unchanged and exactly restored.
No new solver, live KFD or performance result is claimed.

R96 locally accepts
**NATIVE-2B.5A**, production-used auxiliary preparation and CREATE/install phase
composition, above signed planning checkpoint
`10902ca32a853448f79b59cfcf22072e3cdd9325`. The preceding accepted runtime source
is signed R95 `da90a0038c6ec4c697faf0fbe93d597e9fa36e1a`.
The [R96 record](evidence/local-r96-auxiliary-shared-engine-2026-09-11/README.md)
contains seventeen final source gates, twelve auxiliary checks and four compiled
negative mutations. GNU/musl each pass 2,519 runtime tests with five ignored;
5,648 source identities remain unchanged and restored. The failed first
full-source attempt and formatting check remain historical, not acceptance.

At accepted R96, the shared-engine fixtures cover success and eight late-failure
cells with original owners, plus a production-glue guard. They use a fixture outer scope
and scripted platform leaves, not complete concrete Linux outer settlement.
After R99, the remaining native sequence is **.5B-3 -> 2C -> DATA-ADOPT**.
R96's duplicate-primary-ID case reaches lower CREATE
`Ambiguous` rejection with no committed ID or outputs. It does not exercise the
later retained auxiliary/SDMA roster rejection.

### Accepted R97 Scope

The .5B-1 implementation has a private original-parent adapter, an owning
outer scope and one production-used auxiliary driver. The shared-engine fixture
calls that driver instead of copying its orchestration. The separate early-prefix
oracle and failure matrix now exist: original-parent transport and poisoned
ledgers, exact memory/account/token ownership, real loan/reclaim rejection,
preparation/control faults and cleanup-panic precedence are exercised without
weakening R96's late oracle.

Fourteen integrated functions, twelve new over R96, cover 369 shared auxiliary
driver runs: 366 failures and three successes. One separate primary-only capacity
fixture calls the real borrowed preflight twice. The control sweep pins 36
boundary occurrences and 72 error/panic cells; native allocation/map/seal/write
and projection failures retain exact returned or in-session owners. Both frozen
and restored construction suites pass 53 tests. All four final mutations compile
and fail their intended behavioral test; the retake mutation detects lost error
precedence, not an observed extra CREATE.

At R97, the next Native task was **.5B-2 local platform composition**, followed by
**.5B-3 CREATE/installation coverage**. R97's scripted platform leaves do not
qualify the concrete Linux platform composition. Callback failures are tested
before allocating/returning owners, not for callback-internal unreturned custody.
Append-only pending-slot tests do not qualify native released-slot reuse.

### Accepted R98 Scope

The [CO-1 classifier](runtime-completion-observation-contract-v1.md) is now
implemented and consumed by ordinary operation progress. Six frozen/restored
tests and four compiled behavioral negatives pass; all 5,653 source identities
are exactly restored. Full GNU/musl suites each pass 2,537 tests with five ignored.
All seventeen full-source gates and eight focused lifecycle/audit checks pass.
The earlier six-pass formatting-overlap run is not acceptance.

Resources' [VER-1A.1 contract/inventory](runtime-context-version-journal-v1.md)
now exists and has been independently reviewed. Journal/model implementation is
still absent. Ordered writers and recovery are mandatory before whole-surface
reuse; the initial one-writer profile must not silently restrict ordinary work.
No new formal, live KFD or performance acceptance follows from either packet.

### Accepted R99 Scope

The .5B-2 implementation composes one fixture-local runtime registration/gate/phase
with the original primary and shared auxiliary driver. Seventeen scenarios run
through both original-runtime routes, for 34 auxiliary constructions. The first
matrix, two registration tests, 32 frozen integration tests and all-feature/all-target
Clippy pass. Five compiled behavioral negatives reject and all 5,655 source
identities are restored. The broader 54-test restored construction run also
passes with unchanged source; all seventeen source gates and ten auxiliary
checks pass, with independently reviewed source and evidence. Local mappings,
event bindings, protection and cleanup are real
helpers; CREATE remains scripted successful. .5B-3, native qualification and
formal adapter correspondence are not closed by this packet. Only seven
test-fixture files change; production mechanisms and proof inputs remain unchanged.

### Accepted R100 Scope

The .5B-3A packet adds an admitted CREATE oracle while preserving the prior
strict successful-CREATE assertions. Seven outcomes through both original-runtime
routes cover fourteen auxiliary constructions: definite no-effect, indeterminate
with/without returned ID, input drift, panic, invalid successful outputs and
changed outputs accompanying failed-no-effect. Both focused tests pass after
correcting the new oracle to recognize the intentionally retained empty resource
prefix. The original failure is retained; no production change was needed.
Both frozen/restored construction suites pass all 56 tests. Four compiled
behavioral negatives reject, and the exact frozen source is restored. All
seventeen final source gates and ten auxiliary checks pass.
At R100, 3B/3C, live KFD, formal correspondence and performance remained open.

### Accepted R101 Scope

Two test functions cover thirty failures plus two successful trace baselines
through both original-runtime routes. Sixteen currentness failures distinguish
pre/post-CREATE and pre/post-doorbell Err/panic. Fourteen callback failures cover
runtime-created, output recovery, ID recovery and event-ID panic. Exact history,
error/panic precedence, owner placement and local registration/cleanup are
checked without weakening the earlier CREATE or successful-pair oracles.
All 58 frozen/restored construction tests pass, as do seventeen final source
gates and ten auxiliary checks. All four compiled behavioral negatives reject;
all 5,657 source hashes are restored. At R101, 3C, 2C and DATA-ADOPT remained
open, along with formal correspondence, live KFD and performance qualification.

### Accepted R102 Scope

Two tests cover sixteen constructions: six retained-roster rejections, eight
late-slot rejections and two reuse successes, plus four separate borrowed
preflight checks. Exact candidate custody is distinct from ID-only SDMA and
deliberately inconsistent auxiliary metadata; existing first-slot assertions
are preserved. All 60 frozen/restored construction tests, seventeen source
gates and ten auxiliary checks pass. Six compiled mutations reject and all
5,658 source hashes are restored. Generation-4-to-5 metadata reuse does not
qualify native teardown/recreation or incarnation succession. Callback-internal
custody, 2C, DATA-ADOPT, formal correspondence, live KFD and performance remain
open.

## Swarm Ownership

### Remaining Work At A Glance

| Order | Lead | Bounded deliverable | Dependency / acceptance |
| --- | --- | --- | --- |
| Locally accepted Native packets | Primary + Native review | 2C: replacement inputs and ordinary live rebind | R103 roots replacement inputs; R104 retains ordinary preparation through settlement and restores lanes before terminal transport. CPU/shared-sequence acceptance only |
| Next Native packets | Native | 2C pristine rebind, insertion and release, then DATA-ADOPT | Retain continuation and incomplete prefixes; preserve exact provenance and require confirmed disposal |
| First Admission packet | Admission | CO-2A: Context submission-identity tests | Existing validators; eight ingresses, stale/reused IDs and preserved precedence |
| Next Admission packets | Admission | CO-2B descriptor identity and CO-3A private reply/custody composition | Existing source matching and R80/R83 lifecycle; no invented completion adapter |
| First Resources packets | Resources | VER-1A.2a-d: issuance, membership, settlement/cost, authenticated proofs | Reviewed contract; model-only until .3 has a production consumer |
| Next Resources packets | Resources | VER-1A.3 journal, .4/.5 hooks, complete VER-1B, then VER-2 leases | Exact Context IDs; complete mutation coverage, ordered writers and recovery before reuse |
| Native/runtime integration | Primary + Native/Admission | 2C -> DATA-ADOPT -> ISSUE -> CO-4/COMPLETE -> typed API -> GRAPH/DRAIN | Real publication/completion identity, exact readback and custody; cross-run reuse also needs VER-2 |
| Resource closure | Resources + Native | Aggregate domains, native backing/control budgets, host/native residency and total retained memory | Charged bootstrap/terminal headroom, compound pre-effect admission and no double charging |
| Qualification | Primary + all lanes | Incremental adapter proofs, native depth/overlap/fault tests, matched HIP/HSA workloads | Separate authenticated proof, hardware and performance results; shared-machine scheduling |

The three workers reviewed these assignments independently. Their bounded review
turns are complete; queued implementation packets are not running unattended.
Later A3-A7 and compiler/Worker/release handoffs remain below, outside A1/A2.

At the user's renewed swarm request, three existing workers independently
reviewed the current source, R104's local scope and the remaining roadmap. They
returned the bounded work orders below. Their read-only review turns
are complete. The implementation queues below are assignments, not unattended
background jobs. Primary owns
edits, integration, conflict resolution, tests, proofs and publication. Shared
Context/backend/queue changes and builds are serialized.

| Lane and worker | First bounded task | Follow-on queue |
| --- | --- | --- |
| Native: `native_replacement_handoff` | N2 pristine rebind handoff ready above locally accepted R104 | Insertion/release/teardown -> generated data adoption -> native publication handoff |
| Admission: `submission_identity_handoff` | CO-2A identity/oracle handoff ready | CO-2B descriptive identity and CO-3 reply/custody composition -> issue/completion integration -> typed future -> generated graph/drain |
| Resources: `r102_evidence_review` | VER-1A.2a issuance handoff ready | Membership -> settlement/cost -> proofs -> production journal -> complete mutation hooks/ordered writers/recovery -> cross-run leases |
| Primary | Integrate 2C, CO-2A and VER-1A.2 without conflicting shared edits | Cross-lane integration, formal correspondence, hardware scheduling, matched benchmarks and signed pushes to both topic remotes |

### Immediate Handoffs

| Lane | First deliverable | Source boundary and cross-review |
| --- | --- | --- |
| Native | N2 pristine rebind custody after R104 local acceptance | Extend `queue_live/rebind.rs`; replace consuming rebind orchestration/forwarding in the two `pristine_abort.rs` modules. Root continuation/preparation before the loan, preserve exact next generation and entered-pristine poison policy, and reuse settled commit/facade transport. Abort/control disposal is unchanged. |
| Admission | CO-2A five-case submission-identity matrix using the existing Context validators | New `context/tests/submission_identity_tests.rs`; Primary wires `context.rs`. Resources checks stale/reused identity; Native checks rejection before backend entry. No replacement validator or native receipt. |
| Resources | VER-1A.2a executable issuance model, then .2b-d membership/settlement/proofs | New `runtime-model/src/context_version_journal.rs`; use the frozen activation/watermark policy and explicit A/W capacities. Reuse Context IDs, bounded Reserved slots and monotonic issuance. Production consumption follows in .3; Primary owns exports and authenticated pins. |
| Primary | Integrate one reviewed packet at a time and record its exact acceptance scope | Shared Context/backend/queue edits, builds, proof runs, hardware and publication remain serialized. |

R98 supplies `completion_contract.rs` and the reviewed journal contract;
`context/versions.rs` and its executable model remain absent. R104 locally
accepts ordinary live rebind after R103 replacement-input custody. The next wave is
**N2 pristine rebind + CO-2A + VER-1A.2a**. CO-2B descriptor
coverage and the CO-3 gap audit can use the next available review slot without
waiting for native adoption. Resources implements the model before the journal.
The .5B-3A/B/C designs are independently reviewable, but their integration shares
the primary trace, platform and memory fixtures. Shared module wiring, Context
mutation hooks, backend issue and fixture edits pass through Primary one packet
at a time.

### Ready And Dependent Work

| Wave | Native | Admission | Resources |
| --- | --- | --- | --- |
| Ready now | N2 pristine rebind | CO-2A Context identity is already independent; reviewed CO-2B and CO-3A handoffs | VER-1A.2a issuance model is already independent |
| After each lane's first gate | N3 insertion, N4 detach/release/returning destroy, then N5 DATA-ADOPT | CO-2B descriptor matrix and CO-3A lifecycle composition | .2b membership -> .2c settlement/cost -> .2d proof acceptance -> .3 bounded journal |
| Integration | 2C replacement/insertion, then nonpublishing DATA-ADOPT | ISSUE with Native, then CO-4/COMPLETE and typed API | .4/.5 initial mutation hooks, then complete VER-1B with ordered writers and recovery |
| A1/A2 closure | Native depth, memory pressure and overlap qualification | Generated GRAPH/DRAIN and end-to-end typed execution | VER-2 cross-run leases, aggregate accounting and residency closure |

CO-2A/2B and the preissue CO-3 contract/oracle do not require native adoption.
Native identity checks join actual DATA-ADOPT/ISSUE records and close at CO-4;
ISSUE cannot depend on preexisting proof of its own publication receipt.
First non-reusing ISSUE does not require cross-run leases, but its mutation hook
must be specified with Resources.
Cross-run reuse does require complete VER-1B and VER-2. Formal correspondence is
reviewed with each packet; it is not deferred to the final hardware wave.

## Native Queue

1. **R95 locally accepted.** Opening currentness now runs inside retained
   construction custody before the model loan. The shared borrowed preflight
   preserves pure `JournalCapacity` rejection. Error/panic tests retain the exact
   original parent without preparation, loan or retake; the auxiliary single-owner
   guard and both ledger poison assertions are present. Eight compiled mutations
   reject and exact source restoration passes. All seventeen final source gates
   and twelve auxiliary checks pass; broader native integration remains open.
2. **NATIVE-2B.5: R96 accepts .5A; R97 accepts .5B-1; R99 accepts .5B-2.**
   R100/R101/R102 accept the named local .5B-3A/3B/3C matrices below.
   Run the actual
   production-used sequence after a completed primary constructor, borrowing
   the same engine, foundation, memory/accounts and original platform owners.
   Compare exact primary-plus-auxiliary owners, not just counters. A second
   scripted constructor is not acceptance.
3. **NATIVE-2C: replacement/insertion custody.** Retain consumed session/data
   before planning and returned owners through closing observation. Cover
   occupied slots, exhausted generations, old/new identities and recycled
   versus pristine-abort provenance.
4. **DATA-ADOPT.** Install generated adoption only after 2B/2C acceptance. Enter
   non-discardable native custody before effects; bind original bytes without
   publication. Cover initial, auxiliary and reused lanes, partial failure,
   Stop/drain and exact abort/disposal. Reuse existing R80 reservations and R83
   lifecycle rather than adding duplicate allocation or completion ownership.
5. **ISSUE handoff.** With Admission, bind one linear permit to actual resources
   and one logical submission through deferred flush/retry. Definite
   nonpublication and uncertain publication remain distinct.

### Auxiliary Acceptance Packets

Paths below are relative to `crates/fe2o3-kfd/src/`. Primary owns edits and
execution; Native owns each design/review handoff.

| Packet | Dependency and affected files | Exit gate |
| --- | --- | --- |
| .5A accepted R96 | `queue_live.rs`, `queue_live/construction_auxiliary` and shared primary/preparation fixtures | Frozen focused suite, all four final compiled negative mutations, all seventeen source gates and twelve auxiliary checks pass. Exact source is restored; final evidence is independently reviewed. Full outer/platform acceptance remains .5B. |
| .5B-1 accepted R97 | After .5A; `queue_live/construction_auxiliary.rs`, its integration tests and narrow real loan/reclaim fixture forwards | Shared production outer driver after successful primary construction; named early-prefix/operation/reclaim/cleanup matrix, full-parent transport and pure borrowed capacity rejection. Frozen/restored 53-test suites, all 17 source gates, 12 auxiliary checks and four behavioral mutations pass. Concrete local platform and complete CREATE/install coverage remain below. |
| .5B-2 accepted R99 | After .5B-1; primary `integration_platform.rs`, auxiliary platform cases and existing `queue_linux/primary_fixture.rs` helpers | Named 34-case matrix plus two registration tests; 32 frozen integration and 54 restored construction tests, 17 source gates, 10 auxiliary checks and five compiled negatives pass. Original primary retention, actual local lease/phase, cleanup and event binding are checked. Local Linux mappings are not live KFD qualification. |
| .5B-3A accepted R100 | After .5B-2; new auxiliary CREATE tests and three narrow existing fixture changes | Seven scenarios through both runtime routes, 14 failures, exact original ownership/history and admitted-prefix distinctions. Frozen/restored 56-test suites, 17 source gates, 10 auxiliary checks and four compiled negatives pass with 5,656 restored source hashes. |
| .5B-3B accepted R101 | After 3A; new auxiliary recovery tests and prefix-module wiring only | 30 failures plus two trace baselines, exact history/currentness and phase-dependent owners. Frozen/restored 58-test suites, 17 source gates, 10 auxiliary checks and four compiled negatives pass with 5,657 restored source hashes. |
| .5B-3 CREATE and installation | After .5B-1; combine with .5B-2 for platform cells; auxiliary integration tests and narrow existing fixture injections | Cover no-effect, indeterminate, malformed and panicking CREATE; output/ID recovery; retained auxiliary/SDMA roster collisions; pre/post-CREATE and pre/post-doorbell currentness; doorbell/gate failures; occupied and reusable slots. No failed installation, spent-generation reuse or lost original owner. Full-source and compiled mutation gates close only this named CPU/local-helper matrix. |

### R97 Completed Work Units

| Unit | Deliverable and exit assertion |
| --- | --- |
| R97-1: early-prefix oracle, locally accepted | Separate stage-aware oracle preserves R96's stricter late `assert_pair`. Join original primary owners with auxiliary data, preparation/control prefixes, real terminal tokens and exact account/native records, without double counting markers. Observe real foundation location rather than assuming reclaim succeeded. |
| R97-1: terminal transport, locally accepted | On admitted terminal failure require an empty live parent slot, occupied terminal-parent slot and actual poisoned ledgers. On success require the inverse; pure pre-effect rejection leaves the original parent unchanged. Snapshot equality alone misses poison and slot-transfer omissions. |
| R97-2: ingress/opening, locally accepted | Pure capacity rejection has no opening, loan, preparation, retake or CREATE. Opening and loan errors/panics preserve the original parent; no retake without a returned loan. Capacity pressure uses model-only history and the real borrowed preflight, not the full public Linux entrypoint. |
| R97-2: returned prefixes, locally accepted | Exercise the named preparation/control error-and-panic matrix after original primary success. Auxiliary code-memory ordinals are session-global 3-5; preparation-stage ordinals remain local 0-2. Preserve the trace and exact original bytes, owners and charges. Callback failure before allocating or returning owners does not qualify callback-internal unreturned ownership. |
| R97-2: operation/reclaim, locally accepted | Cross operation success/error/panic with reclaim success/pre-error/pre-panic/post-error/post-panic. Exercise actual loan/reclaim and separately genuine certificate rejection. Failed reclaim permits no CREATE. |
| R97-2: cleanup, locally accepted | Store the complete terminal parent before cleanup; preserve the first panic even when cleanup panics. |
| R97-3: local acceptance complete | Final lint, frozen/restored tests, four compiled behavioral negatives, all 17 source gates and 12 auxiliary checks pass with 5,651 exact source identities. Retained evidence includes preliminary failed attempts; independent source/evidence review covers only the named .5B-1 scope. |

### R99 Implemented Local Platform Handoff

R99 installs one fixture-local gate before original primary
construction and retains the same local resources through auxiliary construction.
Runtime admission is fallible before minting a fixture owner, so a real gate
rejection cannot create a phantom owner. It reuses actual local registration/phase,
shadow initialization, protection, restore, publication and cleanup helpers,
without a second constructor or a fake production runtime descriptor whose Drop
touches the process-global gate.

The test-local `LocalRuntimeRegistrationV1` retains the exact local gate,
opener PID and runtime phase, using existing `admit_runtime`,
`commit_first_enabled` and `admit_runtime_transition` helpers. Its Drop poisons
only that local gate, not a fabricated successful native disable. The existing
primary fixture's post-Drop expectation reflects this; owner identity is observed
independently of whether the retained gate has become poisoned.

The matrix covers success, admission/arm/event/install rejection, shadow-init and
restore error/panic, cleanup panic before/after disposal, late doorbell/gate
failure and cross-event substitution. It asserts original primary retention,
auxiliary-only unpublished cleanup and both published payloads retained after
late failure, inspecting owned local mappings before fixture disposal. Synthetic
events and independent VM reservations are not real KFD BO aliasing, native
doorbell execution, runtime-enable ioctls or confirmed native teardown.

The subsequent 2C packet owns existing replacement in
`queue_live.rs::recreate_compute_aql_queue_with_fixed_dispatch` and binding in
`queue_live/fixed_dispatch.rs`, not a second constructor. DATA-ADOPT then owns
the proposed runtime `kfd_backend/generated_adoption.rs` and narrow existing
shell/Context/compute hooks. Local slot-reuse tests do not establish a native
destroy/recreate cycle.

Full 2B acceptance still does not establish callback-internal unreturned-owner
custody, concurrent bootstrap, live device behavior, new formal refinement or
performance. Keep those boundaries explicit rather than absorbing them into a
green fixture count.

### .5B-3 Patch-Ready Handoff

Native's three bounded local matrix packets are accepted. **3A**, accepted R100,
adds the admitted-prefix oracle and CREATE outcome/malformed-output matrix. **3B**,
accepted R101, covers recovery, currentness and assembly. **3C**, accepted R102,
adds retained-roster and destination-slot coverage. Shared fixture edits and
builds were serialized. All three have focused tests, compiled behavioral
negatives and applicable full gates; none supplies native or formal acceptance
merely by passing its CPU matrix.

R100 adds a separate admitted-prefix oracle without weakening `assert_pair`, which
assumes successful CREATE outputs except its existing duplicate-primary-ID case.
Its CREATE matrix covers unsuccessful CREATE while retaining exact original
engine/account owners. 3B adds the admitted-but-unpublished boundary.

| Group | Existing control or narrow addition | Required distinction |
| --- | --- | --- |
| CREATE outcomes, accepted R100 | Set `Trace.create` modes 1-5 only after primary success | No-effect leaves `Planned`; indeterminate/drift leaves `Ambiguous`; panic leaves `CreatePending`. Returned ID and admitted outputs differ. All five published local shadow payloads; none permits retry or unpublished-shadow cleanup. |
| Malformed outputs, accepted R100 | Fake-leaf cases for successful invalid doorbell output and failed-no-effect with changed outputs | Reject without manufacturing accepted output authority or dropping the rooted prefix. |
| Recovery/assembly, accepted R101 | Existing `Trace.fault` at recovery, runtime-created and event-id boundaries | Engine-owned outputs survive before construction output recovery; queue confirmation alone does not advance the local runtime phase. |
| Currentness, accepted R101 | Successful-trace occurrences for pre-CREATE, post-CREATE, pre-doorbell and post-doorbell Err/panic | Pre-CREATE retains unpublished state and performs no CREATE; post-CREATE retains engine outputs; doorbell stages retain the completed lane with absent/present doorbell respectively. |
| Roster/slot checks, accepted R102 | Explicit fixture-owned retained rosters and real prepare/check/install slot helpers | No destination overwrite, lost roster owner or spent-generation reuse; distinguish pure preflight from late retained failure. |

The current profile permits exactly primary plus one auxiliary lane. A second
live auxiliary is not a valid-profile success fixture. Label injected late
roster/slot inconsistency honestly; natural duplicate IDs reject earlier in the
shared engine. Directional/striped SDMA roster checks need narrow rooted
test-only inputs and do not qualify native SDMA bootstrap. Generalize target-slot
observations instead of globally weakening first-slot/resource-count assertions.
The existing runner unwraps initial slot preparation, so pure rejection tests
must call the real preflight directly against the retained successful primary.

For 3B, capture one successful auxiliary trace per runtime route and select
unique auxiliary windows around `currentness/publish/create`,
`create/currentness/runtime-created`, `event-id/currentness/doorbell` and
`doorbell-observe/currentness/gate-finish`. Convert positions into the existing
global currentness occurrence count instead of hard-coding ordinals.
Returned currentness errors append `CurrentnessLost` for primary then auxiliary,
set both model phases Ambiguous and set engine poison. Panic bypasses that
transition: do not fabricate a history event or engine-poison bit. Preserve the
original history prefix; change only the expected primary model phase for those
returned errors, never its memory/account/platform identities or ledger storage.

Pre-CREATE retains cleaned unpublished state, no CREATE or outputs; post-CREATE
retains engine outputs but no construction outputs or queue-live registration.
Doorbell-side failures retain the completed lane with absent/present doorbell
respectively and a queue-live registration. Add Err/panic for runtime-created,
output recovery and ID recovery, plus event-ID panic (its callback cannot return
an error). These callback failures retain Active model state without currentness
quarantine. Exact original error stages and first panic remain required. The
accepted R101 matrix has thirty failure cells plus two successful trace baselines;
it does not close 3C roster/slot or native qualification.

### Accepted .5B-3C Roster And Slot Matrix

R102 uses the same auxiliary fixture and unchanged shared driver, with explicit
retained-roster observations. `Original` now retains optional SDMA fixture
inputs and `Parent::target` borrows them. Noncollision, auxiliary, directional
SDMA and striped SDMA collision cases run after successful scripted CREATE.
Collisions reject before event observation, assembly or doorbell and retain
engine-confirmed outputs, recovered construction outputs and all unassembled
owners. ID-only inputs are defensive observations, not proof of native SDMA
bootstrap or complete prior-native-owner custody.

The fixture factors `run_auxiliary_with_slot` around the existing driver. A vacant
metadata slot at generation 4 installs the real constructed auxiliary at 5
without vector growth; the prior generation remains rejected. Occupied real
successful slots and exhausted vacant generations use direct borrowed preflight
and must leave the original scope unchanged without opening, loan or CREATE.
Deliberately altered inert prepared index/generation/kind or an unreserved append
destination exercises late rejection: completed bundle, mapped doorbell and
unfinished creation arm stay rooted, with no gate finalization or installation.

Do not repeat the existing eleven-case pure vacancy-drift test, no-growth test
or stale-handle unit tests. Preserve strict original account/platform/ledger
oracles and add expected target/roster coordinates explicitly. These failures do
not fabricate `CurrentnessLost`. The two-compute-lane profile still excludes a
second simultaneously live auxiliary as a positive fixture.

The accepted matrix has eight scenarios through both runtime routes: sixteen
auxiliary constructions. Noncolliding SDMA observations accompany each
generation-4-to-5 success, plus the three roster collisions and four late-slot
failures above. Each successful fixture checks exhausted-generation preflight
before construction and occupied-slot preflight afterward, for four pure checks.
The new `integration_roster_slot_tests.rs` uses narrow fixture runner/target
inputs and explicitly ID-only SDMA observations. Existing first-slot oracle
wrappers are preserved; the new oracle selects its actual candidate lane
explicitly and checks injected metadata separately.

Actual destroy/recreate, consumed-session replacement, retired/detached data
insertion, old/new typed owner retention through loan/retake and native
generation succession belong to **2C**, including recycled versus pristine-abort
provenance. A successful metadata-slot test does not close those lifecycles.

### 2C Production Custody Handoff

The read-only Native audit identifies the following implementation packets after
3C. These are source-level typed-ownership gaps, not reproduced native frees or
accounting refunds. Reuse existing preparation, allocation and release sequences.

| Order | Production boundary | Bounded change and acceptance |
| --- | --- | --- |
| Replacement input: R103 locally accepted | `Gfx942RecycledDispatchResourcesV1::recreate_compute_aql_queue_with_fixed_dispatch` in `queue_live.rs` | Original memory/programs/packets/data/predecessor metadata are rooted before validation/planning. The generation-aware in-place R89 forwarder is implemented. The named matrix covers invalid ring/program/generation, planning/preparation Err/panic and late construction rejection. This does not qualify preceding destruction or all invalid geometry contracts. |
| Ordinary live rebind: R104 locally accepted | `bind_fixed_dispatch` in `queue_live/fixed_dispatch.rs`, settlement in `queue_live/rebind.rs` | The [ordinary rebind contract](runtime-ordinary-rebind-custody-v1.md) roots original inputs and R89 preparation before the loan; retains completed preparation across retake/validation; commits only after checked extraction; and defers monotonic terminal transport until lane restoration. Original-engine composition remains separately open. |
| Pristine rebind | `queue_live/pristine_abort.rs` | Preserve R82 continuation/provenance and settlement, but retain preparation prefixes in place rather than only its successful return. A pristine continuation never grants recycled-generation authority. |
| Insertion/replacement | Detached data methods in `queue_live/fixed_dispatch.rs` | Reserve identity-vector capacity before effects; retain returned typed owners outside the loan callback; commit ordinal/count after retake. Exercise occupied/reserved ordinals, capacity failure and closing rejection. Lower coherent/device initializers also need in-place prefix retention wherever callback-internal custody is claimed. |
| Detach/release/returning destroy | `fixed_dispatch.rs`, `queue_dispatch_binding.rs` and `queue_live.rs` release paths, including `destroy_auxiliary_compute_lane_v1` | Root untouched data, remaining controls and original session/accounts around fallible cleanup. Auxiliary removal must retain the taken lane and every teardown prefix on Err/panic; a reusable vacancy requires confirmed full disposal. Test first/middle/last disposal failure and successful disposal followed by failed retake; do not resurrect disposed authority or charges. |

R103 uses the existing generic primary root without new root fields: its
preparation payload owns the destroyed receipt, predecessor generation, original
program vector and `FixedDispatchPreparationCustodyV1`. It uses `after_recycled`,
not `after_detached`: zero remains stale. The matrix tests zero/exhaustion
rejection, ordinary 7-to-8 replacement and the last admissible successor.

The preparation snapshot now includes complete packet descriptors and original
boxed backing; replacement-local assertions add program/vector identities, the
receipt and predecessor generation. Data ownership and vector backing are
preserved through partial preparation and completed-dispatch transfer. Envelopes
still borrow caller module bytes; retaining their vector does not own those
bytes. R103 locally accepts the named matrix. These CPU receipt observations do not
establish preceding native destruction, native incarnation succession or formal
adapter correspondence.

Live packets have an additional facade dependency: `with_compute_lane_v1` holds
the original primary in stack-local `selected`, swaps the auxiliary into `self`,
and later restores through the original auxiliary vector index. Calling
`take_for_terminal_auxiliary_construction_v1` inside that callback would omit
`selected` and invalidate restoration. Whole-session terminal transport must
happen after restoration, or the facade must root selected-lane state and the
pending operation together. Test identical primary/auxiliary failures and exact
primary restoration before terminal transport. The replacement-input packet is
independent of this live-lane change.

R104 reuses `model_loan.rs` unchanged. The completed owner stays inside
preparation through retake and post-retake validation, followed by checked
extraction and the existing nonfallible dispatch/ledger commit. Its private,
monotonic facade request defers whole-parent transport until restoration, even
when a callback swallows a bind error or catches its panic. Callback-panic
poisoning precedes transport. The direct wrapper transports immediately;
healthy preflight rejection does not newly terminalize the parent.

The ordinary in-place forwarder passes `after_detached` directly to preparation,
not through R103's recycled-only forwarder. Helper-level zero behavior is
preserved; actual recycled detach rejects zero. Operation panic wins over
closing failure; absent an operation panic, retake error/panic retains its
existing precedence. Returned ordinary errors do not acquire blanket process
poisoning. Existing loan/pristine helpers retain their additional poison policy.

N1's [local acceptance](evidence/local-r104-ordinary-rebind-custody-2026-09-12/README.md)
contains seventeen source gates, ten auxiliary checks and six compiled dynamic
negatives: dropped input root, skipped validation, extracted failed Complete,
overwritten transport request, omitted lane restoration and globalized returned
error. Every source identity was restored after each mutation. Frozen/restored
rebind suites pass 10/10; construction suites pass 69/69. Original-engine
composition, later auxiliary vector slots beyond the first, native incarnation
succession, new formal correspondence and performance remain unqualified.

N2's next implementation handoff is:

1. Extend `LiveRebindRootV1` with `Option<PristineDispatchContinuationV1>` and
   an entered-pristine marker. After unchanged successful pristine preflight,
   root the continuation and preparation before opening the common loan.
2. Replace the consuming pristine forwarder with an in-place preparation call
   using borrowed programs and `continuation.resume()` directly, without `?`.
   Generation rejection must enter failed-Generation custody. Opening failure
   retains the unconsumed continuation; entered preparation consumes it once.
3. Preserve the continuation's exact next generation and mint a fresh recipe
   occurrence, not predecessor-plus-one or recycled authority. Never restore a
   consumed continuation for retry. Reuse R104 validation, commit and facade
   transport; remove the redundant old rebind settlement, not abort/disposal.
4. Preserve entered-pristine local/process-terminal errors without changing
   ordinary or preflight policy. Migrate failure assertions from the old
   installed `session.dispatch` to exact retained preparation ownership.
5. Cover real continuation generations 1/7/8/last, fresh occurrences, opening
   rejection/panic/exhaustion, operation crossed with retake faults, every
   preparation stage, failed Complete, validation and swallowed primary/auxiliary
   failures. Preserve R104 ordinary and R82 pristine regressions; add decisive
   compiled negatives and exact-source restoration. Native composition remains
   separate qualification.

N3/N4 lower-helper reviews are independent; shared live-lane edits integrate
serially after N2. Split N3's lower initializer-prefix custody
from its live insertion integration; returned-value retention alone does not
cover an initializer's incomplete internal prefix.

Actual destroy/recreate, native backing, concurrent bootstrap and native generation
succession still require separately scheduled hardware qualification. Successful
return-value custody alone does not qualify unreturned lower-callback prefixes.

## Admission Queue

1. **CO-1: locally accepted R98.** Separate observations, reply
   disposition and permission to dispose owners. Table-test pending, rejected,
   success-candidate, failure/cancellation, quiescence without result, uncertainty
   and already-settled states. Success observation alone permits no decode or
   disposal; rejected observation permits re-observation, not reissue.
2. **CO-2: exact identity.** Independently mutate source/preparation, Context
   generation, submissions, device/stream/lane, queue/publication occurrence and
   every allocation incarnation. Reject substitution/replay without consuming
   the retained owner or reserved reply.
3. **CO-3: reply and custody composition.** After CO-1, review alongside CO-2.
   Cover repeated rejection, late failure, partial retirement, adapter panic,
   observer loss, Stop/shutdown and waker replacement. Assert callback ordering,
   zero/one adapter calls and one existing reply. Use a data-only adapter;
   runtime must not depend on host-private decoding.
4. **ISSUE -> CO-4/COMPLETE.** Requires DATA-ADOPT, CO-1, CO-2A/2B and the
   preissue CO-3 contract/oracle. Native identity checks integrate with ISSUE
   and close at CO-4. Exact native
   completion, complete readback, closing currentness and native disposition
   precede decode/readiness. Connect the existing R85 decoder; do not reserve
   the R80 reply/readback roster again.
5. **API -> GRAPH/DRAIN.** One executor-neutral typed future, with blocking as
   a join over that same path. Test poll/wake races, capacity, reentrancy,
   cancellation and owner-local non-Send contracts. Then qualify repeated
   generated graphs, dependencies, accepted-prefix drain and dropped observers.
   Cross-run input reuse additionally requires the complete version journal.

### CO-1 Implemented Boundary

The private classifier is implemented at
`async_engine/generated_operation/completion_contract.rs`. Its initial real
production consumer is ordinary `async_engine/operation.rs::Operation::advance`
after submission, not generated preparation, whose adoption hooks remain absent.
Borrow `&Result<RuntimeCompletionStatusV1, RuntimeErrorV1<E>>` so non-`Clone`
backend errors remain owned by their existing path. Preserve one poll/query
sequence, rejection counters, raw reply timing and Context custody.

Classification is descriptive: no class grants typed decode, disposal, retry or
native authority. Use existing `Reply::complete` / `r61_reply_may_resolve_v1` for
the already-settled gate instead of adding another settlement state machine.
Generated preparation, the R80 roster and its reserved reply are downstream
consumers, not resources for CO-1 to recreate. Do not add another public
completion API, native receipt, readback allocation, decoder or host dependency.

| Observation family | Required classification/test |
| --- | --- |
| Pending or rejected | Reply remains pending; retain ownership. Re-observation is allowed, reissue is not. |
| Succeeded | Candidate only, with no decode, readiness or disposal authority. |
| Failed or cancelled | No typed output; failure reporting and confirmed native retirement are separate. |
| Quiescent without result | No successful content or decode permission. |
| Terminal or uncertain | Error reporting cannot release potentially live ownership or authorize retry. |
| Already settled or replayed | No second reply, adapter call or state reopening. |

The first focused tests cover each row, non-consuming/non-`Clone` error handling,
sticky settlement and allocation-free classification over prebuilt inputs using
the existing test-only allocator counter. Reuse ordinary rejected-poll,
cancellation and DRN-3A regressions to check production behavior. No adapter is
called by this classifier. Later
CO-3 composes the existing reservation/adoption Harness rather than duplicating
its Stop, observer-loss and waker machinery. Production adoption hooks remain
absent; R85 decoding is implemented but not native-connected.

### CO-2A Next Handoff

The current source review finds a test-coverage packet, not a missing production
validator. After CO-1 acceptance, exercise the existing Context `submission_record` and
`live_submission_record` gates without adding another validator. Admission owns
a proposed `context/tests/submission_identity_tests.rs`; Primary owns its module
wiring. Exercise all eight ingresses: poll, wait, pure query, event recording,
completion callbacks, cancel, drain and consuming release. The five focused
test groups are:

1. Exact coordinates: independently change Context brand, logical submission
   ID, backend submission, stream or device using real neighboring handles.
   This plans forty rejection cases, with valid controls.
2. Cached success: repeat those coordinates separately with handle-only cached
   success and genuinely completed retained records. Neither cache may bypass
   exact identity validation; use fresh fixtures for each ingress/cache mode.
3. Backend ID reuse: genuinely complete and release the original, reuse its
   backend ID for a new logical submission, and reject the old snapshot without
   affecting the replacement owner.
4. Retained versus live: actually destroy the stream, then check both valid and
   malformed handles. Poll/wait/event require live binding; retained query,
   callback, cancel, drain and release keep their distinct existing semantics.
5. Precedence: preserve Context-terminal, graph-reservation and deadline
   ordering for valid and malformed handles; pure query remains available.

Identity rejections precede backend calls, callback delivery and record changes.
Rejected consuming release returns every supplied handle field unchanged.
These are planned cases, not executed acceptance counts.

Existing prepared/reserved/active-key replay, cross-Context handles and
byte-identical generated-source substitution tests already execute. Reuse them.
Separate source-roster work still needs every descriptor coordinate and the
immutable-source positive path after one-shot control transfer. Lane,
queue/publication occurrence and allocation incarnations require actual
DATA-ADOPT/ISSUE records; descriptive fixtures cannot close native identity.
CO-2A closes only Context submission-identity coverage, not full CO-2.

The patch-ready fixture uses genuine neighboring submissions plus private
test-only handle snapshots. Snapshot submission/event records, existing ID
allocators, backend counters and cleanup logs; add only the missing test-backend
cancel entry counter. Use a future drain deadline for identity rejection:
drain checks deadline before identity, whereas wait validates live binding first.
After actual stream destruction, pure query, cached cancel/drain and live
poll/wait/event paths intentionally differ. Preserve real graph-reservation and
terminal-Context precedence instead of moving every ingress to live validation.
For graph precedence, prepare and reserve a real graph before submitting its
action. Ordinary retained submissions prevent graph reservation; do not
manufacture this state by assigning a reservation around existing submissions.
Use compiled negatives that remove backend/stream/device comparisons, move
cached success ahead of validation or revive a stale logical handle by backend
ID lookup. Removing only the explicit Context-generation comparison is not a
decisive negative: the branded map key independently rejects that substitution.

Use genuinely produced cache states. `record_event` followed by `wait_event`
completes the retained record without populating the supplied handle's cache;
release the event before successful-release controls. Ordinary `wait` completes
both. Snapshot that completed handle and genuinely release the original to
obtain a stale handle-only cache with no retained record. Do not roll a completed
record back to Pending to manufacture a live handle-only cache. The existing
one-shot mock `handle_override` can then reuse the old backend ID on a new
logical submission; prove the replacement remains untouched and subsequently
completes normally. Shared fixture edits are limited to module wiring and the
missing cancel-entry counter.

### CO-2B And CO-3 Follow-On Packets

CO-2B owns a new `authorized_execution/tests/generated_identity.rs`, with Primary
wiring `authorized_execution.rs`. Three tests cover every roster coordinate in
both match directions and source matching, immutable validation after one-shot
control transfer, and post-transfer artifact/authority/currentness rejection.
Mutate count, readback bytes, fixup count, dispatch hash, each occupied slot's
ordinal/extent/access, absent slots and unexpected trailing slots independently.
Preserve original source buffers and transferred control. Reuse existing
pre-transfer/substitution tests; no production validator change is indicated.
These are descriptive checks, not native publication/allocation receipts.

CO-3A owns a gap-only matrix under
`async_engine/tests/owned_tests/preparation_tests/completion_tests.rs`. Reuse the
R80/R83 Harness, existing private `Reply<()>` cells, registry and owner probes:
two-owner cell isolation, Stop notification before conclusive disposal,
retirement-prefix conservation across A success/B error or panic/C retained,
and private completion-cell latest-waker/panicking-wake containment. Repeated
progress must not retry B or dispose C. Track consumer credit separately from
payload custody. A test-only borrowed completion-future accessor can register
identifiable wakers before ticket consumption; do not extract/clone the consumer
or reserve another reply. Primary owns that narrow visibility change.

The generated driver has no success/readback-completion adapter yet. Do not add
a dummy adapter merely to claim composition. Freeze its data-only ordering
contract now; success, partial readback and late-currentness tests require real
ISSUE/COMPLETE callbacks in CO-4. Reuse existing preparation, Stop, observer-loss,
replay and outer-future waker tests rather than reproducing them.

## Resources Queue

1. **VER-1A.1: reviewed contract/inventory present.** Freeze exact
   Context/allocation/device/writer identities, finite capacity, nonwrapping
   `Available/Pending/Unknown` versions, whole-destination rosters and retirement
   rules. Graph-local history is not persistent Context authority.
2. **VER-1A.2 -> .3: model, then journal.** Implement the executable model with
   property proofs, then its production consumer with preallocated Context
   metadata and move-only tickets. The standalone .2 remains model-only.
   Whole-roster admission/settlement is atomic. Wrong writers, omitted members,
   replay, overflow and first/middle/last failures cannot partially mutate the
   journal; dropped tickets cannot restore availability.
3. **VER-1A.4/.5 -> VER-1B: hooks and acceptance.** Integrate host-write and
   ordinary/graph-copy hooks, preserving logical destinations before backend
   translation. Invalidate before effects and settle before callbacks. Extend
   one mutation family at a time through peer copies, every launch family,
   generated issue, currentness, cancellation, cleanup and retirement. Complete
   ordered overlapping writers and Unknown recovery before whole-surface reuse;
   an opt-in one-writer staging profile cannot restrict ordinary work silently.
4. **VER-2: input leases.** Enable only after complete mutation coverage. Reject
   outside-graph writes, overlaps, stale/foreign/replayed identities and unknown
   publication. Caller-declared kernel access is insufficient authority.
5. **Memory closure.** Split MEM-DOM-1 into its independently ready domain
   contract and subsequent shared-accounting/Context integration. Test repeated
   Context creation, parent exhaustion, foreign children and simultaneous
   quarantine with reserved terminal headroom. N1B -> MEM-3 then cover remaining
   kernarg/executable backing, controls and occupied slots. MEM-4A's isolated
   host-image ceiling is independently ready; MEM-4B native residency follows
   backing/control integration. MEM-5 must include commands, captures,
   replies/results, registries, arenas, journals and quarantine, without
   double-charging aliases. Concurrent-bootstrap pre-effect reservation is a
   separate missing contract, not permission to relax the creation gate.

Preserve logical destination rosters before `prepare_context_launch_v1`,
`prepare_context_copy_v1` and `peer_copy` translate identities. Journal
settlement must precede callbacks in `transition_submission_status`. Existing
single-account credits are not aggregate domains. Complete version hooks block
cross-run input leases, not the first non-reusing generated ISSUE; freeze that
submission's mutation hook now without inventing a second identity allocator.

### VER-1A.1 Contract Boundary

The reviewed [contract and inventory](runtime-context-version-journal-v1.md)
is the input to VER-1A.2, not another drafting assignment. It does not implement
the executable model, Context journal, hooks or cross-run leases.

Resources' next model handoff is an allocation-free whole-roster transition
planner consumed by the eventual journal. Validate the complete canonical input
and output capacity before writing scratch; first/middle/last failures leave it
unchanged. An exact writer cannot be replayed after its record is freed. A
reviewed issuance watermark observes existing Context IDs at registration, not
at Begin. Retained reservations preserve valid out-of-order admission. It is not
a second allocator. Proofs must cover the shared planner definitions and
separately identify roster extraction and actual Context commit obligations.

Before implementation, Resources and Primary must encode the reviewed capacity,
canonical roster, writer replay and planner/commit boundaries below.
Counts come from retained private journal state, not caller witnesses. A checked
transition-plan proof is not a proof of Context's eventual atomic commit.
The .3 journal consumes that model with preallocated entries, writer records and
scratch, retaining complete membership independently of dropped tickets. Its
acceptance includes late-member rejection, replay, exhaustion and unwind before
.4/.5 adds real Context hooks. NoEffect receipt producers and bounded ordered
writer/Unknown recovery remain explicit VER-1B work.

### Reviewed Writer Issuance Handoff

| Packet | Deliverable | Exit gate |
| --- | --- | --- |
| VER-1A.2a | Existing-ID projections, bounded writer slots, issuance watermark and Reserved lifecycle in new `runtime-model/src/context_version_journal.rs` | Out-of-order Reserved eligibility, stale slot/replay rejection and atomic capacity failure; complete Begin follows in .2b |
| VER-1A.2b | Direct allocation references, writer-owned member chains, preallocated plans and full invariant auditor | Exact membership/free-slot partition/acyclicity; no partial Begin on first/middle/last failure |
| VER-1A.2c | Success, exact NoEffect and Unknown settlement plus counted-access tests | Whole retained roster, burned attempt epochs, sticky Unknown and fixed-k work independent of unrelated A/W |
| VER-1A.2d | New `verus/context_version_journal_v1.rs` and property-specific negative sources | Authenticated positive verification and expected-negative rejection; exact source/runner/transcript pins and observed obligation counts |
| VER-1A.3 | `runtime/src/context/versions.rs` consumes shared plans through actual Context paths | Existing identities, preserved logical rosters, admission before effects and allocation-free exclusive commit |

R103 freezes the [issuance activation/capacity policy](runtime-context-version-journal-v1.md#r103-planning-freeze):
fresh-Context construction-only opt-in before any `next_id()`, a fixed zero
initial watermark, and explicit immutable positive A/W capacities bounded by
the existing 1,048,576 allocation/submission maxima. Packet .2a allocates writer
slots only; .2b adds membership/scratch. Registration observes existing IDs,
while later use of an already Reserved writer ignores newer registrations.
Neither this policy nor .2a changes ordinary Context behavior or implements
Begin/settlement. The production journal still starts at .3.

Resources owns isolated model/proof/test sources. Primary owns model exports,
`verify-verus.sh` registrations, sealed rosters, source/runner/transcript pins,
`check-negative-quality.py` exact negative roster/fingerprints and existing
Context integration. Reuse the executable-projection pattern without treating
R70/R73 proofs or the existing proof inventory as journal acceptance. Preserve
historical proof files. Production correspondence starts with the actual .3
consumer; an unused runtime hook is not integration.

The sole existing `Context::next_id` allocator remains authoritative. Current
ordinary launch/copy/peer paths mint their submission immediately before backend
entry; prepared launches/copies, graph reservations and R80 reserved tickets do
not pre-mint submission IDs. Preflight journal writer capacity, mint with that
allocator and register allocation-free under exclusive Context ownership.

Registration requires an exact Context/local-ID/kind key above the prior
registered-issuance watermark. A bounded private writer slot is `Vacant`,
`Reserved`, `Pending` or `Unknown`. Begin consumes the exact Reserved record,
not an ID-order predicate: registering 41 and 44, settling 44, then beginning 41
is valid. Settled or explicitly pre-effect-aborted IDs cannot register again,
even after slot reuse. A dropped ticket does not free its reservation. Unknown
retains its complete membership until separately specified recovery/disposal.
Allocation attempt epochs follow Begin order, not numerical writer-ID order.

The .2a implementation is limited to registration, exact Reserved lookup and
explicit pre-effect Reserved abort. It must not present a header-only transition
as complete destination Begin. Freeze journal activation and initial-watermark
policy: no arbitrary import of earlier unregistered IDs and no watermark reset
after issuance. Match the existing checked-add-before-return allocators: Context
generation/local ID `u64::MAX - 1` is issuable; `u64::MAX` is not. The model is
intended for eventual .3 consumption; no production journal consumer exists yet.

The model stays `no_std` with `alloc`, no unsafe code and no I/O. Use inert
Context-generation/local-ID/writer-kind projections, not imports of runtime
types: runtime already depends on runtime-model. Authentic private extraction,
move-only production tickets and construction-only runtime activation belong
to .3. Primary wires `runtime-model/src/lib.rs`; .2a must not add a runtime
activation API or `context/versions.rs`.

For .2a, test independent A/W boundaries; Context/local zero and maximum values;
registration 41 then 44 with both still retrievable; unregistered 42 rejection;
foreign Context/kind/slot and stale-reference rejection; abort followed by slot
reuse; and dropped-reference capacity retention. Rejection preserves a complete
state snapshot. A full test-only partition/unique-key auditor and fixed-work
counters check invariants without adding arena scans to operations. Compiled
negatives should reject a lookup watermark check, abort watermark rollback,
omitted exact-key comparison and watermark mutation before capacity rejection.
These are planned executable checks, not completed tests or authenticated proof.

Use independent configured allocation/writer bounds A/W within Context's
1,048,576-entry limit. W includes Reserved/Pending/Unknown headers, including
attempts without a returned submission. For the staged single-pending-writer
profile, retain membership once per allocation, exact cardinality in the writer
header and A-sized preallocated planning scratch: O(A + W) journal storage.
Prepared logical rosters are separately bounded metadata. Canonicalize by full
logical allocation identity, preserving device/extents and removing aliases;
settlement checks journal-owned complete membership and inverse cardinality.
Ordered writers later need an explicit total-membership bound and predecessor
links; this profile cannot silently restrict ordinary ordered work.

Separate register/begin/settle plans validate all inputs and arithmetic before
writing scratch. The eventual Context journal applies the exact plan under
exclusive ownership without intervening callbacks, allocation or backend entry,
and roots Pending before effects. That Rust commit is a separate correspondence
obligation, not a consequence of pure plan arithmetic. Required properties cover
issued-ID replay, out-of-order Reserved admission, reused-slot rejection, atomic
roster failure, complete settlement, NoEffect lineage versus attempt epoch,
Unknown/ticket retention and capacity/epoch exhaustion. This is a reviewed design
handoff, not an implemented or proved journal.

Freeze a separate journal/profile capacity and reject allocation admission before
creating an untrackable allocation. Context's 1,048,576-entry bounds and the
credit engine's 65,536-record bound are different quantities, not a journal
capacity choice. Bounded metadata alone does not establish aggregate charging;
bootstrap/account-arena charging remains MEM-DOM/MEM-5 work.

### Reviewed Journal Cost And Membership Handoff

Reuse the existing Context allocation index, with private exact journal-slot
references in allocation/submission records. Do not add another identity map or
allocator. Preallocated allocation entries retain extent, epoch, lineage and a
pending-member reference. Writer headers retain their exact key, phase and
member-chain head/count. Each membership node binds the exact writer/allocation,
attempt epoch, prior lineage and next node. Slot positions are not identities.

For the staged single-pending-writer profile, membership capacity M = A gives
O(A + W) retained journal storage. Reserved headers consume W without membership
nodes until Begin; prepared logical rosters remain separately bounded. A writer
owns its immutable complete membership chain, so settlement takes its exact
reference, not a caller-selected destination subset.

| Common operation | Required cost boundary |
| --- | --- |
| Writer registration | O(1) vacant-slot selection and issuance checks |
| Preparation with n input views | O(n log n) canonicalization using separately reserved input-roster storage |
| Begin with k distinct destinations | k existing-map lookups and O(k) preflight/commit using A-sized journal scratch |
| Whole-writer settlement | O(k) validation of the retained chain followed by O(k) commit |
| Allocation retirement | Existing-map lookup and exact journal-slot check |
| Context terminalization | Set global reuse denial without an immediate allocation-wide rewrite |

HashMap lookup is expected O(1), not a deterministic worst-case bound. Separate
initialization, shutdown and auditing costs must not become an implicit O(A/W)
scan on every Begin or settlement. Begin preflights available membership slots
and every destination before changing free lists or state. Settlement validates
all nodes, identities, backlinks, count and terminating link before any commit.
Preparation validates all input views and sorts/deduplicates an owned logical
mutation-key projection in place, without reordering or discarding original ABI
bindings. This projection is proposed, not retained by the current preparation
code. The ordinary-launch profile bounds bindings at 128; generated fixed
dispatch bounds its roster at 16. Other profiles must specify their input bound.
Canonicalization is allocation-free only after that workspace is reserved, not
allocation-free preparation. The current ordinary binding-count check follows
caller vector construction and cannot bound arbitrary caller allocation peaks.

Begin receives the retained canonical k <= A roster. n > A is admissible when
k <= A; A-sized journal scratch is not storage for every aliased input view.
Prepared-roster capacity, including unused capacity after deduplication, remains
separate from the O(A + W) journal bound. Repeated Begin does not repeat alias
canonicalization but still validates/processes all k distinct destinations.

The optimized traversal requires a proved initialization/preservation invariant:
live nodes belong to exactly one writer and allocation, each pending allocation
points back to its node, the writer chain is exactly its admitted roster, chains
are acyclic, counts are exact, and free/occupied slots partition each arena.
Header cardinality alone cannot exclude orphaned members elsewhere. Keep a full
invariant auditor for CPU/property tests, not every production operation; touched
validation does not detect arbitrary corruption of unrelated state.

Plans remain ephemeral under exclusive Context ownership. Complete fallible
preflight before a non-reentrant, allocation-free commit; detached stale plans
are not admissible. Prove complete installation/settlement, untouched-state
framing, nonwrapping epochs, slot-reuse rejection and bounded traversal. Tests
hold k fixed while increasing A/W and count accesses, reject truncated/cyclic/
foreign/stale chains without partial settlement, compare every admitted member,
exercise out-of-order reservations, and inject first/middle/last failures.

Full ordered-writer compatibility needs an explicit total membership bound M
and allocation-side predecessor/successor links. Later writers must survive
earlier settlement; NoEffect follows actual predecessor lineage. State dependent
chain-progress costs separately. This remains a design/proof work order, not an
implemented journal or performance measurement.

The contract must name the following actual mutation surfaces under
`crates/fe2o3-runtime/src/` before journal implementation:

| Family | Inventory boundary |
| --- | --- |
| Allocation lifecycle | `context.rs::allocate/release_allocation/cleanup`; `context/generated_shells.rs::install_generated_shells_v1/retire_generated_shells_v1` |
| Host writes | `write_allocation`: range validation before journal admission, journal admission before backend effects |
| Ordinary/graph copies | Shared `prepare_context_copy_v1` and `submit_prepared_copy_v1`; preserve original logical destination/device |
| Peer copies | `peer_copy`: preserve logical identity before backend translation |
| Kernel writers | Shared `prepare_context_launch_v1` and `submit_prepared_launch_v1`, including snapshot/graph/atomic/collective families and future generated ISSUE |
| Settlement/retirement | `transition_submission_status`, `completion_backend_result`, `mark_stream_quiescent`, cancellation/drain, protocol sealing, terminal/panic/currentness and cleanup |

Use existing Context IDs for synchronous writers and exact runtime submission
IDs for asynchronous writers. The Context retains complete pending rosters;
dropped tickets cannot restore availability. Available describes mutation
lineage, not initialized or correct contents. Complete VER-1B coverage remains a
prerequisite for cross-run leases, not for the first non-reusing generated ISSUE.

Separate a monotonic attempt epoch from content lineage. Exact successful writer
settlement commits the new lineage. A separately named `NoEffect` settlement
requires exact, attempt-bound definite-no-write evidence and restores the prior
lineage without rolling back the attempt epoch or Context identity. Rejected
poll/query observations for an already issued writer are not `NoEffect`
evidence. Generic cancellation status, quiescence without result and unknown
publication alone cannot restore reusable authority. Exact attempt-bound
no-write cancellation receipts remain a required evidence-producer follow-on.
Test wrong/omitted roster members, epoch wrap,
NoEffect epoch rollback, rejected polling as NoEffect and lineage zero being
misinterpreted as initialized contents.

### Independent Resource Packets

| Packet | Ready boundary and dependency | Exit gate |
| --- | --- | --- |
| MEM-DOM-1A -> 1B | Domain/terminal-headroom contract now; shared accounting and Context/session integration afterward | One exact root/child hierarchy, charged bootstrap/arenas, parent exhaustion, repeated Contexts and simultaneous quarantine without premature refunds. Concurrent-bootstrap reservation remains a separate contract. |
| MEM-N1B-1 -> N1B-2 -> MEM-3 | Native backing, then AQL/USERPTR/control/occupied-slot integration with Native | Whole compound admission before effects; exact retained allocation/map/error/panic prefixes. Reuse R70 admission and existing ledgers. |
| MEM-4A -> 4B | Isolated host-image ceiling now; native residency after backing/control integration | Repeated loads, live leases, rejected eviction and ambiguous unload. Executable GTT is not VRAM. |
| PRF + MEM-5 | Incremental adapter correspondence and total resource inventory | Include commands, captures, results, journals, arenas, quarantined roots and callback/panic-payload exclusions; authenticate named properties separately from tests and hardware. |
| CO-PROOF | After the frozen CO-1 interface; isolated model/proofs, Primary-owned runtime projection and registry wiring | One production-consumed normalized policy, Pending/rejection retention and terminal/panic precedence. Reuse R61/R62/R64; prove the named shared definitions and separately identify Rust enum/ownership/native correspondence gaps. |

R65 graph-local history, R67/R70 single-account credits, optional N1/N2 backing
budgets, both cache policies and R73/R80/R85 charged storage already exist.
Extend and qualify them; do not rebuild them as a second accounting system.
They do not establish persistent Context version authority, aggregate ceilings
or newer adapter refinement.

## Integration And Qualification

```text
R97 .5B-1 -> R99 .5B-2 -> .5B-3 -> 2C -> DATA-ADOPT --+
CO-1 -> CO-2A/2B + preissue CO-3 -----------------+-> ISSUE -> COMPLETE -> API -> GRAPH/DRAIN
                         native identity joins DATA-ADOPT/ISSUE -> CO-4
VER-1A -> complete VER-1B -> VER-2 --------------------------------------------> cross-run input reuse
```

Independently ready work can fill a free worker slot: SCALE-1A correctness
fixtures, a native N1/N2/cache budget harness, R76/R78/R82 qualification cells,
aggregate-domain design, and incremental adapter correspondence. Existing
overlap/drain runners and ordinary cache policies need qualification, not
duplicate implementation. Backing/control/slot admission precedes larger native
depth. Host queue depth alone does not establish native in-flight depth.

Each accepted source packet needs focused tests, applicable full-source gates,
compiled negative mutations, exact source identities and independent review.
Authenticated formal refinement, live Linux/KFD behavior and performance are
separate acceptance columns. Protected generated execution also requires exact
compiler/machine evidence; CPU fixtures cannot manufacture that prerequisite.

Record each packet in four separate columns: **CPU/source acceptance, formal
correspondence, live Linux/KFD qualification, matched performance**. Require
exact source/toolchain identities and retained failure attempts. A proof
inventory audit is not a solver run, and a successful fixture is not a proved
production adapter. Review correspondence incrementally, starting with the
existing R83 lifecycle and R82 cleanup boundaries, instead of deferring proofs
until after API integration.

Primary schedules MI300X work serially, checks shared-machine availability,
uses task-owned processes/staging and cleans up only those owned resources.
Disruptive fault tests require an isolated window. Matched HIP/HSA/KFD producers
must validate complete outputs before measuring latency, throughput, bandwidth,
CPU use, memory bounds and tail latency. Physical overlap needs device timelines.
No full parity or orders-of-magnitude improvement is accepted by this plan.

## Later Milestones

These remain outside A1/A2 closure and reuse the same three worker slots.

| Milestone | Lead and exit scope |
| --- | --- |
| A3: local multi-GPU | Native with Admission/Resources: topology, sharding/replicas, peer/staged transfer and group drain; all admitted GPUs, exact versions and partial-failure isolation |
| A4: distributed control | Admission/Primary after distributed semantics contracts: authenticated sessions, membership epochs, receipts and drain across two hosts without duplicate publication |
| A5: data/collectives | Native transport plus Resources versions/credits: bounded reference transfers and separately qualified broadcast, reduce-scatter, all-gather and all-reduce |
| A6: failure qualification | Admission/Primary: participant, network, device, transfer and collective faults; no unsafe replay, early disposal or false complete outputs |
| A7: production performance | Native/Primary: precommitted single-device, multi-GPU and two-host gates; matched baselines, recovery costs and direct-KFD dependency/symbol audits |

Broad device-language support and production atomics/collectives also depend on
their compiler semantic-to-machine contracts. Closing A1/A2 alone does not close
those ownership boundaries or issue #182.

The renewed issue-list check still finds open runtime-labeled work outside
this A1/A2 swarm. Primary coordinates these dependencies rather than assigning
their implementation to an idle runtime lane:

| Handoff | Owning work and acceptance boundary |
| --- | --- |
| Worker/capsule authority | #209 and broker/handoff #130/#131/#132; concrete reviewed Worker V3 refinement backend and owned proof artifacts, not caller-provided digests or qualification gates |
| Target admission | [#274](https://github.com/harsh-nod/fe2o3/issues/274): independently reviewed gfx950/MI350X driver/firmware profile, queue resources, memory ownership, checked token and service-host join. gfx942 authority or compile-only evidence cannot qualify gfx950 execution. |
| Mixed SIMT/tile consumer | [#275](https://github.com/harsh-nod/fe2o3/issues/275), with #134/#271/#272: generated launch/resource contracts and kernel-family qualification through the existing pipeline. Reconcile exact compiler-owner handoffs; do not duplicate their active work or wait for all distributed milestones before developing the initial admitted path. |
| Protected kernels | #89/#88/#98/#104/#105/#123 plus the compiler-owned device-language G2/G4 milestones; exact ABI/effect contracts, independent output/layout oracles and architecture-specific execution |
| Release and diagnostics | Offline installation #252, disposable-machine deployment #253 and debugger descendant containment #269; keep deployment and debugger ownership separate from queue construction |
| Broad qualification | G8 and A6/A7: differential testing, supported-target evidence, isolated fault campaigns and matched complete-output performance, not CPU suite wall time |

These are coordination queues, not additional running agents. Worker V3's
concrete refinement backend and owned verification artifacts are not supplied
by the present host admission module. The Native, Admission and Resources
workers own the bounded A1/A2 packets above; Primary owns cross-team integration.

Protected scalar GEMM still depends on the separately open
[#214 machine/IEEE refinement](https://github.com/harsh-nod/fe2o3/issues/214),
checked through the GitHub API at this refresh (`updatedAt`
`2026-09-01T18:56:15Z`). Keep Worker/capsule, deployment and debugger release
work with their owning teams; this three-lane A1/A2 assignment does not close
all issues carrying the runtime label.
