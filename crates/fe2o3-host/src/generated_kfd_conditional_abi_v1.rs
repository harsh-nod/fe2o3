//! Complete inert ABI join, before reading the conditional slice roster.
use super::*;
use crate::generated_argument_plan::{
    GeneratedPackingComponentKindV1 as Kind, canonical_disjoint_slice_layout_v1,
    canonical_scalar_layout_v1, canonical_slice_layout_v1, descriptor_scalar_to_artifact,
    descriptor_scalar_to_rust_layout,
};
use fe2o3_artifacts::{PointerWidth, RustLayoutEvidenceV1, RustPhysicalComponentV1};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, ArgumentCursorV3, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1,
    LogicalArgumentRefV3, OwnershipSemantics, PhysicalAbiComponentKind, PhysicalComponentV3,
    SourceTypeRecordV3,
};

// Canonical legacy type/layout hashing has at most two physical components here.
// 4096 covers simultaneous framed encodings, vector growth and hash temporaries;
// nominal pointer-sized scalars and raw pointers are rejected before encoding.
const TYPE_IDENTITY_SCRATCH: usize = 4096;
pub(super) const ABI_SCRATCH_STORAGE: usize = TYPE_IDENTITY_SCRATCH
    + size_of::<ArgumentCursorV3<'static, 'static>>()
    + size_of::<LogicalArgumentRefV3<'static, 'static>>()
    + size_of::<SourceTypeRecordV3>()
    + size_of::<DeviceLayoutRecordV1>()
    + size_of::<DeviceLayoutDescriptorV1>()
    + size_of::<PhysicalComponentV3>()
    + size_of::<RustLayoutEvidenceV1>()
    + 2 * size_of::<RustPhysicalComponentV1>();

pub(super) fn validate(
    plan: &GeneratedArgumentPackingPlanV1,
    observation: &GeneratedKfdPackingObservationV1,
    table: &DeviceDescriptorTableV4<'_>,
    kernel: &KernelDescriptorRefV4<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(64)?;
    if plan.kernel_id() != kernel.kernel_id()
        || plan.pointer_width() != PointerWidth::Bits64
        || plan.kernarg_size() != u64::from(kernel.abi_layout().explicit_argument_size())
        || plan.kernarg_alignment() != kernel.abi_layout().kernarg_segment_alignment()
        || plan.argument_count() != kernel.argument_count()
        || plan.component_count() != kernel.component_count()
        || plan.component_count() != observation.components.len()
    {
        return Err(binding("complete generated ABI header"));
    }
    let mut cursor = kernel.arguments();
    let mut physical_index = 0;
    for index in 0..plan.argument_count() {
        let field = plan
            .argument(index)
            .ok_or_else(|| binding("generated field"))?;
        let row = cursor
            .next(&mut |n| budget.charge_work(n))?
            .ok_or_else(|| binding("missing logical field"))?;
        budget.charge_work(field.name().as_str().len())?;
        budget.charge_work(row.name().len())?;
        budget.charge_work(64)?;
        if usize::from(row.source_index()) != index || row.name() != field.name().as_str() {
            return Err(binding("logical field ordinal/name"));
        }
        let source = table.source_type(row.source_type(), &mut |n| budget.charge_work(n))?;
        let layout = table.device_layout(row.device_layout(), &mut |n| budget.charge_work(n))?;
        budget.charge_work(TYPE_IDENTITY_SCRATCH)?;
        let (expected_layout, expected_type, scalar) = match source.descriptor() {
            SourceTypeDescriptorV3::Scalar(s) => (
                DeviceLayoutDescriptorV1::scalar(s),
                canonical_scalar_layout_v1(
                    descriptor_scalar_to_rust_layout(s),
                    PointerWidth::Bits64,
                )
                .type_identity(),
                Some(s),
            ),
            SourceTypeDescriptorV3::SharedSlice(s) => (
                DeviceLayoutDescriptorV1::shared_slice(s),
                canonical_slice_layout_v1(
                    descriptor_scalar_to_rust_layout(s),
                    PointerWidth::Bits64,
                    false,
                )
                .type_identity(),
                None,
            ),
            SourceTypeDescriptorV3::DisjointSlice(s) => (
                DeviceLayoutDescriptorV1::disjoint_slice(s),
                canonical_disjoint_slice_layout_v1(
                    descriptor_scalar_to_rust_layout(s),
                    PointerWidth::Bits64,
                    RustDisjointIndexSpaceV1::Index1D,
                )
                .type_identity(),
                None,
            ),
            // This legacy sealed plan has no nominal usize/isize or raw-pointer
            // custody. Equal machine width cannot supply the missing source type.
            _ => return Err(binding("unsupported generated nominal source")),
        };
        if layout.descriptor() != &expected_layout
            || field.type_identity() != expected_type
            || field.size() != u64::from(expected_layout.size_bytes())
            || field.alignment() != u32::from(expected_layout.alignment_bytes())
        {
            return Err(binding("logical source/layout identity"));
        }
        let ownership = match row.ownership() {
            OwnershipSemantics::ByValue => ArgumentOwnership::ByValue,
            OwnershipSemantics::SharedBorrow => ArgumentOwnership::SharedBorrow,
            OwnershipSemantics::UniqueBorrow => ArgumentOwnership::UniqueBorrow,
        };
        let access = match row.access() {
            AccessMode::ByValue => Access::ByValue,
            AccessMode::ReadOnly => Access::ReadOnly,
            AccessMode::WriteOnly => Access::WriteOnly,
            AccessMode::ReadWrite => Access::ReadWrite,
        };
        let alias = match row.alias() {
            AliasSemantics::Value => AliasClass::Value,
            AliasSemantics::SharedReadOnly => AliasClass::SharedReadOnly,
            AliasSemantics::Exclusive => AliasClass::Exclusive,
        };
        let mutable = if ownership == ArgumentOwnership::UniqueBorrow {
            Mutability::Mutable
        } else {
            Mutability::Immutable
        };
        let space = if scalar.is_some() {
            AddressSpace::Value
        } else {
            AddressSpace::Global
        };
        if field.ownership() != ownership
            || field.access() != access
            || field.alias_class() != alias
            || field.mutability() != mutable
            || field.address_space() != space
        {
            return Err(binding("logical field effects"));
        }
        let component_count = if scalar.is_some() { 1 } else { 2 };
        if row.component_count() != component_count {
            return Err(binding("logical component count"));
        }
        match (scalar, field.kind()) {
            (Some(s), AbiKind::Scalar(actual)) if descriptor_scalar_to_artifact(s) == actual => {}
            (
                None,
                AbiKind::Slice {
                    element_size,
                    element_alignment,
                },
            ) if element_size
                == descriptor_scalar_to_rust_layout(expected_layout.scalar_type()).size_bytes()
                && u64::from(element_alignment) == element_size => {}
            _ => return Err(binding("logical field kind")),
        }
        for part in 0..component_count {
            let component = row.component(part, &mut |n| budget.charge_work(n))?;
            budget.charge_work(64)?;
            let (kind, generated_kind, size, alignment, access, alias) = match (scalar, part) {
                (Some(s), 0) => (
                    PhysicalAbiComponentKind::ScalarByValue(s),
                    Kind::Scalar,
                    field.size(),
                    field.alignment(),
                    AccessMode::ByValue,
                    AliasSemantics::Value,
                ),
                (None, 0) => (
                    PhysicalAbiComponentKind::GlobalPointer,
                    Kind::SlicePointer,
                    8,
                    8,
                    row.access(),
                    row.alias(),
                ),
                (None, 1) => (
                    PhysicalAbiComponentKind::SliceLengthU64,
                    Kind::SliceLength,
                    8,
                    8,
                    AccessMode::ByValue,
                    AliasSemantics::Value,
                ),
                _ => return Err(binding("physical component kind")),
            };
            let offset = field
                .offset()
                .checked_add(8 * part as u64)
                .ok_or(Resource::Arithmetic)?;
            let generated = plan
                .component(physical_index)
                .ok_or_else(|| binding("generated component"))?;
            if component.kind != kind
                || u64::from(component.offset) != offset
                || u64::from(component.size) != size
                || u32::from(component.alignment) != alignment
                || component.access != access
                || component.alias != alias
                || generated.argument_index() != index
                || generated.kind() != generated_kind
                || generated.offset() != offset
                || generated.size() != size
                || generated.alignment() != alignment
                || observation.components[physical_index] != generated
            {
                return Err(binding("complete physical ABI/packing observation"));
            }
            physical_index += 1;
        }
    }
    if cursor.next(&mut |n| budget.charge_work(n))?.is_some()
        || physical_index != plan.component_count()
    {
        return Err(binding("complete logical/physical roster"));
    }
    Ok(())
}
