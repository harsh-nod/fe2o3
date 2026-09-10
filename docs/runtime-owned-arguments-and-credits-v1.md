# Owned Generated Arguments And Requested-Allocation Credits V1

This contract describes the implemented GEN-1 and restricted MEM-1 slices in
the [A1/A2 work plan](runtime-a1-a2-swarm-plan.md). It does not admit general
generated native launches or establish a complete physical memory budget.

## Module Boundaries

| Owner | Responsibility |
| --- | --- |
| `fe2o3-host/generated_runtime_arguments.rs` | Owned typed data, authenticated argument-plan validation, per-invocation footprint preflight and inert result decoding |
| `fe2o3-macros` | Emit owned wrappers from the same parsed signature, ABI, effects and index mappings as the borrowed adapter |
| `fe2o3-runtime-model/r67_resource_credits.rs` | Fixed-vector arithmetic and exact owner/phase transition decisions |
| `fe2o3-runtime/resource_credits.rs` | Private synchronized account, bounded owner arena and move-only reservation/retention tokens |
| `fe2o3-runtime/context/allocation_admission.rs` | Opt-in requested-byte/record admission, backend failure classification and successful logical disposal integration |
| Native KFD adapters | Retain actual native storage independently; physical backing and slot charges are subsequent MEM-2/3/4 work |

Neither a footprint nor an observation digest is a capability. The credit
account is resource admission, not kernel execution authority. These boundaries
do not expose raw device pointers or accept a caller's boolean as proof of
native completion.

## Owned Generated Data

Generated `RuntimeArguments` accept owned scalar/slice values. Read, write and
read/write slice wrappers own `Box<[T]>`; they do not widen the lifetime of a
borrowed `GeneratedWorkerV3KfdInvocation`. Private output state keeps decoder
custody independent from the result observer's lifetime.

`prepare_generated_runtime_arguments` checks current compiler publication,
validates the existing argument packing plan, performs a complete read-only
footprint pass, then binds/encodes. It rechecks publication and requires the
actual bound footprint to match preflight. A later argument exceeding the bound
cannot cause earlier outputs to be encoded during preflight.

The per-invocation limits cover:

- Payload bytes: two kernarg copies, owned typed seeds and encoded buffer copies.
- Result bytes: returned buffers and decoded typed output copies, including
  results retained after inputs are dropped.
- Binding and output-observer cardinality, checked arithmetic and byte lengths.

These are logical byte/count limits, not an allocator-overhead, native-residency
or aggregate engine reservation. The generated trait is unsafe to implement:
its safety contract requires exact argument coverage and complete accounting.
Application-provided unsafe implementations cannot be treated as compiler proof.

The decoder validates the complete returned buffer shape/access/length and
capacity before delivering any output. It consumes owned data; it does not
authenticate the producer or mint a completed runtime operation. Observer drop
does not cancel native work or release decoder custody. Repeated result take,
wrong shape/type and stale private output custody reject.

GEN-2 must privately pair the arguments and decoder with the exact invocation
permit, normalize returned buffer capacities, integrate global credit lifetime,
validate Context allocation freshness, and retain compiler/native authority
through quiescence. The blocking entry must eventually join that same async
path. No positive production execution claim follows from GEN-1 fixture tests.

## Transactional Credits

The model vector has nineteen distinct dimensions. Requested logical bytes,
physical residency classes and occupied slots are not interchangeable, and
unintegrated dimensions are not measured usage. Every arithmetic operation scans
the fixed vector and either produces a complete valid result or rejects.

An account preallocates a bounded record arena and free-slot list. Each token
retains its exact account, slot and nonwrapping owner generation. The synchronized
adapter consumes production-used model transition decisions before mutating
the ledger; no callback or fallible allocation follows successful transition
preflight while its account mutex is held.

| State/action | Credit effect |
| --- | --- |
| Reserve | Debit the complete vector and occupy one exact owner record, or change neither |
| Drop an unissued reservation | Return its exact charge |
| Retain before backend entry | Transfer custody without changing the charge |
| Definite no-effect rejection | Return the exact attempt's retained charge |
| Successful disposal of represented resources | Return the retained charge once |
| Timeout, observer drop, uncertain outcome or failed disposal | No refund |
| Drop retained custody | Quarantine the record without reuse or refund |
| Stale owner, generation exhaustion or invariant loss | Reject; invariant loss prevents further admission/refund |

One retained `Arc` anchor per ambiguous account conservatively preserves that
ledger after its external handles disappear. This is a process-lifetime fallback,
not a global quarantine ceiling. Repeated Context creation, actual arena bytes
and simultaneous failures still require MEM-5 aggregate accounting.

## Context Allocation Profile

`configure_allocation_admission_v1(device, requested_bytes, allocation_records)`
installs an opt-in account for one exact Context device. The default behavior is
unchanged. Installation/replacement rejects while the device has live unaccounted
allocations or retained/quarantined credits, or while a graph owns the Context.
Invalid replacement configuration cannot discard an existing account.

Before configured allocation enters the backend, Context reserves registry
capacity, then retains `RequestedAllocationBytes + AllocationRecords` together.
Its failure classification is deliberately conservative:

- Definite backend rejection refunds the attempt.
- Quiescent allocation failure is not evidence of disposal; it retains a
  quarantined charge even if the backend returned no logical handle.
- Terminal failure or backend panic seals the Context and retains custody.
- Zero or duplicate returned handles seal the Context while retaining the charge.
- Failed logical release retains the charge; only successful release refunds it.

Explicit release and cleanup share the same configured backend-release hook.
`allocation_admission_usage_v1` remains a read-only inspection on terminal
Contexts. Cleanup reports include credit records, so an unidentified quarantined
allocation attempt cannot produce a falsely complete cleanup report.

Successful logical allocation release may recycle physical backing into a native
pool. MEM-1 refunds the logical requested-allocation charge, not that cached
backing's residency. Actual backing, padding, queues, signals, kernargs, module
images, executable materialization and cached storage require separate native
cost extraction and retained ownership. No new allocator or queue is introduced.

## Proof And Qualification Limits

The R67 Verus companion proves fixed-vector reserve/release loops and exact
record-decision properties: complete admission, arithmetic conservation, owner
matching, no retained cancellation-as-unissued, no quarantine refund/reuse and
no duplicate refund. Fourteen obligations and eight deliberately failing
mutations are authenticated by the existing pinned proof runner.

The separately compiled Rust functions have reviewed correspondence. The
concrete mutex/arena/account, Context/native observation adapters, actual disposal,
global inventory and whole executor are not covered by a whole-state refinement
theorem. GPU, firmware, kernel and allocator behavior remain outside these
proofs. Safety does not assume eventual device completion.

CPU tests cover whole-vector rejection, ownership/reuse, stale tokens, concurrency,
poisoning, generation exhaustion, Context/device isolation, pre-backend limits,
release/failure/panic paths and cleanup incompleteness. Generated-host unit and
downstream compile-fail tests cover data ownership and packing, not execution.

The R66 qualification example additionally checks seven exact requested
allocations, rejection of an eighth request with unchanged usage, retention
through work completion and zero requested credits after successful cleanup.
Its independent checker validates full buffer/padding results and retained native
identities for eight R26 compute/directional-copy cells. Only a separately audited
signed live capture can fill that hardware cell. Retained custody is not evidence
of physical overlap, and this qualifier contains no performance measurement.
