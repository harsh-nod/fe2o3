use super::*;
mod trace_api {
    pub use crate::*;
}
#[path = "trace_fixture.rs"]
mod trace_fixture;
use trace_fixture::{Fixture, Instruction};

fn span(fixture: &Fixture, first: u64, last: u64) -> Gfx942MovPrefixAddU32V1 {
    let (_, analysis) = fixture.analysis();
    Gfx942MovPrefixAddU32V1::decode(analysis.trace(), &fixture.function, first, last).unwrap()
}

#[test]
fn zero_and_maximal_prefix_execute_actual_moves_then_add() {
    for count in [0, 1, 63] {
        let mut instructions = vec![Instruction::mov(5, 36); count];
        instructions.push(Instruction::add(0, if count == 0 { 36 } else { 5 }, 129));
        let fixture = Fixture::new(instructions);
        let model = span(&fixture, 4, 4 + 4 * count as u64);
        assert_eq!(model.moves().len(), count);
        assert_eq!(model.end_offset(), 8 + 4 * count as u64);
        assert_eq!(
            model.terminal_origins(),
            [
                Gfx942U32OriginV1::EntrySgpr(36),
                Gfx942U32OriginV1::Constant(1)
            ]
        );
        for initial in [0, 1, u32::MAX] {
            for scc in [false, true] {
                let mut expected = std::array::from_fn(|index| index as u32 * 73);
                expected[36] = initial;
                let mut state = Gfx942ScalarIntegerStateV1::new(expected, scc);
                let result = model.execute(&mut state);
                if count != 0 {
                    expected[5] = initial;
                }
                expected[0] = initial.wrapping_add(1);
                assert_eq!(result.value, expected[0]);
                assert_eq!(result.scc, initial == u32::MAX);
                assert_eq!(state.registers(), &expected);
                assert_eq!(state.scc(), result.scc);
            }
        }
        assert!(!model.grants_launch_authority());
        assert!(!model.establishes_compiler_refinement());
    }
}

#[test]
fn prefix_origins_snapshot_aliases_order_and_literal_words() {
    let fixture = Fixture::new(vec![
        Instruction::mov(5, 36),
        Instruction::mov(36, 2),
        Instruction::selected(true, 7, 255, 0, 0xffff_ffff),
        Instruction::mov(7, 7),
        Instruction::add(5, 7, 5),
    ]);
    let model = span(&fixture, 4, 24);
    assert_eq!(model.end_offset(), 28);
    assert_eq!(
        model.terminal_origins(),
        [
            Gfx942U32OriginV1::Constant(u32::MAX),
            Gfx942U32OriginV1::EntrySgpr(36)
        ]
    );
    let mut expected = std::array::from_fn(|index| index as u32 + 1);
    let old = expected;
    let mut state = Gfx942ScalarIntegerStateV1::new(old, false);
    let result = model.execute(&mut state);
    expected[36] = old[2];
    expected[7] = u32::MAX;
    expected[5] = old[36] - 1;
    assert_eq!(result.value, old[36] - 1);
    assert!(result.scc);
    assert_eq!(state.registers(), &expected);
}

#[test]
fn span_boundaries_and_block_membership_fail_closed() {
    let mut fixture = Fixture::new(vec![
        Instruction::selected(true, 5, 255, 0, 0x1234_5678),
        Instruction::add(0, 36, 5),
    ]);
    let (_, analysis) = fixture.analysis();
    for (first, last) in [
        (5, 12),
        (8, 12),
        (4, 8),
        (4, 16),
        (12, 4),
        (0, 12),
        (20, 20),
    ] {
        assert!(
            Gfx942MovPrefixAddU32V1::decode(analysis.trace(), &fixture.function, first, last)
                .is_err()
        );
    }
    assert!(Gfx942MovPrefixAddU32V1::decode(analysis.trace(), "other", 4, 12).is_err());
    fixture.split_at = Some(1);
    let (_, analysis) = fixture.analysis();
    assert_eq!(
        Gfx942MovPrefixAddU32V1::decode(analysis.trace(), &fixture.function, 4, 12),
        Err(Gfx942MovPrefixAddErrorV1::CrossBlock)
    );
}

#[test]
fn unsupported_or_extra_prefix_operations_are_never_skipped() {
    for middle in [
        Instruction::add(5, 36, 129),
        Instruction {
            encoding: vec![0, 0, 0x80, 0xbf],
            opcode: "S_NOP_vi".into(),
            destination: None,
            sources: vec![trace_api::PhysicalMachineOperandValueV1::SignedImmediate(0)],
            scc: false,
        },
        Instruction {
            opcode: "S_MOV_B64_vi".into(),
            ..Instruction::mov(5, 36)
        },
    ] {
        let fixture = Fixture::new(vec![
            Instruction::mov(5, 36),
            middle,
            Instruction::add(0, 5, 129),
        ]);
        let (_, analysis) = fixture.analysis();
        assert!(matches!(
            Gfx942MovPrefixAddU32V1::decode(analysis.trace(), &fixture.function, 4, 12),
            Err(Gfx942MovPrefixAddErrorV1::Instruction(_))
        ));
    }
    let mut instructions = vec![Instruction::mov(5, 36); 64];
    instructions.push(Instruction::add(0, 5, 129));
    let fixture = Fixture::new(instructions);
    let (_, analysis) = fixture.analysis();
    assert_eq!(
        Gfx942MovPrefixAddU32V1::decode(analysis.trace(), &fixture.function, 4, 260),
        Err(Gfx942MovPrefixAddErrorV1::TooManyInstructions)
    );
    let fixture = Fixture::new(vec![Instruction::mov(5, 36)]);
    let (_, analysis) = fixture.analysis();
    assert!(matches!(
        Gfx942MovPrefixAddU32V1::decode(analysis.trace(), &fixture.function, 4, 4),
        Err(Gfx942MovPrefixAddErrorV1::Instruction(_))
    ));
}
