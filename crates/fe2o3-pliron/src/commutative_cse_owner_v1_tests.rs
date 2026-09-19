//! Actual import/pass/event/export/checker controls, not ordinary-source evidence.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirDefinitionDescendantKindV1 as DescendantKind, Constant, Function, MemoryAccess,
    Module, Operation, OperationKind as Kind, ScalarType, Signature, Terminator, Type, ValueDef,
    ValueId,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 2_000_000_000;
const FLOOR: usize = 113;
#[path = "owned_commutative_continuation_v1_tests.rs"]
mod owning_tests;
struct Input {
    owner: Owner,
    storage: usize,
}
fn own(module: &Module) -> Input {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    Input {
        owner,
        storage: storage.retained_storage(),
    }
}
fn expression(id: u32, ty: ScalarType, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ty)),
        Kind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn ret(block: &mut BasicBlock, value: u32) {
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(value)],
    });
}
fn branch(block: &mut BasicBlock, target: u32) {
    block.terminator = Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    });
}
fn module(ty: ScalarType, blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("arbitrary-component-subject");
    let ty = Type::Scalar(ty);
    module.functions.push(Function::internal_helper(
        "not_a_dispatch_key",
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        blocks,
    ));
    module
}
fn pair(ty: ScalarType, op: BinaryOp, swap: bool, cross: bool) -> (Module, Module) {
    let mut first = BasicBlock::new(BlockId(10));
    first.operations.push(expression(2, ty, op, 0, 1));
    let duplicate = expression(
        3,
        ty,
        op,
        if swap { 1 } else { 0 },
        if swap { 0 } else { 1 },
    );
    let (input, output) = if cross {
        branch(&mut first, 20);
        let mut second = BasicBlock::new(BlockId(20));
        second.operations.push(duplicate);
        ret(&mut second, 3);
        let input = vec![first.clone(), second.clone()];
        second.operations.clear();
        ret(&mut second, 2);
        (input, vec![first, second])
    } else {
        first.operations.push(duplicate);
        ret(&mut first, 3);
        let input = vec![first.clone()];
        first.operations.pop();
        ret(&mut first, 2);
        (input, vec![first])
    };
    (module(ty, input), module(ty, output))
}
fn run(input: &Input) -> (CheckedCommutativeBitwiseOptimizationV1<'_>, usize, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + input.storage;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let output = optimize_checked_commutative_bitwise_cse_v1(&input.owner, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(std::ptr::eq(output.input(), &input.owner));
    assert!(!output.grants_authority());
    let observation = (budget.work(), budget.peak_storage());
    budget.reserve_storage(output.retained_storage()).unwrap();
    assert_eq!(output.replay(&mut budget).unwrap(), output.proved_pairs());
    budget.release_storage(output.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    (output, observation.0, observation.1)
}
fn assert_output(input: Module, expected: Module, count: usize) {
    let input = own(&input);
    let expected = own(&expected);
    let (actual, _, _) = run(&input);
    assert_eq!(
        actual.owner().canonical().canonical_bytes(),
        expected.owner.canonical().canonical_bytes()
    );
    assert_eq!(actual.proved_pairs(), count);
    assert_eq!(actual.execution().changed(), count != 0);
    assert_eq!(
        actual
            .occurrences()
            .candidate()
            .definition_outputs
            .iter()
            .filter(|row| row.kind == DescendantKind::Substituted)
            .count(),
        count
    );
}

#[test]
fn actual_service_all_fixed_integer_types_ops_orientations_and_dominance() {
    for ty in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for op in [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor] {
            for swap in [false, true] {
                for cross in [false, true] {
                    let (input, output) = pair(ty, op, swap, cross);
                    assert_output(input, output, 1);
                }
            }
        }
    }
}

fn reversed_chain(depth: u32) -> (Module, Module) {
    let ty = ScalarType::U32;
    let mut entry = BasicBlock::new(BlockId(0));
    let mut left = 0;
    for index in 0..depth {
        let id = 10 + index;
        entry
            .operations
            .push(expression(id, ty, BinaryOp::BitXor, left, 1));
        left = id;
    }
    branch(&mut entry, 1);
    let mut input = vec![entry.clone()];
    let mut output = vec![entry];
    // Execution is block1 -> block2 -> ...; physical omitted-definition order
    // is deepest first, forcing checker dependencies not to be pre-proved.
    for index in (0..depth).rev() {
        let mut block = BasicBlock::new(BlockId(index + 1));
        let lhs = if index == 0 { 0 } else { 1000 + index - 1 };
        block
            .operations
            .push(expression(1000 + index, ty, BinaryOp::BitXor, 1, lhs));
        if index + 1 == depth {
            ret(&mut block, 1000 + index);
        } else {
            branch(&mut block, index + 2);
        }
        input.push(block.clone());
        block.operations.clear();
        if index + 1 == depth {
            ret(&mut block, 10 + index);
        }
        output.push(block);
    }
    (module(ty, input), module(ty, output))
}

#[test]
fn actual_nested_reverse_layout_chains_preserve_original_occurrences() {
    for depth in [2, 8, 32] {
        let (input, expected) = reversed_chain(depth);
        let body = input.functions[0].body.as_ref().unwrap();
        assert_eq!(body.blocks[1].id, BlockId(depth));
        assert!(matches!(body.blocks[1].operations[0].kind,
            Kind::Binary { rhs, .. } if rhs == ValueId(1000 + depth - 2)));
        assert_output(input, expected, depth as usize);
    }
}

#[test]
fn no_cse_for_other_integer_operator_or_distinct_argument_atoms() {
    let (mut input, _) = pair(ScalarType::U32, BinaryOp::Add, true, true);
    assert_output(input.clone(), input.clone(), 0);
    input.functions[0].body.as_mut().unwrap().blocks[0].operations[0] =
        expression(2, ScalarType::U32, BinaryOp::BitAnd, 0, 1);
    input.functions[0].body.as_mut().unwrap().blocks[1].operations[0] =
        expression(3, ScalarType::U32, BinaryOp::BitAnd, 0, 0);
    assert_output(input.clone(), input, 0);
}

#[test]
fn sibling_branches_do_not_share_a_non_dominating_anchor() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(1));
    left.operations
        .push(expression(3, ScalarType::U32, BinaryOp::BitOr, 0, 1));
    ret(&mut left, 3);
    let mut right = BasicBlock::new(BlockId(2));
    right
        .operations
        .push(expression(4, ScalarType::U32, BinaryOp::BitOr, 1, 0));
    ret(&mut right, 4);
    let mut input = module(ScalarType::U32, vec![entry, left, right]);
    input.functions[0].signature.parameters.push(Type::BOOL);
    input.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(2));
    assert_output(input.clone(), input, 0);
}

#[test]
fn preserves_effects_trapping_producers_and_duplicate_edge_occurrences() {
    let (mut input, mut expected) = pair(ScalarType::U32, BinaryOp::BitAnd, true, false);
    for (subject, value) in [(&mut input, 3), (&mut expected, 2)] {
        subject.functions[0]
            .signature
            .parameters
            .push(Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ));
        let body = subject.functions[0].body.as_mut().unwrap();
        body.parameters.push(ValueId(4));
        body.blocks[0]
            .operations
            .push(expression(5, ScalarType::U32, BinaryOp::Divide, 0, 1));
        body.blocks[0].operations.push(Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(4),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        body.blocks[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(6), Type::BOOL),
            Kind::Constant(Constant::Bool(true)),
        ));
        body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(6),
            then_target: BlockId(20),
            then_arguments: vec![ValueId(value)],
            else_target: BlockId(20),
            else_arguments: vec![ValueId(value)],
        });
        let mut join = BasicBlock::new(BlockId(20));
        join.parameters
            .push(ValueDef::new(ValueId(7), Type::Scalar(ScalarType::U32)));
        ret(&mut join, 7);
        body.blocks.push(join);
    }
    let owner = own(&input);
    let (actual, _, _) = run(&owner);
    assert_eq!(actual.occurrences().candidate().edges.len(), 2);
    assert_eq!(actual.occurrences().candidate().edge_arguments.len(), 2);
    assert_output(input, expected, 1);
}

#[test]
fn renamed_multiple_functions_with_repeated_ids_have_separate_actual_owners() {
    let (mut input, mut expected) = pair(ScalarType::I64, BinaryOp::BitXor, true, true);
    for subject in [&mut input, &mut expected] {
        let mut second = subject.functions[0].clone();
        second.id = "unrelated_second_name".into();
        subject.functions.push(second);
    }
    assert_output(input, expected, 2);
}

#[test]
fn empty_and_declaration_only_subjects_do_not_need_a_fabricated_live_graph_body() {
    let mut empty = Module::new("empty");
    assert_output(empty.clone(), empty.clone(), 0);
    empty.functions.push(Function::declaration(
        "external_declaration",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
    ));
    assert_output(empty.clone(), empty, 0);
}

#[test]
fn unreachable_duplicate_stays_complete_and_unchanged() {
    let (mut input, _) = pair(ScalarType::U32, BinaryOp::BitOr, true, true);
    let body = input.functions[0].body.as_mut().unwrap();
    ret(&mut body.blocks[0], 2);
    assert_output(input.clone(), input, 0);
}

#[test]
fn all_width_unreachable_islands_keep_complete_original_operations_and_edges() {
    for ty in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
    ] {
        for op in [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor] {
            for topology in 0..4 {
                let (mut input, _) = pair(ty, op, true, true);
                let body = input.functions[0].body.as_mut().unwrap();
                ret(&mut body.blocks[0], 2);
                match topology {
                    0 => {}
                    1 => branch(&mut body.blocks[1], 20),
                    2 => {
                        branch(&mut body.blocks[0], 30);
                        branch(&mut body.blocks[1], 30);
                        let mut reachable = BasicBlock::new(BlockId(30));
                        ret(&mut reachable, 2);
                        body.blocks.push(reachable);
                    }
                    _ => {
                        branch(&mut body.blocks[1], 30);
                        let mut other = BasicBlock::new(BlockId(30));
                        other.operations.push(expression(4, ty, op, 0, 1));
                        ret(&mut other, 4);
                        body.blocks.push(other);
                    }
                }
                // Real import/pass/export/replay must preserve every unreachable
                // operation and CFG edge; no catch, filter, repair or DCE.
                assert_output(input.clone(), input, 0);
            }
        }
    }
}

#[test]
fn observed_failure_and_unwind_follow_first_confirmed_physical_mutation() {
    let (input, _) = reversed_chain(2);
    let input = own(&input);
    let bytes = input.owner.canonical().canonical_bytes().to_vec();
    for fault in [1, 2] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + input.storage;
        budget.reserve_storage(floor).unwrap();
        let result = optimize(&input.owner, &mut budget, fault);
        assert!(crate::kir_occurrence_capture_v1::CommutativeCapture::last_actual_mutation_seen());
        if fault == 1 {
            assert!(matches!(
                result,
                Err(Error::Capture(KirOptimizationMapErrorV12::Relation))
            ));
        } else {
            assert!(matches!(result, Err(Error::Panicked)));
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(input.owner.canonical().canonical_bytes(), bytes);
    }
}

#[test]
fn actual_capture_rejects_missing_cyclic_out_of_range_and_duplicate_finish() {
    let (input, _) = reversed_chain(2);
    let input = own(&input);
    for fault in 3..=6 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + input.storage;
        budget.reserve_storage(floor).unwrap();
        let error = optimize(&input.owner, &mut budget, fault).unwrap_err();
        assert!(matches!(
            (fault, error),
            (3 | 5, Error::Capture(KirOptimizationMapErrorV12::Coverage))
                | (4, Error::Capture(KirOptimizationMapErrorV12::Relation))
                | (6, Error::Capture(KirOptimizationMapErrorV12::Passes))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn independent_checker_refuses_a_live_same_typed_but_false_captured_target() {
    let (input, _) = reversed_chain(2);
    let input = own(&input);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + input.storage;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        optimize(&input.owner, &mut budget, 7),
        Err(Error::Relation(
            CanonicalKirCommutativeBitwiseCseErrorV1::Rule(
                "only single-result bitwise definitions"
            )
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn live_exact_and_one_short_work_storage_preserve_prepaid_input_floor() {
    let (input, _) = reversed_chain(8);
    let input = own(&input);
    let (_, exact_work, exact_storage) = run(&input);
    let floor = FLOOR + input.storage;
    for (work_limit, storage_limit, success) in [
        (exact_work, exact_storage, true),
        (exact_work - 1, STORAGE, false),
        (WORK, exact_storage - 1, false),
        (0, STORAGE, false),
        (WORK, floor, false),
    ] {
        let mut work = Work::new(work_limit);
        let storage_denied = {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = optimize_checked_commutative_bitwise_cse_v1(&input.owner, &mut budget);
            assert_eq!(result.is_ok(), success, "unexpected result: {result:?}");
            if success {
                assert_eq!(budget.work(), exact_work);
                assert_eq!(budget.peak_storage(), exact_storage);
            } else {
                assert!(!matches!(
                    result,
                    Err(Error::Panicked | Error::PassRejected)
                ));
            }
            drop(result);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            budget.failed_storage().is_some()
        };
        if !success {
            assert!(storage_denied || work.failed_work().is_some());
        }
    }
}

#[test]
fn repeated_imports_have_identical_actual_output_rows_and_resource_profile() {
    let (input, _) = reversed_chain(8);
    let input = own(&input);
    let (first, work, peak) = run(&input);
    let (second, other_work, other_peak) = run(&input);
    assert_eq!(
        first.owner().canonical().canonical_bytes(),
        second.owner().canonical().canonical_bytes()
    );
    let a = first.occurrences().candidate();
    let b = second.occurrences().candidate();
    assert_eq!(a.functions, b.functions);
    assert_eq!(a.blocks, b.blocks);
    assert_eq!(a.operations, b.operations);
    assert_eq!(a.definitions, b.definitions);
    assert_eq!(a.definition_outputs, b.definition_outputs);
    assert_eq!(a.uses, b.uses);
    assert_eq!(a.edges, b.edges);
    assert_eq!(a.edge_arguments, b.edge_arguments);
    assert_eq!((work, peak), (other_work, other_peak));
}

#[test]
fn scoped_cleanup_does_not_touch_a_replacement_ledger_or_undercut_floor() {
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    {
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut foreign_storage = 0;
        let result = resources::scoped(&mut budget, |budget, _| {
            budget.charge_work(7)?;
            budget.reserve_storage(37)?;
            foreign_storage = budget.storage();
            let mut foreign = Budget::new(&mut foreign_work, STORAGE);
            foreign.reserve_storage(foreign_storage)?;
            *budget = foreign;
            Ok(())
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(budget.storage(), foreign_storage);
        assert_eq!(budget.work(), 0);
    }
    assert_eq!(work.work(), 7);
    assert_eq!(foreign_work.work(), 0);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = resources::scoped(&mut budget, |budget, _| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), FLOOR - 1);
}

#[test]
fn panic_payload_destructor_runs_after_owned_reservation_cleanup() {
    struct Payload(Arc<AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            panic!("payload destructor");
        }
    }
    let seen = Arc::new(AtomicBool::new(false));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        resources::scoped::<()>(&mut budget, |budget, _| {
            budget.reserve_storage(37)?;
            std::panic::panic_any(Payload(seen.clone()))
        })
    }));
    assert!(result.is_err());
    assert!(seen.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn moving_the_budget_slot_is_not_authorized_by_an_equal_live_ledger_token() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let binding = resources::Binding::new(&budget);
    let token = budget.work_ledger_identity_v1();
    let moved = Box::new(budget);
    assert!(moved.work_ledger_identity_v1() == token);
    assert!(matches!(
        binding.check(&moved),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(moved.storage(), FLOOR);
}

#[test]
fn rejected_result_destructor_does_not_release_or_charge_a_foreign_budget() {
    struct Rejected(Arc<AtomicBool>);
    impl Drop for Rejected {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            panic!("rejected result");
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = resources::scoped(&mut budget, |budget, _| {
        budget.charge_work(7)?;
        budget.reserve_storage(37)?;
        let mut foreign = Budget::new(&mut foreign_work, STORAGE);
        foreign.reserve_storage(FLOOR + 37)?;
        *budget = foreign;
        Ok(Rejected(dropped.clone()))
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), FLOOR + 37);
    assert_eq!(budget.work(), 0);
}

#[test]
fn new_capacity_preflight_rejects_arithmetic_without_allocation() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        resources::vector::<u64>(usize::MAX, &mut budget),
        Err(Resource::Arithmetic)
    ));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), 3);
}
