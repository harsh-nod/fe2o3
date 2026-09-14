//! Metered canonical coordinate lookup. Raw SSA IDs never size these indexes.
use super::{Budget, Error, Inventory, Result};
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Operation,
    CanonicalKirTransitionRangeV1 as Rows, CanonicalKirUseCoordinateV1 as Use,
};
use std::ops::Range;

fn at(range: &Range<usize>, ordinal: u32, bound: usize) -> Result<usize> {
    let index = range
        .start
        .checked_add(usize::try_from(ordinal).map_err(|_| Error::Arithmetic)?)
        .ok_or(Error::Arithmetic)?;
    if range.start > range.end || range.end > bound || index >= range.end {
        return Err(Error::InvalidCoordinate);
    }
    Ok(index)
}

pub(super) fn range(rows: Rows, bound: usize, budget: &mut Budget<'_>) -> Result<Range<usize>> {
    budget.charge_work(1)?;
    let start = usize::try_from(rows.start).map_err(|_| Error::Arithmetic)?;
    let end = start
        .checked_add(usize::try_from(rows.len).map_err(|_| Error::Arithmetic)?)
        .ok_or(Error::Arithmetic)?;
    if end > bound {
        return Err(Error::InvalidCoordinate);
    }
    Ok(start..end)
}

pub(super) fn function(
    inventory: &Inventory<'_>,
    coordinate: Function,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(1)?;
    let index = usize::try_from(coordinate.0).map_err(|_| Error::Arithmetic)?;
    inventory
        .functions()
        .get(index)
        .filter(|row| row.coordinate == coordinate)
        .ok_or(Error::InvalidCoordinate)?;
    Ok(index)
}

pub(super) fn block(
    inventory: &Inventory<'_>,
    coordinate: Block,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(1)?;
    let function = function(inventory, coordinate.function, budget)?;
    let index = at(
        &inventory.functions()[function].blocks,
        coordinate.block,
        inventory.blocks().len(),
    )?;
    if inventory.blocks()[index].coordinate != coordinate {
        return Err(Error::InvalidCoordinate);
    }
    Ok(index)
}

pub(super) fn operation(
    inventory: &Inventory<'_>,
    coordinate: Operation,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(1)?;
    let block = block(inventory, coordinate.block, budget)?;
    let index = at(
        &inventory.blocks()[block].operations,
        coordinate.operation,
        inventory.operations().len(),
    )?;
    if inventory.operations()[index].coordinate != coordinate {
        return Err(Error::InvalidCoordinate);
    }
    Ok(index)
}

pub(super) fn definition(
    inventory: &Inventory<'_>,
    coordinate: Definition,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(1)?;
    let index = match coordinate {
        Definition::FunctionArgument {
            function: coordinate,
            argument,
        } => {
            let function = function(inventory, coordinate, budget)?;
            let row = &inventory.functions()[function];
            if usize::try_from(argument).map_err(|_| Error::Arithmetic)?
                >= row.function.signature.parameters.len()
            {
                return Err(Error::InvalidCoordinate);
            }
            at(&row.definitions, argument, inventory.definitions().len())?
        }
        Definition::BlockArgument {
            block: coordinate,
            argument,
        } => {
            let block = block(inventory, coordinate, budget)?;
            at(
                &inventory.blocks()[block].parameters,
                argument,
                inventory.definitions().len(),
            )?
        }
        Definition::Result {
            operation: coordinate,
            result,
        } => {
            let operation = operation(inventory, coordinate, budget)?;
            at(
                &inventory.operations()[operation].results,
                result,
                inventory.definitions().len(),
            )?
        }
    };
    if inventory.definitions()[index].coordinate != coordinate {
        return Err(Error::InvalidCoordinate);
    }
    Ok(index)
}

pub(super) fn used(
    inventory: &Inventory<'_>,
    coordinate: Use,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(1)?;
    let index = match coordinate {
        Use::OperationOperand {
            operation: coordinate,
            operand,
        } => {
            let operation = operation(inventory, coordinate, budget)?;
            at(
                &inventory.operations()[operation].operands,
                operand,
                inventory.uses().len(),
            )?
        }
        Use::TerminatorOperand {
            block: coordinate,
            operand,
        } => {
            let block = block(inventory, coordinate, budget)?;
            at(
                &inventory.blocks()[block].terminator_uses,
                operand,
                inventory.uses().len(),
            )?
        }
    };
    if inventory.uses()[index].coordinate != coordinate {
        return Err(Error::InvalidCoordinate);
    }
    Ok(index)
}

pub(super) fn edge(
    inventory: &Inventory<'_>,
    coordinate: Edge,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(1)?;
    let block = block(inventory, coordinate.source, budget)?;
    let index = at(
        &inventory.blocks()[block].edges,
        coordinate.successor,
        inventory.edges().len(),
    )?;
    if inventory.edges()[index].coordinate != coordinate {
        return Err(Error::InvalidCoordinate);
    }
    Ok(index)
}

pub(super) fn edge_argument(
    inventory: &Inventory<'_>,
    coordinate: EdgeArgument,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(1)?;
    let edge = edge(inventory, coordinate.edge, budget)?;
    let index = at(
        &inventory.edges()[edge].bindings,
        coordinate.argument,
        inventory.edge_arguments().len(),
    )?;
    if inventory.edge_arguments()[index].coordinate != coordinate {
        return Err(Error::InvalidCoordinate);
    }
    Ok(index)
}

pub(super) fn definition_function(coordinate: Definition) -> Function {
    match coordinate {
        Definition::FunctionArgument { function, .. } => function,
        Definition::BlockArgument { block, .. } => block.function,
        Definition::Result { operation, .. } => operation.block.function,
    }
}
