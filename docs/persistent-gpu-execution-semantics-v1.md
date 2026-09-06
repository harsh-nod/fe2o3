# Persistent GPU Execution Semantics V1

Status: issue #135 P0 semantic model and architecture decision. This document
defines no executable lowering and grants no artifact, proof, launch, or runtime
authority.

## Authority And Scope

This document is the P0 contract for finite megakernel fusion and persistent GPU
services. It is a child of issue #134 D9 and extends only the architecture and
claims made available by #134 D0-D7. In particular, it preserves:

- Rust as the only source language and the one-executable-body rule;
- fe2o3-owned canonical identities independent of Pliron text, contexts, arena
  allocation, traversal order, and printer output;
- structured algorithm semantics separated from schedule and target choices;
- proof sidecars separated from executable IR;
- independent property claims with the #134 evidence statuses;
- explicit Rust/MIR, KIR, target, artifact, runtime, and hardware boundaries.

The definitions below are requirements for later implementation. They do not
assert that #134 D0-D7, issue #106, issue #107, or issue #135 P1-P5 have exited.
All illustrative records are semantic notation, not accepted Rust, Pliron, KIR,
LLVM, or AMDGPU syntax.

The following are outside this P0 authority:

- executable queue, scheduler, fusion, persistent-worker, or AMD lowering;
- a new source language or a new megakernel/persistent dialect;
- an open task interpreter, dynamic device-code loading, arbitrary task tags,
  or arbitrary function pointers;
- multi-device service semantics, preemption guarantees, or recovery from a
  physical device fault;
- unconditional workgroup fairness, starvation freedom, deadlock freedom,
  service termination, latency, or performance;
- HSACO-level `machine_refined` evidence.

An implementation must reject proof-required admission when a task variant,
queue or lifecycle transition, primitive, scope/order pair, failure event,
cancellation outcome, or progress assumption is absent from its named,
versioned model.

## Architecture Decision

Fusion and persistence are orthogonal plan dimensions:

```text
FusionDimension =
    Unfused
  | FiniteFusion(FusionPlanId)

ExecutionDimension =
    FiniteDispatch
  | PersistentService(PersistentPlanId)
```

Their combinations have these meanings:

| Fusion | Execution | Meaning |
|---|---|---|
| `Unfused` | `FiniteDispatch` | The admitted graph runs as separate finite dispatches. |
| `FiniteFusion` | `FiniteDispatch` | One finite megakernel implements the selected bounded graph subgraph. |
| `Unfused` | `PersistentService` | Resident workers repeatedly execute one closed homogeneous or heterogeneous task family without graph fusion being implied. |
| `FiniteFusion` | `PersistentService` | Resident workers repeatedly execute finite fused handlers or phases from a closed task family; this is a persistent megakernel. |

`FiniteFusion` describes the bounded semantic extent of one invocation. It does
not describe how long a kernel remains resident. `PersistentService` describes
repeated acquisition and execution under a lifecycle protocol. It does not
imply heterogeneous handlers or fusion. Every acquired handler invocation is
finite under the handler contract even when the service intentionally waits for
future tasks indefinitely.

A fusion choice may not silently select persistence, and a persistence choice
may not silently add fusion. Each dimension is canonicalized independently and
all identities and evidence affected by either choice are invalidated when that
choice changes.

### One-body rule

A handwritten finite or persistent kernel remains one ordinary authenticated
`#[kernel]` Rust body. A compiler-fused megakernel is a derived implementation
of an authenticated `dispatch.*` graph, not a second source implementation. Its
receipt must map every fused phase, value, effect, materialization boundary, and
source origin back to the authoritative graph.

Proof harnesses, entry shims, ABI support, and host descriptors may be derived
around that body under #134 D0-D3. They may not contain a second algorithm body.
Proof erasure and executable-body correspondence remain governed by #134 D6.

## Canonical Identities

Every identity in this section is the digest of a domain tag, schema version,
and canonical encoding of all listed inputs. Canonical encodings use explicit
field tags and lengths, canonical enum discriminants, canonical integer widths
and byte order, and deterministic ordering for maps and sets. An identity may
not include a process address, allocation address, Pliron identity, display
string, traversal order, filesystem accident, or nondeterministic map order.

The digest algorithm and wire encoding are selected by the #134 canonical
identity contract. This document defines identity inputs, not a competing wire
format.

### `TaskSchemaId`

`TaskSchemaId` binds a closed task family:

```text
TaskSchemaId = canonical_id(
  task-schema-v1,
  schema_version,
  ordered_variants = [
    (canonical_tag,
     variant_name_identity,
     payload_FnAbi_and_layout,
     payload_lifetime_and_region_contract,
     handler_AlgorithmId,
     handler_numerical_contract_id,
     handler_preconditions_and_postconditions,
     handler_effect_and_capability_closure,
     cancellation_points_and_outcomes,
     unsafe_or_external_obligations)
  ],
  unknown_tag_policy = reject,
  schema_failure_policy_id
)
```

Variant tags are unique and explicit. Source declaration order is an input only
when the schema says it is canonical; otherwise variants are ordered by their
canonical tags. Changing a tag, ABI, padding/layout fact, handler algorithm,
contract, cancellation behavior, or unsafe obligation changes `TaskSchemaId`.
Unknown or duplicate tags reject.

### `SchedulerModelId`

`SchedulerModelId` binds abstract scheduling behavior rather than a convenient
implementation name:

```text
SchedulerModelId = canonical_id(
  scheduler-model-v1,
  queue_model_and_version,
  queue_capacity_and_generation_domain,
  queue_discipline_and_batch_policy,
  delivery_policy,
  dependency_and_completion_epoch_semantics,
  lifecycle_and_admission_policy,
  cancellation_policy_id,
  failure_model_id,
  synchronization_contract_id,
  progress_contract_id_or_none
)
```

Queue hierarchy, stealing, role specialization, retry, or chunking is not
covered by another scheduler merely because it exposes the same host API. Such
a change receives a new scheduler identity and requires refinement to the same
abstract task semantics.

### `FusionPlanId`

`FusionPlanId` binds one finite graph transformation:

```text
FusionPlanId = canonical_id(
  fusion-plan-v1,
  authoritative_dispatch_graph_id,
  fused_node_and_edge_ids,
  phase_partition_and_order,
  materialized_values_and_boundaries,
  value_effect_dependency_and_origin_map,
  layout_and_region_choices,
  barrier_and_convergence_contracts,
  numerical_order_or_reassociation_contract,
  schedule_parameters,
  legality_rule_set_id,
  transformation_receipt_schema_id
)
```

The node set is encoded in authoritative graph order plus stable node identity,
not pass visitation order. A boundary, phase, materialization, effect, layout,
barrier, numerical-order, or schedule change produces a different identity.
The identity does not itself prove graph refinement.

### `PersistentPlanId`

`PersistentPlanId` binds one service plan:

```text
PersistentPlanId = canonical_id(
  persistent-plan-v1,
  TaskSchemaId,
  SchedulerModelId,
  worker_topology_and_role_partition,
  worker_and_workgroup_state_partition,
  resident_worker_and_wave_requirements,
  queue_and_state_resource_plan,
  launch_mode_and_cooperative_requirements,
  handler_or_fusion_plan_references,
  drain_stop_and_failure_policy_ids,
  resource_contract_id
)
```

The task schema and scheduler are separate inputs so handler-family changes do
not masquerade as scheduler changes. If a handler is a finite fused graph, its
algorithm/refinement contract binds the applicable `FusionPlanId`; persistence
does not make that fusion evidence reusable automatically.

### Service identities

```text
ServiceExecutableId = canonical_id(
  service-executable-v1,
  PersistentPlanId,
  TargetPlanId,
  launch_contract_id,
  compiler_and_toolchain_ids,
  LLVM_module_id,
  object_id,
  HSACO_identity,
  resource_and_origin_map_ids
)

ServiceRunId = canonical_id(
  service-run-v1,
  ServiceExecutableId,
  physical_device_identity,
  runtime_context_identity,
  stream_or_queue_identity,
  service_epoch,
  queue_allocation_identity_and_epoch,
  state_allocation_identity_and_epoch,
  input_output_allocation_identities_and_epochs,
  launch_instance_identity
)
```

The executable fields are reserved identity inputs for P4; P0 does not produce
them. Run identities use authenticated allocation identities and epochs, not
raw addresses. Reusing storage under a new allocation or service epoch changes
`ServiceRunId` even when every address happens to be unchanged.

### Evidence and invalidation

Every property statement also binds its #134 `ProofInputId`, property schema
version, statement digest, semantic model package, covered compiler boundary,
and all applicable identities above. At minimum, these changes invalidate the
listed evidence:

| Changed input | Evidence invalidated |
|---|---|
| Task tag, ABI, handler, or handler contract | Task schema, persistent plan, executable, run, handler composition, accounting, and affected functional/numerical evidence |
| Queue, delivery, dependency, cancellation, failure, or progress semantics | Scheduler, persistent plan, executable, run, and every scheduler/service property |
| Fused node, phase, effect, boundary, layout, or numerical order | Fusion plan and all phase, graph-refinement, numerical, resource, and performance evidence |
| Worker topology, state partition, launch mode, or resource contract | Persistent plan, executable, run, residency, progress, and performance evidence |
| Target, compiler, LLVM, object, HSACO, or post-link resource metadata | Executable and all artifact-bound, machine, residency, progress, and performance evidence |
| Device, context, service epoch, or allocation epoch | Run-bound runtime observations, tickets, leases, and completion records |
| Model, theorem, solver, checker, validator, or trusted assumption | Every property record that names it |

Identity equality establishes sameness of canonical inputs only. It does not
establish legality, proof, refinement, visibility, progress, or performance.

## Abstract Service State

The authoritative P0 state is a labeled transition system. A service state has
at least the following components:

```text
ServiceStateV1 = {
  run_id: ServiceRunId,
  lifecycle: LifecycleState,
  admission_cutoff: Optional<SubmissionSequence>,
  queue_slots: SlotId -> SlotState,
  logical_generation: SlotId -> Nat,
  encoded_generation: SlotId -> GenerationWord,
  tasks: TaskId -> TaskRecord,
  leases: LeaseId -> LeaseState,
  workers: WorkerId -> WorkerState,
  dependencies: TaskId -> Set<DependencyEpoch>,
  phase_epochs: RegionId -> PhaseEpoch,
  completion_records: TaskId -> CompletionRecord,
  memory_events: Sequence<MemoryEvent>,
  visibility: Observer -> Set<MemoryEvent>,
  failure: Optional<FailureRecord>
}
```

State not present in this record may be introduced only by a versioned model
extension with an abstraction/refinement map. Concrete queue encodings may use
different representations, but their accepted traces must refine this state
machine at named linearization points.

### Global invariants

Every reachable state must satisfy all of the following independently of any
fairness assumption:

1. The `ServiceRunId`, service epoch, queue identity, and state allocation epoch
   are immutable during a run.
2. Each accepted task is associated with exactly one current task record and at
   most one queue slot key at a time.
3. A slot key is `(run, queue, slot, logical_generation)`. Every ticket, lease,
   task reference, and completion record that names a slot names that full key.
4. At most one live lease exists for a slot key, task, and acquisition event.
5. A published payload is fully initialized, ABI-valid, lifetime-valid, and
   release-published before any successful acquire may expose it.
6. A worker executes only the handler selected by the authenticated task tag
   in `TaskSchemaId` and only after the task dependencies are visible as
   satisfied.
7. Completion consumes the live lease once and publishes handler outputs and a
   completion record before the slot becomes reusable.
8. Slot reuse advances logical generation and cannot make an old ticket,
   lease, cancellation request, or completion record current again.
9. Immutable, worker-local, workgroup-local, and synchronized shared service
   regions do not overlap in ways forbidden by their region contracts.
10. Queue, state, payload, input, and output storage remains borrowed until an
    admitted stopped or device-quiesced failure outcome permits release.

## Lifecycle State Machine

```text
LifecycleState =
    Starting
  | Running
  | Draining
  | Stopping
  | Stopped
  | Failed(FailureDisposition)

FailureDisposition = DeviceMayStillAccess | DeviceQuiesced
```

Allowed lifecycle edges are:

```text
Starting -> Running
Starting -> Failed
Running  -> Draining
Running  -> Stopping
Running  -> Failed
Draining -> Stopping
Draining -> Failed
Stopping -> Stopped
Stopping -> Failed
```

`Stopped` and `Failed` are terminal service outcomes. A failure observer may
refine `DeviceMayStillAccess` to `DeviceQuiesced` only through a named runtime
event and failure contract; this is not a lifecycle restart.

Lifecycle transitions have these semantics:

| Transition | Required condition and effect |
|---|---|
| `start` | Resources and identities are fixed; no task is accepted until worker launch and service publication establish `Running`. |
| `begin_drain` | Atomically changes `Running` to `Draining`, records an admission cutoff, and rejects later submissions. It does not imply completion. |
| `drain_ready` | Holds only when every accepted task at or before the cutoff has an allowed visible terminal outcome and no reservation, published task, live lease, executing handler, or unpublished completion effect remains. |
| `begin_stop_after_drain` | Requires `drain_ready`; requests worker exit and enters `Stopping`. |
| `begin_direct_stop` | Uses a named non-draining stop/cancellation policy that classifies every outstanding task. It makes no graceful-drain claim. |
| `stop_observed` | Requires every worker exited, required device-to-host completion visibility, and no possible device access to retained storage; enters `Stopped`. |
| `fail` | Records the named runtime/failure event, closes admission, and classifies outstanding operations according to `FailureModelId`. `Failed` alone does not imply device quiescence or safe storage release. |

`drain_ready`, worker exit, kernel termination, host observation of termination,
and safe release of storage are distinct predicates. A service can be correct
while `Running` and idle forever. A graceful drain theorem does not imply that
arbitrary direct stop or failure paths are graceful.

## Task State Machine

Only a successful acceptance event brings a task into the accounting domain.
A host-side proposal rejected before acceptance is not an accepted task.

```text
TaskState =
    Accepted
  | Reserved(SlotKey)
  | Initialized(SlotKey)
  | Published(SlotKey)
  | Acquired(LeaseId)
  | Executing(LeaseId, HandlerPhase)
  | CompletionPending(LeaseId, Outcome)
  | Completed(CompletionRecord)
  | Cancelled(CancellationStage, CompletionRecord)
  | Failed(TaskFailureOutcome)
```

Normal successful execution follows:

```text
Accepted -> Reserved -> Initialized -> Published -> Acquired -> Executing
         -> CompletionPending -> Completed
```

The task record binds its canonical tag, immutable payload identity, payload
regions and lifetimes, dependency epochs, accepted submission sequence, slot
key, and permitted outcomes. A handler may not reinterpret the payload under a
different tag or ABI.

Task cancellation is stage-sensitive:

| Stage | Required semantic outcome |
|---|---|
| Before acceptance | The proposal is rejected; no accepted-task accounting claim applies. |
| `Reserved` or `Initialized` | Cancellation may win before publication, produce one visible cancelled outcome, and move the slot toward reclaim without exposing the payload to a worker. |
| `Published` | Cancellation races with acquire at named linearization points. If withdrawal wins, no lease may issue. If acquire wins, cancellation becomes a request governed by the handler contract. |
| `Acquired` or `Executing` | The host cannot reclaim or invalidate payload storage. Only an admitted cooperative cancellation point may produce a cancelled terminal outcome, and the lease is still consumed exactly once. |
| `CompletionPending` | Completion publication and cancellation ordering decide one terminal record; they cannot both produce terminal outcomes. |
| `Completed`, `Cancelled`, or `Failed` | Cancellation is too late or idempotently observes the existing terminal record; it cannot rewrite history. |

Cancellation support is per variant. A variant without authenticated
cancellation points runs to its normal terminal outcome after acquire unless a
named service failure intervenes. Cancellation safety does not imply task
progress or graceful drain.

## Queue Slot State Machine

For logical generation `g`, a slot follows this normal cycle:

```text
Empty(g)
  -> Reserved(g, task)
  -> Initialized(g, task)
  -> Published(g, task)
  -> Acquired(g, task, lease)
  -> Executing(g, task, lease)
  -> Completed(g, task, outcome)
  -> Reclaimable(g, task, outcome)
  -> Empty(g + 1)
```

The transitions mean:

| Operation | Preconditions | Abstract effect and linearization |
|---|---|---|
| `reserve` | Lifecycle is `Running`; slot is `Empty(g)`; capacity permits acceptance. | Creates the accepted task and `Reserved(g, task)`. Successful reservation is the acceptance linearization point. |
| `initialize` | Caller owns `Reserved(g, task)` and payload borrows are valid. | Writes the complete payload and changes the abstract slot to `Initialized`. This is not publication. |
| `publish` | Payload and dependency record are initialized. | Release-publishes them and changes the slot to `Published`. The publication marker is the enqueue linearization point. |
| `acquire` | Slot is `Published`; dependencies are satisfied and visible; lifecycle policy permits acquisition. | One worker wins ownership, observes payload through acquire visibility, creates one lease, and changes the slot to `Acquired`. The ownership operation is the dequeue linearization point. |
| `begin_execute` | Worker owns the unique current lease and handler tag is admitted. | Changes slot and task to `Executing`; no accounting completion is implied. |
| `complete` | Handler has an allowed result and owns the live lease. | Consumes the lease, release-publishes outputs and one completion record, and changes the slot to `Completed`. Completion publication is the terminal-outcome linearization point. |
| `mark_reclaimable` | Required completion observers and payload lifetimes permit reclamation. | Changes `Completed` to `Reclaimable`; no old reference becomes current. |
| `reclaim` | No live lease or current payload reference exists and ABA obligations hold. | Advances logical and encoded generation according to the generation model and changes the slot to `Empty(g + 1)`. |

Cancellation adds only the stage-specific edges defined above. Failure adds
only edges named by `FailureModelId`. No implementation may skip a semantic
state merely because its physical representation combines fields; it must
still exhibit a checked refinement to the corresponding abstract events.

## Lease State Machine

A lease is a proof-sensitive, non-forgeable capability branded by:

```text
LeaseKey = (
  ServiceRunId,
  service_epoch,
  queue_identity,
  SlotId,
  logical_generation,
  TaskId,
  acquisition_event,
  WorkerId
)
```

Its states are:

```text
Issued -> Executing -> Consumed(Completed | Cancelled | Failed)
```

An implementation may combine `Issued` and `Executing`, but successful acquire
creates exactly one logical lease and every terminal operation consumes it
exactly once. A lease is neither `Copy` nor cloneable, cannot be reconstructed
from queue bytes, cannot cross a service or generation brand, cannot be
forgotten under the admitted safe profile, and cannot be invalidated by a host
dropping a ticket or requesting cancellation.

A handler result without consumption of the current lease is not a completion.
Consumption of a lease without a permitted terminal record is not successful
completion. Exact-consumption MIR admission and controlled destruction depend
on #134 D1 and D5 and are P1 work, not established here.

## Generation And ABA State Machine

The semantic generation is an unbounded natural number. A physical queue may
encode it in a bounded word only through a named refinement:

```text
logical_generation: Nat
encoded_generation: logical_generation mod generation_modulus
```

For each slot:

1. Only `reclaim` advances logical generation.
2. Every operation compares the full current slot key in the semantic model.
3. A stale ticket, lease, cancellation, or completion event with generation
   `g` cannot affect generation `g + n` for any positive `n`.
4. Reuse waits until all current lease and payload-lifetime obligations are
   discharged.
5. Bounded encoding must prove that no live or replayable reference can be
   confused when the encoded word wraps, including integer overflow and ABA.
6. The service epoch prevents a reference from a prior service run from
   becoming current in a later run, even if slot and encoded generation match.

Legal capacity, generation width, maximum outstanding references, wrap rules,
and reset behavior are symbolic scheduler parameters. If their arithmetic
invariant is absent, overflows, or cannot exclude ABA, queue admission rejects;
testing wraparound does not upgrade that result to `Proved`.

## Worker, Dependency, And Phase State

Workers use the abstract states:

```text
Starting -> Idle -> Acquiring -> Running(TaskId, HandlerPhase)
                         ^             |
                         +-------------+
Idle | Acquiring -> Exiting -> Exited
any nonterminal state -> Failed
```

The loop back to acquisition occurs only after lease consumption. A worker may
remain idle or acquiring without violating safety. Worker exit is permitted
only by lifecycle policy and does not establish service termination until all
required workers and runtime completion events are observed.

A task is eligible only when every dependency epoch in its authenticated task
record has a visible satisfying completion. Handler output writes happen before
release publication of the completion epoch; a dependent acquire observes that
epoch with matching acquire visibility before it may observe the dependent
payload or execute. A dependency cycle, unknown epoch, stale service epoch, or
scope/order combination that cannot establish the edge rejects.

Handler phases carry immutable phase IDs and region epochs. Retained registers,
fragments, LDS, shared state, and output permits are owned by the declared
worker/workgroup and phase. Advancing an epoch invalidates prior-phase access.
Fusion or persistence cannot extend a region lifetime or reuse an epoch without
an explicit checked mapping and property evidence.

## Delivery And Failure Policies

`DeliveryPolicy` is part of `SchedulerModelId` and is interpreted only inside
its named `FailureModelId`:

| Policy | Required statement |
|---|---|
| `at_most_once` | For each accepted task, at most one handler execution owns a valid lease and at most one terminal outcome is published. Loss or a failed/indeterminate outcome is allowed only where the named failure model says so. |
| `exactly_once` | For each accepted, non-cancelled task covered by the admitted execution and failure preconditions, exactly one handler execution and one allowed visible terminal completion occur. Cancellation has exactly one policy-defined terminal outcome. |

`exactly_once` is not an unconditional physical-fault or process-crash claim.
Watchdog timeout, device reset, stream destruction, context loss, process exit,
driver/runtime failure, and loss of host observation must each be classified by
the failure model. An event that can leave execution or completion unknown caps
or invalidates `task_accounted` according to the exact statement; it may not be
silently treated as completion or safe retry.

The initial model has no transparent retry after acquire. A future retry model
must distinguish logical task outcome from physical attempts, issue a new
scheduler identity, and prove that handler effects are retry-safe. Recovery
from physical device faults remains outside this version.

## AMD Memory And Synchronization Assumptions

This section defines abstract obligations for a later AMD mapping. It does not
select LLVM intrinsics or AMD instructions.

Each correctness-relevant memory event records:

```text
MemoryEvent = {
  operation,
  address_space,
  location_or_region,
  atomicity_scope,
  availability_scope,
  visibility_scope,
  ordering,
  cache_policy,
  required_wait_or_completion,
  participating_agents,
  stable_source_and_semantic_origin
}
```

The named AMD model must distinguish wavefront, workgroup, supported cluster,
agent, and system scopes. Cluster scope is unavailable unless the target model
and launch contract explicitly support it. A scope must include all relevant
producers and consumers; workgroup scope cannot publish a global queue between
workgroups, and agent scope alone cannot establish host visibility.

Atomicity scope is independent of availability and visibility scope. An atomic
ownership update does not by itself make non-atomic payload writes available or
visible. A wait or barrier does not by itself establish missing publication
order. Cache policy and required completion operations are part of the target
contract, not performance-only annotations.

### Required happens-before edges

The abstract model requires these edges:

1. Producer payload and dependency writes happen before release publication of
   the `Published` marker.
2. Successful acquire of that marker happens before payload and dependency
   reads and makes the released writes visible to the acquiring worker.
3. Handler output writes happen before release publication of the completion
   record and dependency epoch.
4. A dependent worker's acquire of the completion epoch happens before reads
   of those outputs.
5. Device completion publication happens before the admitted host acquire or
   runtime completion observation that exposes results and permits release.
6. Host initialization and submission publication happen before a device
   acquire when the producer is the host.

The exact order, scope, make-available, make-visible, cache, and wait sequence
for each edge is selected by a versioned target primitive contract under #134
D7. A missing or unsupported mapping rejects P4 artifact admission.

LDS is workgroup-local unless a named target feature says otherwise. LDS phase
reuse requires compatible participant convergence, barrier/order semantics,
completion waits, and epoch advancement. An LDS barrier never publishes a
global queue to other workgroups or the host.

### Grid synchronization

An ordinary launch may not use a grid-wide barrier. A cooperative grid barrier
requires all of:

- an explicit target capability and cooperative launch mode;
- a participant set and convergence theorem;
- a simultaneous-residency bound derived from exact post-link resources;
- a launched grid no larger than that admitted bound;
- a named runtime and progress contract.

A producer/consumer or locking protocol that needs an unscheduled workgroup to
run is unsupported unless its progress contract establishes the necessary
residency and scheduling behavior. Occupancy estimates alone do not establish
this condition.

## Progress Model

All safety, linearizability, accounting-safety, cancellation-safety, and phase
ownership statements quantify over every permitted interleaving without a
fairness premise. Progress is an optional, separately named contract:

```text
ProgressContractV1 = {
  progress_model_version,
  target_and_device_identity,
  launch_mode_and_grid,
  exact_post_link_resource_identity,
  required_resident_workgroups_and_waves,
  cooperative_residency_margin,
  scheduler_fairness_granularity,
  eligible_worker_scheduling_assumption,
  atomic_RMW_forward_progress_assumption,
  handler_termination_preconditions,
  producer_and_host_runtime_service_assumptions,
  contention_and_interference_bounds,
  watchdog_timeout_reset_and_context_exclusions,
  drain_and_stop_environment_assumptions
}
```

No field may be replaced by an observation such as "worked in a stress test."
In particular:

- atomicity does not imply eventual success of a contended read-modify-write;
- residency does not imply fair scheduling among workers or tasks;
- barrier convergence does not imply deadlock freedom;
- an eligible task does not imply an available worker unless stated;
- per-task progress does not imply drain progress while submissions remain
  open;
- drain progress does not imply direct-stop progress or kernel termination;
- service progress does not imply a latency bound;
- hardware validation does not turn runtime or fairness assumptions into a
  machine-checked proof.

Without an exact progress contract, `service_progress` is `Unsupported`. When
target/runtime fairness or scheduling remains environmental, the claim is no
stronger than `Contracted`, even if formal reasoning proves progress from that
assumption. A target or post-link resource change invalidates the contract
rather than silently weakening the property.

## Theorem Families

Let `Traces(model, plan, initial)` be all finite and infinite traces admitted by
the named semantics package, and let `AcceptedBeforeCutoff(trace)` be tasks
whose acceptance linearized no later than the drain cutoff. Required theorem
families are instantiated per exact identity and property statement.

### T1 Queue safety

For every trace, slot accesses are in bounds and generation-current; payload
accesses have valid provenance, lifetime, ABI, initialization, and region
authority; and no stale ticket or lease affects a reused slot. This theorem has
no progress premise.

### T2 Publication and visibility

For every successful acquire, the named AMD memory contract establishes that
the worker observes the payload and dependency writes that happened before
publication. The corresponding completion theorem establishes visibility of
handler results and completion epochs to admitted device and host consumers.
It names order, atomicity scope, availability/visibility scope, cache, and wait
assumptions explicitly.

### T3 Queue linearizability

Projecting reserve, publish, acquire, completion, cancellation, and reclaim
events at their named linearization points yields a legal history of the
abstract bounded queue and cancellation policy. A safety proof of the concrete
memory accesses does not imply this theorem.

### T4 Task accounting

For every accepted task in the admitted failure model, the projected history
satisfies exactly its declared `at_most_once` or `exactly_once` statement and
excludes duplicate terminal records, lost covered tasks, lease duplication,
and generation confusion. This theorem does not imply handler correctness.

### T5 Dependency ordering

Every handler starts only after all declared dependency epochs are satisfied
and visible. Its completion is release-published before a dependent becomes
acquirable. The trace projection is a permitted topological/interleaved order
of the accepted dependency graph.

### T6 Handler refinement

For each closed variant, every terminating handler execution that satisfies its
preconditions refines its exact `AlgorithmId` and numerical contract. Handler
evidence is independently supplied by #134 D3-D7 and the applicable bounded
kernel-family work. Scheduler correctness does not prove this theorem.

### T7 Scheduler composition and refinement

The canonical composition rule is:

```text
queue_invariant(s0)
&& acquire(s0, lease, task, s1)
&& handler_contract(task, result)
&& complete(s1, lease, result, s2)
  ==> queue_invariant(s2)
   && one_permitted_task_transition(task, result)
```

Generic queue and scheduler lemmas are proved for symbolic legal capacity,
generation, worker, and schema parameters. Each handler theorem is composed at
its authenticated tag. A new task mixture instantiates this rule; it does not
create an unrelated flattened-switch proof.

### T8 Graph and phase refinement

For finite fusion, the receipt maps each graph node, value, dependency, effect,
materialization, numerical-order decision, region, and phase to the finite
megakernel. For persistent execution, retained state and phase epochs remain
within their declared ownership. A terminating finite invocation refines the
authoritative graph under the named contract. Neither fusion nor repetition
inherits source proof or performance evidence without this mapping.

### T9 Quiescence and shutdown

If graceful drain is reported, every task in `AcceptedBeforeCutoff` has one
allowed visible terminal outcome, and no reservation, published task, live
lease, executing handler, or unpublished completion effect remains. If stopped
is reported, all workers have exited and the runtime establishes no remaining
device access. This theorem does not establish eventual drain or stop.

### T10 Conditional progress

Under exactly the named progress, residency, atomic, handler-termination, host,
runtime, and failure assumptions, each eligible covered task reaches its
allowed terminal state and a closed graceful drain reaches `drain_ready`.
Per-task progress, drain progress, direct-stop progress, worker exit, and kernel
termination are separate instantiated statements. None is inferred when its
premises are absent.

### Top-level statement shape

Each theorem follows the #134 D6 shape:

```text
admitted(plan, launch, initial, model)
&& trace in Traces(model, plan, initial)
  ==> selected_safety_property(trace)
   && (selected_termination_condition(trace)
       ==> selected_refinement_postcondition(trace))
```

Progress theorems add `ProgressContractV1` as a visible premise. No theorem may
hide a target, runtime, fairness, failure, or termination premise in a generic
`admitted` boolean without a versioned witness or declared evidence status.

## Independent Property Claims

Issue #135 adds these property records to the independent #134 property matrix:

| Property | Exact boundary |
|---|---|
| `queue_safe` | Queue metadata and payload memory satisfy bounds, lifetime, ABI, initialization, generation, and race obligations. |
| `queue_linearizable` | Concrete accepted queue and cancellation operations refine the named abstract history at named linearization points. |
| `task_accounted` | Accepted tasks satisfy the exact delivery and failure policy statement. |
| `dependency_ordered` | Eligibility, execution, completion, and dependent visibility respect authenticated dependency epochs. |
| `phase_refined` | Finite fused and persistent phases refine graph/task semantics and preserve region/epoch ownership. |
| `quiescence_safe` | Reported drain or stop implies exactly the named quiescent state; no progress is implied. |
| `cancellation_safe` | Every admitted stage-specific cancellation preserves queue, lease, lifetime, and accounting invariants. |
| `service_progress` | Eligible work or a closed drain eventually advances only under the exact visible progress contract. |

Each property has exactly one #134 status: `Proved`, `Validated`, `Contracted`,
`Checked`, or `Unsupported`. Their meanings are unchanged. A property record
contains at least:

- property and evidence schema versions, exact statement, and statement digest;
- status and covered source/MIR, KIR, `amdgcn.*`, LLVM, object, or HSACO boundary;
- applicable graph, algorithm, schedule, fusion, task schema, scheduler,
  persistent plan, launch, target, executable, and run identities;
- memory, progress, cancellation, delivery, and failure model identities;
- theorem, Verus, solver, checker, compiler, validator, and artifact identities;
- preconditions, retained dynamic checks, trusted contracts, environmental
  assumptions, evidence references, and unsupported sub-properties.

The following implications are explicitly invalid:

```text
queue_safe          -/-> queue_linearizable
queue_linearizable  -/-> task_accounted
task_accounted      -/-> handler refinement or numerical correctness
dependency_ordered  -/-> service_progress
phase_refined       -/-> queue_safe or handler refinement
quiescence_safe     -/-> drain progress or kernel termination
cancellation_safe   -/-> cancellation progress
service_progress    -/-> kernel termination, latency, or performance
any performance result -/-> any correctness or progress property
```

The inherited #134 properties such as `memory_safe`, `race_free`,
`barrier_convergent`, `deadlock_free`, `functionally_refined`,
`numerically_bounded`, `deterministic`, and `machine_refined` remain separate.
There is no `persistent_verified` or unqualified `verified` badge. A source- or
KIR-level proof cannot issue HSACO-level `machine_refined` evidence.

## Deterministic P0 Examples

The symbolic names below stand for canonical digests of the fully expanded
records above. They are examples of identity inputs and theorem statements, not
evidence that an implementation exists.

### Finite fused MoE graph

```text
fusion_dimension = FiniteFusion(FusionPlanId {
  graph = MoeStepGraphId,
  nodes = [RouteNodeId, PermuteNodeId, GroupedGemmNodeId, CombineNodeId],
  phases = [route, materialize_expert_boundaries, expert, combine],
  materialized_values = [ExpertBoundaryValueId],
  numerical_order = MoeStepNumericalContractId,
  receipt_schema = FusionReceiptV1
})
execution_dimension = FiniteDispatch
```

Required independent statements include finite graph/phase refinement,
applicable handler functional and numerical properties, and finite-kernel
safety. No service lifecycle, queue, accounting, or progress property is
created by this plan.

### Persistent homogeneous GEMM service

```text
fusion_dimension = Unfused
task_schema = TaskSchemaId {
  variants = [(tag = 0, abi = GemmTaskAbiId, handler = GemmAlgorithmId)]
}
scheduler = SchedulerModelId {
  queue = BoundedQueueModelV1,
  delivery = AtMostOnce(AdmittedFailureModelId),
  dependencies = CompletionEpochModelV1,
  progress = None
}
execution_dimension = PersistentService(PersistentPlanId {
  task_schema,
  scheduler,
  workers = HomogeneousGemmWorkerPlanId,
  lifecycle = DrainThenStopV1
})
```

The initial `service_progress` statement is `Unsupported` because no progress
contract is present. Queue safety, accounting, quiescence, cancellation, and
GEMM handler refinement remain separate theorem/evidence records.

### Persistent heterogeneous inference service

```text
fusion_dimension = FiniteFusion(MixedInferenceHandlerFusionPlanId)
task_schema = TaskSchemaId {
  variants = [
    (tag = 0, abi = GemmTaskAbiId,      handler = GemmAlgorithmId),
    (tag = 1, abi = AttentionTaskAbiId, handler = AttentionAlgorithmId),
    (tag = 2, abi = MoeTaskAbiId,       handler = MoeAlgorithmId)
  ]
}
scheduler = SchedulerModelId {
  queue = BoundedGenerationRingModelV1,
  delivery = ExactlyOnce(GracefulRunFailureModelId),
  dependencies = CompletionEpochModelV1,
  progress = Gfx942NamedProgressContractId
}
execution_dimension = PersistentService(PersistentPlanId {
  task_schema,
  scheduler,
  workers = MixedInferenceWorkerResourcePlanId,
  lifecycle = DrainThenStopV1
})
```

This example requires one generic scheduler theorem, three independently bound
handler theorems, and three composition instantiations. The symbolic gfx942
progress contract is an input to a future statement, not a claim that its
residency, fairness, runtime, or artifact assumptions have been established.
Changing the variant set or any handler contract changes the task schema and
all dependent identities.

## Trusted-Computing-Base Reporting

Every property record must report the exact trusted boundary relevant to its
status. Candidate entries include:

- the #134 versioned GPU Rust, SIMT, memory/progress, layout, numerical, and
  refinement models;
- the task, queue, generation, lifecycle, dependency, cancellation, delivery,
  and failure model versions in this document;
- Verus, solver, proof generator, proof-erasure projection, and certificate
  checkers for `Proved` claims;
- rustc extraction, MIR admission, Pliron/KIR verification, and transformation
  receipt checkers at their reported statuses;
- AMD architecture semantics, target primitive contracts, resource and origin
  validators, and the exact artifact boundary they cover;
- HSA/ROCm runtime, driver, firmware, physical GPU, cooperative-launch, cache,
  watchdog, timeout, reset, and host visibility contracts;
- target scheduling, residency, atomic forward-progress, fairness, host service,
  and environmental assumptions used by `service_progress`;
- external handler/library/intrinsic contracts and each unsupported
  sub-property.

Documentation or testing of an AMD or runtime behavior validates a model or
contract at most; it does not make the vendor implementation or physical GPU a
machine-checked theorem. Uncovered premises cap the affected property at their
declared #134 status.

## P1-P5 Dependency And Exit Gates

These gates are cumulative and fail closed. They define required evidence, not
current completion status. A phase cannot promote an independent property that
its exit evidence does not establish.

### P1 Rust surface and host lifecycle

Entry dependencies:

- this P0 model and canonical identity inputs are accepted;
- #134 D0 supplies the one-body, identity, descriptor, and proof-erasure
  baseline;
- #134 D1 supplies authenticated cross-crate MIR admission and exact-consumption
  checks for proof-sensitive values;
- #134 D3 supplies authenticated task/handler APIs, typed host descriptors,
  ABI binding, and asynchronous borrow retention;
- #134 D5 supplies brands, typestate, region permissions, epoch semantics, and
  controlled destruction.

Exit gate:

- task tags, payload ABI, handler contracts, and `TaskSchemaId` are
  authenticated and deterministic;
- queue, slot, generation, lease, cancellation, state partition, lifecycle,
  persistent descriptor, prepared launch, and host handle typestates implement
  exactly the P0 state transitions;
- admitted safe Rust cannot forge, copy, clone, forget, reconstruct, confuse,
  or invalidly drop a slot, lease, service epoch, or generation;
- payload and state borrows remain live through terminal completion or an
  admitted stopped/device-quiesced failure outcome;
- hostile and compile-fail fixtures reject stale tickets, wrong tags/ABI,
  cross-service leases, invalid cancellation, and release of live storage with
  source diagnostics.

P1 does not establish scheduler refinement, AMD visibility, progress, or
artifact correspondence.

### P2 IR, fusion, and scheduler representation

Entry dependencies:

- P1 has exited for every represented task and host lifecycle operation;
- #134 D2 provides a lossless executable KIR bridge and independent
  effect/capability recomputation;
- #134 D3 provides structured handler/graph semantics and numerical contracts;
- #134 D4 provides schedule/tile/layout models and transformation receipts;
- #134 D5 provides explicit memory effects, barriers, epochs, and region
  permissions in `gpu.*`;
- #134 D7 provides target feature/resource vocabulary needed to state, but not
  yet qualify, AMD synchronization and residency requirements.

Exit gate:

- existing dialects, without a new dialect, represent every task, queue,
  lifecycle, dependency, cancellation, phase, scope/order, and progress field;
- a bounded MoE or FlashAttention graph has one deterministic finite fusion
  plan and receipt retaining all graph, value, effect, numerical, region, and
  source origins;
- a bounded reference scheduler and generation ring retain the full abstract
  transition system through explicit target-neutral operations, without opaque
  protocol calls or post-hoc recognition of arbitrary atomics;
- cooperative launch and residency requirements remain explicit admission
  obligations;
- canonical round-trip and hostile mutations reject changed dependency,
  materialization, scope, ordering, generation, barrier, residency, task tag,
  or lifecycle semantics before artifact admission.

This document authorizes no P2 lowering implementation. P2 representation does
not establish a proof or machine property.

### P3 proofs and model validation

Entry dependencies:

- P2 has exited with canonical semantic snapshots and receipts;
- #134 D6 supplies the versioned semantics package, same-source Verus backend,
  primitive registry, proof-erasure projection, coverage checker, and evidence
  binding;
- issue #106 is generalized before any claim crosses authenticated Rust/MIR to
  KIR for task, queue, lease, lifecycle, or cancellation behavior;
- bounded GEMM, attention, and MoE handlers supply exact, independently scoped
  algorithm and numerical contracts.

Exit gate:

- symbolic legal queue capacity and generation parameters prove `queue_safe`,
  `queue_linearizable`, and the declared `task_accounted` statement for the
  reference ring;
- finite megakernel receipts prove graph and phase refinement;
- the mixed bounded service composes each handler theorem with scheduler,
  dependency, ownership, cancellation, and graceful-drain theorems;
- bounded reference execution and AMD litmus expectations agree with the same
  model identities;
- mutations of publication order/scope, wraparound, ABA, lease uniqueness,
  duplicate/lost completion, stale epoch, cancellation, dependency, phase, or
  quiescence fail their named obligations;
- `service_progress` remains `Unsupported` without the exact progress contract
  and is no stronger than `Contracted` while target/runtime fairness is an
  environmental assumption.

P3 source or KIR proof cannot issue HSACO-level `machine_refined` evidence.

### P4 AMD artifact and hardware qualification

Entry dependencies:

- P3 has exited for the exact semantic and target plan;
- #134 D7 provides the exact target primitive semantics, AMD legalization,
  pre-LLVM resource estimator, post-HSACO reconciler, and stable origin maps;
- issue #106 covers the exact authenticated source/MIR-to-KIR path;
- issue #107 or a reviewed successor covers every persistent KIR-to-LLVM,
  object, and ISA atomic, fence, wait, barrier, branch, resource, and origin
  mapping;
- an exact gfx942 launch/runtime/failure/progress contract is named.

Exit gate:

- artifact inspection finds the required publication, acquisition, completion,
  dependency, cancellation, and lifecycle sequences and no unexpected atomic,
  fence, barrier, scratch, spill, call, or control-flow behavior;
- exact post-link VGPR/SGPR/AGPR, LDS, scratch, workgroup, and occupancy data
  satisfies the admitted residency floor and invalidates evidence on change;
- ordinary grid barriers and cooperative grids exceeding the simultaneous
  residency bound reject;
- gfx942 contention, wraparound, empty/full, mixed-duration, drain,
  cancellation, shutdown, timeout, and admitted failure tests preserve all
  safety/accounting canaries;
- each exact artifact reports progress as `Proved`, `Validated`, `Contracted`,
  or `Unsupported` according to its actual boundary and assumptions.

Hardware stress or observation cannot promote formal queue, accounting,
functional, numerical, or progress evidence.

### P5 performance qualification and rollout

Entry dependencies:

- P4 has exited for every candidate to be timed;
- finite unfused and finite host-launched references use the same graph or task
  stream, input contract, numerical policy, and target environment;
- thresholds, task mixes, queue loads, resource limits, clocks/power/noise
  policy, and fallback rules are committed before results are admitted;
- #134 D8 constrained autotuning and variant dispatch is a separate unresolved
  dependency outside this document's D0-D7 authority.

Exit gate:

- reports separate kernel-only and end-to-end submission, queueing,
  acquisition, execution, completion, drain, and shutdown costs;
- finite megakernels are compared with the same unfused graph, and persistent
  services with the same finite host-launched task stream;
- exact admitted profiles meet their precommitted occupancy, residency,
  no-scratch/no-spill, throughput, and tail-latency thresholds;
- low-load, resource-hostile, unsupported-progress, or unqualified workloads
  deterministically select the finite fallback;
- shadow comparison and rollout retain the property matrix, identity binding,
  failure policy, and rollback boundary;
- no benchmark, profiler result, occupancy observation, or tuning choice
  promotes any correctness, refinement, accounting, quiescence, or progress
  property.

P5 cannot exit from #134 D0-D7 alone. This document records the gate but makes
no claim about #134 D8 or later milestone completion.

## Unresolved Dependencies

The following remain explicit blockers or external work packages:

- #134 D0-D7 must provide their stated one-body, Rust/MIR, KIR, structured,
  schedule/layout, permission/epoch, formal-model, AMD target, and resource
  contracts. This document does not mark any of them complete.
- Issue #106 must cover the admitted task, queue, lease, generation, lifecycle,
  cancellation, and service-state Rust/MIR-to-KIR subset before corresponding
  cross-boundary claims are available.
- Issue #107 or a generalized successor must cover exact persistent atomics,
  scopes, fences, availability/visibility operations, waits, barriers,
  branches, resource metadata, object, and ISA behavior.
- Issues #122-#125 must supply the bounded FlashAttention and MoE semantics,
  handler contracts, numerical policies, and evidence consumed by composition.
- A reviewed gfx942 AMD memory/progress/runtime/failure contract and exact
  post-link residency evidence are required before P4 progress qualification.
- #134 D8 remains required for constrained autotuning and deterministic variant
  dispatch before P5; it is intentionally outside this document's authority.
- Concrete schema registration, implementation ownership, executable lowering,
  model checking, Verus proofs, litmus tests, artifact validators, hardware
  qualification, and performance thresholds are P1-P5 work.

Until those dependencies exit, this document supplies deterministic statements
and fail-closed admission requirements only. It issues no executable,
`Proved`, `Validated`, performance-qualified, or `machine_refined` result.
