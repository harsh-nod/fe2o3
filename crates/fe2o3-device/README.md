# fe2o3-device

`fe2o3-device` is the `no_std` source vocabulary for GPU kernels. Its safe
capability API distinguishes facts that Rust can enforce from obligations that
the production compiler must prove.

New target-neutral kernels should import `fe2o3_device::prelude::*`. The prelude
contains the context hierarchy, geometry, memory roles, synchronization epochs,
subgroups, collectives, atomics, matrix authority, and kernel result types. It
deliberately excludes target adapters and legacy independent `current()` APIs.

## Capability hierarchy

```text
KernelContext<'kernel, Kernel, Target, Launch>
  -> Invocation3D<KernelBrand>
       -> Grid<'_, KernelBrand>
       -> Workgroup<'_, KernelBrand>             (arithmetic observations)
  -> with_workgroup(for<'workgroup> ...)
       -> WorkgroupCapability<'workgroup, KernelBrand, Epoch>
            -> Subgroup<ExactWidth, KernelBrand, Epoch>
            -> WorkgroupLds<State, KernelBrand, Epoch>
            -> WorkgroupMemoryView<Space, Role, WorkgroupBrand, Epoch>
            -> PendingAsyncCopy<KernelBrand, Epoch>
            -> ScopedAtomic<Space, Scope, KernelBrand, Epoch>
```

Private fields, invariant brands, higher-ranked workgroup lifetimes, and
move-only epoch tokens prevent safe host construction, cross-kernel or
cross-workgroup substitution, stale-epoch use, and transfer through `Send` or
`Sync`. `SubgroupWidth32` and `SubgroupWidth64` are neutral exact-width
requirements. `Wave32`, `Wave64`, and `WaveLane` remain deprecated AMD
compatibility aliases; they preserve the exact width.

Barrier and async-wait transitions consume a `WorkgroupCapability<..., Epoch>`
and return `WorkgroupCapability<..., NextEpoch<Epoch>>`. Published LDS and
atomic handles must carry the same epoch. Scope, order, address-space set, and
atomic access role are sealed type parameters, so unsupported combinations do
not type-check.

These types do **not** prove that all participants converge, that arbitrary LDS
indices are race-free, or that a target supports a requested operation. Those
remain production compiler obligations.

## Memory views

The closed source memory contract names three address spaces and keeps access
and alias authority explicit in each view type:

| View | Address space | Typical safe role | Identity |
| --- | --- | --- | --- |
| `Global` | `GlobalAddressSpace` | `ReadOnly`, `DisjointWrite`, or scoped atomic | kernel/target/launch brand |
| `PrivateMemoryView` | `PrivateAddressSpace` | `ExclusiveReadWrite` | kernel/target/launch brand and view lifetime |
| `WorkgroupMemoryView` | `WorkgroupAddressSpace` | disjoint write, then published `ReadOnly` | kernel, generative workgroup, and synchronization epoch |

`ReadOnly` means shared immutable aliases, `ExclusiveReadWrite` means one
exclusive alias, `DisjointWrite<Mapping>` means write-only access selected by a
matching nonduplicable index, and `AtomicReadWrite<Scope>` grants only scoped
atomic operations. These pairings are sealed. Safe source cannot relabel an
address space, role, brand, mapping, or epoch.

`KernelContext::private_memory` creates no kernarg. A workgroup allocation is
issued by `WorkgroupCapability::allocate_memory`; each invocation obtains its
matching `memory_index_1d`, and `publish_memory` consumes both the write view and
old epoch before returning a read view in `NextEpoch`. The source types enforce
the local protocol. Production analysis must still prove allocation extent,
complete initialization, race freedom, convergence, and barrier order.

`Private` and `capability_memory::Workgroup` are migration aliases. They retain
the exact role and brand; the workgroup alias is explicitly initial-epoch only.
The stable prelude exports only the canonical view names.

## Unsafe raw boundary

`PrivateMemoryView::from_raw_parts` and `WorkgroupMemoryView::from_raw_parts`
are the only raw constructors for these local views. Both are `unsafe`, require
the matching context capability, and accept the exact associated
`UNSAFE_RAW_OBLIGATION_V1` record. The record fixes:

- address space, access, and alias identities;
- allocation provenance and extent;
- alignment and element layout;
- lifetime and cross-invocation race freedom; and
- initialization whenever the role permits reads.

The record is inert data, not proof or authority. MIR admission must authenticate
the constructor terminal and either discharge every recorded obligation or fail
the build. Passing a record for another space or role fails closed even in a
non-production host execution.

## Fail-closed terminals

The new source surface intentionally traps unless production import recognizes
its exact diagnostic item and validates the corresponding MIR/KIR contract.
The terminal identities are:

- `fe2o3_device_subgroup_current_v1`
- `fe2o3_device_typed_workgroup_barrier_v1`
- `fe2o3_device_typed_subgroup_barrier_v1`
- `fe2o3_device_typed_workgroup_fence_v1`
- `fe2o3_device_typed_subgroup_fence_v1`
- `fe2o3_device_workgroup_lds_allocate_v1`
- `fe2o3_device_workgroup_lds_initialize_by_invocation_v1`
- `fe2o3_device_workgroup_lds_publish_v1`
- `fe2o3_device_workgroup_lds_read_published_v1`
- `fe2o3_device_scoped_global_atomic_v1`
- `fe2o3_device_scoped_atomic_load_v1`
- `fe2o3_device_scoped_atomic_store_v1`
- `fe2o3_device_scoped_atomic_fetch_add_v1`
- `fe2o3_device_scoped_atomic_compare_exchange_v1`
- `fe2o3_device_subgroup_reduce_sum_v1`
- `fe2o3_device_subgroup_inclusive_scan_sum_v1`
- `fe2o3_device_typed_workgroup_reduce_sum_v1`
- `fe2o3_device_typed_workgroup_inclusive_scan_sum_v1`
- `fe2o3_device_typed_workgroup_exclusive_scan_sum_v1`
- `fe2o3_device_workgroup_async_copy_v1`
- `fe2o3_device_workgroup_async_wait_v1`
- `fe2o3_device_capability_global_bind_atomic_v1`
- `fe2o3_device_private_memory_allocate_v1`
- `fe2o3_device_private_memory_load_v1`
- `fe2o3_device_private_memory_exclusive_load_v1`
- `fe2o3_device_private_memory_exclusive_store_v1`
- `fe2o3_device_private_memory_disjoint_store_v1`
- `fe2o3_device_private_memory_from_raw_parts_v1`
- `fe2o3_device_workgroup_memory_allocate_v1`
- `fe2o3_device_workgroup_memory_index_1d_v1`
- `fe2o3_device_workgroup_memory_publish_v1`
- `fe2o3_device_workgroup_memory_load_v1`
- `fe2o3_device_workgroup_memory_exclusive_load_v1`
- `fe2o3_device_workgroup_memory_exclusive_store_v1`
- `fe2o3_device_workgroup_memory_disjoint_store_v1`
- `fe2o3_device_workgroup_memory_from_raw_parts_v1`

Required importer and lowering work:

- authenticate `with_workgroup` scope issuance and preserve its generative
  workgroup brand;
- lower exact-width subgroup issuance after target capability resolution;
- prove workgroup/subgroup barrier convergence and preserve scope, ordering,
  address-space set, and epoch transitions;
- bind `AtomicReadWrite<Scope>` only to coherent, correctly aligned global
  storage with one exact atomic width and no conflicting non-atomic aliases;
- lower scoped atomic load/store/RMW/compare-exchange, including valid failure
  ordering and exact address-space/scope tuples;
- prove one in-bounds disjoint initialization write per invocation before LDS
  publication and preserve workgroup identity across every access;
- lower subgroup/workgroup collectives with exact width, geometry, numerical,
  scratch, and convergence requirements;
- admit matrix access only for an exact supported subgroup width and preserve
  the subgroup/epoch brand through fragments;
- lower global-to-workgroup async copy and wait while preserving source role,
  destination ownership, pending-token linearity, and epoch advancement.
- authenticate private/workgroup view construction and unsafe raw-obligation
  records, then lower their exact address-space, role, brand, and epoch tuples.

Legacy unbranded APIs remain for compatibility. They are not substitutes for
the branded production path and grant no publication, load, or launch
authority.
