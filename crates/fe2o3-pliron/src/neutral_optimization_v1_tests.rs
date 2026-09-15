//! Actual-pass custody/occurrence regressions. These do not claim formal proof,
//! ranked admission, source/ISA completion, LLVM or native execution qualification.

use super::*;
use crate::kir_optimization_map_v12::LiveKeyV12;
use crate::{KirBridgeCoordinateV1 as Coordinate, KirOptimizationEndpointV12 as Endpoint};
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1, CheckedBinaryOperator, Constant,
    Function, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef,
    ValueId,
};
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as DescendantKind, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationOriginV1 as Origin, CanonicalKirUseCoordinateV1 as UseCoordinate,
};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
include!("neutral_optimization_v1_native_profile_tests.rs");
fn u32_type() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn value(id: u32) -> ValueDef {
    ValueDef::new(ValueId(id), u32_type())
}
fn boolean(id: u32) -> ValueDef {
    ValueDef::new(ValueId(id), Type::BOOL)
}
struct Input {
    owner: Owner,
    storage: usize,
}
impl std::ops::Deref for Input {
    type Target = Owner;
    fn deref(&self) -> &Owner {
        &self.owner
    }
}
fn owner(module: &Module) -> Input {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    Input {
        owner,
        storage: storage.retained_storage(),
    }
}
fn execute(input: &Input) -> KirNeutralOptimizationOutputV1<'_> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(17 + input.storage).unwrap();
    let (mut graph, storage) = KirPlironGraphV12::import(input, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let output = graph
        .execute_production_neutral_optimization_v1(&mut budget)
        .unwrap()
        .extract()
        .unwrap();
    budget
        .reserve_storage(output.storage().retained_storage())
        .unwrap();
    assert!(output.map().matches_execution(output.report()));
    assert!(std::ptr::eq(output.input(), &input.owner));
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    // This helper explicitly transfers output custody out of its local ledger.
    budget
        .release_storage(output.storage().retained_storage() + input.storage)
        .unwrap();
    assert_eq!(budget.storage(), 17);
    output
}
fn legacy(input: &Input) -> (Owner, PlironOptimizationReportV1, KirOptimizationMapV12) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input.storage).unwrap();
    let (mut graph, storage) = KirPlironGraphV12::import(input, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (report, report_storage) = graph
        .execute_production_optimization_v12(&mut budget)
        .unwrap();
    budget
        .reserve_storage(report_storage.retained_storage())
        .unwrap();
    let (output, _, map, extracted) = graph
        .extract_optimized_canonical_kir_module_with_map_v12(&mut budget)
        .unwrap();
    budget
        .reserve_storage(extracted.retained_storage())
        .unwrap();
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    budget
        .release_storage(
            extracted.retained_storage() + report_storage.retained_storage() + input.storage,
        )
        .unwrap();
    assert_eq!(budget.storage(), 0);
    (output, report, map)
}
fn duplicate_edges(constant: Option<bool>) -> Module {
    let mut entry = BasicBlock::new(BlockId(10));
    if let Some(condition) = constant {
        entry.operations.push(Operation::effect_free(
            boolean(0),
            OperationKind::Constant(Constant::Bool(condition)),
        ));
    }
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(70),
        then_arguments: vec![ValueId(1), ValueId(3)],
        else_target: BlockId(70),
        else_arguments: vec![ValueId(2), ValueId(3)],
    });
    let mut join = BasicBlock::new(BlockId(70));
    join.parameters = vec![value(4), value(5)];
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let mut module = Module::new("duplicate-occurrences");
    let (parameters, ids) = if constant.is_some() {
        (
            vec![u32_type(), u32_type(), u32_type()],
            vec![ValueId(1), ValueId(2), ValueId(3)],
        )
    } else {
        (
            vec![Type::BOOL, u32_type(), u32_type(), u32_type()],
            vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        )
    };
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(parameters, vec![u32_type()]),
        ids,
        vec![entry, join],
    ));
    module
}
fn b(block: u32) -> BlockCoordinate {
    BlockCoordinate {
        function: FunctionCoordinate(0),
        block,
    }
}

#[test]
fn dce_preserves_duplicate_edge_occurrences_and_shifted_use_origins() {
    let input = owner(&duplicate_edges(None));
    let output = execute(&input);
    let rows = output.occurrences().candidate();
    assert_eq!(rows.edges.len(), 2);
    for (successor, row) in rows.edges.iter().enumerate() {
        assert_eq!(
            row.output,
            Edge {
                source: b(0),
                successor: successor as u32
            }
        );
        assert_eq!(row.input, row.output);
    }
    assert_eq!(rows.edge_arguments.len(), 2);
    for row in rows.edge_arguments {
        assert_eq!(row.input.argument, 0);
        assert_eq!(row.input.edge, row.output.edge);
        assert_eq!(row.output.argument, 0);
    }
    let condition_and_arguments = rows
        .uses
        .iter()
        .filter_map(|row| {
            matches!(row.output, UseCoordinate::TerminatorOperand { block, .. } if block == b(0))
                .then_some(row.input)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        condition_and_arguments,
        vec![
            UseCoordinate::TerminatorOperand {
                block: b(0),
                operand: 0
            },
            UseCoordinate::TerminatorOperand {
                block: b(0),
                operand: 1
            },
            UseCoordinate::TerminatorOperand {
                block: b(0),
                operand: 3
            },
        ]
    );
    let body = output.owner().module().functions[0].body.as_ref().unwrap();
    assert_eq!(
        body.parameters.len(),
        4,
        "unused ABI arguments are never DCE block parameters"
    );
    assert_eq!(body.blocks[1].parameters.len(), 1);
    assert!(output.report().passes().iter().any(|pass| pass.changed()));
    let (legacy, legacy_report, legacy_map) = legacy(&input);
    assert_eq!(
        legacy.canonical().canonical_bytes(),
        output.owner().canonical().canonical_bytes()
    );
    assert_eq!(&legacy_report, output.report());
    assert_eq!(
        &legacy_map,
        output.map(),
        "neutral hooks do not change existing target events or bytes"
    );
}

#[test]
fn zero_result_branch_replacement_preserves_the_selected_duplicate_occurrence() {
    for (condition, selected) in [(false, 1), (true, 0)] {
        let input = owner(&duplicate_edges(Some(condition)));
        let output = execute(&input);
        let rows = output.occurrences().candidate();
        assert_eq!(rows.blocks.len(), 1);
        assert_eq!(rows.segments.len(), 2);
        assert_eq!(rows.segments[0].input, b(0));
        assert_eq!(
            rows.segments[0].connector,
            Some(Edge {
                source: b(0),
                successor: selected
            })
        );
        assert_eq!(rows.segments[1].input, b(1));
        assert_eq!(rows.segments[1].connector, None);
        assert!(rows.edges.is_empty());
        assert_eq!(rows.uses.len(), 1);
        assert_eq!(
            rows.uses[0].input,
            UseCoordinate::TerminatorOperand {
                block: b(1),
                operand: 0
            }
        );
        let original_join = rows
            .definitions
            .iter()
            .find(|row| {
                row.input
                    == Definition::BlockArgument {
                        block: b(1),
                        argument: 0,
                    }
            })
            .unwrap();
        let descendant = rows.definition_outputs[original_join.outputs.start as usize];
        assert_eq!(original_join.outputs.len, 1);
        assert_eq!(descendant.kind, DescendantKind::Substituted);
        assert_eq!(
            descendant.output,
            Definition::FunctionArgument {
                function: FunctionCoordinate(0),
                argument: selected
            }
        );
    }
}

#[test]
fn materialized_checked_results_and_surviving_arithmetic_have_distinct_origins() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::effect_free(
        value(0),
        OperationKind::Constant(Constant::U32(9)),
    ));
    entry.operations.push(Operation::effect_free(
        value(1),
        OperationKind::Constant(Constant::U32(7)),
    ));
    entry.operations.push(Operation::checked_binary(
        value(2),
        boolean(3),
        CheckedBinaryOperator::Add,
        ValueId(0),
        ValueId(1),
    ));
    entry.operations.push(Operation::effect_free(
        value(4),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
    ));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(2), ValueId(3), ValueId(4)],
    });
    let mut source = Module::new("checked-and-retained-results");
    source.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![u32_type(), Type::BOOL, u32_type()]),
        vec![],
        vec![entry],
    ));
    let input = owner(&source);
    let output = execute(&input);
    let rows = output.occurrences().candidate();
    let original = |operation, result| Definition::Result {
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: b(0),
            operation,
        },
        result,
    };
    for result in 0..2 {
        let row = rows
            .definitions
            .iter()
            .find(|row| row.input == original(2, result))
            .unwrap();
        let descendants = &rows.definition_outputs
            [row.outputs.start as usize..(row.outputs.start + row.outputs.len) as usize];
        assert!(!descendants.is_empty());
        assert!(
            descendants
                .iter()
                .all(|d| d.kind == DescendantKind::Substituted)
        );
        assert!(
            rows.operations
                .iter()
                .any(|op| op.origin == Origin::ConstantFrom(original(2, result)))
        );
    }
    let row = rows
        .definitions
        .iter()
        .find(|row| row.input == original(3, 0))
        .unwrap();
    let descendants = &rows.definition_outputs
        [row.outputs.start as usize..(row.outputs.start + row.outputs.len) as usize];
    assert_eq!(
        descendants
            .iter()
            .filter(|d| d.kind == DescendantKind::Retained)
            .count(),
        1
    );
    assert_eq!(
        descendants
            .iter()
            .filter(|d| d.kind == DescendantKind::Substituted)
            .count(),
        1
    );
    assert!(
        descendants
            .windows(2)
            .all(|pair| pair[0].output < pair[1].output)
    );
    assert!(
        rows.operations
            .iter()
            .any(|op| matches!(op.origin, Origin::Retained(c) if c.operation == 3))
    );
}

#[test]
fn abandoned_lease_and_unobserved_operand_replacement_cannot_publish() {
    let input = owner(&duplicate_edges(None));
    for abandon in [true, false] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(29 + input.storage).unwrap();
        let (mut graph, storage) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let lease = graph
            .execute_production_neutral_optimization_v1(&mut budget)
            .unwrap();
        if abandon {
            drop(lease);
        } else {
            let roster = lease
                .capture
                .as_ref()
                .unwrap()
                .with_roster_meter(|meter| lease.graph.neutral_live_roster_v1(1000, meter))
                .unwrap();
            let entry = roster
                .iter()
                .find_map(|&(key, endpoint)| match (key, endpoint) {
                    (
                        LiveKeyV12::Operation(raw),
                        Endpoint::Operation(Coordinate::Terminator {
                            function: 0,
                            block: 0,
                        }),
                    ) => Some(raw),
                    _ => None,
                })
                .unwrap();
            let raw = entry.deref(&lease.graph.session.context);
            let other = raw.get_operand(2);
            drop(raw);
            // A direct API mutation deliberately bypasses every observer. The
            // fresh graph remains type-correct; the occurrence census must reject.
            pliron::operation::Operation::replace_operand(
                entry,
                &lease.graph.session.context,
                1,
                other,
            );
            assert!(matches!(
                lease.extract(),
                Err(KirNeutralOptimizationErrorV1::Occurrences(
                    KirOptimizationMapErrorV12::Coverage
                ))
            ));
        }
        assert!(graph.validate_custody_v12().is_err());
        assert!(
            graph
                .extract_optimized_canonical_kir_module_v12(&mut budget)
                .is_err()
        );
        let retained = graph.retained_storage();
        drop(graph);
        budget.release_storage(retained).unwrap();
        budget.release_storage(input.storage).unwrap();
        assert_eq!(budget.storage(), 29);
    }
}

#[test]
fn empty_and_declaration_owners_preserve_exact_definition_rosters() {
    for declarations in [false, true] {
        let mut source = Module::new("m");
        if declarations {
            source.functions.push(Function::declaration(
                "external",
                Signature::new(vec![u32_type(), Type::BOOL], vec![]),
            ));
        }
        let input = owner(&source);
        let output = execute(&input);
        assert_eq!(
            input.canonical().canonical_bytes(),
            output.owner().canonical().canonical_bytes()
        );
        let rows = output.occurrences().candidate();
        assert!(rows.blocks.is_empty() && rows.operations.is_empty() && rows.uses.is_empty());
        assert_eq!(rows.definitions.len(), if declarations { 2 } else { 0 });
        for (i, row) in rows.definitions.iter().enumerate() {
            assert_eq!(
                row.input,
                Definition::FunctionArgument {
                    function: FunctionCoordinate(0),
                    argument: i as u32
                }
            );
            assert_eq!(row.outputs.len, 1);
            let descendant = rows.definition_outputs[row.outputs.start as usize];
            assert_eq!(descendant.output, row.input);
            assert_eq!(descendant.kind, DescendantKind::Retained);
        }
    }
}

#[test]
fn execute_envelope_exact_and_one_under_preserve_nonzero_prefixes() {
    let input = owner(&Module::new("m"));
    assert_eq!(input.canonical().canonical_bytes().len(), 37);
    // Empty census: F=B=O=V=R=U=E=D=C=Q=0, N=1, event/target caps=8.
    // Allocation-free root census=4; W=32*1*8+256*1=512.
    // S=2048*1+32*8+128*8+8192=11520, independently derived.
    const CENSUS_WORK: usize = 4;
    const NEUTRAL_WORK: usize = 512;
    const NEUTRAL_STORAGE: usize = 11_520;
    // Native envelope for B=37: V=32,806; observer N=1, E=T=8.
    // Observer W=64*(1+1)=128 and S=1536+4096=5632.
    const EXECUTION_WORK: usize = 27_367_936;
    const EXECUTION_PERSISTENT: usize = 272_176;
    const EXECUTION_TEMPORARY: usize = 528_992;
    let report = std::mem::size_of::<PlironOptimizationReportV1>()
        + 7 * std::mem::size_of::<crate::PlironOptimizationPassReportV1>();
    let profile = Limits::for_structure(Default::default()).unwrap();
    assert_eq!(profile.work().unwrap(), NEUTRAL_WORK);
    assert_eq!(profile.storage().unwrap(), NEUTRAL_STORAGE);
    for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
        let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let (mut graph, imported) = KirPlironGraphV12::import(&input, &mut setup).unwrap();
        let floor = 31 + input.storage + imported.retained_storage();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(
            13 + CENSUS_WORK + NEUTRAL_WORK + EXECUTION_WORK - work_under,
        );
        let mut budget = Budget::new(
            &mut work,
            floor + NEUTRAL_STORAGE + EXECUTION_PERSISTENT + EXECUTION_TEMPORARY + report
                - storage_under,
        );
        budget.charge_work(13).unwrap();
        budget.reserve_storage(floor).unwrap();
        match graph.execute_production_neutral_optimization_v1(&mut budget) {
            Ok(lease) => {
                assert_eq!((work_under, storage_under), (0, 0));
                assert_eq!(
                    lease.budget.storage(),
                    floor + NEUTRAL_STORAGE + EXECUTION_PERSISTENT + report
                );
                drop(lease);
            }
            Err(error) => {
                assert_ne!((work_under, storage_under), (0, 0));
                assert!(matches!(
                    error,
                    KirNeutralOptimizationErrorV1::Execution(
                        PlironOptimizationErrorV12::Resources(_)
                    )
                ));
            }
        }
        let retained = graph.retained_storage();
        drop(graph);
        budget.release_storage(retained).unwrap();
        budget.release_storage(input.storage).unwrap();
        assert_eq!(budget.storage(), 31);
    }
}

#[test]
fn default_only_switch_connectors_and_unreachable_cycles_are_observed() {
    for integer in [false, true] {
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
        let mut join = BasicBlock::new(BlockId(30));
        join.parameters.push(value(2));
        join.terminator = Some(Terminator::Return {
            values: vec![ValueId(2)],
        });
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
        let mut source = Module::new("switch-connector-and-dead-cycle");
        source.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![u32_type(), u32_type()], vec![u32_type()]),
            vec![ValueId(0), ValueId(1)],
            vec![entry, join, dead_a, dead_b],
        ));
        let input = owner(&source);
        let output = execute(&input);
        let rows = output.occurrences().candidate();
        assert_eq!(rows.blocks.len(), 1);
        assert_eq!(rows.segments.len(), 2);
        assert_eq!(
            rows.segments[0].connector,
            Some(Edge {
                source: b(0),
                successor: 0
            })
        );
        assert_eq!(rows.segments[1].input, b(1));
        assert!(rows.edges.is_empty());
        assert_eq!(
            rows.uses.len(),
            1,
            "the default-only selector was explicitly retired"
        );
        assert_eq!(
            rows.uses[0].input,
            UseCoordinate::TerminatorOperand {
                block: b(1),
                operand: 0
            }
        );
        assert_eq!(
            output.owner().module().functions[0]
                .signature
                .parameters
                .len(),
            2
        );
    }
}

#[test]
fn missing_occurrence_cannot_hide_inside_a_later_erase_notification() {
    let input = owner(&duplicate_edges(None));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input.storage).unwrap();
    let (mut graph, imported) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(imported.retained_storage()).unwrap();
    let limits = graph.neutral_occurrence_limits_v1(&mut budget).unwrap();
    budget.charge_work(limits.work().unwrap()).unwrap();
    budget.reserve_storage(limits.storage().unwrap()).unwrap();
    let capture = graph.begin_neutral_occurrence_capture_v1(limits).unwrap();
    let old_limits = crate::kir_optimization_map_v12::CaptureLimitsV12::for_bytes(
        input.canonical().canonical_bytes().len(),
    )
    .unwrap();
    budget.charge_work(old_limits.work().unwrap()).unwrap();
    budget
        .reserve_storage(old_limits.storage().unwrap())
        .unwrap();
    let old = graph.begin_optimization_capture_v12().unwrap();
    let epoch = graph
        .session
        .operation_graph_snapshot_v1(&graph.root)
        .unwrap()
        .epoch();
    let pass = crate::KIR_PLIRON_PRODUCTION_PASSES_V12[0];
    assert!(capture.begin_pass(pass, epoch));
    assert!(old.begin_pass(pass, epoch));
    let roster = capture
        .with_roster_meter(|meter| graph.neutral_live_roster_v1(limits.nodes, meter))
        .unwrap();
    let term = roster
        .iter()
        .find_map(|&(key, endpoint)| match (key, endpoint) {
            (
                LiveKeyV12::Operation(raw),
                Endpoint::Operation(Coordinate::Terminator {
                    function: 0,
                    block: 0,
                }),
            ) => Some(raw),
            _ => None,
        })
        .unwrap();
    let other = term.deref(&graph.session.context).get_operand(3);
    pliron::operation::Operation::replace_operand(term, &graph.session.context, 1, other);
    let mut observer = capture.observer(old.observer());
    observer.observe(
        &graph.session.context,
        pliron::irbuild::observer::RewriteEvent::OperationErased(term),
    );
    assert_eq!(
        capture.failure(),
        Some(KirOptimizationMapErrorV12::Coverage)
    );
    assert!(!capture.end_pass(&graph.session.context, epoch));
    // The hostile notification did not perform a pass or admit a result.
    drop(observer);
    drop(roster);
    drop(capture);
    drop(old);
    drop(graph);
    budget
        .release_storage(
            limits.storage().unwrap()
                + old_limits.storage().unwrap()
                + imported.retained_storage()
                + input.storage,
        )
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn definition_target_cap_is_exact_without_observed_peak_calibration() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(0), ValueId(1)],
    });
    let mut source = Module::new("two-retained-definitions");
    source.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![u32_type(), u32_type()], vec![u32_type(), u32_type()]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    let input = owner(&source);
    for target_limit in [2, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(43 + input.storage).unwrap();
        let (mut graph, imported) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
        budget.reserve_storage(imported.retained_storage()).unwrap();
        let mut limits = graph.neutral_occurrence_limits_v1(&mut budget).unwrap();
        // Only the private capture test changes its admission cap. The public
        // neutral executor has no resource-policy/transform selector.
        limits.targets = target_limit;
        budget.charge_work(limits.work().unwrap()).unwrap();
        budget.reserve_storage(limits.storage().unwrap()).unwrap();
        let capture = graph.begin_neutral_occurrence_capture_v1(limits).unwrap();
        let (report, execution) = graph
            .execute_production_optimization_with_occurrences_v1(&mut budget, Some(&capture))
            .unwrap();
        budget
            .reserve_storage(execution.retained_storage())
            .unwrap();
        let (output, bridge, map, extracted) = graph
            .extract_optimized_canonical_kir_module_with_map_v12(&mut budget)
            .unwrap();
        budget
            .reserve_storage(extracted.retained_storage())
            .unwrap();
        budget.reserve_storage(limits.storage().unwrap()).unwrap();
        let roster = capture
            .with_roster_meter(|meter| graph.neutral_live_roster_v1(limits.nodes, meter))
            .unwrap();
        let rows = capture.finish(&graph.session.context, &roster, &map, output.module());
        if target_limit == 2 {
            let rows = rows.unwrap();
            let candidate = rows.candidate();
            assert_eq!(candidate.definitions.len(), 2);
            assert_eq!(candidate.definition_outputs.len(), 2);
            assert!(
                candidate
                    .definition_outputs
                    .iter()
                    .all(|row| row.kind == DescendantKind::Retained)
            );
            assert!(report.passes().iter().all(|pass| !pass.changed()));
            drop(rows);
        } else {
            assert!(matches!(rows, Err(KirOptimizationMapErrorV12::Limit)));
            assert_eq!(capture.failure(), Some(KirOptimizationMapErrorV12::Limit));
        }
        drop(roster);
        budget.release_storage(limits.storage().unwrap()).unwrap();
        drop(output);
        drop(bridge);
        drop(map);
        drop(report);
        budget
            .release_storage(extracted.retained_storage() + execution.retained_storage())
            .unwrap();
        drop(capture);
        budget.release_storage(limits.storage().unwrap()).unwrap();
        let graph_storage = graph.retained_storage();
        drop(graph);
        budget
            .release_storage(graph_storage + input.storage)
            .unwrap();
        assert_eq!(budget.storage(), 43);
    }
}

#[test]
fn wrapper_receipt_counts_only_uncovered_inline_payload_and_profile_overflow_rejects() {
    let components = std::mem::size_of::<Owner>()
        + std::mem::size_of::<PlironOptimizationReportV1>()
        + std::mem::size_of::<KirBridgeOptimizedReceiptV1>()
        + std::mem::size_of::<KirOptimizationMapV12>()
        + std::mem::size_of::<KirNeutralOccurrenceRowsV1>();
    let remainder = output_wrapper_storage_v1().unwrap();
    assert_eq!(
        components + remainder,
        std::mem::size_of::<KirNeutralOptimizationOutputV1<'_>>()
    );
    assert!(
        remainder
            >= std::mem::size_of::<&Owner>()
                + std::mem::size_of::<KirNeutralOptimizationStorageV1>()
    );
    assert!(matches!(
        Limits::for_structure(crate::kir_occurrence_capture_v1::StructuralCensus {
            values: usize::MAX,
            ..Default::default()
        }),
        Err(KirOptimizationMapErrorV12::Arithmetic)
    ));
}

#[test]
fn final_coordinate_roster_lookup_work_is_exact_and_precharged() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(0), ValueId(1)],
    });
    let mut source = Module::new("metered-coordinate-roster");
    source.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![u32_type(), u32_type()], vec![u32_type(), u32_type()]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    let input = owner(&source);
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let (graph, imported) = KirPlironGraphV12::import(&input, &mut setup).unwrap();
    let scratch = graph
        .neutral_occurrence_limits_v1(&mut setup)
        .unwrap()
        .storage()
        .unwrap();
    for under in [0, 1] {
        // 4*F+4 root/function-index units, F=1; then 1 function,
        // 1 block, 2 arguments, 1 terminator, and 0 result lookups = 13.
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + 13 - under);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(5).unwrap();
        budget
            .reserve_storage(47 + input.storage + imported.retained_storage() + scratch)
            .unwrap();
        let result = graph.neutral_live_roster_v1(3, &mut |units| {
            budget
                .charge_work(units)
                .map_err(KirOptimizationMapErrorV12::from)
        });
        if under == 0 {
            let roster = result.unwrap();
            assert_eq!(roster.len(), 3);
            drop(roster);
        } else {
            assert!(matches!(
                result,
                Err(KirOptimizationMapErrorV12::Resources(_))
            ));
        }
        budget.release_storage(scratch).unwrap();
        // Input/graph custody is explicitly transferred back to the enclosing
        // setup scope after this isolated coordinate-roster budget audit.
        budget
            .release_storage(input.storage + imported.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 47);
        assert!(graph.validate_custody_v12().is_ok());
    }
    drop(graph);
}

#[test]
fn structural_profiles_are_independent_of_serialized_metadata() {
    let mut source = duplicate_edges(None);
    let mut padded = duplicate_edges(None);
    for module in [&mut source, &mut padded] {
        for ordinal in 0..17 {
            module.functions.push(Function::declaration(
                format!("external_{ordinal:02}"),
                Signature::new(vec![], vec![]),
            ));
        }
    }
    // Keep each identity below the existing 4096-byte canonical bound while
    // making total inert identity bytes exceed 64KiB across both graph shapes.
    padded.id = format!("module_{}", "x".repeat(4_000)).into();
    for (ordinal, function) in padded.functions.iter_mut().enumerate() {
        function.id = format!("function_{ordinal:02}_{}", "y".repeat(4_000)).into();
    }
    let short = owner(&source);
    let long = owner(&padded);
    assert!(
        long.canonical().canonical_bytes().len()
            > short.canonical().canonical_bytes().len() + 65_000
    );
    let mut profiles = Vec::new();
    for input in [&short, &long] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(53 + input.storage).unwrap();
        let (graph, imported) = KirPlironGraphV12::import(input, &mut budget).unwrap();
        budget.reserve_storage(imported.retained_storage()).unwrap();
        let before_work = budget.work();
        let before_storage = budget.storage();
        let profile = graph.neutral_occurrence_limits_v1(&mut budget).unwrap();
        assert_eq!(budget.work() - before_work, 27);
        assert_eq!(budget.storage(), before_storage);
        profiles.push(profile);
        drop(graph);
        budget
            .release_storage(imported.retained_storage() + input.storage)
            .unwrap();
        assert_eq!(budget.storage(), 53);
    }
    assert_eq!(profiles[0], profiles[1]);
    // The 17 empty declarations add only 17 function claims to the N=66 body.
    assert_eq!(profiles[0].nodes, 83);
    assert_eq!(profiles[0].storage().unwrap(), 284_416);
}

#[test]
fn structural_census_profiles_and_exact_work_are_derived_from_physical_arities() {
    let mut declaration = Module::new("declared");
    declaration.functions.push(Function::declaration(
        "external",
        Signature::new(vec![u32_type(), Type::BOOL], vec![]),
    ));
    // Tuple columns: source, census work, N, execution work, capture storage.
    // Census work = 4 + F + defined_F + B + O; no operand/result traversal.
    // W=256*(N*N+N); S=3328*N+8192. These are equations, not measured peaks.
    for (source, census_work, nodes, execution_work, storage) in [
        (Module::new("m"), 4, 1, 512, 11_520),
        (declaration, 5, 3, 3_072, 18_176),
        // F=1 B=2 O=2 V=6 R=0 U=6 E=2 D=0 C=1 Q=5:
        // I=35, growth=18+3+10=31, N=66.
        (duplicate_edges(None), 10, 66, 1_132_032, 227_840),
        // One original constant replaces an entry argument: O=3,V=6,R=1.
        (duplicate_edges(Some(true)), 11, 68, 1_201_152, 234_496),
    ] {
        let input = owner(&source);
        let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let (graph, imported) = KirPlironGraphV12::import(&input, &mut setup).unwrap();
        for under in [0, 1] {
            let floor = 59 + input.storage + imported.retained_storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(7 + census_work - under);
            let mut budget = Budget::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(7).unwrap();
            let result = graph.neutral_occurrence_limits_v1(&mut budget);
            if under == 0 {
                let limits = result.unwrap();
                assert_eq!(limits.nodes, nodes);
                assert_eq!(limits.events, 8 * nodes);
                assert_eq!(limits.targets, 8 * nodes);
                assert_eq!(limits.work().unwrap(), execution_work);
                assert_eq!(limits.storage().unwrap(), storage);
                assert_eq!(budget.work(), 7 + census_work);
            } else {
                assert!(matches!(
                    result,
                    Err(KirOptimizationMapErrorV12::Resources(ResourceError::Work(
                        _
                    )))
                ));
            }
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor);
            assert!(!graph.optimization_started);
            assert!(graph.validate_custody_v12().is_ok());
            budget
                .release_storage(input.storage + imported.retained_storage())
                .unwrap();
            assert_eq!(budget.storage(), 59);
        }
        drop(graph);
    }
}

#[test]
fn structural_node_cap_is_admitted_exactly_and_never_clamped() {
    use crate::kir_occurrence_capture_v1::StructuralCensus;
    let exact = Limits::for_structure(StructuralCensus {
        functions: 262_144,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(exact.nodes, 262_144);
    assert!(matches!(
        Limits::for_structure(StructuralCensus {
            functions: 262_145,
            ..Default::default()
        }),
        Err(KirOptimizationMapErrorV12::Limit)
    ));
    assert!(matches!(
        Limits::for_structure(StructuralCensus {
            operands: usize::MAX,
            ..Default::default()
        }),
        Err(KirOptimizationMapErrorV12::Arithmetic)
    ));
}
