//! Physical typed-scalar adapter. No source slot, lifetime or native role is created.
use super::*;
use crate::{CanonicalKirDefinitionRefV1, CanonicalKirInventoryV18, CanonicalKirOperationRefV1};
use fe2o3_kernel_ir::{
    AccessMode, CastKind, MemoryAccess, Module, PointerType, ScalarType, StorageLayoutIdV1,
    StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1, VerifiedCanonicalKernelIrModuleV12,
    VerifiedCanonicalKernelIrModuleV18,
};

// The owner mode cannot be supplied by a caller or changed after construction.
pub(super) trait PrivateMemoryOwner {
    const TYPED: bool;
    fn layouts(&self) -> &[StorageLayoutV1];
}
impl PrivateMemoryOwner for VerifiedCanonicalKernelIrModuleV12 {
    const TYPED: bool = false;
    fn layouts(&self) -> &[StorageLayoutV1] {
        &[]
    }
}
impl PrivateMemoryOwner for VerifiedCanonicalKernelIrModuleV18 {
    const TYPED: bool = true;
    fn layouts(&self) -> &[StorageLayoutV1] {
        &self.module().storage_layouts
    }
}

/// Exact physical initialization of this V18 inventory only. Different reaching
/// stores can establish initialization without an exact `latest_stores` anchor.
/// This is not a source allocation/slot-generation correspondence, value
/// equivalence, or native memory permission.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CheckedCanonicalKirPrivateMemoryV1,
///     CheckedCanonicalKirPrivateMemoryV18};
/// fn old_consumer(_: &CheckedCanonicalKirPrivateMemoryV1<'_, '_>) {}
/// fn no_conversion(proof: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>) {
///     old_consumer(proof);
/// }
/// ```
pub type CheckedCanonicalKirPrivateMemoryV18<'a, 'g> =
    CheckedCanonicalKirPrivateMemoryV1<'a, 'g, VerifiedCanonicalKernelIrModuleV18>;

impl CheckedCanonicalKirPrivateMemoryV18<'_, '_> {
    /// The authenticated direct typed allocation of an exact RW-to-RO cast.
    /// This allocation-free query grants neither source nor native authority;
    /// the cast remains absent from the memory-operation census. Consumers
    /// account for their query frames and work, as for `address`/`operation`.
    pub fn access_restriction(
        &self,
        operation: usize,
    ) -> Option<&CanonicalKirPrivateMemoryAddressV1> {
        let row = self.inventory().operations().get(operation)?;
        if self.operation(operation)
            || row.results.len() != 1
            || !matches!(
                row.operation.kind,
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    ..
                }
            )
        {
            return None;
        }
        self.address(row.results.start)
    }
}

/// Checks whole scalar `StorageObject` reads/writes in addition to the unchanged
/// ordinary private-scalar family. The layout table comes only from the exact
/// immutable inventory owner. Reserve the returned receipt immediately, as for
/// the V1 physical constructor. No diagnostic record is accepted as input.
///
/// ```no_run
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV18,
///     CanonicalKirPrivateMemoryLimitsV1, CanonicalKirPrivateMemoryStorageV1,
///     CanonicalKirPrivateMemoryErrorV1, CheckedCanonicalKirPrivateMemoryV18,
///     check_canonical_kir_private_memory_v18};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn physical<'a, 'g>(inventory: &'a CanonicalKirInventoryV18<'g>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> Result<(CheckedCanonicalKirPrivateMemoryV18<'a, 'g>,
///         CanonicalKirPrivateMemoryStorageV1), CanonicalKirPrivateMemoryErrorV1>
/// {
///     check_canonical_kir_private_memory_v18(inventory,
///         CanonicalKirPrivateMemoryLimitsV1 { max_cells: 64 }, budget)
/// }
/// ```
pub fn check_canonical_kir_private_memory_v18<'a, 'g>(
    inventory: &'a CanonicalKirInventoryV18<'g>,
    limits: CanonicalKirPrivateMemoryLimitsV1,
    budget: &mut Budget<'_>,
) -> R<(
    CheckedCanonicalKirPrivateMemoryV18<'a, 'g>,
    CanonicalKirPrivateMemoryStorageV1,
)> {
    scoped(budget, |budget| {
        reserve_headers(budget)?;
        let proof = build(inventory, limits.max_cells, budget)?;
        charge(budget, 4)?;
        let receipt = CanonicalKirPrivateMemoryStorageV1(proof.retained_storage()?);
        Ok((proof, receipt))
    })
}

fn h<T>() -> R<usize> {
    size_of::<T>()
        .checked_add(size_of::<R<T>>())
        .ok_or_else(arithmetic)
}

// One reusable slot bundle per construction, not per operation. These are the
// additional typed adapter's simultaneously live owned/borrowed representations;
// the existing neutral allocator/CFG bundles remain independently accounted.
fn headers() -> R<usize> {
    let terms = [
        h::<&VerifiedCanonicalKernelIrModuleV18>()?,
        h::<&Module>()?,
        h::<&[StorageLayoutV1]>()?,
        h::<&Constant>()?,
        h::<Option<u64>>()?,
        h::<u64>()?,
        h::<&StorageLayoutIdV1>()?,
        h::<StorageLayoutIdV1>()?,
        h::<&Option<ValueId>>()?,
        h::<&u32>()?,
        h::<&AddressSpace>()?,
        h::<Option<&StorageLayoutV1>>()?,
        h::<&StorageLayoutV1>()?,
        h::<ScalarType>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<u32>()?,
        h::<Option<u16>>()?,
        h::<bool>()?,
        h::<Address>()?,
        h::<Result<usize, std::num::TryFromIntError>>()?,
        h::<Option<&CanonicalKirOperationRefV1<'_>>>()?,
        h::<&CanonicalKirOperationRefV1<'_>>()?,
        h::<&CanonicalKirOperationRefV1<'_>>()?,
        h::<&fe2o3_kernel_ir::Operation>()?,
        h::<Option<&CanonicalKirDefinitionRefV1<'_>>>()?,
        h::<&CanonicalKirDefinitionRefV1<'_>>()?,
        h::<&CanonicalKirDefinitionRefV1<'_>>()?,
        h::<&Type>()?,
        h::<&Type>()?,
        h::<Type>()?,
        h::<Type>()?,
        h::<&PointerType>()?,
        h::<&Box<Type>>()?,
        h::<ValueId>()?,
        h::<ValueId>()?,
        h::<Option<ValueId>>()?,
        h::<MemoryAccess>()?,
        h::<Option<(ValueId, MemoryAccess, bool)>>()?,
        h::<Option<(ScalarType, usize)>>()?,
        h::<(ScalarType, usize)>()?,
        h::<Option<&fe2o3_kernel_ir::ValueDef>>()?,
        h::<&fe2o3_kernel_ir::ValueDef>()?,
        h::<()>()?,
        h::<()>()?,
        h::<(
            &CanonicalKirInventoryV18<'_>,
            CanonicalKirPrivateMemoryLimitsV1,
        )>()?,
        h::<(
            &mut Cleanup<'_, '_>,
            (
                &CanonicalKirInventoryV18<'_>,
                CanonicalKirPrivateMemoryLimitsV1,
            ),
        )>()?,
        h::<
            AssertUnwindSafe<(
                &mut Cleanup<'_, '_>,
                (
                    &CanonicalKirInventoryV18<'_>,
                    CanonicalKirPrivateMemoryLimitsV1,
                ),
            )>,
        >()?,
        h::<(
            CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
            CanonicalKirPrivateMemoryStorageV1,
        )>()?,
        restriction_headers()?,
    ];
    terms.into_iter().try_fold(0usize, |sum, term| {
        sum.checked_add(term).ok_or_else(arithmetic)
    })
}
fn reserve_headers(budget: &mut Budget<'_>) -> R<()> {
    charge(budget, 86)?;
    budget.reserve_storage(headers()?)?;
    Ok(())
}

fn restriction_headers() -> R<usize> {
    let terms = [
        h::<&CanonicalKirInventoryV18<'_>>()?,
        h::<&CanonicalKirOperationRefV1<'_>>()?,
        h::<&[Option<Address>]>()?,
        h::<&mut Budget<'_>>()?,
        h::<&OperationKind>()?,
        h::<&ValueId>()?,
        h::<&Type>()?,
        h::<&Type>()?,
        h::<&Type>()?,
        h::<&PointerType>()?,
        h::<&PointerType>()?,
        h::<&PointerType>()?,
        h::<&StorageLayoutIdV1>()?,
        h::<&StorageLayoutIdV1>()?,
        h::<&StorageLayoutIdV1>()?,
        h::<&[CanonicalKirDefinitionRefV1<'_>]>()?,
        h::<Option<&CanonicalKirDefinitionRefV1<'_>>>()?,
        h::<&CanonicalKirDefinitionRefV1<'_>>()?,
        h::<&CanonicalKirDefinitionRefV1<'_>>()?,
        h::<&[CanonicalKirOperationRefV1<'_>]>()?,
        h::<Option<&CanonicalKirOperationRefV1<'_>>>()?,
        h::<&CanonicalKirOperationRefV1<'_>>()?,
        h::<Option<&Option<Address>>>()?,
        h::<&Option<Address>>()?,
        h::<Option<Address>>()?,
        h::<Option<Option<Address>>>()?,
        h::<Address>()?,
        h::<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<(ValueId, bool)>()?,
        h::<AccessMode>()?,
        h::<bool>()?,
        h::<bool>()?,
        h::<()>()?,
        h::<Result<Option<usize>, CanonicalKirInventoryErrorV1>>()?,
    ];
    terms.into_iter().try_fold(0usize, |sum, term| {
        sum.checked_add(term).ok_or_else(arithmetic)
    })
}

pub(super) fn restrict_address<O: PrivateMemoryOwner>(
    inventory: &CanonicalKirInventoryV1<'_, O>,
    row: &CanonicalKirOperationRefV1<'_>,
    addresses: &[Option<Address>],
    budget: &mut Budget<'_>,
) -> R<Address> {
    charge(budget, 36)?;
    let OperationKind::Cast {
        kind: CastKind::RestrictPointerAccess,
        value,
        to,
    } = &row.operation.kind
    else {
        return Err(refused("private", "exact typed pointer restriction"));
    };
    if !O::TYPED || row.results.len() != 1 || row.operands.len() != 1 || !row.effects.is_empty() {
        return Err(refused("private", "exact typed pointer restriction"));
    }
    let definition = index(inventory, row.coordinate.block.function, *value, budget)?;
    let address = addresses
        .get(definition)
        .copied()
        .flatten()
        .ok_or_else(|| refused("private", "restriction has exact allocation lineage"))?;
    let allocation = inventory
        .operations()
        .get(address.allocation)
        .ok_or_else(|| refused("private", "restriction has exact allocation lineage"))?;
    let OperationKind::Alloca {
        element: Type::StorageObject(layout),
        ..
    } = allocation.operation.kind
    else {
        return Err(refused(
            "private",
            "restriction belongs to typed scalar allocation",
        ));
    };
    if allocation.coordinate.block.function != row.coordinate.block.function
        || allocation.results.len() != 1
        || allocation.results.start != definition
        || address.offset != 0
    {
        return Err(refused(
            "private",
            "restriction has direct typed allocation lineage",
        ));
    }
    let source = inventory
        .definitions()
        .get(definition)
        .ok_or_else(|| refused("private", "restriction source definition"))?;
    let result = inventory
        .definitions()
        .get(row.results.start)
        .ok_or_else(|| refused("private", "restriction result definition"))?;
    if source.value != Some(*value)
        || !matches!(source.ty, Type::Pointer(pointer)
            if pointer.address_space == AddressSpace::Private
            && pointer.access == AccessMode::ReadWrite
            && matches!(pointer.pointee.as_ref(), Type::StorageObject(found) if *found == layout))
        || !matches!(to, Type::Pointer(pointer)
            if pointer.address_space == AddressSpace::Private
            && pointer.access == AccessMode::ReadOnly
            && matches!(pointer.pointee.as_ref(), Type::StorageObject(found) if *found == layout))
        || !matches!(result.ty, Type::Pointer(pointer)
            if pointer.address_space == AddressSpace::Private
            && pointer.access == AccessMode::ReadOnly
            && matches!(pointer.pointee.as_ref(), Type::StorageObject(found) if *found == layout))
        || result.coordinate
            != (fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                operation: row.coordinate,
                result: 0,
            })
        || scalar_stride(
            inventory.owner().layouts(),
            layout,
            address.alignment,
            budget,
        )? != address.stride
    {
        return Err(refused(
            "private",
            "restriction preserves exact private scalar layout",
        ));
    }
    Ok(address)
}

pub(super) fn count_literal(value: &Constant) -> Option<u64> {
    match *value {
        Constant::U8(v) => Some(u64::from(v)),
        Constant::U16(v) => Some(u64::from(v)),
        Constant::U32(v) => Some(u64::from(v)),
        Constant::U64(v) | Constant::Index(v) => Some(v),
        Constant::I8(v) if v >= 0 => Some(v as u64),
        Constant::I16(v) if v >= 0 => Some(v as u64),
        Constant::I32(v) if v >= 0 => Some(v as u64),
        Constant::I64(v) if v >= 0 => Some(v as u64),
        _ => None,
    }
}

fn scalar_layout(
    layouts: &[StorageLayoutV1],
    id: StorageLayoutIdV1,
    budget: &mut Budget<'_>,
) -> R<(ScalarType, usize)> {
    charge(budget, 8)?;
    let layout = layouts
        .get(id.0 as usize)
        .ok_or_else(|| refused("private", "exact owner scalar storage layout"))?;
    let StorageLayoutKindV1::Scalar(scalar) = layout.kind else {
        return Err(refused("private", "whole scalar storage object only"));
    };
    let size = usize::try_from(layout.size).map_err(|_| arithmetic())?;
    let exact_size = match scalar.bit_width() {
        Some(bits) => size == usize::from(bits.div_ceil(8)),
        None => scalar == ScalarType::Index && matches!(size, 1 | 2 | 4 | 8 | 16),
    };
    if !exact_size
        || layout.alignment == 0
        || !layout.alignment.is_power_of_two()
        || u64::from(layout.alignment) > layout.size
    {
        return Err(refused("private", "exact owner scalar storage layout"));
    }
    Ok((scalar, size))
}

pub(super) fn scalar_stride(
    layouts: &[StorageLayoutV1],
    id: StorageLayoutIdV1,
    alignment: u32,
    budget: &mut Budget<'_>,
) -> R<usize> {
    let (_, stride) = scalar_layout(layouts, id, budget)?;
    charge(budget, 3)?;
    let layout = layouts
        .get(id.0 as usize)
        .ok_or_else(|| refused("private", "exact owner scalar storage layout"))?;
    if !alignment.is_power_of_two() || alignment < layout.alignment {
        return Err(refused(
            "private",
            "allocation satisfies scalar storage layout",
        ));
    }
    Ok(stride)
}

pub(super) fn check_access<O: PrivateMemoryOwner>(
    inventory: &CanonicalKirInventoryV1<'_, O>,
    row: &CanonicalKirOperationRefV1<'_>,
    definition: usize,
    address: Address,
    budget: &mut Budget<'_>,
) -> R<()> {
    charge(budget, 16)?;
    let allocation = inventory
        .operations()
        .get(address.allocation)
        .ok_or_else(|| refused("private", "exact typed allocation occurrence"))?;
    let OperationKind::Alloca {
        element: Type::StorageObject(layout),
        ..
    } = allocation.operation.kind
    else {
        return Err(refused(
            "private",
            "typed access belongs to typed allocation",
        ));
    };
    // The private definition table is populated only by exact Alloca or the
    // checked restriction above. No projected/equal-bit address is inserted.
    if allocation.results.len() != 1 || address.offset != 0 {
        return Err(refused("private", "whole typed allocation address only"));
    }
    let (scalar, stride) = scalar_layout(inventory.owner().layouts(), layout, budget)?;
    let pointer = inventory
        .definitions()
        .get(definition)
        .ok_or_else(|| refused("private", "exact typed address definition"))?;
    let (operand, writing) = match row.operation.kind {
        OperationKind::Storage(StorageOperationV1::ReadValue { address, .. }) => (address, false),
        OperationKind::Storage(StorageOperationV1::WriteValue { address, .. }) => (address, true),
        _ => return Err(refused("private", "whole scalar storage read or write")),
    };
    if allocation.coordinate.block.function != row.coordinate.block.function
        || pointer.value != Some(operand)
    {
        return Err(refused("private", "exact typed access allocation operand"));
    }
    if !matches!(pointer.ty, Type::Pointer(pointer)
        if pointer.address_space == AddressSpace::Private
        && pointer.access == if definition == allocation.results.start {
            AccessMode::ReadWrite
        } else {
            AccessMode::ReadOnly
        }
        && pointer.pointee.as_ref() == &Type::StorageObject(layout))
        || stride != address.stride
        || (writing && definition != allocation.results.start)
    {
        return Err(refused(
            "private",
            "exact typed allocation pointer and stride",
        ));
    }
    match row.operation.kind {
        OperationKind::Storage(StorageOperationV1::ReadValue { .. }) => {
            charge(budget, 3)?;
            if row.results.len() != 1 || row.operation.results[0].ty != Type::Scalar(scalar) {
                return Err(refused("private", "whole scalar storage read result"));
            }
        }
        OperationKind::Storage(StorageOperationV1::WriteValue { value, .. }) => {
            let value = index(inventory, row.coordinate.block.function, value, budget)?;
            charge(budget, 3)?;
            if !row.results.is_empty() || inventory.definitions()[value].ty != &Type::Scalar(scalar)
            {
                return Err(refused("private", "whole scalar storage write value"));
            }
        }
        _ => return Err(refused("private", "whole scalar storage read or write")),
    }
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_private_memory_typed_v18_tests.rs"]
mod tests;
