use super::super::storage_v18::tests::{Input, LIMITS, SPACE, WORK, diamond, fixture, input};
use super::*;
use fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CastKind, Constant, Function, Module, Operation, OperationKind as Op, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId,
};

const U32: Type = Type::Scalar(ScalarType::U32);

fn observed<'a>(
    source: &'a Input,
    budget: &mut Budget<'_>,
) -> KirNeutralOptimizationOutputMixedPureCseV18<'a> {
    let floor = budget.storage();
    let value =
        optimize_neutral_kernel_ir_mixed_pure_cse_v18(&source.owner, LIMITS, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(value.storage().retained_storage())
        .unwrap();
    value
}

fn operations(owner: &Owner) -> impl Iterator<Item = &Operation> {
    owner
        .module()
        .functions
        .iter()
        .flat_map(|f| f.body.iter())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
}

fn xor_count(owner: &Owner) -> usize {
    operations(owner)
        .filter(|op| {
            matches!(
                op.kind,
                Op::Binary {
                    op: BinaryOp::BitXor,
                    ..
                }
            )
        })
        .count()
}

#[test]
fn mixed_pure_cse_executes_four_passes_without_changing_policy9_or_memory_effects() {
    let source = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    let floor = source.storage + 29;
    budget.reserve_storage(floor).unwrap();
    let historical =
        crate::optimize_neutral_kernel_ir_integer_worklist_v18(&source.owner, LIMITS, &mut budget)
            .unwrap();
    assert_eq!(historical.execution().policy_version(), 9);
    assert_eq!(xor_count(historical.owner()), 2);
    drop(historical);
    let value = observed(&source, &mut budget);
    assert_eq!(xor_count(value.owner()), 1);
    assert_eq!(value.execution().policy_version(), 10);
    assert_eq!(value.execution().graph_schema(), 18);
    assert!(!value.execution().grants_authority());
    assert_eq!(
        value
            .report()
            .passes()
            .iter()
            .map(|row| row.pass())
            .collect::<Vec<_>>(),
        crate::fixed_policy_v3::POLICY10_PASSES
    );
    assert_eq!(
        &value.execution().canonical_bytes()[..8],
        &[10, 0, 1, 0, 4, 0, 18, 0]
    );
    assert_eq!(value.execution().canonical_bytes().len(), 576);
    assert!(value.map().matches_execution(value.report()));
    value
        .map()
        .check_against(&source.owner, value.owner(), &mut budget)
        .unwrap();
    assert_eq!(
        source.owner.module().storage_layouts,
        value.owner().module().storage_layouts
    );
    // CSE/DCE do not acquire permission to merge memory or execution effects,
    // or to discard an ordinary arithmetic operation with overflow semantics.
    for predicate in [
        (|op: &Operation| matches!(op.kind, Op::Storage(_))) as fn(&Operation) -> bool,
        |op: &Operation| matches!(op.kind, Op::Execution(_)),
        |op: &Operation| {
            matches!(
                op.kind,
                Op::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
        },
    ] {
        assert_eq!(
            operations(&source.owner).filter(|op| predicate(op)).count(),
            operations(value.owner()).filter(|op| predicate(op)).count()
        );
    }
    let checked = value.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(checked.input_audit_bytes(), source.owner.canonical_bytes());
    assert!(!checked.grants_authority());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn mixed_pure_cse_checks_dominance_and_preserves_loop_and_sibling_cfgs() {
    let mut siblings = diamond(false);
    let body = siblings.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.clear();
    body.blocks[2].operations = vec![Operation::effect_free(
        ValueDef::new(ValueId(12), U32),
        Op::Binary {
            op: BinaryOp::BitXor,
            lhs: ValueId(1),
            rhs: ValueId(2),
        },
    )];
    body.blocks[2].terminator = Some(Terminator::Return {
        values: vec![ValueId(12)],
    });
    for (module, expected_xor) in [
        (diamond(false), Some(1)),
        (diamond(true), None),
        (siblings, Some(2)),
    ] {
        let source = input(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        let floor = source.storage + 31;
        budget.reserve_storage(floor).unwrap();
        let value = observed(&source, &mut budget);
        if let Some(expected) = expected_xor {
            assert_eq!(xor_count(value.owner()), expected);
        }
        let before = &source.owner.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        let after = &value.owner().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert_eq!(
            before.iter().map(|b| b.id).collect::<Vec<_>>(),
            after.iter().map(|b| b.id).collect::<Vec<_>>()
        );
        let candidate = value.occurrences().candidate();
        assert_eq!(candidate.blocks.len(), before.len());
        assert_eq!(candidate.segments.len(), before.len());
        assert!(candidate.segments.iter().all(|row| row.connector.is_none()));
        assert!(candidate.edges.iter().all(|row| row.input == row.output));
        value.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn mixed_pure_cse_keeps_constant_branch_and_duplicate_destination_occurrences() {
    for selected in [false, true] {
        let mut module = diamond(false);
        let body = module.functions[0].body.as_mut().unwrap();
        if selected {
            body.blocks[0].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(9), Type::BOOL),
                Op::Constant(Constant::Bool(true)),
            ));
            let Some(Terminator::ConditionalBranch { condition, .. }) =
                &mut body.blocks[0].terminator
            else {
                unreachable!()
            };
            *condition = ValueId(9);
        } else {
            body.blocks.truncate(2);
            body.blocks[1].parameters = vec![ValueDef::new(ValueId(20), U32)];
            body.blocks[1].operations.clear();
            body.blocks[1].terminator = Some(Terminator::Return {
                values: vec![ValueId(20)],
            });
            body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(0),
                then_target: BlockId(1),
                then_arguments: vec![ValueId(1)],
                else_target: BlockId(1),
                else_arguments: vec![ValueId(2)],
            });
        }
        let source = input(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(source.storage + 37).unwrap();
        let value = observed(&source, &mut budget);
        assert_eq!(
            value.owner().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .len(),
            if selected { 3 } else { 2 }
        );
        let candidate = value.occurrences().candidate();
        assert_eq!(candidate.edges.len(), 2);
        assert_ne!(candidate.edges[0].output, candidate.edges[1].output);
        if !selected {
            assert_eq!(candidate.edge_arguments.len(), 2);
        }
        value.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(budget.storage(), source.storage + 37);
    }
}

#[test]
fn mixed_pure_cse_checks_complete_rows_and_same_count_substitutions_independently() {
    let source = input(&fixture());
    let foreign = input(&diamond(false));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    let floor = source.storage + foreign.storage + 41;
    budget.reserve_storage(floor).unwrap();
    let value = observed(&source, &mut budget);
    assert_eq!(
        value
            .map()
            .check_against(&foreign.owner, value.owner(), &mut budget),
        Err(crate::KirOptimizationMapErrorV12::Identity)
    );
    let (a, ac) = CanonicalKirInventoryV18::derive_v18(&source.owner, &mut budget).unwrap();
    budget.reserve_storage(ac.retained_storage()).unwrap();
    let (b, bc) = CanonicalKirInventoryV18::derive_v18(value.owner(), &mut budget).unwrap();
    budget.reserve_storage(bc.retained_storage()).unwrap();
    let candidate = value.occurrences().candidate();
    check_canonical_kir_transition_v18(&a, &b, candidate, &mut budget).unwrap();
    let mut missing = candidate;
    missing.functions = &[];
    assert!(matches!(
        check_canonical_kir_transition_v18(&a, &b, missing, &mut budget),
        Err(CanonicalKirTransitionErrorV1::IncompleteRows)
    ));
    let mut descendants = candidate.definition_outputs.to_vec();
    let different = descendants
        .iter()
        .position(|row| row.output != descendants[0].output)
        .unwrap();
    descendants[0].output = descendants[different].output;
    let mut wrong = candidate;
    wrong.definition_outputs = &descendants;
    assert!(check_canonical_kir_transition_v18(&a, &b, wrong, &mut budget).is_err());
    let mut uses = candidate.uses.to_vec();
    assert!(uses.len() > 1);
    uses[0].input = uses[1].input;
    let mut wrong = candidate;
    wrong.uses = &uses;
    assert!(check_canonical_kir_transition_v18(&a, &b, wrong, &mut budget).is_err());
    drop(b);
    drop(a);
    budget
        .release_storage(ac.retained_storage() + bc.retained_storage())
        .unwrap();
    value.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn mixed_pure_cse_actual_report_cannot_be_replaced_by_policy9_at_identical_endpoints() {
    let source = input(&Module::new("same-noop-endpoints"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(source.storage + 43).unwrap();
    let value = observed(&source, &mut budget);
    let historical =
        crate::optimize_neutral_kernel_ir_integer_worklist_v18(&source.owner, LIMITS, &mut budget)
            .unwrap();
    let historical_credit = historical.storage().retained_storage();
    budget.reserve_storage(historical_credit).unwrap();
    assert_eq!(
        value.owner().canonical_bytes(),
        historical.owner().canonical_bytes()
    );
    assert!(!value.map().matches_execution(historical.report()));
    let table = source
        .owner
        .storage_table_identity_with_budget_v18(&mut budget)
        .unwrap();
    let (resources, _) = crate::optimization_v12::policy3_execution_resources_v1(
        source.owner.canonical_bytes().len(),
        1,
    )
    .unwrap();
    let before = budget.work();
    let result = MixedPureCseExecutionWitnessV18::from_execution(
        &source.owner,
        value.owner(),
        table,
        historical.report(),
        value.map(),
        crate::fixed_policy_v3::ExecutionProfileV1 {
            resources,
            registered_nodes: 1,
            cse_work: 0,
        },
        &mut budget,
    );
    assert!(matches!(result, Err(Resource::Accounting)));
    assert_eq!(budget.work() - before, 12);
    drop(historical);
    budget.release_storage(historical_credit).unwrap();
    value.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), source.storage + 43);
}

#[test]
fn mixed_pure_cse_has_exact_and_one_short_execution_limits() {
    let source = input(&Module::new("empty-mixed-pure-cse-boundary"));
    let floor = source.storage + 47;
    let run = |work_limit: usize, storage_limit: usize| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result =
            optimize_neutral_kernel_ir_mixed_pure_cse_v18(&source.owner, LIMITS, &mut budget);
        let ok = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        let state = (
            ok,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        );
        drop(budget);
        (state, work.failed_work())
    };
    let (full, failure) = run(WORK, SPACE);
    assert!(full.0);
    assert_eq!(failure, None);
    assert_eq!(run(full.1, full.2).0, full);
    let short_work = run(full.1 - 1, full.2);
    assert!(!short_work.0.0);
    assert!(short_work.1.is_some());
    assert_eq!(short_work.0.3, None);
    let short_storage = run(full.1, full.2 - 1);
    assert!(!short_storage.0.0);
    assert!(short_storage.0.3.is_some());
    assert_eq!(short_storage.1, None);
}

#[test]
fn mixed_pure_cse_adoption_rejects_foreign_ledgers_without_changing_either_floor() {
    let source = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(source.storage + 53).unwrap();
    let value = observed(&source, &mut budget);
    let incoming = budget.storage();
    let mut other_work = Work::new(WORK);
    let mut other = Budget::new(&mut other_work, SPACE);
    other.reserve_storage(incoming).unwrap();
    assert!(matches!(
        value.try_check_and_finish_v18(&mut other),
        Err(KirCheckedNeutralOptimizationErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(budget.storage(), incoming);
    assert_eq!(other.storage(), incoming);
}

#[test]
fn mixed_pure_cse_noop_multiple_functions_and_repeated_fixture_execution_are_nominal() {
    let mut multiple = fixture();
    multiple.functions.extend(diamond(false).functions);
    for module in [Module::new("empty"), fixture(), multiple] {
        let source = input(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        let floor = source.storage + 59;
        budget.reserve_storage(floor).unwrap();
        let first = observed(&source, &mut budget)
            .try_check_and_finish_v18(&mut budget)
            .unwrap();
        assert_eq!(
            first.owner().module().functions.len(),
            module.functions.len()
        );
        let bytes = first.owner().canonical_bytes().to_vec();
        let retained = first.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        let second =
            optimize_neutral_kernel_ir_mixed_pure_cse_v18(first.owner(), LIMITS, &mut budget)
                .unwrap();
        budget
            .reserve_storage(second.storage().retained_storage())
            .unwrap();
        assert_eq!(second.owner().canonical_bytes(), bytes);
        assert!(second.report().passes().iter().all(|row| !row.changed()));
        let second = second.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(second.execution().policy_version(), 10);
        drop(second);
        drop(first);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn mixed_pure_cse_preserves_slice_pointer_type_and_access_cast_distinctions() {
    let mut block = BasicBlock::new(BlockId(0));
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut arguments = Vec::new();
    let mut returned = Vec::new();
    for (index, scalar, access) in [
        (0, ScalarType::U32, AccessMode::ReadOnly),
        (1, ScalarType::U32, AccessMode::ReadWrite),
        (2, ScalarType::U64, AccessMode::ReadOnly),
    ] {
        for slice in [false, true] {
            let argument = 2 * index + u32::from(slice);
            let ty = |space| {
                if slice {
                    Type::slice(Type::Scalar(scalar), space, access)
                } else {
                    Type::pointer(Type::Scalar(scalar), space, access)
                }
            };
            inputs.push(ty(AddressSpace::Global));
            arguments.push(ValueId(argument));
            for duplicate in 0..2 {
                let id = 10 + 2 * argument + duplicate;
                let to = ty(AddressSpace::Generic);
                outputs.push(to.clone());
                returned.push(ValueId(id));
                block.operations.push(Operation::effect_free(
                    ValueDef::new(ValueId(id), to.clone()),
                    Op::Cast {
                        kind: if slice {
                            CastKind::SliceToGeneric
                        } else {
                            CastKind::PointerToGeneric
                        },
                        value: ValueId(argument),
                        to,
                    },
                ));
            }
        }
    }
    block.terminator = Some(Terminator::Return { values: returned });
    let mut module = Module::new("mixed-type-preserving-cse");
    module.storage_layouts = fixture().storage_layouts;
    module.functions.push(Function::internal_helper(
        "expose",
        Signature::new(inputs, outputs),
        arguments,
        vec![block],
    ));
    let source = input(&module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(source.storage + 61).unwrap();
    let value = observed(&source, &mut budget);
    assert_eq!(
        operations(value.owner())
            .filter(|op| matches!(op.kind, Op::Cast { .. }))
            .count(),
        6
    );
    value.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), source.storage + 61);
}

#[test]
fn mixed_pure_cse_replays_checked_binary_synthesis_before_sharing_false_constants() {
    let mut block = BasicBlock::new(BlockId(0));
    block
        .operations
        .push(super::super::storage_v18::tests::constant(2, 0));
    for (result, flag, argument) in [(3, 4, 0), (5, 6, 1)] {
        block.operations.push(Operation::checked_binary(
            ValueDef::new(ValueId(result), U32),
            ValueDef::new(ValueId(flag), Type::BOOL),
            fe2o3_kernel_ir::CheckedBinaryOperator::Add,
            ValueId(argument),
            ValueId(2),
        ));
    }
    for id in [7, 8] {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), U32),
            Op::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ));
    }
    block.terminator = Some(Terminator::Return {
        values: (3..=8).map(ValueId).collect(),
    });
    let mut module = Module::new("mixed-checked-synthesis-cse");
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(
            vec![U32, U32],
            vec![U32, Type::BOOL, U32, Type::BOOL, U32, U32],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    let source = input(&module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(source.storage + 67).unwrap();
    let value = observed(&source, &mut budget);
    assert_eq!(xor_count(value.owner()), 1);
    assert_eq!(
        operations(value.owner())
            .filter(|op| matches!(op.kind, Op::Constant(Constant::Bool(false))))
            .count(),
        1
    );
    assert!(
        value
            .occurrences()
            .candidate()
            .operations
            .iter()
            .any(|row| matches!(
                row.origin,
                fe2o3_kernel_ir::CanonicalKirOperationOriginV1::ConstantFrom(_)
            ))
    );
    value.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), source.storage + 67);
}

#[test]
fn mixed_pure_cse_uncalled_hostile_capture_preserves_early_accounting_refusal() {
    use std::{cell::Cell, rc::Rc};
    struct Hostile(Rc<Cell<usize>>);
    impl Drop for Hostile {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("uncalled capture destructor");
        }
    }
    let source = input(&fixture());
    for fault in 0..3 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(source.storage + 71).unwrap();
        let value = observed(&source, &mut budget);
        let dropped = Rc::new(Cell::new(0));
        let hostile = Hostile(Rc::clone(&dropped));
        let result = match fault {
            0 => {
                let incoming = (budget.storage(), budget.work());
                let mut foreign_work = Work::new(WORK);
                let mut foreign = Budget::new(&mut foreign_work, SPACE);
                foreign.reserve_storage(incoming.0).unwrap();
                let before = (foreign.storage(), foreign.work());
                let result = value.try_check_and_finish_with_v18(&mut foreign, move |_, _| {
                    let _capture = hostile;
                    Ok::<_, Infallible>(((), 0))
                });
                assert_eq!((foreign.storage(), foreign.work()), before);
                assert_eq!((budget.storage(), budget.work()), incoming);
                result
            }
            1 => {
                budget.release_storage(1).unwrap();
                let before = (budget.storage(), budget.work());
                let result = value.try_check_and_finish_with_v18(&mut budget, move |_, _| {
                    let _capture = hostile;
                    Ok::<_, Infallible>(((), 0))
                });
                assert_eq!((budget.storage(), budget.work()), before);
                result
            }
            _ => {
                let slot = std::ptr::from_ref(&budget) as usize;
                let mut moved = Box::new(budget);
                assert_ne!(slot, std::ptr::from_ref(&*moved) as usize);
                let before = (moved.storage(), moved.work());
                let result = value.try_check_and_finish_with_v18(&mut moved, move |_, _| {
                    let _capture = hostile;
                    Ok::<_, Infallible>(((), 0))
                });
                assert_eq!((moved.storage(), moved.work()), before);
                result
            }
        };
        assert!(matches!(
            result,
            Err(KirCheckedNeutralOptimizationErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!(dropped.get(), 1);
    }
}

#[test]
fn mixed_pure_cse_actual_rewrite_adoption_has_exact_and_one_short_headroom() {
    let source = input(&fixture());
    let floor = source.storage + 73;
    let run = |work_headroom: usize, storage_headroom: usize| {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(floor).unwrap();
        let value = observed(&source, &mut budget);
        assert_eq!(xor_count(value.owner()), 1);
        // Normalize only the post-execution allowance. Separate Pliron contexts
        // legitimately have different pointer-index probe counts.
        let work_padding = WORK
            .checked_sub(budget.work())
            .unwrap()
            .checked_sub(work_headroom)
            .unwrap();
        budget.charge_work(work_padding).unwrap();
        let storage_padding = SPACE
            .checked_sub(budget.storage())
            .unwrap()
            .checked_sub(storage_headroom)
            .unwrap();
        let old_peak = budget.peak_storage();
        budget.reserve_storage(storage_padding).unwrap();
        assert!(budget.storage() > old_peak);
        let start = (budget.work(), budget.storage());
        let result = value.try_check_and_finish_v18(&mut budget);
        let selected = match &result {
            Ok(_) => None,
            Err(KirCheckedNeutralOptimizationErrorV1::Resource(error))
            | Err(KirCheckedNeutralOptimizationErrorV1::Inventory(
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error),
            ))
            | Err(KirCheckedNeutralOptimizationErrorV1::Transition(
                CanonicalKirTransitionErrorV1::Resource(error),
            )) => Some(error),
            Err(other) => panic!("unexpected adoption refusal: {other:?}"),
        };
        let resource = match selected {
            None => 0,
            Some(Resource::Work(_)) => 1,
            Some(Resource::Storage(_)) => 2,
            other => panic!("unexpected resource refusal: {other:?}"),
        };
        drop(result);
        assert_eq!(budget.storage(), floor + storage_padding);
        let state = (
            resource,
            budget.work() - start.0,
            budget.peak_storage() - start.1,
            budget.failed_storage(),
        );
        budget.release_storage(storage_padding).unwrap();
        assert_eq!(budget.storage(), floor);
        drop(budget);
        (state, work.failed_work())
    };
    let (full, failed) = run(WORK / 2, SPACE / 2);
    assert_eq!(full.0, 0);
    assert_eq!(failed, None);
    let exact = run(full.1, full.2);
    assert_eq!(exact.0, full);
    assert_eq!(exact.1, None);
    let short_work = run(full.1 - 1, full.2);
    assert_eq!(short_work.0.0, 1);
    assert!(short_work.1.is_some());
    assert_eq!(short_work.0.3, None);
    let short_storage = run(full.1, full.2 - 1);
    assert_eq!(short_storage.0.0, 2);
    assert!(short_storage.0.3.is_some());
    assert_eq!(short_storage.1, None);
}
