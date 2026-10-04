use super::*;
use crate::conditional_fill_program_v1::{check_module, tests::genuine_inputs};

fn replace_blocks(
    function: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    replace_body(function, function.abi().clone(), blocks)
}

fn replace_body(
    function: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap()
}

#[test]
fn genuine_source_rejects_changed_calls_values_borrows_control_and_span_coverage() {
    let (_, inputs) = genuine_inputs();
    let source = inputs.semantic_mir();
    let id = source.select_kernel_body_v1().unwrap().body().index();
    let function = &source.functions()[id as usize];
    let module = fe2o3_kernel_ir::decode_module_v9(inputs.kernel_ir().canonical_bytes()).unwrap();
    let shape = check_module(&module).unwrap();
    check_body(&inputs, &shape, id, function).unwrap();
    let mut rejected = 0;
    for (block_index, block) in function.blocks().iter().enumerate() {
        let mut alternatives = Vec::new();
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            alternatives.push(SemanticTerminatorKindV1::Return);
            alternatives.push(SemanticTerminatorKindV1::Goto(
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(block_index as u32),
                ),
            ));
            let mut arguments = call.arguments().to_vec();
            if let Some(value) = arguments.last_mut() {
                *value = SemanticOperandV1::Constant(SemanticConstantV1::new(
                    value.ty(),
                    SemanticConstantValueV1::ZeroSized,
                ));
            } else {
                arguments.push(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    call.destination().unwrap().place().ty(),
                    SemanticConstantValueV1::ZeroSized,
                )));
            }
            alternatives.push(SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    arguments,
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ));
        }
        for alternative in alternatives {
            let mut blocks = function.blocks().to_vec();
            blocks[block_index] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                block.statements().to_vec(),
                SemanticTerminatorV1::new(block.terminator().source(), alternative),
            )
            .unwrap();
            assert!(check_body(&inputs, &shape, id, &replace_blocks(function, blocks)).is_err());
            rejected += 1;
        }
        for (index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let alternative = match assignment.value().kind() {
                SemanticRvalueKindV1::Cast { .. } => {
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                        assignment.destination().ty(),
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
                    )))
                }
                SemanticRvalueKindV1::Borrow { kind, place } => SemanticRvalueKindV1::Borrow {
                    kind: if *kind == SemanticBorrowKindV1::Shared {
                        SemanticBorrowKindV1::Mutable
                    } else {
                        SemanticBorrowKindV1::Shared
                    },
                    place: place.clone(),
                },
                _ => continue,
            };
            let mut statements = block.statements().to_vec();
            statements[index] = SemanticStatementV1::new(
                statement.source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    assignment.destination().clone(),
                    SemanticRvalueV1::new(assignment.destination().ty(), alternative),
                )),
            );
            let mut blocks = function.blocks().to_vec();
            blocks[block_index] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap();
            assert!(check_body(&inputs, &shape, id, &replace_blocks(function, blocks)).is_err());
            rejected += 1;
        }
    }
    assert_eq!(rejected, 12);
    for site in shape.operations.keys() {
        let mut changed = check_module(&module).unwrap();
        changed.operations.remove(site);
        assert!(check_body(&inputs, &changed, id, function).is_err());
    }
    let mut changed = check_module(&module).unwrap();
    changed.operations.insert((u32::MAX, 0), Some(Value::Index));
    assert!(check_body(&inputs, &changed, id, function).is_err());
}

#[test]
fn terminal_witness_copy_consumes_and_invalidates_borrows_without_allowing_duplication() {
    let (_, inputs) = genuine_inputs();
    let source = inputs.semantic_mir();
    let id = source.select_kernel_body_v1().unwrap().body().index();
    let function = &source.functions()[id as usize];
    let (borrow, place) = function
        .blocks()
        .iter()
        .flat_map(|b| b.statements())
        .find_map(|s| {
            let SemanticStatementKindV1::Assign(a) = s.kind() else {
                return None;
            };
            match a.value().kind() {
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } => Some((a.destination(), place)),
                _ => None,
            }
        })
        .unwrap();
    let index = place.local().index() as usize;
    let borrowed = borrow.local().index() as usize;
    let output = function
        .locals()
        .iter()
        .position(|l| l.role() == SemanticLocalRoleV1::Argument(0))
        .unwrap();
    let fresh = || {
        let origin = Origin([id, 0, 0, index as u32, place.ty().index()]);
        let mut recipe = Recipe::default();
        let witness = recipe.push(origin, Expr::ThreadIndex).unwrap();
        let reference = recipe.push(origin, Expr::SharedBorrow(witness)).unwrap();
        SourceState {
            source,
            function,
            output,
            locals: BTreeMap::from([
                (index, Local::Index),
                (
                    borrowed,
                    Local::SharedIndex {
                        referent: index,
                        generation: 0,
                    },
                ),
            ]),
            generations: BTreeMap::new(),
            expressions: BTreeMap::from([(index, witness), (borrowed, reference)]),
            recipe,
            origin,
        }
    };
    let witness = SemanticOperandV1::Copy(place.clone());
    let reference = SemanticOperandV1::Copy(borrow.clone());
    let mut state = fresh();
    assert!(state.operand(&witness).is_err());
    assert!(state.operand(&reference).is_ok());
    assert_eq!(state.consume_index(&witness).unwrap().0, Local::Index);
    assert!(state.consume_index(&witness).is_err());
    assert!(state.operand(&reference).is_err());
    state.assign(index, (Local::Index, Id(0))).unwrap();
    assert!(
        state.operand(&reference).is_err(),
        "rebinding cannot revive the old borrow"
    );
    for kind in [
        SemanticStatementKindV1::StorageDead(place.local()),
        SemanticStatementKindV1::StorageLive(place.local()),
    ] {
        let mut state = fresh();
        state
            .statement(&SemanticStatementV1::new(function.source(), kind))
            .unwrap();
        state.assign(index, (Local::Index, Id(0))).unwrap();
        assert!(state.operand(&reference).is_err());
    }
    let mut state = fresh();
    state
        .operand(&SemanticOperandV1::Move(place.clone()))
        .unwrap();
    assert!(state.operand(&reference).is_err());
    let mut state = fresh();
    assert_eq!(
        state
            .consume_index(&SemanticOperandV1::Move(place.clone()))
            .unwrap()
            .0,
        Local::Index
    );
    assert!(state.consume_index(&witness).is_err());
    assert!(state.operand(&reference).is_err());
}

#[test]
fn source_profile_rejects_non_unit_return_and_wrong_export() {
    let (_, inputs) = genuine_inputs();
    let source = inputs.semantic_mir();
    let id = source.select_kernel_body_v1().unwrap().body().index();
    let function = &source.functions()[id as usize];
    let module = fe2o3_kernel_ir::decode_module_v9(inputs.kernel_ir().canonical_bytes()).unwrap();
    let shape = check_module(&module).unwrap();
    assert_eq!(
        check_source(&inputs, &shape, "other"),
        Err(Error::Source("entry symbol"))
    );
    let abi = function.abi();
    let changed = SemanticFunctionAbiV1::from_rustc(
        abi.identity(),
        abi.layout_identity(),
        abi.canon_abi(),
        abi.extern_abi(),
        abi.can_unwind(),
        abi.c_variadic(),
        abi.fixed_count(),
        abi.arguments().to_vec(),
        SemanticAbiValueV1::new(abi.source_input_types()[0], SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(abi.source_argument_ownership().to_vec())
    .unwrap();
    let changed = replace_body(function, changed, function.blocks().to_vec());
    assert_eq!(
        check_body(&inputs, &shape, id, &changed),
        Err(Error::Source("signature"))
    );
}
