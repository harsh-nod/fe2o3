//! Logical Generic exposure never changes the concrete allocation or permissions.
use crate::SimulationTargetV1;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, CastKind, ScalarType, Type};

pub(crate) fn supports_type(ty: &Type, target: SimulationTargetV1) -> bool {
    let (element, space) = match ty {
        Type::Pointer(pointer) => (pointer.pointee.as_ref(), pointer.address_space),
        Type::Slice(slice) => (slice.element.as_ref(), slice.address_space),
        _ => return false,
    };
    space == AddressSpace::Generic
        && matches!(element, Type::Scalar(scalar) if target.scalar_bits(*scalar).is_some())
}

pub(crate) fn supports_parts(
    kind: CastKind,
    from: (ScalarType, AddressSpace, AccessMode),
    to: (ScalarType, AddressSpace, AccessMode),
    target: SimulationTargetV1,
) -> bool {
    let admitted_source = match kind {
        CastKind::PointerToGeneric => matches!(
            from.1,
            AddressSpace::Global
                | AddressSpace::Private
                | AddressSpace::Workgroup
                | AddressSpace::Constant
        ),
        CastKind::SliceToGeneric => matches!(from.1, AddressSpace::Global | AddressSpace::Constant),
        _ => false,
    };
    admitted_source
        && (from.1 != AddressSpace::Constant || from.2 == AccessMode::ReadOnly)
        && from.0 == to.0
        && from.2 == to.2
        && to.1 == AddressSpace::Generic
        && target.scalar_bits(to.0).is_some()
}

pub(crate) fn supports_cast(
    kind: CastKind,
    from: &Type,
    to: &Type,
    target: SimulationTargetV1,
) -> bool {
    let (source_element, source_space, source_access, element, space, access) =
        match (kind, from, to) {
            (CastKind::PointerToGeneric, Type::Pointer(from), Type::Pointer(to)) => (
                from.pointee.as_ref(),
                from.address_space,
                from.access,
                to.pointee.as_ref(),
                to.address_space,
                to.access,
            ),
            (CastKind::SliceToGeneric, Type::Slice(from), Type::Slice(to)) => (
                from.element.as_ref(),
                from.address_space,
                from.access,
                to.element.as_ref(),
                to.address_space,
                to.access,
            ),
            _ => return false,
        };
    let (Type::Scalar(from), Type::Scalar(to)) = (source_element, element) else {
        return false;
    };
    supports_parts(
        kind,
        (*from, source_space, source_access),
        (*to, space, access),
        target,
    )
}
