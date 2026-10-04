use super::*;
use fe2o3_kernel_descriptor::{BlockSizeV1, DESCRIPTOR_QUERY_STORAGE_V5, KernelDescriptorRefV5};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
use fe2o3_lower_mir_kernel::ProductionSourceLaunchRootInputV1;

pub(super) fn check(
    source: &ReplayedNativeSourceV1,
    packet: &NativeConditionalSourcePacketInputV2<'_>,
    output: &VerifiedCanonicalKernelIrModuleV12,
    table: &DeviceDescriptorTableV5<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    account::temporary(budget, DESCRIPTOR_QUERY_STORAGE_V5, |budget| {
        let semantic = source.source().semantic_ssa().source_semantic();
        let launches = source.source().source_launch().roots();
        budget.charge_work(7)?;
        if packet.roots.is_empty()
            || packet.roots.len() != semantic.roots().len()
            || packet.roots.len() != launches.len()
            || packet.roots.len() != source.source().executable().module().kernels.len()
            || packet.roots.len() != output.module().kernels.len()
            || packet.roots.len() != table.kernel_count()
            || packet.roots.len() != packet.canonical_kernel_order.len()
        {
            return Err(Error::mismatch("complete source/N/F/V5 root roster"));
        }
        // C1 checked a full-binding sorted permutation, not a name selector.
        for (descriptor_ordinal, &source_ordinal) in
            packet.canonical_kernel_order.iter().enumerate()
        {
            budget.charge_work(103)?;
            let ordinal = usize::try_from(source_ordinal).map_err(|_| Resource::Arithmetic)?;
            let row = packet
                .roots
                .get(ordinal)
                .ok_or_else(|| Error::mismatch("source root ordinal"))?;
            let root = semantic.roots()[ordinal];
            let function = &semantic.functions()[root.index() as usize];
            let entry = function
                .kernel_entry()
                .ok_or_else(|| Error::mismatch("source kernel entry"))?;
            let launch = launches[ordinal];
            if row.semantic_root != root.index()
                || launch.selected_root() != root
                || launch.kernel_binding() != row.launch.kernel_binding()
                || row.launch.kernel_binding() != *entry.kernel_binding_identity().as_bytes()
                || launch.source_launch() != row.launch.launch()
                || row.launch_rank != launch.source_rank()
            {
                return Err(Error::mismatch("exact source root/binding/launch"));
            }
            let descriptor = table
                .kernel(descriptor_ordinal, &mut |n| budget.charge_work(n))
                .map_err(|error| Error(Cause::Descriptor(error)))?;
            check_descriptor(
                row.launch,
                entry.export_symbol().as_bytes(),
                &descriptor,
                budget,
            )?;

            let mut found = false;
            for kernel in &output.module().kernels {
                budget.charge_work(
                    kernel
                        .id
                        .as_str()
                        .len()
                        .checked_add(kernel.entry.as_str().len())
                        .and_then(|n| n.checked_add(entry.export_symbol().as_bytes().len()))
                        .and_then(|n| n.checked_add(4))
                        .ok_or(Resource::Arithmetic)?,
                )?;
                if kernel.id.as_str().as_bytes() == entry.export_symbol().as_bytes() {
                    if found
                        || kernel.entry.as_str() != kernel.id.as_str()
                        || kernel.domain.rank() != row.launch_rank
                    {
                        return Err(Error::mismatch("exact bound F kernel entry/rank"));
                    }
                    found = true;
                }
            }
            if !found {
                return Err(Error::mismatch("missing bound F kernel"));
            }
        }
        Ok(())
    })
}

fn check_descriptor(
    source: ProductionSourceLaunchRootInputV1<'_>,
    export: &[u8],
    descriptor: &KernelDescriptorRefV5<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let lengths = [
        source.logical_name().len(),
        descriptor.logical_name().len(),
        export.len(),
        descriptor.entry_name().len(),
        export.len(),
        descriptor.descriptor_symbol().len(),
    ];
    budget.charge_work(
        lengths
            .into_iter()
            .try_fold(48usize, |n, len| n.checked_add(len))
            .ok_or(Resource::Arithmetic)?,
    )?;
    let launch = descriptor.launch();
    let workgroup = match launch.block_size() {
        BlockSizeV1::Exact(v) => Some([v.x(), v.y(), v.z()]),
        _ => None,
    };
    let grid = launch.max_grid();
    if descriptor.kernel_id() != KernelId::from_bytes(source.kernel_binding())
        || descriptor.logical_name() != source.logical_name()
        || descriptor.entry_name().as_bytes() != export
        || descriptor
            .descriptor_symbol()
            .strip_suffix(".kd")
            .map(str::as_bytes)
            != Some(export)
        || launch.rank() != source.launch().rank()
        || workgroup.is_none()
        || workgroup != source.launch().exact_workgroup()
        || [grid.x(), grid.y(), grid.z()] != source.launch().max_grid()
    {
        return Err(Error::mismatch(
            "exact source/V5 binding/name/export/launch",
        ));
    }
    // Actual-F ABI/capabilities/flat-size/LDS and exact text are checked last by A.
    Ok(())
}

#[cfg(test)]
#[path = "roster_tests.rs"]
mod tests;
