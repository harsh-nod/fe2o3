use super::*;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

mod trace_api {
    pub use crate::*;
}
mod trace_fixture;
use trace_fixture::TraceFixture;

fn selected(destination: u8, left: u8, right: u8, literal: u32) -> TraceFixture {
    let mut fixture = TraceFixture::registers(destination, left, right);
    for (index, selector) in [left, right].into_iter().enumerate() {
        fixture.operands[index + 1].0 = match selector {
            0..=101 => PhysicalMachineOperandValueV1::Register(format!("SGPR{selector}")),
            128..=192 => PhysicalMachineOperandValueV1::SignedImmediate(i64::from(selector - 128)),
            193..=208 => PhysicalMachineOperandValueV1::SignedImmediate(192 - i64::from(selector)),
            255 => PhysicalMachineOperandValueV1::SignedImmediate(i64::from(literal)),
            _ => PhysicalMachineOperandValueV1::SignedImmediate(0),
        };
    }
    if left == 255 || right == 255 {
        fixture.encoding.extend_from_slice(&literal.to_le_bytes());
    }
    fixture
}

#[test]
fn closed_source_and_destination_encodings_are_exhaustive() {
    for destination in 0..=127 {
        let trace = TraceFixture::registers(destination, 0, 1).instruction();
        let result = Gfx942SAddU32V1::decode(&trace);
        if destination <= 101 {
            let decoded = result.unwrap();
            assert_eq!(decoded.destination(), destination);
            assert_eq!(
                decoded.sources(),
                [Gfx942U32SourceV1::Sgpr(0), Gfx942U32SourceV1::Sgpr(1)]
            );
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
        for (left, right) in [(selector, 0), (0, selector)] {
            let trace = selected(2, left, right, 0x8765_4321).instruction();
            let result = Gfx942SAddU32V1::decode(&trace);
            assert_eq!(
                result.is_ok(),
                selector <= 101 || (128..=208).contains(&selector) || selector == 255,
                "selector {selector}, left {left}: {result:?}"
            );
        }
    }
}

#[test]
fn integer_inline_and_shared_literal_values_are_exact() {
    for selector in 128..=208 {
        let instruction = selected(2, selector, selector, 0).instruction();
        let decoded = Gfx942SAddU32V1::decode(&instruction).unwrap();
        let expected = if selector <= 192 {
            u32::from(selector - 128)
        } else {
            (192_i32 - i32::from(selector)) as u32
        };
        assert_eq!(
            decoded.sources(),
            [Gfx942U32SourceV1::Constant(expected); 2]
        );
    }
    for literal in [0, 1, 64, 0x1234_5678, 0x8000_0000, u32::MAX] {
        for (left, right) in [(255, 0), (0, 255), (255, 255)] {
            let mut fixture = selected(2, left, right, literal);
            let decoded = Gfx942SAddU32V1::decode(&fixture.instruction()).unwrap();
            assert_eq!(decoded.encoding().len(), 8);
            for (index, selector) in [left, right].into_iter().enumerate() {
                if selector == 255 {
                    assert_eq!(
                        decoded.sources()[index],
                        Gfx942U32SourceV1::Constant(literal)
                    );
                    fixture.operands[index + 1].0 =
                        PhysicalMachineOperandValueV1::SignedImmediate(i64::from(literal as i32));
                }
            }
            assert_eq!(
                Gfx942SAddU32V1::decode(&fixture.instruction()).unwrap(),
                decoded
            );
        }
    }
}

#[test]
fn actual_opcode_bits_and_exact_literal_extent_are_required() {
    for bit in 23..32 {
        let mut fixture = TraceFixture::registers(2, 0, 1);
        let word = u32::from_le_bytes(fixture.encoding.clone().try_into().unwrap()) ^ (1 << bit);
        fixture.encoding = word.to_le_bytes().to_vec();
        assert_eq!(
            Gfx942SAddU32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::UnsupportedEncoding)
        );
    }
    for bytes in [
        vec![0, 1, 2],
        vec![0, 1, 2, 0x80, 0, 0, 0, 0],
        vec![0, 255, 2, 0x80],
        vec![0, 255, 2, 0x80, 1, 2, 3],
    ] {
        let mut fixture = TraceFixture::registers(2, 0, 1);
        fixture.encoding = bytes;
        assert_eq!(
            Gfx942SAddU32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::InvalidEncodingLength)
        );
    }
}

#[test]
fn trace_opcode_operand_and_implicit_effect_mutations_fail_closed() {
    use Gfx942IntegerSemanticsErrorV1 as E;
    let baseline = TraceFixture::registers(2, 0, 1);
    let mut mutations = vec![];
    let mut fixture = baseline.clone();
    fixture.opcode = "S_SUB_U32_vi".into();
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
    for index in 0..3 {
        let mut fixture = baseline.clone();
        fixture.operands[index].1 = Some(0);
        mutations.push((fixture, E::OperandShapeMismatch));
        for register in ["SGPR3", "SGPR00", "VGPR0", "SGPR0_SGPR1", "SCC"] {
            let mut fixture = baseline.clone();
            fixture.operands[index].0 = PhysicalMachineOperandValueV1::Register(register.into());
            mutations.push((fixture, E::OperandValueMismatch));
        }
    }
    let mut fixture = baseline.clone();
    fixture.implicit_definitions.clear();
    mutations.push((fixture, E::ImplicitEffectsMismatch));
    let mut fixture = baseline.clone();
    fixture.implicit_definitions = vec!["EXEC".into(), "SCC".into()];
    mutations.push((fixture, E::ImplicitEffectsMismatch));
    let mut fixture = baseline;
    fixture.implicit_uses = vec!["SCC".into()];
    mutations.push((fixture, E::ImplicitEffectsMismatch));
    for (fixture, expected) in mutations {
        assert_eq!(
            Gfx942SAddU32V1::decode(&fixture.instruction()),
            Err(expected)
        );
    }
}

#[test]
fn trace_immediates_cannot_truncate_or_change_operand_kind() {
    let fixture = selected(2, 0, 255, 0xffff_ffff);
    for value in [
        PhysicalMachineOperandValueV1::SignedImmediate(0),
        PhysicalMachineOperandValueV1::SignedImmediate(0x1_ffff_ffff),
        PhysicalMachineOperandValueV1::SignedImmediate(-0x1_0000_0001),
        PhysicalMachineOperandValueV1::SingleFloatImmediate(u32::MAX),
        PhysicalMachineOperandValueV1::DoubleFloatImmediate(u64::MAX),
        PhysicalMachineOperandValueV1::AbsoluteExpression(-1),
    ] {
        let mut wrong = fixture.clone();
        wrong.operands[2].0 = value;
        assert_eq!(
            Gfx942SAddU32V1::decode(&wrong.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::OperandValueMismatch)
        );
    }
}

#[test]
fn branch_memory_scheduling_and_trap_effects_are_rejected() {
    for flags in [8, 16, 32] {
        let mut fixture = TraceFixture::registers(2, 0, 1);
        fixture.flags = flags;
        assert_eq!(
            Gfx942SAddU32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::InstructionEffectsMismatch)
        );
    }
    let mut returning = TraceFixture::registers(2, 0, 1);
    returning.returning = true;
    returning.flags = 4;
    assert_eq!(
        Gfx942SAddU32V1::decode(&returning.instruction()),
        Err(Gfx942IntegerSemanticsErrorV1::InstructionEffectsMismatch)
    );
    for (memory, flags) in [(1, 1), (2, 2), (3, 3), (4, 1), (5, 2), (6, 3)] {
        let mut fixture = TraceFixture::registers(2, 0, 1);
        fixture.memory = memory;
        fixture.width = 4;
        fixture.flags = flags;
        assert_eq!(
            Gfx942SAddU32V1::decode(&fixture.instruction()),
            Err(Gfx942IntegerSemanticsErrorV1::InstructionEffectsMismatch)
        );
    }
}

#[test]
fn widened_add_value_and_carry_match_independent_overflowing_add() {
    let boundaries = [
        0,
        1,
        15,
        16,
        63,
        64,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_0000,
        u32::MAX - 1,
        u32::MAX,
    ];
    for left in boundaries {
        for right in boundaries {
            let expected = left.overflowing_add(right);
            assert_eq!(
                gfx942_add_u32_v1(left, right),
                Gfx942U32AddResultV1 {
                    value: expected.0,
                    scc: expected.1
                }
            );
        }
    }
    let mut random = 0xcafe_0123_u32;
    for _ in 0..4096 {
        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
        let right = random.rotate_left(17);
        let expected = random.overflowing_add(right);
        assert_eq!(
            gfx942_add_u32_v1(random, right),
            Gfx942U32AddResultV1 {
                value: expected.0,
                scc: expected.1
            }
        );
    }
}

#[test]
fn execution_is_alias_safe_and_preserves_every_other_projected_register() {
    for (destination, left, right) in [(0, 0, 1), (1, 0, 1), (0, 0, 0), (2, 0, 1), (101, 101, 101)]
    {
        let decoded = Gfx942SAddU32V1::decode(
            &TraceFixture::registers(destination, left, right).instruction(),
        )
        .unwrap();
        for prior_scc in [false, true] {
            for fill in [0, 1, u32::MAX] {
                let before = std::array::from_fn(|index| fill.wrapping_add(index as u32));
                let expected =
                    before[usize::from(left)].overflowing_add(before[usize::from(right)]);
                let mut state = Gfx942ScalarIntegerStateV1::new(before, prior_scc);
                let result = decoded.execute(&mut state);
                assert_eq!((result.value, result.scc), expected);
                assert_eq!(state.scc(), expected.1);
                for (index, &value) in state.registers().iter().enumerate() {
                    assert_eq!(
                        value,
                        if index == usize::from(destination) {
                            expected.0
                        } else {
                            before[index]
                        }
                    );
                }
                assert!(!decoded.grants_launch_authority());
                assert!(!decoded.establishes_compiler_refinement());
            }
        }
    }
}

#[test]
fn execution_reads_integer_constants_without_incoming_scc() {
    for (left, right, literal) in [(128, 192, 0), (193, 129, 0), (255, 255, 0x8000_0000)] {
        let decoded =
            Gfx942SAddU32V1::decode(&selected(100, left, right, literal).instruction()).unwrap();
        let values = decoded.sources().map(|source| match source {
            Gfx942U32SourceV1::Constant(value) => value,
            _ => unreachable!(),
        });
        for scc in [false, true] {
            let mut state =
                Gfx942ScalarIntegerStateV1::new([37; GFX942_ORDINARY_SGPR_COUNT_V1], scc);
            let expected = values[0].overflowing_add(values[1]);
            let result = decoded.execute(&mut state);
            assert_eq!((result.value, result.scc), expected);
            assert!(
                state
                    .registers()
                    .iter()
                    .enumerate()
                    .all(|(index, &value)| index == 100 || value == 37)
            );
        }
    }
}

fn mc(tool: &Path, disassemble: bool, input: &str) -> String {
    let mut command = Command::new(tool);
    command
        .env_clear()
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .args(["--triple=amdgcn-amd-amdhsa", "--mcpu=gfx942", "--show-inst"]);
    command.arg(if disassemble {
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

fn parse_disassembled_operand(text: &str) -> PhysicalMachineOperandValueV1 {
    if let Some(index) = text.strip_prefix('s') {
        return PhysicalMachineOperandValueV1::Register(format!(
            "SGPR{}",
            index.parse::<u8>().unwrap()
        ));
    }
    let value = if let Some(hex) = text.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).unwrap()
    } else {
        text.parse::<i64>().unwrap()
    };
    PhysicalMachineOperandValueV1::SignedImmediate(value)
}

fn parse_disassembled_instruction(
    output: &str,
) -> Vec<(PhysicalMachineOperandValueV1, Option<u16>)> {
    output
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("s_add_u32 ")
                .or_else(|| line.trim().strip_prefix("s_add_u32\t"))
        })
        .expect("native instruction text")
        .split("//")
        .next()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .split(',')
        .map(|operand| (parse_disassembled_operand(operand.trim()), None))
        .collect()
}

#[test]
fn native_disassembly_annotations_do_not_become_operands() {
    let expected = TraceFixture::registers(2, 0, 1).operands;
    for output in [
        "\ts_add_u32 s2, s0, s1 ; <MCInst #31919 S_ADD_U32_vi",
        "\ts_add_u32\ts2, s0, s1 // <MCInst #26921 S_ADD_U32_vi",
        "\ts_add_u32 s2, s0, s1\n; <MCInst #31919 S_ADD_U32_vi",
    ] {
        assert_eq!(parse_disassembled_instruction(output), expected);
    }
}

/// This is real LLVM-MC assembly/disassembly compatibility, not authenticated
/// production-worker custody. SCC/flags in the envelope remain synthetic facts.
#[test]
#[ignore = "explicit native LLVM-MC compatibility; set FE2O3_GFX942_LLVM_MC"]
fn native_llvm_mc_s_add_u32_compatibility_v1() {
    let tool = std::env::var_os("FE2O3_GFX942_LLVM_MC")
        .expect("explicit native test requires FE2O3_GFX942_LLVM_MC");
    let tool = Path::new(&tool);
    assert!(tool.is_absolute() && tool.is_file());
    let cases = [
        ("s_add_u32 s2, s0, s1", selected(2, 0, 1, 0)),
        ("s_add_u32 s0, s0, s1", selected(0, 0, 1, 0)),
        ("s_add_u32 s101, s101, s101", selected(101, 101, 101, 0)),
        ("s_add_u32 s2, s0, 1", selected(2, 0, 129, 0)),
        ("s_add_u32 s2, s0, -1", selected(2, 0, 193, 0)),
        ("s_add_u32 s2, -16, 64", selected(2, 208, 192, 0)),
        (
            "s_add_u32 s2, s0, 0x12345678",
            selected(2, 0, 255, 0x1234_5678),
        ),
        (
            "s_add_u32 s2, 0x12345678, 0x12345678",
            selected(2, 255, 255, 0x1234_5678),
        ),
    ];
    for (assembly, mut fixture) in cases {
        let assembled = mc(tool, false, &format!("{assembly}\n"));
        assert_eq!(assembled.matches("S_ADD_U32_vi").count(), 1);
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
        let decoded = mc(tool, true, &disassembly_input);
        assert_eq!(decoded.matches("S_ADD_U32_vi").count(), 1);
        let operands = parse_disassembled_instruction(&decoded);
        assert_eq!(operands, fixture.operands);
        fixture.encoding = bytes;
        fixture.operands = operands;
        let model = Gfx942SAddU32V1::decode(&fixture.instruction()).unwrap();
        assert_eq!(model.encoding(), fixture.encoding);
    }
}
