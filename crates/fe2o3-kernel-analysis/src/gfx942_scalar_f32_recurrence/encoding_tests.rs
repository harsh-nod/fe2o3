use super::*;
use crate::{
    PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1,
    PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1, PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1,
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineEffectBudgetV1,
    PhysicalMachineEffectEntryRequestV1, PhysicalMachineEffectEvidenceV1,
    PhysicalMachineExecutionChallengeV1, PhysicalMachineOperandValueV1,
    PhysicalMachineToolchainIdentityV1,
};

const FUNCTION: &str = "recurrence_encoding_test";

// Canonical test records exercise the private structural checker, never authenticated custody.
#[derive(Clone)]
struct Instruction {
    opcode: &'static str,
    encoding: Vec<u8>,
    definitions: u16,
    operands: Vec<PhysicalMachineOperandValueV1>,
    tied_operand: Option<(usize, u16)>,
    implicit_definitions: Vec<&'static str>,
    implicit_uses: Vec<&'static str>,
    branch: u8,
    flags: u16,
    memory: u8,
}

fn instruction(opcode: &'static str, destination: u8, sources: &[u8]) -> Instruction {
    let (word, uses) = match (opcode, sources) {
        ("V_MUL_F32_e32_vi" | "V_ADD_F32_e32_vi", [left, right]) => (
            if opcode == "V_MUL_F32_e32_vi" {
                0x0a00_0000
            } else {
                0x0200_0000
            } | (u32::from(destination) << 17)
                | (u32::from(*right) << 9)
                | 0x100
                | u32::from(*left),
            vec!["EXEC", "MODE"],
        ),
        ("V_MOV_B32_e32" | "V_MOV_B32_e32_vi", [source]) => (
            0x7e00_0300 | (u32::from(destination) << 17) | u32::from(*source),
            vec!["EXEC"],
        ),
        _ => panic!("unsupported test instruction"),
    };
    Instruction {
        opcode,
        encoding: word.to_le_bytes().to_vec(),
        definitions: 1,
        operands: std::iter::once(destination)
            .chain(sources.iter().copied())
            .map(|index| PhysicalMachineOperandValueV1::Register(format!("VGPR{index}")))
            .collect(),
        tied_operand: None,
        implicit_definitions: vec![],
        implicit_uses: uses,
        branch: 0,
        flags: 0,
        memory: 0,
    }
}

fn decode(
    body: &[Instruction],
) -> (
    PhysicalMachineEffectRequestV1,
    PhysicalMachineTraceEvidenceV1,
) {
    let mut instructions = body.to_vec();
    if instructions
        .last()
        .is_none_or(|instruction| instruction.branch != 4)
    {
        instructions.push(Instruction {
            opcode: "S_ENDPGM_vi",
            encoding: 0xbf81_0000_u32.to_le_bytes().to_vec(),
            definitions: 0,
            operands: vec![PhysicalMachineOperandValueV1::SignedImmediate(0)],
            tied_operand: None,
            implicit_definitions: vec![],
            implicit_uses: vec![],
            branch: 4,
            flags: 12,
            memory: 0,
        });
    }
    let mut payload = vec![0; 8];
    let mut offsets = Vec::new();
    for instruction in &instructions {
        offsets.push(payload.len() as u64);
        payload.extend_from_slice(&instruction.encoding);
    }
    let size = payload.len() as u64 - 8;
    let request = PhysicalMachineEffectRequestV1::new(
        PhysicalMachineExecutionChallengeV1::from_sha256_bytes([1; 32]),
        PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([2; 32]),
        PhysicalMachineToolchainIdentityV1::from_sha256_bytes([3; 32]),
        payload,
        vec![
            PhysicalMachineEffectEntryRequestV1::new(
                FUNCTION,
                PhysicalMachineEffectBudgetV1::new(0, 0, 0, 1, 0),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let mut effects = PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.to_vec();
    push_u32(&mut effects, 0);
    push_u16(&mut effects, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1);
    effects.extend_from_slice(&request.execution_challenge().as_bytes());
    effects.extend_from_slice(&request.identity().sha256());
    push_u64(&mut effects, request.identity().byte_len());
    effects.extend_from_slice(&request.payload_identity().sha256());
    push_u64(&mut effects, request.payload_identity().byte_len());
    effects.extend_from_slice(&request.analyzer_identity().as_bytes());
    effects.extend_from_slice(&request.toolchain_identity().as_bytes());
    push_u16(&mut effects, 1);
    push_u16(&mut effects, 1);
    push_text(&mut effects, FUNCTION);
    effects.extend_from_slice(&[4; 32]);
    push_u64(&mut effects, 8);
    push_u64(&mut effects, size);
    push_u32(&mut effects, 1);
    push_text(&mut effects, FUNCTION);
    push_u64(&mut effects, 8);
    push_u64(&mut effects, size);
    push_u16(&mut effects, 0);
    push_u32(&mut effects, 1);
    push_text(&mut effects, FUNCTION);
    push_text(&mut effects, FUNCTION);
    push_u64(&mut effects, *offsets.last().unwrap());
    effects.push(4);
    push_u16(&mut effects, 0);
    finish_test_record(
        &mut effects,
        PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len(),
    );
    let effects =
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &effects).unwrap();

    let mut trace = PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1.to_vec();
    push_u32(&mut trace, 0);
    push_u16(&mut trace, PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1);
    trace.extend_from_slice(&request.execution_challenge().as_bytes());
    for (hash, length) in [
        (request.identity().sha256(), request.identity().byte_len()),
        (effects.identity().sha256(), effects.identity().byte_len()),
        (
            request.payload_identity().sha256(),
            request.payload_identity().byte_len(),
        ),
    ] {
        trace.extend_from_slice(&hash);
        push_u64(&mut trace, length);
    }
    trace.extend_from_slice(&request.analyzer_identity().as_bytes());
    trace.extend_from_slice(&request.toolchain_identity().as_bytes());
    push_u16(&mut trace, 1);
    push_u32(&mut trace, instructions.len() as u32);
    for (index, instruction) in instructions.iter().enumerate() {
        push_text(&mut trace, FUNCTION);
        push_u32(&mut trace, index as u32);
        push_u64(&mut trace, offsets[index]);
        push_u32(&mut trace, 1);
        push_u16(&mut trace, u16::from(instruction.branch != 4));
        if instruction.branch != 4 {
            push_u32(&mut trace, index as u32 + 1);
        }
    }
    push_u32(&mut trace, instructions.len() as u32);
    for (index, instruction) in instructions.iter().enumerate() {
        push_text(&mut trace, FUNCTION);
        push_u64(&mut trace, offsets[index]);
        push_u32(&mut trace, index as u32);
        push_text(&mut trace, instruction.opcode);
        push_u16(&mut trace, instruction.encoding.len() as u16);
        trace.extend_from_slice(&instruction.encoding);
        push_u16(&mut trace, instruction.definitions);
        push_u16(&mut trace, instruction.operands.len() as u16);
        for (operand_index, operand) in instruction.operands.iter().enumerate() {
            trace.push(match operand {
                PhysicalMachineOperandValueV1::Register(_) => 1,
                PhysicalMachineOperandValueV1::SignedImmediate(_) => 2,
                PhysicalMachineOperandValueV1::SingleFloatImmediate(_) => 3,
                PhysicalMachineOperandValueV1::DoubleFloatImmediate(_) => 4,
                PhysicalMachineOperandValueV1::AbsoluteExpression(_) => 5,
            });
            push_u16(
                &mut trace,
                instruction
                    .tied_operand
                    .filter(|(index, _)| *index == operand_index)
                    .map_or(u16::MAX, |(_, target)| target),
            );
            match operand {
                PhysicalMachineOperandValueV1::Register(name) => push_text(&mut trace, name),
                PhysicalMachineOperandValueV1::SignedImmediate(value)
                | PhysicalMachineOperandValueV1::AbsoluteExpression(value) => {
                    push_u64(&mut trace, *value as u64)
                }
                PhysicalMachineOperandValueV1::SingleFloatImmediate(value) => {
                    push_u32(&mut trace, *value)
                }
                PhysicalMachineOperandValueV1::DoubleFloatImmediate(value) => {
                    push_u64(&mut trace, *value)
                }
            }
        }
        for registers in [
            &instruction.implicit_definitions,
            &instruction.implicit_uses,
        ] {
            push_u16(&mut trace, registers.len() as u16);
            for register in registers {
                push_text(&mut trace, register);
            }
        }
        trace.push(instruction.branch);
        push_u64(
            &mut trace,
            if matches!(instruction.branch, 1 | 2) {
                offsets[index + 1]
            } else {
                0
            },
        );
        push_u16(&mut trace, instruction.flags);
        trace.push(instruction.memory);
        push_u16(&mut trace, if instruction.memory == 0 { 0 } else { 4 });
    }
    finish_test_record(&mut trace, PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1.len());
    let trace =
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &trace).unwrap();
    (request, trace)
}

fn finish_test_record(bytes: &mut [u8], domain_length: usize) {
    let length = bytes.len() as u32;
    bytes[domain_length..domain_length + 4].copy_from_slice(&length.to_le_bytes());
}

fn analyze(
    body: &[Instruction],
) -> Result<Gfx942ScalarF32RecurrenceStepArtifactV1, Gfx942ScalarF32RecurrenceStepAnalysisErrorV1> {
    let (request, trace) = decode(body);
    analyze_recurrence_step(
        AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1::from_parts([5; 32], 64),
        &request,
        &trace,
        FUNCTION,
    )
}

#[test]
fn canonical_vop_encodings_accept_aliases_and_repeated_sources() {
    // LLVM 22.1.8 test/MC/AMDGPU/vop2.s independently records these VI words.
    for (opcode, expected) in [
        ("V_MUL_F32_e32_vi", [0x02, 0x07, 0x02, 0x0a]),
        ("V_ADD_F32_e32_vi", [0x02, 0x07, 0x02, 0x02]),
    ] {
        assert_eq!(instruction(opcode, 1, &[2, 3]).encoding, expected);
        for (destination, left, right) in [
            (1, 2, 3),
            (1, 1, 3),
            (3, 2, 3),
            (1, 2, 2),
            (0, 0, 0),
            (255, 255, 255),
        ] {
            let (_, trace) = decode(&[instruction(opcode, destination, &[left, right])]);
            let shape = arithmetic_shape(&trace.instructions()[0]).unwrap();
            assert_eq!(
                shape.destination,
                Gfx942RegisterUnitV1::Vgpr(u16::from(destination))
            );
            assert_eq!(
                shape.sources,
                [
                    Gfx942RegisterUnitV1::Vgpr(u16::from(left)),
                    Gfx942RegisterUnitV1::Vgpr(u16::from(right))
                ]
            );
        }
    }
    assert_eq!(
        instruction("V_MOV_B32_e32_vi", 0, &[4]).encoding,
        [0x04, 0x03, 0x00, 0x7e]
    );
    for opcode in ["V_MOV_B32_e32", "V_MOV_B32_e32_vi"] {
        for (destination, source) in [(0, 4), (0, 0), (255, 255)] {
            let (_, trace) = decode(&[instruction(opcode, destination, &[source])]);
            assert_eq!(
                admitted_register_copy_source(
                    &trace.instructions()[0],
                    Gfx942RegisterUnitV1::Vgpr(u16::from(destination))
                )
                .unwrap(),
                Some(Gfx942RegisterUnitV1::Vgpr(u16::from(source)))
            );
        }
    }
}

fn assert_rejected(instruction: Instruction) {
    let (_, trace) = decode(std::slice::from_ref(&instruction));
    let decoded = &trace.instructions()[0];
    if instruction.opcode.starts_with("V_MOV_B32") {
        assert_eq!(
            admitted_register_copy_source(decoded, Gfx942RegisterUnitV1::Vgpr(1)).unwrap(),
            None
        );
    } else {
        assert!(
            arithmetic_shape(decoded).is_err(),
            "accepted {}",
            instruction.opcode
        );
    }
}

#[test]
fn every_raw_opcode_format_and_register_bit_is_reconciled() {
    for original in [
        instruction("V_MUL_F32_e32_vi", 1, &[2, 3]),
        instruction("V_ADD_F32_e32_vi", 1, &[2, 3]),
        instruction("V_MOV_B32_e32_vi", 1, &[2]),
    ] {
        for bit in 0..32 {
            let mut changed = original.clone();
            changed.encoding[bit / 8] ^= 1 << (bit % 8);
            assert_rejected(changed);
        }
        for selector in [0_u16, 128, 249, 250, 255] {
            let mut changed = original.clone();
            changed.encoding[0] = selector as u8;
            changed.encoding[1] &= !1;
            assert_rejected(changed);
        }
        for length in [3, 5, 8] {
            let mut changed = original.clone();
            changed.encoding.resize(length, 0);
            assert_rejected(changed);
        }
    }
}

#[test]
fn operand_roles_and_implicit_effects_must_match_the_instruction() {
    for original in [
        instruction("V_MUL_F32_e32_vi", 1, &[1, 3]),
        instruction("V_ADD_F32_e32_vi", 1, &[1, 3]),
        instruction("V_MOV_B32_e32_vi", 1, &[1]),
    ] {
        for index in 0..original.operands.len() {
            for name in ["VGPR7", "SGPR1", "VGPR1_VGPR2"] {
                let mut changed = original.clone();
                changed.operands[index] = PhysicalMachineOperandValueV1::Register(name.to_owned());
                assert_rejected(changed);
            }
        }
        for value in [
            PhysicalMachineOperandValueV1::SignedImmediate(1),
            PhysicalMachineOperandValueV1::SingleFloatImmediate(0x3f80_0000),
            PhysicalMachineOperandValueV1::DoubleFloatImmediate(0x3ff0_0000_0000_0000),
            PhysicalMachineOperandValueV1::AbsoluteExpression(1),
        ] {
            let mut changed = original.clone();
            changed.operands[1] = value;
            assert_rejected(changed);
        }
        for count in [0, 2] {
            let mut changed = original.clone();
            changed.definitions = count;
            assert_rejected(changed);
        }
        for extra in [false, true] {
            let mut changed = original.clone();
            if extra {
                changed.operands.push(changed.operands[1].clone());
            } else {
                changed.operands.pop();
            }
            assert_rejected(changed);
        }
        for tie in [(0, 1), (1, 0)] {
            let mut changed = original.clone();
            changed.tied_operand = Some(tie);
            assert_rejected(changed);
        }
        for definition in ["EXEC", "MODE", "SCC", "VGPR1"] {
            let mut changed = original.clone();
            changed.implicit_definitions = vec![definition];
            assert_rejected(changed);
        }
        for uses in [
            vec![],
            vec!["MODE"],
            vec!["EXEC_HI", "EXEC_LO"],
            vec!["EXEC", "MODE", "SCC"],
        ] {
            let mut changed = original.clone();
            changed.implicit_uses = uses;
            assert_rejected(changed);
        }
        let mut changed = original.clone();
        changed.implicit_uses = if original.opcode.starts_with("V_MOV") {
            vec!["EXEC", "MODE"]
        } else {
            vec!["EXEC"]
        };
        assert_rejected(changed);
    }
}

#[test]
fn flags_memory_and_control_facts_cannot_be_added_to_vop_records() {
    // Native makeInstructionTrace serializes load/store/terminator/barrier/
    // predicable/trap only, not isConvergent or mayRaiseFPException.
    for original in [
        instruction("V_MUL_F32_e32_vi", 1, &[2, 3]),
        instruction("V_ADD_F32_e32_vi", 1, &[2, 3]),
        instruction("V_MOV_B32_e32_vi", 1, &[2]),
    ] {
        for (flags, memory) in [(8, 0), (16, 0), (32, 0), (1, 4), (2, 5), (3, 6)] {
            let mut changed = original.clone();
            changed.flags = flags;
            changed.memory = memory;
            assert_rejected(changed);
        }
        for branch in [1, 2, 4] {
            let mut changed = original.clone();
            changed.branch = branch;
            changed.flags = 4;
            assert_rejected(changed);
        }
    }
}

fn recurrence() -> Vec<Instruction> {
    vec![
        instruction("V_MOV_B32_e32_vi", 0, &[4]),
        instruction("V_MUL_F32_e32_vi", 1, &[2, 3]),
        instruction("V_ADD_F32_e32_vi", 0, &[1, 0]),
    ]
}

#[test]
fn exact_encoding_checks_preserve_recurrence_dataflow_and_inert_artifact() {
    let artifact = analyze(&recurrence()).unwrap();
    assert_eq!(artifact.multiply_offset(), 12);
    assert_eq!(artifact.add_offset(), 16);
    assert_eq!(artifact.product_register(), 1);
    assert_eq!(artifact.accumulator_register(), 0);
    assert_eq!(artifact.result_register(), 0);
    assert_eq!(artifact.product_source_operand_index(), 0);
    assert_eq!(artifact.accumulator_source_operand_index(), 1);
    assert!(artifact.validates_separate_recurrence_step_dataflow_shape());
    assert!(!artifact.establishes_gfx942_instruction_semantics());
    assert!(!artifact.establishes_machine_loop_recurrence());
    assert!(!artifact.establishes_compiler_refinement());
    assert!(!artifact.grants_worker_v3_refinement_authority());
    assert!(!artifact.grants_load_or_launch_authority());
    assert_eq!(
        Gfx942ScalarF32RecurrenceStepArtifactV1::decode_canonical(artifact.canonical_bytes())
            .unwrap(),
        artifact
    );
    assert_eq!(
        analyze(&recurrence()).unwrap().canonical_bytes(),
        artifact.canonical_bytes()
    );

    let mut body = recurrence();
    body[0] = instruction("V_MOV_B32_e32", 0, &[0]);
    body[1] = instruction("V_MUL_F32_e32_vi", 1, &[1, 1]);
    analyze(&body).unwrap();
    body[0].encoding[0] ^= 1;
    assert!(matches!(
        analyze(&body),
        Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::UnsupportedStepInputDefinition {
                offset: 8
            }
        )
    ));

    let mut body = recurrence();
    body[2] = instruction("V_ADD_F32_e32_vi", 0, &[4, 0]);
    assert!(matches!(
        analyze(&body),
        Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductDoesNotReachAdd)
    ));
    body[2] = instruction("V_ADD_F32_e32_vi", 0, &[0, 1]);
    assert!(matches!(
        analyze(&body),
        Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductOperandPosition { actual: 1 })
    ));
    body[2] = instruction("V_ADD_F32_e32_vi", 7, &[1, 0]);
    assert!(matches!(
        analyze(&body),
        Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ResultDoesNotUpdateAccumulator {
                result: 7,
                accumulator: 0
            }
        )
    ));
    let mut body = recurrence();
    body.swap(1, 2);
    assert!(matches!(
        analyze(&body),
        Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::MultiplyDoesNotDominateAdd)
    ));
    let mut body = recurrence();
    body[0].opcode = "V_FMAMK_F32_gfx940";
    assert!(matches!(
        analyze(&body),
        Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::FusedDefinitionReachesAdd { offset: 8 })
    ));
    body.insert(1, instruction("V_MOV_B32_e32_vi", 0, &[0]));
    assert!(matches!(
        analyze(&body),
        Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::FusedDefinitionReachesAdd { offset: 8 })
    ));
    let mut body = recurrence();
    body[1].opcode = "V_FMAC_F32_e64_vi";
    assert!(matches!(
        analyze(&body),
        Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::WrongMultiplyCount { actual: 0 })
    ));
}
