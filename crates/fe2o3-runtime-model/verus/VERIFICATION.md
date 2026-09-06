# Runtime model verification

This directory contains the issue #137 Verus specifications and the additive R7
asynchronous-resource, R8 execution-contract, R9 native-evidence, R10 closed
execution-composition, R11 runtime-semantics, R12 native-concurrency, R13
logical-scheduler, R14 async-observer, R16 Worker V5 semantic-boundary, R17
persistent-native-allocation, R18 persistent-local-SDMA-adapter, R19
directional-persistent-local-SDMA-adapter, R20 runtime-facade directional
chunking, R21 runtime scripted-failure-seam, R22 batched directional
persistent-SDMA-window, R23 same-device D2D persistent-SDMA-window, R24
portable-progress, R25 persistent-compute storage-bridge, R27 persistent
dispatch-control, R28 persistent-hot-currentness-scope, R30 bound
host-content-certificate, R31 single-packet/window-refinement, R32
directional-currentness-handoff, R33 fused-synchronous-directional-SDMA, R34
fused-asynchronous-directional-SDMA, R35 fused-retained-control-replay, R36
fused-completion-poll/recycle, R37 typed-native-SDMA-wait-activation, R38
bounded-persistent-compute-wait/recycle, and R39 scoped-persistent-SDMA-wait
policy models. The authenticated runner proves 877 obligations and rejects 349
expected-negative mutations over finite abstract
values and traces. The
materialization input and image sequences are
capped at 64 MiB and its phase trace has exactly four entries. The
lifecycle-history sequence lengths are not bounded by these proofs.

`runtime_lifecycle_v1.rs` proves:

1. a retaining dispatch is bound to the exact VM, physical-device identity,
   and device generation carried by its referenced mapping; and
2. releasing a mapping preserves the runtime invariant when no prepared,
   published, or ambiguous dispatch retains that mapping.

`device_identity_generation_v1.rs` proves:

1. registering a fresh device generation preserves unique active generations;
2. registering a VM preserves its exact active device-generation binding;
3. an active VM cannot be substituted onto another generation of the same
   physical device; and
4. while a current generation is active, that generation or an older one
   cannot be reused as a fresh admission.

`device_projection_refinement_v1.rs` proves the pure boundary introduced for
the executable adapter:

1. the model projection retains every field represented in the formal
   canonical observation, including the literal V1 profile and UAPI-schema
   identities, initial wrapping VRAM-loss counter, and contracted reset-fence
   facts;
2. a canonical record satisfying the explicitly modeled V1 predicates projects
   to a model value satisfying the same exact profile/schema identities and
   contracted currentness facts;
3. the projection preserves the explicit 1-through-16-entry topology
   inventory, its pairwise physical/KFD/render/PCI identity uniqueness, and the
   unique selected-device match without replacing the inventory with an opaque
   hash;
4. appending a later generation preserves its exact predecessor link and the
   single-physical-device history invariant.

`memory_lifecycle_v1.rs` proves the initial R2 pure memory obligations:

1. a mapping retains the exact VM, physical-device generation, allocation ID,
   allocation generation, opaque-handle observation, and canonical bounded
   device set represented in the formal binding;
2. a failed map records exactly the reported successful device prefix;
3. a failed unmap treats `n_success` as an absolute cumulative prefix, assigns
   that value without adding prior progress, and retains the unreported suffix;
4. a failed unmap reporting the full prefix remains ambiguous and retains its
   prior conservative range;
5. a substituted device set produces no map state; and
6. any non-released mapping or live publication blocks allocation free.

`queue_lifecycle_v1.rs` proves the initial R4 compute-AQL queue obligations:

1. one concrete canonical four-resource plan exists, preventing the resource
   predicates from being accepted only vacuously;
2. ring, control, end-of-pipe, and context-save resources retain their exact
   composite VM, allocation ID, allocation generation, mapping ID, and
   publication identities and are pairwise distinct;
3. a successful create transition retains the exact plan, queue generation,
   resource sequence, configuration, and opaque returned queue ID, including
   queue ID zero;
4. CREATE status, unchanged/caller-labeled sentinel, returned `u32::MAX`, and
   known-ID collision conditions select Active versus fail-closed Ambiguous
   outcomes exactly; indeterminate CREATE retains a classified returned ID;
5. indeterminate update, disable, and destroy observations become Ambiguous only
   from their matching legal pending phases and retain the exact resources,
   configuration, and prior queue ID; CREATE is excluded from this generic
   relation and is covered only by the field/status-aware relation above;
6. CancelledBeforeCreate and Destroyed are exactly the two non-retaining
   terminals, reached by plan cancellation and successful destroy respectively;
7. direct destroy begins only from Active or Disabled, retains that exact source
   in the pending record, and a failed-no-effect observation restores the same
   source while success reaches Destroyed;
8. generic memory release cannot discharge a live publication structurally
   owned by an exact queue VM, instance, and generation;
9. every retaining queue blocks release of each exact composite mapping in its
   plan;
10. an ambiguous known queue ID remains reserved, while CreatePending or any
   number of Ambiguous states with no known ID poison process-level future
   CREATE; only CreatePending itself is globally single-flight; and
11. appending a history event preserves the exact prior sequence as a prefix
    and places the new event at the next index.

`load_plan_v1.rs` proves the initial R3 abstract load-plan relation:

1. every admitted segment retains the exact 4 KiB page-rounding equations,
   checked `u64` file, memory, and mapping ranges, containing mapping range,
   and the plan retains a checked image span no larger than 64 MiB;
2. the three segments are in canonical increasing virtual-address order, are
   pairwise disjoint in file, memory, and page-rounded mapping ranges, and have
   exactly one each of read-only, read-execute, and read-write permissions; and
3. an admitted descriptor has the same file-to-virtual-address delta within
   exactly one same-permission containing `PT_LOAD` segment.

`materialization_v1.rs` proves the next R3 abstract materialization operation:

1. the three canonical source and destination ranges use checked offset/end
   arithmetic, remain within the 64 MiB input and image bounds, and have
   pairwise-disjoint destinations;
2. the deterministic full-zero transition creates an image of the requested
   length with every byte equal to zero;
3. the deterministic copy-range transition writes the corresponding exact
   source byte at every destination index and preserves every byte outside the
   destination range;
4. for every canonical bounded three-segment plan and exact-length input, the
   defined zero/copy-first/copy-second/copy-third execution has all four states
   at the exact image length and its final byte at every index follows the
   corresponding deterministic transition;
5. the completed execution therefore places every byte from each of the three
   exact input source ranges at its checked destination;
6. every checked range disjoint from all three copy destinations remains zero;
   and
7. the modeled mapping prefixes, in-memory suffixes including BSS, modeled
   mapping tails, and inter-segment gaps satisfy that derived zero-preservation
   property; and
8. one concrete canonical three-segment plan is inhabited, and its constructed
   final image contains both a nonzero copied byte and an uncopied zero byte.

The materialization model receives already-formed mapping ranges. No theorem in
this file composes `MaterializationPlanV1` with
`load_plan_v1::canonical_load_plan_v1`, imports the separate load-plan
invariants, or proves that its mapping starts and sizes use 4096-byte rounding.
The 4 KiB and page-rounded properties listed above remain claims of the separate
`load_plan_v1.rs` proof only.

`aql_publication_v1.rs` proves the initial R4 abstract independent-header
packet-publication, single-producer reservation, and completion-signal
relations. Its release-word theorem is scoped to `0x1402`; it does not cover
the separately admitted wait-for-prior `0x1502` executable header:

1. copying one canonical 64-byte INVALID packet body into a checked bounded ring
   slot writes every exact source byte to the corresponding destination and
   preserves every ring byte outside that slot;
2. the modeled release-`u32` transition produces the exact little-endian
   system-scoped kernel-dispatch header, preserves both copied setup bytes, and
   preserves every byte outside the four-byte publication word;
3. composing those two transitions records an INVALID body before the final
   invariant header, retains the source setup dimension, and preserves the exact
   source and destination frame;
4. every accepted reservation advances the write counter exactly once, retains
   capacity, records the exact prior packet ID and modulo slot, and preserves the
   canonical bounded no-wrap state;
5. two accepted transitions linked through the exact intermediate state have
   distinct consecutive packet IDs and advance the initial write counter by two;
6. an accepted read observation is nondecreasing from the prior observation;
7. a full ring produces the exact Full rejection and no reservation;
8. every rejection branch returns the exact input state unchanged;
9. the pending completion-signal image is exactly 64 bytes, has little-endian
   USER kind one at byte zero and pending value one at byte eight, and has zero
   in every other byte;
10. classification of every supplied `i64` maps one to Pending, zero to
    Completed, and preserves every other value as Unexpected; and
11. concrete packet, ring, signal-image, classification, two-reservation, and
   full-ring witnesses inhabit the predicates, including a nonzero copied byte
   and a preserved byte outside the
   destination frame.

The AQL model operates only on mathematical byte sequences, integer and
natural-number counter states, and explicit successor relations. `release-u32`
names one abstract state transition; it is not evidence of a CPU atomic
operation, ordering, coherence, device visibility, or firmware consumption. No
theorem imports or refines `fe2o3-aql`, its byte encoder, initializer,
classifier, callback trait, or executable Rust ring model, native queue memory,
a doorbell, or an observed hardware read pointer.
The classifier's acquired-value name is only an input label; the proof performs
no load and establishes neither atomic object initialization nor CPU/GPU
visibility.

The two-step theorem proves linearity only along one supplied mathematical
successor chain. It does not establish uniqueness among independently
constructed Rust values, counter truth, completion, liveness, or performance.

`r7_async_resources_v1.rs` proves eight abstract asynchronous-resource,
memory-pool, and multi-device obligations:

1. leasing an eligible free block retains its exact pool, device, block, and
   generation coordinates and makes the block non-reusable;
2. submitting an exact leased block moves it to in-flight custody without
   changing its storage identity;
3. exact completion followed by release advances the lease generation and
   prevents the old lease from matching the free block;
4. a stale generation cannot submit a reused block;
5. two retained blocks in a valid roster cannot name the same storage;
6. an admitted peer copy retains its exact source and destination generations
   and executes on the destination device coordinate;
7. an incomplete dependency frontier leaves a reserved peer copy unpublished;
   and
8. quarantined storage is retained and never reusable.

These theorems do not refine `memory_pool.rs`, the KFD SDMA packet encoder,
queue counters, mapped atomics, the multi-device router, or any native call.
They do not prove that a hardware completion occurred or that a pool operation
is lock-free, wait-free, or performant.

`r8_execution_contracts_v1.rs` proves ten abstract scheduling, atomic, and
collective obligations:

1. reserving a copy leaves it unpublished and retains the destination epoch;
2. an incomplete dependency frontier prevents publication;
3. ready publication retains the exact operation, resource-generation, and
   destination-device binding;
4. two admitted overlapping operations have no read/write or write/write
   resource conflict;
5. a valid abstract atomic location has a supported 32- or 64-bit width and an
   offset aligned to that width;
6. an abstract aligned 32- or 64-bit fetch-add linearization retains its exact
   resource, generation, order, and scope, returns the old value, and records
   the mathematical sum as the new value;
7. a non-final, previously unseen collective member arrival remains in the
   gathering phase and cannot be published;
8. the final unique-member arrival makes the collective ready but does not
   publish it;
9. publishing a ready collective retains its device and epoch and requires the
   complete membership set; and
10. a duplicate member arrival does not advance the collective.

The R8 file models mathematical operations and values. It neither implements
wrapping device-integer arithmetic nor refines the executable runtime or KFD
adapter. In particular, its `destination_epoch` is a retained abstract field,
not a proof that executable submission performed no memory access. Its
copy type models whole, distinct resources without byte ranges or a physical
alias relation. Same-resource ranged copies accepted by an executable backend
are outside its domain. Its conflict predicate is a scheduling policy, not a
proof of concurrent hardware execution. Its atomic order, scope, and coherence
fields are retained labels; the proof performs no CPU or GPU atomic operation.
Its collective membership set is bounded to 256 positive participant
identities, but proves no wave convergence, LDS behavior, numerical reduction,
firmware progress, or cross-device communication.

`r9_native_evidence_v1.rs` proves fourteen abstract mapping, XGMI-route, and
machine-evidence obligations:

1. a canonical one-through-64-entry KFD GPU-ID sequence is positive, strictly
   ordered, and duplicate-free;
2. mapping begins with the exact canonical device array and zero map/unmap
   prefixes;
3. a failed MAP outcome retains exactly the absolute cumulative successful
   prefix reported for the full KFD GPU-ID roster;
4. an admitted partial successful-compensation transition retains the absolute
   cumulative prefix and remains unreleasable;
5. the abstract successful-compensation transition releases only after its
   absolute cumulative prefix equals the exact mapped prefix;
6. an admitted XGMI route retains its exact source/destination device and
   generation, topology generation, hive, directional IO-link type/index,
   bandwidth, engine mask, selected engine, and reset-currentness coordinates;
7. reversing the source and destination cannot satisfy directional route
   currentness;
8. a stale topology generation blocks XGMI route admission;
9. matching machine evidence retains the exact artifact, gfx942 COV6 target,
   symbol, descriptor, machine-code digest, checked instruction-class receipt,
   declared semantic-contract identity, and kernel identity;
10. substituting the checked instruction-class receipt rejects the evidence
    binding;
11. any stale device, code, mapping, queue, or reset-fence coordinate blocks
    dispatch; and
12. an evidence-bound dispatch publishes only when every exact binding is
    current and its dependency frontier is complete;
13. a native XGMI copy publishes only with two fully active mapping owners, an
    exact current directional route and selected engine, and distinct
    nonoverlapping source/destination ranges; and
14. timeout or indeterminate XGMI completion quarantines the copy and retains
    both exact mapping owners.

The selected native route is one retained directional KFD `io_links` record.
KFD `p2p_links` records remain part of the topology snapshot but are not a
selected coordinate of the current `Gfx942XgmiRouteV1`, so the R9 route model
does not invent a P2P-link ID. The executable model additionally checks that
the recommended engine mask contains exactly one bit and that its index is the
selected gfx942 XGMI SDMA engine. The Verus model retains the exact mask and
engine coordinates but does not prove bit-level mask decoding.

## R9 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Abstract canonical mapping, successful compensation, route/copy currentness and custody, evidence equality, and dispatch gating | **Proved** | The fourteen theorems in `r9_native_evidence_v1.rs`; mathematical values only. The compensation theorem has no ioctl result-status input and does not prove errno-at-full-prefix handling. |
| Executable `r9_native_evidence.rs` state machines and rejection tests | **Checked** | Rust tests cover absolute cumulative prefixes, including a retry after prior progress, failed-full-prefix quarantine, canonical arrays, directional topology, exact evidence equality, and stale dispatch rejection. There is no Rust-to-Verus refinement theorem. |
| Instruction-class receipt | **Checked** | The model binds the digest of a separately checked decoder/classifier receipt. It does not prove decoded instructions implement the declared atomic or collective semantics. |
| KFD topology, `n_success`, VM mapping ownership, queue selection, reset observations, firmware, and coherence | **Contracted** | A native adapter must authenticate observations, preserve owners, and fail closed on uncertain side effects. |
| Compiler preservation and machine-code semantics | **Not established** | Artifact and receipt identities are bound exactly, but no theorem relates Rust/device-language semantics through LLVM to decoded gfx942 instructions. This is not machine-semantic refinement. |
| Native XGMI transfer, atomic/collective results, progress, and performance | **Measured** | Exact-commit hardware qualification is evidence for the tested machine only; it is not a proof. |

The native-copy theorem is not a refinement of the concrete SDMA queue state
machine. In particular, a concrete batch reobserves full directional topology
at its scope edges and checks both process/reset fences per operation, but does
not rediscover sysfs topology for every packet. A concrete wait timeout may
retain its exact ticket for retry instead of entering the abstract
`Quarantined` phase, while a completion followed by failed currentness retains
both mappings in a distinct completed-but-indeterminate result. The common
proved safety property is custody: neither an uncertain abstract completion
nor these concrete failure results authorize early mapping release. Per-packet
topology currentness, the concrete ticket/record correspondence, and those
concrete phase distinctions require a future Rust-to-Verus refinement.

`r10_closed_execution_v1.rs` proves twenty abstract closed-composition
obligations:

1. an incomplete dependency, including one produced on another stream, blocks
   publication;
2. a ready operation publishes with its exact stream, execution device, kind,
   dependencies, leases, batch identity, and publication epoch;
3. two distinct operations with disjoint lease sets can remain published
   simultaneously;
4. an unready prepared batch has no partial-publication transition;
5. a ready batch publishes its complete exact roster at one batch identity and
   publication epoch;
6. completed pool-block release preserves storage identity, advances its
   generation, and only then returns it to the free phase;
7. a stale lease cannot equal a later generation of the same pool block;
8. a peer copy has two distinct device owners and executes on its destination
   device coordinate;
9. post-publication cancellation and timeout retain the exact lease set and
   published phase;
10. indeterminate failure quarantines the operation, retains its leases, and
    does not make it releasable;
11. corresponding atomic load, store, and RMW records separately retain the
    declared operation, order, scope, required fence plan, and abstract value
    relation;
12. a substituted observed atomic scope cannot satisfy correspondence;
13. a divergent or incomplete Wave64 arrival cannot publish; and
14. converged Wave64 barrier, reduction, inclusive-scan, and exclusive-scan
    records publish the exact mathematical input, total, or prefix sequence.

The numbering above groups related theorems; the verifier reports twenty proof
obligations. The executable `closed_execution.rs` model is bounded to 64
streams, 64 pools, 4,096 blocks, 4,096 operations, 64 leases per operation, and
256 dependencies per operation. Its Rust tests check simultaneous compute
operations, cross-stream dependency gating, failure-atomic prepared batches,
generation-safe allocation reuse, exact two-device peer-copy custody,
cancellation/timeout/quarantine retention, atomic label/fence/value rejection,
and Wave64 wrapping reduction/scan results.

## R10 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Abstract closed operation, batch, pool, peer-owner, atomic-record, and Wave64 relations | **Proved** | Twenty theorems in `r10_closed_execution_v1.rs`; finite mathematical values only. |
| Executable `closed_execution.rs` transitions and negative tests | **Checked** | Rust unit tests exercise the bounded state machine and executable wrapping arithmetic. |
| Executable Rust to Verus correspondence | **Not established** | The Rust and Verus models intentionally use parallel structures, but there is no refinement theorem connecting their implementations. |
| Atomic instruction decoding and compiler preservation | **Not established** | Atomic observed-operation/order/scope/fence fields are caller-supplied abstract observations, not decoded-instruction or compiler certificates. |
| Wave convergence, barrier behavior, LDS, and machine arithmetic | **Contracted** | The model checks exact 64-lane arrival and computes host-side results; it does not establish GPU control-flow convergence or ISA behavior. |
| KFD/HSA/HIP queues, mappings, coherence, completion, progress, and performance | **Contracted or measured** | Native adapters and hardware qualification must bind real resources and observations to the model. No model value grants native authority. |

`r11_runtime_semantics_v1.rs` proves eighteen abstract facade-semantics
obligations:

1. a recorded event aliases its pending source submission;
2. a conclusive observation becomes the shared event/submission status;
3. first completion discharges every registered callback with the exact status;
4. a repeated observation cannot discharge callbacks again;
5. a live event prevents source-submission release;
6. an exact atomic operation/scope/order/geometry contract with both capability
   layers is admitted, including a base-valid partial grid;
7. legal compare-exchange success/failure order and weak-mode contracts are
   admitted;
8. an illegal compare-exchange failure order is rejected;
9. compare-exchange-only failure-order or weak controls reject on non-CAS
   operations;
10. any substituted atomic contract field is rejected;
11. a missing execution-detail atomic capability fails closed;
12. a workgroup collective admits its exact geometry-derived membership;
13. a collective membership mismatch is rejected;
14. a collective grid with a partial tail workgroup is rejected;
15. a single-stream system collective is rejected;
16. an active persistent mapping is retained by its exact batch;
17. conclusive batch completion restores the same persistent mapping identity;
    and
18. indeterminate completion quarantines the mapping and blocks release.

The executable `r11_runtime_semantics.rs` model additionally checks bounded
event/source lookup, terminal callback registration, event-gated release,
capability rejection, collective geometry validation, repeated persistent-map
reuse, atomic partial-grid admission, the complete compare-exchange order
lattice in weak and strong modes, non-CAS control rejection, and all-mapping
quarantine. Complete workgroups are a collective-only requirement. It performs
no callback, backend call, mapping syscall, atomic operation, collective, or
device execution.

## R11 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Abstract shared completion, callback discharge, launch admission, and persistent-map custody | **Proved** | Eighteen theorems in `r11_runtime_semantics_v1.rs`; mathematical values only. Atomic admission includes compare-exchange failure-order and weak-mode legality. Collective geometry requires every grid dimension to contain at least one workgroup and divide exactly by its workgroup dimension. |
| Executable bounded R11 model | **Checked** | Rust unit tests exercise independently implemented model transitions and rejection cases. |
| Executable Rust to Verus correspondence | **Not established** | No theorem connects `src/r11_runtime_semantics.rs` or the runtime facade to the Verus structures. |
| Native atomic, collective, callback, event, and persistent mapping behavior | **Not established** | Concrete adapters must authenticate capability and completion observations and retain native custody. |
| Compiler, ISA, hardware progress, and performance | **Not established** | No R11 model value is execution authority or hardware evidence. |

`r12_native_concurrency_v1.rs` proves twenty-three abstract multi-queue custody
obligations:

1. only an exact, stable device capability supporting at least two requested
   compute queues admits the requested queue and slot counts;
2. single-queue, unstable, and over-capacity requests are rejected;
3. queue occurrences and slot generations must match exactly;
4. an unready dependency prevents publication, while every dependency identity
   must resolve in the modeled submission sequence to a successful terminal
   producer before publication;
5. a ready consumer publishes without releasing slot or resource custody, and
   a failed or otherwise non-successful producer leaves it reserved;
6. an exact terminal observation retains custody until explicit release;
7. an event for another slot or queue cannot complete a submission, while
   terminal events for distinct slots may be observed out of order;
8. cancellation before publication relinquishes custody, advances the live
   slot generation exactly once, and permits drain;
9. cancellation or release after publication does not relinquish custody;
10. a reserved dependent found by scanning the modeled submission sequence
    blocks producer release; otherwise terminal release relinquishes custody,
    advances the live slot generation, and permits drain;
11. currentness loss cancels reserved work with the same generation advance but
    converts published work to resource-retaining quarantine;
12. indeterminate custody cannot be released or drained;
13. drain is bound to the exact queue occurrence; and
14. recreating a drained current queue preserves its numeric queue ID while
    advancing its occurrence exactly once.

The numbering groups related theorems; the verifier reports twenty-three proof
obligations. The executable `r12_native_concurrency.rs` model is bounded to 16
queue occurrences, 64 slots per queue, 4,096 submissions, 256 dependencies per
submission, 64 resources per submission, and 8,192 registered resources. Its
Rust tests cover exact capability binding, out-of-order completion, cross-queue
and stale-generation rejection, dependency gating, failed dependencies,
prepublication cancellation, post-publication retention, currentness-loss
quarantine, occurrence-bound drain, and drained-queue occurrence advance.

## R12 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Abstract multi-queue admission, dependency, terminal-event, cancellation, release, currentness, drain, and queue-recreation relations | **Proved** | Twenty-three theorems in `r12_native_concurrency_v1.rs`; finite mathematical state only. Dependency readiness and reserved-dependent retention are derived by scanning abstract submission sequences and matching submission identities. |
| Executable bounded R12 model | **Checked** | Rust unit tests exercise an independently implemented finite state machine with exact occurrence and generation identities. |
| Executable Rust to Verus correspondence | **Not established** | No theorem connects `src/r12_native_concurrency.rs` to the Verus structures or transitions. |
| Native KFD multi-queue creation, scheduling, completion, reset, and quiescence | **Not established** | A native adapter must authenticate capabilities, queue occurrences, resources, terminal observations, and currentness. Model values grant no native authority. |
| HSA/HIP parity, progress, fairness, overlap, and performance | **Not established** | These are implementation and matched-hardware measurement obligations, not consequences of the R12 model. |

`r13_logical_scheduler_v1.rs` proves twenty abstract logical-stream scheduler
obligations:

1. a valid scheduler state contains between one and 64 distinct per-stream
   head/tail records while exposing exactly two physical lanes, and three or
   more represented logical streams therefore exceed the physical-lane count;
2. valid submission dependency count and depth remain bounded at 32, and a
   represented stream predecessor is included in the effective dependency set;
3. only the head of the exact registered logical-stream record may publish,
   including when compute and copy operation classes alternate in one FIFO;
4. an incomplete dependency frontier or any overlap with the aggregate
   retained resources of active lanes blocks publication;
5. a ready head deterministically leases one free lane numbered zero or one,
   takes resource custody, and preserves distinct lane owners;
6. a terminal observation must name the submission's exact leased lane and that
   lane must still name the submission as owner; the transition returns that
   lane and retains resources until release;
7. cancellation applies only to an unpublished tail and restores its exact
   predecessor as the prior tail; non-tail and published cancellation are
   no-ops;
8. a queued dependent, including a represented implicit stream successor,
   retains its terminal producer's resources, while an unreferenced terminal
   may release them; and
9. currentness loss cancels queued work and converts published or terminal
   custody into unreleasable quarantine, retaining the submission's recorded
   lane coordinate.

The numbering groups related theorems; Verus reports twenty proof
obligations. The independent executable `r13_logical_scheduler.rs` model is
bounded to 64 logical streams, exactly two lanes, 4,096 submissions, 32
dependencies and dependency depth per submission, 64 resources per submission,
and 8,192 registered resources. Its effective dependency set includes the
implicit same-stream predecessor, so predecessor failure, terminal custody, and
depth accounting use the same bounded edge as explicit dependencies. Active
resource conflict ownership is separate from lifetime retention: an ordered
successor can take active ownership after terminal producer observation while
older terminal submissions retain the resource identity. Lifetime retainers do
not act as active-conflict owners, and releasing one retainer does not discard
the others. Its Rust tests exercise more logical streams than lanes,
mixed compute/copy FIFO order, successful, failed, and implicit dependencies,
dependency bounds, ordered same-resource progress, active and terminal resource
conflicts, lane-bound terminal events, tail restoration, dependent release
retention, device-bound identities, and currentness quarantine.

## R13 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Abstract per-stream FIFO-record selection, dependency, resource-conflict, lane-lease, terminal, cancellation, release, and currentness relations | **Proved** | Twenty theorems in `r13_logical_scheduler_v1.rs`; finite mathematical state only. Active resource occupancy is supplied as an aggregate mathematical set. The model proves preservation properties of admitted states, not a stream-registration transition. |
| Executable bounded R13 model | **Checked** | Rust unit tests exercise an independently implemented finite state machine with caller-constructible identities and observations. |
| Executable Rust to Verus correspondence | **Not established** | No theorem connects `src/r13_logical_scheduler.rs` to the Verus structures or transitions. |
| Native KFD scheduler, AQL/SDMA publication, completion, currentness, or quiescence | **Not established** | A sealed adapter must authenticate every concrete transition before the model can support runtime authority. |
| Fairness, progress, arbitrary stream counts, HSA/HIP parity, or performance | **Not established** | The executable model is bounded, the proofs are safety-only, and no matched-hardware result follows from them. |

Run the proofs and all expected-negative mutations with the exact Verus
release whose executable, complete release closure, version, proof sources,
source checker, transcript, and mutations are pinned under `verus/pins`:

```sh
VERUS=/absolute/path/to/verus \
  crates/fe2o3-runtime-model/verus/verify-verus.sh
```

`scripts/ci-local.sh verus` invokes the same authenticated runner. The
`runtime-model-verus.yml` pull-request workflow downloads the named release and
then relies on this runner's executable and complete-closure pins before any
proof result is accepted.

The mutations must fail at their named postconditions: release while retained,
VM generation substitution, stale generation reuse, topology/render PCI
substitution, dropped DRM schema identity, lost history predecessor, mixed
cross-source identity, a dropped final reset-fence observation, allocation free
while a partial mapping remains, cumulative unmap progress incorrectly added to
prior progress, a failed full-prefix unmap treated as releasable, a queue
resource substituted across roles, an indeterminate queue destroy treated as
releasable, queue history overwritten instead of appended, a caller-labeled
returned CREATE sentinel accepted as a queue ID, load segments whose memory
bytes are disjoint but whose rounded pages overlap, generic release ignoring a
queue publication owner, ambiguous queue-ID reuse, allocation-generation
substitution in a mapping identity, ambiguity entered from a non-pending
operation, generic indeterminate handling incorrectly admitting CREATE, a
second CREATE beginning while the first ID is unresolved,
cancellation incorrectly retaining resources, descriptor
containment that substitutes a different file-to-virtual-address delta, the
production-shaped copy transition substituting another source byte, and the
production-shaped zero-first transition omitting the first zero byte, an INVALID
packet body changed to vendor type zero, setup substitution during the modeled
release word, reservation replay without write advance, acceptance of a
regressed read observation, overwrite of a full ring, reuse of a released block
without advancing its generation, execution of a peer copy on its source
device, eager copy publication, conflicting overlap, returning a fetch-add's
new value instead of its old value, inverted dependency readiness, destination
binding substitution, resource-generation substitution, destination-epoch
substitution, omitted atomic alignment, dropped atomic coherence, early
collective publication, a duplicate collective member incorrectly advancing
the phase, a duplicate GPU ID in a canonical native mapping, nonzero initial
mapping progress, addition of an absolute MAP or UNMAP cumulative prefix to its
prior prefix, early or incomplete compensation,
a reversed or stale XGMI route, loss of its reset fence, artifact or checked
instruction-receipt substitution, stale dispatch evidence, publication with
incomplete dependencies, copy publication with an inactive mapping, and owner
release after uncertain XGMI completion; R10 dependency bypass, partial batch
publication, pool-generation reuse, peer-owner reversal, post-publication
cancellation release, quarantine release, atomic scope substitution, omitted
release fence, wrong RMW return value, early Wave64 publication, and an
inclusive-scan prefix off by one lane; R11 callback redischarge, event-status
substitution, omitted atomic execution capability, admission of a release
failure order for compare-exchange, collective membership mismatch, admission
of a partial tail workgroup, mapping release during a live batch, and mapping
release after indeterminate completion; R12 admission of a single compute
queue, queue-occurrence substitution, slot-generation substitution, dependency
bypass, cross-queue terminal observation, published cancellation or release
dropping custody, currentness loss dropping rather than quarantining published
custody, drain of indeterminate work, terminal release despite a reserved
dependent, slot recycle without generation advance, stale-occurrence drain, and
queue recreation without occurrence advance; R13 admission of a third physical
lane, a dependency count above its bound, non-head FIFO or dependency bypass,
publication despite an active resource overlap, duplicate lane ownership, a
terminal event from a foreign lane or same-numbered lane with a foreign owner,
non-tail cancellation, terminal release
despite a queued dependent, and currentness loss without quarantine; and R16
Worker V4 downgrade acceptance, an over-bound dependency count, semantic
variant mismatch, custody on malformed admission, semantic-contract
substitution, terminal reopening, collapsed worker/sidecar scope predicates,
and sidecar contract substitution.
R18 mutations additionally reject duplicate native authority, allocation-VM,
logical-queue, native-SDMA-queue, native-child-occurrence, attachment-generation,
host-generation, persistent-range, completion
range, completion-ticket, and completion-native substitutions; reversed D2H
engine or endpoint polarity; incomplete prepublication restoration; confirmed
publication from a stale ticket; retained ambiguity without quarantine; timeout
without ticket/native custody; native release while published; stale frontier
retirement; release with a retained settled frontier; and exhaustion after 65
sequential settle/retire cycles. The launcher
rejects source substitution, lexically audits all proof files for trusted
constructs, clears the environment, bounds execution time, pins Z3 through the
authenticated Verus release closure, and rechecks the authenticated inputs after
verification.

`r14_async_observer_v1.rs` proves ten abstract obligations for the bounded
event-observation layer: the waiter bound is 65,536; invalid, duplicate, and
over-capacity registration leave the waiter count unchanged; a pending
observation preserves the exact registration; terminal status and runtime error
outcomes are not substituted; abandonment and engine stop preserve submission
and event custody without cancellation or release; and event-key ordering is
lexicographic in context generation and event identity. Eight expected-negative
mutations demonstrate rejection when those properties are reversed. The
independent executable `src/r14_async_observer.rs` model additionally tests
stable ordering, out-of-order completion, immediate terminal registration, and
shutdown outcomes.

The R14 proof does not refine `src/r14_async_observer.rs` or the production Rust
async engine. It proves no property of threads, channels, locks, wakers, backend
polling, progress, fairness, latency, KFD, HSA, HIP, or hardware. In particular,
observation neither publishes deferred work nor provides asynchronous device
execution by itself; the runtime's declared progress operation remains a
separate obligation.

`r16_worker_semantic_boundary_v1.rs` verifies twenty-one abstract obligations for an
already-decoded Worker V5 semantic request and direct-KFD sidecar boundary:

1. only the exact V5 handshake opens the ready state;
2. every non-V5 handshake seals without attempting or accepting custody;
3. the exact atomic `63 + K + 29B + 8D` and collective
   `69 + K + 29B + 8D` frame formulas fit the 65 MiB frame cap when `K` is at
   most 1 MiB, `B` is at most 128, and `D` is at most 256;
4. every modeled geometry component has the production `u32` bound, workgroup
   multiplication has the `u32` bound, and collective participant products have
   the production `u64` bound;
5. composed binding/dependency admission is explicitly narrower than Worker
   wire validation and does not assign those checks to the codec;
6. malformed composed admission seals without increasing attempted or accepted
   custody;
7. a valid request starts one exact in-flight attempt without yet counting an
   accepted custody;
8. worker-wire and direct-KFD sidecar contract validity are distinct: the
   worker boundary permits System atomics and Device collectives, rejects
   System collectives, while the direct sidecar permits only non-System atomics
   and Workgroup collectives;
9. recoverable rejection and quiescence restore ready state without accepting
   custody or fabricating a success;
10. only a nonzero success accepts custody, exactly once, and preserves the
   in-flight contract and launch;
11. zero-handle success, terminal response, malformed response, timeout, and
    EOF seal with the request explicitly indeterminate and without fabricating
    accepted custody;
12. terminal state is absorbing for requests and responses;
13. the initial state satisfies a well-shaped reachable-state invariant;
14. negotiation preserves that invariant;
15. composed request admission preserves that invariant;
16. response classification preserves that invariant;
17. the handshake/request/nonzero-success composition preserves the exact
    contract, launch, counts, and invariant;
18. sidecar summaries use the exact V1 schema/version, nonzero encoded length
    bounded to 16 MiB, at most 16,384 records, exact typed/ordinary counts, and
    complete-history equality with the source runtime profile;
19. an exact sidecar sequence is an ordered, unique, per-index bijection between
    retained dispatch publications, producer observations, and records; and
20. any per-index substituted sidecar record fails that sequence join.

The recursive typed-record count contributes the twenty-first verified obligation
reported by Verus.

Ten R16 expected-negative mutations reject handshake downgrade, dependency and
pre-custody bound weakening, exact-contract or sidecar substitution, collapsed
Worker/direct-sidecar scope predicates, terminal reopening, variant mismatch,
recoverable-response custody fabrication, and unreachable terminal in-flight
state.

The independent `no_std` executable model in
`src/r16_worker_semantic_boundary.rs` has nine unit tests over the same abstract
surface. It checks concrete model vectors for binding-patch alignment, bounds,
zero placeholders, disjointness, nonempty checked regions, and unique
dependencies. In the Verus file those facts are supplied as decoded-summary
booleans; no theorem connects them to a byte parser.

R16 request admission is intentionally a composed pre-custody abstraction. It
does not assign each check to the production worker parser, dispatcher, or
backend/facade call site. Attempted requests and in-flight/indeterminate custody
are separate from accepted custody, which advances only after an abstract
nonzero-success response; none is a concrete backend-invocation counter. The
sidecar theorem covers ordered modeled fields but deliberately omits the
SHA-derived record and content identities. Neither the Verus specification nor
the executable
model refines production Rust, a parser or codec, serde/JSON, SHA-256 or content
identity, subprocess creation and teardown, channels, transport, authentication,
timeouts, kill/reap behavior, compiler or ISA lowering, KFD/native execution,
GPU atomics or collectives, hardware completion, liveness, or performance. The
handshake values are abstract labels, not a proof of wire-byte compatibility.

## R16 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Abstract V5 negotiation, bounded request, composed pre-custody admission, response classification, reachability, and exact ordered sidecar-join relations | **Proved** | Twenty-one verified obligations in `r16_worker_semantic_boundary_v1.rs`; decoded mathematical summaries only. |
| Independent bounded R16 executable model | **Checked** | Nine Rust unit tests exercise the separately implemented `no_std` finite model. |
| Executable model or production Rust to Verus correspondence | **Not established** | No theorem connects either Rust implementation to the Verus structures, predicates, or transitions. |
| Parser, serde, SHA, subprocess, transport, native KFD/GPU semantics, completion, liveness, or performance | **Not established** | These surfaces are explicitly outside the R16 model and require separate refinement or evidence. |

## R17 persistent native allocation

`r17_persistent_native_allocation_v1.rs` proves 32 obligations over one
abstract owner. Admission binds a nonzero, page-sized allocation of at most 256
MiB to exact R2-shaped allocation/mapping identities, an exact canonical pair
of device incarnations, and a private registry incarnation. The model fixes a
reusable 64-slot use ledger and 256-dependency input bound. Slot reuse advances
generation. A private registry incarnation rejects state-changing tokens and
dependency witnesses from reconstructed registries; numeric observation keys
and receipts remain non-authoritative and may coincide across reconstructions.

Compute and local-SDMA descriptors bind the allocation's exact home device and
VM plus a nonzero queue occurrence; local SDMA also binds one of two engines.
XGMI route-metadata classifications validate an R9-shaped current directional
route, the selected engine from the `[2,16)` R9 roster, both device identities,
and owner-relative source-read or destination-write access. They do not bind
the route to this R2 mapping and grant no mapping or publication authority.
Half-open ranges are nonempty, nonoverflowing, and within the admitted
allocation extent. Overlapping reads are compatible; overlap with a writer is
excluded unless an exact successful terminal predecessor is named as a ready
dependency. Unrelated terminal conflicts still block publication. Timeout and
exact terminal observations retain custody, reserved dependents block terminal
release, currentness loss cancels unpublished work and quarantines every
published state, and quarantined owners cannot release. A concrete mixed
compute/local-SDMA/XGMI-metadata witness prevents the class predicates from
being vacuous.

The independent executable model has 19 focused Rust unit tests plus
compile-fail examples for `Clone` and `Send`. Its private `Rc` incarnation is a
Rust type-system device, not a Verus refinement. Verus cannot prove Rust
auto-traits, OS-thread affinity, native allocation/mapping, queue publication,
KFD or SDMA behavior, currentness observation truth, GPU completion, liveness,
or performance. There is no executable-Rust-to-Verus correspondence theorem,
no two-registry atomic XGMI join, and no 1 GiB aggregate allocation claim.

## R17 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Bounded owner admission, registry/slot identities, class/range binding, conflicts, dependency gate, custody, quarantine, and release | **Proved** | Thirty-two verified obligations over mathematical summaries in `r17_persistent_native_allocation_v1.rs`. |
| Independent bounded executable lifecycle | **Checked** | Nineteen Rust tests plus compile-fail `Clone`/`Send` examples; no refinement theorem. |
| Rust auto-traits, OS thread, native memory, queue publication, KFD/SDMA/GPU execution, liveness, or performance | **Not established** | Explicitly outside the R17 model. |

## R18 persistent local SDMA adapter

`r18_persistent_local_sdma_adapter_v1.rs` proves 34 obligations over one
abstract persistent local device allocation, one ordinary host allocation, and
one targeted gfx942 SDMA operation. The binding retains the allocation, mapping,
VM and device generations; host session, allocation, generation, and coherent
visibility; persistent and host half-open ranges; copy extent; persistent-use
identity; logical parent queue occurrence; native KFD queue slot; engine; and
planned ticket slot and generation. The logical queue ID and native KFD queue
ID are separate fields.
Native KFD queue slot zero is admitted, while values at or above 1024 are
rejected.

The direction roster is exact: device-to-host uses ordinary SDMA engine zero
and binds the persistent allocation as a read source; host-to-device uses engine
one and binds it as a write destination. The model deliberately admits one
in-flight adapter use because its sole abstract native authority changes
location between the prepared request, queue record, persistent owner, and
quarantine custody. Recoverable prepublication failure restores the exact
binding and prepared lease. Confirmed publication requires the planned ticket.
An SDMA retained error moves directly from prepared custody to quarantine: it is
not called published, because mapped completion or ring writes may have failed
before write-pointer publication. Pending and timeout observations retain the
same ticket and queue custody. Preparation currentness loss permanently
quarantines without claiming a ticket. Completion requires the exact ticket,
native and host identities, ranges, and extent; an incomplete currentness
envelope permanently quarantines the queue-retained ticket instead of leaving
published custody resumable. Completion settlement creates an exact retained
frontier keyed by the native allocation and persistent-use slot/generation;
release remains blocked until that frontier is retired by an exact observation.
Stale or substituted frontier observations are atomic no-ops, and the bounded
reuse proof shows that exact retirement leaves no occupied slot after 65
sequential uses. Normal release remains blocked until cancellation or retired
settlement and complete owner quiescence, and every quarantine is absorbing.

Twenty-four expected-negative mutations cover the listed identity, direction,
custody, ambiguity, completion, frontier-retirement, reuse, and release
boundary classes. These mutations are small countermodels to the named mathematical
properties; they do not inject faults into production Rust.

R18 is a standalone summary, not a refinement proof. It does not connect its
types or transitions to either R17 model, the concrete persistent-allocation
ledger, the separately implemented R18 adapter, or `sdma.rs`. In particular,
the concrete persistent operation enum itself carries no queue/engine receipt,
and R17's local-SDMA class does not distinguish child native SDMA queues that
share a logical parent queue; the concrete adapter's separate attachment record
has no refinement theorem to this file.
The abstract authority count does not prove unique Rust ownership, move/drop or
panic/unwind behavior, auto-traits, OS-thread affinity, address validity, ioctl
truth, queue-record installation, mapped-write or release-fence ordering,
doorbell delivery, currentness observations, firmware DMA, GPU completion,
memory visibility, liveness, or performance.

## R18 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Abstract local allocation/host/range/direction/queue/ticket binding, single-location custody, and exact settled-frontier retirement | **Proved** | Thirty-four obligations in `r18_persistent_local_sdma_adapter_v1.rs`; one mathematical in-flight adapter use at a time plus a bounded sequential-reuse summary. |
| Identity, direction, restoration, ambiguity, timeout, completion, frontier, reuse, and release countermodels | **Rejected** | Twenty-four pinned expected-negative proof files fail only at their named postconditions. |
| R17, concrete persistent-ledger, or native SDMA refinement | **Not established** | No theorem links the abstract R18 values or transitions to executable Rust or KFD state. |
| Native ordering, DMA/completion truth, liveness, HIP/HSA parity, or performance | **Not established** | Requires a concrete sealed adapter, hardware execution evidence, and matched benchmarks. |

## R19 directional persistent local SDMA adapter

`r19_directional_persistent_local_sdma_adapter_v1.rs` proves 46 obligations
over one bounded allocation and one exact bidirectional child-queue pair. The
allocation binds a nonzero pool generation and distinct logical and physical
extents with `logical <= physical <= 256 MiB`; the physical extent is 4 KiB
page-rounded, while copy ranges are checked against the logical extent. Both
children share one logical parent queue and pair occurrence, have distinct
native queue IDs below 1024, and bind device-to-host to engine zero/read/source
and host-to-device to engine one/write/destination.

The adapter is single-flight across the pair. Direction is selected explicitly
for every submission and is not constrained by the prior direction: repeated
H2D/H2D, repeated D2H/D2H, and alternating sequences are all admitted. Exact
completion, restoration, settlement, and frontier retirement must finish
before the next submission. Retirement clears the R17 settled frontier; it
does not manufacture a post-retirement dependency token, and the next R17
reservation has no dependency. A recursive transition composes prepare,
publication, completion, restoration, settlement, and exact retirement before
the next recursive prepare. It derives 130 mixed-direction uses and 70 repeated
H2D uses with zero occupied slots and no pending frontier after retirement.

Confirmed, recoverable, and retained publication classifications remain
distinct. Pending and timeout retain the exact child ticket. The proof uses
bounded pair-global rotating ticket coordinates independent of the R17 use
generation; concrete child queues have independent planners and are outside
this proof. Succeeded and signed `i32` failed terminal statuses are exact
through restore and settlement. Preparation currentness ambiguity quarantines
without claiming a live ticket; ambiguity after possible native custody
quarantines with the exact ticket and clears the terminal status. Release,
rebind, and demotion require idle current custody and no pending frontier.
Demotion advances the pool generation; old frontiers cannot match a re-promoted
allocation. Exact frontiers bind the complete allocation and mapping identity,
parent and child pair, attachment, pool, direction, and adapter incarnation.
Executable Rust additionally binds transition tokens to a private `Rc`
incarnation.

The independent executable model has 23 focused Rust tests and compile-fail
examples for `Clone` and `Send`. Twenty pinned expected-negative mutations
cover child collision, physical extent, direction, repeated same-direction
admission, selected-child tickets, optional-ticket quarantine, timeout custody,
terminal status and restore currentness, early release, prepare-before-retire,
rebind/demotion busy gates, frontier allocation/pair/incarnation substitution
and actual recursive reuse, and isolated pool-generation ABA.

R19 is not a refinement proof of R17, R18, executable Rust, or KFD. The Verus
file proves mathematical summaries only; it does not establish Rust ownership,
native allocation or queue state, mapped-write ordering, doorbell delivery,
DMA/completion truth, liveness, HIP/HSA parity, or performance.

## R19 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Directional pair identity, extents, child roles, ticket custody, terminal status, currentness quarantine, stateful frontier/rebind/demotion gates, arbitrary sequential direction, pool-generation retirement, and bounded recursive reuse | **Proved** | Forty-six obligations in `r19_directional_persistent_local_sdma_adapter_v1.rs`. |
| Independent executable custody lifecycle | **Checked** | Twenty-three Rust tests plus compile-fail `Clone`/`Send`; it directly composes a private R17 registry but has no refinement theorem. |
| Boundary countermodels | **Rejected** | Twenty pinned expected-negative files fail only at their named postconditions. |
| Executable-Rust, R17/R18, or native KFD refinement; hardware behavior or performance | **Not established** | Explicitly outside the R19 proof boundary. |

## R20 runtime-facade directional chunking

`r20_runtime_facade_directional_chunking_v1.rs` proves 31 obligations for a
single-allocation, single-flight facade over abstract R19 packet custody. H2D
and D2H bind exact host/device storage roles; H2H and D2D are unsupported and
their preflight is mutation-free. Exact dependency identity and status gate
publication. Pending dependencies retain `Ready`; failed dependencies settle a
retained conclusive failure, while dependency quiescence settles an exact
quiescent-without-result record.

Every successful chunk binds its byte offset to prior completed progress, caps
length at `0x003f_ffe0`, and carries an exact direction-selected ticket into an
R19-shaped frontier. Completion cannot expose the continuation until exact
frontier retirement. Poll is observational and never publishes a continuation;
only flush changes `Ready` to `Published`. Recursive actual-prior-state
transitions derive that 256 MiB requires 65 separately retired packets, with
exact destination-dirty progress, one final completion, and no remaining
ticket or frontier. Exact terminal release permits repeated same-direction or
mixed-direction admission.

A zero-progress retryable prepublication failure restores packet custody and
settles the accepted target as retained conclusive failure code `-1`. A retry after
partial progress restores packet custody and settles an exact, nonresumable
quiescent-without-result marker. Exact later poll observes either terminal by
submission identity; a foreign identity cannot observe or release it. Release
removes the target retainer. Opaque publication/currentness failure preserves
one authority in process-teardown custody. Cancellation is confined to an
unpublished `Ready` target with zero completed bytes.

The independent executable model directly composes the R19 executable adapter
and has 14 focused Rust tests. Fifteen pinned expected-negative mutations cover
direction/storage, offsets, ticket/frontier identity, poll publication, early
continuation, cancellation, dependency bypass, partial progress, terminal
quiescence and exact poll, opaque custody, frontier retirement, pool ABA,
currentness, and the 65-packet bound.

R20 is a standalone mathematical summary, not a correspondence theorem. It
does not prove that the executable Rust model implements the Verus transition,
or that runtime/KFD code implements either model. It proves no native syscall,
queue, DMA, completion, device-memory visibility, hardware liveness,
HIP/HSA parity, or performance claim.

## R20 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Direction/storage, dependency, packet offset, ticket/frontier, retirement, dirty/completion, retry, quiescence, target-retain, cancellation, currentness, and bounded recursive chunking relations | **Proved** | Thirty-one obligations in `r20_runtime_facade_directional_chunking_v1.rs`. |
| Independent executable facade over R19 lifecycle | **Checked** | Fourteen focused Rust tests; direct executable composition without a Verus correspondence theorem. |
| Boundary countermodels | **Rejected** | Fifteen pinned expected-negative files fail only at their named postconditions. |
| Executable-Rust, runtime/KFD, native hardware, liveness, HIP/HSA parity, or performance refinement | **Not established** | Explicitly outside the R20 proof boundary. |

## R21 runtime scripted failure seam

`r21_runtime_scripted_failure_seam_v1.rs` proves 37 obligations for an
independent abstract facade failure seam with exactly one native authority.
Promotion retry preserves exact host custody. Demotion retry before native
demotion preserves exact device custody; only a recovered owner after successful
demotion followed by recycle failure enters cleanup-only demoted-device custody.
Retryable hidden cleanup preserves that recovered authority, and teardown from
promotion, demotion, submission, polling, retirement, recycle, or hidden cleanup
moves the sole authority into opaque process-teardown custody.

Dependency-pending submission is separate from published polling and is
observation-only. An initial retryable submission becomes retained conclusive
failure code `-1`; the same classification after completed progress becomes an
exact quiescent-without-result record without erasing progress. In particular,
a completed D2H chunk records exact host-dirty progress, and a later retryable
submission preserves that host mutation and upgrades to quiescence. Published poll
pending, retry, and timeout preserve exact state and custody. Terminal metadata
mismatch and frontier/recycle mismatch fail closed. Exact completion must pass
through terminal custody, retirement, and slot recycle before a continuation or
terminal result is visible; successful recycle advances the exact slot
generation. Exact terminal release restores device-ready custody, foreign
release is atomic, and teardown blocks both submission and allocation release.

The executable R21 model has 17 focused Rust tests. It structurally owns one
private custody enum with no `Clone` or `Copy` implementation, reuses R20 request
and endpoint types and the R18 packet cap, and can derive its immutable binding
from an idle R19 snapshot. Fifteen pinned expected-negative mutations cover the
promotion, corrected demotion/recovered-cleanup distinction, initial/partial
submission, dependency pending, poll retry, timeout, completion metadata,
retirement, recycle, early continuation, hidden cleanup, and teardown release.

R21 is a standalone mathematical and executable test model. It does not prove a
correspondence with R20, concrete runtime/KFD code, a native failure injector,
or real failures. It proves no syscall, DMA/completion truth, device-memory
visibility, cleanup liveness, hardware behavior, HIP/HSA parity, or performance.

## R21 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Promotion/demotion/recovered cleanup, initial/partial submit and partial-host-mutation classification, dependency/poll observation, completion/retirement/recycle identity, timeout custody, terminal release, and teardown relations | **Proved** | Thirty-seven obligations in `r21_runtime_scripted_failure_seam_v1.rs`. |
| Independent move-only executable failure model | **Checked** | Seventeen focused Rust tests; request-type reuse and snapshot binding without a Verus or concrete-runtime correspondence theorem. |
| Boundary countermodels | **Rejected** | Fifteen pinned expected-negative files fail only at their named postconditions. |
| R20, executable runtime/KFD, native fault injection, hardware, liveness, HIP/HSA parity, or performance refinement | **Not established** | Explicitly outside the R21 proof boundary. |

## R22 batched directional persistent SDMA windows

`r22_batched_directional_persistent_sdma_windows_v1.rs` proves 41 obligations
for an independent abstract directional window machine. A prepared window has
one through 63 packets, exact packet-count arithmetic, contiguous packet
offsets, exact full-window coverage, distinct modulo-64 slots, generations
derived by incrementing each selected slot's independent prior counter, the
selected directional child, and one aggregate lease. A wrapped witness advances
previously unused slot 63 from generation zero to one, reused slot zero from one
to two, and leaves unselected slot one unchanged.
Preparation has no publication effect. Confirmed publication commits the exact
packet count while incrementing the write-pointer and doorbell counters exactly
once each.

Retry before queue custody restores ready custody without consuming the
aggregate lease or changing publication counters. Once published, pending and
timeout observations preserve the whole window. A partial completion retains
published custody and cannot expose a continuation. Recovered publication
without an exact terminal result becomes quiescent and preserves the entire
possibly-mutated extent, including the host extent for D2H. Retained or
substituted publication/completion state becomes opaque process teardown while
retaining exactly one authority.

Exact terminal completion creates a frontier containing the complete window
roster. Continuation and directional dirty progress become visible only after
exact frontier retirement. The bounded 256 MiB witness requires two windows,
63 plus two packets, 65 packets total, and an exact final 2,048-byte packet.
Terminal release preserves reuse for repeated or opposite directions.

The executable R22 model has 18 focused Rust tests. It derives its immutable
binding from an idle R19 snapshot, but independently owns a private aggregate
custody enum with no `Clone` or `Copy` implementation. Public plans, completion
metadata, and frontiers are observations and do not carry authority. Nineteen
pinned expected-negative mutations cover the window bound, exact coverage,
contiguity, slot uniqueness, independent per-slot generation, ticket child,
preparation visibility, pointer and doorbell counts, prepublication restoration,
postpublication classification,
D2H host mutation, partial and timeout custody, completion identity, frontier
roster, early continuation, opaque custody, and the 256 MiB window count.

R22 is not a correspondence theorem for executable R19 or R22 Rust. Its
write-pointer and doorbell counters are mathematical transition fields, not
evidence of CPU atomics, ordering, coherence, firmware consumption, or DMA.
It proves no native KFD execution, hardware completion truth, liveness,
HIP/HSA parity, or performance.

## R22 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Window bounds, coverage, per-slot ticket generations, ticket roster, one publication/doorbell action, custody, mutation extent, exact frontier retirement, and ordered continuation | **Proved** | Forty-one obligations in `r22_batched_directional_persistent_sdma_windows_v1.rs`; finite mathematical values only. |
| Independent move-only executable window model | **Checked** | Eighteen focused Rust tests; R19 identity reuse without a Rust-to-Verus or R19 state-machine correspondence theorem. |
| Boundary countermodels | **Rejected** | Nineteen pinned expected-negative files fail only at their named postconditions. |
| Executable R19/R22, runtime/KFD, native hardware, liveness, HIP/HSA parity, or performance refinement | **Not established** | Explicitly outside the R22 proof boundary. |

## R23 same-device D2D persistent SDMA windows

`r23_same_device_d2d_persistent_sdma_windows_v1.rs` proves 46 obligations for
an independent same-device D2D aggregate-window model. Each admitted operation
owns two distinct move-only allocation authorities and an exact paired
source-read/destination-write lease. Admission requires distinct allocation,
mapping, and backing identities, nonoverlapping mapped extents and requested
ranges, and exact agreement on the VM, physical device, logical queue,
queue occurrence, native queue, and the fixed local H2D SDMA engine used by the
native same-device path.

One window contains one through 63 paired packets with exact contiguous source
and destination coverage. Its ticket roster uses distinct ring slots and the
next generation from each selected slot's independent generation counter;
unselected counters are unchanged. Source and destination persistent-use
generations are consumed when their reservations succeed, before publication.
A destination-reservation failure therefore consumes only the earlier source
generation; a later clean prepublication retry preserves both consumed
generations while restoring both allocation authorities. Preparation is
unpublished. Confirmed publication commits the complete roster with one
write-pointer advance and one doorbell action for the whole window.

Pending and timeout observations preserve both authorities, both leases, and
the exact aggregate roster for repoll. Partial aggregate completion does not
retire a packet prefix, certify destination dirty bytes, or expose a
continuation. Exact authenticated completion must cover every ticket and the
full aggregate byte count before it creates a paired frontier. Destination
dirty progress advances only after exact full-frontier retirement, and only
then may the next window continue. Native execution failure, currentness loss,
completion substitution, or indeterminate publication enters absorbing
quarantine while retaining both authorities and every lease already acquired.
Quarantine entry is validity-preserving even before admission, when no lease
has yet been acquired.

The executable R23 model has 24 focused Rust tests. Its private authority and
lease types have no `Clone` or `Copy` implementation. Twenty-eight pinned
expected-negative witnesses cover allocation and backing aliases, device and VM
binding, mapped overlap and range bounds, exact lease roles and ranges, window
bound, packet coverage and pairing, slot uniqueness and independent per-slot
generation, ticket queue binding, preparation visibility, write-pointer and
doorbell counts, prepublication restoration, pending and timeout custody,
partial aggregate retirement, unauthenticated dirty progress, aggregate
completion, paired frontier identity, quarantine custody, validity-preserving
quarantine entry and absorption, and early continuation. The proof-source
policy forbids module inclusion, so these are authenticated standalone bounded
mutation witnesses, not source-coupled mutation tests of the positive R23 file.

R23 is an independent finite mathematical model. It does not refine R19, R22,
the executable Rust model, the runtime, or KFD. Publication counters are model
fields, not evidence of atomics, ordering, coherence, firmware consumption, or
DMA. The proof establishes no native execution or completion truth, liveness,
HIP/HSA parity, or performance.

## R23 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Paired ownership and leases, reservation-time use generations, fixed H2D child selection, nonaliasing, exact D2D window coverage, independent per-slot generations, one publication/doorbell action, whole-window custody, exact successful completion and frontier retirement, dirty progress, failure quarantine, and ordered continuation | **Proved** | Forty-six obligations in `r23_same_device_d2d_persistent_sdma_windows_v1.rs`; finite mathematical values only. |
| Independent move-only executable D2D window model | **Checked** | Twenty-four focused Rust tests; no Rust-to-Verus correspondence theorem. |
| Boundary countermodels | **Rejected** | Twenty-eight pinned standalone expected-negative witnesses fail only at their named postconditions; they do not import the positive R23 model. |
| Executable Rust, R19/R22, runtime/KFD, native hardware, liveness, HIP/HSA parity, or performance refinement | **Not established** | Explicitly outside the R23 proof boundary. |

## R24 portable progress

`r24_portable_progress_v1.rs` proves 34 obligations for an independent bounded
portable-progress model. Registration atomically binds one existing logical
event and stream custody pair into the progress roster after all admission
checks; a rejected preflight, active event duplicate, active stream duplicate,
stopped engine, or exhausted capacity leaves both abstract pair counts
unchanged. The `event_installed` and
`stream_installed` fields denote retained logical runtime-resource custody,
not active async-waiter or progress-registry membership. The model bounds
both active registration capacity and append-only history at 65,536 and
independently bounds poll and flush work at 1,024 visits per call. Active
capacity and duplicate checks ignore retired history, so retryable-poll,
terminal, and abandoned occurrences permit a later exact re-registration.

Poll and flush use separate stable cyclic cursors over an append-only abstract
roster. A 65-packet transfer first exposes one 63-packet pending window. Only a
completed poll creates the two-packet continuation, and only a later flush may
publish it. A pending poll preserves progress eligibility. A retryable backend
poll rejection preserves phase and logical event, stream, and native custody,
but resolves that observer and retires its progress eligibility. A retryable
flush preserves both logical custody and progress eligibility. Terminal
success and terminal quarantine retire progress eligibility by setting
`observing=false`
while retaining the logical event, stream, and native custody; both terminal
phases are absorbing. The vector remains an append-only historical roster, not
a claim of live progress-registry membership. Transitions select only the
current active occurrence, even when the same identity exists in retired
history. Abandon and drop only disable observation; they preserve phase and
custody. Stop retires all active abstract pair counts while preserving history
and logical custody, without performing a final poll, flush, or cursor advance.

The executable R24 model has 16 focused Rust tests. Nineteen pinned
expected-negative witnesses cover event/stream half-installation, independent
active event and stream duplication, retired-capacity reuse, poll and flush
budget overruns, cyclic roster escape and identity substitution, continuation
before poll, retryable poll custody and progress retirement, retryable flush
progress eligibility, terminal nonabsorption and failure to retire progress
eligibility, abandon
progress and drop custody, Stop progress and registration mutation, and the
exact 63+2 packet split. The proof-source policy
forbids module inclusion, so these are authenticated standalone bounded
mutation witnesses, not source-coupled mutation tests of the positive R24 file.

R24 is an independent finite mathematical model. It does not establish a
Rust-to-Verus correspondence theorem and does not refine runtime threads, the
native runtime, KFD, HSA, HIP, firmware, hardware scheduling, or hardware
liveness. Bounded visit counts and cyclic order are safety properties here;
they do not prove that an OS thread is scheduled or that any device operation
eventually completes.

The executable model derives event identity from context generation plus event
id, and stream identity from context generation plus stream id. The count-only
Verus registration abstraction instead receives separate authenticated
`event_duplicate` and `stream_duplicate` predicates; it proves each rejection
independently but does not derive either predicate from a concrete roster.

## R24 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Atomic event/stream progress pairing, independent duplicate rejection, active-capacity reuse over bounded history, independent budgets, stable cyclic visitation, poll-gated 63+2 continuation, retryable preservation, terminal progress retirement and absorption, observation-only abandon/drop, and Stop without final progress | **Proved** | Thirty-four obligations in `r24_portable_progress_v1.rs`; finite mathematical values only. |
| Independent executable portable-progress model | **Checked** | Sixteen focused Rust tests; no Rust-to-Verus correspondence theorem. |
| Boundary countermodels | **Rejected** | Nineteen pinned standalone expected-negative witnesses fail only at their named postconditions; they do not import the positive R24 model. |
| Rust/native runtime, runtime threads, KFD/HSA/HIP, firmware, hardware scheduling, hardware liveness, parity, or performance refinement | **Not established** | Explicitly outside the R24 proof boundary. |

## R25 persistent-compute storage bridge

`r25_persistent_compute_storage_bridge_v1.rs` proves 38 obligations for one
independent finite storage bridge. One caller-constructed storage identity is
retained through `FullH2dReady -> PreparedCompute -> Published -> Completed ->
Restored -> Device`. Preparation derives Read, Write, or ReadWrite authority
from abstract kernel effects. Read and ReadWrite require prior initialization;
a full Write establishes initialization only at exact frontier retirement. The
logical and physical ranges both start at zero and equal the single bounded
storage extent. The extent is nonzero and at most 256 MiB.

All preparation predicates are checked before the abstract phase changes.
Successful preparation selects the persistent fast path, whose generic
materialization count remains exactly zero. Retryable publication and restore
are exact no-effect transitions. Pending completion retains the complete
published state. Successful completion must authenticate the storage identity,
operation generation, full range, and derived authorization before restore is
enabled. Ambiguous publication, unauthenticated completion, ambiguous restore,
and post-retention faults enter an absorbing quarantine.

Only an authenticated Completed state may restore, and only the exact active
frontier may retire Restored custody to quiescent Device state. Retirement
preserves storage identity, records the active generation as the retired
frontier, and makes the next admitted generation strictly newer. Stale storage,
frontier, and active-generation values leave the model unchanged. Generation
inputs and the storage extent are bounded by finite `u64` coordinates even
though the Verus carrier uses mathematical natural numbers.

The executable R25 model has 17 focused Rust tests. Eighteen pinned standalone
expected-negative witnesses cover storage substitution, derived authorization,
read initialization, exact full extent, fallback after fast-path selection,
retryable and ambiguous publication, pending custody, completion-before-restore,
completion-coordinate authentication, post-retention quarantine, retryable
restore, exact retirement frontier, generation ABA, nonzero generic
materialization, quarantine reopening, retirement storage substitution, and
frontier advance. They do not import the positive R25 proof source.

R25 identities, effects, outcomes, and completion coordinates are finite
mathematical inputs, not native evidence. R25 establishes no Rust-to-Verus
correspondence theorem and does not refine the executable model, runtime, KFD,
HSA, HIP, firmware, coherence, hardware execution, completion truth, liveness,
parity, or performance. It also does not prove that a concrete kernel's
metadata truthfully describes its reads or writes.

## R25 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Exact storage retention, derived access, initialization gate, full logical/physical extent, no fallback/materialization, pending and retryable retention, completion authentication before restore, terminal quarantine, exact frontier retirement, and generation rejection | **Proved** | Thirty-eight obligations in `r25_persistent_compute_storage_bridge_v1.rs`; finite mathematical values only. |
| Independent executable persistent-compute bridge model | **Checked** | Seventeen focused Rust tests; no Rust-to-Verus correspondence theorem. |
| Boundary countermodels | **Rejected** | Eighteen pinned standalone expected-negative witnesses fail only at their named postconditions and do not import the positive R25 model. |
| Rust ownership, executable-model refinement, runtime/KFD/HSA/HIP, firmware, hardware truth, liveness, parity, or performance | **Not established** | Explicitly outside the R25 proof boundary. |

## R27 persistent dispatch control

`r27_persistent_dispatch_control_v1.rs` proves 20 obligations for an independent
finite prepare-once/replay-many control model. The model distinguishes
`Ordinary`, `Attached`, and `DataDetached` control phases. An attached control
owns exactly one abstract data authority. Recycle-detach transfers that
authority to the external owner exactly once while retaining the identity,
premise, code, kernarg, packet, and exact detached generation. Replay is
admitted only from `DataDetached`, for the exact retained identity and exact
recycled predecessor, and advances the generation by one while transferring
the sole authority back to the attached control.

Publication requires an attached authority, the exact retained identity, and
the exact active generation. Identity or generation substitution is an atomic
no-effect transition. A dedicated control-only eviction is admitted only from
`DataDetached` with zero queue authorities, one external authority, one
premise, and the exact detached generation. Eviction consumes the retained
control resources without changing that detached-generation ledger or the
authority event balance.

Six pinned standalone expected-negative witnesses cover predecessor
substitution, replay from the wrong phase, authority duplication, generation
reuse, identity-substituted publication, and loss of detached generation on
control eviction. They do not import the positive R27 proof source.

R27 identities, generations, and authority counters are finite mathematical
inputs. The proof has no Rust-to-Verus correspondence theorem and does not
refine executable ownership, runtime/KFD behavior, HSA, HIP, firmware,
hardware execution, completion truth, liveness, parity, or performance.

## R27 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Exact recycled predecessor and detached-phase replay, single-authority transfer, strict generation advance, identity-gated publication, and detached-ledger-preserving control eviction | **Proved** | Twenty obligations in `r27_persistent_dispatch_control_v1.rs`; finite mathematical values only. |
| Boundary countermodels | **Rejected** | Six pinned standalone expected-negative witnesses fail only at their named postconditions and do not import the positive R27 model. |
| Rust ownership, executable runtime/KFD/HSA/HIP, firmware, hardware truth, liveness, parity, or performance refinement | **Not established** | Explicitly outside the R27 proof boundary. |

## R28 persistent hot-currentness scope

`r28_persistent_hot_currentness_scope_v1.rs` proves 31 obligations for an
independent finite policy model of the reviewed persistent-compute replay path.
An initial bind consumes one full-audit outcome. A retained-control replay
consumes one fresh operational checkpoint before submission. Successful submit,
completion, and recycle transitions consume exactly three, two, and two
operational checkpoints respectively. Ring occupancy is retryable only after
the two pre-effect submit checkpoints and preserves the exact prepared receipt.

Failure dispositions are stage-specific. They distinguish submit failures after
zero, one, two, and three checkpoints from a post-submit publication-ledger
failure; completion and recycle distinguish inner-operation failure from
post-envelope ledger failure. Terminal states retain the address-free production
custody stage: attached, published, completed, recycled, data detached, storage
detached, restored, retained control, or control released. Detach and cancel
encode the custody stages reported by the reviewed `queue_live.rs` paths. Close
checks detached closeability before stable-binding authentication, consumes one
full-audit outcome, and distinguishes retained from already released control.

The active-attempt invariant binds the stable identity, attachment successor,
and exact predecessor frontier. Each legal open, replay, submit, completion,
recycle, detach, cancel, and close transition preserves the complete state
invariant. Attachment or dispatch values without a successor are rejected
atomically. Mathematical audit and completion counters do not drive a terminal
transition; the executable observation counters saturate rather than fabricating
a production generation failure.

Two pinned expected-negative transition models retain the positive replay and
close admission/control-flow shapes while respectively deleting the replay
operational increment and the close full-audit increment. Their named
postconditions fail under those coupled mutations.

The Rust model's private owners are non-clone, but R28 proves no production
authority count or Rust ownership theorem. Audit outcomes remain caller-supplied
contracts. There is no Rust-to-Verus correspondence theorem and no proof of
Linux currentness, reset detection, KFD/HSA/HIP behavior, hardware execution,
completion truth, liveness, parity, or performance.

## R28 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Full open/close, replay pulse, 3/2/2 success envelopes, retry boundary, exact binding frontier, terminal custody stages, cancellation, generation rejection, and transition invariant preservation | **Proved** | Thirty-one obligations in `r28_persistent_hot_currentness_scope_v1.rs`; contracted inputs and finite policy state only. |
| Executable policy model | **Checked** | Focused Rust transition tests; private non-clone fields are not production authority evidence. |
| Coupled boundary countermodels | **Rejected** | Two pinned mutated replay/close transition bodies fail their named checkpoint postconditions. |
| Rust ownership, production authority conservation, concrete currentness, runtime/KFD/HSA/HIP, hardware truth, liveness, parity, or performance refinement | **Not established** | Explicitly outside the R28 proof boundary. |

## R30 bound host-content certificate

`r30_bound_host_content_certificate_v1.rs` proves 38 obligations for an
independent finite certificate and promotion model. A certificate binds the
queue identity and generation, storage identity and generation, pool
generation, logical and physical extents, exact zero-based full range, and an
abstract content digest. Certification additionally requires equal logical and
physical byte counts, matching the authenticated full-write and H2D transfer
contract rather than certifying padded unequal extents.

The full-write transition follows the reviewed production ordering. It clears
the prior certificate before the opening currentness observation. Opening
ambiguity therefore records an invalidation with no possible write; after a
current opening, the model records the possible write, and closing ambiguity
leaves the host uncertified. A certificate is established only after an exact
full write and current closing observation. CPU destination writes, SDMA
destination writes, resize, and recycle record certificate invalidation before
possible mutation. H2D source use and exact full-H2D completion preserve the
source certificate.

Exact completed-H2D custody binds storage and the equal full range independently
of the optional stored certificate. Promotion first authenticates that custody, then evaluates the
opening and closing currentness observations, and only afterward classifies a
missing or mismatched candidate certificate as retryable. Retryable mismatch is
an exact no-effect transition and retires no completion frontier. Ambiguity at
either observation enters an absorbing terminal state, records the distinct
opening or closing stage, and retains the exact completion plus the optional
stored host certificate without retirement. Success atomically consumes the
pending completion, advances its exact frontier, and mints Ready containing
only the authenticated digest and completion generation. The returned host
certificate is initially preserved, but Ready validity is independent of it;
subsequent host destination or recycle transitions invalidate/change host state
without changing the Ready digest.

The executable R30 model has 14 focused Rust transition tests. Three pinned
standalone coupled mutations respectively retain a certificate across a
destination mutation, omit the production-ordered clear before opening
currentness, and classify candidate mismatch before closing currentness. Each
fails only at its named postcondition and imports no positive proof source.

Digest values are opaque contracted inputs: neither model computes nor proves
SHA-256. Coherent CPU write completion, DMA/HSA completion and visibility, and
both currentness observations are also contracted inputs. There is no
Rust-to-Verus correspondence theorem and no proof that executable Rust or the
production runtime refines either finite model.

## R30 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Full identity/range/digest binding, equal certified extents, invalidate-before-mutate ordering, H2D source preservation, two-observation promotion ordering, retry atomicity, exact ambiguity custody, terminal absorption, and atomic Ready mint/retirement | **Proved** | Thirty-eight obligations in `r30_bound_host_content_certificate_v1.rs`; finite mathematical values and contracted observations only. |
| Executable certificate model | **Checked** | Fourteen focused Rust transition tests; no Rust-to-Verus correspondence theorem. |
| Coupled boundary countermodels | **Rejected** | Three pinned mutated transition bodies fail their named invalidation, precheck-clear, and currentness-before-mismatch postconditions. |
| SHA-256 correctness, coherent-write truth, DMA/HSA completion or visibility, currentness truth, executable Rust refinement, runtime/KFD/HSA/HIP, firmware, hardware, liveness, parity, or performance | **Not established** | Explicitly outside the R30 proof boundary. |

## R31 single-packet/window refinement

`r31_single_packet_window_refinement_v1.rs` proves 41 obligations for an
independent finite relation between a bounded R19-shaped single-copy lifecycle
and the corresponding one-element R22-shaped window lifecycle. The request is
nonempty, in range at both endpoints, and no larger than
`0x003f_ffe0` bytes. Projection retains the queue, storage, generation, range,
direction, certificate, completion, custody, authority, lease, ticket,
currentness-count, mutation, retirement, digest, and terminal-stage
coordinates while fixing the normalized window request count to one.

Single and one-element-window transitions are defined independently. Generic
submit, poll, and promotion theorems establish their lockstep projection, and
complete H2D and D2H trace theorems compose those steps. Successful publication
owns exactly one packet, ticket, and lease. The modeled production envelope
records three directional and one queue operational checks for publication and
two queue operational checks for a pending or completed poll. These are
abstract event counts, not observations that Linux or hardware performed the
checks.

H2D request construction preserves an exact bound host-content certificate.
D2H destination construction invalidates it; retry after that construction
remains invalidated, while a rejection before request construction preserves
the prior certificate. Closing ambiguity conservatively records possible D2H
host mutation because native publication may already have occurred. Exact
completion retains direction, byte count, both
offsets, and packet count one across normalization. Full-H2D promotion evaluates
opening and closing currentness before certificate mismatch classification.
Mismatch is retryable without retirement. Either ambiguity retains completion
and retires nothing in exact stage-specific terminal custody. Success alone
retires the frontier once and mints compute-ready state carrying the bound
abstract digest.

The executable R31 model has 16 focused Rust tests. Three pinned standalone
transition-coupled mutations respectively retain the certificate and deny
possible host mutation across D2H closing ambiguity, swap completion offsets during normalization, and
retire completion before closing-currentness classification. Each fails only
at its named postcondition and imports no positive proof source.

The formal models do not import or prove equivalence to the executable R19 or
R22 state machines. They also do not represent heap allocation, so the proof
does not establish an allocation-free fast path. Requests, certificates,
digests, currentness outcomes, completion observations, and mutation reports
are contracted mathematical inputs. There is no Rust-to-Verus, source-to-ISA,
KFD, HSA, HIP, driver, firmware, hardware, progress, liveness, parity, or
performance refinement.

## R31 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Bounded one-request normalization, independent submit/poll/promotion lockstep, exact packet/ticket/lease cardinality, operational-check event counts, H2D certificate preservation, D2H invalidation, exact completion metadata, ambiguity custody, and success-only retirement | **Proved** | Forty-one obligations in `r31_single_packet_window_refinement_v1.rs`; finite mathematical values and contracted observations only. |
| Executable single/window model | **Checked** | Sixteen focused Rust transition tests; no Rust-to-Verus correspondence theorem. |
| Coupled boundary countermodels | **Rejected** | Three pinned mutated transition bodies fail their named invalidation, completion-offset, and promotion-retirement postconditions. |
| Allocation behavior, currentness/completion truth, executable Rust refinement, runtime/KFD/HSA/HIP, driver, firmware, hardware, progress, liveness, parity, or performance | **Not established** | Explicitly outside the R31 proof boundary. |

## R32 directional currentness handoff

`r32_directional_sdma_currentness_handoff_v1.rs` proves 34 obligations for an
independent finite comparison of the former directional SDMA publication
lifecycle and the R32 shared-currentness lifecycle. The reference machine uses
separate successful-prepare close and publication-open observations. The R32
machine uses one shared queue observation after successful preparation, moves a
queue-occurrence/native-queue/direction/packet-roster-bound handoff directly
into publication, and retains the final directional close. Under the explicit
refinement premise that the former close and open receive the same contracted
currentness value, both machines return the same external outcome, exact
request/prepared/published custody class, binding, ticket roster, certificate
state, and publication-attempt classification.

On successful preparation and a current shared observation, the reference
machine records four operational checks and R32 records three. The proof models
these as abstract event counts; it does not establish the cost or truth of any
Linux observation. The shared event and publication are adjacent, and the
modeled interval contains zero fallible and zero native actions. A failed shared
observation is terminal with prepared custody, attempts no publication, and
does not execute the post-publication close. Once publication is attempted, all
recoverable, retained, and published outcomes retain that final close and exact
custody classification.

Every modeled lower preparation failure, including the release-checked ticket
roster mismatch, takes the old explicit close. Only a retryable lower failure
with a current close restores request custody; close loss, owner poison, and
roster mismatch retain terminal request custody. Ticket rosters contain one to
64 occurrences and bind every ticket to the logical queue generation, native
queue, direction, and exact occurrence. H2D certificate state is unchanged and
D2H certificate state remains invalidated. The same-device D2D state is an
identity projection and is not transformed by R32.

The executable R32 model has 16 focused Rust tests. A structural test confirms
that its private handoff carrier has no `Clone` derive or implementation, and a
compile-fail doctest rejects cloning the public owning model. The formal
mathematical handoff is openly represented so Verus can prove its coordinate
equalities; Verus mathematical values do not prove Rust privacy, move-only
ownership, or non-duplication. Production privacy and source ordering therefore
remain executable/source-review properties, not consequences of this proof.
Three pinned transition-coupled mutations respectively elide the retained
prepare-failure close, expose shared-check failure as retryable request custody,
and insert a native action between shared observation and publication. Each
fails only at its named postcondition and imports no positive proof source.

Currentness values, lower preparation/publication outcomes, identities,
certificates, and roster contents are contracted mathematical inputs. There is
no Rust-to-Verus correspondence theorem and no proof that production Rust
refines either finite machine. The model does not cover allocation success,
owner-memory loan mechanics, borrow exclusivity, compiler reordering, unwind,
panic, reset detection, syscalls, KFD/HSA/HIP, driver, firmware, hardware,
coherence, DMA visibility, progress, liveness, parity, or performance.

## R32 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Shared observation refinement, exact 4-to-3 successful check count, failure custody, immediate handoff/publication event order, final-close retention, full ticket binding, certificate preservation/invalidation, and same-device identity | **Proved** | Thirty-four obligations in `r32_directional_sdma_currentness_handoff_v1.rs`; finite mathematical values and contracted observations only. |
| Executable handoff model | **Checked** | Sixteen focused Rust transition/structure tests plus one compile-fail non-`Clone` doctest; no Rust-to-Verus correspondence theorem. |
| Coupled boundary countermodels | **Rejected** | Three pinned mutated transition bodies fail their named close-retention, terminal-prepared-custody, and immediate-publication postconditions. |
| Rust privacy/move semantics, allocation/loan behavior, currentness truth, executable Rust refinement, KFD/HSA/HIP, driver, firmware, hardware, coherence, progress, liveness, parity, or performance | **Not established** | Explicitly outside the R32 proof boundary. |

## R33 fused synchronous directional SDMA

`r33_fused_synchronous_directional_sdma_v1.rs` proves 45 obligations for an
independent finite comparison between the former R32 submit-then-wait
composition and the R33 fused synchronous composition. The reference machine
records five successful operational-currentness observations: submit opening,
prepublication, submit close, wait opening, and final currentness. The fused
machine records three: opening, prepublication, and final currentness. It also
records four versus two abstract owner/model-foundation loan events. One shared
opening-loan result and separate former submit, former submit-close, former
wait, and fused-execution loan results make every fallible loan boundary
explicit. These are mathematical event counts and contracted results, not
measurements or proofs of Rust allocation, borrowing, syscalls, or hardware
cost.

The equivalence theorem has three explicit premises. The sticky/aligned-
currentness premise gives the removed successful submit close and wait opening
the same contracted value as the retained prepublication observation. A
path-sensitive retained-loan premise relates the former submit loan and fused
execution loan only where their different retake points can change custody. A
path-sensitive removed-loan premise requires former submit-close and wait loan
success only where failure would distinguish the results. For example, a
retained publication already has terminal prepared-queue-retained custody, and
an unrestorable completed record already has terminal completed-unrestored
custody, so irrelevant removed-loan failures are not assumed away. Under these
premises, both machines return the same request binding, planned and returned
ticket values, host-certificate state, external outcome, exact custody class,
publication and wait classification, and lower-record retirement decision. A
countermodel demonstrates that eliding the middle observations is unsound
without the currentness premise.

The fused successful path moves the queue, queue generation, native queue,
direction, offsets, length, sequence, ticket generation, and certificate state
through a mathematical handoff directly into publication. Publication is the
next modeled event with zero intervening fallible or native actions. The wait
remains in that publication loan. The prepared persistent use and detached
request exist before the fused loan opens, so loan-open failure has terminal
prepared custody and an admitted D2H request has already invalidated its host
certificate. A lower completion record is exposed and removed only after the
final-currentness event succeeds. Final-currentness loss retains terminal
published custody and leaves the lower queue record unretired. After successful
final currentness the lower completed record is removed before host/device
ownership restoration. Failed `restore_directional_completed_sdma_copy_v1`, or
failure to retake the enclosing loan after that removal, retains exact terminal
completed-unrestored custody; only successful restoration returns completed
custody.

The transition set covers opening and loan-open failure, retryable and poisoned
preparation failure, prepublication currentness loss, recoverable and retained
publication, returned-ticket mismatch, timeout, lower wait failure,
final-currentness loss, completion restoration success/failure, and every
distinct loan open/retake failure. A confirmed
publication with a returned ticket different from the planned ticket is
terminal published, records both ticket values, performs its final currentness
observation, and never enters wait. Retained publication is instead terminal
prepared-queue-retained custody and preserves the lower returned ticket without
claiming it is the planned ticket. Timeout retains the exact confirmed
published ticket. Lower wait failure retains terminal published custody.
Retake failure after preparation rejection or recoverable publication is
terminal prepared; after timeout it is terminal published; and after a
successfully checked and removed completion record it is terminal
completed-unrestored.
H2D preserves the host certificate. D2H invalidates it after admission and
request construction, including before a fused-loan-open failure, while opening
failure preserves the prior certificate. Same-device D2D identity is an
unchanged projection.

The executable R33 model has 23 focused Rust tests. A structural test confirms
that its private prepared-handoff carrier has no `Clone` derive or
implementation, and a compile-fail doctest rejects cloning the public owning
model. Verus mathematical values remain copyable, so the proof does not
establish Rust privacy, move-only ownership, loan mechanics, or borrow
exclusivity. Four pinned transition-coupled mutations respectively elide the
middle close/open observations without the alignment premise, release published
custody on timeout, and retire a completed record before failed final
currentness, and release a completed record after failed restoration. Each
fails at its named postcondition and imports no positive proof source.

Currentness values, lower outcomes, identities, certificates, and loan results
are contracted mathematical inputs. There is no Rust-to-Verus correspondence
theorem and no proof that production Rust refines either finite machine. The
model does not establish native/KFD/HSA/HIP behavior, driver or firmware
correctness, hardware completion or coherence, progress, liveness, parity, or
performance.

## R33 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Premised submit/wait-to-fused external equivalence, exact 5-to-3 observation and 4-to-2 abstract-loan counts, path-sensitive removed-loan requirements, immediate handoff/publication, same-loan wait, final-currentness-before-retirement, exact completed-restoration custody, planned/returned ticket binding and mismatch handling, certificate behavior, and same-device identity | **Proved** | Forty-five obligations in `r33_fused_synchronous_directional_sdma_v1.rs`; finite mathematical values and contracted observations only. |
| Executable fused-composition model | **Checked** | Twenty-three focused Rust transition/structure tests plus one compile-fail non-`Clone` doctest; no Rust-to-Verus correspondence theorem. |
| Coupled boundary countermodels | **Rejected** | Four pinned mutated transition bodies fail their named alignment, timeout-custody, retirement-order, and completion-restoration-custody postconditions. |
| Rust privacy/move semantics, concrete loans or allocation, currentness/completion truth, executable Rust refinement, runtime/KFD/HSA/HIP, driver, firmware, hardware, coherence, progress, liveness, parity, or performance | **Not established** | Explicitly outside the R33 proof boundary. |

## R34 fused asynchronous directional SDMA

`r34_fused_asynchronous_directional_sdma_v1.rs` proves 54 obligations for an
independent finite comparison of the former and fused public asynchronous
single-copy submission paths reviewed at production commit
`b015b81f862220d48671e1c4809b8ce858a317e7`. Admission occurs before every
modeled loan. The former composition then uses separate opening,
lower-preparation/publication, and final-currentness loans. The fused
composition uses one loan spanning opening currentness, persistent
reserve/prepare/detach and request construction, lower preparation,
prepublication currentness, immediate handoff/publication, final currentness,
and retake. On a successful confirmed submission both compositions perform
three currentness observations, while their abstract executed loan counts are
exactly three and one. These are finite event counts, not allocation, syscall,
latency, or performance measurements.

The external-equivalence theorem requires an explicit input-only,
path-sensitive loan premise. It does not call either runner or compare their
outputs. The premise accounts for the former opening loan retaking before
request construction while the fused loan retakes only at the end, requires a
successful fused retake for retryable request-preparation rejection, relates
the former execution/final retake conjunction to the fused retake only when it
can change a lower-failure or publication result, and permits irrelevant
retake differences after prepublication loss or retained publication. A
bounded exhaustive Rust test independently enumerates the model's complete
finite observation space and checks that every admitted premise implies the
executable model's external-equivalence relation. This remains testing of the
model, not Rust-to-Verus or production refinement.

The transition set includes retryable and terminal admission; opening loan and
currentness failure; use-request, reserve, prepare, and detach rejection;
retryable and poisoned lower preparation; its closing observation;
prepublication loss and the fused second closing observation; recoverable,
retained, and confirmed publication; final currentness; planned-ticket
occurrence validation; returned-ticket equality; and every former/fused retake
stage. Request-preparation rejection remains retryable only after a successful
fused retake. Once detach and request construction succeed, a fused retake
failure is never retryable: lower/recoverable failures retain terminal prepared
custody and confirmed publication retains terminal published custody. The
former opening-retake failure instead occurs before request construction and
retains terminal request custody.

A failed prepublication observation performs the modeled second fused close
but remains terminal prepared regardless of that result. The mathematical
handoff is followed immediately by publication with zero intervening fallible
or native actions. Recoverable publication returns retryable request custody
only after successful final currentness and relevant retakes. Retained
publication always retains terminal prepared-queue-retained custody. Confirmed
publication succeeds only when final currentness and retake succeed, the
planned ticket names the exact queue occurrence, and the returned ticket equals
the plan; otherwise exact terminal published custody retains both ticket
values. H2D preserves the host certificate. D2H invalidates it only after
successful request construction, so opening and request-preparation failures
preserve the prior certificate.

The executable R34 model has 25 focused Rust tests, including the exhaustive
finite premise check. A structural test checks that the private prepared
handoff carrier has no `Clone` implementation, and a compile-fail doctest
rejects cloning the public owning model. Verus mathematical values remain
copyable, so these proofs do not establish Rust privacy, move-only ownership,
loan mechanics, or borrow exclusivity. Four pinned independent mutations omit
the final retake, return retryable custody after detach, insert fallible work
between handoff and publication, and accept a substituted returned ticket.
Each fails its named postcondition and imports no positive proof source.

All currentness values, operation outcomes, identities, certificates, tickets,
and loan results are contracted mathematical inputs. There is no Rust-to-Verus
correspondence theorem and no proof that production Rust refines either finite
machine. The model does not establish runtime/KFD/HSA/HIP behavior, driver or
firmware correctness, hardware completion or coherence, progress, liveness,
parity, or performance.

## R34 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Premised former/fused external equivalence, exact successful 3-to-1 abstract-loan reduction with three currentness observations in both, admission/preparation order, path-sensitive retake requirements, immediate handoff/publication, exact retryable/terminal custody, and planned/returned ticket binding | **Proved** | Fifty-four obligations in `r34_fused_asynchronous_directional_sdma_v1.rs`; finite mathematical values and contracted observations only. |
| Executable fused-composition model | **Checked** | Twenty-five focused Rust tests, including exhaustive finite observation enumeration, plus one compile-fail non-`Clone` doctest; no Rust-to-Verus correspondence theorem. |
| Boundary countermodels | **Rejected** | Four pinned mutations fail their named omitted-retake, retry-after-detach, intervening-fallible-work, and ticket-substitution postconditions. |
| Rust privacy/move semantics, concrete loans or allocation, currentness truth, executable or production Rust refinement, runtime/KFD/HSA/HIP, driver, firmware, hardware, coherence, progress, liveness, parity, or performance | **Not established** | Explicitly outside the R34 proof boundary. |

## R35 fused retained-control replay

`r35_fused_retained_control_replay_v1.rs` proves 13 obligations for an
independent finite comparison of the former and fused retained-dispatch-control
replay bind paths reviewed at production commit
`4b324bbd53e4c6e767c5c5f2f18817c133edbe03`. The model is limited to the
retained-control branch; it makes no claim about initial dispatch-control
construction. Admission and use request/reserve/prepare happen before every
modeled foundation loan. The former composition has one loan for mapped facts,
then detach and authenticated data construction, then a second loan for replay
retention, followed by the operational-currentness and complete-authority-set
audit. The fused composition has one loan spanning mapped facts, detach,
authenticated construction, replay retention, and that final audit. On success
the abstract foundation-loan attempts are exactly two and one, while both paths
retain the same modeled admission-currentness and final operational-currentness
observations. These are finite event counts, not syscall or latency evidence.

The projected custody-and-commit equivalence theorem requires an explicit
input-only, path-sensitive loan premise. It invokes neither runner and compares
no result states. The premise relates the former first loan to the fused loan before
detach, accounts for the former first retake occurring before detach while the
fused retake occurs after the pipeline, aligns a former second-loan open failure
with fused retention failure, and requires the relevant final retakes only when
they can distinguish success from terminal custody. A bounded exhaustive Rust
test independently enumerates all 196,608 finite observation combinations and
checks all 186,288 premise-admitted cases. This is executable-model testing,
not a Rust-to-Verus correspondence theorem.

Before detach, failure returns retryable input only when the complete loan
round trip, prepared-use cancellation, and session-health observation all
succeed. Cancellation failure installs terminal Attached custody. After detach,
authenticated construction failure retains exact Storage custody, replay
retention failure retains Data custody, final-audit failure retains Attached
custody, and no such outcome is retryable. Ready followed by failed fused
retake is also terminal Attached. Every installed terminal attachment binds the
exact queue occurrence, attachment generation, storage identity, effect,
predecessor generation, retained dispatch control, and next attachment
generation. Success additionally clears the recycled predecessor roster.

R35 corrects one internal authority label rather than claiming exact internal
state equivalence with the former path. If quarantine succeeds, terminal
prepared authority is `Quarantined`; if quarantine itself fails, the fused path
preserves the returned live `Prepared` authority. The projected relation
compares custody and exact commit coordinates, but it intentionally excludes
production/public error identity, the model terminal failure stage, the
internal authority label, every event index, foundation-loan counts, and
currentness counts. The successful-path loan and currentness counts are checked
by separate theorems and tests; they are not part of the equivalence relation.
The executable model has 14 focused Rust tests,
including the exhaustive premise check. Private Storage/Data/Attached carriers
have structural non-`Clone` checks, and a compile-fail doctest rejects cloning
the public owning model. Verus mathematical values remain copyable and prove no
Rust ownership or borrow property.

Four pinned, standalone expected-negative mutations respectively commit Ready
after failed retake, return retryable input after detach, substitute Data for
Storage custody at construction failure, and discard Prepared authority after
failed quarantine. Each imports no positive proof source and fails its named
postcondition.

All admission, native-operation, currentness, cancellation, quarantine, and
loan outcomes are contracted mathematical inputs. There is no Rust-to-Verus
correspondence theorem and no proof that the executable model or production
Rust refines the Verus machine. Nothing here establishes runtime, KFD, HSA, or
HIP behavior; allocation or concrete loan mechanics; driver or firmware
correctness; hardware completion or coherence; progress; liveness; parity; or
performance.

## R35 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Premised former/fused projected custody-and-commit equivalence, exact successful 2-to-1 foundation-loan reduction with preserved currentness observations, admission/preparation order, pre-detach retry gate, post-detach Storage/Data/Attached custody, Ready-retake terminalization, failed-quarantine Prepared retention, and exact commit coordinates | **Proved** | Thirteen obligations in `r35_fused_retained_control_replay_v1.rs`; finite mathematical values and contracted observations only. The projection excludes production/public error identity, model terminal failure stage, internal authority label, event indices, foundation-loan counts, and currentness counts; successful-path counts are separate theorems. |
| Executable fused replay model | **Checked** | Fourteen focused Rust tests, including 196,608-case exhaustive enumeration, explicit projection-boundary coverage, and move-only checks, plus one compile-fail non-`Clone` doctest; no Rust-to-Verus correspondence theorem. |
| Boundary countermodels | **Rejected** | Four pinned mutations fail their named omitted-retake, retry-after-detach, custody-substitution, and failed-quarantine-authority postconditions. |
| Rust privacy/move semantics, concrete loans or allocation, currentness truth, executable or production Rust refinement, runtime/KFD/HSA/HIP, driver, firmware, hardware, coherence, progress, liveness, parity, or performance | **Not established** | Explicitly outside the R35 proof boundary. |

## R36 fused completion poll and recycle

`r36_fused_completion_poll_recycle_v1.rs` proves 15 obligations for an
independent finite comparison of abstract split poll-then-recycle and fused
completion-poll/recycle compositions reviewed at production commit
`d32aa6e61e49fb16e44ba3cd715563e9e452b23f`. All poll, recycle, midpoint, and
currentness results are contracted mathematical observations.

Pending returns exact Published custody after two currentness checks and does
not capture a midpoint, reset a signal, or attempt either dispatch or
attachment recycle. Ready advances the abstract dispatch-completion and
allocation-completion stages before capturing the caller-supplied midpoint;
any reset or later recycle event follows that midpoint. Successful split and
fused paths preserve the same projected custody and logical ordering while
performing exactly four and three abstract currentness checks respectively.
The input-only fusion premise requires the removed split recycle-opening check
to succeed only on Ready paths. It invokes neither execution relation and
compares no output states.

Poll failures route as Poll and retain Published custody through published
state, dispatch-generation, and completion-observation failure, then Completed
custody at dispatch- or allocation-completion failure. Failures reached after
Ready route as Recycle. Signal-generation, signal-reset, closing-currentness,
recycle-currentness, and recycle-infrastructure failures retain Completed
custody. Dispatch-recycle failure retains Recycled custody after the signal
reset. Successful retirement is ordered midpoint, reset, closing currentness,
dispatch recycle, then attachment recycle. Every outcome has exactly one
Published, Completed, or Recycled stage authority.

The executable model has nine focused Rust tests. Its exhaustive test checks
all 196 finite observation combinations and all 182 premise-admitted cases.
Private Published/Completed/Recycled carriers have structural non-`Clone`
checks, and a compile-fail doctest rejects cloning the public owning model.
Verus mathematical values remain copyable and prove no Rust ownership or
borrow property. Four standalone pinned countermodels cover recycle on Pending,
midpoint capture before Ready, wrong reset-failure custody, and dispatch
retirement before signal reset.

The projection deliberately excludes the currentness-check count because that
is the optimized coordinate, and it makes no claim about production/public
error identity, real `Instant` values, or physical event timing. The separate
successful-count theorem is a finite arithmetic fact, not latency or
performance evidence. There is no Rust-to-Verus correspondence theorem and no
proof that the executable model or production Rust refines the Verus machine.

## R36 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Premised split/fused projected custody-and-ordering equivalence, Pending short-circuit, Ready midpoint placement, exact Published/Completed/Recycled failure custody, Poll/Recycle routing, one stage authority, and retirement ordering | **Proved** | Fifteen obligations in `r36_fused_completion_poll_recycle_v1.rs`; contracted finite observations and logical event indices only. |
| Successful currentness-check reduction | **Proved** | Separate theorem establishes abstract four-to-three counts with successful observations; the count is excluded from the equivalence projection. |
| Executable completion-poll/recycle model | **Checked** | Nine focused Rust tests, including all 196 finite cases and 182 premise-admitted cases, structural move-only checks, plus one compile-fail non-`Clone` doctest; no Rust-to-Verus correspondence theorem. |
| Boundary countermodels | **Rejected** | Four pinned mutations fail their named recycle-on-Pending, midpoint-before-Ready, failure-custody, and retirement-order postconditions. |
| Production/public error identity, real clock values or physical timing, concrete currentness/completion truth, Rust privacy or borrow semantics, executable or production Rust refinement, runtime/KFD/HSA/HIP, driver, firmware, hardware, coherence, progress, liveness, parity, or performance | **Not established** | Explicitly outside the R36 proof boundary. |

## R37 typed native SDMA wait activation

`r37_typed_native_sdma_wait_activation_v1.rs` proves 15 obligations for an
independent finite model reviewed against signed production commit
`f81d67fa603ecf23ebd101556b327ae80f13c5ec`. The commit is provenance, not a
refinement link. The model starts with exact abstract Published custody for a
directional or same-device submission and consumes one caller-supplied native
wait observation.

Only `ExactTypedTimeout` with the exact modeled native owner and request
identity returns recoverable Published custody. A matching non-timeout
retryable observation is terminal and retains exact Pending native custody.
Pending and Completed identity-change observations are terminal and retain the
changed owner returned at their respective stage rather than substituting the
expected owner. Teardown retains its exact opaque terminal token. Every
terminal path removes the operational active and published-index memberships
while preserving the exact in-flight source and destination storage
generations, dependency count, allocation-custody counts, stream count,
current stream membership, and surrounding stream frame.

Timeout restores the exact active Published phase, sorted-index frame, both
in-flight storage coordinates, dependency and allocation retain counts, stream
membership and frame, and native Published owner. Both zero and positive
abstract deadline classes perform exactly one modeled native observation.
This is a finite event count, not evidence about a real `Instant`, elapsed
time, syscall count, polling implementation, or device observation.

Exact completion alone may advance the model. Settlement restores both exact
storage tokens, records success, and releases one modeled dependency,
source-custody, destination-custody, and stream owner. A partial-window
completion restores storage but remains active in Ready custody, is absent from
the published index, retains the logical owners, and performs zero continuation
publications; explicit flush remains outside this transition. The routing
model proves that explicit Poll always selects the pre-existing Poll route,
Published waits select the matching directional or same-device native route,
and non-Published waits retain the legacy polling route. It does not model or
prove the internal semantics of those unchanged routes.

The executable model checks exactly 56 legal cases: two custody-frame
profiles, two copy kinds, two deadline classes, and seven meaningful outcomes
(two completion dispositions, exact timeout, non-timeout retryable, pending
identity change, completed identity change, and teardown). A separate exact
eight-case matrix covers Poll/Wait across directional Published, same-device
Published, Ready, and other entry phases. The public owning model and its
private Published authority have structural non-`Clone` checks, and one
compile-fail doctest rejects cloning the owning model. Verus mathematical
carriers remain copyable and establish no Rust ownership or borrowing fact.

Seven standalone pinned countermodels respectively accept a non-timeout retry
as timeout, skip the zero-deadline observation, omit published-index
restoration on timeout, substitute expected custody for the changed returned
owner, eagerly republish a Ready continuation, drop source-allocation custody
from Ready, and lose current-stream membership from Ready. Each imports no
positive proof source and fails its named postcondition.

All error classes, identities, counts, frames, storage tokens, deadlines,
completion dispositions, and native outcomes are contracted mathematical
inputs. The model excludes production/public error text and error identity,
real map/vector contents, allocation bytes, queue and driver state, actual
native owner validity, native completion truth, hardware clocks, coherence,
progress, liveness, and performance. There is no Rust-to-Verus correspondence
theorem and no proof that the executable model or production Rust refines the
Verus machine.

## R37 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Typed-timeout exclusivity, one abstract wait observation, exact timeout restoration, exact non-timeout/identity-change/teardown terminal custody, success-only settlement or Ready continuation, and exact modeled storage/dependency/allocation/stream coordinates | **Proved** | Fifteen obligations in `r37_typed_native_sdma_wait_activation_v1.rs`; contracted finite observations and mathematical coordinates only. |
| Poll and wait route selection | **Proved** | Explicit Poll remains Poll; only Published directional/same-device waits select native wait. This proves route selection, not unchanged route internals. |
| Executable typed-wait model | **Checked** | Seven focused Rust tests, including exactly 56 legal transition cases, an exact eight-case route matrix, structural move-only checks, and one compile-fail non-`Clone` doctest. |
| Boundary countermodels | **Rejected** | Seven pinned mutations fail non-timeout classification, zero-deadline observation, timeout restoration, identity custody, continuation-publication, source-custody, and stream-currentness postconditions. |
| Production/public error identity, real time or physical event count, actual map/vector contents, concrete native ownership or completion, Rust privacy or borrow semantics, executable or production Rust refinement, runtime/KFD/HSA/HIP, driver, firmware, hardware, coherence, progress, liveness, parity, or performance | **Not established** | Explicitly outside the R37 proof boundary. |

## R38 bounded persistent-compute wait and recycle

`r38_bounded_persistent_compute_wait_recycle_v1.rs` proves 19 obligations for
one abstract Published persistent-compute dispatch. It is designed against
signed production commit `a1ea30cffbd24a5714a5fe0318b4231f42e98727`. The input contains an exact
lane/submission/stream/allocation/module/dependency/event and queue occurrence,
the relevant generations and completion marker, positive retain counts, a
finite `Pending^n` prefix, one R36 fused poll/recycle terminal result, a
deadline boundary, an observation maximum, and queue presence. All are
contracted mathematical observations rather than facts obtained from Rust or a
native queue.

The R38 route projection begins only after the earlier R37 active-SDMA guard
has not matched. Its `Other` phase denotes another entry in that residual
compute-routing domain and excludes published directional or same-device SDMA
operations, which use the R37 native route.

The constant-time transition observes the R36 composition before checking a
deadline after Pending. Zero deadline therefore produces exactly one
observation. A finite maximum terminates without an increment beyond the
maximum. Timeout restores the exact Published active execution, active lane and
submission, lane/submission and lane/stream indexes, `ComputeInFlight` storage,
module/dependency/event retain counts, allocation owner count and current
owner, stream tail and current owner, completion reservation, absent completion
record, and unique Published authority. Reaching Ready composes through the
R36 Recycled result and records the supplied midpoint only after the Ready
observation. A lower KFD foreign poll or recycle preflight returns retryable
Published or Completed custody without poisoning the lower queue; the modeled
runtime handler removes its active state, retains that exact custody, and enters
its terminal state. These preflights are defensive contracted inputs for a
foreign/phase mismatch and their inclusion proves no production reachability,
including after a nonempty Pending prefix. The eleven internal failure stages
return ProcessTeardown authority with an exact Published, Completed, or
Recycled retained-native stage and opaque nonzero token; in particular,
DispatchRecycle retains Recycled internally and is never modeled as returned
retryable Recycled custody. Missing queue retains Published authority without
claiming an observation. Every result has exactly one stage authority.

The executable model checks exactly 756 model-admitted contracted present-queue
cases: two identity/count profiles, three deadline boundaries, three observation
maxima, three Pending-prefix lengths, and 14 R36 terminal results (Recycled, two
exact lower retryable preflights consumed by the runtime handler, and
ProcessTeardown forms of all eleven internal failures). Two missing-queue
profiles and an exact eight-case Poll/Wait route matrix are checked separately.
The route matrix proves only selection within that residual domain: Poll
remains Poll, Published persistent wait selects the bounded route, and
Prepared, Materialized, and scoped Other waits retain the legacy poll fallback.
A structural source check and two compile-fail doctests confirm that the
executable owner contains one private Published authority, is not cloneable,
and is consumed by one transition attempt. This is affine, not linear,
ownership: explicit drop and invalid limits/script errors do not preserve or
prove custody. Verus mathematical carriers are copyable, and every R38 state
transition theorem assumes valid contracted binding, limits, and script inputs.
Neither fact establishes Rust ownership behavior outside those boundaries.

Six standalone pinned countermodels respectively skip the zero-deadline first
observation, lose the lane/submission index on timeout, substitute Completed
for the internally retained Recycled stage of DispatchRecycle ProcessTeardown, drop missing-queue Published authority, continue
past a Pending deadline into eager recycle, and increment beyond the observation
maximum. Each imports no positive proof source and fails its named
postcondition.

## R38 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| First observation before zero deadline; exact finite Pending-prefix observation count; deadline/maximum termination | **Proved** | Nineteen obligations in `r38_bounded_persistent_compute_wait_recycle_v1.rs`; natural-number bounds and contracted observations only. |
| Timeout restoration | **Proved** | The timeout theorem explicitly enumerates binding, route, result, terminal status, active execution, lane/submission and lane/stream indexes, allocation storage, module/dependency/event retains, allocation ownership/current owner, stream tail/current owner, completion reservation, submission-record absence, observation/composition count, midpoint/Ready/recycle flags, and all four authority counts. |
| Ready, lower retryable preflights, and all eleven internal failure stages | **Proved** | Ready explicitly enumerates every state coordinate through an R36 Recycled result. One symbolic preflight theorem proves that the runtime handler consumes lower retryable Poll/Recycle owners into terminal retained Published/Completed custody, removes active execution, and poisons the runtime. The resulting runtime custody is neither retryable nor publicly returned. One symbolic ProcessTeardown theorem enumerates every coordinate for all eleven internal stages, including exact retained-native Published/Completed/Recycled stage, midpoint visibility, opaque token, and unique teardown authority. DispatchRecycle is internal Recycled retention, never retryable Recycled return. |
| Missing queue | **Proved** | The missing-queue theorem explicitly enumerates every coordinate, zero observations, no R36 invocation, terminal state, and one retained Published authority. |
| Poll, Prepared, Materialized, and scoped Other behavior | **Route selection proved** | Within the residual compute-routing domain after the R37 active-SDMA guard misses, Poll remains the Poll route and Prepared, Materialized, and Other waits retain legacy polling. Published directional and same-device SDMA waits are outside this R38 route domain and use the R37 native route. Internals of the unchanged routes are not modeled. |
| Executable bounded-wait model | **Checked** | Seven focused Rust tests cover 756 model-admitted contracted present-queue cases, two missing-queue profiles, eight route cases, input rejection, maximum termination, structural affine-owner checks, and compile-fail checks for cloning and transition replay. The owner is consumed by one attempt; explicit drop and invalid limits/script errors do not preserve or prove custody. The defensive retryable-preflight cases are not a reachability proof. |
| Boundary countermodels | **Rejected** | Six pinned mutations fail zero-observation, timeout lane/index, stage custody, missing-queue authority, eager recycle, and observation-overflow postconditions. |
| Production/public error type or identity, real `Instant` or sleep/backoff behavior, physical event indices, concrete map/vector contents, actual native ownership/completion, Rust privacy/borrowing/drop/panic behavior, custody preservation across invalid executable inputs or explicit drop, executable or production Rust refinement, runtime/KFD/HSA/HIP, driver, firmware, hardware, coherence, progress, liveness, timing, parity, or performance | **Not established** | Explicitly outside the R38 proof boundary. The finite maximum is a model bound, not a proof about production `u64::MAX`. |

## R39 scoped persistent-SDMA wait policy

`r39_scoped_persistent_sdma_wait_policy_v1.rs` proves 20 obligations for one
abstract completion observation and subsequent wait-policy decision. Commit
`4be5243dbe835c94618a62b3702f9624cd8f9d1f` was a human design-review reference,
not an authenticated proof input or a refinement link. The runner pins the R39
positive proof and standalone countermodel sources; it does not pin or
authenticate a production commit or production Rust files. The aggregate
transcript records runner results and likewise does not authenticate production
code. Nanosecond positions, the completion observation, and the complete R37
wait snapshot are contracted mathematical inputs rather than values obtained
from Rust, a clock, or a native queue.

The profile selector admits the exact 50,000ns elapsed active-spin floor only
for directional persistent single, directional persistent window, and
same-device persistent window waits. Generic persistent single, ordinary
single, ordinary batch/striped, fused synchronous directional, XGMI single,
XGMI batch, and persistent-compute waits retain the default policy. The default
constants are exactly 64 spin attempts, 16 yield attempts, 25,000ns initial
sleep, and 1,000,000ns maximum sleep.

The modeled floor endpoint uses checked addition: an overflow falls back to the
deadline and an in-range endpoint is clamped to the deadline. Completion is
observed before deadline classification, so Ready wins even when the deadline
has expired and an expired Pending result records one observation without a
pause action. The deadline check and action selection use ordered, separately
contracted time samples. A Pending result that was live at the deadline-check
sample increments or saturates the attempt counter before selecting an action
at the later sample. If the deadline passes between samples, default action
selection proceeds with zero saturating remaining duration; it is not
retroactively classified as a timeout. The floor comparison is strict; at the
exact floor boundary the default adaptive stage resumes. Sleep is bounded by
saturating remaining deadline duration, and exponential backoff is capped at
the maximum. A cursor start after an already computed deadline is admitted.

The executable model checks exactly 156 cases: 13 distinct public R37 snapshots
covering both copy kinds and initial, timeout, success, continuation, identity,
retryable, and teardown outcomes, crossed with 12 Ready/Pending, deadline,
start-after-deadline, between-sample deadline passage, floor, adaptive-stage,
sleep-clamp, saturation, and overflow scenarios. The output explicitly compares
every public R37 snapshot coordinate. Separate tests check the exact three-site
allowlist, seven exclusions, checked-add/clamp and strict-boundary behavior,
zero-deadline ordering, ordered sample and sleep input rejection, and private
move-only owner structure. One compile-fail doctest rejects cloning the owner.

Ten standalone pinned countermodels respectively substitute 25us for the floor,
omit an allowlisted route, activate an excluded route, wrap checked addition,
omit deadline clamping, make the floor boundary inclusive, omit attempt
counting, check the deadline before observation, lose a timeout stream-frame
coordinate, and substitute Ready custody/continuation state. Each imports no
positive proof source and fails its named postcondition.

## R39 claim matrix

| Surface | Status | Exact boundary |
| --- | --- | --- |
| Three-site profile selection and seven exclusions | **Proved** | The selector proves the exact 50,000ns profile for the three named persistent-SDMA sites and Default for all seven named exclusions. The proof does not establish that production dispatch reaches the abstract site supplied to the model. |
| Checked floor endpoint and adaptive wait action | **Proved** | Checked-add overflow fallback, deadline clamp, strict floor comparison, default spin/yield stages, saturating attempt increment, saturating remaining-deadline sleep bound, and maximum backoff are mathematical properties over contracted natural-number times. |
| Observation-before-deadline behavior | **Proved** | Ready wins at an expired deadline; expired Pending records one observation and no action. Ordered deadline-check and action samples admit deadline passage between samples, which resumes default action with zero remaining duration rather than retroactive timeout. No physical completion or clock property is established. |
| Full R37 snapshot preservation | **Proved** | Every public R37 binding, route, outcome, active phase, index, storage, retain, custody, terminal, observation, settlement, completion, and continuation coordinate is mirrored and retained for timeout, Ready, and general steps. |
| Executable finite model | **Checked** | Six focused Rust tests cover 156 snapshot/scenario cases, exact route scope, boundaries, zero deadline, start-after-deadline, between-sample deadline passage, ordered-input rejection, and private owner structure. The executable model is not proved to refine the Verus model or production Rust. |
| Boundary countermodels | **Rejected** | Ten pinned independent mutations fail floor, route, overflow, clamp, boundary, attempt, observation, timeout-custody, and Ready-custody/continuation postconditions. |
| Concrete `Instant`, CPU spin/yield/sleep execution, Rust-to-Verus or production refinement, native queue ownership/completion, runtime/KFD/HSA/HIP, driver, firmware, hardware, coherence, progress, liveness, timing, parity, or performance | **Not established** | Explicitly outside the R39 proof boundary. Source-structure and nonhardware Rust tests are checks, not refinement proofs. |

The projection proof establishes the mathematical relation implemented by the
pure canonical-record mapping; it is not a proof that the executable Rust
implements that relation, nor that the adapter observed truthful kernel data.
The lifecycle, memory, and queue files prove abstract transition relations, not
refinement of `src/model.rs`, `src/device_identity.rs`,
`src/memory_lifecycle.rs`, `src/queue_lifecycle.rs`, or
`src/r12_native_concurrency.rs`, `src/r13_logical_scheduler.rs`, or
`src/r14_async_observer.rs`. All
receipts remain model-only and are not production device authority. A later
sealed adapter refinement must
authenticate the KFD topology, DRM render, partition, schema, and process XNACK
observations, bind the dynamically allocated KFD device node to the opened file
descriptor and sysfs device, and connect concrete ioctl/sysfs results to the
canonical record. `DeviceGenerationV1` is a software admission
incarnation for stale-token rejection; topology correlation does not detect or
attest a GPU reset. The reset booleans and wrapping VRAM-loss value are retained
contracted observations only; these proofs do not establish an all-reset
generation, ABA freedom, or correctness of the KFD event stream. Firmware
execution, hardware completion, progress, liveness, coherency, performance, and
absence of kernel/firmware defects remain outside this proof boundary. The R2
proofs do not establish executable-Rust refinement, VA reservation or native
allocation success, KFD `n_success` truth, syscall rollback, CPU/GPU coherence,
page-table state, or quiescence. An adapter must turn malformed or uncertain
side-effecting results into the model's unreleasable ambiguous state. The model
also does not prove that a copied R1 admission token is still active in a
separately evolved `DeviceIdentityStateV1`; that state-composition refinement is
required before a production adapter can consume the memory transitions.

The R4 model is bounded to sixteen queue incarnations, 256 append-only history
entries, and exactly four mapped resource roles. Per-role memory kind,
coherence, and access expectations are explicit plan inputs checked against the
R2 memory state; this proof does not assert that those policy choices are KFD,
gfx942, or ROCr hardware truths. The unchanged CREATE output sentinel and a
caller-labeled returned sentinel are both rejected, while zero is a valid
returned ID. Executable publications carry structural Generic versus exact
ComputeAqlQueue ownership; the generic public memory transition cannot release
the latter. CreatePending and unknown ambiguous CREATE results poison later
CREATE for the process lifetime of this model, and known ambiguous IDs remain
collision-retaining. No model value creates or owns a
native KFD queue. The proof does not establish executable-Rust
refinement, ioctl truth or atomicity, queue-ID ownership, target resource sizes,
doorbell mapping or arithmetic, executable AQL packet publication, completion,
quiescence, firmware execution, liveness, or performance. Those remain adapter,
target profile, dispatch, and hardware-refinement obligations.

The R7 resource proof uses mathematical identities, phases, natural numbers,
and finite sequences. The executable pool model separately exercises best-fit
reuse, generation advance, completion-gated release, quarantine retention, and
capacity rejection, but it is tested Rust rather than Verus-refined Rust. The
native gfx942 SDMA implementation additionally freezes the reviewed queue and
packet manifests and checks exact packet bytes. KFD ioctl truth, doorbell MMIO,
CPU/GPU coherence, firmware consumption, native multi-device isolation,
completion, progress, liveness, and performance remain contracted or measured,
not proved.

The R3 load-plan and materialization proofs establish only the stated
mathematical relations over already-formed abstract values. They do not prove
that `fe2o3-amdhsa-loader::plan` implements those relations or that its
untrusted ELF byte parser constructs the modeled records. Given a canonical
abstract plan and exact-length mathematical input sequence, the materialization
proof constructs the zero and three copy states rather than assuming a final
image relation. Those sequences and transitions are still mathematical values,
not evidence that executable Rust performed them. In particular, the proof does
not refine `ValidatedEnvelope`, `MaterializationPlan`, slice identity, a CPU or
GPU copy, allocation identity, or any syscall to the abstract operation. It does
not decode or verify metadata or symbols, execute relocations, authenticate
content, establish W^X transitions, or prove materialization on a GPU. Separate
executable parser/copy/syscall refinement and loaded-image proofs remain
required before any `loader_refined` authority claim. There is also no theorem
connecting the materialization plan to the separately proved load-plan profile,
so the materialization proof alone establishes neither 4 KiB alignment nor page
rounding.
