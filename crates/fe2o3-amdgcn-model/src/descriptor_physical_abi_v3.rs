//! Actual KIR representation compatibility; never nominal Rust/borrow provenance.
use crate::native_v12_text_descriptor_replay_v3::{self as shared, E, R, Root};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_descriptor::{
    AccessMode as Access, AliasSemantics as Alias, DeviceDescriptorTableV3 as Table,
    DeviceLayoutDescriptorV1 as Layout, OwnershipSemantics as Ownership,
    PhysicalAbiComponentKind as Component, ScalarTypeV1 as Scalar,
    SourceTypeDescriptorV3 as Source, SourceTypeRecordV3, device_layout_record_v3,
};
use fe2o3_kernel_ir::{
    AccessMode as KirAccess, AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, FunctionRole,
    ScalarType as KirScalar, Type, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use shared::queries::{KernelQuery, TableQuery};
use std::mem::size_of;

/// This is only physical compatibility. KIR lacks unique-borrow/disjoint source
/// identities, and usize/U64 or isize/I64 may share a representation. A genuine
/// independent source/ownership/nominal/formal join remains mandatory.
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedDescriptorPhysicalAbiV3 as R;
/// fn duplicate(v: R<'_, '_, '_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedDescriptorPhysicalAbiV3 as R;
/// fn forge<'a>() -> R<'a, 'a, 'a> { R::default() }
/// ```
pub struct ReplayedDescriptorPhysicalAbiV3<'o, 'd, 'w> {
    output: &'o Owner,
    table: &'d Table<'w>,
    retained: usize,
}
impl<'o, 'd, 'w> ReplayedDescriptorPhysicalAbiV3<'o, 'd, 'w> {
    pub const fn output(&self) -> &'o Owner {
        self.output
    }
    pub const fn descriptors(&self) -> &'d Table<'w> {
        self.table
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_source_ownership(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn scalar(value: Scalar) -> KirScalar {
    match value {
        Scalar::I8 => KirScalar::I8,
        Scalar::U8 => KirScalar::U8,
        Scalar::I16 => KirScalar::I16,
        Scalar::U16 => KirScalar::U16,
        Scalar::I32 => KirScalar::I32,
        Scalar::U32 => KirScalar::U32,
        Scalar::I64 => KirScalar::I64,
        Scalar::U64 => KirScalar::U64,
        Scalar::F16 => KirScalar::F16,
        Scalar::F32 => KirScalar::F32,
        Scalar::F64 => KirScalar::F64,
    }
}
fn shape(source: Source, ty: &Type, access: Access) -> bool {
    let element = scalar(source.physical_scalar());
    match (source, ty) {
        (Source::Scalar(_) | Source::Usize | Source::Isize, Type::Scalar(actual)) => {
            *actual == element && access == Access::ByValue
        }
        (Source::SharedSlice(_), Type::Slice(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.element.as_scalar() == Some(element)
                && access == Access::ReadOnly
                && actual.access == KirAccess::ReadOnly
        }
        (Source::DisjointSlice(_), Type::Slice(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.element.as_scalar() == Some(element)
                && matches!(
                    (access, actual.access),
                    (Access::WriteOnly, KirAccess::WriteOnly)
                        | (Access::ReadWrite, KirAccess::ReadWrite)
                )
        }
        (Source::GlobalMutPointer(_), Type::Pointer(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.pointee.as_scalar() == Some(element)
                && access == Access::ReadWrite
                && actual.access == KirAccess::ReadWrite
        }
        _ => false,
    }
}
fn align(value: u32, alignment: u32) -> R<u32> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(E::Invalid("physical alignment"));
    }
    Ok(value
        .checked_add(alignment - 1)
        .ok_or(Resource::Arithmetic)?
        / alignment
        * alignment)
}
fn arguments<'wire: 'view, 'view, D: TableQuery<'wire> + 'view>(
    table: &'view D,
    row: &D::Kernel<'view>,
    function: &Function,
    root: usize,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(4)?;
    let count = row.argument_count();
    if function.role != FunctionRole::KernelEntry
        || !function.signature.results.is_empty()
        || function.body.is_none()
        || count != function.signature.parameters.len()
    {
        return Err(E::Physical {
            root,
            argument: None,
            field: "entry signature",
        });
    }
    let mut cursor = row.arguments();
    let mut offset = 0;
    let mut alignment = 1;
    let mut components = 0usize;
    for index in 0..count {
        budget.charge_work(192)?;
        let error = |field| E::Physical {
            root,
            argument: Some(index),
            field,
        };
        let argument = cursor
            .next(&mut |w| budget.charge_work(w))
            .map_err(E::Descriptor)?
            .ok_or_else(|| error("argument count"))?;
        let source = table.source_type(argument.source_type(), budget)?;
        let layout = table.device_layout(argument.device_layout(), budget)?;
        let kind = source.descriptor();
        let element = kind.physical_scalar();
        let expected = match kind {
            Source::Scalar(_) | Source::Usize | Source::Isize => Layout::scalar(element),
            Source::SharedSlice(_) => Layout::shared_slice(element),
            Source::DisjointSlice(_) => Layout::disjoint_slice(element),
            Source::GlobalMutPointer(_) => Layout::global_mut_pointer(element),
        };
        let expected_source =
            SourceTypeRecordV3::new(kind, &mut |w| budget.charge_work(w)).map_err(E::Descriptor)?;
        let expected_layout =
            device_layout_record_v3(expected.clone(), &mut |w| budget.charge_work(w))
                .map_err(E::Descriptor)?;
        if source != expected_source || layout != expected_layout {
            return Err(error("record identity/layout"));
        }
        let by_value = matches!(kind, Source::Scalar(_) | Source::Usize | Source::Isize);
        let slice = matches!(kind, Source::SharedSlice(_) | Source::DisjointSlice(_));
        let (ownership, alias) = if by_value {
            (Ownership::ByValue, Alias::Value)
        } else if matches!(kind, Source::SharedSlice(_)) {
            (Ownership::SharedBorrow, Alias::SharedReadOnly)
        } else {
            (Ownership::UniqueBorrow, Alias::Exclusive)
        };
        // This checks record consistency and actual physical access, not whether
        // a source Rust argument really carried an exclusive ownership proof.
        if usize::from(argument.source_index()) != index
            || argument.ownership() != ownership
            || argument.alias() != alias
            || !shape(
                kind,
                &function.signature.parameters[index],
                argument.access(),
            )
        {
            return Err(error("physical kind/access/record ownership"));
        }
        let a = u32::from(expected.alignment_bytes());
        alignment = alignment.max(a);
        offset = align(offset, a)?;
        let expected_components = if slice { 2 } else { 1 };
        if argument.component_count() != expected_components {
            return Err(error("component count"));
        }
        components = components
            .checked_add(expected_components)
            .ok_or(Resource::Arithmetic)?;
        for index in 0..expected_components {
            let actual = argument
                .component(index, &mut |w| budget.charge_work(w))
                .map_err(E::Descriptor)?;
            let (kind, at, size, alignment) = if index == 1 {
                (
                    Component::SliceLengthU64,
                    offset.checked_add(8).ok_or(Resource::Arithmetic)?,
                    8,
                    8,
                )
            } else if by_value {
                (
                    Component::ScalarByValue(element),
                    offset,
                    expected.size_bytes(),
                    expected.alignment_bytes(),
                )
            } else {
                (Component::GlobalPointer, offset, 8, 8)
            };
            let (access, alias) = if index == 1 {
                (Access::ByValue, Alias::Value)
            } else {
                (argument.access(), argument.alias())
            };
            if (
                actual.kind,
                actual.offset,
                actual.size,
                actual.alignment,
                actual.access,
                actual.alias,
            ) != (kind, at, size, alignment, access, alias)
            {
                return Err(error("physical component"));
            }
        }
        offset = offset
            .checked_add(u32::from(expected.size_bytes()))
            .ok_or(Resource::Arithmetic)?;
    }
    budget.charge_work(8)?;
    if cursor
        .next(&mut |w| budget.charge_work(w))
        .map_err(E::Descriptor)?
        .is_some()
        || components != row.component_count()
    {
        return Err(E::Physical {
            root,
            argument: None,
            field: "complete argument/component roster",
        });
    }
    offset = align(offset, alignment)?;
    let layout = row.abi_layout();
    if layout.explicit_argument_size() != offset
        || layout.kernarg_segment_size() != offset.checked_add(256).ok_or(Resource::Arithmetic)?
        || layout.kernarg_segment_alignment() != alignment
    {
        return Err(E::Physical {
            root,
            argument: None,
            field: "explicit kernarg and COV6 tail",
        });
    }
    Ok(())
}
pub(super) fn check<'wire, D: TableQuery<'wire>>(
    inventory: &Inventory<'_>,
    table: &D,
    roster: &[Root],
    budget: &mut Budget<'_>,
) -> R<()> {
    let scratch = D::QUERY_STORAGE
        .checked_mul(4)
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(scratch)?;
    for root in roster {
        budget.charge_work(3)?;
        let kernel = &inventory.kernels()[root.kernel];
        let function = inventory
            .functions()
            .get(kernel.entry.0 as usize)
            .ok_or(E::Invalid("inventory entry"))?
            .function;
        let row = table.kernel(root.descriptor, budget)?;
        arguments(table, &row, function, root.kernel, budget)?;
    }
    budget.release_storage(scratch)?;
    Ok(())
}

/// Independently derives an actual-owner inventory and exact root bijection.
/// Caller retains prepaid inputs; success transfers only this borrowed header.
pub fn check_canonical_v12_descriptor_physical_abi_v3<'o, 'd, 'w>(
    output: &'o Owner,
    selected: Profile,
    table: &'d Table<'w>,
    budget: &mut Budget<'_>,
) -> R<ReplayedDescriptorPhysicalAbiV3<'o, 'd, 'w>> {
    shared::scoped(budget, |budget| {
        shared::profile(table, selected, budget)?;
        let (inventory, receipt) = Inventory::derive(output, budget).map_err(E::Inventory)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let roots = shared::roots(&inventory, table, budget)?;
        check(&inventory, table, &roots, budget)?;
        let retained = size_of::<ReplayedDescriptorPhysicalAbiV3<'_, '_, '_>>();
        budget.charge_work(1)?;
        budget.reserve_storage(retained)?;
        Ok(ReplayedDescriptorPhysicalAbiV3 {
            output,
            table,
            retained,
        })
    })
}

#[cfg(test)]
#[path = "descriptor_physical_abi_v3_tests.rs"]
mod tests;
