//! Keep physical backing distinct from the exposed SSA address space.
use super::*;

impl PointerValue {
    pub(super) fn logical_address_space(&self) -> AddressSpace {
        if self.exposed_generic {
            AddressSpace::Generic
        } else {
            self.address_space
        }
    }
}
impl SliceValue {
    pub(super) fn logical_address_space(&self) -> AddressSpace {
        if self.exposed_generic {
            AddressSpace::Generic
        } else {
            self.address_space
        }
    }
}

pub(super) fn expose(
    value: &RuntimeValue,
    kind: CastKind,
    to: &Type,
    target: SimulationTargetV1,
) -> Result<RuntimeValue, SimulationExecutionErrorKindV1> {
    let mismatch = || SimulationExecutionErrorKindV1::RuntimeType {
        value: None,
        expected: "exact concrete scalar pointer or slice exposure",
    };
    let (from, element, space, access) = match (kind, value, to) {
        (CastKind::PointerToGeneric, RuntimeValue::Pointer(pointer), Type::Pointer(to)) => (
            (
                pointer.element,
                pointer.logical_address_space(),
                pointer.access,
            ),
            to.pointee.as_ref(),
            to.address_space,
            to.access,
        ),
        (CastKind::SliceToGeneric, RuntimeValue::Slice(slice), Type::Slice(to)) => (
            (slice.element, slice.logical_address_space(), slice.access),
            to.element.as_ref(),
            to.address_space,
            to.access,
        ),
        _ => return Err(mismatch()),
    };
    let Type::Scalar(element) = element else {
        return Err(mismatch());
    };
    if !crate::generic_exposure_v18::supports_parts(kind, from, (*element, space, access), target) {
        return Err(mismatch());
    }
    let mut result = value.clone();
    match &mut result {
        RuntimeValue::Pointer(pointer) => pointer.exposed_generic = true,
        RuntimeValue::Slice(slice) => slice.exposed_generic = true,
        _ => return Err(mismatch()),
    }
    Ok(result)
}

#[cfg(test)]
#[path = "execute_generic_exposure_v18_tests.rs"]
mod tests;
