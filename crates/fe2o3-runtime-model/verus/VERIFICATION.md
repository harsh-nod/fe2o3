# Runtime model verification

This directory contains the issue #137 Verus specifications and the additive R7
asynchronous-resource, R8 execution-contract, R9 native-evidence, R10 closed
execution-composition, R11 runtime-semantics, and R12 native-concurrency models.
The authenticated runner proves 142 obligations and rejects 92
expected-negative mutations over finite abstract values and traces. The
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
queue recreation without occurrence advance. The launcher
rejects source substitution, lexically audits all proof files for trusted
constructs, clears the environment, bounds execution time, pins Z3 through the
authenticated Verus release closure, and rechecks the authenticated inputs after
verification.

The projection proof establishes the mathematical relation implemented by the
pure canonical-record mapping; it is not a proof that the executable Rust
implements that relation, nor that the adapter observed truthful kernel data.
The lifecycle, memory, and queue files prove abstract transition relations, not
refinement of `src/model.rs`, `src/device_identity.rs`,
`src/memory_lifecycle.rs`, `src/queue_lifecycle.rs`, or
`src/r12_native_concurrency.rs`. All
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
