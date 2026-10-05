use super::*;

#[path = "../../tests/support/local_checked_u32_add.rs"]
mod source_fixture;
mod trace_api {
    pub use fe2o3_kernel_analysis::*;
}
#[path = "../../../fe2o3-kernel-analysis/src/gfx942_integer_semantics_v1/tests/trace_fixture.rs"]
mod trace_fixture;
use trace_fixture::TraceFixture;

fn source(inputs: &ValidatedCompilerProofInputsV4) -> SourceCoordinates {
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    source_coordinates(inputs, &module, 0)
        .unwrap_or_else(|error| panic!("fixture source profile: {error:?}\n{module:#?}"))
}

fn machine(function: &str, literal: u8) -> TraceFixture {
    let mut fixture = TraceFixture::registers(9, 35, 128 + literal);
    fixture.function = function.to_owned();
    fixture.operands[2].0 =
        trace_api::PhysicalMachineOperandValueV1::SignedImmediate(i64::from(literal));
    fixture
}

fn block_mut<'a>(
    inputs: &ValidatedCompilerProofInputsV4,
    module: &'a mut Module,
) -> &'a mut fe2o3_kernel_ir::BasicBlock {
    let anchor = inputs.semantic_u32_induction_kir_anchors()[0];
    let function = module
        .functions
        .iter_mut()
        .find(|f| f.body.is_some())
        .unwrap();
    function
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id.0 == anchor.kernel_ir_block())
        .unwrap()
}

#[test]
fn exact_retained_source_shape_extracts_only_conditional_coordinates() {
    let inputs = source_fixture::source_inputs();
    let source = source(&inputs);
    assert_eq!(source.local, 1);
    assert_eq!(source.result_local, 4);
    assert_eq!(source.literal, 1);
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    let function = module
        .functions
        .iter()
        .find(|function| function.body.is_some())
        .unwrap();
    let definition = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| {
            block
                .parameters
                .iter()
                .any(|parameter| parameter.id.0 == source.lhs)
        })
        .unwrap();
    assert_ne!(definition.id.0, source.anchor.kernel_ir_block());
    assert!(analyze_control_flow(function).unwrap().dominates(
        definition.id,
        fe2o3_kernel_ir::BlockId(source.anchor.kernel_ir_block())
    ));
    let fixture = machine(&source.function, 1);
    assert!(Gfx942SAddU32V1::decode(&fixture.instruction()).is_ok());
    let (request, analysis) = fixture.analysis();
    let coordinates = machine_coordinates(source, &request, &analysis, 4).unwrap();
    // These are separate coordinates, not an established local/SSA/register equality.
    assert_eq!(coordinates.entry.semantic_local, 1);
    assert_eq!(coordinates.entry.kernel_ir_value, coordinates.source.lhs);
    assert_eq!(coordinates.entry.machine_source_sgpr, 35);
    assert_eq!(coordinates.entry.machine_instruction_offset, 4);
    assert_eq!(coordinates.results.semantic_result_local, 4);
    assert_eq!(
        coordinates.results.kernel_ir_value,
        coordinates.source.anchor.value_result()
    );
    assert_eq!(
        coordinates.results.kernel_ir_overflow,
        coordinates.source.anchor.overflow_result()
    );
    assert_eq!(coordinates.results.machine_destination_sgpr, 9);
    assert_eq!(coordinates.definition, Gfx942ReachingDefinitionV1::LiveIn);
}

#[test]
fn changed_kir_literal_operand_operation_and_results_reject() {
    let inputs = source_fixture::source_inputs();
    let anchor = inputs.semantic_u32_induction_kir_anchors()[0];
    let original = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    for case in 0..7 {
        let mut module = original.clone();
        let block = block_mut(&inputs, &mut module);
        let operation_index = anchor.kernel_ir_operation() as usize;
        match case {
            0 => {
                block.operations[operation_index - 1].kind =
                    OperationKind::Constant(Constant::U32(2))
            }
            1 => {
                let OperationKind::Binary { lhs, rhs, .. } =
                    &mut block.operations[operation_index].kind
                else {
                    panic!("checked add")
                };
                std::mem::swap(lhs, rhs);
            }
            2 => {
                let OperationKind::Binary { op, .. } = &mut block.operations[operation_index].kind
                else {
                    panic!("checked add")
                };
                *op = BinaryOp::Checked(CheckedBinaryOperator::Subtract);
            }
            3 => block.operations[operation_index].results.swap(0, 1),
            4 => block.operations[operation_index].results[0].ty = Type::Scalar(ScalarType::U64),
            5 => block.operations[operation_index].results[1].ty = Type::Scalar(ScalarType::U32),
            6 => {
                let OperationKind::Binary { lhs, .. } = &mut block.operations[operation_index].kind
                else {
                    panic!("checked add")
                };
                *lhs = ValueId(u32::MAX);
            }
            _ => unreachable!(),
        }
        let error = source_coordinates(&inputs, &module, 0).unwrap_err();
        match case {
            0 => assert!(matches!(
                error,
                Gfx942LocalCheckedU32AddErrorV1::LiteralMismatch
            )),
            1 => assert!(matches!(
                error,
                Gfx942LocalCheckedU32AddErrorV1::UnsupportedKernelOperand
            )),
            6 => assert!(matches!(
                error,
                Gfx942LocalCheckedU32AddErrorV1::AmbiguousKernelDefinition
            )),
            _ => assert!(matches!(
                error,
                Gfx942LocalCheckedU32AddErrorV1::KernelShape
            )),
        }
    }
    // Mutating private helper inputs above never substitutes the public retained KIR owner.
    assert!(source_coordinates(&inputs, &original, 0).is_ok());
    assert!(matches!(
        source_coordinates(&inputs, &original, usize::MAX),
        Err(Gfx942LocalCheckedU32AddErrorV1::Anchor)
    ));
}

#[test]
fn ambiguous_kir_definitions_and_unmodeled_parameter_types_reject() {
    let inputs = source_fixture::source_inputs();
    let anchor = inputs.semantic_u32_induction_kir_anchors()[0];
    let original = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    let mut module = original.clone();
    let block = block_mut(&inputs, &mut module);
    block
        .operations
        .push(block.operations[anchor.kernel_ir_operation() as usize - 1].clone());
    assert!(matches!(
        source_coordinates(&inputs, &module, 0),
        Err(Gfx942LocalCheckedU32AddErrorV1::AmbiguousKernelDefinition)
    ));

    let mut module = original.clone();
    let coordinates = source(&inputs);
    let function = module
        .functions
        .iter_mut()
        .find(|function| function.body.is_some())
        .unwrap();
    function
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.parameters)
        .find(|parameter| parameter.id.0 == coordinates.lhs)
        .unwrap()
        .ty = Type::Scalar(ScalarType::U64);
    assert!(matches!(
        source_coordinates(&inputs, &module, 0),
        Err(Gfx942LocalCheckedU32AddErrorV1::UnsupportedKernelOperand)
    ));

    let mut module = original;
    let function = module
        .functions
        .iter_mut()
        .find(|function| function.body.is_some())
        .unwrap();
    let cfg = analyze_control_flow(function).unwrap();
    let body = function.body.as_mut().unwrap();
    let unavailable_block = body
        .blocks
        .iter()
        .position(|block| {
            !cfg.dominates(block.id, fe2o3_kernel_ir::BlockId(anchor.kernel_ir_block()))
        })
        .expect("fixture has a block that does not dominate the checked add");
    let defining_block = body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .parameters
                .iter()
                .any(|parameter| parameter.id.0 == coordinates.lhs)
        })
        .unwrap();
    let index = defining_block
        .parameters
        .iter()
        .position(|parameter| parameter.id.0 == coordinates.lhs)
        .unwrap();
    let parameter = defining_block.parameters.remove(index);
    body.blocks[unavailable_block].parameters.push(parameter);
    assert!(matches!(
        source_coordinates(&inputs, &module, 0),
        Err(Gfx942LocalCheckedU32AddErrorV1::UnsupportedKernelOperand),
    ));
}

#[test]
fn changed_machine_literal_register_profile_and_encoding_reject() {
    let inputs = source_fixture::source_inputs();
    let function = source(&inputs).function;
    let (request, analysis) = machine(&function, 2).analysis();
    assert!(matches!(
        machine_coordinates(source(&inputs), &request, &analysis, 4),
        Err(Gfx942LocalCheckedU32AddErrorV1::LiteralMismatch)
    ));

    let mut registers = TraceFixture::registers(9, 35, 36);
    registers.function.clone_from(&function);
    let (request, analysis) = registers.analysis();
    assert!(matches!(
        machine_coordinates(source(&inputs), &request, &analysis, 4),
        Err(Gfx942LocalCheckedU32AddErrorV1::LiteralMismatch)
    ));

    let mut changed = machine(&function, 1);
    changed.encoding[1] = 130;
    let (request, analysis) = changed.analysis();
    assert!(matches!(
        machine_coordinates(source(&inputs), &request, &analysis, 4),
        Err(Gfx942LocalCheckedU32AddErrorV1::MachineInstruction(_))
    ));
}

#[test]
fn self_consistent_gfx950_analysis_cannot_enter_gfx942_refinement() {
    let inputs = source_fixture::source_inputs();
    let fixture = machine(&source(&inputs).function, 1);
    let (request942, analysis942) = fixture.analysis();
    assert!(machine_coordinates(source(&inputs), &request942, &analysis942, 4).is_ok());
    let (request, analysis) =
        fixture.analysis_for_target(trace_api::PhysicalMachineTargetV1::Gfx950XnackMinusCov6);
    assert_eq!(
        request.exact_payload_bytes(),
        request942.exact_payload_bytes()
    );
    assert_eq!(analysis.effects().request_identity(), request.identity());
    assert_eq!(analysis.target(), request.target());
    assert!(matches!(
        machine_coordinates(source(&inputs), &request, &analysis, 4),
        Err(Gfx942LocalCheckedU32AddErrorV1::MachineBinding)
    ));
}

#[test]
fn substituted_analysis_request_function_and_offset_reject() {
    let inputs = source_fixture::source_inputs();
    let function = source(&inputs).function;
    let (request, analysis) = machine(&function, 1).analysis();
    let (other_request, _) = machine(&function, 2).analysis();
    assert!(matches!(
        machine_coordinates(source(&inputs), &other_request, &analysis, 4),
        Err(Gfx942LocalCheckedU32AddErrorV1::MachineBinding)
    ));
    assert!(matches!(
        machine_coordinates(source(&inputs), &request, &analysis, 99),
        Err(Gfx942LocalCheckedU32AddErrorV1::MachineBinding)
    ));
    let (other_request, other_analysis) = machine("different_function", 1).analysis();
    assert!(matches!(
        machine_coordinates(source(&inputs), &other_request, &other_analysis, 4),
        Err(Gfx942LocalCheckedU32AddErrorV1::MachineBinding)
    ));
}

#[test]
fn actual_backedge_dataflow_rejects_live_in_and_loop_carried_definition() {
    let inputs = source_fixture::source_inputs();
    let mut fixture = TraceFixture::registers(5, 5, 129);
    fixture.function = source(&inputs).function;
    fixture.operands[2].0 = trace_api::PhysicalMachineOperandValueV1::SignedImmediate(1);
    let (request, analysis) = fixture.analysis_with_backedge();
    assert_eq!(analysis.trace().blocks().len(), 3);
    assert_eq!(analysis.trace().blocks()[0].successors(), &[1]);
    assert_eq!(analysis.trace().blocks()[1].successors(), &[1, 2]);
    assert!(analysis.trace().blocks()[2].successors().is_empty());
    let dataflow = Gfx942MachineDataflowV1::derive(analysis.trace()).unwrap();
    assert_eq!(
        dataflow
            .reaching_definitions_before(&fixture.function, 8, Gfx942RegisterUnitV1::Sgpr(5))
            .unwrap(),
        vec![
            Gfx942ReachingDefinitionV1::LiveIn,
            Gfx942ReachingDefinitionV1::Instruction { offset: 8 }
        ],
    );
    assert!(matches!(
        machine_coordinates(source(&inputs), &request, &analysis, 8),
        Err(Gfx942LocalCheckedU32AddErrorV1::AmbiguousMachineDefinition),
    ));
    // This decodes synthetic CFG evidence; it does not execute the loop or establish termination.
}
