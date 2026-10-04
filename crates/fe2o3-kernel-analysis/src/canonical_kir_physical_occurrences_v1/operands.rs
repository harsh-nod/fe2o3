//! Bounded operand-role queries through existing dense inventory ranges.

use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate,
    CanonicalKirUseCoordinateV1 as UseCoordinate, MatrixOperationKind, MemoryIntrinsicOperation,
    Terminator,
};

use super::*;
use crate::canonical_kir_contract_catalog_v1::find_binding_index;

pub(super) fn operation_index(
    inventory: &Inventory<'_>,
    coordinate: OperationCoordinate,
) -> Result<usize> {
    let block = block_ref(inventory, coordinate.block)?;
    let ordinal = coordinate.operation as usize;
    if ordinal >= block.operations.len() {
        return Err(Error::InconsistentInventory);
    }
    let index = add(block.operations.start, ordinal)?;
    let row = inventory
        .operations()
        .get(index)
        .ok_or(Error::InconsistentInventory)?;
    if row.coordinate != coordinate {
        return Err(Error::InconsistentInventory);
    }
    Ok(index)
}

fn block_ref<'a, 'g>(
    inventory: &'a Inventory<'g>,
    coordinate: BlockCoordinate,
) -> Result<&'a crate::CanonicalKirBlockRefV1<'g>> {
    let function = inventory
        .functions()
        .get(coordinate.function.0 as usize)
        .ok_or(Error::InconsistentInventory)?;
    let ordinal = coordinate.block as usize;
    if ordinal >= function.blocks.len() {
        return Err(Error::InconsistentInventory);
    }
    let block = inventory
        .blocks()
        .get(add(function.blocks.start, ordinal)?)
        .ok_or(Error::InconsistentInventory)?;
    if block.coordinate != coordinate {
        return Err(Error::InconsistentInventory);
    }
    Ok(block)
}

fn only_result(operation: &CanonicalKirOperationRefV1<'_>) -> Result<usize> {
    if operation.results.len() != 1 {
        return Err(Error::InconsistentInventory);
    }
    Ok(operation.results.start)
}

pub(super) fn allocation_index(
    inventory: &Inventory<'_>,
    operation: usize,
) -> Result<Option<usize>> {
    let row = inventory
        .operations()
        .get(operation)
        .ok_or(Error::InconsistentInventory)?;
    if !matches!(row.operation.kind, OperationKind::WorkgroupMemory(_)) {
        return Ok(None);
    }
    Ok(Some(only_result(row)?))
}

fn origin(
    inventory: &Inventory<'_>,
    aliases: &CanonicalKirMustAliasV1<'_, '_>,
    definition: usize,
    budget: &mut Budget<'_>,
) -> Result<Option<usize>> {
    let Some(row) = aliases.allocation_for_definition(definition, budget)? else {
        return Ok(None);
    };
    let Definition::Result {
        operation,
        result: 0,
    } = row.coordinate
    else {
        return Err(Error::InconsistentInventory);
    };
    let allocation = allocation_index(inventory, operation_index(inventory, operation)?)?
        .ok_or(Error::InconsistentInventory)?;
    if !inventory
        .definitions()
        .get(allocation)
        .is_some_and(|actual| std::ptr::eq(actual, row))
    {
        return Err(Error::InconsistentInventory);
    }
    Ok(Some(allocation))
}

pub(super) fn binding_index(
    inventory: &Inventory<'_>,
    catalog: &Catalog,
    allocation: usize,
    budget: &mut Budget<'_>,
) -> Result<Option<usize>> {
    let row = inventory
        .definitions()
        .get(allocation)
        .ok_or(Error::InconsistentInventory)?;
    let Definition::Result {
        operation,
        result: 0,
    } = row.coordinate
    else {
        return Err(Error::InconsistentInventory);
    };
    let value = row.value.ok_or(Error::InconsistentInventory)?;
    Ok(find_binding_index(
        catalog.bindings(),
        operation.block.function.0,
        value.0,
        budget,
    )?)
}

pub(super) fn pointer_address(
    inventory: &Inventory<'_>,
    definition: usize,
    space: AddressSpace,
    aliases: Option<&CanonicalKirMustAliasV1<'_, '_>>,
    budget: &mut Budget<'_>,
) -> Result<CanonicalKirPhysicalAddressV1> {
    use CanonicalKirPhysicalAddressIssueV1 as Issue;
    use CanonicalKirPhysicalAddressV1 as Address;
    if space != AddressSpace::Workgroup {
        return Ok(Address::OutsideWorkgroupModel(space));
    }
    let aliases = aliases.ok_or(Error::InconsistentInventory)?;
    if let Some(allocation) = origin(inventory, aliases, definition, budget)? {
        return Ok(Address::Allocation(allocation));
    }
    let row = inventory
        .definitions()
        .get(definition)
        .ok_or(Error::InconsistentInventory)?;
    let Definition::Result {
        operation,
        result: 0,
    } = row.coordinate
    else {
        return Ok(Address::Unresolved(Issue::UnknownOrigin));
    };
    let operation = operation_index(inventory, operation)?;
    let op = &inventory.operations()[operation];
    if !matches!(op.operation.kind, OperationKind::GetElementPointer { .. }) {
        return Ok(Address::Unresolved(Issue::UnsupportedProducer));
    }
    if op.operands.len() != 2 || only_result(op)? != definition {
        return Err(Error::InconsistentInventory);
    }
    let base = inventory
        .uses()
        .get(op.operands.start)
        .ok_or(Error::InconsistentInventory)?;
    match origin(inventory, aliases, base.definition, budget)? {
        Some(allocation) => Ok(Address::ElementOffset {
            allocation,
            operation,
            offset_use: op.operands.start + 1,
        }),
        None => Ok(Address::Unresolved(Issue::UnsupportedOffsetBase)),
    }
}

fn forwarding(
    inventory: &Inventory<'_>,
    address: CanonicalKirPhysicalAddressV1,
    target_definition: usize,
    edge_argument: Option<usize>,
    aliases: Option<&CanonicalKirMustAliasV1<'_, '_>>,
    budget: &mut Budget<'_>,
) -> Result<CanonicalKirPhysicalPointerRoleV1> {
    let same_allocation = match address {
        CanonicalKirPhysicalAddressV1::Allocation(allocation) => {
            origin(
                inventory,
                aliases.ok_or(Error::InconsistentInventory)?,
                target_definition,
                budget,
            )? == Some(allocation)
        }
        _ => false,
    };
    Ok(CanonicalKirPhysicalPointerRoleV1::Forwarding {
        target_definition,
        edge_argument,
        same_allocation,
    })
}

pub(super) fn pointer_role(
    inventory: &Inventory<'_>,
    used: &CanonicalKirUseRefV1,
    address: CanonicalKirPhysicalAddressV1,
    aliases: Option<&CanonicalKirMustAliasV1<'_, '_>>,
    budget: &mut Budget<'_>,
) -> Result<CanonicalKirPhysicalPointerRoleV1> {
    use CanonicalKirPhysicalEscapeV1 as Escape;
    use CanonicalKirPhysicalPointerRoleV1 as Role;
    match used.coordinate {
        UseCoordinate::OperationOperand { operation, operand } => {
            let index = operation_index(inventory, operation)?;
            let op = &inventory.operations()[index];
            let operand = operand as usize;
            if operand >= op.operands.len()
                || !inventory
                    .uses()
                    .get(add(op.operands.start, operand)?)
                    .is_some_and(|row| std::ptr::eq(row, used))
            {
                return Err(Error::InconsistentInventory);
            }
            if address_slot(&op.operation.kind, operand).is_some() {
                return Ok(Role::AccessAddress);
            }
            match &op.operation.kind {
                OperationKind::VerificationContract(_) if operand == 0 => Ok(Role::ContractStorage),
                OperationKind::GetElementPointer { .. } if operand == 0 => Ok(Role::ElementBase {
                    result: only_result(op)?,
                }),
                OperationKind::Select { .. } if operand == 1 || operand == 2 => {
                    forwarding(inventory, address, only_result(op)?, None, aliases, budget)
                }
                OperationKind::Call { .. } => Ok(Role::Escape(Escape::Call)),
                OperationKind::InlineAssembly(_) => Ok(Role::Escape(Escape::OpaqueAssembly)),
                OperationKind::Store { .. }
                | OperationKind::GuardedStore { .. }
                | OperationKind::VectorStore(_)
                | OperationKind::Atomic(_)
                | OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                    ..
                }) => Ok(Role::Escape(Escape::StoredValue)),
                _ => Ok(Role::Escape(Escape::UnsupportedUse)),
            }
        }
        UseCoordinate::TerminatorOperand { block, operand } => {
            let row = block_ref(inventory, block)?;
            let operand = operand as usize;
            if operand >= row.terminator_uses.len()
                || !inventory
                    .uses()
                    .get(add(row.terminator_uses.start, operand)?)
                    .is_some_and(|actual| std::ptr::eq(actual, used))
            {
                return Err(Error::InconsistentInventory);
            }
            let prefix = match row.terminator {
                Terminator::Return { .. } => return Ok(Role::Escape(Escape::Return)),
                Terminator::Branch { .. } => 0,
                Terminator::ConditionalBranch { .. }
                | Terminator::Switch { .. }
                | Terminator::IntegerSwitch { .. } => 1,
                Terminator::Unreachable => return Err(Error::InconsistentInventory),
            };
            if operand < prefix {
                return Ok(Role::Escape(Escape::UnsupportedUse));
            }
            // Exact original operand order is selector, then each edge's arguments.
            // The existing dense binding ranges preserve repeated edge occurrences.
            let first = inventory
                .edges()
                .get(row.edges.start)
                .filter(|_| !row.edges.is_empty())
                .ok_or(Error::InconsistentInventory)?;
            let index = add(first.bindings.start, operand - prefix)?;
            let argument = inventory
                .edge_arguments()
                .get(index)
                .ok_or(Error::InconsistentInventory)?;
            if argument.coordinate.edge.source != block
                || argument.value != used.value
                || argument.incoming_definition != used.definition
            {
                return Err(Error::InconsistentInventory);
            }
            forwarding(
                inventory,
                address,
                argument.target_definition,
                Some(index),
                aliases,
                budget,
            )
        }
    }
}

/// At most two fixed address slots, in the existing local-effect visitor order.
pub(super) fn address_slot(kind: &OperationKind, operand: usize) -> Option<usize> {
    match kind {
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping { .. }) => {
            (operand < 2).then_some(operand)
        }
        OperationKind::Load { .. }
        | OperationKind::Store { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::VectorLoad(_)
        | OperationKind::VectorStore(_)
        | OperationKind::Atomic(_)
        | OperationKind::MemoryIntrinsic(
            MemoryIntrinsicOperation::VolatileLoad { .. }
            | MemoryIntrinsicOperation::VolatileStore { .. },
        ) => (operand == 0).then_some(0),
        OperationKind::Matrix(matrix)
            if matches!(
                matrix.kind,
                MatrixOperationKind::LdsLoad { .. } | MatrixOperationKind::LdsStore { .. }
            ) =>
        {
            (operand == 0).then_some(0)
        }
        _ => None,
    }
}

pub(super) fn footprint(kind: &OperationKind) -> CanonicalKirPhysicalFootprintV1 {
    use CanonicalKirPhysicalFootprintV1 as Footprint;
    match kind {
        OperationKind::Load { .. }
        | OperationKind::Store { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::MemoryIntrinsic(
            MemoryIntrinsicOperation::VolatileLoad { .. }
            | MemoryIntrinsicOperation::VolatileStore { .. },
        ) => Footprint::TypedAccess,
        OperationKind::VectorLoad(_) | OperationKind::VectorStore(_) => Footprint::VectorAccess,
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping { .. }) => {
            Footprint::RepeatedElements
        }
        OperationKind::Atomic(_) => Footprint::AtomicAccess,
        _ => Footprint::Unmodeled,
    }
}
