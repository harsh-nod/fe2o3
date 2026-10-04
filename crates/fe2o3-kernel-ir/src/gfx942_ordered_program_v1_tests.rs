//! Synthetic local-contract controls; no source/native/artifact authority.
use super::*;

use crate::{AccessMode, AddressSpace, Constant, Type, ValueDef};
type Error = Gfx942OrderedProgramErrorV1;
type Registers = Gfx942OrderedProgramRegistersV1;
fn source() -> AssemblySourceIdentity {
    AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32])
}
fn program() -> Gfx942U32ProgramV1 {
    Gfx942U32ProgramV1::from_descriptors(3, words(&[0x85, 0x133, 0x19d])).unwrap()
}
fn region() -> Gfx942OrderedProgramV1 {
    Gfx942OrderedProgramV1::new(
        source(),
        Registers::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
        program(),
    )
    .unwrap()
}
fn operation() -> Operation {
    Operation::new(
        vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))],
        OperationKind::Gfx942OrderedProgram(region()),
    )
}
fn validate(operation: &Operation) -> Result<ValidatedGfx942OrderedProgramV1, Error> {
    validate_gfx942_ordered_program_v1(operation, |id| (id.0 < 3).then_some(ScalarType::U32))
}

const CASES: [[u32; 3]; 6] = [
    [0, 0, 0],
    [u32::MAX, 0, 1],
    [u32::MAX, 1, 2],
    [0x8000_0000, 0, 0x8000_0000],
    [0xaaaa_5555, 0x5555_aaaa, 19],
    [19, 23, 42],
];

fn words(active: &[u16]) -> [u16; GFX942_U32_PROGRAM_MAX_STEPS_V1] {
    assert!(active.len() <= GFX942_U32_PROGRAM_MAX_STEPS_V1);
    let mut result = [0; GFX942_U32_PROGRAM_MAX_STEPS_V1];
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
    let program = Gfx942U32ProgramV1::from_descriptors(3, words(&active)).unwrap();
    assert_eq!(program.packed_words(), [0x0000_019d_0133_0085, 0, 0, 0]);
    assert_eq!(program.count(), 3);
    assert_eq!(program.instructions().len(), 3);
    let typed: Vec<_> = program.instructions().collect();
    assert_eq!(
        Gfx942U32ProgramV1::from_instructions(&typed).unwrap(),
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
    let program = Gfx942U32ProgramV1::from_packed(16, packed).unwrap();
    assert_eq!(program.descriptors(), &descriptors);
    assert_eq!(program.packed_words(), packed);
    assert_eq!(
        program
            .instructions()
            .map(Gfx942ProgramInstructionV1::descriptor)
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
    let repeated = Gfx942U32ProgramV1::from_descriptors(16, [0x0008; 16]).unwrap();
    let single = Gfx942U32ProgramV1::from_descriptors(1, words(&[0x0008])).unwrap();
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
        let decoded = Gfx942ProgramInstructionV1::from_descriptor(word);
        assert_eq!(decoded.is_ok(), metadata(word).is_some(), "word={word:04x}");
        if let Ok(instruction) = decoded {
            structurally_valid += 1;
            assert_eq!(instruction.descriptor(), word);
        }
        let program = Gfx942U32ProgramV1::from_descriptors(1, words(&[word]));
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
            let program = Gfx942U32ProgramV1::from_descriptors(2, words(&active));
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
            let program = Gfx942U32ProgramV1::from_descriptors(2, words(&active));
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
            Gfx942U32ProgramV1::from_descriptors(count, descriptors).is_ok(),
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
                    Gfx942U32ProgramV1::from_descriptors(count, mutated),
                    Err(Gfx942ProgramErrorV1::NonZeroPadding {
                        step,
                        descriptor: 1 << bit
                    })
                );
            }
        }
    }
    let move_out = Gfx942ProgramInstructionV1::Move {
        destination: Gfx942ProgramDestinationV1::Output,
        source: Gfx942ProgramRoleV1::Input0,
    };
    assert_eq!(
        Gfx942U32ProgramV1::from_instructions(&[]),
        Err(Gfx942ProgramErrorV1::StepCount { count: 0 })
    );
    assert_eq!(
        Gfx942U32ProgramV1::from_instructions(&[move_out; 17]),
        Err(Gfx942ProgramErrorV1::StepCount { count: 17 })
    );
}

#[test]
fn packing_is_const_and_all_sixty_four_bit_positions_round_trip() {
    const WORDS: [u16; 16] = [0x0008; 16];
    const PACKED: [u64; 4] = pack_program_descriptors_v1(WORDS);
    const UNPACKED: [u16; 16] = unpack_program_descriptors_v1(PACKED);
    assert_eq!(UNPACKED, WORDS);
    for pack in 0..4 {
        for bit in 0..64 {
            let mut input = [0; 4];
            input[pack] = 1_u64 << bit;
            let unpacked = unpack_program_descriptors_v1(input);
            let mut expected = [0; 16];
            expected[4 * pack + bit / 16] = 1_u16 << (bit % 16);
            assert_eq!(unpacked, expected);
            assert_eq!(pack_program_descriptors_v1(unpacked), input);
        }
    }
    let program = Gfx942U32ProgramV1::from_packed(16, PACKED).unwrap();
    assert_eq!(program.packed_words(), PACKED);
}

#[test]
fn precise_error_classes_reject_reserved_unary_reads_and_missing_output() {
    for bit in 10..16 {
        assert_eq!(
            Gfx942ProgramInstructionV1::from_descriptor(0x0008 | (1 << bit)),
            Err(Gfx942ProgramDescriptorErrorV1::ReservedBits { bits: 1 << bit })
        );
    }
    for tag in [6, 7] {
        assert_eq!(
            Gfx942ProgramInstructionV1::from_descriptor(8 | tag),
            Err(Gfx942ProgramDescriptorErrorV1::Opcode { tag: tag as u8 })
        );
    }
    assert_eq!(
        Gfx942ProgramInstructionV1::from_descriptor(0x0058),
        Err(Gfx942ProgramDescriptorErrorV1::Source { operand: 0, tag: 5 })
    );
    assert_eq!(
        Gfx942ProgramInstructionV1::from_descriptor(0x0389),
        Err(Gfx942ProgramDescriptorErrorV1::Source { operand: 1, tag: 7 })
    );
    for tag in 1..=7_u8 {
        assert_eq!(
            Gfx942ProgramInstructionV1::from_descriptor(8 | (u16::from(tag) << 7)),
            Err(Gfx942ProgramDescriptorErrorV1::MoveUnusedSource { tag })
        );
    }
    assert_eq!(
        Gfx942U32ProgramV1::from_descriptors(1, words(&[0x0039])),
        Err(Gfx942ProgramErrorV1::ReadBeforeDefinition {
            step: 0,
            roles: 0b01000
        })
    );
    assert_eq!(
        Gfx942U32ProgramV1::from_descriptors(1, words(&[0x0048])),
        Err(Gfx942ProgramErrorV1::ReadBeforeDefinition {
            step: 0,
            roles: 0b10000
        })
    );
    assert_eq!(
        Gfx942U32ProgramV1::from_descriptors(1, words(&[0x0000])),
        Err(Gfx942ProgramErrorV1::OutputNotDefined)
    );
    assert_eq!(
        Gfx942U32ProgramV1::from_descriptors(1, words(&[0x0389])),
        Err(Gfx942ProgramErrorV1::Descriptor {
            step: 0,
            error: Gfx942ProgramDescriptorErrorV1::Source { operand: 1, tag: 7 }
        })
    );
}

#[test]
fn output_first_scratch_optional_repeated_inputs_and_wrapping_are_explicit() {
    let xor_add = Gfx942U32ProgramV1::from_descriptors(2, words(&[0x008d, 0x0149])).unwrap();
    let twice_output = Gfx942U32ProgramV1::from_descriptors(2, words(&[0x0008, 0x0249])).unwrap();
    let subtract = Gfx942U32ProgramV1::from_descriptors(1, words(&[0x008a])).unwrap();
    for [a, b, c] in CASES {
        assert_eq!(xor_add.evaluate([a, b, c]), (a ^ b).wrapping_add(c));
        assert_eq!(twice_output.evaluate([a, b, c]), a.wrapping_add(a));
        assert_eq!(subtract.evaluate([a, b, c]), a.wrapping_sub(b));
    }
    assert_eq!(subtract.evaluate([0, 1, 0]), u32::MAX);
    assert_eq!(twice_output.evaluate([0x8000_0000, 0, 0]), 0);
    let alternate = Gfx942U32ProgramV1::from_descriptors(2, words(&[0x0010, 0x0008])).unwrap();
    let direct = Gfx942U32ProgramV1::from_descriptors(1, words(&[0x0008])).unwrap();
    assert_ne!(alternate, direct); // A dead scratch write must not disappear.
    assert_eq!(
        alternate.evaluate([19, 23, 42]),
        direct.evaluate([19, 23, 42])
    );
}

#[test]
fn every_single_binding_mutant_obeys_distinct_zero_through_sixty_three() {
    let baseline = [32, 33, 34, 35, 36];
    for index in 0..5 {
        for register in 0..=u8::MAX {
            let mut candidate = baseline;
            candidate[index] = register;
            let expected = candidate.iter().all(|&number| number < 64)
                && (0..5).all(|i| (0..i).all(|j| candidate[i] != candidate[j]));
            let actual = Gfx942OrderedProgramRegistersV1::new(
                candidate[0],
                candidate[1],
                [candidate[2], candidate[3], candidate[4]],
            );
            assert_eq!(actual.is_ok(), expected, "bindings={candidate:?}");
            if let Ok(bindings) = actual {
                assert_eq!(
                    [
                        bindings.scratch(),
                        bindings.output(),
                        bindings.inputs()[0],
                        bindings.inputs()[1],
                        bindings.inputs()[2]
                    ],
                    candidate
                );
                assert_eq!(
                    bindings.vgpr_high_water(),
                    *candidate.iter().max().unwrap() + 1
                );
            }
        }
    }
    let bindings = Gfx942OrderedProgramRegistersV1::new(0, 63, [1, 2, 3]).unwrap();
    assert_eq!(bindings.vgpr_high_water(), 64);
    for (role, number) in [
        (Gfx942ProgramRoleV1::Scratch, 0),
        (Gfx942ProgramRoleV1::Output, 63),
        (Gfx942ProgramRoleV1::Input0, 1),
        (Gfx942ProgramRoleV1::Input1, 2),
        (Gfx942ProgramRoleV1::Input2, 3),
    ] {
        assert_eq!(bindings.binding(role), number);
    }
    assert_eq!(
        Gfx942OrderedProgramRegistersV1::new(64, 0, [1, 2, 3]),
        Err(Gfx942OrderedProgramErrorV1::RegisterOutOfRange)
    );
    assert_eq!(
        Gfx942OrderedProgramRegistersV1::new(32, 32, [34, 35, 36]),
        Err(Gfx942OrderedProgramErrorV1::RegisterOverlap)
    );
}
#[test]
fn all_alias_pairs_and_all_out_of_profile_indices_are_rejected() {
    for first in 0..5 {
        for second in first + 1..5 {
            let mut bindings = [0, 1, 2, 3, 4];
            bindings[second] = bindings[first];
            assert_eq!(
                Registers::new(
                    bindings[0],
                    bindings[1],
                    [bindings[2], bindings[3], bindings[4]]
                ),
                Err(Error::RegisterOverlap)
            );
        }
    }
    for position in 0..5 {
        for invalid in 64..=u8::MAX {
            let mut bindings = [0, 1, 2, 3, 4];
            bindings[position] = invalid;
            assert_eq!(
                Registers::new(
                    bindings[0],
                    bindings[1],
                    [bindings[2], bindings[3], bindings[4]]
                ),
                Err(Error::RegisterOutOfRange)
            );
        }
    }
}

#[test]
fn each_missing_source_reference_is_rejected_without_authenticating_nonzero_ids() {
    for axis in 0..4 {
        let mut ids = [[1; 32], [2; 32], [3; 32], [4; 32]];
        ids[axis] = [0; 32];
        assert_eq!(
            Gfx942OrderedProgramV1::new(
                AssemblySourceIdentity::new(ids[0], ids[1], ids[2], ids[3]),
                region().registers(),
                [ValueId(0); 3],
                program(),
            ),
            Err(Error::IncompleteSourceIdentity)
        );
    }
    let mut mostly_zero = [0; 32];
    mostly_zero[31] = 1;
    let inert = AssemblySourceIdentity::new(mostly_zero, mostly_zero, mostly_zero, mostly_zero);
    assert!(
        Gfx942OrderedProgramV1::new(inert, region().registers(), [ValueId(0); 3], program())
            .is_ok()
    );
}

#[test]
fn validation_retains_exact_inputs_result_and_shape_only_references() {
    let validated = validate(&operation()).unwrap();
    assert_eq!(validated.result(), ValueId(3));
    assert_eq!(validated.inputs(), &[ValueId(0), ValueId(1), ValueId(2)]);
    assert_eq!(validated.source(), source());
    assert_eq!(validated.registers(), region().registers());
    assert_eq!(validated.program(), region().program());
    let mut same_ssa = operation();
    same_ssa.kind = OperationKind::Gfx942OrderedProgram(
        Gfx942OrderedProgramV1::new(source(), region().registers(), [ValueId(0); 3], program())
            .unwrap(),
    );
    assert_eq!(validate(&same_ssa).unwrap().inputs(), &[ValueId(0); 3]);
}

#[test]
fn wrong_operation_result_arity_and_non_u32_results_are_rejected() {
    let mut candidate = operation();
    candidate.kind = OperationKind::Constant(Constant::U32(7));
    assert_eq!(validate(&candidate), Err(Error::NotOrderedProgram));
    candidate = operation();
    candidate.results.clear();
    assert_eq!(validate(&candidate), Err(Error::ResultArity));
    candidate = operation();
    candidate
        .results
        .push(ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)));
    assert_eq!(validate(&candidate), Err(Error::ResultArity));
    for ty in [
        Type::Scalar(ScalarType::I32),
        Type::Scalar(ScalarType::U64),
        Type::Scalar(ScalarType::F32),
        Type::Scalar(ScalarType::Bool),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    ] {
        candidate = operation();
        candidate.results[0].ty = ty;
        assert_eq!(validate(&candidate), Err(Error::ResultType));
    }
}

#[test]
fn every_missing_or_non_u32_input_definition_is_rejected() {
    for bad_id in 0..3 {
        for bad_type in [
            None,
            Some(ScalarType::I32),
            Some(ScalarType::U64),
            Some(ScalarType::F32),
        ] {
            assert_eq!(
                validate_gfx942_ordered_program_v1(&operation(), |id| {
                    if id == ValueId(bad_id) {
                        bad_type
                    } else {
                        Some(ScalarType::U32)
                    }
                }),
                Err(Error::InputType)
            );
        }
    }
}

#[test]
fn validator_rechecks_private_shape_before_returning_a_descriptor() {
    // Private malformed values model future internal construction mistakes; no
    // public constructor can create these states.
    let mut malformed = region();
    malformed.source.statement = [0; 32];
    let mut candidate = operation();
    candidate.kind = OperationKind::Gfx942OrderedProgram(malformed);
    assert_eq!(validate(&candidate), Err(Error::IncompleteSourceIdentity));
    malformed = region();
    malformed.registers.output = malformed.registers.scratch;
    candidate.kind = OperationKind::Gfx942OrderedProgram(malformed);
    assert_eq!(validate(&candidate), Err(Error::RegisterOverlap));
    malformed.registers.output = 64;
    candidate.kind = OperationKind::Gfx942OrderedProgram(malformed);
    assert_eq!(validate(&candidate), Err(Error::RegisterOutOfRange));
}

#[test]
fn required_capabilities_are_exact_sorted_and_charge_all_three_publications() {
    use crate::{CanonicalKernelIrWorkBudgetV1, TargetCapability, WaveWidth};
    let expected = [
        TargetCapability::Extension {
            namespace: AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME.to_owned(),
        },
        TargetCapability::Extension {
            namespace: crate::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
            name: crate::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.to_owned(),
        },
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ];
    let operation = operation();
    assert_eq!(
        operation.required_capabilities(),
        expected.clone().into_iter().collect()
    );
    assert_eq!(operation.required_capability_visitation_work_v1(), Some(1));
    for (limit, succeeds, published) in [(4, true, 3), (3, false, 2)] {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(limit);
        budget.charge_work(1).unwrap();
        let mut count = 0;
        let result = operation.try_visit_required_capabilities_v1(|capability| {
            budget.charge_work(1)?;
            assert!(capability.matches(&expected[count]));
            count += 1;
            Ok::<_, crate::CanonicalKernelIrWorkLimitV1>(())
        });
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(count, published);
        assert_eq!(budget.work(), limit);
    }
}
#[test]
fn payload_and_validator_recheck_every_private_program_invariant() {
    let malformed = [
        Gfx942U32ProgramV1 {
            count: 0,
            words: [0; 16],
        },
        Gfx942U32ProgramV1 {
            count: 17,
            words: [8; 16],
        },
        Gfx942U32ProgramV1 {
            count: 1,
            words: words(&[0x408]),
        },
        Gfx942U32ProgramV1 {
            count: 1,
            words: words(&[0x48]),
        },
        Gfx942U32ProgramV1 {
            count: 1,
            words: words(&[0]),
        },
        Gfx942U32ProgramV1 {
            count: 1,
            words: words(&[8, 1]),
        },
    ];
    for program in malformed {
        let error = Gfx942U32ProgramV1::from_descriptors(program.count, program.words).unwrap_err();
        assert_eq!(
            Gfx942OrderedProgramV1::new(source(), region().registers(), [ValueId(0); 3], program),
            Err(Error::InvalidProgram(error))
        );
        let mut payload = region();
        payload.program = program;
        let mut candidate = operation();
        candidate.kind = OperationKind::Gfx942OrderedProgram(payload);
        assert_eq!(validate(&candidate), Err(Error::InvalidProgram(error)));
    }
}

#[test]
fn retained_steps_and_register_roles_are_independent_of_scalar_equivalence() {
    let single = Gfx942U32ProgramV1::from_descriptors(1, words(&[8])).unwrap();
    let repeated = Gfx942U32ProgramV1::from_descriptors(16, [8; 16]).unwrap();
    let a = Gfx942OrderedProgramV1::new(source(), region().registers(), [ValueId(0); 3], single)
        .unwrap();
    let b = Gfx942OrderedProgramV1::new(source(), region().registers(), [ValueId(0); 3], repeated)
        .unwrap();
    assert_ne!(a, b);
    assert_eq!(
        a.program().evaluate([19, 23, 42]),
        b.program().evaluate([19, 23, 42])
    );
    assert_eq!(b.program().instructions().len(), 16);
    for (role, expected) in [
        (Gfx942ProgramRoleV1::Scratch, 32),
        (Gfx942ProgramRoleV1::Output, 33),
        (Gfx942ProgramRoleV1::Input0, 34),
        (Gfx942ProgramRoleV1::Input1, 35),
        (Gfx942ProgramRoleV1::Input2, 36),
    ] {
        assert_eq!(b.registers().binding(role), expected);
    }
}
