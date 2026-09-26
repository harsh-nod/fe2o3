# Current Runtime Swarm Work Orders

## Milestone Snapshot

As of 2026-09-25, [#182](https://github.com/harsh-nod/fe2o3/issues/182)
remains open. These are exit-criteria statuses, not API implementation counts.
No full HIP/HSA behavioral or performance parity is accepted.

| Milestone | Status | Remaining exit gates |
| --- | --- | --- |
| A0: semantics and ownership | Partial foundations | Complete distributed ownership, protocol, failure and trusted-boundary contracts |
| A1: single-device async | Active, incomplete | Protected generated execution, native high-depth/out-of-order qualification, aggregate accounting and production refinement |
| A2: dependencies and overlap | Active, incomplete | Repeated generated compute/copy graphs, Context completion-reconciliation proofs, physical overlap and bounded residency |
| A3: local multi-GPU | Partial foundations | XGMI copy witnesses exist; unified compute, all-admitted-GPU sharding, group drain and partial-failure qualification remain |
| A4: distributed control | Open | Authenticated two-host sessions, epochs, publication receipts and interruption-safe terminal classification |
| A5: distributed data and collectives | Open | Two-host versioned transfers and qualified broadcast, reduce-scatter, all-gather and all-reduce |
| A6: failure qualification | Partial coverage | Scripted failures and ordinary native cleanup exist; isolated device/network/participant/collective fault campaigns remain |
| A7: performance and release | Not qualified | Matched performance thresholds, scaling, device timelines, resource/tail metrics and production closure audits |

Broader accepted lane checkpoints remain Native R125, Admission R118B
C1/C2/C3 and Resources R116/V3. Protected Worker/compiler refinement,
device-language and atomic/collective authority, target expansion, deployment
and debugger handoffs remain separate open work under [Later Milestones](#later-milestones).

## Latest Qualification

Latest scaled-depth development (2026-09-25): a separate scale-feature-only
ignored native canary now inspects all 2,048 original retained receipts across
two lanes, exact runtime and profiler joins, distinct runtime-custody and native
epoch-table saturation, final buffer contents and complete table disposal.
Unexpected accepted tokens remain owned through explicit cleanup before failing
qualification. Nine synthetic checker tests cover receipt, ownership, accounting,
ordering and publication-metadata mutations. See the
[development packet](evidence/dev-scale-depth-2026-09-25/README.md) for validation
results and limits. The final full runtime run reports 1,444 passed, the same
three telemetry socket `EPERM` failures and 28 ignored. Strict Clippy, the exact
scale-feature checker run, default-feature checking, formatting and all 46
doctests pass. This cell has not executed on MI300X; retained is not
unfinished or physically concurrent. Repeated reuse/rebind, async-owner high
depth, aggregate admission, scaled refinement, signed native replay and matched
performance remain open. No accepted checkpoint or A1/A2 status changes.

Latest runtime capacity integration (2026-09-25): the separate
`scale-qualification` exact-vecadd constructor preallocates both runtime tables
before KFD startup and shares its account/profile with both native construction
routes. Scaled per-allocation custody preallocates 1,024 owners, refunds partial
preflight failures and retains the full debit through final-owner retirement.
Generated, DeviceLocal and persistent scaled routes reject early. Existing
constructors, dependency limits and XGMI limits are unchanged. See the
[runtime development packet](evidence/dev-scale-runtime-2026-09-25/README.md).
All ten new CPU tests pass. The full runtime run reports 1,435 passed, the same
three socket-inspection `EPERM` failures and 27 ignored; strict Clippy,
default-feature checking, formatting and 46 runtime doctests pass. SSH remains
unavailable at hostname resolution, so no native result is added.
Aggregate memory admission, scaled refinement, native retained-depth measurement
and matched performance remain open; accepted checkpoints and A1/A2 are unchanged.

Earlier capacity foundation (2026-09-25): the
[accounted capacity foundation](runtime-scale-capacity-v1.md) adds fixed,
fallibly allocated metadata tables and feature-gated native 1024-slot profile
propagation through construction and rebinding. Default native and public
runtime constructors remain at 64. Fourteen scaled-profile tests, 147 native
dispatch-binding tests, 51 rebind tests, 29 accounting tests and three runtime
capacity tests pass; these groups overlap. Strict Clippy, default-feature
checking, formatting and 76 doctests pass. The full runtime run reports 1,425
passed, the same three telemetry `InspectSocket(EPERM)` failures, and 27 ignores.
[Development logs and limits](evidence/dev-scale-cap-storage-2026-09-25/README.md)
do not establish full CPU qualification: the whole-KFD attempt hit its 900-second
limit, and a separately scoped KFD subset had one socket-admission failure.
At that checkpoint the runtime opt-in and 256-owner custody bottleneck were
still open; the integration above addresses those two items. Complete scaled
joins, aggregate admission, new formal refinement, native depth and matched
performance remain open. A1/A2 and the accepted lane checkpoints are unchanged.

Latest observer/backpressure development (2026-09-25): signed `97b8d41fe` adds
two CPU regressions for Pending timeout recovery/new-waker registration and
frozen-launch QueueFull credit refunds, admission retry deadlines, and three
compiled native observer/backpressure canaries. The
[retained observer packet](evidence/dev-mixed-observers-2026-09-25/README.md)
does **not** pass full CPU qualification: GNU and musl each report 1,422 passed,
three existing telemetry admission failures and 27 hardware-only ignores.
An independent socket probe reproduces `EPERM` at `SO_DOMAIN` and the remaining
socket inspections; production validation stays fail-closed. The initial musl
runner linker-flag failure is also retained, followed by a corrected build.
Both new CPU regressions pass on both targets. Native timeout/Drop actions are
bracketed inside one owner callback by the same Pending receipt to reject a
completed-observer race. All 46 doctests, default-feature checking, strict Clippy
and formatting pass. These native cells, full CPU qualification, formal
refinement, physical overlap, performance and A1/A2 closure remain open. Default
64-slot limits and ReadWrite early-publication restrictions are unchanged.
Retained replay binds 5,981 signed inputs and all 16 qualification commands;
the seven classifier groups pass. Exact-owned local cleanup removes
1,946,607,616 allocated bytes with independent path absence and post-cleanup
replay. No new remote resources were created.

Latest mixed-duration development (2026-09-25): the
[short/long CPU packet](evidence/dev-mixed-duration-2026-09-25/README.md)
adds separately admitted fixed-work objects, independent full-byte oracles and
two compiled native correctness/async-owner canaries. Signed `ca1edd1423` passes
1,423 runtime tests per GNU/musl target, all 46 doctests, strict Clippy, formatting
and byte-identical object rebuilds; 24 hardware-only tests remain ignored per
target. All 227 original scratch artifacts are retained and replayed.
The [native attempt](evidence/dev-mixed-duration-native-2026-09-25/README.md)
was interrupted during upload: no native test launched. Its cleanup command
reports exact staging removal, but independent remote absence is unverified.
Original build mounts became unavailable after the execution-context change;
no local build-tree removal is claimed. SCALE-1 native correctness, SCALE-2
ordering/depth, physical overlap and matched performance remain open. The
two-operation ReadWrite profile is not a high-depth pipeline profile. A1/A2 and
the broader accepted lane checkpoints are unchanged.

Latest leaf-outcome proof development (2026-09-25): the
[exact leaf outcome packet](evidence/dev-completion-leaf-outcomes-2026-09-25/README.md)
passes all fourteen phases from signed `7185cc6fb`. Opening, relocated and closing
whole-root proofs each verify 53 obligations at unchanged limits. Constructed
finite-projection leaves return exact Success/Quiescence in two iterations or
the exact injected settlement error in one. Four logical mutants, twelve
classifier groups, eight source-checker groups and formatting pass; 5966 source
inputs bind directly to signed blobs. All 372 artifacts, including failed and
interrupted attempts, are retained before exact-owned cleanup of 10309632
allocated bytes. Production planner code is unchanged; CPU and hardware suites
were not rerun for this proof-only slice. Real construction/journal refinement,
native concurrency and matched performance remain open; A1/A2 do not close.

Latest planner proof development (2026-09-25): the
[shared planner safety packet](evidence/dev-completion-reconciliation-proof-2026-09-25/README.md)
passes all fourteen signed-source phases from `7935918fc`. Both standalone whole
root positives verify 49 obligations at unchanged limits, including exact
selected-dependency ancestry, gated success/quiescence and bounded 513-step
yield. GNU/musl each pass 1,416 runtime tests with 22 hardware-only ignores; all
46 doctests, eight source-checker groups, strict Clippy and formatting pass.
Both source brackets match 5,963 inputs. All 69 fresh-run artifacts are retained;
outside-sandbox exact-owned cleanup removes 1,940,582,400 allocated bytes with
independent absence. Earlier transient development logs are unavailable and are
not claimed as retained or accepted. This is finite-graph development evidence,
not final outcome-witness/mutation/replay qualification or real Context/journal
refinement. Evolving journal observations, generated/native/resource gates and
matched performance remain open. A1/A2 and accepted checkpoints are unchanged.

Latest planner development (2026-09-25): the
[shared completion reconciliation packet](evidence/dev-completion-reconciliation-body-2026-09-25/README.md)
extracts the complete bounded production planner unchanged and adds three
real-Context regressions. A 256-node chain resumes after exact 513/513/219-step
passes; a later dependency failure preserves its successful cursor prefix; a
303-operation graph distinguishes total graph size from the 256-entry roster
and stack bounds. All nine phases pass from signed `53aba0d65`: GNU/musl each
pass 1,416 runtime tests with 22 hardware-only ignores, all 46 doctests, strict
Clippy, default-feature checks, formatting and six checker tests pass. Parsed
old/new bodies match exactly, and both source brackets match 5,955 inputs.
All 52 scratch artifacts are retained before exact-owned cleanup and independent
absence, removing 1,940,402,176 allocated bytes. This is extraction and CPU
regression evidence, not a new Verus theorem, full Context refinement, native
qualification or performance result. The complete finite-graph proof remains
open; A1/A2 and accepted lane checkpoints remain unchanged.

Latest host-data development (2026-09-25): the
[completed-result input packet](evidence/dev-completed-result-input-2026-09-25/README.md)
adds a consuming read-only input constructor that preserves the original typed
allocation and credit until the next input has been admitted and encoded. It
avoids one caller-side typed clone; encoding remains linear, and successor
credit is reserved independently rather than transferred or refunded early.
All fourteen phases pass from signed `49aa95f33`: GNU/musl each pass 175 host
tests, including nine new ownership/accounting/preparation tests; GNU runtime
passes 1,413 with 22 hardware-only ignores; 26 host doctests, accounting/macros,
generated downstream compilation, three compile negatives, six runner checks,
strict Clippy and formatting pass. Both source brackets match 5,953 inputs.
The first run's inherited Clippy failure is retained; its correction only moves
an unchanged test module. All 123 scratch artifacts are retained before
exact-owned cleanup and independent absence, reclaiming 6,813,011,968 allocated
bytes. This is completed host-data chaining, not device-resident forwarding,
in-flight dependency transfer, protected Worker execution, formal adapter
refinement or measured performance. A1/A2 and accepted lane checkpoints remain
unchanged.

Latest selected-reader development (2026-09-25): the
[selected completion packet](evidence/dev-selected-reader-completion-2026-09-25/README.md)
removes one redundant stable-roster validation and shares selected-root journal
effects, retirement and marker clearing between production and proof. All eleven
CPU phases pass from signed `aef1c70d9`: GNU/musl each pass 1,413 runtime tests
with 22 hardware-only ignores; all 46 doctests, strict Clippy and formatting pass.
Both whole-root proof brackets verify 1,303 obligations with no errors or
diagnostics, all seven logical mutations reject, and source/tool brackets match.
All 4,518 scratch artifacts, including rejected development attempts and the
manual musl timing failures, are retained before exact-owned cleanup and
independent absence, removing 2,016,534,528 allocated bytes. This is bounded
development evidence, not a relocated replay audit or full Context refinement:
HashMap/prevalidation binding, allocator-capacity correspondence, quarantine,
later completion effects, generated execution, native/resource gates and matched
performance remain open. A1/A2 and accepted lane checkpoints are unchanged.

Latest Context CPU validation (2026-09-25): the
[completion failure-prefix packet](evidence/dev-context-completion-faults-2026-09-25/README.md)
adds five tests covering 40 scenarios at the real Context adapters: errors before
journal effects, panics before/after committed effects, early roster validation
and late writer identity rejection. Exact map/marker retention, journal counts
and lineage, dependency/callback retention, original backend diagnostics and
allocation-credit quarantine pass. Hooks are test-only; production behavior and
shared proof bodies are unchanged. The final nine-phase run from signed
`ab26e8e0` passes GNU/musl each with 1,412 runtime tests and 22 hardware-only
ignores, all 46 doctests, default-feature checks, strict Clippy, formatting and
four receipt-runner tests. Both retained runs bracket the same 5,947 source
inputs. Exact-owned cleanup reclaims 1,935,204,352 allocated bytes with independent
absence checks. This closes the missing bounded post-prevalidation Context fault
test coverage, not map/journal executable correspondence, producer-first proofs,
native/generated/resource gates or performance. A1/A2 and accepted checkpoints
remain unchanged.

Latest completion qualification (2026-09-25): the
[shared journal-effect packet](evidence/dev-completion-journal-effects-2026-09-25/README.md)
passes all 38 stages from signed `957c5ce64`. Both extended whole-root brackets
verify 1,282 obligations; both control brackets verify eight; all 30 logical
mutations reject; the frozen owner regression verifies 1,271. Exact source/tool
brackets, retained replay, eight checker groups, six packet groups and five
cleanup tests pass. All 1,227 scratch files are retained before exact-owned
removal and independent absence, reclaiming 24,326,144 allocated bytes.
The one inherited invariant-opacity directive changes no assertions, contracts
or limits. Earlier resource failures remain rejected. Production runtime code
and its previous 1,407-test GNU/musl results are unchanged; no new native or
performance claim is added. Context map/quarantine binding and producer-first
reconciliation remain open, as do generated execution, aggregate resources,
A1/A2 and all broader accepted checkpoints. The development notes below are
history, not remaining gates for this completed bounded campaign.

Latest proof-gate development (2026-09-25): the full completion journal-effect
root now verifies 1,282 obligations with zero errors and no diagnostics. One
local invariant-opacity directive in an inherited disposal witness resolves
the earlier resource-limit failure; assertions, contracts, solver limits and
production runtime code are unchanged. The new 477-input successor checker
preserves all 22 historical control defects and adds eight shared-body effect
mutations. All eight CPU calibration groups pass. The signed-source positive
brackets, logical negatives, frozen regression and retained replay remain
required before qualification. A1/A2 and accepted lane checkpoints are unchanged.

Latest development (2026-09-25): the
[completion journal-effect composition](runtime-completion-journal-effects-v1.md)
shares the input-release sequence, writer outcome declaration/dispatch and
completion prefix with production. The separate journal adapter and logical
executor pass eleven module obligations, including constructor-origin mixed-input
capacity failures and writer-only Success/NoEffect/Unknown witnesses. The
unchanged control harness passes eight obligations against the nested body.
GNU/musl each pass 1,407 runtime tests with 22 hardware-only ignores; the new
40-case Context regression covers all optional input/writer combinations across
success, failure, quiescence, terminal ambiguity and confirmed cancellation.
This is development evidence, not the successor authenticated campaign. The
initial missing-intermediate proof failure, shorter whole-root timeout, witness
assertion/resource failures and corrected test expectation remain recorded.
Error-prefix journal states are pre-quarantine, not final Context-return states.
Context maps/quarantine, reconciliation, generated/native/resource gates and
A1/A2 remain open; the accepted lane checkpoints are unchanged.

Latest control-flow qualification (2026-09-25): the
[production completion-settlement control body](runtime-completion-settlement-control-v1.md)
is shared by Rust and Verus without changing its prechecks or call semantics.
The signed-source 27-stage campaign passes eight-obligation positive brackets
and all 22 logical mutations. These prove ordered Result control flow and exact
argument/result forwarding, not Context method effects or callback behavior.
Two new CPU tests and the full 1,406-test GNU/musl runtime suites pass, with 22
native ignores each; 46 doctests, strict Clippy and formatting pass. Four checker
groups, original/relocated replay and source/tool continuity pass. All 113 scratch
files are retained before exact-owned cleanup and independent absence, reclaiming
1,994,178,560 path-accounted allocated bytes. Journal/Context composition, producer-first
reconciliation, generated/native/resource gates and A1/A2 remain open.

Latest proof qualification (2026-09-25): the
[constructor-origin mixed lifecycle packet](evidence/dev-owner-mixed-lifecycle-2026-09-24/README.md)
passes all sixteen stages from signed `33b4606f4`: both whole-root positives
verify 1,271 obligations with zero errors, all nine logical mutations reject,
and the frozen mixed/stable regressions verify 738/720. Retained replay, ten
checker groups, four publication-test groups and source/tool continuity pass.
All 1,068 scratch artifacts, including the rejected first campaign, are retained
before exact-owned removal and independent absence; cleanup removes 23,113,728
path-accounted allocated bytes. A separate test-only CPU regression covers
discarded producer results after physical consumer success: GNU/musl each pass
1,404 runtime tests with 22 hardware-only ignores; 46 doctests, strict Clippy and
formatting pass. This closes the bounded constructor-origin mixed-lifecycle
campaign, not Context reconciliation, global proof registration, protected
generated execution, aggregate resources, native overlap or performance.
A1/A2 and all broader accepted checkpoints remain unchanged. Next is production
completion-settlement correspondence and the remaining generated/native/resource
gates. The earlier development and rejected campaign records below are history.

Latest development (2026-09-24): [constructor-origin mixed input
lifecycle](runtime-context-mixed-lifecycle-v1.md) integrates one acquisition event
with both reader histories, rejection preservation, arena reuse, terminal
producer status, exact release-trace continuity and Unknown retention. The
whole-root development run verifies 1,271 obligations with zero errors; eight
checker groups and the pinned 190-file Verus closure pass. This is not accepted
source-bound campaign evidence: positive brackets, nine logical negatives,
frozen regressions and final replay remain required. Production runtime sources
are unchanged. A1/A2 and the accepted lane checkpoints remain unchanged. Next is
the signed-source campaign, then production completion-reconciliation refinement
and the remaining generated/native/resource gates.

Qualification update (2026-09-25): the first signed-source campaign from
`145c19485` is now rejected, not running:
the opening whole-root positive verifies 1,271 obligations with zero errors and
eight logical mutations reject as intended. The ninth (`missing-atomic-event`)
reports both a postcondition failure and solver resource exhaustion, so the
checker refuses it as a logical negative. The closing positive, frozen
regressions and final closure were not reached. The complete failed attempt is
retained in task-owned local scratch; the evidence collection draft remains
unaccepted and no cleanup or packet-publication success is claimed. This does
not establish a production runtime defect or advance A1/A2. The next step is a
bounded, discriminating atomic-event mutation before a fresh full campaign,
without raising solver limits or weakening the rejection policy.

The atomic-event control is now retargeted to the existing shared append helper,
omitting only the event while preserving both state snapshots. Its development
run reports clean logical postcondition failures, accepted by the unchanged
negative classifier; all eight checker groups pass, including a new combined
logical/resource diagnostic rejection. Independent review finds no change to
proved sources, contracts, limits or the expected 1,271-obligation count. This is
a control correction, not qualification: a fresh signed-source campaign is next.

Latest mixed-input correspondence (2026-09-24): the
[independent proof packet](evidence/dev-mixed-acquisition-proof-2026-09-24/README.md)
passes all fifteen signed-source campaign stages. Both whole-root positives
verify 738 obligations with zero errors, all nine logical mutations reject, and
the unchanged stable-reader regression verifies 720. The logical model derives
exact results, both output rosters and final represented ownership independently
of the shared production method. Synthetic witnesses cover late rejection,
mixed success and shared-capacity rejection; they are not constructor-origin
traces. Eleven checker groups, relocated replay, packet/collector tests and
source/tool continuity pass. All 78 scratch artifacts are retained before
exact-owned cleanup and independent absence. Scoped process diagnostics retain
inaccessible fields rather than asserting global absence. The initial cleanup-
transcript audit refusal is retained; corrected six-group replay and publication
receipt corruption tests pass. No new CPU-runtime,
native or performance result is added. Global proof registration, constructor-
origin mixed lifecycle integration, Context maps/binding coverage, writer/read
transaction composition, async carriage and completion reconciliation remain
open. A1/A2 remain incomplete; the accepted Native R125, Admission R118B
C1/C2/C3 and Resources R116/V3 checkpoints are unchanged. Next is one mixed operation
in the existing constructor-origin lifecycle, then production reconciliation
correspondence and the remaining native/generated/resource gates.

Latest mixed-input qualification (2026-09-24): the
[atomic acquisition CPU packet](evidence/dev-mixed-input-acquisition-cpu-2026-09-24/README.md)
passes all fifteen stages. GNU/musl each pass 1,403 runtime and 1,028 model tests,
with 22 hardware-only runtime and eighteen existing model ignores; all 73
doctests, formatting, strict Clippy and 3,965 unchanged source inputs pass.
The journal preflights both complete input classes before either commit; Context
retains both roots before acquisition and publishes neither marker until both
reference vectors are complete. Error and panic tests establish retained
custody without backend entry, not panic rollback. The shared-body whole-root
Verus development run passes 1,244 obligations with zero errors, but independent
logical mixed correspondence, authenticated mutations, constructor-origin mixed
traces and Context/reconciliation proofs remain open. The interrupted proof and
first cleanup refusal remain retained. Corrected replay and 21 negative-test
groups pass; exact-owned cleanup removes 829,005,824 path-accounted allocated
bytes. Process diagnostics report inaccessible fields rather than asserting
global absence. No fresh native or performance qualification is added. A1/A2,
Native R125, Admission R118B C1/C2/C3 and Resources R116/V3 remain unchanged.
Next are independent mixed-acquisition correspondence and source-bound native
requalification, followed by the remaining reconciliation/generated/resource gates.

Latest native producer qualification (2026-09-24): the
[two-case MI300X packet](evidence/dev-native-producer-mi300x-2026-09-24/README.md)
passes queued and already-published producer chains on GPU 1 from signed
`f3f8206fe`. Both tests retain exact input/dependency custody, release public
events before progress, observe the consumer first and check complete A/B/C/D
buffers: 2,097,152 bytes total, four launches and zero persistent input
materializations. All six strict endpoint observations, ten remote commands,
byte-exact collection, owned remote cleanup and independent absence pass.
The cold musl ELF also passes all 1,400 CPU tests with 22 hardware-only ignores.
Retained replay passes; local collection preserves 138 artifacts and removes
530,509,824 path-accounted allocated bytes of owned scratch. No protected
generated execution, physical overlap, fault, production refinement, aggregate
memory or performance qualification is added. A1/A2 and broader acceptance
remain unchanged. Next are production mixed-input acquisition and completion
reconciliation correspondence, then the remaining generated/resource/native gates.

Latest native producer witness CPU qualification (2026-09-24): the
[two-case preparation packet](evidence/dev-native-producer-witness-cpu-2026-09-24/README.md)
passes all thirteen corrected stages. GNU/musl each pass 1,400 runtime tests,
with 22 hardware-only ignores, and 35 focused passes; 46 GNU runtime doctests,
default-feature checks, strict all-target Clippy, formatting and 3,961 unchanged
source inputs pass. The only source delta is three test paths. New ignored
witnesses use the unchanged R57 authority for queued and already-published
producers, released public events, consumer-first observation and complete
A/B/C/D oracles. Review corrected logical/physical completion confusion and
premature completion-map indexing; unsuccessful attempts are retained. Native
execution is not established by this CPU packet. The next gate is a signed,
source-bound native campaign. Production refinement, generated graphs, aggregate
memory and matched performance remain open; A1/A2 and broader acceptance are unchanged.

Latest active-producer CPU qualification (2026-09-24): the
[persistent input-admission packet](evidence/dev-active-producer-cpu-2026-09-24/README.md)
passes all thirteen stages. GNU/musl each pass 35 focused and 1,400 runtime tests,
with twenty hardware-only ignores; all 46 runtime doctests, default-feature
checks, formatting, strict Clippy and 3,960 unchanged source inputs pass.
Exact producer-aware consumers can retain active three-binding Read owners as
deferred dependencies; destination readiness, complete owner rosters, restored
input backing and final artifact authority remain required before publication.
No new queue, pending registry, materialization fallback or unsafe implementation
is added. Eight new tests cover success, cancellation, rejected custody/shapes,
restoration failure, final-authority rejection, terminal retention and failed
dependency receipts. The initial assertion failure and its diagnostic rerun
remain recorded. Corrected replay and ten evidence-test groups pass; exact-owned
cleanup removes 1,041,485,824 allocated bytes. Native queued/published producer
witnesses, production refinement, generated graphs, aggregate memory and matched
performance remain open. Native R125, Admission R118B C1/C2/C3, Resources R116/V3,
A1/A2 and #182 acceptance are unchanged.

Latest typed-launch CPU qualification (2026-09-24): the
[producer-aware launch packet](evidence/dev-producer-aware-launch-cpu-2026-09-24/README.md)
passes all seventeen stages. GNU/musl each pass 27 focused, 1,392 runtime,
1,561 KFD and 1,021 model tests, retaining twenty hardware-only runtime ignores
and eighteen manual performance/scale model ignores. All 100 doctests, static
checks and 3,960 unchanged source inputs pass. Context and the frozen async
owner retain exact event/producer dependencies, mixed stable/pending input leases
and original binding coverage through bounded producer-first reconciliation.
Single-/multi-device KFD authenticate the same identities without adding an
executor or Worker protocol. Byte-exact evidence replay and eleven verifier-test
groups pass. The original collector's final-receipt failure is preserved;
separate non-deleting recovery verifies cache absence after 1,366,761,472 owned
allocated bytes were removed. Native chains, active-producer backing admission,
mixed-transaction/reconciliation proofs, generated graphs, aggregate memory and
matched performance remain open. Native R125, Admission R118B C1/C2/C3,
Resources R116/V3 and A1/A2 acceptance are unchanged.

Latest native runtime qualification (2026-09-24): the
[current-source matrix](evidence/dev-runtime-native-matrix-mi300x-2026-09-24/README.md)
passes all 22 test commands across two campaigns from signed `354032512`.
The original strict campaign remains rejected: after six passing commands,
the primary-panic immediate endpoint records 1% GPU busy. Its delayed pass
does not override that refusal. The separate sixteen-case suffix passes all
48 endpoint observations. Combined evidence retains 24 harness frames,
65 admitted endpoints and one refusal across 92 remote commands, with the
same cold-built musl ELF and signed source. All 1,365 CPU tests pass; all twenty
hardware-only test names are exercised through the 22 native variants.
Byte-exact collection, exact-owned remote removal, independent absence and
offline replay pass. Local collection retains all attempts, source archive and
ELF before removing 627,740,672 bytes of owned scratch. This does not qualify
protected Worker execution, physical overlap, native faults, aggregate memory,
production refinement or performance. A1/A2 and the broader accepted checkpoints
remain unchanged. Next is producer-aware typed-launch integration with exact
pending-input leases, followed by its independent native/proof qualification.

Latest topology CPU qualification (2026-09-24): the
[link-directory packet](evidence/dev-topology-link-directory-cpu-2026-09-24/README.md)
removes one duplicate directory check per I/O/P2P link without caching topology
or changing the full-host currentness contract. GNU/musl each pass all 1,561 KFD
tests and 1,365 runtime tests, with twenty hardware tests still ignored; all 73
doctests, strict Clippy, formatting and thirteen command stages pass. Replay
checks 3,956 unchanged source inputs, exact named test rosters and 61 raw
artifacts; all eighteen verifier tests pass. The interrupted first attempt is
retained, and both owned Cargo caches are removed. A matched native comparison
of this candidate remains required: no speedup, formal refinement, A1/A2 closure
or broader accepted lane advance is claimed.

Latest strict native characterization (2026-09-24): the
[fresh hot-batch campaign](evidence/dev-xgmi-hot-batch-strict-mi300x-2026-09-24/README.md)
passes all eighteen depth-1/16/32 processes, 134 remote commands, 108 strict
endpoint observations and nineteen local commands on GPUs 5/6. Byte-exact
collection, all three retained ELFs, owned remote removal, independent absence
and strict replay pass. KFD depth-32 whole-batch p50 is 15.427-15.540 ms, versus
HSA 1.065-1.087 ms and HIP 1.015-1.041 ms, with different host timing boundaries.
KFD useful-byte throughput rises to 2.159-2.175 GB/s through batching, but remains
well below the comparators. This is shared-host characterization, not an A7
threshold pass or isolated engine bandwidth. Local cleanup reclaims 424,538,112
bytes. The prior SSH-failed campaign stays rejected. All broader accepted lane
checkpoints and A1/A2 remain unchanged; matched candidate optimization and the
generated-execution/resource/proof gates remain open.

Latest recovered native evidence (2026-09-24): the
[matched hot-batch attempt](evidence/dev-xgmi-hot-batch-mi300x-2026-09-24/README.md)
is rejected by its strict controller gate after SSH timeout and temporary DNS
failure. The remote controller completed all eighteen depth-1/16/32 trials on
GPUs 5/6; its 134 successful commands, 108 strict endpoint observations and
eighteen parsed result records were recovered byte-exactly and replayed. All
three executed ELFs are retained. Separate recovery inventory, collection,
owned removal and path/process absence pass. The original failure remains
immutable, the strict verifier still rejects it, and recovered timings are
excluded from accepted comparisons. Local cleanup reclaims 424,591,360 bytes.
Next is a fresh strict campaign, not a parity or A7 claim. All accepted lane
checkpoints and A1/A2 remain unchanged.

Latest benchmark qualification (2026-09-24): the
[matched hot-batch CPU packet](evidence/dev-xgmi-hot-batch-cpu-2026-09-24/README.md)
qualifies all-slot HIP/HSA persistent-hot callbacks, the bounded KFD hot-only
depth gate and a strict maintained depth-1/16/32 result parser. All 21 command
stages pass: GNU/musl default/all-feature example matrices, real comparator
builds, actual callbacks with CPU APIs, UBSan, compiled negative mutations,
formatting and strict Clippy. One existing optional Rust/C++ segment differential
is skipped. The ten-path source delta leaves production libraries unchanged.
Owned build cleanup reclaims 1,438,052,352 allocated bytes. Native depth-16/32
execution and matched performance are next; no native, formal-refinement,
aggregate-memory or A1/A2 acceptance is added.

Latest matched native performance (2026-09-24): the
[fresh-link scratch comparison](evidence/dev-topology-link-scratch-mi300x-2026-09-24/README.md)
passes complete-buffer checks, explicit teardown and all endpoint observations
on GPUs 5/6, but establishes no latency gain. At one MiB/depth one, process-level
p50 ranges are 14,304.621-14,387.585 us for baseline KFD,
14,319.261-14,347.637 us for candidate KFD, 30.155-30.806 us for HSA and
38.347-38.678 us for HIP. Separate diagnostics attribute median
94.630-94.681 percent to topology discovery. Host timing boundaries differ;
these are not isolated copy-engine measurements. The first cohort reused a
baseline ELF and is excluded in full; corrected cold builds retain distinct
KFD ELFs. No A7 or general HIP/HSA parity claim follows. Whole-host currentness
must not be silently weakened to obtain a speedup.

Latest native resource qualification (2026-09-24): the
[backing-budget MI300X packet](evidence/dev-xgmi-backing-budget-native-mi300x-2026-09-24/README.md)
passes on GPUs 5/6 from signed `419fe4818`. Both isolated capacity rejections
leave healthy unchanged accounts; both original one-byte requests succeed after
release with fresh Context identities. Forward/reverse copies, two retained
publication snapshots, all 36,875 checked bytes and final zero backing accounts
pass. All six strict endpoint observations, byte-exact collection, owned remote
cleanup and independent absence checks pass. Offline replay and six mutation-test
groups pass. This qualifies the exact ordinary pressure/retry/copy/shutdown
workload, not native faults, aggregate memory, general multi-GPU compute, formal
correspondence or matched performance. A1/A2 and accepted lane checkpoints remain
unchanged. The next performance work must retain whole-host freshness rather
than infer a bandwidth gain from this correctness witness.

Latest native-witness preparation (2026-09-24): the
[backing-budget witness CPU packet](evidence/dev-xgmi-backing-budget-witness-cpu-2026-09-24/README.md)
qualifies asymmetric native capacity/retry and bidirectional-copy oracles without
changing production runtime sources. Five example tests pass on each GNU/musl
target and twenty Python tests run without skips. Formatting, strict Clippy,
tool continuity and 3,917 unchanged inputs pass. Pre-import and remote-bootstrap
authentication, scrubbed Git, qualified source ancestry and remote cleanup
sequencing have hostile-input tests. Interrupted and superseded cohorts remain
recorded. Fresh native execution and sealed replay are next; native fault,
aggregate memory, formal correspondence, performance and A1/A2 gates stay open.

Latest resource integration (2026-09-24): the
[XGMI backing-budget CPU packet](evidence/dev-xgmi-backing-budgets-cpu-2026-09-24/README.md)
connects argument-ordered endpoint limits and inert usage snapshots to the
original native accounts. Only typed, healthy, pre-native allocation capacity
rejections return retryable Capacity; native uncertainty and queue-creation
pressure retain terminal custody. Sixteen new regressions pass. Full GNU and
musl each pass 1,552 KFD and 1,365 runtime tests, with twenty runtime hardware
ignores; 73 doctests, formatting, strict Clippy and 3,910 unchanged inputs pass.
The four-thread campaign prospectively extends the full-library bounds after
two retained timeouts; neither failed campaign is promoted to acceptance.
The owned build cache is removed, retaining logs. A fresh asymmetric-budget
native pressure/retry/copy/shutdown witness is next. Aggregate memory,
generated execution, formal correspondence and matched performance remain open.
Native R125, Admission R118B, Resources R116/V3 and A1/A2 acceptance are unchanged.

Latest async API qualification (2026-09-24): the
[early-event CPU packet](evidence/dev-async-operation-events-cpu-2026-09-24/README.md)
adds separate dependency-event and final-observation futures for frozen typed
launches, ordinary async copies and directed peer copies. The shared rooted
driver records events on a separate advance; directed operations gain no
implicit flush. Both reply credits precede command admission. Enqueue alone
does not admit a consumer, and event success is not completion or retry
authority. Seventeen new regressions cover pending diamonds, bounds, Stop,
errors/panics, non-Send owner modes and drain. GNU and musl each pass 1,358 tests
with twenty hardware-only ignores; 46 doctests, formatting, strict Clippy and
3,906 unchanged inputs pass. The owned 661-MiB cache is removed, retaining raw
logs. No native or performance campaign is added. Next are native XGMI backing
budget integration and broader async/native qualification; generated execution,
aggregate accounting and production-code proof correspondence remain open.
Native R125, Admission R118B, Resources R116/V3 and A1/A2 acceptance are unchanged.

Latest native qualification (2026-09-24): the
[directed owner diamond](evidence/dev-xgmi-directed-owner-native-mi300x-2026-09-24/README.md)
passes on GPUs 5/6 from signed `87a9a4c90`, using the unchanged failed witness.
All four copies succeed with pending inputs, one owner thread and producer events
released before progress. All 327,680 buffer bytes pass independent payload,
guard and unchanged-source checks. Runtime shutdown reports Released and complete
cleanup; all six strict host endpoints, collection, owned remote cleanup and
separate absence checks pass. Sealed replay and six mutation-test groups pass.
The original refusal below remains failed. The private checkout and 1.4-GiB
build cache are removed, with logs/payload retained. This validates the exact
native diamond and ordinary cleanup, not faults, formal refinement, aggregate
memory or performance. Native R125, Admission R118B, Resources R116/V3 and A1/A2
acceptance remain unchanged; broader native graphs and the remaining gates stay open.

Latest native-gap correction (2026-09-24): the
[shared-source CPU packet](evidence/dev-xgmi-shared-source-cpu-2026-09-24/README.md)
jointly admits exact directed read/read sharing and selects allocation-disjoint
FIFO publication prefixes. Complete-ready-set flush and exact aggregate requests
reject shared mappings before publication without terminalizing healthy custody.
Sixteen new regressions cover production owner admission, prefix/roster helpers,
both directions, capacity/precedence and Pending/recovered-publication progress.
GNU and musl each pass 1,341 runtime tests with twenty hardware-only ignores;
46 doctests, six example tests per target, fourteen Python tests, formatting,
strict Clippy and 3,904 unchanged inputs pass. Two source reviews find no blocker.
The diamond witness is unchanged and awaits a fresh signed native campaign.
The historical refusal below remains failed. Native R125, Admission R118B,
Resources R116/V3, A1/A2, formal-refinement and performance acceptance are unchanged.

Latest native result (2026-09-24): the
[directed owner attempt](evidence/dev-xgmi-directed-owner-refused-mi300x-2026-09-24/README.md)
fails graph qualification on GPUs 5/6. Root and left are admitted, but right is
rejected because the native allocation-owner gate treats independent readers of
the same source as a hazard. Internal shutdown retains pending resources until
process exit. All six host endpoints, exact collection, external owned cleanup
and separate path/process absence checks pass; these do not qualify the graph.
The sealed refusal replay and six mutation-test groups pass. Correction must
jointly admit exact directed read/read sharing and publish allocation-disjoint
FIFO prefixes, preserving full-ready-set flush and exact aggregate contracts.
Relaxing admission alone would take the same mapping twice. Fresh CPU and native
qualification remain required; Native R125, Admission R118B, Resources R116/V3,
A1/A2, formal-refinement and performance acceptance are unchanged.

Latest native-witness preparation (2026-09-24): the
[directed owner packet](evidence/dev-directed-owner-witness-cpu-2026-09-24/README.md)
adds a four-copy native XGMI dependency-diamond example and source-bound sibling
campaign. The owner releases producer events after tracked join admission and
before progress, checks all five complete buffers, and always inspects explicit
shutdown. Six example tests pass on both GNU and musl; fourteen Python tests
include compiled invalid-CLI checks and mocked failure/postflight orchestration.
Formatting, strict all-target Clippy, tool brackets and 3,902 unchanged inputs
pass. Production runtime and historical witness/evidence sources are unchanged.
Native execution and its sealed replay are next; fault, refinement, aggregate
memory and matched performance gates remain open. Native R125, Admission R118B,
Resources R116/V3 and A1/A2 acceptance are unchanged.

Latest async integration (2026-09-24): the
[directed async packet](evidence/dev-directed-async-peer-cpu-2026-09-24/README.md)
adds ordinary and tracked peer-copy methods using the exact Context directed
progress contract. A private compile-time policy reuses the existing operation
lifecycle without registering an implicit operation flush. Independent stream
registrations and ordinary operations retain their own budgets. Sealed producer
rejections preserve the original diagnostic and uncertain custody. Twenty-one
new CPU tests cover chains/diamonds, scheduling, ownership, credits and hostile
outcomes. GNU and musl each pass 1,325 runtime tests with twenty hardware-only
ignores; 46 doctests, formatting, strict Clippy and 3,818 unchanged inputs pass.
Two independent source reviews find no remaining blocker. The
[contract](runtime-directed-async-peer-v1.md) records the next native dependency
diamond and complete-buffer/cleanup oracles. Native chains and faults, executable
refinement, aggregate memory and matched performance remain open; Native R125,
Admission R118B, Resources R116/V3 and A1/A2 acceptance are unchanged.

Latest Context integration (2026-09-24): the
[directed pending-input packet](evidence/dev-directed-context-peer-cpu-2026-09-24/README.md)
adds the distinct directed scalar Context API, exact pending-writer/input
reservations, retained native terminal facts and bounded producer-first
reconciliation across all ordinary observation paths. Failure, quiescence and
cancellation release consumer inputs without waiting on pending siblings;
Unknown cannot become Success. Legacy, ordered and generated profiles remain
separate. GNU and musl each pass 1,304 runtime tests with twenty hardware-only
ignores; 46 doctests, formatting, strict Clippy and 3,814 unchanged inputs pass.
Two independent source reviews find no remaining blocker. The
[contract](runtime-directed-context-peer-v1.md) scopes one backend action per
submission observation, not an entire stream synchronization. Next is dedicated
async-engine registration without an implicit operation flush, then native
pending-input chains, faults and cleanup. Executable refinement, aggregate
memory and matched performance remain open. This does not advance the accepted
Native R125, Admission R118B or Resources R116/V3 checkpoints or close A1/A2.

Latest directed backend checkpoint (2026-09-23): the
[directed scalar SPI packet](evidence/dev-directed-scalar-peer-cpu-2026-09-23/README.md)
adds explicit success-gated native XGMI admission with exact device/region/stream
and event-to-producer matching. Provisional roots precede shared admission;
directed metadata survives completion until guarded release. Progress uses the
existing one-action selector without changing FIFO readiness or legacy/Worker
contracts. GNU and musl each pass 1,285 runtime tests with twenty hardware-only
ignores; 46 doctests, formatting, strict all-target Clippy and 3,810 unchanged
inputs pass. Two independent code reviews find no remaining correctness or
custody findings. Context pending sources still reject: producer reservations,
retained terminal observations, producer-first reconciliation and bounded async
integration are next, followed by native chains. No native, formal-refinement,
performance or A1/A2 acceptance is added; broader lane checkpoints are unchanged.

Latest Context checkpoint (2026-09-23): the
[scalar dependency-custody packet](evidence/dev-context-peer-custody-cpu-2026-09-23/README.md)
retains exact scalar operation provenance and complete event-to-producer rosters
before backend entry, independently of public events. Release and cleanup guard
producer retains; consumer quiescence discharges them once, while uncertain
attempts remain rooted even without the optional journal. Context now owns the
existing producer-aware journal, but pending-source reads still reject. GNU and
musl each pass 1,265 runtime tests with twenty hardware-only ignores; 46 doctests,
formatting, strict all-target Clippy and 3,807 unchanged inputs pass. Two independent
source reviews find no remaining implementation blocker. Next are explicit scalar
success-gated route admission, producer reservations, producer-first reconciliation
and bounded async integration, then native chain qualification. Historical proofs
retain their captured source; no new native, formal-refinement, performance or
A1/A2 acceptance is added, and the broader lane checkpoints remain unchanged.

Latest scalar progress checkpoint (2026-09-23): the
[bounded XGMI progress packet](evidence/dev-xgmi-scalar-progress-cpu-2026-09-23/README.md)
adds one-action consumer/dependency progress with root-only completion,
retained dependency validation and idempotent FIFO membership in the existing
active record. A separate HashSet design was rejected after reproducing its
restoration-capacity hazard. GNU and musl each pass 1,251 runtime tests with
twenty hardware-only ignores; 46 doctests, formatting, strict all-target Clippy
and 3,805 unchanged source inputs pass. Independent source review finds no
healthy-path flag-transition defect; native rollback remains unqualified.
The [contract](runtime-xgmi-scalar-progress-v1.md) preserves scalar poll/wait and
nonwaiting flush. Next is exact Context producer/dependency retention and
producer-first reconciliation, then consumer-driven native chains. Context
pending-producer reads still reject. No native, formal-refinement, performance
or A1/A2 acceptance is added; the broader lane checkpoints remain unchanged.

Latest native correction (2026-09-23): the
[scalar XGMI flush CPU packet](evidence/dev-xgmi-scalar-flush-cpu-2026-09-23/README.md)
removes synchronous prefix draining and its unbounded backoff. Scalar flush now
publishes one complete ready set of at most 63, rejects larger sets before
publication, and never waits for a scalar batch to complete. GNU and musl each
pass 1,224 runtime tests with twenty hardware-only ignores; 46 doctests,
formatting, strict all-target Clippy and unchanged-input checks pass. This is a
CPU-tested contract correction, not formal correspondence or native evidence.
Larger-backlog bounded progress and pending-producer integration remain next;
the accepted lane checkpoints and A1/A2 status are unchanged.

Latest proof checkpoint (2026-09-23): the
[constructor-origin owner lifecycle packet](evidence/dev-owner-lifecycle-2026-09-23/README.md)
completes its signed-source 62-command campaign: both 1,240-obligation positives,
all 49 strict logical negatives and the raw/inspection/constructor regressions
(384/1,179/1,156 obligations) pass. Runtime-model tests pass 1,021 unit tests
with eighteen existing ignores and 27 doctests; strict Clippy, formatting,
release compilation and source/tool brackets pass. The complete 190-record
packet binds 449 source inputs. Its auditor rejects 39 rehashed malformed
record sets and two source-policy changes, and supports bare Git replay.

This establishes the named constructor-origin normal-return relations, not
physical allocation/unwind, global identity freshness or native execution.
Context still rejects pending-producer reads. Next is custody-safe, idempotent
bounded backend progress, followed by exact producer/dependency ownership,
producer-first outcome reconciliation and native chain qualification. Native
R125, Admission R118B and Resources R116/V3 remain the broader accepted lane
checkpoints; A1/A2, #182 and HIP/HSA parity remain open. No GPU or performance
measurement is added by this proof packet.

<a id="renewed-swarm-dispatch"></a>

The latest [post-R114 dispatch](runtime-swarm-dispatch-r114.md) records the
current three-lane handoffs: Native's remaining live detach/control cleanup,
Admission's generated ISSUE/COMPLETE handoff after accepted R118B C1/C2/C3, and
Resources' V4-J1 proof handoff above the locally accepted
[V3 settlement model](runtime-context-version-settlement-v1.md).
Its immediate assignments supersede the older checkpoint tables below, whose
detailed contracts and historical observations are retained.

Latest implementation (2026-09-18): [direct destination SDMA readback](runtime-primary-queue-release-v1.md#direct-destination-readback)
removes the boxed intermediate buffer and redundant host copy from public
HostVisible reads and DeviceLocal staging readback. It preserves the existing
admission, currentness, retake, custody and cleanup-error policies. CPU tests
cover allocation counts and destination visibility on failure. The full GNU
run passes 1,412 KFD and 1,105 runtime tests; full musl runtime also passes 1,105,
with twenty runtime tests ignored on each target. Ten focused musl KFD tests,
73 doctests and the static checks pass. The subsequent
[native readback campaign](evidence/dev-sdma-direct-readback-native-2026-09-18/README.md)
passes the existing 4-KiB DeviceLocal and HostVisible cold-allocation probes,
including native-versus-shadow read routing, accounting and shutdown assertions.
All six strict endpoints admit; complete collection, exact-owned cleanup and
eleven recorded process-group absence checks close. This does not qualify native
faults or formal correspondence. The copy comparator's DMA timers exclude this
readback, so no matched HIP/HSA throughput gain follows. Accepted milestones
and A1/A2/#182 remain unchanged.

The corrected [primary teardown native run](evidence/dev-primary-envelope-late-selection-native-2026-09-18/README.md)
executes all three test commands successfully from signed `a0db73625`: ordinary
shutdown, installed-root error retention and original-payload panic retention.
The strict campaign still rejects: the panic case's immediate observer records
`sysfs-before-busy` at 1%, although subsequent direct samples and SMI report zero,
no selected PID is observed, and the fixed delayed endpoint passes. The original
refusal is preserved, without retry or causal attribution. All three native
transcripts, collection, exact-owned cleanup and separate absence checks are
retained. This is native assertion evidence, not complete strict-campaign, R126,
formal-correspondence or matched-performance acceptance.

A separate [HIP smoke on newly free GPU 4](evidence/dev-hip-native-copy-smoke-free-gpu4-2026-09-18/README.md)
passes thirteen fully validated 256 MiB round trips and explicit allocation/stream
cleanup from the exact committed comparator. Fresh preflight passed. The
immediate observer retained its original refusal for busy-only telemetry at
T0+0.034 seconds, as permitted by this prospectively declared smoke protocol;
the single fixed delayed endpoint at T0+20.030 seconds passed strictly. The
earlier preflight-refused attempt below remains refused. The new owned remote
directory was removed and recorded-process/path absence independently checked.
This is one-process copy correctness and protocol qualification, not a matched
HIP/HSA ratio, physical-engine equivalence or performance-parity claim.

The earlier [primary teardown native campaign](evidence/dev-r126-primary-envelope-native-2026-09-18/README.md)
passes the normal-shutdown regression on GPU 4, including complete readback,
backing refund and queue-profile checks. Its error fixture then fails before
fault injection because it checks retained-primary eligibility before public
shutdown has settled dispatch and pool custody. Both cases' fresh admission and
strict immediate/fixed-delayed observations pass; the campaign stops without
running the panic case. The owned remote directory is collected, removed and
independently checked absent. This is not error/panic-envelope qualification.
The fixture correction places its assertions at the actual production selection
after shutdown preparation and requires terminal retry to leave that observer
untouched. Its [fresh CPU campaign](evidence/dev-primary-envelope-late-selection-cpu-2026-09-18/README.md)
passes 1,102 runtime tests with twenty ignored on both GNU and scoped musl; all
46 doctests and static checks pass on the final source. Production shutdown code
is unchanged. The subsequent native results above do not upgrade the old failed
run or close strict campaign qualification.

The preceding [primary teardown CPU campaign](evidence/dev-primary-envelope-cpu-2026-09-18/README.md)
passes 1,102 runtime tests on both GNU and scoped musl, with twenty ignored,
plus 46 runtime doctests and the static checks. Two newly compiled hardware-only
error/panic probes enter the production-shared installed-primary helper and
check original custody and terminal-state preservation. That CPU packet does
not execute either probe; deterministic post-install faults are not native ioctl
failures.
R125 remains the accepted Native checkpoint, with R126 under development.

Latest development (2026-09-18): the
[enrollment prerequisite proof](runtime-context-version-enrollment-v1.md)
verifies the ordered admission prefix and conditional final write/truncate
loop, deriving reader preservation from vacant selected slots. Both positive
runs report 168 verified obligations; seven executable mutations each fail the
intended postcondition. Production sorting/search, collision-path output
restoration, allocation partition preservation and an end-to-end enrollment
witness remain unproved. R116/V3 remains the accepted Resources checkpoint.

The [HIP payload validator](evidence/dev-hip-copy-payload-cpu-2026-09-18/README.md)
passes eleven parser calibration groups, ten comparator/mock groups and two
argument tests. Its independent expected configuration, exact round roster,
cleanup counts and native status checks do not establish host admission or
performance. The separately
[prepared native smoke](evidence/dev-hip-native-copy-smoke-1890a64e1-2026-09-18/README.md)
was refused before launch: GPU 4 exceeded the unchanged busy/VRAM bounds and
the PID capture reported a process attached to all eight GPUs. No native HIP
process ran, no retry replaced that result, and the exact owned remote directory
was removed with independent path and recorded-process absence checks. No
matched performance ratio is available.

Latest bootstrap development (2026-09-18): the
[Worker V3 generated-only bootstrap](evidence/dev-worker-generated-bootstrap-2026-09-18/README.md)
opens allocation/stream/copy facilities without generic launch authority and
returns the inherited handoff's original authenticated executable owner before
runtime construction. Ordinary, atomic and collective facade launches reject
before argument encoding; all 37 generated ISSUE CPU cases use this restricted
profile. GNU and scoped musl each pass 1,102 runtime and 283 host tests, with
eighteen and four ignored respectively. The protected verifier/refinement
provider, proof artifacts and actual sandbox/native composition remain missing.
Accepted checkpoints, A1/A2 and #182 are unchanged.

The separate [HIP copy-only diagnostic](evidence/dev-hip-copy-only-cpu-2026-09-18/README.md)
adds bounded directional raw rounds, full-buffer validation and cleanup-gated
output without the legacy allocator workload. Ten CPU mock groups and two
argument-helper tests pass. No native HIP or matched performance is claimed.

The new [committed-source matched attempt](evidence/dev-kfd-native-wait-9b9265c69-2026-09-18/README.md)
ran KFD A/1, KFD B/1 and HSA D/1 on GPU 4, each validating thirteen 256 MiB round
trips. It stopped on D/1's immediate sysfs busy reading of 46%; VRAM was below
the threshold and the later PID capture identified no GPU 4 attachment. Later
idle readings do not erase that refusal or establish its cause. C and later
blocks did not run, no HIP cell exists, and no matched ratio is accepted. The
owned 463 MB remote directory was removed and path/recorded process-group absence
confirmed, with unrelated unreadable `/proc` entries explicitly disclosed.

Earlier copy qualification (2026-09-18): a guarded GPU 4
[native accounting test](evidence/dev-copy-accounting-mi300x-live-credits-2026-09-18/README.md)
passes one public 256 MiB H2D/D2H pair with every returned byte checked, default
cache behavior, exact pool/backing accounting, zero host/device backing after
ordinary shutdown, and inert repeated shutdown. Preflight, immediate and delayed
host endpoints pass. The new
[complete host observer](evidence/dev-copy-host-observation-2026-09-18/README.md)
captures PID attribution even when VRAM or status fails; its 19 CPU tests and the
unchanged guard's 75 tests pass. GNU and scoped musl each pass 1,101 runtime tests
with eighteen hardware/opt-in cases ignored in the
[final CPU campaign](evidence/dev-copy-accounting-live-credits-2026-09-18/README.md).
Two earlier fixture-oracle failures remain separately archived: pool capacity is
not page-rounded backing, and normal live backing credits are retained. Neither
failed attempt reached the explicit copies. No production runtime algorithm or
cache policy was changed. These are sequential shared-host observations, not an
exclusive reservation, matched HIP/HSA performance result, formal refinement, or
an explanation for earlier VRAM refusals. Accepted checkpoints, A1/A2 and #182
are unchanged.

Earlier development (2026-09-18): nonblocking event, stream-progress and paired
observer registration removes the caller-owner's registration gap without
blocking or starting a thread. The existing scheduler retains validation,
capacity, duplicate ordering and paired all-or-nothing admission. Provisional
observer Drop, acknowledgment wakes, queue Stop and reply credits have focused
CPU coverage in the
[registration archive](evidence/dev-async-observer-registration-2026-09-18/README.md).
GNU and scoped musl each pass 1,101 runtime and 282 host tests, including all
thirteen new registration cases; 70 doctests and the static checks pass.
Synchronous calls still reject blocking self-waits. Protected application/native
execution, formal refinement and performance remain separate; accepted
checkpoints, A1/A2 and #182 are unchanged.

Separate native retry (2026-09-18): the [pinned-source GPU 4 attempt](evidence/dev-kfd-native-wait-5d70cb0a-2026-09-18/README.md)
completed thirteen validated A/1 KFD round trips, then failed the unchanged
postflight VRAM guard. The guard returned before its PID query, so this is an
unexplained VRAM refusal, not established contention. The later passing guard
does not restore acceptance. B/C/HSA did not run, no HIP cell exists in this
protocol, and no matched ratio is available. Only the owned 454 MiB remote
directory was removed; an independent absence/reference scan passed.

Earlier development (2026-09-18): [caller-driven owned progress](runtime-generated-typed-completion-v1.md#canonical-application-integration)
adds a thread-affine runtime owner that shares the background scheduler and
retirement rules without spawning a thread. Its cooperative ticks and borrowed
future driver preserve accepted work across deadlines, reject callback reentrancy
and blocking self-waits, and retain uncertain native custody. The
[development archive](evidence/dev-current-thread-owner-2026-09-18/README.md)
records 21 new CPU tests, with 1,088 runtime and 282 host passes on both GNU and
scoped musl, plus 69 doctests. It keeps these results separate from protected/native
execution, formal refinement and performance. That packet did not yet include
nonblocking observer registration. Accepted checkpoints, A1/A2 and #182 are
unchanged.

Earlier development (2026-09-18): the [C5 typed bundle adapter](runtime-generated-typed-completion-v1.md#heterogeneous-bundles)
collects 2 through 64 heterogeneous outputs atomically under the original
completion receipt, preserving the original storage and independent credits.
Eleven new CPU tests cover late-slot failures, maximum arity and observer
lifecycle. GNU and scoped musl each pass 282 host and 1,067 runtime tests, with
four and seventeen ignored respectively; 67 public API doctests pass. The
[qualification archive](evidence/dev-c5-typed-bundle-2026-09-18/README.md)
separates this compositional CPU evidence from protected native bundle execution,
formal refinement and performance. Accepted checkpoints, A1/A2 and #182 are unchanged.

The subsequent [matched native-wait attempt](evidence/dev-kfd-native-wait-matched-mi300x-2026-09-18/README.md)
stopped after A/1 when another process attached to all eight GPUs. Thirteen
legacy KFD round trips and teardown succeeded; no B/C/HSA cell launched and no
performance ratio is accepted. The owned remote scratch directory is removed.
The canonical `cargo fe2o3 run` application sandbox forbids thread creation.
Caller-driven progress addresses the runtime thread-creation prerequisite;
actual sandbox composition and the missing production verifier/refinement
provider remain independent qualification requirements.

Earlier development (2026-09-18): the opt-in persistent SDMA native-wait
[diagnostic success paths](evidence/dev-kfd-native-wait-smoke-mi300x-2026-09-18/README.md)
now run on MI300X with both 1 ms and 25 us requested-sleep ceilings. Each policy
validates 13 full-buffer round trips, 52 identity-bound window records and
explicit teardown; all five shared-host endpoint guards pass. This is not a
matched performance result, native fault campaign, or formal refinement. The
earlier [interrupted comparison](evidence/dev-kfd-native-wait-mi300x-2026-09-18/README.md)
remains unaccepted. Accepted checkpoints, A1/A2 and #182 are unchanged.

Earlier development (2026-09-17): [reader arena invariants](runtime-context-read-invariant-v1.md)
derives free-slot uniqueness and proves constructor/acquire/release preservation,
exact count/lookup/exclusion consequences, and register/abort reader framing.
Exact modeled operation traces preserve the invariant and never reissue reader
incarnations. The 155 whole-crate obligations include 127 inherited and 28 new;
a separate sixteen-case invariant-sensitivity campaign adds one test obligation.
The nonempty formal witness and Rust whole-state lifecycle/4,000-step traces
exercise overlap, last-reader exclusion, unrelated writer outcomes and slot reuse.
The [qualification archive](evidence/dev-v4j4-reader-invariant-2026-09-17/README.md)
records actual results and limits. General base enrollment/membership/settlement
composition, production Rust/native refinement and physical storage remain open.
Accepted checkpoints are unchanged; A1/A2 and #182 remain incomplete.

Earlier development (2026-09-17): [reader commit contents](runtime-context-read-commit-v1.md)
composes the unchanged ordered preflight with executable acquisition/release
loops. It specifies exact lease/output contents, reader multiplicities, free-list
order and incarnation updates, with rejection framing and a scan-to-commit safety
bridge. Successful acquisition retains an explicit selected-free uniqueness
premise; release uniqueness is derived from exact lookup and canonical order.
The proof has 127 whole-crate obligations (103 inherited, 24 new), a separate
15-mutation campaign, recursive pinned include auditing and two new Rust batch
and epoch-boundary witnesses. The
[qualification receipt](evidence/dev-v4j3-reader-commits-2026-09-17/README.md)
records the actual gate outcomes. This does not prove full arena invariant
preservation, base membership/settlement reachability, Rust/native refinement,
physical storage or HIP/HSA parity. Accepted checkpoints are unchanged; A1/A2
remain open.

Earlier development (2026-09-17): [reader contents and ordered preflight](runtime-context-read-preflight-v1.md)
adds a concrete proof over the unchanged V4-J1 journal definitions, source-bound
executable mutations and exact Rust error-priority witnesses. Its scope stops
before acquisition/release commits, invariant preservation and Rust/native
composition. The [qualification receipt](evidence/dev-v4j2-reader-preflight-2026-09-17/README.md)
keeps those obligations separate; accepted checkpoints and A1/A2 remain unchanged.

Preceding development: [generated input leases](runtime-context-generated-read-leases-v1.md)
bind ReadOnly shell custody to the original protected attempt, including launches
without a writer or returned backend handle. Exact completion or quiescent Stop
releases inputs before writer settlement; foreign readers still block retirement.
The [qualification receipt](evidence/dev-v6-generated-read-leases-2026-09-17/README.md)
separates CPU/model evidence from native execution, formal composition and
performance. Accepted checkpoints and A1/A2 remain unchanged.

Preceding development: [typed-kernel read leases](runtime-context-kernel-read-leases-v1.md)
retain pure-Read input batches independently of writer roots, including all-read-only
launches. Original identities are revalidated at issue; versions are captured after
ordered predecessor settlement. The [qualification receipt](evidence/dev-v6-kernel-read-leases-final-2026-09-17/README.md)
keeps CPU lifetime/forwarding evidence separate from initialized-input authority,
native kernels, formal composition and performance. Accepted checkpoints and
A1/A2 remain unchanged.

Preceding development: [copy-source read leases](runtime-context-copy-read-leases-v1.md)
retain original local/peer copy sources through exact quiescence and exclude
source mutation and retirement while readers remain. The owning bounded model
keeps the original issuance proof's scope separate from unproved reader
composition. See the [qualification receipt](evidence/dev-v6-copy-read-leases-2026-09-17/README.md).
Initialized inputs, general kernel reads, native/formal/performance acceptance
and A1/A2 remain open; accepted checkpoints are unchanged.

Preceding development: [generated writer journal integration](runtime-context-version-journal-generated-v1.md)
binds protected ISSUE to original writable allocation IDs and separates generic
observations from protected completion. Successful protected settlement precedes
shell retirement; Stop disposes the complete Unknown writer and independent
read-only members through the existing batch boundary. The
[qualification receipt](evidence/dev-v6-generated-journal-2026-09-17/README.md)
is CPU/test evidence, not native, formal or performance acceptance. A1/A2 and the
accepted checkpoints remain unchanged.

Preceding development: [ordinary Unknown writer disposal](runtime-context-version-journal-disposal-v1.md)
now joins per-allocation owner disposal to exact whole-roster journal retirement.
Disposed Context IDs cannot be used or released twice; all writer credits remain
charged until the complete original roster is disposed. Rejected/quiescent
cleanup can resume; terminal/panicking cleanup retains the remaining custody.
The [CPU qualification receipt](evidence/dev-v6-unknown-disposal-2026-09-17/README.md)
does not qualify native pool residency, protected generated execution, formal
correspondence or performance. Accepted checkpoints and A1/A2 remain unchanged.

Preceding development: the opt-in Context journal now joins ordinary
async launch/copy writers to completion, explicit cancellation and retained
Unknown outcomes. Original submission IDs and independent writer roots preserve
custody even without a returned handle or after metadata release. See the
[async writer boundary](runtime-context-version-journal-async-v1.md) and its
[qualification archive](evidence/dev-v6-async-writers-2026-09-17/README.md).
Generated protected execution, input leases, ordered writers, native residency,
formal correspondence, native qualification and matched performance remain open;
this does not advance the accepted checkpoints or close A1/A2.

Preceding development: the [C6 owned-Stop correction](evidence/dev-c6-owned-stop-2026-09-17/README.md)
now consumes original ready generated replies before deciding whether an active
graph still requires retained custody. Eight new tests include 52 deterministic
owner-loop scenarios across phase boundaries, observer loss, decoder outcomes and
mixed pending work. Frozen-source GNU and scoped musl each pass 942 runtime and
271 host tests, with seventeen and four ignored respectively; all quality gates
and 61 doctests pass.
Production native/formal/performance acceptance and the checkpoints below remain
unchanged.

Preceding [C6 generated graph execution](runtime-generated-graph-v1.md)
now integrates exact graph reservations, original typed completion receipts,
owner-preserving admission, cancellation and accepted-prefix drain in the
existing scheduler. Nineteen focused CPU tests pass. Frozen-source GNU and scoped
musl qualification each pass 934 runtime and 271 host tests, with seventeen and
four ignored respectively; all quality gates and 61 doctests pass. It also
corrects pre-ISSUE direct typed drain and prevents
duplicate generic drain observation of generated submissions. Protected native
graph execution, formal correspondence and performance remain unqualified.

Preceding private C4 completion and the
[C5 typed completion API](runtime-generated-typed-completion-v1.md) now have
separately sealed CPU qualification. C5 adds the original completion observer,
identity-bound charged typed output and blocking join without another reply or
decoder. GNU and scoped musl each pass 915 runtime and 271 host tests, with
seventeen and four ignored respectively; all quality gates and 58 doctests pass.
Protected native typed execution is not qualified. Beyond the C6 development
above and the typed bundle adapter described first, protected native bundle
qualification, remaining production journal integration, formal correspondence
and matched performance remain open; accepted milestones below are unchanged.

Current accepted Native checkpoint: [R125 live prepared persistent cancellation](runtime-live-persistent-cancel-v1.md),
above [R124 ordinary recycled detach](runtime-live-recycled-detach-v1.md).
GNU/musl each pass 2,901 tests with five ignored; 25 leaf gates, 18 compiled
negatives on 18 maps, restored 47/744 suites and 207 calibrations pass.
Two independent reviews verify all 5,709 source identities and the
[576-artifact archive](evidence/local-r125-live-persistent-cancel-2026-09-15/README.md).
Next is retained ordinary primary-queue Release custody, followed by generated
DATA-ADOPT, ISSUE, completion/readback and Stop/drain/graph integration.
Acceptance is CPU/test only; native, formal, aggregate-memory and performance
qualification remain open. A1/A2 and issue #182 remain incomplete.

[R126 development](runtime-primary-queue-release-v1.md) now includes borrowed
foundation restoration, four-resource cleanup, retained Linux teardown and the
ordinary-primary runtime owner. The shared production ordering driver now also
uses original completed-constructor fixture owners. Sixteen constructed-parent
tests cover additional dispatch-data, model-commit and post-abort cleanup paths;
a one-device packetless native probe confirms successful concrete public-root
Drop. Remaining parent/runtime fault coverage and fresh qualification stay open;
R125 is still accepted.

The new R126 directional-SDMA extension retains both original owners and their
native/resource cleanup prefixes under the primary root. It removes the normal
allocation workflow's SDMA routing exclusion, with genuine-token failure tests
and a passing native public allocation/shutdown probe. Five additional
constructed-parent tests bring that cohort to twenty-one. See the R126 document for
execution evidence and remaining qualification; this does not advance the
accepted checkpoint or close A1/A2.

The subsequent R126 pool-trim packet retains the active SDMA buffer and original
metadata through borrowed cleanup and model retake, rejects terminal retries,
guards unfinished queue Drop, and terminalizes runtime trim panics. GNU/musl
each pass 27 constructed-parent tests, 274 shared-memory tests and all 745 runtime
tests; the native public workflow confirms one cached 4096-byte buffer before
successful shutdown. The [development receipt](evidence/dev-r126-pool-trim-2026-09-16/README.md)
separates CPU fault coverage from packetless native success. Full qualification,
native failure/accounting evidence and remaining R126 joins are still open.

The [late-release packet](evidence/dev-r126-late-release-2026-09-16/README.md)
now closes the late queue-resource/signal currentness and model CPU joins, plus
genuine post-pristine-abort detached-ledger admission. GNU/musl each pass 308
selected KFD tests and 745 runtime tests, with one hardware test ignored.
Public native faults, other profiles and full R126 qualification remain open.

Subsequent R126 development binds the first ordinary dispatch to the original
primary, retains NEW/AUX/REBOUND materialization and resident-overwrite owners on
failure, and roots fresh SDMA allocations through native mapping and model
retake. See the [current release status](runtime-primary-queue-release-v1.md#fresh-sdma-allocation-custody)
and [allocation evidence](evidence/dev-r126-sdma-allocation-2026-09-16/README.md).
Runtime host initialization/upload-staging retention and indexed host-write
panic settlement are now implemented, with nine focused tests, GNU/musl runtime
regressions and six MI300X regression probes. See the
[host-write receipt](evidence/dev-r126-host-write-2026-09-16/README.md).
Directional promotion now retains input through model validation/retake and
runtime diagnostic unwind, with constructed and scripted failure matrices. See
the [promotion receipt](evidence/dev-r126-sdma-promotion-2026-09-16/README.md).
Retained recycle/release now roots its input or disposal receipt through cleanup,
model retake and runtime diagnostics. GNU/musl each pass 1,344 KFD and 789 runtime
tests, and eight isolated MI300X probes pass, including zero-cache disposal
before trim. The [recycler receipt](evidence/dev-r126-sdma-recycle-2026-09-16/README.md)
separates injected CPU failures from native success and AUX budget rejection.
Directional demotion now retains input through model loan/retake and runtime
diagnostics, with allocation-free restoration into the original device box.
Eight constructed and nine new runtime tests cover exact mapping/ledger identity,
healthy retry, error/panic settlement and public credit behavior; see the
[demotion receipt](evidence/dev-r126-sdma-demotion-2026-09-16/README.md).
GNU/musl each pass 1,352 KFD and 798 runtime tests; eight isolated MI300X
regression probes pass and the private remote scratch is removed.
The lower synchronous-copy driver now retains allocation, buffer and lease
custody through preparation, publication, wait and model retake. Fourteen new
constructed tests exercise shared mapped operations and actual failure prefixes;
GNU/musl each pass 1,366 KFD and 798 runtime tests, and eight MI300X regression
probes pass. The [development receipt](evidence/dev-r126-sdma-synchronous-2026-09-16/README.md)
records final regression status separately from milestone acceptance.
Runtime outer synchronous-copy normalization, retirement and readback custody
are now implemented, with typed diagnostics and restoration into the original
device box. Eighteen new tests cover exact owners, error/panic precedence and
public read visibility; GNU/musl each pass 816 runtime tests with six ignored.
See the [runtime synchronous receipt](evidence/dev-r126-runtime-synchronous-2026-09-16/README.md).
Native validation was pending in that packet. The later native development
receipt below checks successful copy/readback and pool disposal on the updated
source; native failure matrices remain separate requirements.
Typed backing-capacity disposition now crosses the lower/runtime boundary
without string classification. Warm rejection refunds configured Context credit;
cold rejection was quiescent and still quarantined that credit in that packet.
Fourteen new tests cover lower settlement, pooled compatibility, runtime ownership and
Context behavior; see the [allocation-disposition receipt](evidence/dev-r126-sdma-allocation-disposition-2026-09-16/README.md).
Allocation during pending compute now reuses an established primary/directional
SDMA route. Cold queue creation still rejects active compute without entering
the allocation driver. Seven new CPU matrices cover ordinary primary/auxiliary,
queued work and persistent prepared/published runtime custody, including exact
capacity refunds and terminal initialization retention. Two opt-in native probes
compare real retained ordinary receipt identities across HostVisible/DeviceLocal
allocation; both now pass in the later native development receipt below. See the
[pending-allocation receipt](evidence/dev-r126-pending-compute-allocation-2026-09-16/README.md).
Native/profile/pipeline qualification, formal correspondence and performance
remain open. These are development packets, not R126 or A1/A2 acceptance; R125
remains the accepted CPU/test checkpoint.

The allocation-settlement extension now carries an explicit no-owner outcome
from direct KFD through Context and the multi-device router. Cold typed capacity
failure can refund requested bytes/records while preserving its public
`BackendQuiescent` error; generic quiescence, later initialization/cleanup
failures, and existing Worker V1/V4/V5 protocols remain conservative. See the
[settlement receipt](evidence/dev-r126-allocation-settlement-2026-09-16/README.md)
for validation scope. This does not advance R126, A1/A2 or issue #182 acceptance.

Two native cold-capacity probes are now prepared for HostVisible and DeviceLocal
Context allocation, with explicit isolation/device guards, cold and warm
rejection checks, successful retry, native readback and exact zero-cache disposal
observations. Both now pass in the later native development receipt below; see the
[probe preparation receipt](evidence/dev-r126-native-cold-probes-2026-09-16/README.md).
The preparation receipt's compilation and CPU guard remain distinct evidence.

Auxiliary teardown now retains the original slot and cleanup receipts through
temporary model loan/retake, then publishes vacancy only after closing
currentness. Runtime shutdown clears the exact destroyed auxiliary handle before
later primary work, so a quiescent primary-custody allocation rejection can retry
without a stale second destroy. Constructed-owner cleanup/reuse/failure tests and
a then-unexecuted two-stream native retry probe are recorded in the
[auxiliary release receipt](evidence/dev-r126-auxiliary-release-2026-09-16/README.md).
GNU/musl each pass 1,389 KFD and 842 runtime tests, with eleven native tests
ignored; exact test rosters, source identities and post-run binaries match.
This remains R126 development before N5 adoption, not a new accepted checkpoint.

The [native development receipt](evidence/dev-r126-native-closure-2026-09-17/README.md)
now records eight passing probes on physical MI300X GPU 1 at source `967a62dff`.
These include both cold-capacity probes, both pending-compute allocation probes,
auxiliary shutdown retry, and allocation/copy/pool shutdown regressions. A new
workload then appeared on GPU 1; the guard stopped before the ninth test. The
remaining five planned invocations are not counted as executed. Exact binary
hashes and owned scratch cleanup are recorded. This is bounded native behavior
evidence, not full R126 qualification, physical-overlap proof or a benchmark.

N5 now has a private borrowed native-source handoff that revalidates the original
HSACO/program/ABI and exact source roster while bracketing callback currentness.
It preserves the original buffers and one-shot packet; higher-ranked lifetimes
and an owner-free callback result constrain escape. See the
[development receipt](evidence/dev-n5-native-source-2026-09-17/README.md).
GNU/musl each pass 852 runtime tests with eleven native tests ignored; ten
focused tests per target, three compiled negatives and 33 doctests pass.
That packet supplied the borrowed-source prerequisite only. Subsequent
[N5 native adoption development](runtime-generated-native-adoption-v1.md) now
implements backend phases, four binding routes, lane leasing, rooted retirement
and the private generated async hooks. CPU validation and the unexecuted native
probes are separated in its evidence receipt. All eight MI300X GPUs were occupied;
native route/failure and production carrier/async qualification remain open.
This does not complete N5 or advance the accepted milestones.

The subsequent [N5 readiness correction](evidence/dev-n5-adoption-readiness-2026-09-17/README.md)
defers accepted adoption while lanes are occupied or persistent compute excludes
it. Read-only checks retain the original payload/ticket/hold, preserve ordinary
round-robin progress, and allow empty-prefix Stop/drain. Only clean contention is
retryable; identity/health errors and panics remain terminal. This is CPU-tested
development work, not native or N5 acceptance.

The subsequent [SDMA creation-custody repair](evidence/dev-sdma-creation-custody-2026-09-17/README.md)
retains returned owners across model-retake errors and panics in all six public
creation adapters. CPU fixtures exercise exact retained ownership, real model
reclaim rejection, first-panic precedence and inert retry. They do not execute
native CREATE_QUEUE or qualify additional teardown profiles. This remains R126
development; R125, R118B C1/C2/C3 and R116/V3 remain the accepted checkpoints.

The subsequent [private I2 development](runtime-generated-issue-v1.md) connects
the existing owned generated driver to exact Context/backend submission IDs,
classified native publication, retained poll/recycle receipts and Stop-only
disposal. Generic polling cannot publish deferred work. Physical completion
does not deliver a generated result; ordinary drain retains the operation until
C4 exists. The [development receipt](evidence/dev-i2-generated-issue-2026-09-17/README.md)
separates CPU, exact-fixture native and still-missing protected composition
evidence. This is not I2/N5/R126 acceptance; C4/C5/C6, journal integration,
formal correspondence, aggregate memory and performance remain open.

The following [C4 native readback prerequisite](evidence/dev-c4-native-readback-2026-09-17/README.md)
observes every original initialized coherent DATA member into existing
destinations, including unused read-only inputs and initialized but unwritten
output bytes. Exact roster/plan/submission/generation/cardinality checks precede
copying. Corrected GNU/musl each pass 1,403 KFD, 896 runtime and 45 service-host
tests, with seventeen native ignores. Four separate MI300X lower-fixture reruns
pass with equal pre/post VRAM and owned scratch cleanup. Initial manifest and
native accounting failures remain preserved and excluded. The host decoder,
Context settlement and original completion-cell join remain next; this is not
C4 or milestone acceptance.

The subsequent [private C4 completion integration](runtime-generated-completion-v1.md)
now joins original host destinations, full readback and closing currentness to
exact native/Context settlement, the original decoder/result gate and original
completion cell. Its sealed CPU cohort passes 908 runtime and 262 host tests
on GNU and a musl build with optional legacy HIP linkage disabled. Strict gates
and 53 doctests pass; canonical integration matches all thirty-eight frozen
source/inventory identities. Protected Worker/carrier/native execution and
native fault qualification remain unproven. Public C5 typed completion and C6
generated graph/drain remain next; accepted checkpoints are unchanged.

The subsequent [C5 CPU candidate](evidence/dev-c5-typed-completion-2026-09-17/README.md)
preserves the original completion cell and exact result gate across public
activation. A one-output typed future prebinds both move-only observers, returns
unchanged owners on binding rejection, and preserves its receipt for other
heterogeneous outputs. Poll and blocking join use the same completion/extraction
path. Eighteen frozen source identities and both complete GNU/musl rosters match;
independent source and evidence review pass. This does not accept protected
native completion, C5 as a whole, A1/A2 or issue #182.

Preceding accepted Native checkpoint: [R123 live retained-control release](runtime-live-retained-control-release-v1.md),
with [336 raw artifacts](evidence/local-r123-live-retained-control-release-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,865 tests with
five ignored; seventeen source gates, ten auxiliary checks, restored 128/17
suites, ten compiled negatives across ten maps and 134 checker calibrations
pass. All 5,703 source identities match and all 76 recorded owned process groups
are absent. Acceptance covers this retained persistent-control integration at
the CPU/test boundary only. Native next takes ordinary recycled detach and other
applicable live/control routes, queue cleanup and N5 adoption. Native, formal,
aggregate-memory and performance qualification remain open; A1/A2 are incomplete.

Preceding accepted Native checkpoint: [R122 live detached-data release](runtime-live-data-release-v1.md),
with [402 raw artifacts](evidence/local-r122-live-data-release-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,846 tests with
five ignored; seventeen source gates, ten auxiliary checks, restored 35/10
suites, twelve compiled negatives across twelve maps and 89 parser calibrations
pass. All 5,700 source identities match and all 93 recorded owned process groups
are absent. The exact compiler-path checker rejection remains separate history.
This accepts live data release and runtime outer-roster ownership at the CPU/test
boundary, not all N4 routes, native execution, formal refinement, total memory
or performance parity. Repeated lower-ledger work can still be quadratic.
R123 subsequently accepts retained persistent-control release; remaining live
detach and queue cleanup precede applicable N5 adoption. A1/A2 remain incomplete.

Preceding accepted Native checkpoint: [R121 ordinary and typed-data cleanup](runtime-ordinary-data-cleanup-v1.md),
with [665 raw artifacts](evidence/local-r121-ordinary-data-cleanup-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,818 tests with
five ignored; all seventeen source gates, ten auxiliary checks, restored 59/17
suites, 26 compiled negatives across 25 maps and thirty parser calibrations pass.
All 5,696 source identities match. The musl timeout and reproduced strict-parser
rejection remain separate history. This accepts lower cleanup at its local
CPU/test boundary, not live/queue composition, native execution, formal
correspondence, aggregate memory or performance parity. R122 subsequently
accepts live data release and runtime outer-roster retention at the CPU boundary.

Current accepted Resources checkpoint: [R116/V3 settlement](runtime-context-version-settlement-v1.md#integrated-candidate),
with [378 retained artifacts](evidence/local-r116-context-version-settlement-2026-09-14/README.md).
Reviewed full GNU/musl runs each pass 2,746 tests with five ignored across 48
libtest harnesses and one unchanged harnessless benchmark. All 17 source gates,
ten auxiliary checks, 19/42/12/2 frozen/restored suites and 29 compiled negatives
pass. Nine runner, 31 freeze and 122 qualification-contract tests pass. All
5,688 source identities, the closed collector and both independent archive
reviews pass. This accepts executable-model settlement and counted touched work,
not production Context integration, authenticated receipts, proofs, recovery,
reuse, native execution or performance.

[V4-J1 development](runtime-context-version-journal-issuance-v1.md) now integrates
the canonical issuance proof with actual whole-journal contents framing,
source-bound executable mutations and the missing Rust count-guard tests.
Constructor allocation failure, physical storage, unwind, mechanically verified
Rust correspondence and production Context integration remain open. See the
[development receipt](evidence/dev-v4j1-issuance-2026-09-16/README.md); this does
not advance the accepted R116/V3 checkpoint or claim full journal verification.

Original integrated GNU/musl runs each passed 2,744 tests with five ignored;
two subsequently appended review tests mean those runs are preliminary history,
not final-source prerequisites. The original
isolated 761-model/40-journal results remain separately recorded.

Preceding accepted Native checkpoint: [R117 detached persistent-control cleanup](runtime-detached-persistent-control-cleanup-v1.md),
with [372 retained artifacts](evidence/local-r117-detached-persistent-control-cleanup-2026-09-14/README.md).
GNU/musl each pass 2,762 tests with five ignored. All 17 source gates, ten
auxiliary checks, 16/15/37/7/9 frozen/restored suites and 32 compiled negatives
pass. Nine runner, 31 freeze and 117 qualification-contract tests pass. All
5,689 source identities, the closed collector and both independent archive
reviews pass. Isolated failures retain their original maps. Acceptance covers
scripted lower detached controls only; persistent-data bridges, live/queue
composition, N5, native, formal, memory and performance remain open.
Preceding accepted Native checkpoint:
[R119 persistent returned-data cleanup](runtime-persistent-returned-data-cleanup-v1.md),
integrated above published R118B, with
[464 raw artifacts](evidence/local-r119-persistent-returned-data-cleanup-2026-09-15/README.md)
and two passing independent archive reviews. GNU/musl each pass 2,795 tests with
five ignored. All 17 source gates, ten auxiliary checks and six frozen/restored
suites (15/16/15/37/7/9) pass. All 37 compiled production negatives reach their
named behavioral failures across 30 distinct source maps, with all 5,693 source
identities restored after each. Core/history/runner/freeze/lifecycle/qualification
contracts pass 48/46/9/26/22/227 checks; the closed collector matches the archive.
The isolated cohort and its zero-test compile-only attempt retain their original
provenance. The first integrated musl timeout remains rejected history; only the
completed retry under the unchanged deadline and environment supplies acceptance.
This accepts scripted persistent returned-data/control cleanup, not ordinary
mixed cleanup, data disposal, live/queue composition, N5, native execution,
formal correspondence, aggregate memory bounds or performance parity. R121
subsequently accepts the lower ordinary cleanup and typed-data disposal boundary.

Current accepted Admission checkpoint:
[R118B C1/C2/C3 qualification](runtime-admission-lifecycle-qualification-r118.md),
with [1,432 retained raw artifacts](evidence/local-r118b-admission-lifecycle-2026-09-15/README.md)
and two passing independent archive reviews. Fresh GNU/musl each pass 2,780
tests with five ignored. All seventeen source gates, ten auxiliary checks and
six frozen/restored suites (9/4/5/1/15/14) pass. The 78 compiled negatives cover
74 distinct source maps, with 75 production executions, one combined-defense
execution and two helper-calibration executions distinguished explicitly.

All 5,692 source identities are restored, and the closed collector matches the
archive. Core/prior-history/corrected-history/runner/freeze/lifecycle/qualification
contracts pass 48/45/27/9/57/22/167 checks. Stopped R118, including its rejected
destructor-abort case, remains unaccepted history. The corrected full campaign's
case 73 has a normal named failure; no earlier negative or preliminary regression
is reused as a passing result. This checkpoint changes tests only and adds no
native, formal, total-memory or performance acceptance. Admission next joins
N5 adoption with generated ISSUE/COMPLETE; those production paths remain open.

Preceding Native checkpoint: [R115/N4-R2 lower returning-control cleanup](runtime-returning-control-cleanup-v1.md),
with [285 retained artifacts](evidence/local-r115-returning-control-cleanup-2026-09-14/README.md).
GNU/musl each pass 2,727 tests with five ignored across 48 libtest harnesses and
one unchanged harnessless benchmark. All 17 source gates, ten auxiliary checks,
15/37/7/9 frozen/restored suites and 16 compiled negatives pass. Nine runner,
30 freeze and 87 qualification-contract tests pass. All 5,685 source identities,
the closed collector and both independent archive reviews pass. This accepts
only the lower returning-control subset, not persistent/data cleanup, live
transport, queue teardown, native execution, formal refinement or performance.
R117 subsequently adds the lower detached-persistent boundary above.

Historical checkpoint: [R114/N4-R1 pristine control cleanup](runtime-pristine-control-cleanup-v1.md),
with [254 retained artifacts](evidence/local-r114-pristine-control-cleanup-2026-09-13/README.md).
GNU/musl each pass 2,712 tests with five ignored across 48 libtest harnesses and
one unchanged harnessless benchmark. All 17 source gates, ten auxiliary checks,
37/7/9 frozen/restored suites and 15 compiled negatives pass. Nine runner,
30 freeze and 80 qualification-contract tests pass. Exact restoration of all
5,683 source identities, the closed collector and both independent archive
reviews pass. This is local CPU/test acceptance, not ordinary/returning or data
cleanup, native GPU execution, formal refinement or performance qualification.

For the latest concise assignments, dependencies and exit tests, use
[Runtime Swarm: Remaining Packets](runtime-swarm-next-packets.md).
The three-lane read-only swarm reviewed implementation handoffs, behavioral
oracles and immutable evidence. Primary completed integration and local
qualification; the next implementation packets remain separately assigned.

Historical checkpoint: [R113/V2 membership and whole-roster Begin](runtime-context-version-membership-v1.md),
with [retained evidence](evidence/local-r113-context-version-membership-2026-09-13/README.md).
Seventeen source gates and ten auxiliary checks pass. GNU/musl each pass 2,695
tests with five ignored across 48 libtest harnesses; one existing harnessless
CSV benchmark is accounted separately. Seventeen compiled negatives reject at
their exact behavioral oracles, and restored journal/membership suites pass
23/12. All 5,680 source identities are restored. The closed collector and
independent archive review verify 344 raw artifacts, including retained failures.
Nine runner, nine freeze and 52 corrected collector-contract tests pass. This is
executable-model acceptance only, not settlement, production Context hooks,
authenticated proofs, native execution, aggregate memory or performance.
At R113, Native next took N4-R1. Admission still qualifies C1 and Resources
freezes the reviewed [V3 settlement draft](runtime-context-version-settlement-v1.md).
C1's isolated candidate passes nine focused tests, all 724 runtime-library
tests and strict all-feature/all-target Clippy after a test-only enum correction;
the failed compile is retained.
It was later integrated in stopped R118; corrected R118B now supplies local
qualification without accepting the stopped campaign retroactively.

Historical checkpoint: [R112/N3-L3-D](runtime-uninitialized-device-insertion-custody-v1.md),
with [retained evidence](evidence/local-r112-uninitialized-device-insertion-2026-09-13/README.md).
All seventeen source gates, ten auxiliary checks and nine frozen/restored suites
pass. GNU and musl each pass 2,683 tests with five ignored across 48 harnesses.
All sixteen compiled negatives reject at exact behavioral oracles and all 5,679
source identities are restored. The closed collector and independent archive
review pass. The 263 raw artifacts preserve the two preliminary failures and
excluded UTC-rewind attempt; the pinned continuation supplies a fresh accepted
allocator run and the remaining suites. This adds CPU/source acceptance only,
not native, formal, aggregate-memory or performance qualification.
At R112, refreshed read-only handoffs confirmed C1 and V2 as independent next
packets. R113 subsequently accepted the frozen membership contract, including
empty Pending, full-allocation-key ordering, the final valid epoch increment
and spare-capacity Busy rejection. No production Context consumer is added.

Historical checkpoint: R111 locally accepts
[N3-L3-C uninitialized coherent insertion](runtime-uninitialized-coherent-insertion-custody-v1.md),
with [retained evidence](evidence/local-r111-uninitialized-coherent-insertion-2026-09-12/README.md).
Seventeen source gates, ten auxiliary checks and fourteen compiled negatives
pass. GNU/musl each pass 2,657 tests with five ignored across 48 harnesses;
six frozen/restored suites pass 19/19/14/10/8/4. All 5,676 source identities
match; the collector and independent audit verify 217 raw artifacts. This is
named CPU/shared-sequencer and two direct-session preflight/missing-engine
acceptance, not native, authenticated formal or performance qualification.
At R111, Native next took L3-D; C1 and V2 remained independently ready. The refreshed
[handoffs](runtime-swarm-next-packets.md#next-packet-handoffs) specify L3-D's mixed
terminal slot, C1's test support and V2's whole-roster Begin boundary.

The preceding checkpoint, R110, locally accepts
[N3-L2 initialized coherent insertion](runtime-live-coherent-insertion-custody-v1.md),
with [retained evidence](evidence/local-r110-live-coherent-insertion-2026-09-12/README.md).
Seventeen source gates and ten auxiliary checks pass; GNU/musl each pass 2,638
tests with five ignored across 48 harnesses. Frozen/restored coherent insertion,
device insertion, initializer, borrowed and model-loan suites pass 19/14/10/8/4.
Ten compiled negatives reject at exact assertions and all 5,675 source identities
match. The final collector, exact archive hashes and independent evidence review
pass. No new solver, native or performance qualification is added.
At R110, Native next took N3-L3-C/D uninitialized insertion; Admission C1 and
Resources V2 membership remained independent. Primary owns integration and qualification.

The preceding Native checkpoint, R109, locally accepts
[N3-L1 initialized-device live insertion](runtime-live-device-insertion-custody-v1.md),
with [retained evidence](evidence/local-r109-live-device-insertion-2026-09-12/README.md).
At R109, next was N3-L2 coherent insertion, Admission C1 and Resources V2
membership. Seventeen source gates and ten auxiliary checks pass; GNU/musl each
pass 2,619 tests with five ignored across 48 harnesses. Frozen/restored insertion,
initializer and model-loan suites pass 14/14, 11/11 and 4/4. Eleven compiled
negatives reject at exact behavioral oracles and all 5,673 source identities
match. Preliminary failures and the first exact-path checker rejection remain
preserved. Sixteen clock-contract tests bind eight immutable helpers; the final
collector's closed transcript passes. No new solver, native or performance
acceptance is added. Primary owns integration and qualification.

The preceding Resources checkpoint, R108, locally accepts
[V1 Context writer issuance](runtime-context-version-journal-v1.md#r108-issuance-acceptance),
with [retained evidence](evidence/local-r108-context-writer-issuance-2026-09-12/README.md).
R108 adds the executable issuance model, not a production Context journal.
The
[R105-candidate breakdown](runtime-swarm-dispatch-r105-candidate.md) remains the
detailed dependency map, but its pending-acceptance status is historical.
This accepts the named CPU/model matrix, not native, authenticated
formal or performance qualification.

R108 passes seventeen source gates and ten auxiliary checks. GNU/musl each pass
2,605 tests with five ignored across 48 harnesses. Frozen/fresh-restored model,
credit and batch suites pass 11/11, 3/3 and 6/6. Twelve clock-qualified compiled
negatives reject at exact oracles and all 5,669 source identities are restored.
The original split auxiliary roster, collector rejection and both rejected
mutation cohorts remain preserved. Sixteen evidence-clock tests bind seven
helpers; actual UTC-floor gating fixes the observed interprocess ordering gaps
without rewriting raw child-finish timestamps or changing test deadlines.
No new solver, Linux/KFD, aggregate-memory or performance acceptance is added.

The preceding Native checkpoint remains
[R107/N3-D device initialization custody](runtime-device-initialization-custody-v1.md),
with its own [retained evidence](evidence/local-r107-device-initialization-custody-2026-09-12/README.md).
R107 passes seventeen source gates and ten auxiliary checks;
GNU/musl each pass 2,594 tests with five ignored. Frozen/restored initializer,
borrowed and transition suites pass 11/11, 8/8 and 30/30. Nine compiled negatives
reject at their exact oracles and all 5,667 source identities are restored.
Its original inline-root GNU failure and isolated stack-abort reproduction are
preserved separately. A fallibly preallocated terminal slot replaces inline
engine storage; the same runtime test now passes without a runner stack override.
Original ambient stack limits were not recorded. No new formal, native or
performance acceptance is claimed. See the
[next live integration boundary](runtime-device-initialization-custody-v1.md#next-live-integration).

The historical R113 dispatch was refreshed for its local executable-model acceptance,
above accepted R112 `a2feef229758381b66f962ad8be2b87843eb33a3`.
Scope: finish A1/A2, then advance the remaining
[issue #182 milestones](https://github.com/harsh-nod/fe2o3/issues/182).
The issue was checked through the GitHub API and remains open; its reported
`updatedAt` is `2026-09-12T10:51:18Z`.

This document supersedes the immediate assignment rows in the
[detailed dispatch](runtime-a1-a2-swarm-dispatch-r83.md) and
[next-wave roadmap](runtime-a1-a2-next-wave.md), not their historical evidence.

## Historical R113 Dispatch

Three read-only workers independently reviewed Native, Admission and Resources
through R113 and returned the assignments below. Their review turns are complete;
implementation packets are queued, not unattended background jobs. Primary owns
edits, shared wiring, integration, serialized validation and signed publication
to both topic remotes. Primary completed R113's local source/test qualification;
no solver runs, SSH or GPU jobs were launched.

R108's constructor, existing-ID registration, exact Reserved lookup and abort
model is locally accepted with eleven test functions and twelve compiled
negatives. Resources then took V2 membership/Begin, accepted as R113; V3
settlement is next, not another issuance implementation. R108 adds neither a production Context journal
nor membership, settlement, authenticated proofs or cross-run leases.

R106 background: it extends existing transition custody across coherent
initialization's copy stage. All seventeen source gates and ten auxiliary checks pass. GNU/musl each
pass 2,583 tests with five ignored; frozen/restored coherent, borrowed and
transition suites pass 10/10, 8/8 and 30/30. Seven compiled negatives reject at
their intended oracles, and all 5,665 source identities are exactly restored.
The first collector's cross-crate roster rejection and preliminary attempts
remain preserved. Publication follows planning-only parent
`42d614d352f39fb13b4026b6d2c2b9af94a41200` above accepted R105
`3ae84f8542ef82d4615f5e5ff377e2a2b076e29a`.
Original-engine composition, live KFD, authenticated adapter refinement and
matched performance remain open.

The first three implementation packets are independent; shared module wiring
and all builds remain serialized. Each worker supplies a bounded source/test
handoff, not an unreviewed concurrent edit to Context or the queue owner.

R106's ten tests exercise 130 initializer outcomes and five direct private-copy
rejections, plus one positive pre-effect retry control. CPU and mapped owners,
the original foundation, unrelated allocation and exact charges remain in the
same fixture. The existing borrowed regression supplies the negative's immediate
quarantine oracle before retry. These are shared-adapter tests with scripted
native leaves, not successful Linux initialization or protected execution.

| Worker | First packet | Concrete exit requirement |
| --- | --- | --- |
| Native: `native_replacement_handoff` | N4-R1: pristine active-control cleanup | Root the active control before disposal callbacks; preserve exact mapped/unmapped/disposed custody, progress and untouched data/continuation. Reuse existing terminal-abort storage. |
| Admission: `submission_identity_handoff` | C1: CO-2A in new `context/tests/submission_identity_tests.rs` | Exercise all eight existing ingresses with exact-coordinate substitution, genuine cached completion, backend-ID reuse, destroyed-stream semantics and rejection precedence. Rejection must leave supplied handles, retained owners and pre-existing callbacks unchanged and precede backend entry. Primary owns integration of the candidate's module wiring and test-backend cancel counter. |
| Resources: `r102_evidence_review` | Implement and qualify the frozen V3 settlement/cost contract | Retained-roster success, exact NoEffect and sticky Unknown; no epoch rollback, partial release or commit-time growth. Preserve accepted V1/V2 and count O(k) touched work. Proofs are V4 and production integration V5. |
| Primary | I1: integrate reviewed N4-R1, C1 and V3 packets above accepted R113 | Retain failed attempts, run full/focused/auxiliary and compiled-negative checks, restore exact source and review evidence before signed publication to both repositories. Keep local source, model/proof, native and performance acceptance separate. |

### Historical R113 Implementation Batches

The three worker slots are Native, Admission and Resources; Primary is the
fourth slot. Workers supply read-only source/test reviews by default. Primary
owns all edits, shared wiring, test campaigns and publication. The batches below
are assigned work orders, not claims that code is implemented or that workers
continue running after their review returns. R107-R113's named local acceptance is
recorded above; it does not close the following implementation or qualification packets.

| Owner | Next bounded implementation batch | Required local exit |
| --- | --- | --- |
| Native | N4-R1: pristine active-control cleanup | Root the active control before disposal callbacks; retain exact failed-prefix custody and untouched data/continuation without cleanup retry. |
| Native, following applicable R1 contracts | N4-R2 and data cleanup custody | Root owners before validation and preserve untouched owners through failed cleanup; confirmed disposal alone permits release/reuse. |
| Admission | C1: existing Context identity gates, starting with five coordinates across eight ingresses, then genuine cache and backend-ID reuse | Rejected handles/records/pre-existing callbacks unchanged; no backend entry; consuming release returns the supplied handle; destroyed-stream and terminal/deadline/graph precedence preserved |
| Resources | V3: implement the frozen success/NoEffect/Unknown settlement and cost contract | Exact retained-member settlement, sticky Unknown and burned epochs; failure atomicity and counted O(k) touched work without commit-time allocation |
| Primary | I1: integrate one reviewed batch at a time; separately prepare Q1/Q2/Q3 acceptance contracts | Immutable candidate source, focused/full/auxiliary checks, decisive compiled negatives, exact restoration and independent evidence review before signed pushes to both topic remotes |

R107's N3-D implementation lives in the private
`crates/fe2o3-kfd/src/shared_memory/device_initialization.rs`, with Primary-owned
`shared_memory.rs` wiring. The owning root uses a fallibly preallocated terminal
slot in the engine after the original inline-storage stack regression. This adds
one pre-effect allocation per engine, not per initializer or failure. Keep its in-place core
usable by R109's device-insertion owner. A per-call native-attempt marker belongs
immediately before `reserve_va`; old engine activity or quarantine is not proof
that this call crossed that boundary. Invalid source/layout/capacity inputs are
host-only rejections and need not be retained forever. Preserve first-currentness
behavior separately; an ambiguous reservation attempt requires retention even
before a device record exists. Do not fabricate an unreturned lease.

The N3-D fixture must call the same complete-entry helper as production, not
manually reconstruct validation/allocation ordering. Use an unrelated live
anchor with spare capacity, both configured and unconfigured budgets, and exact
account/device/VM/lease/native-argument comparisons. Device initialization leaves
the original queue foundation unchanged. Cover all six currentness boundaries,
allocation/map/access/readback/unmap errors and panics, and final GPU-map
prefixes. R107 adds narrow mapping-argument observations before fault injection;
these do not qualify actual Linux routing. For admitted or otherwise
terminal failures, clear injected faults only after asserting quarantine, then
prove no additional native work or debit. Separately retain a pre-effect retry
control: the unconfigured first-currentness panic and healthy validation/capacity
rejections must not acquire new terminal state merely from initializer custody.

C1's new `crates/fe2o3-runtime/src/context/tests/submission_identity_tests.rs`
starts with forty pending and forty event-completed, uncached-handle
coordinate/ingress rejection cells plus valid controls. Reuse
existing validators and genuine completed/released submissions; do not reset
records to manufacture cached states. Primary owns integration of the candidate's
module wiring and test-backend cancel-entry counter. These are isolated
preliminary checks, not packet acceptance; the nine functions pass as recorded above.

V1's new `crates/fe2o3-runtime-model/src/context_version_journal.rs` remains
`no_std` with `alloc`, no unsafe code or I/O. Preserve the frozen construction-only
policy, zero watermark and independent explicit A/W capacities. Register 41,
then 44, and still resolve 41; reject unregistered 42, replay after abort/reuse,
foreign coordinates and invalid integer limits. Dropped references retain
capacity. This batch does not implement Begin, settlement, a production Context
journal or another ID allocator.

The accepted V1 matrix includes valid Context generation `1`, both defensive abort
capacity-rejection branches, populated-journal work controls and rejection-work
counts. Deliberately corrupted private states use exact rejection snapshots,
not the healthy-state auditor. Counted primitive accesses and a textual source
guard are not a general complexity proof or an allocator measurement.
Constructor allocation failure remains a coverage limitation: it was not
injected. Membership and Begin followed R108 and are now accepted as the R113
model; settlement and the production Context journal remain subsequent packets.

### Dependency Checks

| Work | Ready independently | Actual integration prerequisite |
| --- | --- | --- |
| N4-R lower cleanup | Contract/source review and then its bounded implementation | N4-L live cleanup and N4-QA/QP teardown consume the lower cleanup contracts; they need not wait for C1 or V1 |
| C2 descriptor identity; C3 reply custody | Both test packets are ready now; C1-first is scheduling only | Joint I2 consumes C1/C2 and the preissue C3 contract/oracles |
| V4 journal proofs | Start with stable V1 definitions, extend as V2/V3 land | Complete proof acceptance covers membership/settlement too; inventory checks are not solver evidence |
| V5 production journal | Adapter implementation can start once V1-V3 contracts are stable | Production verification needs authenticated model proofs plus correspondence for the actual commit, not just an unused hook |
| M1 domains; M2 cost inventory; M3 host-image ceiling | Domain design, backing/control cost work and the isolated host-image ceiling can start now | Aggregate claims need integrated domains; native residency needs backing/control ownership; total M4 closure includes concrete journals and leases |
| I2 non-reusing generated ISSUE | Freeze the mutation-hook contract before integration | N5 DATA-ADOPT plus C1/C2/C3; full V7/V8 reuse is not a prerequisite |
| C6 generated GRAPH/DRAIN | Extend existing ordinary graph/drain contracts | Real generated ISSUE/COMPLETE is required; C5-first is the planned API order, not a separate semantic prerequisite |
| Cross-run input reuse | Contract/model work can proceed with the journal lane | Complete V7 mutation coverage/ordered writers/recovery and V8 exact input leases before enabling reuse |

At the historical R113 checkpoint, C1 had an isolated, preliminarily tested
candidate but was not integrated; C2/C3's proposed test files were absent.
R108 and R113 supplied V1/V2's named model-only acceptance, with V3 settlement
then Resources' next implementation packet. Later progress is recorded above.
A production Context journal and authenticated correspondence remain open.
Existing ordinary typed async launch, graph/drain, identity validators,
resource credits and native budgets must be extended, not replaced.

### Primary Qualification Packets

| Packet | Concrete next work | Completion boundary |
| --- | --- | --- |
| Q1: adapter correspondence | Map each production transition to its shared model definition and owned resource projection; register property-specific proofs/negatives as each adapter lands | Authenticated source/tool/runner/transcript identities and explicit external contracts; abstract proof success alone cannot close native ownership |
| Q2: native resource qualifier | Add the missing N1/N2/cache-budget example, runner and checker using the existing budget configuration APIs; qualify exact charges, pressure, teardown and currentness prefixes | Actual original-engine Linux/KFD observations, configured limits and retained failures; existing R66/drain runs do not exercise these optional budgets |
| Q2 reliability follow-up | Reproduce R103's default-concurrency musl watchdog failures under recorded load/concurrency | Preserve original failures; distinguish scheduler delay from runtime behavior without weakening deadlines or treating filtered reruns as full-suite acceptance |
| Q3: matched performance | Freeze comparable kernels, complete-output oracles, residency/copy semantics, sizes, in-flight depth, warmup and thresholds before tuning KFD/HIP/HSA producers | Report workload-scoped latency, throughput, bandwidth, CPU/memory and tails with raw repetitions; device timelines establish physical overlap, not host queue depth |

Worker V3/compiler ownership is a separate production gate, not a fourth
background worker. The generated preparation path still supplies no production
adoption hooks, and the host admission module ships no concrete semantic-to-machine
refinement backend or owned proof artifacts. Primary coordinates that dependency
with the owning teams; a fabricated receipt or passing fixture cannot enable
protected execution. Later A3-A7 and the external issue handoffs below retain
their own acceptance criteria.

MI300X scheduling stays Primary-owned: check availability, use task-owned staging
and processes, archive results, and remove only those resources. Disruptive fault
tests need an isolated window. No SSH, GPU job or solver run is part of this
dispatch refresh.

The refreshed bounded handoffs add these implementation constraints:

- N3-C now retains its CPU token across copy/map failures. N3-D must root the
  owned source before validation and keep the device lease through CPU
  initialization and the final GPU map, using a borrowed map core. Preserve
  PUBLIC flags, complete arbitrary-source readback and repeated-byte no-second-
  readback behavior. N3-L consumes both accepted lower paths.
- C1 starts with five identity coordinates across eight ingresses. Only
  poll/wait/event require a live stream; retained-identity operations preserve
  their existing semantics. Use genuinely completed/released handles and
  actual backend-ID reuse, not manually reset records. Keep validators unchanged.
- V1 uses a preallocated W-slot Reserved arena/free stack and full-key slot
  references. Lookup/abort must not reapply the registration watermark. Dropped
  references retain capacity; abort never rolls the watermark back. No Begin,
  second ID allocator, runtime activation API or production journal is added.
  Require constant-work issuance/lookup/abort with no post-construction
  allocation; V2/V3 separately establish O(k) touched work and O(A + W) storage.
  Compiled negatives must detect watermark-based lookup, abort rollback,
  missing full-key validation and watermark changes on capacity rejection.

### Historical R113 Scheduling Waves

| Wave | Native | Admission | Resources | Primary |
| --- | --- | --- | --- | --- |
| Ready now | N4-R1 pristine control cleanup, then applicable lower cleanup | Qualify isolated C1; C2/C3 independently ready | Implement frozen V3 settlement; M1 design, M2 cost inventory and M3 host-image ceiling independently ready | Integrate one immutable-source campaign at a time; latest Native checkpoint and assignments are at the top of this document |
| After local prerequisites | N4-L after N4-R; auxiliary/full destruction after cleanup contracts | C2/C3 in the next review slots; their order is scheduling only | V3 settlement/cost, V4 proofs, then V5 production journal | Integrate reviewed packets without conflicting shared edits |
| Integration | N5 nonpublishing DATA-ADOPT, then joint I2 ISSUE | I2, C4 COMPLETE, C5 generated API and C6 generated GRAPH/DRAIN | V6 initial hooks, V7 complete mutation/recovery, V8 cross-run leases; M2/native residency | Original-engine composition, incremental correspondence and retained-memory closure |
| Qualification | Native depth, budget, overlap and teardown matrix | Wake/cancel/drain and complete-output oracles | Exact versions, bounded retained resources and terminal charges | Q1 proofs, Q2 hardware/fault qualification, Q3 matched HIP/HSA benchmarks |

The same three worker slots rotate through these packets. An independently
ready packet is not blocked by its position in a worker's queue. Builds, shared
source edits, proof runs and MI300X scheduling remain Primary-owned and
serialized; no hardware work is launched by this dispatch.

### Ordered Backlogs

These labels are short dispatch aliases for the detailed contracts below, not
new mechanisms or proof claims. Within-lane order is the worker's planned
sequence; it is not a prerequisite where the packets are explicitly independent.

| Lane | Ordered packets after the first assignment | Source boundary / acceptance |
| --- | --- | --- |
| Native, locally accepted baseline | N3-L: live data insertion/replacement | R109-R112 retain incomplete prefixes and returned owners, reserve identity capacity before effects and commit metadata only after retake. Named CPU/shared-sequencer and concrete preflight/missing-engine acceptance; native/formal qualification stays open. |
| Native | N4-R: lower release -> N4-L: live detach/release | Existing `queue_dispatch_binding.rs`, shared-memory release/unmap and `queue_live/fixed_dispatch.rs`. First/middle/last cleanup errors and panics retain untouched owners. Failed retake after disposal cannot restore retry authority or commit a reusable hole. |
| Native | N4-QA: auxiliary destroy; N4-QP: full/returning destroy | Existing `queue_live.rs` teardown paths after the lower/live cleanup contracts. Retain the taken lane and original parent through every cleanup prefix; exercise actual later-slot destroy/recreate and ordinary/attached/detached return modes. Slot reuse requires confirmed full disposal. |
| Native | N5: DATA-ADOPT -> joint I2: ISSUE | Existing generated preparation, shell and backend owners. Adoption binds original bytes without publication; ISSUE binds one logical submission and one permit to real resources without retrying uncertain publication. |
| Admission | C2: CO-2B descriptor identity; C3: CO-3A reply/custody gaps | Both are independently ready now using existing authorization validators and R80/R83 lifecycle fixtures. No second validator, reply, decoder or dummy native completion adapter; neither waits for N5. |
| Admission | Joint I2: ISSUE -> C4: CO-4/COMPLETE -> C5: generated typed-output future/API -> C6: generated GRAPH/DRAIN | Extend existing ordinary async futures and graph/drain support. Actual publication/completion identity, complete readback and closing currentness precede retained decoding/readiness. Exercise wake races, bounded admission, observer loss, cancellation and accepted-prefix drain. |
| Resources | V3: .2c settlement/cost -> V4: .2d authenticated proofs -> V5: .3 production journal | R113 accepts V2 membership at the model boundary. Preserve exact member/free-slot invariants and O(k) touched work. Prove shared definitions, then separately qualify the actual Context commit; an unused hook is not integration. |
| Resources | V6: .4/.5 initial hooks -> V7: complete VER-1B mutation coverage, ordered writers and Unknown recovery -> V8: VER-2 cross-run leases | Every write family must invalidate before effects and settle before callbacks. Do not enable reuse with only host-write/copy coverage or a single-writer staging profile. |
| Resources with Native | M1: aggregate domains/headroom; M2: native backing/control/slot admission; M3: host/native cache residency; M4: total retained-memory bound | Reuse existing accounts, budgets and caches. Domain design and host-image ceiling are independently ready; native residency and compound pre-effect admission depend on backing/control integration. Include terminal retention without double charging. |
| Primary with all lanes | Q1: incremental adapter correspondence; Q2: timing/depth/overlap/fault qualification; Q3: matched HIP/HSA performance | Keep CPU/source, authenticated formal, live KFD and performance acceptance separate. Investigate R103's unaccepted default-concurrency musl watchdog failures without weakening deadlines. |

R104 supplies N1, R105 supplies N2, R106 supplies N3-C and R107 supplies N3-D's
named local acceptance; R109-R112 supply N3-L's named local acceptance.
The remaining critical integration path is applicable **N4-R -> N4-L/QA/QP**,
then **N5 DATA-ADOPT -> I2 ISSUE ->
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
facade fixtures. R105 replaces the consuming pristine helper at its own named
CPU boundary, without establishing successful public/native binding. Neither an
injected closure nor a passing ownership snapshot establishes native execution.

C1/C2/C3 are integrated and locally qualified in R118B; their preliminary isolated
focused/runtime-library checks retain their original source cohorts. R108 and R113 supply
V1/V2's named model-only acceptance. R116 now adds locally accepted settlement
in that model, as recorded at the top.
Existing Context identity validators, ordinary typed async launches and ordinary
graph/drain paths should be reused. Generated production preparation still
supplies no adoption hooks, and R65's graph-local versions are not a Context
journal or cross-run input lease. These are implementation boundaries, not
reasons to duplicate the existing runtime.

Q2 still needs the actual native N1/N2/cache budget qualifier, example and
checker. Existing R66/drain runners do not exercise those optional budgets and
cannot substitute for that missing harness. The concrete Worker V3 refinement
backend and owned proof artifacts remain a required production join; fixture
ISSUE/COMPLETE cannot supply protected-execution authority.

M4's retained-memory inventory must include R107's preallocated terminal slot
once per engine. Native reservation may retain extra vector capacity after a
later failure; unchanged logical metadata does not imply unchanged allocated
bytes. Qualify both without double charging the same owner.

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

R113 locally accepts **V2 allocation membership and whole-roster Begin**, with
[retained evidence](evidence/local-r113-context-version-membership-2026-09-13/README.md).
Seventeen source gates, ten auxiliary checks and seventeen compiled negatives
pass. GNU/musl each pass 2,695 tests with five ignored across 48 libtest harnesses,
plus separately accounted harnessless benchmark output. Restored journal and
membership suites pass 23/12. All 5,680 source identities and 344 raw artifact
hashes match; the closed collector and independent audit pass. Eleven dynamic
tests and one source guard are new. The archive retains rejected attempts and
52 corrected collector-contract tests, plus nine runner and nine freeze tests.
No settlement, production Context integration, authenticated formal, native,
aggregate-memory or performance qualification is added.
Historical next at R113: **N4-R1 cleanup + C1 qualification + V3 settlement contract freeze**.

R112 locally accepts **N3-L3-D uninitialized device insertion**, with
[retained evidence](evidence/local-r112-uninitialized-device-insertion-2026-09-13/README.md).
Seventeen source gates, ten auxiliary checks and sixteen compiled negatives
pass. GNU/musl each pass 2,683 tests with five ignored across 48 harnesses;
nine frozen/restored suites pass 14/12/11/19/19/14/10/8/4. All 5,679 source
identities match. Twenty-five dynamic tests and one wiring guard are new; two
negatives cover reused model-loan substrate. The closed collector and independent
audit pass with 263 raw artifacts, preserving two preliminary failures and one
excluded UTC-rewind attempt with its separately pinned continuation. No native,
formal, performance or aggregate-memory qualification is added.
At R112, next was: **N4-R1 cleanup + C1 identity + integrated R113/V2 qualification**.

R111 locally accepts **N3-L3-C uninitialized coherent insertion**, with
[retained evidence](evidence/local-r111-uninitialized-coherent-insertion-2026-09-12/README.md).
Seventeen source gates, ten auxiliary checks and fourteen compiled negatives
pass. GNU/musl each pass 2,657 tests with five ignored across 48 harnesses;
six frozen/restored suites pass 19/19/14/10/8/4. All 5,676 source identities match.
Eighteen dynamic tests and one wiring guard are new; two negatives cover reused
model-loan substrate. The closed collector and independent audit pass, with 217
raw artifacts preserving three preliminary failures and the excluded overlap.
No native, formal, performance or aggregate-memory qualification is added.
At R111, next was: **N3-L3-D device insertion + C1 identity + V2 membership**.

R110 locally accepts **N3-L2 initialized coherent insertion**, with
[retained evidence](evidence/local-r110-live-coherent-insertion-2026-09-12/README.md).
Seventeen source gates, ten auxiliary checks and ten compiled negatives pass.
GNU/musl each pass 2,638 tests with five ignored across 48 harnesses;
frozen/restored coherent insertion, device insertion, initializer, borrowed and
model-loan suites pass 19/14/10/8/4. All 5,675 source identities match. Eighteen
dynamic tests and one routing guard are new; two negatives cover reused model-loan
substrate. All preliminary attempts and 183 exact raw artifacts remain preserved.
The final collector and independent evidence review pass. No solver, live native
success or performance qualification is claimed.
At R110, next was: **N3-L3-C/D uninitialized insertion + C1 identity + V2 membership**.

R109 locally accepts **N3-L1 initialized-device live insertion**, with
[retained evidence](evidence/local-r109-live-device-insertion-2026-09-12/README.md).
Seventeen source gates, ten auxiliary checks and eleven compiled negatives pass.
GNU/musl each pass 2,619 tests with five ignored across 48 harnesses;
frozen/restored insertion, initializer and model-loan suites pass 14/14, 11/11
and 4/4. All 5,673 source identities match. Preliminary failures and the original
checker rejection remain preserved. Scripted-engine composition and concrete
missing-engine facade coverage are distinct; neither qualifies live native
success, authenticated formal refinement or performance.
At R109, next was: **N3-L2 coherent insertion + C1 identity + V2 membership**.

R108 locally accepts **V1 Context writer issuance**, with
[retained evidence](evidence/local-r108-context-writer-issuance-2026-09-12/README.md).
Seventeen source gates, ten auxiliary gates and twelve clock-qualified compiled
negatives pass. GNU/musl each pass 2,605 tests with five ignored across 48 harnesses;
frozen/fresh-restored model, credit and batch suites pass 11/11, 3/3 and 6/6.
All 5,669 source identities match. Both rejected mutation cohorts and the
original auxiliary roster rejection remain preserved. This is model-only
acceptance, not a production journal, new proof, native or performance result.
At R108, next was: **N3-L1 live insertion + C1 identity + V2 membership**.

R107 locally accepts **N3-D device initialization custody**, with
[retained evidence](evidence/local-r107-device-initialization-custody-2026-09-12/README.md).
Seventeen source gates, ten auxiliary checks and nine compiled negatives pass.
GNU/musl each pass 2,594 tests with five ignored; frozen/restored initializer,
borrowed and transition suites pass 11/11, 8/8 and 30/30. All 5,667 source
identities match. Original inline-candidate GNU/stack failures and preliminary
attempts remain preserved. This is CPU/shared-sequence acceptance plus the named
runtime stack regression, not native, authenticated formal or performance
qualification. At R107, next was: **N3-L live insertion + C1 identity + V1 issuance**.

R106 locally accepts **N3-C coherent initialization custody**, with
[retained evidence](evidence/local-r106-coherent-initialization-custody-2026-09-12/README.md).
Seventeen source gates, ten auxiliary checks and seven compiled behavioral
negatives pass. GNU/musl each pass 2,583 tests with five ignored; frozen/restored
coherent, borrowed and transition suites pass 10/10, 8/8 and 30/30. All 5,665
source identities match. Ten new tests contain 135 matrix cases plus one
pre-effect retry control; the collector separates initializer and private-copy
outcomes. Its original roster rejection is preserved. No new solver, native or
performance result is claimed. Next: **N3-D device initialization + C1 identity +
V1 issuance**.

R105 locally accepts **N2 pristine rebind custody**, with
[retained evidence](evidence/local-r105-pristine-rebind-custody-2026-09-12/README.md).
Eight new dynamic tests and two migrated names cover 119 scenarios. All
seventeen source gates, ten auxiliary checks and nine repeated compiled
negatives satisfy acceptance; GNU/musl each pass 2,573 tests with five ignored.
All 5,664 source identities match. Original chronology rejection, preliminary
attempts and their raw artifacts remain retained. This is CPU/shared-sequence
acceptance only. Next: **N3-C initialization + C1 identity + V1 issuance**.

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
At R104, next was **N2 pristine rebind + CO-2A Context identity + VER-1A.2a issuance**.

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
| Locally accepted Native packets | Primary + Native review | Replacement inputs, ordinary and pristine rebind | R103/R104/R105 retain their named CPU/shared-sequence acceptance; no original-engine or hardware qualification |
| Locally accepted initializers | Primary + Native/Resources review | R106/N3-C coherent and R107/N3-D device initialization custody | Named CPU/shared-sequence acceptance only; reuse the accepted lower helpers, not a duplicate initializer |
| Locally accepted live insertion | Primary + Native/Resources review | R109/N3-L1 initialized-device, R110/N3-L2 initialized coherent, R111/N3-L3-C uninitialized coherent and R112/N3-L3-D uninitialized device insertion | Named CPU/shared-sequencer and concrete public missing-engine boundaries: R109/R110 facades versus R111/R112 direct APIs. No native success or formal refinement claim |
| Next Native packets | Native | Qualify R126 synchronous-copy custody, typed capacity disposition, allocation settlement, warm pending-compute allocation and remaining queue lifecycle, then N5 DATA-ADOPT | R124 ordinary recycled detach and R125 live prepared cancellation are accepted at their CPU/test boundaries. R126 ownership/teardown extensions, typed capacity disposition, allocation settlement and warm pending allocation are implemented development work; native/profile/pipeline qualification remains open. Explicit no-owner settlement now refunds direct Context credit; generic, later-stage and Worker quiescence still quarantine it. Confirmed disposal alone does not permit slot reuse |
| Locally accepted Admission packets | Admission | R118B C1 Context identity, C2 descriptor identity and C3 reply/custody tests | Eighteen added tests, full GNU/musl and 78 compiled negatives; existing validators and lifecycle machinery, with no native/formal/performance acceptance |
| Next Admission packets | Admission | Joint I2 ISSUE, C4 COMPLETE, then C5 typed output and C6 GRAPH/DRAIN | N5 adoption, exact publication/completion custody and retained R118B regressions; no invented completion adapter |
| First Resources packets | Resources | Qualify canonical V4-J1 issuance contents, then Rust/storage correspondence and membership/settlement proofs | R108 issuance, R113 membership and R116 settlement are locally accepted executable models. The canonical V4-J1 proof, source-bound executable mutations and Rust count-guard tests are now integrated development work; the linked receipt records verification results separately from production Rust/model correspondence, storage and Context integration, which remain open |
| Next Resources packets | Resources | VER-1A.3 journal, .4/.5 hooks, complete VER-1B, then VER-2 leases | Exact Context IDs; complete mutation coverage, ordered writers and recovery before reuse |
| Native/runtime integration | Primary + Native/Admission | N3/N4 -> DATA-ADOPT -> ISSUE -> CO-4/COMPLETE -> generated typed API -> GRAPH/DRAIN | C1/C2/C3 join ISSUE; real publication/completion identity, exact readback and custody; cross-run reuse also needs V7/V8 |
| Resource closure | Resources + Native | Aggregate domains, native backing/control budgets, host/native residency and total retained memory | Charged bootstrap/terminal headroom, compound pre-effect admission and no double charging |
| Qualification | Primary + all lanes | Incremental adapter proofs, native depth/overlap/fault tests, matched HIP/HSA workloads | Separate authenticated proof, hardware and performance results; shared-machine scheduling |

The three workers reviewed these assignments independently. Their bounded review
turns are complete; queued implementation packets are not running unattended.
Later A3-A7 and compiler/Worker/release handoffs remain below, outside A1/A2.

### Historical R104 Dispatch

The immediate handoffs and wave table in this subsection record the R104
planning state. R105 has since accepted N2, and R106 has accepted N3-C locally. Use
[Renewed Swarm Dispatch](#renewed-swarm-dispatch) for current assignments and
dependencies; the historical rows below do not queue N2 again.

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

The child test module already has the required private access. Record structs
lack `PartialEq`; project submission, event, stream, allocation, module and
kernel records into sorted keyed field tuples rather than changing production
derives. Include Context-local identity state, backend handle sets, capacities,
terminal/graph flags, mock memory bytes/storage, cleanup history, configuration
and every entry counter. Do not snapshot the process-global Context generation
allocator, which other tests may legitimately advance. Existing accounting
usage/retained-record queries do not expose individual credit-owner identities.

Callback boxes are non-Clone. Snapshot each keyed vector's length, capacity,
buffer pointer and ordered callback data pointers, with non-ZST captured
invocation/drop probes. Rejected registration drops its newly supplied callback
once without invoking it; only pre-existing callbacks must remain unchanged.
Match `RuntimeErrorV1::Validation(UnknownSubmission)` directly instead of
comparing whole errors; pure `query_submission` returns the bare
`RuntimeValidationErrorV1`. Consuming release must restore the token returned by
`into_parts()` before checking every supplied field. No visibility widening,
waker accessor or production validator change is required for C1.

### CO-2B And CO-3 Follow-On Packets

CO-2B owns a new `authorized_execution/tests/generated_identity.rs`, with Primary
wiring `authorized_execution.rs`. The [reviewed four-test handoff](runtime-swarm-next-packets.md#c2-reviewed-test-handoff)
covers every roster coordinate in both match directions and source matching,
immutable validation after one-shot control transfer, post-transfer artifact
substitution, and post-transfer authority/currentness rejection.
Mutate count, readback bytes, fixup count, dispatch hash, each occupied slot's
ordinal/extent/access, absent slots and unexpected trailing slots independently.
Preserve original source buffers and transferred control. Reuse existing
pre-transfer/substitution tests; no production validator change is indicated.
These are descriptive checks, not native publication/allocation receipts.

Post-transfer validation must use the immutable
`RuntimeGfx942GeneratedSourceV1::from_generated_storage` view. The mutable view
intentionally rejects after control transfer and would mask the intended
artifact/authority/currentness errors. Use fresh fixtures and real transfer for
each substitution; retain the destination, prove control never returns, and
validate the original immutable view again after rejection. Descriptor copies
remain test-local and preserve `Arc::ptr_eq` source identity; no production
`Clone` or visibility change is needed. Snapshot accessible packet metadata,
not private packet bindings that the test cannot inspect.

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
2. **VER-1A.2 -> .3: proofs and production journal.** R108/R113/R116 locally
   accept the executable issuance, membership and settlement models. Production
   implementation can proceed against these stable contracts with preallocated
   Context metadata and move-only tickets. Verified V5 acceptance requires V4
   property proofs and Q1 correspondence; the standalone .2 remains model-only.
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

### V3 Settlement Handoff

The [V3 model contract](runtime-context-version-settlement-v1.md) was frozen on
2026-09-14 and is locally accepted as R116 at the executable-model boundary.
It specifies whole-writer success, exact NoEffect and sticky
Unknown operations over V2's retained chain. Evidence projections bind the full
writer reference, including kind; callers cannot supply a destination subset.
These are inert model projections, not authenticated production receipts.

Implementation must preserve the frozen writer/phase and evidence precedence, bounded
count/head validation, full member/reference/backlink/epoch/lineage checks, and
required free-stack/scratch headroom. Validate every touched cell before writing
scratch or committing. Strictly increasing allocation keys plus exact count and
terminating-link checks detect retained-chain cycles without a visited set.
This does not detect unrelated orphaned cells; preservation invariants and the
full test auditor remain necessary.

Success advances lineage to the admitted attempt epoch; NoEffect preserves prior
lineage. Both preserve attempt epochs and registration watermark, clear exact
backlinks and return only that writer/member capacity without changing Reserved
count. Unknown retains the entire chain and blocks ordinary settlement. Empty
Pending settlement returns only writer capacity; empty Unknown retains it.
Reuse existing arenas and scratch, with O(k) touched work and no commit growth.

Repeated Unknown revalidates without mutation; inert evidence is borrowed and
unchanged on rejection. Exact writer/phase rejection precedes evidence mismatch,
then retained-chain corruption and release-only headroom checks. Corrupted
retained allocation references map to InvalidState. Production sealing on
InvalidState remains a separate V5 obligation, not model authority. Accepted tests cover malformed first/middle/last members,
stale evidence after reuse, NoEffect's burned epochs, out-of-order valid writers,
Unknown retention and fixed-k work across larger unrelated arenas. V4 proofs,
V5 Context producers and V7 ordered writers/recovery remain separate work.

### VER-1A.1 Contract Boundary

The reviewed [contract and inventory](runtime-context-version-journal-v1.md)
is the input to VER-1A.2, not another drafting assignment. It does not implement
the executable model, Context journal, hooks or cross-run leases.

After V1 issuance acceptance, Resources' next model handoff is an allocation-free
whole-roster transition planner consumed by the eventual journal. Validate the complete canonical input
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

R108's accepted .2a tests cover independent A/W boundaries; Context/local zero and maximum values;
registration 41 then 44 with both still retrievable; unregistered 42 rejection;
foreign Context/kind/slot and stale-reference rejection; abort followed by slot
reuse; and dropped-reference capacity retention. Rejection preserves a complete
state snapshot. A full test-only partition/unique-key auditor and fixed-work
counters check invariants without adding arena scans to operations. Compiled
negatives should reject a lookup watermark check, abort watermark rollback,
omitted exact-key comparison and watermark mutation before capacity rejection.
These checks have named executable-model acceptance; they are not authenticated
proof or production-journal correspondence.

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
| Threaded release closure | [#277](https://github.com/harsh-nod/fe2o3/issues/277): resolve the strict ELF rejection of the GNU threaded VecAdd benchmark without weakening policy, add release CI coverage and separately investigate the observed teardown failure; functional kernel checks do not establish full-smoke or release qualification |
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
