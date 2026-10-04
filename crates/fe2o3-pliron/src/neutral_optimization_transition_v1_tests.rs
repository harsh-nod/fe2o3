//! Real seven-pass occurrence capture checked against both admitted owners.
//! Fixture construction is outside the compiler ledger; every compiler owner,
//! inventory, observation and checked-view receipt is transferred within it.

use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1 as Inventory, CanonicalKirTransitionErrorV1 as CheckError,
    check_canonical_kir_transition_v1 as check,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1,
    CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as DescendantKind, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate,
    CanonicalKirOperationOriginV1 as Origin, CanonicalKirTransitionCandidateV1 as Candidate,
    CanonicalKirUseCoordinateV1 as UseCoordinate, CheckedBinaryOperator, Constant, Function,
    IntegerSwitchCase, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    SwitchCase, Terminator, Type, UnaryOp, ValueDef, ValueId, VerificationContractKeyV12,
    VerificationContractOperationV12, WorkgroupPipelineEventKindV12,
};
use std::mem::size_of;

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
const PREFIX: usize = 17;

#[path = "checked_control_index_v1_tests.rs"]
mod checked_control_index_v1_tests;

#[path = "native_catalog_transport_v1_tests.rs"]
mod native_catalog_transport_v1_tests;

#[path = "canonical_kir_transition_receipt_capture_v1_tests.rs"]
mod canonical_kir_transition_receipt_capture_v1_tests;

fn u32_type() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn value(id: u32) -> ValueDef {
    ValueDef::new(ValueId(id), u32_type())
}
fn boolean(id: u32) -> ValueDef {
    ValueDef::new(ValueId(id), Type::BOOL)
}
fn b(block: u32) -> BlockCoordinate {
    BlockCoordinate {
        function: FunctionCoordinate(0),
        block,
    }
}
fn result(operation: u32, ordinal: u32) -> Definition {
    Definition::Result {
        operation: OperationCoordinate {
            block: b(0),
            operation,
        },
        result: ordinal,
    }
}
fn constant(id: u32, literal: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), literal.ty()),
        OperationKind::Constant(literal),
    )
}
fn returning(id: u32, values: &[u32]) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(Terminator::Return {
        values: values.iter().map(|&value| ValueId(value)).collect(),
    });
    block
}
fn function_module(
    parameters: Vec<Type>,
    results: Vec<Type>,
    ids: Vec<ValueId>,
    blocks: Vec<BasicBlock>,
) -> Module {
    let mut module = Module::new("actual-neutral-transition");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(parameters, results),
        ids,
        blocks,
    ));
    module
}

fn accepted(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Candidate<'_>,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let before = budget.work();
    let retained = {
        let (checked, storage) = check(input, output, rows, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(std::ptr::eq(checked.input(), input));
        assert!(std::ptr::eq(checked.output(), output));
        assert!(std::ptr::eq(checked.rows().uses, rows.uses));
        assert!(std::ptr::eq(checked.rows().operations, rows.operations));
        assert!(std::ptr::eq(
            checked.rows().definition_outputs,
            rows.definition_outputs
        ));
        assert_eq!(checked.checked_rules().len(), 6);
        assert!(!checked.grants_authority());
        assert!(budget.work() > before);
        storage.retained_storage()
    };
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
}

fn rejected(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Candidate<'_>,
    budget: &mut Budget<'_>,
) -> CheckError {
    let floor = budget.storage();
    let before = budget.work();
    let peak = budget.peak_storage();
    let error = check(input, output, rows, budget).unwrap_err();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() >= before);
    assert!(budget.peak_storage() >= peak);
    error
}

fn with_observed(
    module: &Module,
    budget: &mut Budget<'_>,
    inspect: impl FnOnce(
        &KirNeutralOptimizationOutputV1<'_>,
        &Inventory<'_>,
        &Inventory<'_>,
        &mut Budget<'_>,
    ),
) {
    let floor = budget.storage();
    let (input, input_storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let (input_inventory, input_index_storage) = Inventory::derive(&input, budget).unwrap();
    budget
        .reserve_storage(input_index_storage.retained_storage())
        .unwrap();
    let (mut graph, graph_storage) = KirPlironGraphV12::import(&input, budget).unwrap();
    budget
        .reserve_storage(graph_storage.retained_storage())
        .unwrap();
    let observed = graph
        .execute_production_neutral_optimization_v1(budget)
        .unwrap()
        .extract()
        .unwrap();
    let observed_storage = observed.storage().retained_storage();
    budget.reserve_storage(observed_storage).unwrap();
    assert!(std::ptr::eq(observed.input(), &input));
    assert!(observed.map().matches_execution(observed.report()));
    assert_eq!(observed.report().passes().len(), 7);
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    let (output_inventory, output_index_storage) =
        Inventory::derive(observed.owner(), budget).unwrap();
    budget
        .reserve_storage(output_index_storage.retained_storage())
        .unwrap();
    accepted(
        &input_inventory,
        &output_inventory,
        observed.occurrences().candidate(),
        budget,
    );
    inspect(&observed, &input_inventory, &output_inventory, budget);
    drop(output_inventory);
    budget
        .release_storage(output_index_storage.retained_storage())
        .unwrap();
    drop(observed);
    budget.release_storage(observed_storage).unwrap();
    drop(input_inventory);
    budget
        .release_storage(input_index_storage.retained_storage())
        .unwrap();
    drop(input);
    budget
        .release_storage(input_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

fn with_transition(
    module: &Module,
    inspect: impl FnOnce(
        &KirNeutralOptimizationOutputV1<'_>,
        &Inventory<'_>,
        &Inventory<'_>,
        &mut Budget<'_>,
    ),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(13).unwrap();
    budget.reserve_storage(PREFIX).unwrap();
    with_observed(module, &mut budget, inspect);
    assert_eq!(budget.storage(), PREFIX);
}

fn with_row_copy<T: Copy>(
    rows: &[T],
    budget: &mut Budget<'_>,
    inspect: impl FnOnce(&mut [T], &mut Budget<'_>),
) {
    let retained = size_of::<Vec<T>>() + std::mem::size_of_val(rows);
    budget.reserve_storage(retained).unwrap();
    budget.charge_work(rows.len()).unwrap();
    let mut changed = Vec::new();
    changed.try_reserve_exact(rows.len()).unwrap();
    assert_eq!(
        changed.capacity(),
        rows.len(),
        "fixture requires exact admitted row-copy capacity"
    );
    changed.extend_from_slice(rows);
    inspect(&mut changed, budget);
    drop(changed);
    budget.release_storage(retained).unwrap();
}

fn duplicate_edges(condition: Option<bool>, swapped: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(10));
    if let Some(condition) = condition {
        entry
            .operations
            .push(constant(0, Constant::Bool(condition)));
    }
    let (left, right) = if swapped { (2, 1) } else { (1, 2) };
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(70),
        then_arguments: vec![ValueId(left), ValueId(3)],
        else_target: BlockId(70),
        else_arguments: vec![ValueId(right), ValueId(3)],
    });
    let mut join = returning(70, &[4]);
    join.parameters = vec![value(4), value(5)];
    let (parameters, ids) = if condition.is_some() {
        (
            vec![u32_type(); 3],
            vec![ValueId(1), ValueId(2), ValueId(3)],
        )
    } else {
        (
            vec![Type::BOOL, u32_type(), u32_type(), u32_type()],
            (0..4).map(ValueId).collect(),
        )
    };
    function_module(parameters, vec![u32_type()], ids, vec![entry, join])
}

#[test]
fn actual_duplicate_targets_keep_unequal_payloads_and_reject_origin_swaps() {
    with_transition(
        &duplicate_edges(None, false),
        |observed, input, output, budget| {
            let rows = observed.occurrences().candidate();
            assert_eq!(rows.edges.len(), 2);
            assert_eq!(rows.edge_arguments.len(), 2);
            assert_eq!(output.blocks()[1].block.parameters.len(), 1);
            assert_ne!(
                output.edge_arguments()[0].value,
                output.edge_arguments()[1].value
            );
            with_row_copy(rows.edges, budget, |changed, budget| {
                let first = changed[0].input;
                changed[0].input = changed[1].input;
                changed[1].input = first;
                assert_eq!(
                    rejected(
                        input,
                        output,
                        Candidate {
                            edges: changed,
                            ..rows
                        },
                        budget
                    ),
                    CheckError::Rule("successor occurrence order")
                );
            });
            with_row_copy(rows.edge_arguments, budget, |changed, budget| {
                changed[1].input = changed[0].input;
                assert_eq!(
                    rejected(
                        input,
                        output,
                        Candidate {
                            edge_arguments: changed,
                            ..rows
                        },
                        budget
                    ),
                    CheckError::Rule("edge argument parameter transport")
                );
            });
            with_row_copy(rows.uses, budget, |changed, budget| {
                let first = changed
                    .iter()
                    .position(|row| {
                        row.input
                            == UseCoordinate::TerminatorOperand {
                                block: b(0),
                                operand: 1,
                            }
                    })
                    .unwrap();
                let other = changed
                    .iter()
                    .position(|row| {
                        row.input
                            == UseCoordinate::TerminatorOperand {
                                block: b(0),
                                operand: 3,
                            }
                    })
                    .unwrap();
                changed[first].input = changed[other].input;
                assert!(matches!(
                    rejected(
                        input,
                        output,
                        Candidate {
                            uses: changed,
                            ..rows
                        },
                        budget
                    ),
                    CheckError::Rule(_)
                ));
            });
            accepted(input, output, rows, budget);
        },
    );
}

#[test]
fn actual_selected_duplicate_occurrence_and_merge_are_checked_for_both_polarities() {
    for (condition, selected) in [(false, 1), (true, 0)] {
        with_transition(
            &duplicate_edges(Some(condition), false),
            |observed, _, output, _| {
                let rows = observed.occurrences().candidate();
                assert_eq!(output.blocks().len(), 1);
                assert_eq!(rows.segments.len(), 2);
                assert_eq!(
                    rows.segments[0].connector,
                    Some(Edge {
                        source: b(0),
                        successor: selected
                    })
                );
                assert!(rows.edges.is_empty());
                let return_use = output.uses().last().unwrap();
                assert_eq!(
                    output.definitions()[return_use.definition].coordinate,
                    Definition::FunctionArgument {
                        function: FunctionCoordinate(0),
                        argument: selected
                    }
                );
            },
        );
    }
}

fn grounded_loop() -> Module {
    let mut entry = BasicBlock::new(BlockId(80));
    entry.operations.push(constant(2, Constant::U32(7)));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(2)],
    });
    let mut loop_block = BasicBlock::new(BlockId(2));
    loop_block.parameters.push(value(10));
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(9),
        then_arguments: vec![ValueId(10)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(10)],
    });
    let mut exit = returning(9, &[20]);
    exit.parameters.push(value(20));
    function_module(
        vec![Type::BOOL],
        vec![u32_type()],
        vec![ValueId(1)],
        vec![entry, loop_block, exit],
    )
}

#[test]
fn actual_grounded_loop_materialization_keeps_both_executable_successors() {
    with_transition(&grounded_loop(), |observed, _, output, _| {
        let rows = observed.occurrences().candidate();
        assert_eq!(output.blocks().len(), 3);
        assert_eq!(rows.edges.len(), 3);
        assert!(
            output
                .blocks()
                .iter()
                .all(|block| block.block.parameters.is_empty())
        );
        assert!(rows.edges.iter().any(|row| row.input
            == Edge {
                source: b(1),
                successor: 0
            }));
        assert!(rows.edges.iter().any(|row| row.input
            == Edge {
                source: b(1),
                successor: 1
            }));
        for block in [1, 2] {
            let definition = rows
                .definitions
                .iter()
                .find(|row| {
                    row.input
                        == Definition::BlockArgument {
                            block: b(block),
                            argument: 0,
                        }
                })
                .unwrap();
            // SCCP materializes both parameters before DCE removes their edge
            // slots. Only the exit's constant still has a live return use.
            assert_eq!(definition.outputs.len, u32::from(block == 2));
            assert!(
                rows.definition_outputs[definition.outputs.start as usize
                    ..(definition.outputs.start + definition.outputs.len) as usize]
                    .iter()
                    .all(|row| row.kind == DescendantKind::Substituted)
            );
        }
        assert_eq!(output.operations().len(), 1);
        assert_eq!(
            output.operations()[0].operation.kind,
            OperationKind::Constant(Constant::U32(7))
        );
        assert_eq!(
            rows.operations[0].origin,
            Origin::ConstantFrom(Definition::BlockArgument {
                block: b(2),
                argument: 0,
            })
        );
    });
}

fn arithmetic_effects() -> Module {
    let mut entry = returning(10, &[12, 13, 14, 15]);
    entry.operations = vec![
        constant(10, Constant::U32(9)),
        constant(11, Constant::U32(7)),
        Operation::checked_binary(
            value(12),
            boolean(13),
            CheckedBinaryOperator::Add,
            ValueId(10),
            ValueId(11),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(12),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::VerificationContract(
                VerificationContractOperationV12::WorkgroupPipelineEvent {
                    contract: VerificationContractKeyV12::new(17),
                    kind: WorkgroupPipelineEventKindV12::Stage,
                    storage: ValueId(1),
                    epoch: ValueId(2),
                },
            ),
        ),
        Operation::effect_free(
            value(14),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        ),
        Operation::effect_free(
            value(15),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(3),
            },
        ),
        Operation::effect_free(
            value(16),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(3),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(16),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    function_module(
        vec![
            Type::pointer(u32_type(), AddressSpace::Global, AccessMode::ReadWrite),
            Type::pointer(u32_type(), AddressSpace::Workgroup, AccessMode::ReadWrite),
            Type::INDEX,
            u32_type(),
        ],
        vec![u32_type(), Type::BOOL, u32_type(), u32_type()],
        (0..4).map(ValueId).collect(),
        vec![entry],
    )
}

#[test]
fn actual_checked_pair_substitution_preserves_ordinary_arithmetic_effects_and_marker_order() {
    with_transition(&arithmetic_effects(), |observed, input, output, budget| {
        let rows = observed.occurrences().candidate();
        for ordinal in 0..2 {
            assert!(
                rows.operations
                    .iter()
                    .any(|row| row.origin == Origin::ConstantFrom(result(2, ordinal)))
            );
            let definition = rows
                .definitions
                .iter()
                .find(|row| row.input == result(2, ordinal))
                .unwrap();
            assert!(definition.outputs.len > 0);
            assert!(
                rows.definition_outputs[definition.outputs.start as usize
                    ..(definition.outputs.start + definition.outputs.len) as usize]
                    .iter()
                    .all(|row| row.kind == DescendantKind::Substituted)
            );
        }
        let ordinary = rows
            .definitions
            .iter()
            .find(|row| row.input == result(5, 0))
            .unwrap();
        let descendants = &rows.definition_outputs[ordinary.outputs.start as usize
            ..(ordinary.outputs.start + ordinary.outputs.len) as usize];
        assert_eq!(
            descendants
                .iter()
                .filter(|row| row.kind == DescendantKind::Retained)
                .count(),
            1
        );
        assert_eq!(
            descendants
                .iter()
                .filter(|row| row.kind == DescendantKind::Substituted)
                .count(),
            1
        );
        let ordered = rows.operations.iter().filter_map(|row| match row.origin {
            Origin::Retained(coordinate) if [3, 4, 5, 8].contains(&coordinate.operation) => {
                Some(coordinate.operation)
            }
            _ => None,
        });
        assert!(ordered.eq([3, 4, 5, 8]));
        assert_eq!(
            output
                .operations()
                .iter()
                .filter(|row| matches!(
                    row.operation.kind,
                    OperationKind::Unary {
                        op: UnaryOp::Not,
                        ..
                    }
                ))
                .count(),
            1
        );
        assert_eq!(
            output
                .operations()
                .iter()
                .filter(|row| matches!(row.operation.kind, OperationKind::VerificationContract(_)))
                .count(),
            1
        );
        with_row_copy(rows.operations, budget, |changed, budget| {
            let value_constant = changed
                .iter()
                .position(|row| row.origin == Origin::ConstantFrom(result(2, 0)))
                .unwrap();
            changed[value_constant].origin = Origin::ConstantFrom(result(2, 1));
            assert!(matches!(
                rejected(
                    input,
                    output,
                    Candidate {
                        operations: changed,
                        ..rows
                    },
                    budget
                ),
                CheckError::Rule(_)
            ));
        });
    });
}

fn default_switch(integer: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(10));
    entry.terminator = Some(if integer {
        Terminator::IntegerSwitch {
            selector: ValueId(0),
            cases: vec![],
            default_target: BlockId(30),
            default_arguments: vec![ValueId(1)],
        }
    } else {
        Terminator::Switch {
            selector: ValueId(0),
            cases: vec![],
            default_target: BlockId(30),
            default_arguments: vec![ValueId(1)],
        }
    });
    let mut middle = BasicBlock::new(BlockId(30));
    middle.parameters.push(value(2));
    middle.terminator = Some(Terminator::Branch {
        target: BlockId(40),
        arguments: vec![ValueId(2)],
    });
    let mut tail = returning(40, &[3]);
    tail.parameters.push(value(3));
    let mut dead_a = BasicBlock::new(BlockId(50));
    dead_a.terminator = Some(Terminator::Branch {
        target: BlockId(70),
        arguments: vec![],
    });
    let mut dead_b = BasicBlock::new(BlockId(70));
    dead_b.terminator = Some(Terminator::Branch {
        target: BlockId(50),
        arguments: vec![],
    });
    function_module(
        vec![u32_type(), u32_type()],
        vec![u32_type()],
        vec![ValueId(0), ValueId(1)],
        vec![entry, middle, tail, dead_a, dead_b],
    )
}

#[test]
fn actual_default_only_switch_merge_chains_remove_dead_cycles() {
    for integer in [false, true] {
        with_transition(&default_switch(integer), |observed, _, output, _| {
            let rows = observed.occurrences().candidate();
            assert_eq!(output.blocks().len(), 1);
            assert_eq!(rows.segments.len(), 3);
            assert_eq!(
                rows.segments[0].connector,
                Some(Edge {
                    source: b(0),
                    successor: 0
                })
            );
            assert_eq!(
                rows.segments[1].connector,
                Some(Edge {
                    source: b(1),
                    successor: 0
                })
            );
            assert_eq!(rows.segments[2].input, b(2));
            assert_eq!(rows.segments[2].connector, None);
            assert!(rows.edges.is_empty());
            assert_eq!(rows.uses.len(), 1);
            assert_eq!(
                rows.uses[0].input,
                UseCoordinate::TerminatorOperand {
                    block: b(2),
                    operand: 0
                }
            );
        });
    }
}

#[test]
fn actual_signed_and_wide_switches_keep_distinct_same_target_tuples() {
    for signed in [false, true] {
        let mut entry = BasicBlock::new(BlockId(10));
        entry.terminator = Some(if signed {
            Terminator::IntegerSwitch {
                selector: ValueId(0),
                cases: vec![
                    IntegerSwitchCase {
                        value: Constant::I32(-7),
                        target: BlockId(70),
                        arguments: vec![ValueId(1)],
                    },
                    IntegerSwitchCase {
                        value: Constant::I32(9),
                        target: BlockId(70),
                        arguments: vec![ValueId(2)],
                    },
                ],
                default_target: BlockId(70),
                default_arguments: vec![ValueId(1)],
            }
        } else {
            Terminator::Switch {
                selector: ValueId(0),
                cases: vec![
                    SwitchCase {
                        value: 0,
                        target: BlockId(70),
                        arguments: vec![ValueId(1)],
                    },
                    SwitchCase {
                        value: u64::MAX,
                        target: BlockId(70),
                        arguments: vec![ValueId(2)],
                    },
                ],
                default_target: BlockId(70),
                default_arguments: vec![ValueId(1)],
            }
        });
        let mut join = returning(70, &[3]);
        join.parameters.push(value(3));
        let module = function_module(
            vec![
                Type::Scalar(if signed {
                    ScalarType::I32
                } else {
                    ScalarType::U128
                }),
                u32_type(),
                u32_type(),
            ],
            vec![u32_type()],
            (0..3).map(ValueId).collect(),
            vec![entry, join],
        );
        with_transition(&module, |observed, _, output, _| {
            assert_eq!(observed.occurrences().candidate().edges.len(), 3);
            assert_eq!(output.edges().len(), 3);
            assert_eq!(output.edge_arguments().len(), 3);
            assert_ne!(
                output.edge_arguments()[0].value,
                output.edge_arguments()[1].value
            );
            assert_eq!(
                output.edge_arguments()[0].value,
                output.edge_arguments()[2].value
            );
        });
    }
}

#[test]
fn actual_shared_helper_calls_and_external_declaration_are_preserved() {
    let signature = Signature::new(vec![u32_type()], vec![u32_type()]);
    let mut module = Module::new("shared-helper-and-declaration");
    for name in ["caller-a", "caller-b"] {
        let mut block = returning(10, &[1]);
        block.operations.push(Operation::effect_free(
            value(1),
            OperationKind::Call {
                callee: "shared".into(),
                arguments: vec![ValueId(0)],
            },
        ));
        module.functions.push(Function::internal_helper(
            name,
            signature.clone(),
            vec![ValueId(0)],
            vec![block],
        ));
    }
    let mut shared = returning(30, &[1]);
    shared.operations.push(Operation::effect_free(
        value(1),
        OperationKind::Call {
            callee: "external".into(),
            arguments: vec![ValueId(0)],
        },
    ));
    module.functions.push(Function::internal_helper(
        "shared",
        signature.clone(),
        vec![ValueId(0)],
        vec![shared],
    ));
    module
        .functions
        .push(Function::declaration("external", signature));
    with_transition(&module, |observed, input, output, _| {
        assert_eq!(observed.occurrences().candidate().functions.len(), 4);
        assert_eq!(output.operations().len(), 3);
        assert!(
            output
                .operations()
                .iter()
                .all(|row| matches!(row.operation.kind, OperationKind::Call { .. }))
        );
        assert!(output.functions()[3].function.body.is_none());
        let declaration = Definition::FunctionArgument {
            function: FunctionCoordinate(3),
            argument: 0,
        };
        assert!(
            input
                .definitions()
                .iter()
                .any(|row| row.coordinate == declaration && row.value.is_none())
        );
        let rows = observed.occurrences().candidate();
        let row = rows
            .definitions
            .iter()
            .find(|row| row.input == declaration)
            .unwrap();
        assert_eq!(row.outputs.len, 1);
        let descendant = rows.definition_outputs[row.outputs.start as usize];
        assert_eq!(descendant.output, declaration);
        assert_eq!(descendant.kind, DescendantKind::Retained);
    });
}

#[test]
fn actual_rows_cannot_be_reused_for_a_distinct_optimized_payload_owner() {
    with_transition(
        &duplicate_edges(None, false),
        |first, input, output, budget| {
            with_observed(
                &duplicate_edges(None, true),
                budget,
                |second, _, other_output, budget| {
                    assert!(!std::ptr::eq(first.owner(), second.owner()));
                    assert_ne!(
                        first.owner().canonical().canonical_bytes(),
                        second.owner().canonical().canonical_bytes()
                    );
                    assert_eq!(output.uses().len(), other_output.uses().len());
                    assert!(matches!(
                        rejected(input, other_output, first.occurrences().candidate(), budget),
                        CheckError::Rule(_)
                    ));
                    accepted(input, output, first.occurrences().candidate(), budget);
                },
            );
        },
    );
}

#[test]
fn actual_capture_survives_hostile_row_failure_without_implicit_retry_or_budget_reset() {
    with_transition(
        &duplicate_edges(None, false),
        |observed, input, output, budget| {
            let rows = observed.occurrences().candidate();
            let before = budget.work();
            assert_eq!(
                rejected(
                    input,
                    output,
                    Candidate {
                        uses: &rows.uses[..rows.uses.len() - 1],
                        ..rows
                    },
                    budget
                ),
                CheckError::IncompleteRows
            );
            assert!(budget.work() > before);
            assert_eq!(rows.uses.len(), output.uses().len());
            accepted(input, output, rows, budget);
        },
    );
}

#[test]
fn actual_extraction_then_checker_work_denial_preserves_all_live_receipts() {
    for remaining in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        {
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(PREFIX).unwrap();
            with_observed(
                &duplicate_edges(None, false),
                &mut budget,
                |observed, input, output, budget| {
                    // Independently target the checker's first and second unit
                    // charge, not a success-observed total-work or peak oracle.
                    budget
                        .charge_work(WORK - budget.work() - remaining)
                        .unwrap();
                    let error = rejected(input, output, observed.occurrences().candidate(), budget);
                    let CheckError::Resource(ResourceError::Work(limit)) = error else {
                        panic!("work denial: {error:?}")
                    };
                    assert_eq!(limit.actual(), WORK + 1);
                    assert_eq!(limit.limit(), WORK);
                    assert_eq!(budget.work(), WORK);
                },
            );
            assert_eq!(budget.storage(), PREFIX);
        }
        assert_eq!(work.failed_work(), Some(WORK + 1));
    }
}

#[test]
fn abandoned_actual_execution_lease_retires_capture_but_keeps_graph_custody_failed() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(PREFIX).unwrap();
    let (input, input_storage) = Owner::from_module_ref_with_verification_budget_v12(
        &duplicate_edges(None, false),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&input, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (mut graph, imported) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(imported.retained_storage()).unwrap();
    let before = budget.work();
    let lease = graph
        .execute_production_neutral_optimization_v1(&mut budget)
        .unwrap();
    drop(lease);
    assert!(budget.work() > before);
    assert_eq!(budget.storage(), floor + graph.retained_storage());
    assert!(graph.validate_custody_v12().is_err());
    assert!(
        graph
            .extract_optimized_canonical_kir_module_v12(&mut budget)
            .is_err()
    );
    let retained = graph.retained_storage();
    drop(graph);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(input);
    budget
        .release_storage(input_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), PREFIX);
}
