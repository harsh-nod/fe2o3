//! Structural V5 field agreement. The original collector type witness is absent.
use super::*;
use fe2o3_kernel_descriptor::{
    AccessMode as Access, AliasSemantics as Alias, ConditionalArgumentRoleV1 as Role,
    DeviceLayoutDescriptorV1 as DeviceLayout, LogicalArgumentRefV3 as Argument,
    OwnershipSemantics as Ownership, PhysicalAbiComponentKind as Component, ScalarTypeV1 as Scalar,
    SourceTypeDescriptorV3 as SourceType, SourceTypeRecordV3, device_layout_record_v3,
};
use fe2o3_kernel_ir::{AccessMode as KirAccess, AddressSpace, ScalarType as KirScalar, Type};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentV1, SemanticAbiExtensionV1, SemanticAbiPassModeV1 as Mode,
    SemanticBackendPrimitiveV1 as Primitive, SemanticBackendReprV1 as Repr,
    SemanticBackendScalarV1 as BackendScalar, SemanticRustTypeKindV1 as Kind,
    SemanticScalarTypeV1 as SourceScalar, SemanticSourceArgumentOwnershipV1 as SourceOwnership,
    SemanticTypeDeclV1 as Decl, SemanticTypeShapeV1 as Shape,
};

pub(super) const STORAGE: usize = 2 * size_of::<SourceTypeRecordV3>()
    + 2 * size_of::<fe2o3_kernel_descriptor::DeviceLayoutRecordV1>()
    + size_of::<fe2o3_kernel_descriptor::PhysicalComponentV3>()
    + size_of::<fe2o3_kernel_descriptor::ArgumentCursorV3<'static, 'static>>()
    + size_of::<Argument<'static, 'static>>()
    + size_of::<DeviceLayout>()
    + size_of::<Layout>();

pub(super) struct Layout {
    offset: u32,
    alignment: u32,
    components: usize,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            offset: 0,
            alignment: 1,
            components: 0,
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Borrow existing fields, not another authenticated owner."
)]
pub(super) fn field(
    table: &Table<'_>,
    arg: &Argument<'_, '_>,
    index: usize,
    ty: &Decl,
    adjusted: &SemanticAbiArgumentV1,
    source_ownership: SourceOwnership,
    canonical: &Type,
    state: &mut Layout,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(192)?;
    let source = table.source_type(arg.source_type(), &mut |w| budget.charge_work(w))?;
    let actual_layout = table.device_layout(arg.device_layout(), &mut |w| budget.charge_work(w))?;
    let kind = source.descriptor();
    let scalar = kind.physical_scalar();
    let expected_layout = match kind {
        SourceType::Scalar(_) | SourceType::Usize | SourceType::Isize => {
            DeviceLayout::scalar(scalar)
        }
        SourceType::SharedSlice(_) => DeviceLayout::shared_slice(scalar),
        SourceType::DisjointSlice(_) => DeviceLayout::disjoint_slice(scalar),
        SourceType::GlobalMutPointer(_) => DeviceLayout::global_mut_pointer(scalar),
    };
    let expected_source = SourceTypeRecordV3::new(kind, &mut |w| budget.charge_work(w))?;
    let expected_physical =
        device_layout_record_v3(expected_layout.clone(), &mut |w| budget.charge_work(w))?;
    require(
        source.identity() == expected_source.identity() && actual_layout == expected_physical,
        "nominal/layout record content",
    )?;
    let slice = matches!(
        kind,
        SourceType::SharedSlice(_) | SourceType::DisjointSlice(_)
    );
    let by_value = matches!(
        kind,
        SourceType::Scalar(_) | SourceType::Usize | SourceType::Isize
    );
    let (ownership, alias, expected_ownership) = if by_value {
        (Ownership::ByValue, Alias::Value, SourceOwnership::ByValue)
    } else if matches!(kind, SourceType::SharedSlice(_)) {
        (
            Ownership::SharedBorrow,
            Alias::SharedReadOnly,
            SourceOwnership::SharedBorrow,
        )
    } else {
        (
            Ownership::UniqueBorrow,
            Alias::Exclusive,
            SourceOwnership::ExclusiveOwner,
        )
    };
    require(
        usize::from(arg.source_index()) == index
            && arg.ownership() == ownership
            && arg.alias() == alias
            && source_ownership == expected_ownership
            && shape(kind, canonical, arg.access())
            && matches!(
                (slice, adjusted.mode()),
                (true, Mode::Pair { .. }) | (false, Mode::Direct(_))
            )
            && ty.layout().size_bytes() == Some(u64::from(expected_layout.size_bytes()))
            && ty.layout().alignment_bytes() == u64::from(expected_layout.alignment_bytes()),
        "nominal source/N/physical argument",
    )?;
    nominal_type(ty, kind, adjusted)?;
    let alignment = u32::from(expected_layout.alignment_bytes());
    state.alignment = state.alignment.max(alignment);
    state.offset = align(state.offset, alignment)?;
    let count = if slice { 2 } else { 1 };
    require(arg.component_count() == count, "physical component count")?;
    for i in 0..count {
        budget.charge_work(12)?;
        let actual = arg.component(i, &mut |w| budget.charge_work(w))?;
        let (kind, offset, size, alignment) = if i == 1 {
            (
                Component::SliceLengthU64,
                state.offset.checked_add(8).ok_or(Resource::Arithmetic)?,
                8,
                8,
            )
        } else if by_value {
            (
                Component::ScalarByValue(scalar),
                state.offset,
                expected_layout.size_bytes(),
                expected_layout.alignment_bytes(),
            )
        } else {
            (Component::GlobalPointer, state.offset, 8, 8)
        };
        let (access, alias) = if i == 1 {
            (Access::ByValue, Alias::Value)
        } else {
            (arg.access(), arg.alias())
        };
        require(
            (
                actual.kind,
                actual.offset,
                actual.size,
                actual.alignment,
                actual.access,
                actual.alias,
            ) == (kind, offset, size, alignment, access, alias),
            "exact physical component",
        )?;
    }
    state.components = state
        .components
        .checked_add(count)
        .ok_or(Resource::Arithmetic)?;
    state.offset = state
        .offset
        .checked_add(u32::from(expected_layout.size_bytes()))
        .ok_or(Resource::Arithmetic)?;
    Ok(())
}

pub(super) fn finish(kernel: &Kernel<'_, '_>, state: Layout, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(5)?;
    let offset = align(state.offset, state.alignment)?;
    let layout = kernel.abi_layout();
    require(
        kernel.component_count() == state.components
            && layout.explicit_argument_size() == offset
            && layout.kernarg_segment_size()
                == offset.checked_add(256).ok_or(Resource::Arithmetic)?
            && layout.kernarg_segment_alignment() == state.alignment,
        "complete physical kernarg layout",
    )
}

pub(super) fn role(
    table: &Table<'_>,
    arg: &Argument<'_, '_>,
    role: Role,
    budget: &mut Budget<'_>,
) -> R<()> {
    let kind = table
        .source_type(arg.source_type(), &mut |w| budget.charge_work(w))?
        .descriptor();
    budget.charge_work(3)?;
    require(
        role_matches(kind, arg.access(), role),
        "whole conditional slice role",
    )
}
pub(super) fn role_matches(kind: SourceType, access: Access, role: Role) -> bool {
    match role {
        Role::Input => matches!(kind, SourceType::SharedSlice(_)) && access == Access::ReadOnly,
        Role::Output => {
            matches!(kind, SourceType::DisjointSlice(_))
                && matches!(access, Access::WriteOnly | Access::ReadWrite)
        }
    }
}

fn nominal_type(ty: &Decl, declared: SourceType, adjusted: &SemanticAbiArgumentV1) -> R<()> {
    match (ty.rust_type_kind(), declared) {
        (Kind::Ordinary, SourceType::Scalar(s)) => require(
            ty.shape() == &Shape::Scalar(scalar(s).1),
            "source scalar shape",
        ),
        (
            Kind::Ordinary,
            SourceType::SharedSlice(_)
            | SourceType::DisjointSlice(_)
            | SourceType::GlobalMutPointer(_),
        ) => Ok(()),
        (Kind::Usize, SourceType::Usize) | (Kind::Isize, SourceType::Isize) => {
            let signed = declared == SourceType::Isize;
            require(
                ty.shape() == &Shape::Scalar(SourceScalar::Integer { signed, bits: 64 })
                    && ty.layout().size_bytes() == Some(8)
                    && ty.layout().alignment_bytes() == 8
                    && matches!(adjusted.mode(), Mode::Direct(attrs) if attrs.extension() == SemanticAbiExtensionV1::None)
                    && matches!(ty.layout().backend_repr(), Repr::Scalar(BackendScalar::Initialized { primitive, .. })
                    if *primitive == Primitive::integer(signed, 64, 8)),
                "nominal pointer-width scalar",
            )
        }
        _ => Err(E::Mismatch("nominal source kind")),
    }
}
pub(super) fn align(offset: u32, alignment: u32) -> Result<u32, Resource> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(Resource::Accounting);
    }
    let n = offset
        .checked_add(alignment - 1)
        .ok_or(Resource::Arithmetic)?;
    Ok(n / alignment * alignment)
}
pub(super) fn shape(source: SourceType, ty: &Type, access: Access) -> bool {
    let element = scalar(source.physical_scalar()).0;
    match (source, ty) {
        (SourceType::Scalar(_) | SourceType::Usize | SourceType::Isize, Type::Scalar(actual)) => {
            *actual == element && access == Access::ByValue
        }
        (SourceType::SharedSlice(_), Type::Slice(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.element.as_scalar() == Some(element)
                && access == Access::ReadOnly
                && actual.access == KirAccess::ReadOnly
        }
        (SourceType::DisjointSlice(_), Type::Slice(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.element.as_scalar() == Some(element)
                && matches!(
                    (access, actual.access),
                    (Access::WriteOnly, KirAccess::WriteOnly)
                        | (Access::ReadWrite, KirAccess::ReadWrite)
                )
        }
        (SourceType::GlobalMutPointer(_), Type::Pointer(actual)) => {
            actual.address_space == AddressSpace::Global
                && actual.pointee.as_scalar() == Some(element)
                && access == Access::ReadWrite
                && actual.access == KirAccess::ReadWrite
        }
        _ => false,
    }
}
fn scalar(value: Scalar) -> (KirScalar, SourceScalar) {
    use SourceScalar::{Float, Integer};
    match value {
        Scalar::I8 => (
            KirScalar::I8,
            Integer {
                signed: true,
                bits: 8,
            },
        ),
        Scalar::U8 => (
            KirScalar::U8,
            Integer {
                signed: false,
                bits: 8,
            },
        ),
        Scalar::I16 => (
            KirScalar::I16,
            Integer {
                signed: true,
                bits: 16,
            },
        ),
        Scalar::U16 => (
            KirScalar::U16,
            Integer {
                signed: false,
                bits: 16,
            },
        ),
        Scalar::I32 => (
            KirScalar::I32,
            Integer {
                signed: true,
                bits: 32,
            },
        ),
        Scalar::U32 => (
            KirScalar::U32,
            Integer {
                signed: false,
                bits: 32,
            },
        ),
        Scalar::I64 => (
            KirScalar::I64,
            Integer {
                signed: true,
                bits: 64,
            },
        ),
        Scalar::U64 => (
            KirScalar::U64,
            Integer {
                signed: false,
                bits: 64,
            },
        ),
        Scalar::F16 => (KirScalar::F16, Float { bits: 16 }),
        Scalar::F32 => (KirScalar::F32, Float { bits: 32 }),
        Scalar::F64 => (KirScalar::F64, Float { bits: 64 }),
    }
}
