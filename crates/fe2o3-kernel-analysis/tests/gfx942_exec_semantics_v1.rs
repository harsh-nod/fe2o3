#![cfg(feature = "authenticated-machine-effect")]

use fe2o3_kernel_analysis::{
    Gfx942ExecExecutionErrorV1 as Failure, Gfx942ExecLimitsV1, Gfx942ExecSliceV1,
    Gfx942ExecStateV1, Gfx942RegisterUnitV1, MAX_GFX942_EXEC_OBSERVATIONS_V1,
    MAX_GFX942_EXEC_STEPS_V1, MAX_GFX942_EXEC_TRACE_INSTRUCTIONS_V1,
    PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1,
    PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1, PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1,
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineEffectBudgetV1,
    PhysicalMachineEffectEntryRequestV1, PhysicalMachineEffectEvidenceV1,
    PhysicalMachineEffectRequestV1, PhysicalMachineExecutionChallengeV1,
    PhysicalMachineToolchainIdentityV1, PhysicalMachineTraceEvidenceV1,
    execute_gfx942_exec_slice_v1,
};

const FUNCTION: &str = "arbitrary_control_function";
const START: u64 = 8;

// Test-only wire builders. These records are decoded by the existing canonical trace reader;
// they do not represent an authenticated worker, production artifact, or executable authority.
#[derive(Clone)]
enum Operand {
    Register(&'static str),
    Immediate(i64),
}

#[derive(Clone)]
struct Instruction {
    opcode: &'static str,
    word: u32,
    extra_word: Option<u32>,
    definitions: u16,
    operands: Vec<Operand>,
    implicit_definitions: Vec<&'static str>,
    implicit_uses: Vec<&'static str>,
    branch: u8,
    flags: u16,
    target_override: Option<u64>,
    tied_operand: Option<(usize, u16)>,
}

fn register(code: u8) -> &'static str {
    match code {
        0 => "SGPR0_SGPR1",
        2 => "SGPR2_SGPR3",
        4 => "SGPR4_SGPR5",
        6 => "SGPR6_SGPR7",
        100 => "SGPR100_SGPR101",
        106 => "VCC",
        126 => "EXEC",
        _ => panic!("unsupported test register"),
    }
}

fn source(code: u8) -> Operand {
    match code {
        128..=192 => Operand::Immediate(i64::from(code) - 128),
        193..=208 => Operand::Immediate(192 - i64::from(code)),
        _ => Operand::Register(register(code)),
    }
}

fn mask(opcode: &'static str, destination: u8, left: u8, right: u8) -> Instruction {
    let (base, count, saved) = match opcode {
        "S_MOV_B64_vi" => (0xbe80_0100, 1, false),
        "S_AND_SAVEEXEC_B64_vi" => (0xbe80_2000, 1, true),
        "S_ANDN2_SAVEEXEC_B64_vi" => (0xbe80_2300, 1, true),
        "S_AND_B64_vi" => (0x8680_0000, 2, false),
        "S_OR_B64_vi" => (0x8780_0000, 2, false),
        "S_XOR_B64_vi" => (0x8880_0000, 2, false),
        "S_ANDN2_B64_vi" => (0x8980_0000, 2, false),
        _ => panic!("unsupported test mask"),
    };
    let mut operands = vec![Operand::Register(register(destination)), source(left)];
    if count == 2 {
        operands.push(source(right));
    }
    Instruction {
        opcode,
        word: base
            | (u32::from(destination) << 16)
            | u32::from(left)
            | if count == 2 { u32::from(right) << 8 } else { 0 },
        extra_word: None,
        definitions: 1,
        operands,
        implicit_definitions: if saved {
            vec!["EXEC", "SCC"]
        } else if opcode == "S_MOV_B64_vi" {
            vec![]
        } else {
            vec!["SCC"]
        },
        implicit_uses: if saved { vec!["EXEC"] } else { vec![] },
        branch: 0,
        flags: 0,
        target_override: None,
        tied_operand: None,
    }
}

fn branch(opcode: &'static str, displacement: i16) -> Instruction {
    let (base, kind) = match opcode {
        "S_BRANCH_vi" => (0xbf82_0000, 2),
        "S_CBRANCH_EXECZ_vi" => (0xbf88_0000, 1),
        "S_CBRANCH_EXECNZ_vi" => (0xbf89_0000, 1),
        _ => panic!("unsupported test branch"),
    };
    Instruction {
        opcode,
        word: base | u32::from(displacement as u16),
        extra_word: None,
        definitions: 0,
        operands: vec![Operand::Immediate(i64::from(displacement))],
        implicit_definitions: vec![],
        implicit_uses: if kind == 1 { vec!["EXEC"] } else { vec![] },
        branch: kind,
        flags: if kind == 1 { 4 } else { 12 },
        target_override: None,
        tied_operand: None,
    }
}

fn end() -> Instruction {
    Instruction {
        opcode: "S_ENDPGM_vi",
        word: 0xbf81_0000,
        extra_word: None,
        definitions: 0,
        operands: vec![Operand::Immediate(0)],
        implicit_definitions: vec![],
        implicit_uses: vec![],
        branch: 4,
        flags: 12,
        target_override: None,
        tied_operand: None,
    }
}

fn push16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn push32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn push64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn text(output: &mut Vec<u8>, value: &str) {
    push16(output, value.len() as u16);
    output.extend_from_slice(value.as_bytes());
}
fn finish_length(output: &mut [u8], domain_len: usize) {
    let length = output.len() as u32;
    output[domain_len..domain_len + 4].copy_from_slice(&length.to_le_bytes());
}

fn trace(body: &[Instruction]) -> PhysicalMachineTraceEvidenceV1 {
    let mut instructions = body.to_vec();
    instructions.push(end());
    let mut payload = vec![0; START as usize];
    let mut offsets = Vec::new();
    for instruction in &instructions {
        offsets.push(payload.len() as u64);
        payload.extend_from_slice(&instruction.word.to_le_bytes());
        if let Some(extra) = instruction.extra_word {
            payload.extend_from_slice(&extra.to_le_bytes());
        }
    }
    let code_size = payload.len() as u64 - START;
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
    push32(&mut effects, 0);
    push16(&mut effects, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1);
    effects.extend_from_slice(&request.execution_challenge().as_bytes());
    effects.extend_from_slice(&request.identity().sha256());
    push64(&mut effects, request.identity().byte_len());
    effects.extend_from_slice(&request.payload_identity().sha256());
    push64(&mut effects, request.payload_identity().byte_len());
    effects.extend_from_slice(&request.analyzer_identity().as_bytes());
    effects.extend_from_slice(&request.toolchain_identity().as_bytes());
    push16(&mut effects, 1);
    push16(&mut effects, 1);
    text(&mut effects, FUNCTION);
    effects.extend_from_slice(&[4; 32]);
    push64(&mut effects, START);
    push64(&mut effects, code_size);
    push32(&mut effects, 1);
    text(&mut effects, FUNCTION);
    push64(&mut effects, START);
    push64(&mut effects, code_size);
    push16(&mut effects, 0);
    push32(&mut effects, 1);
    text(&mut effects, FUNCTION);
    text(&mut effects, FUNCTION);
    push64(&mut effects, *offsets.last().unwrap());
    effects.push(4);
    push16(&mut effects, 0);
    finish_length(
        &mut effects,
        PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len(),
    );
    let effects =
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &effects).unwrap();

    let mut trace = PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1.to_vec();
    push32(&mut trace, 0);
    push16(&mut trace, PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1);
    trace.extend_from_slice(&request.execution_challenge().as_bytes());
    trace.extend_from_slice(&request.identity().sha256());
    push64(&mut trace, request.identity().byte_len());
    trace.extend_from_slice(&effects.identity().sha256());
    push64(&mut trace, effects.identity().byte_len());
    trace.extend_from_slice(&request.payload_identity().sha256());
    push64(&mut trace, request.payload_identity().byte_len());
    trace.extend_from_slice(&request.analyzer_identity().as_bytes());
    trace.extend_from_slice(&request.toolchain_identity().as_bytes());
    push16(&mut trace, 1);
    push32(&mut trace, instructions.len() as u32);
    for (index, instruction) in instructions.iter().enumerate() {
        let offset = offsets[index];
        let mut successors = match instruction.branch {
            0 => vec![index as u32 + 1],
            1 | 2 => {
                let target = instruction.target_override.unwrap_or_else(|| {
                    offset
                        .checked_add_signed(i64::from(instruction.word as u16 as i16) * 4 + 4)
                        .unwrap()
                });
                let mut successors = vec![offsets.binary_search(&target).unwrap() as u32];
                if instruction.branch == 1 {
                    successors.push(index as u32 + 1);
                }
                successors
            }
            4 => vec![],
            _ => unreachable!(),
        };
        successors.sort_unstable();
        successors.dedup();
        text(&mut trace, FUNCTION);
        push32(&mut trace, index as u32);
        push64(&mut trace, offset);
        push32(&mut trace, 1);
        push16(&mut trace, successors.len() as u16);
        for successor in successors {
            push32(&mut trace, successor);
        }
    }
    push32(&mut trace, instructions.len() as u32);
    for (index, instruction) in instructions.iter().enumerate() {
        let offset = offsets[index];
        text(&mut trace, FUNCTION);
        push64(&mut trace, offset);
        push32(&mut trace, index as u32);
        text(&mut trace, instruction.opcode);
        push16(
            &mut trace,
            if instruction.extra_word.is_some() {
                8
            } else {
                4
            },
        );
        trace.extend_from_slice(&instruction.word.to_le_bytes());
        if let Some(extra) = instruction.extra_word {
            trace.extend_from_slice(&extra.to_le_bytes());
        }
        push16(&mut trace, instruction.definitions);
        push16(&mut trace, instruction.operands.len() as u16);
        for (operand_index, operand) in instruction.operands.iter().enumerate() {
            trace.push(match operand {
                Operand::Register(_) => 1,
                Operand::Immediate(_) => 2,
            });
            push16(
                &mut trace,
                instruction
                    .tied_operand
                    .filter(|(index, _)| *index == operand_index)
                    .map(|(_, target)| target)
                    .unwrap_or(u16::MAX),
            );
            match operand {
                Operand::Register(name) => text(&mut trace, name),
                Operand::Immediate(value) => push64(&mut trace, *value as u64),
            }
        }
        for names in [
            &instruction.implicit_definitions,
            &instruction.implicit_uses,
        ] {
            push16(&mut trace, names.len() as u16);
            for name in names {
                text(&mut trace, name);
            }
        }
        trace.push(instruction.branch);
        let target = if instruction.branch == 1 || instruction.branch == 2 {
            instruction.target_override.unwrap_or_else(|| {
                offset
                    .checked_add_signed(i64::from(instruction.word as u16 as i16) * 4 + 4)
                    .unwrap()
            })
        } else {
            0
        };
        push64(&mut trace, target);
        push16(&mut trace, instruction.flags);
        trace.push(0);
        push16(&mut trace, 0);
    }
    finish_length(&mut trace, PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1.len());
    PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &trace).unwrap()
}

fn run(
    body: &[Instruction],
    state: &Gfx942ExecStateV1,
) -> Result<fe2o3_kernel_analysis::Gfx942ExecObservationV1, Failure> {
    let trace = trace(body);
    let stop = trace.instructions().last().unwrap().instruction_offset();
    execute_gfx942_exec_slice_v1(
        &trace,
        Gfx942ExecSliceV1::new(FUNCTION, START, stop).unwrap(),
        state,
        Gfx942ExecLimitsV1::default(),
    )
}

fn pair(state: &mut Gfx942ExecStateV1, low: u16, value: u64) {
    state.set_sgpr(low, value as u32).unwrap();
    state.set_sgpr(low + 1, (value >> 32) as u32).unwrap();
}
fn get_pair(state: &Gfx942ExecStateV1, low: u16) -> u64 {
    u64::from(state.sgpr(low).unwrap()) | (u64::from(state.sgpr(low + 1).unwrap()) << 32)
}

#[test]
fn save_branch_restore_uses_exact_control_bytes_and_declared_live_ins() {
    let body = [
        mask("S_AND_SAVEEXEC_B64_vi", 4, 106, 0),
        branch("S_CBRANCH_EXECZ_vi", 1),
        mask("S_MOV_B64_vi", 0, 129, 0),
        mask("S_OR_B64_vi", 126, 126, 4),
    ];
    for (exec, vcc) in [
        (0, 0),
        (u64::MAX, 0),
        (u64::MAX, u64::MAX),
        (1, 1),
        (1 << 31, 1 << 31),
        (1 << 32, 1 << 32),
        (1 << 63, 1 << 63),
    ] {
        let mut state = Gfx942ExecStateV1::default();
        state.set_exec(exec);
        state.set_vcc(vcc);
        state.set_scc(false);
        let original = state.clone();
        let observed = run(&body, &state).unwrap();
        assert_eq!(state, original);
        assert_eq!(observed.initial_state(), &state);
        assert_eq!(get_pair(observed.final_state(), 4), exec);
        assert_eq!(observed.final_state().exec(), Some(exec));
        assert_eq!(observed.final_state().scc(), Some(exec != 0));
        assert_eq!(observed.steps().len(), if exec & vcc == 0 { 3 } else { 4 });
        assert_eq!(observed.steps()[0].exec_after, Some(exec & vcc));
        assert_eq!(observed.function_symbol(), FUNCTION);
        assert_eq!(observed.start_offset(), START);
        assert_eq!(observed.final_pc(), observed.stop_offset());
        assert_eq!(observed.trace_identity(), trace(&body).identity());
        assert_eq!(observed.authenticated_execution_identity(), None);
        assert!(observed.is_conditional_on_supplied_live_ins());
        assert!(!observed.establishes_kernel_entry_state());
        assert!(!observed.establishes_compiler_refinement());
        assert!(!observed.grants_runtime_authority());
    }
}

#[test]
fn each_mask_operation_obeys_its_own_scc_and_operand_order() {
    for (left, right) in [
        (0_u64, 0_u64),
        (u64::MAX, 0),
        (0, u64::MAX),
        (0x8000_0000_0000_0001, 0x0000_0001_ffff_0000),
    ] {
        for (opcode, result) in [
            ("S_AND_B64_vi", left & right),
            ("S_OR_B64_vi", left | right),
            ("S_XOR_B64_vi", left ^ right),
            ("S_ANDN2_B64_vi", left & !right),
        ] {
            let mut state = Gfx942ExecStateV1::default();
            pair(&mut state, 0, left);
            pair(&mut state, 2, right);
            state.set_scc(result == 0);
            let observed = run(&[mask(opcode, 0, 0, 2)], &state).unwrap();
            assert_eq!(get_pair(observed.final_state(), 0), result, "{opcode}");
            assert_eq!(observed.final_state().scc(), Some(result != 0), "{opcode}");
            assert_eq!(observed.final_state().exec(), None);
        }
    }
    for scc in [None, Some(false), Some(true)] {
        let mut state = Gfx942ExecStateV1::default();
        if let Some(scc) = scc {
            state.set_scc(scc);
        }
        let observed = run(&[mask("S_MOV_B64_vi", 100, 193, 0)], &state).unwrap();
        assert_eq!(get_pair(observed.final_state(), 100), u64::MAX);
        assert_eq!(observed.final_state().scc(), scc);
    }
}

#[test]
fn saveexec_reads_aliases_before_writing_and_andn2_complements_exec() {
    for opcode in ["S_AND_SAVEEXEC_B64_vi", "S_ANDN2_SAVEEXEC_B64_vi"] {
        let mut state = Gfx942ExecStateV1::default();
        state.set_exec(0b1100);
        pair(&mut state, 4, 0b1010);
        let observed = run(&[mask(opcode, 4, 4, 0)], &state).unwrap();
        assert_eq!(get_pair(observed.final_state(), 4), 0b1100);
        assert_eq!(
            observed.final_state().exec(),
            Some(if opcode == "S_AND_SAVEEXEC_B64_vi" {
                0b1000
            } else {
                0b0010
            })
        );
        assert_eq!(observed.final_state().scc(), Some(true));
    }
    let mut state = Gfx942ExecStateV1::default();
    state.set_exec(0b1010);
    let observed = run(&[mask("S_ANDN2_SAVEEXEC_B64_vi", 4, 126, 0)], &state).unwrap();
    assert_eq!(get_pair(observed.final_state(), 4), 0b1010);
    assert_eq!(observed.final_state().exec(), Some(0));
    assert_eq!(observed.final_state().scc(), Some(false));
}

#[test]
fn branch_polarity_changes_observable_execution_without_fabricating_equivalence() {
    for opcode in ["S_CBRANCH_EXECZ_vi", "S_CBRANCH_EXECNZ_vi"] {
        for exec in [0, 1, 1 << 63, u64::MAX] {
            let mut state = Gfx942ExecStateV1::default();
            state.set_exec(exec);
            state.set_scc(true);
            let observed = run(
                &[branch(opcode, 1), mask("S_MOV_B64_vi", 0, 129, 0)],
                &state,
            )
            .unwrap();
            let taken = if opcode == "S_CBRANCH_EXECZ_vi" {
                exec == 0
            } else {
                exec != 0
            };
            assert_eq!(
                observed.steps()[0].next_offset,
                if taken { START + 8 } else { START + 4 }
            );
            assert_eq!(
                observed.final_state().sgpr(0),
                if taken { None } else { Some(1) }
            );
            assert_eq!(observed.final_state().scc(), Some(true));
        }
    }
    let observed = run(&[branch("S_BRANCH_vi", 0)], &Gfx942ExecStateV1::default()).unwrap();
    assert_eq!(observed.steps().len(), 1);
    assert_eq!(observed.final_state().exec(), None);
}

#[test]
fn backward_branch_and_exact_one_short_resource_limits() {
    let body = [branch("S_CBRANCH_EXECZ_vi", -1)];
    let trace = trace(&body);
    let slice = Gfx942ExecSliceV1::new(FUNCTION, START, START + 4).unwrap();
    let mut state = Gfx942ExecStateV1::default();
    state.set_exec(1);
    assert_eq!(slice.function_symbol(), FUNCTION);
    assert_eq!(slice.byte_len(), 4);
    assert!(
        execute_gfx942_exec_slice_v1(
            &trace,
            slice,
            &state,
            Gfx942ExecLimitsV1::new(1, 2, 1).unwrap()
        )
        .is_ok()
    );
    for (limits, expected) in [
        (
            Gfx942ExecLimitsV1::new(0, 2, 1).unwrap(),
            Failure::StepLimit {
                completed: 0,
                maximum: 0,
            },
        ),
        (
            Gfx942ExecLimitsV1::new(1, 1, 1).unwrap(),
            Failure::TraceInstructionLimit {
                actual: 2,
                maximum: 1,
            },
        ),
        (
            Gfx942ExecLimitsV1::new(1, 2, 0).unwrap(),
            Failure::ObservationLimit {
                completed: 0,
                maximum: 0,
            },
        ),
    ] {
        assert_eq!(
            execute_gfx942_exec_slice_v1(&trace, slice, &state, limits),
            Err(expected)
        );
    }
    state.set_exec(0);
    assert_eq!(
        execute_gfx942_exec_slice_v1(
            &trace,
            slice,
            &state,
            Gfx942ExecLimitsV1::new(3, 2, 3).unwrap()
        ),
        Err(Failure::StepLimit {
            completed: 3,
            maximum: 3
        })
    );
    assert_eq!(
        execute_gfx942_exec_slice_v1(
            &trace,
            slice,
            &state,
            Gfx942ExecLimitsV1::new(4, 2, 3).unwrap()
        ),
        Err(Failure::ObservationLimit {
            completed: 3,
            maximum: 3
        })
    );
    for (steps, trace, observations) in [
        (MAX_GFX942_EXEC_STEPS_V1 + 1, 0, 0),
        (0, MAX_GFX942_EXEC_TRACE_INSTRUCTIONS_V1 + 1, 0),
        (0, 0, MAX_GFX942_EXEC_OBSERVATIONS_V1 + 1),
        (usize::MAX, usize::MAX, usize::MAX),
    ] {
        assert_eq!(
            Gfx942ExecLimitsV1::new(steps, trace, observations),
            Err(Failure::InvalidLimits)
        );
    }
}

#[test]
fn missing_live_ins_are_not_zero_and_fail_without_modifying_the_input() {
    let body = [mask("S_AND_SAVEEXEC_B64_vi", 4, 0, 0)];
    let mut state = Gfx942ExecStateV1::default();
    state.set_exec(0);
    state.set_sgpr(0, 0).unwrap();
    let original = state.clone();
    assert_eq!(
        run(&body, &state),
        Err(Failure::UndefinedRegister {
            offset: START,
            register: Gfx942RegisterUnitV1::Sgpr(1),
        })
    );
    assert_eq!(state, original);
    assert_eq!(
        run(
            &[branch("S_CBRANCH_EXECZ_vi", 0)],
            &Gfx942ExecStateV1::default()
        ),
        Err(Failure::UndefinedRegister {
            offset: START,
            register: Gfx942RegisterUnitV1::ExecLow
        })
    );
    assert_eq!(
        run(&[mask("S_MOV_B64_vi", 0, 106, 0)], &state),
        Err(Failure::UndefinedRegister {
            offset: START,
            register: Gfx942RegisterUnitV1::VccLow
        })
    );
    assert_eq!(state.set_sgpr(102, 0), Err(Failure::InvalidSgpr(102)));
}

#[test]
fn encoding_operands_and_implicit_effect_mutations_are_rejected() {
    let valid = mask("S_AND_SAVEEXEC_B64_vi", 4, 106, 0);
    let mut cases = Vec::new();
    let mut instruction = valid.clone();
    instruction.word ^= 1 << 8;
    cases.push((instruction, Failure::InvalidEncoding { offset: START }));
    let mut instruction = valid.clone();
    instruction.operands[1] = Operand::Register("EXEC");
    cases.push((instruction, Failure::InvalidOperands { offset: START }));
    let mut instruction = valid.clone();
    instruction.operands[0] = Operand::Register("SGPR3_SGPR4");
    cases.push((instruction, Failure::InvalidOperands { offset: START }));
    let mut instruction = valid.clone();
    instruction.operands[1] = Operand::Register("VGPR0_VGPR1");
    cases.push((instruction, Failure::InvalidOperands { offset: START }));
    let mut instruction = valid.clone();
    instruction.implicit_definitions = vec!["EXEC"];
    cases.push((instruction, Failure::InvalidEffects { offset: START }));
    let mut instruction = valid.clone();
    instruction.implicit_uses.clear();
    cases.push((instruction, Failure::InvalidEffects { offset: START }));
    let mut instruction = valid.clone();
    instruction.flags = 16;
    cases.push((instruction, Failure::InvalidEffects { offset: START }));
    let mut instruction = valid.clone();
    instruction.tied_operand = Some((1, 0));
    cases.push((instruction, Failure::InvalidOperands { offset: START }));
    let mut instruction = mask("S_MOV_B64_vi", 0, 128, 0);
    instruction.implicit_definitions.push("SCC");
    cases.push((instruction, Failure::InvalidEffects { offset: START }));
    let mut instruction = mask("S_MOV_B64_vi", 0, 128, 0);
    instruction.operands[1] = Operand::Immediate(65);
    cases.push((instruction, Failure::InvalidOperands { offset: START }));
    let mut instruction = branch("S_CBRANCH_EXECZ_vi", 0);
    instruction.word ^= 1 << 16;
    cases.push((instruction, Failure::InvalidEncoding { offset: START }));
    let mut instruction = branch("S_CBRANCH_EXECZ_vi", 0);
    instruction.operands[0] = Operand::Immediate(1);
    cases.push((instruction, Failure::InvalidOperands { offset: START }));
    let mut instruction = branch("S_CBRANCH_EXECZ_vi", -1);
    instruction.target_override = Some(START + 4);
    cases.push((instruction, Failure::InvalidBranchTarget { offset: START }));
    let mut instruction = branch("S_BRANCH_vi", 0);
    instruction.flags = 4;
    cases.push((instruction, Failure::InvalidEffects { offset: START }));
    for (instruction, expected) in cases {
        assert_eq!(
            run(&[instruction], &Gfx942ExecStateV1::default()),
            Err(expected)
        );
    }
}

#[test]
fn untaken_unsupported_instructions_and_out_of_slice_targets_fail_preflight() {
    let mut unsupported = mask("S_MOV_B64_vi", 0, 128, 0);
    unsupported.opcode = "S_UNKNOWN_B64_vi";
    let body = [branch("S_CBRANCH_EXECZ_vi", 1), unsupported];
    let mut state = Gfx942ExecStateV1::default();
    state.set_exec(0);
    assert_eq!(
        run(&body, &state),
        Err(Failure::UnsupportedInstruction { offset: START + 4 })
    );
    let body = [
        branch("S_CBRANCH_EXECZ_vi", 1),
        mask("S_MOV_B64_vi", 0, 128, 0),
    ];
    let trace = trace(&body);
    let slice = Gfx942ExecSliceV1::new(FUNCTION, START, START + 4).unwrap();
    assert_eq!(
        execute_gfx942_exec_slice_v1(&trace, slice, &state, Gfx942ExecLimitsV1::default()),
        Err(Failure::InvalidBranchTarget { offset: START })
    );
    let wrong = Gfx942ExecSliceV1::new("other_function", START, START + 4).unwrap();
    assert_eq!(
        execute_gfx942_exec_slice_v1(&trace, wrong, &state, Gfx942ExecLimitsV1::default()),
        Err(Failure::MissingBoundary { offset: START })
    );
    let wrong = Gfx942ExecSliceV1::new(FUNCTION, START, START + 12).unwrap();
    assert_eq!(
        execute_gfx942_exec_slice_v1(&trace, wrong, &state, Gfx942ExecLimitsV1::default()),
        Err(Failure::MissingBoundary { offset: START + 12 })
    );
    for (symbol, start, stop) in [
        ("", START, START + 4),
        (FUNCTION, START, START),
        (FUNCTION, START + 1, START + 4),
        (FUNCTION, START, START + 5),
    ] {
        assert_eq!(
            Gfx942ExecSliceV1::new(symbol, start, stop),
            Err(Failure::InvalidSlice)
        );
    }
}

#[test]
fn extended_encodings_and_interior_slice_boundaries_are_rejected() {
    let mut extended = mask("S_MOV_B64_vi", 0, 128, 0);
    extended.extra_word = Some(0x1234_5678);
    assert_eq!(
        run(&[extended.clone()], &Gfx942ExecStateV1::default()),
        Err(Failure::InvalidEncoding { offset: START })
    );
    let trace = trace(&[extended]);
    let inside = Gfx942ExecSliceV1::new(FUNCTION, START + 4, START + 8).unwrap();
    assert_eq!(
        execute_gfx942_exec_slice_v1(
            &trace,
            inside,
            &Gfx942ExecStateV1::default(),
            Gfx942ExecLimitsV1::default()
        ),
        Err(Failure::MissingBoundary { offset: START + 4 })
    );
}

#[test]
fn multi_step_exact_and_one_short_budgets_never_return_partial_success() {
    let body = [
        mask("S_MOV_B64_vi", 0, 192, 0),
        mask("S_MOV_B64_vi", 2, 208, 0),
        mask("S_XOR_B64_vi", 106, 0, 2),
    ];
    let trace = trace(&body);
    let slice = Gfx942ExecSliceV1::new(FUNCTION, START, START + 12).unwrap();
    let initial = Gfx942ExecStateV1::default();
    let result = execute_gfx942_exec_slice_v1(
        &trace,
        slice,
        &initial,
        Gfx942ExecLimitsV1::new(3, 4, 3).unwrap(),
    )
    .unwrap();
    assert_eq!(result.steps().len(), 3);
    assert_eq!(get_pair(result.final_state(), 0), 64);
    assert_eq!(get_pair(result.final_state(), 2), (-16_i64) as u64);
    assert_eq!(result.final_state().vcc(), Some(64 ^ ((-16_i64) as u64)));
    for (limits, error) in [
        (
            Gfx942ExecLimitsV1::new(2, 4, 3).unwrap(),
            Failure::StepLimit {
                completed: 2,
                maximum: 2,
            },
        ),
        (
            Gfx942ExecLimitsV1::new(3, 4, 2).unwrap(),
            Failure::ObservationLimit {
                completed: 2,
                maximum: 2,
            },
        ),
        (
            Gfx942ExecLimitsV1::new(3, 3, 3).unwrap(),
            Failure::TraceInstructionLimit {
                actual: 4,
                maximum: 3,
            },
        ),
    ] {
        assert_eq!(
            execute_gfx942_exec_slice_v1(&trace, slice, &initial, limits),
            Err(error)
        );
    }
    assert_eq!(initial, Gfx942ExecStateV1::default());
}

#[test]
fn destination_aliases_the_second_source_and_special_masks() {
    let mut initial = Gfx942ExecStateV1::default();
    pair(&mut initial, 0, 0xf000_0000_0000_000f);
    pair(&mut initial, 2, 0x5000_0000_0000_0005);
    initial.set_vcc(0xff);
    initial.set_exec(0x55);
    let result = run(
        &[
            mask("S_ANDN2_B64_vi", 2, 0, 2),
            mask("S_XOR_B64_vi", 126, 106, 126),
            mask("S_MOV_B64_vi", 106, 126, 0),
        ],
        &initial,
    )
    .unwrap();
    assert_eq!(get_pair(result.final_state(), 2), 0xa000_0000_0000_000a);
    assert_eq!(result.final_state().exec(), Some(0xaa));
    assert_eq!(result.final_state().vcc(), Some(0xaa));
    assert_eq!(result.final_state().scc(), Some(true));
}
