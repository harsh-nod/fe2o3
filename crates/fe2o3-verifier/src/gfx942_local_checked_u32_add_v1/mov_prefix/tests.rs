use super::*;

#[path = "../../../tests/support/local_checked_u32_add.rs"]
mod source_fixture;
mod trace_api {
    pub use fe2o3_kernel_analysis::*;
}
#[path = "../../../../fe2o3-kernel-analysis/src/gfx942_integer_semantics_v1/mov_prefix_add/trace_fixture.rs"]
mod trace_fixture;
use trace_fixture::{Fixture, Instruction};

fn source(inputs: &ValidatedCompilerProofInputsV4) -> SourceCoordinates {
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    source_coordinates(inputs, &module, 0).unwrap()
}

fn fixture(inputs: &ValidatedCompilerProofInputsV4, instructions: Vec<Instruction>) -> Fixture {
    let mut fixture = Fixture::new(instructions);
    fixture.function = source(inputs).function;
    fixture
}

#[test]
fn borrowed_prefix_coordinates_derive_original_entry_and_real_machine_result() {
    let inputs = source_fixture::source_inputs();
    let fixture = fixture(
        &inputs,
        vec![
            Instruction::mov(5, 36),
            Instruction::mov(7, 129),
            Instruction::add(9, 5, 7),
        ],
    );
    let (request, analysis) = fixture.analysis();
    let result = prefix_machine_coordinates(source(&inputs), &request, &analysis, 4, 12).unwrap();
    assert_eq!(result.entry.machine_source_sgpr, 36);
    assert_eq!(result.entry.machine_instruction_offset, 4);
    assert_eq!(result.entry.semantic_local, 1);
    assert_eq!(result.entry.kernel_ir_value, result.source.lhs);
    assert_eq!(result.definition, Gfx942ReachingDefinitionV1::LiveIn);
    assert_eq!(result.results.machine_destination_sgpr, 9);
    let mut original = [71; 102];
    original[36] = u32::MAX;
    let mut state = trace_api::Gfx942ScalarIntegerStateV1::new(original, false);
    let actual = result.machine.execute(&mut state);
    original[5] = u32::MAX;
    original[7] = 1;
    original[9] = 0;
    assert_eq!(state.registers(), &original);
    assert_eq!(actual.value, 0);
    assert!(actual.scc);
    // Old single-instruction profile still rejects two register operands.
    assert!(machine_coordinates(source(&inputs), &request, &analysis, 12).is_err());
}

#[test]
fn exact_literal_and_single_entry_origin_are_required_after_the_whole_prefix() {
    let inputs = source_fixture::source_inputs();
    for instructions in [
        vec![Instruction::mov(7, 130), Instruction::add(9, 36, 7)],
        vec![Instruction::mov(7, 2), Instruction::add(9, 36, 7)],
        vec![Instruction::mov(7, 129), Instruction::add(9, 129, 7)],
        vec![
            Instruction::mov(7, 129),
            Instruction::mov(7, 130),
            Instruction::add(9, 36, 7),
        ],
    ] {
        let terminal = 4 * instructions.len() as u64;
        let fixture = fixture(&inputs, instructions);
        let (request, analysis) = fixture.analysis();
        assert!(matches!(
            prefix_machine_coordinates(source(&inputs), &request, &analysis, 4, terminal),
            Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::LiteralMismatch)
        ));
    }
}

#[test]
fn entry_definition_is_checked_before_prefix_not_before_terminal_add() {
    let inputs = source_fixture::source_inputs();
    for instruction in [Instruction::mov(36, 22), Instruction::add(36, 22, 129)] {
        let fixture = fixture(
            &inputs,
            vec![
                instruction,
                Instruction::mov(5, 36),
                Instruction::add(9, 5, 129),
            ],
        );
        let (request, analysis) = fixture.analysis();
        let result =
            prefix_machine_coordinates(source(&inputs), &request, &analysis, 8, 12).unwrap();
        assert_eq!(result.entry.machine_source_sgpr, 36);
        assert_eq!(result.entry.machine_instruction_offset, 8);
        assert_eq!(
            result.definition,
            Gfx942ReachingDefinitionV1::Instruction { offset: 4 }
        );
    }
    let external = Instruction {
        encoding: (0x8080_0000u32 | (36 << 16) | (129 << 8) | 22)
            .to_le_bytes()
            .to_vec(),
        opcode: "S_SUB_U32_vi".into(),
        ..Instruction::add(36, 22, 129)
    };
    let fixture = fixture(
        &inputs,
        vec![
            external,
            Instruction::mov(5, 36),
            Instruction::add(9, 5, 129),
        ],
    );
    let (request, analysis) = fixture.analysis();
    assert!(matches!(
        prefix_machine_coordinates(source(&inputs), &request, &analysis, 8, 12),
        Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::UnsupportedMachineDefinition)
    ));
}

#[test]
fn ambiguous_loop_entry_and_foreign_request_fail_closed() {
    let inputs = source_fixture::source_inputs();
    let mut fixture = fixture(
        &inputs,
        vec![Instruction::mov(5, 36), Instruction::add(36, 5, 129)],
    );
    fixture.backedge = true;
    let (request, analysis) = fixture.analysis();
    let dataflow = Gfx942MachineDataflowV1::derive(analysis.trace()).unwrap();
    let definitions = dataflow
        .reaching_definitions_before(&fixture.function, 8, Gfx942RegisterUnitV1::Sgpr(36))
        .unwrap();
    assert_eq!(definitions.len(), 2);
    assert!(definitions.contains(&Gfx942ReachingDefinitionV1::LiveIn));
    assert!(definitions.contains(&Gfx942ReachingDefinitionV1::Instruction { offset: 12 }));
    assert!(matches!(
        prefix_machine_coordinates(source(&inputs), &request, &analysis, 8, 12),
        Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::AmbiguousMachineDefinition)
    ));
    fixture.backedge = false;
    let (other_request, _) = fixture.analysis();
    assert!(matches!(
        prefix_machine_coordinates(source(&inputs), &other_request, &analysis, 4, 8),
        Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::MachineBinding)
    ));
    fixture.split_at = Some(1);
    let (request, analysis) = fixture.analysis();
    assert!(matches!(
        prefix_machine_coordinates(source(&inputs), &request, &analysis, 4, 8),
        Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::Span(
            Gfx942MovPrefixAddErrorV1::CrossBlock
        ))
    ));
}
