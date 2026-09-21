//! Inert descriptor capability projection; not source, target or launch authority.

use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1};
use fe2o3_kernel_descriptor::{CapabilityV1, DeviceDescriptorTableV1};
use fe2o3_kernel_ir::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME, BF16_F32_M16N16K16_CAPABILITY,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKirOperationCoordinateV1,
    LDS_TILE_16X16_XOR4_CAPABILITY, MATRIX_CAPABILITY_NAMESPACE, OperationKind,
    SCALED_FP4_E2M1_F32_M16N16K128_CAPABILITY, SCALED_FP4_E2M1_FP8_E4M3_F32_M16N16K128_CAPABILITY,
    SCALED_FP8_E4M3_F32_M16N16K128_CAPABILITY, TargetCapabilityRefV1 as Cap,
    VerifiedCanonicalKernelIrModuleV12 as Owner, WaveWidth, atomic_pointer_capability_v1,
};
use std::{
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

const TAGS: [CapabilityV1; 6] = [
    CapabilityV1::Subgroup,
    CapabilityV1::WorkgroupMemory,
    CapabilityV1::MatrixMultiply,
    CapabilityV1::Atomics,
    CapabilityV1::AmdWave,
    CapabilityV1::AmdMfma,
];

/// Fixed-size inert set in descriptor Ord order. No backing allocation or authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DescriptorCapabilitySetV1(u8);
impl DescriptorCapabilitySetV1 {
    pub fn iter(self) -> impl Iterator<Item = CapabilityV1> {
        TAGS.into_iter()
            .enumerate()
            .filter_map(move |(i, tag)| (self.0 & (1 << i) != 0).then_some(tag))
    }
}

fn exact_target(cap: Cap<'_>) -> bool {
    matches!(cap, Cap::Extension { namespace, name }
        if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
            && (name.matches(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
                || name.matches(AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME)))
}

fn matrix(cap: Cap<'_>) -> bool {
    matches!(cap, Cap::Extension { namespace, name }
        if namespace == MATRIX_CAPABILITY_NAMESPACE && [
            BF16_F32_M16N16K16_CAPABILITY,
            SCALED_FP4_E2M1_F32_M16N16K128_CAPABILITY,
            SCALED_FP8_E4M3_F32_M16N16K128_CAPABILITY,
            SCALED_FP4_E2M1_FP8_E4M3_F32_M16N16K128_CAPABILITY,
        ].into_iter().any(|candidate| name.matches(candidate)))
}

/// Sole allocation-free legacy mapping. Flags are inert inputs to this classifier,
/// not evidence. Verifiers must use the closed actual-owner checker below.
/// None preserves the legacy unsupported-capability decision; callers retain
/// their own traversal order and diagnostic formatting.
pub fn project_descriptor_capability_v1(
    capability: Cap<'_>,
    allow_exact_tiled_matrix: bool,
    allow_workgroup_memory: bool,
    has_exact_diagnostic_target: bool,
) -> Option<DescriptorCapabilitySetV1> {
    let bits = match capability {
        Cap::Int64 | Cap::BFloat16 => 0,
        cap if exact_target(cap) => 0,
        Cap::Extension { namespace, name }
            if namespace == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                && name.matches(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME) =>
        {
            0
        }
        Cap::Subgroups | Cap::SubgroupSize(64) => (1 << 0) | (1 << 4),
        Cap::WaveWidth(WaveWidth::Wave64) => 1 << 4,
        Cap::WorkgroupMemory | Cap::WorkgroupBarrier if allow_workgroup_memory => 1 << 1,
        Cap::Atomic { .. } => 1 << 3,
        cap if allow_exact_tiled_matrix && matrix(cap) => (1 << 2) | (1 << 5),
        Cap::Extension { namespace, name }
            if allow_workgroup_memory
                && namespace == MATRIX_CAPABILITY_NAMESPACE
                && name.matches(LDS_TILE_16X16_XOR4_CAPABILITY) =>
        {
            1 << 1
        }
        Cap::Extension { namespace, name }
            if has_exact_diagnostic_target
                && namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
                && name.matches(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME) =>
        {
            0
        }
        _ => return None,
    };
    Some(DescriptorCapabilitySetV1(bits))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorCapabilitySiteV1 {
    Module,
    Kernel(u32),
    Function(u32),
    Operation(CanonicalKirOperationCoordinateV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorCapabilityProjectionErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    InconsistentInventory,
    MissingCall(CanonicalKirOperationCoordinateV1),
    Unsupported(DescriptorCapabilitySiteV1),
    KernelRoster,
    CapabilityMismatch { descriptor: usize },
    Panicked,
}
impl fmt::Display for DescriptorCapabilityProjectionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "descriptor capability projection: {self:?}")
    }
}
impl Error for DescriptorCapabilityProjectionErrorV1 {}
impl From<Resource> for DescriptorCapabilityProjectionErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type E = DescriptorCapabilityProjectionErrorV1;
type Site = DescriptorCapabilitySiteV1;

#[derive(Default)]
struct Scratch {
    reached: Vec<u8>,
    pending: Vec<usize>,
    allow_matrix: bool,
    allow_workgroup: bool,
    exact_target: bool,
    projected: DescriptorCapabilitySetV1,
}

fn scoped<'work>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<(), E>,
) -> Result<(), E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let (result, payload) = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => (result, None),
        Err(payload) => (Err(E::Panicked), Some(payload)),
    };
    let restored = if ledger != budget.work_ledger_identity_v1() {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    // All local backing has dropped; even a hostile payload drops after restoration.
    drop(payload);
    restored.map_err(E::Resource)?;
    result
}

fn backing<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, E> {
    budget.charge_work(1)?;
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    // An allocator's transient excess is not a pre-allocation hard bound.
    // Account accepted capacity before initialization; failure drops rows first.
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok(rows)
}

fn charge_capability(cap: Cap<'_>, budget: &mut Budget<'_>) -> Result<(), E> {
    budget.charge_work(1)?;
    if let Cap::Extension { namespace, name } = cap {
        // At most sixteen bounded key comparisons in either pass/classification.
        let bytes = namespace
            .len()
            .checked_add(name.visible_len().ok_or(Resource::Arithmetic)?)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(16))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(bytes)?;
    }
    Ok(())
}

fn enqueue(index: usize, scratch: &mut Scratch, budget: &mut Budget<'_>) -> Result<(), E> {
    budget.charge_work(1)?;
    let marked = scratch
        .reached
        .get_mut(index)
        .ok_or(E::InconsistentInventory)?;
    if *marked == 0 {
        if scratch.pending.len() == scratch.pending.capacity() {
            return Err(E::InconsistentInventory);
        }
        *marked = 1;
        scratch.pending.push(index);
    }
    Ok(())
}

fn reachability(
    inventory: &CanonicalKirInventoryV1<'_>,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    for kernel in inventory.kernels() {
        budget.charge_work(1)?;
        enqueue(kernel.entry.0 as usize, scratch, budget)?;
    }
    while let Some(index) = scratch.pending.pop() {
        budget.charge_work(1)?;
        let function = inventory
            .functions()
            .get(index)
            .ok_or(E::InconsistentInventory)?;
        let calls = inventory
            .calls()
            .get(function.calls.clone())
            .ok_or(E::InconsistentInventory)?;
        for call in calls {
            budget.charge_work(1)?;
            let target = call.target.ok_or(E::MissingCall(call.coordinate))?;
            enqueue(target.0 as usize, scratch, budget)?;
        }
    }
    Ok(())
}

fn visit_closure(
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
    mut visit: impl FnMut(Cap<'_>, Site, &mut Budget<'_>) -> Result<(), E>,
) -> Result<(), E> {
    for cap in &inventory.owner().module().required_capabilities {
        let cap = Cap::from_owned(cap);
        charge_capability(cap, budget)?;
        visit(cap, Site::Module, budget)?;
    }
    for kernel in inventory.kernels() {
        budget.charge_work(1)?;
        for cap in &kernel.kernel.required_capabilities {
            let cap = Cap::from_owned(cap);
            charge_capability(cap, budget)?;
            visit(cap, Site::Kernel(kernel.ordinal), budget)?;
        }
    }
    for function in inventory.functions() {
        budget.charge_work(1)?;
        for cap in &function.function.required_capabilities {
            let cap = Cap::from_owned(cap);
            charge_capability(cap, budget)?;
            visit(cap, Site::Function(function.coordinate.0), budget)?;
        }
        let operations = inventory
            .operations()
            .get(function.operations.clone())
            .ok_or(E::InconsistentInventory)?;
        for row in operations {
            budget.charge_work(
                row.operation
                    .required_capability_visitation_work_v1()
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let site = Site::Operation(row.coordinate);
            row.operation.try_visit_required_capabilities_v1(|cap| {
                charge_capability(cap, budget)?;
                visit(cap, site, budget)
            })?;
            if let OperationKind::Atomic(atomic) = &row.operation.kind {
                budget.charge_work(3)?;
                let definition = inventory
                    .definition_for_value(function.coordinate, atomic.pointer, budget)
                    .map_err(E::Inventory)?
                    .ok_or(E::InconsistentInventory)?;
                if let Some(capability) = atomic_pointer_capability_v1(atomic, definition.ty) {
                    let cap = Cap::from_owned(&capability);
                    charge_capability(cap, budget)?;
                    visit(cap, site, budget)?;
                }
            }
        }
    }
    Ok(())
}

/// Checks the exact whole-closure capability set of actual F against every
/// descriptor row. Allowances derive only from module declarations and the
/// union of functions reachable from real kernel entries, matching geometry.
/// Kernel-only or unreachable declarations cannot authorize restricted tags.
///
/// Inputs remain caller-reserved. Fresh inventory uses its documented logical
/// storage domain. New scratch headers/requested backing are prepaid; actual
/// capacity excess is reconciled before use. All work is cumulative; every exit
/// drops scratch/inventory before refunding the same entry ledger/floor.
/// Unsupported errors follow module, kernel, function, operation order, not the
/// legacy backend's sorted diagnostic order. No strings are allocated for errors.
/// Source/ABI, root identities, target, geometry, formal and text checks remain
/// separate mandatory obligations. Success returns no admission authority.
pub fn check_canonical_v12_descriptor_capabilities_v1(
    actual_f: &Owner,
    table: &DeviceDescriptorTableV1,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    scoped(budget, |budget| {
        budget.charge_work(3)?;
        let (inventory, receipt) =
            CanonicalKirInventoryV1::derive(actual_f, budget).map_err(E::Inventory)?;
        budget.reserve_storage(receipt.retained_storage())?;
        budget.charge_work(1)?;
        if inventory.kernels().is_empty() || inventory.kernels().len() != table.kernels().len() {
            return Err(E::KernelRoster);
        }
        budget.reserve_storage(size_of::<Scratch>())?;
        let mut scratch = Scratch::default();
        scratch.reached = backing(inventory.functions().len(), budget)?;
        budget.charge_work(inventory.functions().len())?;
        scratch.reached.resize(inventory.functions().len(), 0);
        scratch.pending = backing(inventory.functions().len(), budget)?;
        reachability(&inventory, &mut scratch, budget)?;
        visit_closure(&inventory, budget, |cap, site, _| {
            scratch.exact_target |= exact_target(cap);
            let allowed = match site {
                Site::Module => true,
                Site::Kernel(_) => false,
                Site::Function(f) => {
                    *scratch
                        .reached
                        .get(f as usize)
                        .ok_or(E::InconsistentInventory)?
                        != 0
                }
                Site::Operation(op) => {
                    *scratch
                        .reached
                        .get(op.block.function.0 as usize)
                        .ok_or(E::InconsistentInventory)?
                        != 0
                }
            };
            if allowed {
                scratch.allow_workgroup |= matches!(cap, Cap::WorkgroupMemory);
                scratch.allow_matrix |= matrix(cap);
            }
            Ok(())
        })?;
        visit_closure(&inventory, budget, |cap, site, _| {
            let mapped = project_descriptor_capability_v1(
                cap,
                scratch.allow_matrix,
                scratch.allow_workgroup,
                scratch.exact_target,
            )
            .ok_or(E::Unsupported(site))?;
            scratch.projected.0 |= mapped.0;
            Ok(())
        })?;
        for (descriptor, kernel) in table.kernels().iter().enumerate() {
            budget.charge_work(
                kernel
                    .capabilities()
                    .len()
                    .checked_add(TAGS.len() + 1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if !kernel
                .capabilities()
                .iter()
                .copied()
                .eq(scratch.projected.iter())
            {
                return Err(E::CapabilityMismatch { descriptor });
            }
        }
        Ok(())
    })
}

#[cfg(test)]
#[path = "descriptor_capability_projection_v1_tests.rs"]
mod tests;
