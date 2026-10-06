//! V18 private scalar cells and byte-backed record/array views in the admitted module.
use std::collections::HashMap;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Module, ScalarType, StorageLayoutIdV1, StorageLayoutKindV1,
    StorageOperationV1, StorageProjectionV1, Type, ValueId,
};

use crate::SimulationTargetV1;

pub(crate) fn scalar_layout(
    module: &Module,
    ty: &Type,
    target: SimulationTargetV1,
) -> Option<(StorageLayoutIdV1, ScalarType)> {
    let Type::StorageObject(id) = ty else {
        return None;
    };
    let row = module.storage_layouts.get(usize::try_from(id.0).ok()?)?;
    let StorageLayoutKindV1::Scalar(scalar) = &row.kind else {
        return None;
    };
    // Canonical Index rows are target-independent; execution must join the actual width.
    (row.size == u64::try_from(target.scalar_bytes(*scalar)?).ok()?).then_some((*id, *scalar))
}

pub(crate) fn private_pointer(
    module: &Module,
    ty: &Type,
    target: SimulationTargetV1,
) -> Option<(StorageLayoutIdV1, ScalarType)> {
    let Type::Pointer(pointer) = ty else {
        return None;
    };
    if pointer.address_space != AddressSpace::Private {
        return None;
    }
    scalar_layout(module, &pointer.pointee, target)
}

pub(crate) fn allocation_layout(
    module: &Module,
    ty: &Type,
    target: SimulationTargetV1,
) -> Option<(StorageLayoutIdV1, ScalarType, usize)> {
    let Type::StorageObject(id) = ty else {
        return None;
    };
    let row = module.storage_layouts.get(usize::try_from(id.0).ok()?)?;
    let element = match row.kind {
        StorageLayoutKindV1::Scalar(_) => scalar_layout(module, ty, target)?.1,
        // The scalar field in PointerValue is inert until a scalar leaf is reached.
        // Unsupported children cannot be projected, read, copied or reinterpreted.
        StorageLayoutKindV1::Record(_) | StorageLayoutKindV1::Array { .. } => ScalarType::U8,
        _ => return None,
    };
    Some((*id, element, usize::try_from(row.size).ok()?))
}

pub(crate) fn private_storage_pointer(
    module: &Module,
    ty: &Type,
    target: SimulationTargetV1,
) -> Option<StorageLayoutIdV1> {
    let Type::Pointer(pointer) = ty else {
        return None;
    };
    if pointer.address_space != AddressSpace::Private {
        return None;
    }
    allocation_layout(module, &pointer.pointee, target).map(|(id, _, _)| id)
}

pub(crate) fn supports_operation(
    module: &Module,
    operation: &StorageOperationV1,
    values: &HashMap<ValueId, &Type>,
    target: SimulationTargetV1,
) -> bool {
    if let StorageOperationV1::Project { base, step } = operation {
        let Some(id) = values
            .get(base)
            .and_then(|ty| private_storage_pointer(module, ty, target))
        else {
            return false;
        };
        let row = &module.storage_layouts[id.0 as usize];
        let child = match (&row.kind, step) {
            (StorageLayoutKindV1::Record(fields), StorageProjectionV1::Field(index)) => {
                fields.get(*index as usize).map(|field| field.layout)
            }
            (
                StorageLayoutKindV1::Array { element, .. },
                StorageProjectionV1::ArrayIndex(index),
            ) if matches!(values.get(index), Some(Type::Scalar(ScalarType::Index))) => {
                Some(*element)
            }
            _ => None,
        };
        return child.is_some_and(|id| {
            allocation_layout(module, &Type::StorageObject(id), target).is_some()
        });
    }
    let (address, access, writing) = match operation {
        StorageOperationV1::ReadValue { address, access } => (*address, access, false),
        StorageOperationV1::WriteValue {
            address, access, ..
        } => (*address, access, true),
        _ => return false,
    };
    let Some(ty) = values.get(&address) else {
        return false;
    };
    let Some((_, scalar)) = private_pointer(module, ty, target) else {
        return false;
    };
    let Type::Pointer(pointer) = ty else {
        return false;
    };
    if access.volatile
        || access.address_space != AddressSpace::Private
        || (writing && pointer.access == AccessMode::ReadOnly)
        || (!writing && pointer.access == AccessMode::WriteOnly)
    {
        return false;
    }
    match operation {
        StorageOperationV1::WriteValue { value, .. } => {
            matches!(values.get(value), Some(Type::Scalar(value)) if *value == scalar)
        }
        StorageOperationV1::ReadValue { .. } => true,
        _ => false,
    }
}
