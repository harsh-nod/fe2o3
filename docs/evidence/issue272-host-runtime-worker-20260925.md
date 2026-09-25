# Issue 272 host/runtime worker handoff

Integrated validation is recorded in
[contract retention and runtime transport](conditional-contract-retention-20260925.md).
The worker-local test status below describes the handoff, not the later integration run.

Base: `89c8c8992e8ec51f4db5d9fb6a0725e3a5a9f4e6`.
Branch: `codex/issue272-host-invocation-20260925`.
Private worktree: `/dev/shm/fe2o3-host-worker.bhBbYbqC`.

Applied only the combined
`host-premises-runtime-adaptation-039fce9d7-20260924.patch`
(SHA256 `07ca4ff7e5ae7a26e39bdac4e7e1e50300bd0f66c20933d393b80805d172c655`).
It applied cleanly to the current base. Original two patches were not applied.
No primary-tree edits, builds, test execution, SSH, pushes, or root actions.

## Bounded implementation

- Full inert V4 ABI comparison precedes conditional contract/premise binding:
  every logical field, including scalars outside the premise roster, source and
  device layout, effects, offsets, physical components, and packing observation.
  Unsupported nominal usize/isize/raw-pointer source shapes fail closed; the
  existing sealed legacy plan cannot supply their distinct source custody.
- Ordinary `pack` no longer incurs the adaptation's unaccounted plan clone.
  Crate-private `GeneratedKfdArgumentBinding::pack_with_conditional_plan_v1`
  retains the original sealed plan using the inherited budget and returns
  `(packed, unreserved_plan_storage)`. Its quote covers the owner shell, both
  boxed arrays, field-name allocations, and conservative shared-seal retention.
- Reserve that plan addition on the same account before
  `bind_conditional_premises_v1`. This join charges descriptor traversal,
  complete ABI comparison, bounded canonical type/layout encodings, buffer scans,
  hashing, temporary rows, and retained premise copies. Its
  `retained_storage()` is an additional unreserved premise charge, excluding
  already-owned arguments and the plan reservation.
- Keep the plan reservation until packed arguments drop or
  `into_runtime_inputs` drops the plan. Keep the premise reservation with the
  runtime payload until it drops, including error and unwind. Ordinary packing
  and input-owner accounting are existing caller obligations: this helper only
  adds accounting for the newly retained plan, and does not manufacture the
  missing authenticated account owner.
- No new work meter or ledger is created in production code. Scope cleanup uses
  the inherited budget's `with_prepaid_scope`, preserving work/peak/denial history.

## Capacity follow-up

Capacity follow-up: host temporary row vectors and KFD retained copies now
require capacity to equal the prepaid/requested row count after
`try_reserve_exact`, before filling or retaining rows. Excess capacity fails
closed and is dropped; no larger logical-capacity owner escapes under a smaller
quote. Host checks remain on the original prepaid scope. KFD's fixed checks are
covered by the host constructor's existing structural-work precharge.

Three additional, unrun tests cover exact and empty allocations, synthetic
excess capacity, unchanged allocation pointers on copying, and original-account
cleanup before any use of rejected host scratch. Select host `capacity_tests`
and KFD `bounded_copies_reject_excess_capacity_before_retaining_rows`.

## Required primary ownership

Keep `AuthenticatedWorkerV3ExecutableV1::prepare_generated_kfd_invocation` as
the entry. Primary still owns the closed retained V4 admission-family projection,
proof/relation/finalizer/publication custody, and inherited-account ownership
through `prepare_generated_kfd_arguments_with_current`. Primary must join the
retained contract and exact generated invocation coordinates in
`application_execution_admission` and
`WorkerV3ApplicationExecutionBindingV1` before changing the host authority.

`WorkerV3Gfx942ExecutionAuthorityV1::invocation_binding` remains required with
no default. The existing host owner reports OrdinaryV1 and
`require_ordinary_runtime_family` still refuses conditional safe launch.
Neither inert V4 bytes, a premise payload, nor copied public prepared coordinates
supply authority. This handoff does not claim authenticated V4 end-to-end launch.
KFD's existing numerical live-mapping checks remain before queue publication;
native runtime account custody must come from primary's owning transition.

## Authored tests, all unrun

Beyond the combined patch's tests, added full non-premise scalar type/name/offset
substitutions, missing plan or changed physical observation, ordinary packed
bytes/identity preservation, exact and one-short clone and complete conditional
resource limits, deep clone/seal identity, error and caller-unwind cleanup,
publication-only changes, exact/one-short live extents, adjacent subspans versus
output overlap, and the exact masked-tail arithmetic boundary.

Primary test selections (commands were not run by this worker):

```sh
cargo test -p fe2o3-host --lib conditional_premises_tests
cargo test -p fe2o3-host --lib conditional_clone::tests
cargo test -p fe2o3-host --lib existing_v1_admission_never_promotes_conditional_transport
cargo test -p fe2o3-runtime --lib conditional
cargo test -p fe2o3-runtime --lib runtime_preparation
cargo test -p fe2o3-runtime --lib dispatch_contract
cargo test -p fe2o3-kfd --lib conditional_dispatch_v1
cargo test -p fe2o3-host -p fe2o3-runtime --doc
```

Only pinned rustfmt and diff hygiene are checked here. Rust type checking,
execution, full integration, and publication to both main branches are primary-owned.
Transport and private numerical fixtures do not replace authenticated host-entry
or live hardware acceptance tests.

## Changed paths

- `crates/fe2o3-host/src/generated_argument_plan.rs`
- `crates/fe2o3-host/src/generated_argument_plan_clone_v1.rs`
- `crates/fe2o3-host/src/generated_kfd_arguments.rs`
- `crates/fe2o3-host/src/generated_kfd_conditional_abi_v1.rs`
- `crates/fe2o3-host/src/generated_kfd_conditional_premises_v1.rs`
- `crates/fe2o3-host/src/generated_kfd_conditional_premises_v1_tests.rs`
- `crates/fe2o3-host/src/generated_kfd_invocation.rs`
- `crates/fe2o3-kfd/src/conditional_dispatch_v1.rs`
- `crates/fe2o3-kfd/src/conditional_dispatch_v1_tests.rs`
- `crates/fe2o3-kfd/src/lib.rs`
- `crates/fe2o3-kfd/src/queue_dispatch_live.rs`
- `crates/fe2o3-kfd/src/shared_memory.rs`
- `crates/fe2o3-runtime/examples/gfx942-lds-diagnostic.rs`
- `crates/fe2o3-runtime/src/authorized_execution.rs`
- `crates/fe2o3-runtime/src/conditional_authority_tests.rs`
- `crates/fe2o3-runtime/src/conditional_transport_v1.rs`
- `crates/fe2o3-runtime/src/conditional_transport_v1_tests.rs`
- `crates/fe2o3-runtime/src/lib.rs`
- `crates/fe2o3-runtime/src/runtime_conditional_preparation_tests.rs`
- `crates/fe2o3-runtime/src/runtime_preparation_tests.rs`
- `crates/fe2o3-runtime/tests/kfd_opaque_checkpoint_live.rs`
- `docs/evidence/issue272-host-runtime-worker-20260925.md`
