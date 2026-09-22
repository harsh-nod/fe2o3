//! Closed target-bound V3 requirement replay over the complete actual module.
use crate::native_v12_text_descriptor_replay_v3::{self as shared, E, R, Root};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_descriptor::{
    BlockSizeV1, CapabilityCursorV3, CapabilityV1, DESCRIPTOR_QUERY_STORAGE_V3,
    DeviceDescriptorTableV3 as Table, KernelDescriptorRefV3, KernelTargetRequirementsV2,
    RequiredWavefrontWidthV2,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, OperationKind, TargetCapability,
    TargetCapabilityRefV1, VerifiedCanonicalKernelIrModuleV12 as Owner, WaveWidth,
};
use std::{collections::BTreeSet, mem::size_of};

/// Exact current closed-profile requirement agreement, not formal, runtime,
/// source/ownership or launch authority. The actual final graph stays borrowed.
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedDescriptorRequirementsV3 as R;
/// fn duplicate(v: R<'_, '_, '_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedDescriptorRequirementsV3 as R;
/// fn forge<'a>() -> R<'a, 'a, 'a> { R::default() }
/// ```
pub struct ReplayedDescriptorRequirementsV3<'o, 'd, 'w> {
    output: &'o Owner,
    table: &'d Table<'w>,
    profile: Profile,
    retained: usize,
}
impl<'o, 'd, 'w> ReplayedDescriptorRequirementsV3<'o, 'd, 'w> {
    pub const fn output(&self) -> &'o Owner {
        self.output
    }
    pub const fn descriptors(&self) -> &'d Table<'w> {
        self.table
    }
    pub const fn profile(&self) -> Profile {
        self.profile
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn visit(
    capability: TargetCapabilityRefV1<'_>,
    profile: Profile,
    function: Option<usize>,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(1)?;
    let error = |field| E::Requirement { function, field };
    if let TargetCapabilityRefV1::Extension { namespace, name } = capability {
        let work = namespace
            .len()
            .checked_add(name.visible_len().ok_or(Resource::Arithmetic)?)
            .and_then(|n| n.checked_add(profile.device_target().len()))
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(20))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(work)?;
        if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
            && !name.matches(profile.device_target())
        {
            return Err(error("conflicting exact target"));
        }
        if namespace == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
            && name.matches(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME)
            && profile != Profile::Gfx942
        {
            return Err(error("legacy gfx942 diagnostic on another target"));
        }
    }
    // Actual verified operations supply requirements; exact bound claims are
    // required separately at module/kernel/entry. There is no permission input.
    let diagnostics = matches!(profile, Profile::Gfx942 | Profile::Gfx950);
    let set = crate::project_descriptor_capability_v1(capability, false, false, diagnostics)
        .ok_or_else(|| error("unsupported capability"))?;
    for item in set.iter() {
        budget.charge_work(1)?;
        if item != CapabilityV1::AmdWave {
            return Err(error("deferred target-sensitive capability"));
        }
    }
    Ok(())
}

fn declarations(
    caps: &BTreeSet<TargetCapability>,
    profile: Profile,
    function: Option<usize>,
    budget: &mut Budget<'_>,
) -> R<(bool, bool)> {
    let mut target = false;
    let mut wave = false;
    for cap in caps {
        let borrowed = TargetCapabilityRefV1::from_owned(cap);
        visit(borrowed, profile, function, budget)?;
        budget.charge_work(3)?;
        if let TargetCapabilityRefV1::Extension { namespace, name } = borrowed {
            let work = namespace
                .len()
                .checked_add(name.visible_len().ok_or(Resource::Arithmetic)?)
                .and_then(|n| n.checked_add(profile.device_target().len()))
                .ok_or(Resource::Arithmetic)?;
            budget.charge_work(work)?;
            if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                && name.matches(profile.device_target())
            {
                target = true;
            }
        }
        if matches!(cap, TargetCapability::WaveWidth(WaveWidth::Wave64)) {
            wave = true;
        }
    }
    Ok((target, wave))
}
fn require_bound(bits: (bool, bool), function: Option<usize>) -> R<()> {
    if bits != (true, true) {
        return Err(E::Requirement {
            function,
            field: "missing exact target/Wave64 binding",
        });
    }
    Ok(())
}

pub(super) fn check(
    inventory: &Inventory<'_>,
    table: &Table<'_>,
    profile: Profile,
    roots: &[Root],
    budget: &mut Budget<'_>,
) -> R<()> {
    let module = inventory.owner().module();
    budget.charge_work(1)?;
    require_bound(
        declarations(&module.required_capabilities, profile, None, budget)?,
        None,
    )?;
    for kernel in &module.kernels {
        budget.charge_work(1)?;
        require_bound(
            declarations(&kernel.required_capabilities, profile, None, budget)?,
            None,
        )?;
    }
    let binding_floor = budget.storage();
    let mut bindings = shared::vector::<(bool, bool)>(module.functions.len(), budget)?;
    for (ordinal, function) in module.functions.iter().enumerate() {
        budget.charge_work(1)?;
        let bound = declarations(
            &function.required_capabilities,
            profile,
            Some(ordinal),
            budget,
        )?;
        shared::push(&mut bindings, bound, budget)?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                for operation in &block.operations {
                    budget.charge_work(1)?;
                    if matches!(
                        operation.kind,
                        OperationKind::Atomic(_)
                            | OperationKind::Barrier(_)
                            | OperationKind::Fence(_)
                            | OperationKind::WorkgroupBarrier(_)
                    ) {
                        return Err(E::Requirement {
                            function: Some(ordinal),
                            field: "atomic/synchronization operation deferred",
                        });
                    }
                    operation.try_visit_required_capabilities_v1(|capability| {
                        visit(capability, profile, Some(ordinal), budget)
                    })?;
                }
            }
        }
    }
    let scratch = DESCRIPTOR_QUERY_STORAGE_V3
        .checked_add(size_of::<KernelTargetRequirementsV2>())
        .and_then(|n| n.checked_add(size_of::<KernelDescriptorRefV3<'_, '_>>()))
        .and_then(|n| n.checked_add(size_of::<CapabilityCursorV3<'_, '_>>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(scratch)?;
    for root in roots {
        budget.charge_work(32)?;
        let kernel = inventory.kernels()[root.kernel].kernel;
        let entry = inventory.kernels()[root.kernel].entry.0 as usize;
        require_bound(
            *bindings.get(entry).ok_or(E::Invalid("bound entry index"))?,
            Some(entry),
        )?;
        let row = table
            .kernel(root.descriptor, &mut |w| budget.charge_work(w))
            .map_err(E::Descriptor)?;
        let requirement = table
            .requirement(root.descriptor, &mut |w| budget.charge_work(w))
            .map_err(E::Descriptor)?;
        let error = |field| E::Requirement {
            function: Some(inventory.kernels()[root.kernel].entry.0 as usize),
            field,
        };
        if row.capability_count() != 1 {
            return Err(error("complete AmdWave-only capability roster"));
        }
        let mut capabilities = row.capabilities();
        if capabilities
            .next(&mut |w| budget.charge_work(w))
            .map_err(E::Descriptor)?
            != Some(CapabilityV1::AmdWave)
            || capabilities
                .next(&mut |w| budget.charge_work(w))
                .map_err(E::Descriptor)?
                .is_some()
        {
            return Err(error("complete AmdWave-only capability roster"));
        }
        if requirement.kernel_id() != row.kernel_id()
            || requirement.wavefront_width() != RequiredWavefrontWidthV2::Wave64
            || requirement.cooperative_launch()
            || !requirement.atomics().is_empty()
            || !requirement.synchronization().is_empty()
            || requirement.lds().static_bytes() != 0
            || requirement.lds().max_dynamic_bytes() != 0
        {
            return Err(error("complete closed Wave64/zero-LDS requirements"));
        }
        let launch = row.launch();
        let BlockSizeV1::Exact(block) = launch.block_size() else {
            return Err(error("exact launch block"));
        };
        let size = kernel
            .workgroup_size
            .ok_or_else(|| error("actual workgroup size"))?;
        let flat = size
            .x
            .checked_mul(size.y)
            .and_then(|n| n.checked_mul(size.z))
            .ok_or(Resource::Arithmetic)?;
        if (block.x(), block.y(), block.z()) != (size.x, size.y, size.z)
            || launch.rank() != kernel.domain.rank()
            || launch.max_flat_workgroup_size() != flat
            || launch.static_shared_memory_bytes() != 0
            || launch.max_dynamic_shared_memory_bytes() != 0
        {
            return Err(error("launch/workgroup/flat/LDS agreement"));
        }
        // max_grid is a genuine source launch restriction, not encoded in KIR's
        // dynamic domain. The mandatory source/launch join authenticates it.
    }
    budget.release_storage(scratch)?;
    drop(bindings);
    budget.release_storage(
        budget
            .storage()
            .checked_sub(binding_floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}

/// Complete actual closure including unreachable helpers and declarations;
/// missing or contradictory binding claims refuse before diagnostic discharge.
pub fn check_canonical_v12_descriptor_requirements_v3<'o, 'd, 'w>(
    output: &'o Owner,
    selected: Profile,
    table: &'d Table<'w>,
    budget: &mut Budget<'_>,
) -> R<ReplayedDescriptorRequirementsV3<'o, 'd, 'w>> {
    shared::scoped(budget, |budget| {
        shared::profile(table, selected, budget)?;
        let (inventory, receipt) = Inventory::derive(output, budget).map_err(E::Inventory)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let roots = shared::roots(&inventory, table, budget)?;
        check(&inventory, table, selected, &roots, budget)?;
        let retained = size_of::<ReplayedDescriptorRequirementsV3<'_, '_, '_>>();
        budget.charge_work(1)?;
        budget.reserve_storage(retained)?;
        Ok(ReplayedDescriptorRequirementsV3 {
            output,
            table,
            profile: selected,
            retained,
        })
    })
}

#[cfg(test)]
#[path = "descriptor_capability_projection_v3_tests.rs"]
mod tests;
