use super::*;
use crate::PhysicalMachineOperandValueV1;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

mod trace_api {
    pub use crate::*;
}
#[path = "../tests/trace_fixture.rs"]
mod trace_fixture;
use trace_fixture::TraceFixture;

fn selected(destination: u8, selector: u8, literal: u32) -> TraceFixture {
    let mut fixture = TraceFixture::registers(destination, selector, 0);
    let word = 0xbe80_0000 | (u32::from(destination) << 16) | u32::from(selector);
    fixture.encoding = word.to_le_bytes().to_vec();
    fixture.opcode = "S_MOV_B32_vi".into();
    fixture.operands.truncate(2);
    fixture.implicit_definitions.clear();
    fixture.operands[1].0 = match selector {
        0..=101 => PhysicalMachineOperandValueV1::Register(format!("SGPR{selector}")),
        128..=192 => PhysicalMachineOperandValueV1::SignedImmediate(i64::from(selector - 128)),
        193..=208 => PhysicalMachineOperandValueV1::SignedImmediate(192 - i64::from(selector)),
        255 => PhysicalMachineOperandValueV1::SignedImmediate(i64::from(literal)),
        _ => PhysicalMachineOperandValueV1::SignedImmediate(0),
    };
    if selector == 255 {
        fixture.encoding.extend_from_slice(&literal.to_le_bytes());
    }
    fixture
}

#[test]
fn mov_closed_destination_and_source_encodings_are_exhaustive() {
    for destination in 0..=127 {
        let trace = selected(destination, 1, 0).instruction();
        let result = Gfx942SMovB32V1::decode(&trace);
        if destination <= 101 {
            let decoded = result.unwrap();
            assert_eq!(decoded.destination(), destination);
            assert_eq!(decoded.source(), Gfx942U32SourceV1::Sgpr(1));
            assert_eq!(decoded.encoding(), trace.encoding());
        } else {
            assert_eq!(
                result,
                Err(Gfx942IntegerSemanticsErrorV1::UnsupportedDestination(
                    destination
                ))
            );
        }
    }
    for selector in 0..=255 {
        let result = Gfx942SMovB32V1::decode(&selected(2, selector, 0x8765_4321).instruction());
        if selector <= 101 || (128..=208).contains(&selector) || selector == 255 {
            assert!(result.is_ok(), "selector {selector}: {result:?}");
        } else {
            assert_eq!(
                result,
                Err(Gfx942IntegerSemanticsErrorV1::UnsupportedSource(selector))
            );
        }
    }
}

#[test]
fn mov_inline_and_literal_values_preserve_exact_u32_bits() {
    for selector in 128..=208 {
        let decoded = Gfx942SMovB32V1::decode(&selected(2, selector, 0).instruction()).unwrap();
        let expected = if selector <= 192 {
            u32::from(selector - 128)
        } else {
            (192_i32 - i32::from(selector)) as u32
        };
        assert_eq!(decoded.source(), Gfx942U32SourceV1::Constant(expected));
        assert_eq!(decoded.encoding().len(), 4);
    }
    for literal in [0, 1, 64, 0x1234_5678, 0x8000_0000, u32::MAX] {
        let mut fixture = selected(101, 255, literal);
        let decoded = Gfx942SMovB32V1::decode(&fixture.instruction()).unwrap();
        assert_eq!(decoded.source(), Gfx942U32SourceV1::Constant(literal));
        assert_eq!(decoded.encoding().len(), 8);
        fixture.operands[1].0 =
            PhysicalMachineOperandValueV1::SignedImmediate(i64::from(literal as i32));
        assert_eq!(
            Gfx942SMovB32V1::decode(&fixture.instruction()).unwrap(),
            decoded
        );
    }
}

#[test]
fn mov_opcode_bits_and_exact_literal_extent_are_required() {
    for bit in (8..16).chain(23..32) {
        let mut fixture = selected(2, 0, 0);
        let word = u32::from_le_bytes(fixture.encoding.clone().try_into().unwrap()) ^ (1 << bit);
        fixture.encoding = word.to_le_bytes().to_vec();
        assert_eq!(
            Gfx942SMovB32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::UnsupportedEncoding)
        );
    }
    for bytes in [
        vec![0, 0, 0x82],
        vec![0, 0, 0x82, 0xbe, 0, 0, 0, 0],
        vec![255, 0, 0x82, 0xbe],
        vec![255, 0, 0x82, 0xbe, 1, 2, 3],
        vec![255, 0, 0x82, 0xbe, 1, 2, 3, 4, 5, 6, 7, 8],
    ] {
        let mut fixture = selected(2, 0, 0);
        fixture.encoding = bytes;
        assert_eq!(
            Gfx942SMovB32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::InvalidEncodingLength)
        );
    }
}

#[test]
fn mov_trace_opcode_operands_and_implicit_effects_fail_closed() {
    use Gfx942IntegerSemanticsErrorV1 as E;
    let baseline = selected(2, 0, 0);
    let mut mutations = vec![];
    let mut fixture = baseline.clone();
    fixture.opcode = "S_MOV_B64_vi".into();
    mutations.push((fixture, E::OpcodeMismatch));
    for definitions in [0, 2] {
        let mut fixture = baseline.clone();
        fixture.definitions = definitions;
        mutations.push((fixture, E::OperandShapeMismatch));
    }
    let mut fixture = baseline.clone();
    fixture.operands.pop();
    mutations.push((fixture, E::OperandShapeMismatch));
    let mut fixture = baseline.clone();
    fixture
        .operands
        .push((PhysicalMachineOperandValueV1::SignedImmediate(0), None));
    mutations.push((fixture, E::OperandShapeMismatch));
    for index in 0..2 {
        let mut fixture = baseline.clone();
        fixture.operands[index].1 = Some(0);
        mutations.push((fixture, E::OperandShapeMismatch));
        for register in ["SGPR3", "SGPR00", "VGPR0", "SGPR0_SGPR1", "SCC", "EXEC"] {
            let mut fixture = baseline.clone();
            fixture.operands[index].0 = PhysicalMachineOperandValueV1::Register(register.into());
            mutations.push((fixture, E::OperandValueMismatch));
        }
    }
    let mut fixture = baseline.clone();
    fixture.operands[1].0 = PhysicalMachineOperandValueV1::SignedImmediate(0);
    mutations.push((fixture, E::OperandValueMismatch));
    for register in ["SCC", "EXEC", "SGPR0"] {
        let mut fixture = baseline.clone();
        fixture.implicit_definitions = vec![register.into()];
        mutations.push((fixture, E::ImplicitEffectsMismatch));
        let mut fixture = baseline.clone();
        fixture.implicit_uses = vec![register.into()];
        mutations.push((fixture, E::ImplicitEffectsMismatch));
    }
    for (fixture, expected) in mutations {
        assert_eq!(
            Gfx942SMovB32V1::decode(&fixture.instruction()),
            Err(expected)
        );
    }
}

#[test]
fn mov_immediates_cannot_truncate_or_change_operand_kind() {
    let baseline = selected(2, 255, u32::MAX);
    for value in [
        PhysicalMachineOperandValueV1::SignedImmediate(0),
        PhysicalMachineOperandValueV1::SignedImmediate(0x1_ffff_ffff),
        PhysicalMachineOperandValueV1::SignedImmediate(-0x1_0000_0001),
        PhysicalMachineOperandValueV1::SingleFloatImmediate(u32::MAX),
        PhysicalMachineOperandValueV1::DoubleFloatImmediate(u64::MAX),
        PhysicalMachineOperandValueV1::AbsoluteExpression(-1),
        PhysicalMachineOperandValueV1::Register("SGPR1".into()),
    ] {
        let mut fixture = baseline.clone();
        fixture.operands[1].0 = value;
        assert_eq!(
            Gfx942SMovB32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::OperandValueMismatch)
        );
    }
}

#[test]
fn mov_branch_memory_scheduling_and_trap_effects_are_rejected() {
    for flags in [8, 16, 32] {
        let mut fixture = selected(2, 0, 0);
        fixture.flags = flags;
        assert_eq!(
            Gfx942SMovB32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::InstructionEffectsMismatch)
        );
    }
    let mut returning = selected(2, 0, 0);
    returning.returning = true;
    returning.flags = 4;
    assert_eq!(
        Gfx942SMovB32V1::decode(&returning.instruction()),
        Err(Gfx942IntegerSemanticsErrorV1::InstructionEffectsMismatch)
    );
    for (memory, flags) in [(1, 1), (2, 2), (3, 3), (4, 1), (5, 2), (6, 3)] {
        let mut fixture = selected(2, 0, 0);
        fixture.memory = memory;
        fixture.width = 4;
        fixture.flags = flags;
        assert_eq!(
            Gfx942SMovB32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::InstructionEffectsMismatch)
        );
    }
}

#[test]
fn mov_register_execution_preserves_full_frame_and_scc_for_all_aliases() {
    let before =
        std::array::from_fn(|index| (index as u32).wrapping_mul(0x9e37_79b9) ^ 0xcafe_babe);
    for destination in 0..GFX942_ORDINARY_SGPR_COUNT_V1 as u8 {
        for source in 0..GFX942_ORDINARY_SGPR_COUNT_V1 as u8 {
            let decoded =
                Gfx942SMovB32V1::decode(&selected(destination, source, 0).instruction()).unwrap();
            let original_instruction = decoded.clone();
            let expected_value = before[source as usize];
            let mut expected = before;
            expected[destination as usize] = expected_value;
            for scc in [false, true] {
                let mut state = Gfx942ScalarIntegerStateV1::new(before, scc);
                assert_eq!(decoded.execute(&mut state), expected_value);
                assert_eq!(state.registers(), &expected);
                assert_eq!(state.scc(), scc);
                assert_eq!(decoded, original_instruction);
                assert!(!decoded.grants_launch_authority());
                assert!(!decoded.establishes_compiler_refinement());
            }
        }
    }
}

#[test]
fn mov_literal_execution_preserves_full_frame_and_scc_for_every_destination() {
    let before = std::array::from_fn(|index| u32::MAX - index as u32);
    for destination in 0..GFX942_ORDINARY_SGPR_COUNT_V1 as u8 {
        for value in [0, 1, 64, 0xffff_fff0, 0x8000_0000, u32::MAX] {
            let decoded =
                Gfx942SMovB32V1::decode(&selected(destination, 255, value).instruction()).unwrap();
            let mut expected = before;
            expected[destination as usize] = value;
            for scc in [false, true] {
                let mut state = Gfx942ScalarIntegerStateV1::new(before, scc);
                assert_eq!(decoded.execute(&mut state), value);
                assert_eq!(state.registers(), &expected);
                assert_eq!(state.scc(), scc);
            }
        }
    }
}

fn mc(tool: &Path, disassemble: bool, input: &str) -> String {
    let mut command = Command::new(tool);
    command
        .env_clear()
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .args(["--triple=amdgcn-amd-amdhsa", "--mcpu=gfx942", "--show-inst"])
        .arg(if disassemble {
            "--disassemble"
        } else {
            "--show-encoding"
        });
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("configured native llvm-mc must start");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    eprintln!(
        "llvm-mc {tool:?} disassemble={disassemble}\ninput:\n{input}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "native llvm-mc failed: {:?}",
        output.status
    );
    assert!(output.stderr.is_empty(), "unexpected LLVM diagnostic");
    String::from_utf8(output.stdout).unwrap()
}

fn parse_disassembled_instruction(
    output: &str,
) -> Vec<(PhysicalMachineOperandValueV1, Option<u16>)> {
    output
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("s_mov_b32 ")
                .or_else(|| line.trim().strip_prefix("s_mov_b32\t"))
        })
        .expect("native instruction text")
        .split("//")
        .next()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .split(',')
        .map(|operand| {
            let operand = operand.trim();
            let value = if let Some(index) = operand.strip_prefix('s') {
                PhysicalMachineOperandValueV1::Register(format!(
                    "SGPR{}",
                    index.parse::<u8>().unwrap()
                ))
            } else {
                PhysicalMachineOperandValueV1::SignedImmediate(
                    if let Some(hex) = operand.strip_prefix("0x") {
                        i64::from_str_radix(hex, 16).unwrap()
                    } else {
                        operand.parse::<i64>().unwrap()
                    },
                )
            };
            (value, None)
        })
        .collect()
}

#[test]
fn mov_native_disassembly_annotations_do_not_become_operands() {
    let expected = selected(2, 0, 0).operands;
    for text in [
        "\ts_mov_b32 s2, s0 ; <MCInst #28612 S_MOV_B32_vi",
        "\ts_mov_b32\ts2, s0 // <MCInst #33617 S_MOV_B32_vi",
        "\ts_mov_b32 s2, s0\n; <MCInst #28612 S_MOV_B32_vi",
    ] {
        assert_eq!(parse_disassembled_instruction(text), expected);
    }
}

/// Real LLVM-MC assembly/disassembly compatibility, not authenticated worker
/// custody. Implicit effects and flags in this trace envelope are synthetic.
#[test]
#[ignore = "explicit native LLVM-MC compatibility; set FE2O3_GFX942_LLVM_MC"]
fn native_llvm_mc_s_mov_b32_compatibility_v1() {
    let tool = std::env::var_os("FE2O3_GFX942_LLVM_MC")
        .expect("explicit native test requires FE2O3_GFX942_LLVM_MC");
    let tool = Path::new(&tool);
    assert!(tool.is_absolute() && tool.is_file());
    for (assembly, mut fixture) in [
        ("s_mov_b32 s2, s0", selected(2, 0, 0)),
        ("s_mov_b32 s0, s0", selected(0, 0, 0)),
        ("s_mov_b32 s101, s101", selected(101, 101, 0)),
        ("s_mov_b32 s2, 0", selected(2, 128, 0)),
        ("s_mov_b32 s2, 64", selected(2, 192, 0)),
        ("s_mov_b32 s2, -1", selected(2, 193, 0)),
        ("s_mov_b32 s2, -16", selected(2, 208, 0)),
        ("s_mov_b32 s2, 0x12345678", selected(2, 255, 0x1234_5678)),
        ("s_mov_b32 s2, 0xffffffef", selected(2, 255, 0xffff_ffef)),
    ] {
        let assembled = mc(tool, false, &format!("{assembly}\n"));
        assert_eq!(assembled.matches("S_MOV_B32_vi").count(), 1);
        let encoded = assembled
            .split("encoding: [")
            .nth(1)
            .expect("native encoding output")
            .split(']')
            .next()
            .unwrap();
        let bytes = encoded
            .split(',')
            .map(|byte| u8::from_str_radix(byte.trim().strip_prefix("0x").unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(bytes, fixture.encoding);
        let disassembly_input = bytes
            .iter()
            .map(|byte| format!("0x{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
            + "\n";
        let disassembled = mc(tool, true, &disassembly_input);
        assert_eq!(disassembled.matches("S_MOV_B32_vi").count(), 1);
        let operands = parse_disassembled_instruction(&disassembled);
        assert_eq!(operands, fixture.operands);
        fixture.encoding = bytes;
        fixture.operands = operands;
        let model = Gfx942SMovB32V1::decode(&fixture.instruction()).unwrap();
        assert_eq!(model.encoding(), fixture.encoding);
        let before = std::array::from_fn(|index| (index as u32).wrapping_mul(0x9e37_79b9));
        let expected_value = match model.source() {
            Gfx942U32SourceV1::Sgpr(index) => before[index as usize],
            Gfx942U32SourceV1::Constant(value) => value,
        };
        let mut expected = before;
        expected[model.destination() as usize] = expected_value;
        for scc in [false, true] {
            let mut state = Gfx942ScalarIntegerStateV1::new(before, scc);
            assert_eq!(model.execute(&mut state), expected_value);
            assert_eq!(state.registers(), &expected);
            assert_eq!(state.scc(), scc);
        }
    }
}
