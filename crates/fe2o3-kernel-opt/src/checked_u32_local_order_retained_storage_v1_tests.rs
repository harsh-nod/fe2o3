//! Genuine inert scheduler owners; no compiler-source or production-memory claim.
use super::*;
use crate::{
    U32LocalOrderPreferenceV1, U32LocalOrderRegionV1, prepare_owned_u32_local_order_continuation_v1,
};
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1,
    CanonicalKirFunctionCoordinateV1, Function, Module, Operation, OperationKind, ScalarType,
    Signature, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
const LIMIT: usize = 20_000_000;
fn limits(bytes: Option<usize>, items: usize) -> Limits {
    Limits {
        max_bytes: bytes,
        max_items: items,
    }
}
fn source() -> Module {
    let u32_type = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(19));
    block.operations = [
        (4, BinaryOp::BitXor, 0, 1),
        (5, BinaryOp::BitOr, 2, 3),
        (6, BinaryOp::BitAnd, 4, 5),
    ]
    .into_iter()
    .map(|(result, op, lhs, rhs)| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), u32_type.clone()),
            OperationKind::Binary {
                op,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    })
    .collect();
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(6)],
    });
    let mut module = Module::new("tail-storage-model");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![u32_type.clone(); 4], vec![u32_type]),
        (0..4).map(ValueId).collect(),
        vec![block],
    ));
    for function in &mut module.functions {
        function.required_capabilities = function.derived_capabilities();
    }
    module.required_capabilities = module
        .functions
        .iter()
        .flat_map(|function| function.required_capabilities.iter().cloned())
        .collect();
    module
}
fn create(preference: U32LocalOrderPreferenceV1) -> (Owner, Tail) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (input, storage) =
        Owner::from_module_ref_with_verification_budget_v12(&source(), &mut budget).unwrap();
    budget
        .reserve_storage(storage.retained_storage() + 17)
        .unwrap();
    let region = U32LocalOrderRegionV1 {
        expected_input: *input.canonical().identity(),
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(0),
            block: 0,
        },
        first_operation: 0,
        operation_count: 3,
    };
    let tail =
        prepare_owned_u32_local_order_continuation_v1(&input, region, preference, &mut budget)
            .unwrap();
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    tail.retained_logical_storage_v11(limits(None, 100_000))
        .unwrap();
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
    (input, tail)
}
fn observe(tail: &Tail) -> OwnedU32LocalOrderRetainedStorageV1 {
    tail.retained_logical_storage_v11(limits(None, 100_000))
        .unwrap()
}

#[test]
fn both_real_preferences_observe_one_tail_without_counting_borrowed_input() {
    for preference in [
        U32LocalOrderPreferenceV1::SourceOrder,
        U32LocalOrderPreferenceV1::ReverseReady,
    ] {
        let (input, tail) = create(preference);
        let before = (
            tail.output().canonical().canonical_bytes().to_vec(),
            tail.transition_receipt().canonical_bytes().to_vec(),
            tail.retained_storage(),
        );
        let report = observe(&tail);
        let mut output = Counter::new(limits(None, 100_000));
        tail.output().charge_retained_heap_v11(&mut output).unwrap();
        let mut receipt = Counter::new(limits(None, 10));
        tail.transition_receipt()
            .charge_retained_heap_storage_v1(&mut receipt)
            .unwrap();
        assert_eq!(report.inline_bytes, size_of::<Tail>());
        assert_eq!(report.output_owned_bytes, output.bytes());
        assert_eq!(report.receipt_owned_bytes, receipt.bytes());
        assert_eq!(
            report.total_bytes,
            size_of::<Tail>() + output.bytes() + receipt.bytes()
        );
        assert_eq!(report.visited_items, 1 + output.items() + receipt.items());
        drop(input);
        assert_eq!(observe(&tail), report);
        assert_eq!(tail.output().canonical().canonical_bytes(), before.0);
        assert_eq!(tail.transition_receipt().canonical_bytes(), before.1);
        assert_eq!(tail.retained_storage(), before.2);
        assert!(!tail.grants_authority());
    }
}

#[test]
fn tail_standalone_exact_and_one_short_limits_return_no_partial_report() {
    let (_, tail) = create(U32LocalOrderPreferenceV1::ReverseReady);
    let report = observe(&tail);
    assert_eq!(
        tail.retained_logical_storage_v11(limits(Some(report.total_bytes), report.visited_items)),
        Ok(report)
    );
    assert_eq!(
        tail.retained_logical_storage_v11(limits(
            Some(report.total_bytes - 1),
            report.visited_items
        )),
        Err(Error::ByteLimit)
    );
    assert_eq!(
        tail.retained_logical_storage_v11(limits(None, report.visited_items - 1)),
        Err(Error::ItemLimit)
    );
    assert_eq!(
        tail.retained_logical_storage_v11(limits(None, 0)),
        Err(Error::ItemLimit)
    );
}

#[test]
fn enclosing_header_is_charged_once_and_heap_walk_has_no_tail_root_visit() {
    struct Enclosing {
        _tag: [u8; 17],
        tail: Tail,
    }
    let (_, tail) = create(U32LocalOrderPreferenceV1::SourceOrder);
    let report = observe(&tail);
    let owner = Enclosing {
        _tag: [0; 17],
        tail,
    };
    let expected = size_of::<Enclosing>() + report.total_bytes - report.inline_bytes;
    let mut counter = Counter::new(limits(Some(expected), report.visited_items));
    counter.charge(size_of::<Enclosing>(), 1).unwrap();
    owner.tail.charge_retained_heap_v11(&mut counter).unwrap();
    assert_eq!(
        (counter.bytes(), counter.items()),
        (expected, report.visited_items)
    );
    let mut no_root = Counter::new(limits(None, report.visited_items - 1));
    no_root.charge(size_of::<Enclosing>(), 0).unwrap();
    owner.tail.charge_retained_heap_v11(&mut no_root).unwrap();
    assert_eq!(no_root.items() + 1, report.visited_items);
}

#[test]
fn exhausted_enclosing_counter_refuses_without_unbounded_preliminary_walk() {
    let (_, tail) = create(U32LocalOrderPreferenceV1::ReverseReady);
    let mut items = Counter::new(limits(None, 0));
    assert_eq!(
        tail.charge_retained_heap_v11(&mut items),
        Err(Error::ItemLimit)
    );
    assert_eq!((items.bytes(), items.items()), (0, 0));
    let mut bytes = Counter::new(limits(Some(0), 100_000));
    assert_eq!(
        tail.charge_retained_heap_v11(&mut bytes),
        Err(Error::ByteLimit)
    );
    assert_eq!((bytes.bytes(), bytes.items()), (0, 0));
    let mut arithmetic = Counter::new(limits(None, usize::MAX));
    arithmetic.charge(usize::MAX, 0).unwrap();
    assert_eq!(
        tail.charge_retained_heap_v11(&mut arithmetic),
        Err(Error::Arithmetic)
    );
    assert_eq!((arithmetic.bytes(), arithmetic.items()), (usize::MAX, 0));
}

#[test]
fn independently_created_equal_byte_tails_keep_two_owned_payloads() {
    let (_, first) = create(U32LocalOrderPreferenceV1::SourceOrder);
    let (_, second) = create(U32LocalOrderPreferenceV1::SourceOrder);
    assert_eq!(
        first.output().canonical().canonical_bytes(),
        second.output().canonical().canonical_bytes()
    );
    let first_report = observe(&first);
    let second_report = observe(&second);
    let owner = (first, second);
    let mut counter = Counter::new(limits(None, 100_000));
    counter.charge(size_of::<(Tail, Tail)>(), 1).unwrap();
    owner.0.charge_retained_heap_v11(&mut counter).unwrap();
    owner.1.charge_retained_heap_v11(&mut counter).unwrap();
    assert_eq!(
        counter.bytes(),
        size_of::<(Tail, Tail)>() + first_report.total_bytes - first_report.inline_bytes
            + second_report.total_bytes
            - second_report.inline_bytes
    );
    assert_eq!(
        counter.items(),
        first_report.visited_items + second_report.visited_items - 1
    );
}
