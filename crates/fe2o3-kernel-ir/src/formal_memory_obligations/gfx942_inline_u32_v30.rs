//! Absence of memory effects for the closed V30 vector-u32 assembly profile.
//!
//! Used only after mandatory module verification, with the containing function's
//! complete type table. Success discharges only memory-effect uncertainty, not
//! value equivalence, pointer bounds, source authentication, target admission,
//! or hardware facts. No additional type-table allocation or scan is needed.

use std::collections::BTreeMap;

use crate::{
    AssemblyOption, Gfx942InlineAssemblyInstructionV1 as Instruction, Operation, OperationKind,
    ScalarType, Type, ValueId, validate_gfx942_inline_assembly_v1,
};

pub(super) fn has_closed_memory_effects(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, Type>,
) -> bool {
    let OperationKind::InlineAssembly(assembly) = &operation.kind else {
        return false;
    };
    if assembly.options.len() != 1
        || !assembly.options.contains(&AssemblyOption::NoMemory)
        || !assembly.declared_effects.is_empty()
    {
        return false;
    }
    // Nonzero source IDs are structural requirements, not evidence that a
    // caller-authored module came from the Rust frontend.
    validate_gfx942_inline_assembly_v1(operation, |value| {
        value_types.get(&value).and_then(Type::as_scalar)
    })
    .is_ok_and(|validated| {
        validated.scalar_type() == ScalarType::U32
            && matches!(
                validated.instruction(),
                Instruction::VMovB32
                    | Instruction::VAddU32
                    | Instruction::VSubU32
                    | Instruction::VAndB32
                    | Instruction::VOrB32
                    | Instruction::VXorB32
            )
    })
}
