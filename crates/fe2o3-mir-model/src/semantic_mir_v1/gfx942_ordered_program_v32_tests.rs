//! Synthetic inert model controls, never authenticated source evidence.
use super::*;

const CASES: [[u32; 3]; 6] = [
    [0, 0, 0],
    [u32::MAX, 0, 1],
    [u32::MAX, 1, 2],
    [0x8000_0000, 0, 0x8000_0000],
    [0xaaaa_5555, 0x5555_aaaa, 19],
    [19, 23, 42],
];

fn words(active: &[u16]) -> [u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32] {
    assert!(active.len() <= SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32);
    let mut result = [0; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32];
    result[..active.len()].copy_from_slice(active);
    result
}

// Independent arithmetic descriptor checks: do not call production decoders,
// role/arity helpers, packing helpers or initialization validation.
fn metadata(word: u16) -> Option<(u8, usize, usize, Option<usize>)> {
    if word / 1024 != 0 {
        return None;
    }
    let opcode = (word % 8) as u8;
    let destination = 3 + usize::from((word / 8) % 2);
    let left = usize::from((word / 16) % 8);
    let right = usize::from((word / 128) % 8);
    if opcode > 5 || left > 4 {
        return None;
    }
    if opcode == 0 {
        (right == 0).then_some((opcode, destination, left, None))
    } else if right <= 4 {
        Some((opcode, destination, left, Some(right)))
    } else {
        None
    }
}

fn reference_valid(active: &[u16]) -> bool {
    if active.is_empty() || active.len() > 16 {
        return false;
    }
    let mut defined = [true, true, true, false, false];
    for &word in active {
        let Some((_, destination, left, right)) = metadata(word) else {
            return false;
        };
        if !defined[left] || right.is_some_and(|right| !defined[right]) {
            return false;
        }
        defined[destination] = true;
    }
    defined[4]
}

fn reference_evaluate(active: &[u16], inputs: [u32; 3]) -> u32 {
    assert!(reference_valid(active));
    let mut values = [inputs[0], inputs[1], inputs[2], 0, 0];
    for &word in active {
        let (opcode, destination, left, right) = metadata(word).unwrap();
        let before = values;
        let lhs = u64::from(before[left]);
        let rhs = right.map_or(0, |right| u64::from(before[right]));
        values[destination] = match opcode {
            0 => lhs,
            1 => (lhs + rhs) % (1_u64 << 32),
            2 => ((1_u64 << 32) + lhs - rhs) % (1_u64 << 32),
            3 => lhs & rhs,
            4 => lhs | rhs,
            5 => lhs ^ rhs,
            _ => unreachable!(),
        } as u32;
    }
    values[4]
}

#[test]
fn exact_three_step_bitselect_packing_and_independent_oracle() {
    let active = [0x0085, 0x0133, 0x019d];
    let program = SemanticGfx942U32ProgramV32::from_descriptors(3, words(&active)).unwrap();
    assert_eq!(program.packed_words(), [0x0000_019d_0133_0085, 0, 0, 0]);
    assert_eq!(program.count(), 3);
    assert_eq!(program.instructions().len(), 3);
    let typed: Vec<_> = program.instructions().collect();
    assert_eq!(
        SemanticGfx942U32ProgramV32::from_instructions(&typed).unwrap(),
        program
    );
    for [a, b, mask] in CASES {
        assert_eq!(program.evaluate([a, b, mask]), b ^ ((a ^ b) & mask));
        assert_eq!(
            program.evaluate([a, b, mask]),
            reference_evaluate(&active, [a, b, mask])
        );
    }
}

#[test]
fn exact_sixteen_step_packing_retains_dead_self_and_repeated_steps() {
    let descriptors = [
        0x0000, 0x0018, 0x0131, 0x004a, 0x0233, 0x01ac, 0x0045, 0x0038, 0x0030, 0x0048, 0x01b9,
        0x0242, 0x000b, 0x0094, 0x012d, 0x0028,
    ];
    let packed = [
        0x004a_0131_0018_0000,
        0x0038_0045_01ac_0233,
        0x0242_01b9_0048_0030,
        0x0028_012d_0094_000b,
    ];
    let program = SemanticGfx942U32ProgramV32::from_packed(16, packed).unwrap();
    assert_eq!(program.descriptors(), &descriptors);
    assert_eq!(program.packed_words(), packed);
    assert_eq!(
        program
            .instructions()
            .map(SemanticGfx942ProgramInstructionV32::descriptor)
            .collect::<Vec<_>>(),
        descriptors
    );
    for inputs in CASES {
        assert_eq!(program.evaluate(inputs), inputs[2]);
        assert_eq!(
            program.evaluate(inputs),
            reference_evaluate(&descriptors, inputs)
        );
    }
    let repeated = SemanticGfx942U32ProgramV32::from_descriptors(16, [0x0008; 16]).unwrap();
    let single = SemanticGfx942U32ProgramV32::from_descriptors(1, words(&[0x0008])).unwrap();
    assert_ne!(repeated, single);
    assert_ne!(repeated.packed_words(), single.packed_words());
    assert_eq!(repeated.active_descriptors(), &[0x0008; 16]);
    for inputs in CASES {
        assert_eq!(repeated.evaluate(inputs), single.evaluate(inputs));
    }
}

#[test]
fn all_single_u16_descriptors_have_exact_structural_and_program_acceptance() {
    let mut structurally_valid = 0;
    let mut accepted = 0;
    for word in 0..=u16::MAX {
        let decoded = SemanticGfx942ProgramInstructionV32::from_descriptor(word);
        assert_eq!(decoded.is_ok(), metadata(word).is_some(), "word={word:04x}");
        if let Ok(instruction) = decoded {
            structurally_valid += 1;
            assert_eq!(instruction.descriptor(), word);
        }
        let program = SemanticGfx942U32ProgramV32::from_descriptors(1, words(&[word]));
        assert_eq!(program.is_ok(), reference_valid(&[word]), "word={word:04x}");
        if let Ok(program) = program {
            accepted += 1;
            for inputs in CASES {
                assert_eq!(
                    program.evaluate(inputs),
                    reference_evaluate(&[word], inputs)
                );
            }
        }
    }
    assert_eq!(structurally_valid, 260);
    assert_eq!(accepted, 48);
}

#[test]
fn exhaustive_two_step_metadata_pairs_use_pre_step_definitions() {
    let legal: Vec<_> = (0..1024_u16)
        .filter(|&word| metadata(word).is_some())
        .collect();
    assert_eq!(legal.len(), 260);
    let mut examined = 0;
    let mut accepted = 0;
    for &first in &legal {
        for &second in &legal {
            examined += 1;
            let active = [first, second];
            let program = SemanticGfx942U32ProgramV32::from_descriptors(2, words(&active));
            assert_eq!(
                program.is_ok(),
                reference_valid(&active),
                "words={active:04x?}"
            );
            if let Ok(program) = program {
                accepted += 1;
                for inputs in CASES {
                    assert_eq!(
                        program.evaluate(inputs),
                        reference_evaluate(&active, inputs)
                    );
                }
            }
        }
    }
    assert_eq!(examined, 67_600);
    // First:48 choices per destination. With four defined sources there
    // are4 moves +5*4*4 binary forms =84 choices per second destination.
    // A scratch-first pair must finish at output; output-first permits both.
    assert_eq!(accepted, 48 * 84 + 48 * (2 * 84));
}

#[test]
fn every_u16_metadata_mutant_is_checked_in_both_two_step_positions() {
    // The fixed other step respectively defines output or scratch. All
    // 131,072 candidates include reserved/opcode/role/unary corruptions.
    for mutant in 0..=u16::MAX {
        for active in [[mutant, 0x0008], [0x0010, mutant]] {
            let program = SemanticGfx942U32ProgramV32::from_descriptors(2, words(&active));
            assert_eq!(
                program.is_ok(),
                reference_valid(&active),
                "words={active:04x?}"
            );
            if let Ok(program) = program {
                assert_eq!(
                    program.evaluate([19, 23, 42]),
                    reference_evaluate(&active, [19, 23, 42])
                );
            }
        }
    }
}

#[test]
fn count_and_every_inactive_slot_bit_are_canonical() {
    for count in 0..=u8::MAX {
        let mut descriptors = [0; 16];
        for slot in descriptors.iter_mut().take(usize::from(count).min(16)) {
            *slot = 0x0008;
        }
        assert_eq!(
            SemanticGfx942U32ProgramV32::from_descriptors(count, descriptors).is_ok(),
            (1..=16).contains(&count)
        );
    }
    for count in 1..16_u8 {
        let mut baseline = [0; 16];
        baseline[..usize::from(count)].fill(0x0008);
        for step in usize::from(count)..16 {
            for bit in 0..16 {
                let mut mutated = baseline;
                mutated[step] = 1 << bit;
                assert_eq!(
                    SemanticGfx942U32ProgramV32::from_descriptors(count, mutated),
                    Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
                );
            }
        }
    }
    let move_out = SemanticGfx942ProgramInstructionV32::Move {
        destination: SemanticGfx942ProgramDestinationV32::Output,
        source: SemanticGfx942ProgramRoleV32::Input0,
    };
    assert_eq!(
        SemanticGfx942U32ProgramV32::from_instructions(&[]),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
    assert_eq!(
        SemanticGfx942U32ProgramV32::from_instructions(&[move_out; 17]),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
}

#[test]
fn packing_is_const_and_all_sixty_four_bit_positions_round_trip() {
    const WORDS: [u16; 16] = [0x0008; 16];
    const PACKED: [u64; 4] = pack_program_descriptors_v32(WORDS);
    const UNPACKED: [u16; 16] = unpack_program_descriptors_v32(PACKED);
    assert_eq!(UNPACKED, WORDS);
    for pack in 0..4 {
        for bit in 0..64 {
            let mut input = [0; 4];
            input[pack] = 1_u64 << bit;
            let unpacked = unpack_program_descriptors_v32(input);
            let mut expected = [0; 16];
            expected[4 * pack + bit / 16] = 1_u16 << (bit % 16);
            assert_eq!(unpacked, expected);
            assert_eq!(pack_program_descriptors_v32(unpacked), input);
        }
    }
    let program = SemanticGfx942U32ProgramV32::from_packed(16, PACKED).unwrap();
    assert_eq!(program.packed_words(), PACKED);
}

#[test]
fn precise_error_classes_reject_reserved_unary_reads_and_missing_output() {
    for bit in 10..16 {
        assert_eq!(
            SemanticGfx942ProgramInstructionV32::from_descriptor(0x0008 | (1 << bit)),
            Err(SemanticGfx942ProgramDescriptorErrorV32::ReservedBits { bits: 1 << bit })
        );
    }
    for tag in [6, 7] {
        assert_eq!(
            SemanticGfx942ProgramInstructionV32::from_descriptor(8 | tag),
            Err(SemanticGfx942ProgramDescriptorErrorV32::Opcode { tag: tag as u8 })
        );
    }
    assert_eq!(
        SemanticGfx942ProgramInstructionV32::from_descriptor(0x0058),
        Err(SemanticGfx942ProgramDescriptorErrorV32::Source { operand: 0, tag: 5 })
    );
    assert_eq!(
        SemanticGfx942ProgramInstructionV32::from_descriptor(0x0389),
        Err(SemanticGfx942ProgramDescriptorErrorV32::Source { operand: 1, tag: 7 })
    );
    for tag in 1..=7_u8 {
        assert_eq!(
            SemanticGfx942ProgramInstructionV32::from_descriptor(8 | (u16::from(tag) << 7)),
            Err(SemanticGfx942ProgramDescriptorErrorV32::MoveUnusedSource { tag })
        );
    }
    assert_eq!(
        SemanticGfx942U32ProgramV32::from_descriptors(1, words(&[0x0039])),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
    assert_eq!(
        SemanticGfx942U32ProgramV32::from_descriptors(1, words(&[0x0048])),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
    assert_eq!(
        SemanticGfx942U32ProgramV32::from_descriptors(1, words(&[0x0000])),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
    assert_eq!(
        SemanticGfx942U32ProgramV32::from_descriptors(1, words(&[0x0389])),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
}

#[test]
fn output_first_scratch_optional_repeated_inputs_and_wrapping_are_explicit() {
    let xor_add =
        SemanticGfx942U32ProgramV32::from_descriptors(2, words(&[0x008d, 0x0149])).unwrap();
    let twice_output =
        SemanticGfx942U32ProgramV32::from_descriptors(2, words(&[0x0008, 0x0249])).unwrap();
    let subtract = SemanticGfx942U32ProgramV32::from_descriptors(1, words(&[0x008a])).unwrap();
    for [a, b, c] in CASES {
        assert_eq!(xor_add.evaluate([a, b, c]), (a ^ b).wrapping_add(c));
        assert_eq!(twice_output.evaluate([a, b, c]), a.wrapping_add(a));
        assert_eq!(subtract.evaluate([a, b, c]), a.wrapping_sub(b));
    }
    assert_eq!(subtract.evaluate([0, 1, 0]), u32::MAX);
    assert_eq!(twice_output.evaluate([0x8000_0000, 0, 0]), 0);
    let alternate =
        SemanticGfx942U32ProgramV32::from_descriptors(2, words(&[0x0010, 0x0008])).unwrap();
    let direct = SemanticGfx942U32ProgramV32::from_descriptors(1, words(&[0x0008])).unwrap();
    assert_ne!(alternate, direct); // A dead scratch write must not disappear.
    assert_eq!(
        alternate.evaluate([19, 23, 42]),
        direct.evaluate([19, 23, 42])
    );
}

fn identity(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn minimal_request() -> InertSemanticMirRequestV1 {
    let ty_id = SemanticTypeIdV1::from_index(0);
    let layout = SemanticTypeLayoutV1::new_with_backend_repr(
        Some(4),
        4,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
        )),
        false,
    )
    .unwrap();
    let ty = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(1)),
        SemanticLayoutIdentityV1(identity(2)),
        layout,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    let mode = SemanticAbiPassModeV1::Direct(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap(),
    );
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(3)),
        SemanticLayoutIdentityV1(identity(4)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![SemanticAbiValueV1::new(ty_id, mode.clone())],
        SemanticAbiValueV1::new(ty_id, mode),
    )
    .unwrap();
    let locals = vec![
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(identity(5)),
            ty_id,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(identity(6)),
            ty_id,
            SemanticLocalRoleV1::Argument(0),
            SemanticSourceProvenanceV1::unavailable(),
        ),
    ];
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1(identity(7)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1(identity(8)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1(identity(9)),
        SemanticMonomorphizationIdentityV1(identity(10)),
        SemanticGenericTypeArgumentsIdentityV1(identity(11)),
        SemanticConstGenericArgumentsIdentityV1(identity(12)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1(identity(13))),
        vec![ty],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

const REGISTERS: [u8; 5] = [32, 33, 34, 35, 36];

fn source(function: SemanticFunctionIdentityV1) -> SemanticOrderedProgramSourceV32 {
    SemanticOrderedProgramSourceV32::new([0xb1; 32], function, [0xb3; 32], [0xb4; 32]).unwrap()
}

fn old_source(function: SemanticFunctionIdentityV1) -> SemanticInlineAssemblySourceV30 {
    SemanticInlineAssemblySourceV30::new([0xa1; 32], function, [0xa3; 32], [0xa4; 32]).unwrap()
}

fn literal(value: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(u128::from(value), 1).unwrap()),
    ))
}

fn request(registers: [u8; 5]) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let mut types = request.types.into_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(40)),
        SemanticLayoutIdentityV1(identity(41)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, u128::from(u8::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 8,
        }),
    ));
    request.types = types.into_boxed_slice();
    let abi = &request.functions[0].abi;
    let mut arguments = vec![abi.arguments()[0].value().clone(); 3];
    arguments.extend(vec![
        SemanticAbiValueV1::new(
            SemanticTypeIdV1(1),
            abi.return_value().mode().clone()
        );
        5
    ]);
    let intrinsic_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(20)),
        SemanticLayoutIdentityV1(identity(21)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        arguments,
        abi.return_value().clone(),
    )
    .unwrap();
    let mut callables = request.callables.into_vec();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1(identity(22)),
            SemanticItemDefinitionIdentityV1(identity(23)),
            SemanticMonomorphizationIdentityV1(identity(24)),
            SemanticGenericTypeArgumentsIdentityV1(identity(25)),
            SemanticConstGenericArgumentsIdentityV1(identity(26)),
            SemanticSourceProvenanceV1::unavailable(),
            intrinsic_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(program()),
        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(0xa7)),
    });
    request.callables = callables.into_boxed_slice();
    let mut arguments = vec![
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1(1), vec![], SemanticTypeIdV1(0)).unwrap()
        );
        3
    ];
    arguments.extend(registers.into_iter().map(literal));
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(1),
        arguments,
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1(0), vec![], SemanticTypeIdV1(0)).unwrap(),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
    .with_ordered_program_source_v32(source(request.functions[0].identity));
    request.functions[0].blocks = vec![
        block(30, SemanticTerminatorKindV1::Call(call)),
        block(31, SemanticTerminatorKindV1::Return),
    ]
    .into_boxed_slice();
    request
}

fn block(identity_tag: u8, kind: SemanticTerminatorKindV1) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1(identity(identity_tag)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}

fn call_mut(request: &mut InertSemanticMirRequestV1) -> &mut SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = &mut request.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    call
}

fn binding_mut(request: &mut InertSemanticMirRequestV1) -> &mut SemanticNonBodyCallableBindingV1 {
    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut request.callables[1]
    else {
        unreachable!()
    };
    binding
}

fn program() -> SemanticGfx942U32ProgramV32 {
    SemanticGfx942U32ProgramV32::from_descriptors(3, words(&[0x85, 0x133, 0x19d])).unwrap()
}
fn call(request: &InertSemanticMirRequestV1) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = &request.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    call
}
fn checked(
    request: &InertSemanticMirRequestV1,
) -> Result<SemanticGfx942OrderedProgramCallV32<'_>, SemanticMirErrorV1> {
    checked_call(request, &request.functions[0], call(request))
}
fn assert_invalid(request: InertSemanticMirRequestV1) {
    assert_eq!(
        checked(&request),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
}
fn admitted(request: InertSemanticMirRequestV1) -> AdmittedInertSemanticMirV1 {
    request
        .admit_exact_v32(SemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn call_bindings_require_five_literal_u8_values_and_every_pair_disjoint() {
    for left in 0..5 {
        for right in left + 1..5 {
            let mut registers = REGISTERS;
            registers[right] = registers[left];
            assert_invalid(request(registers));
        }
        for value in 64..=255 {
            let mut registers = REGISTERS;
            registers[left] = value;
            assert_invalid(request(registers));
        }
        for operand in [
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1(2), vec![], SemanticTypeIdV1(1)).unwrap(),
            ),
            SemanticOperandV1::Move(
                SemanticPlaceV1::new(SemanticLocalIdV1(2), vec![], SemanticTypeIdV1(1)).unwrap(),
            ),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1(1),
                SemanticConstantValueV1::ZeroSized,
            )),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1(1),
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(32, 2).unwrap()),
            )),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1(0),
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(32, 4).unwrap()),
            )),
        ] {
            let mut fixture = request(REGISTERS);
            let mut locals = fixture.functions[0].locals.to_vec();
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(50)),
                SemanticTypeIdV1(1),
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ));
            fixture.functions[0].locals = locals.into_boxed_slice();
            call_mut(&mut fixture).arguments[left + 3] = operand;
            assert_invalid(fixture);
        }
    }
    for count in [0, 3, 7, 9] {
        let mut fixture = request(REGISTERS);
        call_mut(&mut fixture).arguments = vec![literal(32); count].into_boxed_slice();
        assert_invalid(fixture);
    }
    for unwind in [
        SemanticUnwindActionV1::Terminate,
        SemanticUnwindActionV1::Cleanup(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallUnwind,
            SemanticBlockIdV1(1),
        )),
    ] {
        let mut fixture = request(REGISTERS);
        call_mut(&mut fixture).unwind = unwind;
        assert_invalid(fixture);
    }
    let mut continuing = request(REGISTERS);
    call_mut(&mut continuing).unwind = SemanticUnwindActionV1::Continue;
    admitted(continuing);
}

#[test]
fn checked_borrow_retains_actual_call_and_full_program_not_source_authority() {
    for registers in [REGISTERS, [0, 63, 1, 62, 2], [63, 0, 62, 1, 61]] {
        let fixture = request(registers);
        let view = checked(&fixture).unwrap();
        assert_eq!(view.program(), program());
        assert_eq!(view.program().descriptors(), program().descriptors());
        assert_eq!(view.source(), source(fixture.functions[0].identity));
        assert_eq!(view.registers().scratch(), registers[0]);
        assert_eq!(view.registers().output(), registers[1]);
        assert_eq!(
            view.registers().inputs(),
            [registers[2], registers[3], registers[4]]
        );
        assert_eq!(
            view.registers().vgpr_high_water(),
            registers.into_iter().max().unwrap() + 1
        );
        assert!(std::ptr::eq(
            view.inputs().as_ptr(),
            call(&fixture).arguments.as_ptr()
        ));
        assert!(uses_v32(&fixture));
        validate_call_source(&fixture, &fixture.functions[0], call(&fixture)).unwrap();
    }
    let owner = admitted(request(REGISTERS));
    let view = owner
        .checked_gfx942_ordered_program_call_v32(SemanticFunctionIdV1(0), SemanticBlockIdV1(0))
        .unwrap();
    assert_eq!(view.program(), program());
    assert!(std::ptr::eq(
        view.inputs().as_ptr(),
        call(&owner.request).arguments.as_ptr()
    ));
    for (function, block) in [(0, 1), (0, 999), (999, 0)] {
        assert_eq!(
            owner.checked_gfx942_ordered_program_call_v32(
                SemanticFunctionIdV1(function),
                SemanticBlockIdV1(block)
            ),
            Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
        );
    }
    let mut wrong_wire = owner;
    wrong_wire.wire_version = SemanticMirWireVersionV1::V31;
    assert_eq!(
        wrong_wire
            .checked_gfx942_ordered_program_call_v32(SemanticFunctionIdV1(0), SemanticBlockIdV1(0)),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
}

#[test]
fn program_source_options_are_required_exclusive_and_bound_to_actual_caller() {
    let mut missing = request(REGISTERS);
    call_mut(&mut missing).ordered_program_source_v32 = None;
    assert_invalid(missing);
    let mut wrong = request(REGISTERS);
    call_mut(&mut wrong).ordered_program_source_v32 =
        Some(source(SemanticFunctionIdentityV1(identity(99))));
    assert_invalid(wrong);
    let mut old = request(REGISTERS);
    let old_ref = old_source(old.functions[0].identity);
    call_mut(&mut old).inline_assembly_source_v30 = Some(old_ref);
    assert_invalid(old);
    let mut pair = request(REGISTERS);
    let pair_ref =
        SemanticOrderedRegionSourceV31::new([1; 32], pair.functions[0].identity, [3; 32], [4; 32])
            .unwrap();
    call_mut(&mut pair).ordered_region_source_v31 = Some(pair_ref);
    assert_invalid(pair);
    let mut ordinary = request(REGISTERS);
    call_mut(&mut ordinary).callee = SemanticCallableIdV1(0);
    assert_eq!(
        validate_call_source(&ordinary, &ordinary.functions[0], call(&ordinary)),
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    );
    assert_invalid(ordinary);
    let mut absent = request(REGISTERS);
    call_mut(&mut absent).callee = SemanticCallableIdV1(999);
    assert_invalid(absent);
    let mut changed = request(REGISTERS);
    call_mut(&mut changed).ordered_program_source_v32 = Some(
        SemanticOrderedProgramSourceV32::new(
            [99; 32],
            changed.functions[0].identity,
            [3; 32],
            [4; 32],
        )
        .unwrap(),
    );
    assert!(checked(&changed).is_ok()); // nonzero declared reference is not authenticated
}

#[test]
fn source_reference_zeroes_and_private_corruption_are_rejected() {
    for slot in 0..4 {
        let mut ids = [[1; 32], [2; 32], [3; 32], [4; 32]];
        ids[slot] = [0; 32];
        assert_eq!(
            SemanticOrderedProgramSourceV32::new(
                ids[0],
                SemanticFunctionIdentityV1(ids[1]),
                ids[2],
                ids[3]
            ),
            Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
        );
        let mut fixture = request(REGISTERS);
        let mut inert = source(fixture.functions[0].identity);
        match slot {
            0 => inert.frontend_unit = [0; 32],
            1 => inert.function = SemanticFunctionIdentityV1([0; 32]),
            2 => inert.contract = [0; 32],
            3 => inert.statement = [0; 32],
            _ => unreachable!(),
        };
        call_mut(&mut fixture).ordered_program_source_v32 = Some(inert);
        assert_invalid(fixture);
    }
    let mut fixture = request(REGISTERS);
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut fixture.callables[1]
    else {
        unreachable!()
    };
    *operation =
        SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(SemanticGfx942U32ProgramV32 {
            count: 1,
            words: words(&[0x48]),
        });
    assert_invalid(fixture);
}

#[test]
fn actual_argument_type_and_abi_mismatches_are_refused_before_borrowing() {
    for position in 0..8 {
        let mut fixture = request(REGISTERS);
        call_mut(&mut fixture).arguments[position] = if position < 3 {
            literal(32)
        } else {
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1(1), vec![], SemanticTypeIdV1(0)).unwrap(),
            )
        };
        assert_invalid(fixture);
    }
    let mut wrong_abi = request(REGISTERS);
    let root_abi = wrong_abi.functions[0].abi.clone();
    binding_mut(&mut wrong_abi).abi = root_abi;
    assert_invalid(wrong_abi);
    let mut missing_type = request(REGISTERS);
    missing_type.types = vec![missing_type.types[0].clone()].into_boxed_slice();
    assert_invalid(missing_type);
    for missing in [2, u32::MAX] {
        for register_type in [false, true] {
            let mut fixture = request(REGISTERS);
            let abi = &mut binding_mut(&mut fixture).abi;
            let missing = SemanticTypeIdV1(missing);
            if register_type {
                abi.source_signature.inputs[3..].fill(missing);
            } else {
                abi.source_signature.output = missing;
                abi.source_signature.inputs[..3].fill(missing);
            }
            assert_invalid(fixture);
        }
    }
}

#[test]
fn no_source_option_on_an_ordinary_call_is_not_a_program_claim() {
    let mut fixture = request(REGISTERS);
    call_mut(&mut fixture).callee = SemanticCallableIdV1(0);
    call_mut(&mut fixture).ordered_program_source_v32 = None;
    assert_eq!(
        validate_call_source(&fixture, &fixture.functions[0], call(&fixture)),
        Ok(())
    );
    fixture.callables = vec![fixture.callables[0].clone()].into_boxed_slice();
    assert!(!uses_v32(&fixture));
    let value = source(fixture.functions[0].identity);
    call_mut(&mut fixture).ordered_program_source_v32 = Some(value);
    assert!(uses_v32(&fixture));
}

#[test]
fn exact_abi_and_scalar_shapes_are_required_even_when_lengths_are_large() {
    for case in 0..10 {
        let mut fixture = request(REGISTERS);
        let abi = &mut binding_mut(&mut fixture).abi;
        match case {
            0 => abi.can_unwind = true,
            1 => abi.source_signature.c_variadic = true,
            2 => abi.canon_abi = SemanticCanonAbiV1::C,
            3 => abi.source_signature.extern_abi = SemanticExternAbiV1::C { unwind: false },
            4 => abi.arguments[3].value.mode = SemanticAbiPassModeV1::Ignore,
            5 => abi.return_value.mode = SemanticAbiPassModeV1::Ignore,
            6 => abi.source_signature.inputs[3] = SemanticTypeIdV1(0),
            7 => abi.source_signature.inputs = vec![SemanticTypeIdV1(0); 1024].into_boxed_slice(),
            8 => abi.arguments = vec![abi.arguments[0].clone(); 1024].into_boxed_slice(),
            9 => abi.source_signature.output = SemanticTypeIdV1(1),
            _ => unreachable!(),
        }
        let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &fixture.callables[1]
        else {
            unreachable!()
        };
        assert!(!signature_matches(&fixture, &binding.abi));
        assert_invalid(fixture);
    }
    for (index, signed, bits) in [(0, true, 32), (0, false, 64), (1, true, 8), (1, false, 16)] {
        let mut fixture = request(REGISTERS);
        fixture.types[index].shape =
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits });
        assert_invalid(fixture);
    }
    let mut old_profile = request(REGISTERS);
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut old_profile.callables[1]
    else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(
        SemanticGfx942OrderedRegionProfileV31::XorAddU32E32,
    );
    assert_invalid(old_profile);
}

#[test]
fn every_single_physical_binding_mutant_obeys_the_declared_range_and_disjointness() {
    for position in 0..5 {
        for number in 0..=u8::MAX {
            let mut candidate = REGISTERS;
            candidate[position] = number;
            let expected = candidate.iter().all(|&register| register < 64)
                && (0..5).all(|i| (0..i).all(|j| candidate[i] != candidate[j]));
            let value = SemanticGfx942OrderedProgramRegistersV32::new(
                candidate[0],
                candidate[1],
                [candidate[2], candidate[3], candidate[4]],
            );
            assert_eq!(value.is_ok(), expected, "bindings={candidate:?}");
            if let Ok(value) = value {
                assert_eq!(value.scratch(), candidate[0]);
                assert_eq!(value.output(), candidate[1]);
                assert_eq!(value.inputs(), [candidate[2], candidate[3], candidate[4]]);
                assert_eq!(
                    value.vgpr_high_water(),
                    *candidate.iter().max().unwrap() + 1
                );
            }
        }
    }
}
