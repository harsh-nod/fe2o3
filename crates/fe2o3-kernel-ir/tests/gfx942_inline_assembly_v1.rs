use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AssemblyConstraint, AssemblyEffect, AssemblyOperand, AssemblyOperandKind, AssemblyOption,
    AssemblySourceIdentity, Gfx942InlineAssemblyErrorV1 as Error,
    Gfx942InlineAssemblyInstructionV1 as Instruction, InlineAssembly, InlineAssemblyTarget,
    Operation, OperationKind, ScalarType, Type, ValueDef, ValueId,
    gfx942_inline_assembly_instruction_v1, validate_gfx942_inline_assembly_v1,
};

fn operation(instruction: Instruction, scalar: ScalarType) -> Operation {
    let mut operands = vec![AssemblyOperand::output(0, instruction.constraint())];
    operands.extend(
        (0..instruction.input_count())
            .map(|index| AssemblyOperand::input(ValueId(index as u32), instruction.constraint())),
    );
    Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(scalar)),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: instruction.mnemonic().to_owned(),
            operands,
            options: BTreeSet::from([AssemblyOption::NoMemory]),
            declared_effects: BTreeSet::new(),
        }),
    )
}

fn assembly(operation: &mut Operation) -> &mut InlineAssembly {
    let OperationKind::InlineAssembly(assembly) = &mut operation.kind else {
        unreachable!()
    };
    assembly
}

#[test]
fn catalog_retains_the_existing_seven_lowering_contracts() {
    for instruction in [
        Instruction::VMovB32,
        Instruction::SMovB32,
        Instruction::VAddU32,
        Instruction::VSubU32,
        Instruction::VAndB32,
        Instruction::VOrB32,
        Instruction::VXorB32,
    ] {
        assert_eq!(
            gfx942_inline_assembly_instruction_v1(instruction.mnemonic()),
            Some(instruction)
        );
        for scalar in [ScalarType::I32, ScalarType::U32] {
            let operation = operation(instruction, scalar);
            let validated = validate_gfx942_inline_assembly_v1(&operation, |_| Some(scalar))
                .expect("existing integer contract");
            assert_eq!(validated.instruction(), instruction);
            assert_eq!(validated.result(), ValueId(2));
            assert_eq!(validated.scalar_type(), scalar);
            assert_eq!(validated.inputs().len(), instruction.input_count());
            assert_eq!(validated.inputs()[0], ValueId(0));
            assert_eq!(
                instruction.has_source_macro(),
                instruction != Instruction::SMovB32
            );
        }
    }
    for unknown in [
        "V_ADD_U32",
        "v_add_u32_e64",
        "v_add_u32 clamp",
        "v_add_u32\ns_endpgm",
        "s_add_u32",
        "global_load_dword",
        "s_waitcnt",
        "v_add_co_u32",
        "",
    ] {
        assert_eq!(
            gfx942_inline_assembly_instruction_v1(unknown),
            None,
            "{unknown}"
        );
    }
}

#[test]
fn fixed_results_cover_wrap_borrow_sign_bits_and_boolean_bit_patterns() {
    for (instruction, inputs, expected) in [
        (Instruction::VMovB32, vec![0x89ab_cdef], 0x89ab_cdef),
        (Instruction::SMovB32, vec![0x8000_0000], 0x8000_0000),
        (Instruction::VAddU32, vec![0xffff_ffff, 1], 0),
        (Instruction::VAddU32, vec![0x7fff_ffff, 1], 0x8000_0000),
        (Instruction::VAddU32, vec![0x8000_0000, 0x8000_0000], 0),
        (Instruction::VSubU32, vec![0, 1], 0xffff_ffff),
        (Instruction::VSubU32, vec![0x8000_0000, 1], 0x7fff_ffff),
        (Instruction::VSubU32, vec![17, 9], 8),
        (
            Instruction::VAndB32,
            vec![0xaaaa_ffff, 0x5555_1234],
            0x0000_1234,
        ),
        (
            Instruction::VOrB32,
            vec![0xaaaa_0000, 0x5555_1234],
            0xffff_1234,
        ),
        (
            Instruction::VXorB32,
            vec![0xaaaa_ffff, 0x5555_1234],
            0xffff_edcb,
        ),
    ] {
        assert_eq!(
            instruction.evaluate_bits(&inputs),
            Ok(expected),
            "{instruction:?}"
        );
    }
}

#[test]
fn arithmetic_matches_wider_integer_reference_without_host_overflow() {
    let values = [0, 1, 2, 17, 0x7fff_ffff, 0x8000_0000, 0xffff_fffe, u32::MAX];
    for lhs in values {
        for rhs in values {
            let sum = (u64::from(lhs) + u64::from(rhs)) % (1_u64 << 32);
            let difference = ((1_u64 << 32) + u64::from(lhs) - u64::from(rhs)) % (1_u64 << 32);
            assert_eq!(
                Instruction::VAddU32.evaluate_bits(&[lhs, rhs]),
                Ok(sum as u32)
            );
            assert_eq!(
                Instruction::VSubU32.evaluate_bits(&[lhs, rhs]),
                Ok(difference as u32)
            );
        }
    }
    for instruction in [Instruction::VMovB32, Instruction::VAddU32] {
        for count in [0, 1, 2, 3] {
            if count != instruction.input_count() {
                assert_eq!(
                    instruction.evaluate_bits(&vec![0; count]),
                    Err(Error::InputArity)
                );
            }
        }
    }
}

#[test]
fn validator_rejects_each_source_identity_gap_and_unknown_instructions() {
    for field in 0..4 {
        let mut operation = operation(Instruction::VAddU32, ScalarType::U32);
        let source = &mut assembly(&mut operation).source;
        match field {
            0 => source.frontend_unit = [0; 32],
            1 => source.function = [0; 32],
            2 => source.contract = [0; 32],
            _ => source.statement = [0; 32],
        }
        assert_eq!(
            validate_gfx942_inline_assembly_v1(&operation, |_| Some(ScalarType::U32)),
            Err(Error::IncompleteSourceIdentity)
        );
    }
    let mut operation = operation(Instruction::VAddU32, ScalarType::U32);
    assembly(&mut operation).mnemonic = "s_waitcnt".to_owned();
    assert_eq!(
        validate_gfx942_inline_assembly_v1(&operation, |_| Some(ScalarType::U32)),
        Err(Error::UnsupportedInstruction)
    );
}

#[test]
fn validator_rejects_hidden_effects_even_with_empty_memory_summaries() {
    for effect in [
        AssemblyEffect::ReadGlobal,
        AssemblyEffect::WriteGlobal,
        AssemblyEffect::ReadWorkgroup,
        AssemblyEffect::WriteWorkgroup,
        AssemblyEffect::Atomic,
        AssemblyEffect::Barrier,
        AssemblyEffect::ControlFlow,
    ] {
        let mut operation = operation(Instruction::VAddU32, ScalarType::U32);
        assembly(&mut operation).declared_effects.insert(effect);
        assert_eq!(
            validate_gfx942_inline_assembly_v1(&operation, |_| Some(ScalarType::U32)),
            Err(Error::EffectMismatch),
            "{effect:?}"
        );
    }
    for options in [
        BTreeSet::new(),
        BTreeSet::from([AssemblyOption::ReadOnly]),
        BTreeSet::from([AssemblyOption::NoMemory, AssemblyOption::ReadOnly]),
    ] {
        let mut operation = operation(Instruction::VAddU32, ScalarType::U32);
        assembly(&mut operation).options = options;
        assert_eq!(
            validate_gfx942_inline_assembly_v1(&operation, |_| Some(ScalarType::U32)),
            Err(Error::EffectMismatch)
        );
    }
}

#[test]
fn validator_rejects_roles_constraints_missing_values_and_type_substitution() {
    for (mutation, expected) in [
        (0, Error::ResultArity),
        (1, Error::ResultType),
        (2, Error::OperandCount),
        (3, Error::OutputOperand),
        (4, Error::OutputOperand),
        (5, Error::InputOperand),
        (6, Error::InputOperand),
        (7, Error::InputOperand),
        (8, Error::InputOperand),
    ] {
        let mut operation = operation(Instruction::VAddU32, ScalarType::U32);
        match mutation {
            0 => operation.results.clear(),
            1 => operation.results[0].ty = Type::Scalar(ScalarType::U64),
            2 => {
                assembly(&mut operation).operands.pop();
            }
            3 => {
                assembly(&mut operation).operands[0].kind =
                    AssemblyOperandKind::Output { result_index: 1 }
            }
            4 => assembly(&mut operation).operands[0].constraint = AssemblyConstraint::Sgpr32,
            5 => assembly(&mut operation).operands[1].constraint = AssemblyConstraint::Sgpr32,
            6 => assembly(&mut operation).operands[1].constraint = AssemblyConstraint::ImmediateI32,
            7 => {
                assembly(&mut operation).operands[1].kind = AssemblyOperandKind::InOut {
                    input: ValueId(0),
                    result_index: 0,
                }
            }
            _ => {
                assembly(&mut operation).operands[1] = AssemblyOperand {
                    kind: AssemblyOperandKind::ImmediateI32(1),
                    constraint: AssemblyConstraint::ImmediateI32,
                }
            }
        }
        assert_eq!(
            validate_gfx942_inline_assembly_v1(&operation, |_| Some(ScalarType::U32)),
            Err(expected),
            "mutation {mutation}"
        );
    }
    let operation = operation(Instruction::VAddU32, ScalarType::U32);
    for scalar in [
        None,
        Some(ScalarType::I32),
        Some(ScalarType::U64),
        Some(ScalarType::F32),
    ] {
        assert_eq!(
            validate_gfx942_inline_assembly_v1(&operation, |_| scalar),
            Err(Error::InputType)
        );
    }
}

#[test]
fn validated_options_do_not_change_the_data_result_or_grant_source_authority() {
    let mut operation = operation(Instruction::VAddU32, ScalarType::U32);
    assembly(&mut operation).options.extend([
        AssemblyOption::Pure,
        AssemblyOption::NoStack,
        AssemblyOption::PreservesFlags,
    ]);
    let validated =
        validate_gfx942_inline_assembly_v1(&operation, |_| Some(ScalarType::U32)).unwrap();
    assert_eq!(validated.instruction().evaluate_bits(&[u32::MAX, 1]), Ok(0));
}
