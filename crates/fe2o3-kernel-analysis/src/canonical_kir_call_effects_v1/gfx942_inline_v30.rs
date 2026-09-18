//! Effect completeness for the closed V30 instruction profile only.
//! The original instruction and all four source identities stay in the owner.
//! This establishes neither source authentication nor scalar/refinement facts.

use super::{Budget, Error, Function, Inventory, Result};
use fe2o3_kernel_ir::{
    AssemblyOperandKind, AssemblyOption, Gfx942InlineAssemblyInstructionV1 as Instruction,
    Operation, OperationKind, ScalarType, validate_gfx942_inline_assembly_v1,
};

pub(super) fn has_closed_effects(
    inventory: &Inventory<'_>,
    function: Function,
    operation: &Operation,
    budget: &mut Budget<'_>,
) -> Result<bool> {
    budget.charge_work(12)?;
    let OperationKind::InlineAssembly(assembly) = &operation.kind else {
        return Ok(false);
    };
    if assembly.options.len() != 1
        || !assembly.options.contains(&AssemblyOption::NoMemory)
        || !assembly.declared_effects.is_empty()
        || !(2..=3).contains(&assembly.operands.len())
        || operation.results.len() != 1
        || assembly.mnemonic.len() > 9
    {
        return Ok(false);
    }
    // Prepay the allocation-free bounded validator: four 32-byte identities,
    // at most nine mnemonic bytes, one result, three operands and one option.
    // Actual inventory lookups below charge their own complete binary searches.
    budget.charge_work(512)?;
    let mut inputs = [None; 2];
    for (slot, operand) in inputs.iter_mut().zip(&assembly.operands[1..]) {
        let AssemblyOperandKind::Input(value) = operand.kind else {
            return Ok(false);
        };
        let definition = inventory
            .definition_for_value(function, value, budget)
            .map_err(|error| match error {
                crate::CanonicalKirInventoryErrorV1::Resource(error) => Error::Resource(error),
                crate::CanonicalKirInventoryErrorV1::InconsistentOwner => {
                    Error::InconsistentInventory
                }
            })?;
        let Some(scalar) = definition.and_then(|definition| definition.ty.as_scalar()) else {
            return Ok(false);
        };
        *slot = Some((value, scalar));
    }
    let Ok(checked) = validate_gfx942_inline_assembly_v1(operation, |value| {
        inputs
            .iter()
            .flatten()
            .find_map(|(id, ty)| (*id == value).then_some(*ty))
    }) else {
        return Ok(false);
    };
    Ok(checked.scalar_type() == ScalarType::U32
        && matches!(
            checked.instruction(),
            Instruction::VMovB32
                | Instruction::VAddU32
                | Instruction::VSubU32
                | Instruction::VAndB32
                | Instruction::VOrB32
                | Instruction::VXorB32
        ))
}
