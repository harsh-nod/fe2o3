//! The existing unroll selection must refuse authored V19 intent.
use super::allowed;
use fe2o3_kernel_ir::{
    Gfx942CompleteBodyDeclarationVNext as Declaration, Gfx942CompleteBodyOriginVNext as Origin,
    Gfx942CompleteBodyStepVNext as Step, Gfx942OrderedProgramRegistersV1 as Registers,
    Gfx942ProgramDestinationV1 as Destination, Gfx942ProgramInstructionV1 as Instruction,
    Gfx942ProgramRoleV1 as Role, OperationKind, ValueId,
};

#[test]
fn authored_declaration_is_not_a_duplicable_unroll_operation() {
    let declaration = Declaration {
        origin: Origin {
            root_axes: [[1; 32]; 5],
            mir_body: [2; 32],
            semantic_block: [3; 32],
            source_signature: [4; 32],
            rustc_fn_abi: [5; 32],
            frontend_bytes_sha256: [6; 32],
            raw_block: 0,
        },
        registers: Registers::new(32, 33, [34, 35, 36]).unwrap(),
        parameters: [ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        labels: [1, 0, 0, 0, 0, 0, 0, 0],
        block_count: 1,
        instruction_count: 1,
    };
    assert!(!allowed(&OperationKind::Gfx942CompleteBodyDeclaration(
        declaration
    )));
}
#[test]
fn authored_step_is_not_a_duplicable_unroll_operation() {
    let step = Step {
        authored_block: 0,
        authored_instruction: 0,
        instruction: Instruction::Move {
            destination: Destination::Output,
            source: Role::Input0,
        },
        operands: [Some(ValueId(1)), None],
    };
    assert!(!allowed(&OperationKind::Gfx942CompleteBodyStep(step)));
}
