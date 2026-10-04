use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, Module,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};

#[path = "checked_optimization_aggregate_v1_tests.rs"]
mod aggregate_resources;

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
const PREFIX: usize = 23;
const PRIOR_WORK: usize = 7;

fn source(literal: u32) -> Module {
    let mut block = BasicBlock::new(BlockId(40));
    for (id, value) in [(17, literal), (93, 99)] {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(value)),
        ));
    }
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(17)],
    });
    let mut module = Module::new("checked-adapter");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
        vec![],
        vec![block],
    ));
    module
}

fn admit(source: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(source, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, storage.retained_storage())
}

// Only partial adoption-boundary tests use an explicitly transferred genuine
// observation. Public-adapter tests keep every optimizer phase on one ledger.
fn prepare(input: &(Owner, usize)) -> KirNeutralOptimizationOutputV1<'_> {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input.1).unwrap();
    let observed = optimize_native_neutral_kernel_ir_v1(&input.0, &mut budget).unwrap();
    assert_eq!(budget.storage(), input.1);
    budget.release_storage(input.1).unwrap();
    observed
}

#[test]
fn actual_non_dense_execution_returns_checked_custody_after_input_drop() {
    let input = admit(&source(7));
    let input_bytes = input.0.canonical().canonical_bytes().to_vec();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(PRIOR_WORK).unwrap();
    budget.reserve_storage(PREFIX + input.1).unwrap();
    let checked = optimize_checked_canonical_kernel_ir_v1(&input.0, &mut budget).unwrap();
    assert_eq!(budget.storage(), PREFIX + input.1);
    assert_eq!(input.0.canonical().canonical_bytes(), input_bytes);
    assert_eq!(
        input.0.module().functions[0].body.as_ref().unwrap().blocks[0].id,
        BlockId(40)
    );
    assert!(checked.report().passes().iter().any(|pass| pass.changed()));
    assert_eq!(checked.report().passes().len(), 7);
    assert!(checked.map().matches_execution(checked.report()));
    let retained = checked.storage().retained_storage();
    budget.reserve_storage(retained).unwrap();
    let input_storage = input.1;
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(checked.native_input_audit_bytes(), input_bytes);
    let body = checked.owner().module().functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks[0].operations.len(), 1);
    assert!(matches!(
        body.blocks[0].operations[0].kind,
        OperationKind::Constant(Constant::U32(7))
    ));
    assert!(!checked.grants_authority());
    assert!(budget.work() > PRIOR_WORK);
    drop(checked);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}

#[test]
fn empty_noop_keeps_the_exact_input_bytes_and_seven_pass_order() {
    let input = admit(&Module::new("m"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(PREFIX + input.1).unwrap();
    let checked = optimize_checked_canonical_kernel_ir_v1(&input.0, &mut budget).unwrap();
    assert_eq!(
        checked.owner().canonical().canonical_bytes(),
        input.0.canonical().canonical_bytes()
    );
    assert!(checked.report().passes().iter().all(|pass| !pass.changed()));
    let actual = checked
        .report()
        .passes()
        .iter()
        .map(|pass| pass.pass().name())
        .collect::<Vec<_>>();
    let expected = crate::KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2
        .iter()
        .map(|pass| pass.name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(!checked.grants_authority());
    assert_eq!(budget.storage(), PREFIX + input.1);
    drop(checked);
}

#[test]
fn foreign_observed_owner_is_rejected_even_when_canonical_bytes_are_equal() {
    let input = admit(&source(7));
    for literal in [7, 8] {
        let other = admit(&source(literal));
        assert_eq!(
            input.0.canonical().canonical_bytes() == other.0.canonical().canonical_bytes(),
            literal == 7
        );
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = PREFIX + input.1 + other.1;
        budget.reserve_storage(floor).unwrap();
        let observed = optimize_native_neutral_kernel_ir_v1(&other.0, &mut budget).unwrap();
        let before = budget.work();
        assert!(matches!(
            finish_observed_v1(&input.0, observed, &mut budget),
            Err(KernelIrCheckedOptimizationErrorV1::InputOwner)
        ));
        assert_eq!(budget.work(), before + 1);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn initial_observation_denial_never_returns_a_fallback_graph_or_resets_work() {
    let input = admit(&Module::new("m"));
    assert_eq!(input.0.canonical().canonical_bytes().len(), 37);
    let mut work = Work::new(PRIOR_WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(PRIOR_WORK).unwrap();
    budget.reserve_storage(PREFIX + input.1).unwrap();
    assert!(matches!(
        optimize_checked_canonical_kernel_ir_v1(&input.0, &mut budget),
        Err(KernelIrCheckedOptimizationErrorV1::Observation(_))
    ));
    assert_eq!(budget.work(), PRIOR_WORK);
    assert_eq!(budget.storage(), PREFIX + input.1);
    assert_eq!(budget.peak_storage(), PREFIX + input.1);
    assert!(budget.charge_work(usize::MAX).is_err());
    assert_eq!(work.failed_work(), Some(PRIOR_WORK + 37));
}

#[test]
fn partial_empty_adoption_has_independent_exact_and_one_under_work() {
    // Existing empty-owner adoption is 49: entry1 + two inventory census/fill
    // pairs4 + checker5 + two one-byte names2 + historical input copy37.
    // This adapter adds exactly one input-owner pointer comparison.
    const ADOPTION: usize = 50;
    const BEFORE_COPY: usize = 13;
    let input = admit(&Module::new("m"));
    assert_eq!(input.0.canonical().canonical_bytes().len(), 37);
    for short in [false, true] {
        let observed = prepare(&input);
        let mut work = Work::new(PRIOR_WORK + ADOPTION - usize::from(short));
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR_WORK).unwrap();
        budget.reserve_storage(PREFIX + input.1).unwrap();
        let result = finish_observed_v1(&input.0, observed, &mut budget);
        assert_eq!(budget.storage(), PREFIX + input.1);
        if short {
            assert!(matches!(
                result,
                Err(KernelIrCheckedOptimizationErrorV1::Check(
                    KirCheckedNeutralOptimizationErrorV1::Resource(Resource::Work(_))
                ))
            ));
            assert_eq!(budget.work(), PRIOR_WORK + BEFORE_COPY);
            assert!(budget.charge_work(usize::MAX).is_err());
            assert_eq!(work.failed_work(), Some(PRIOR_WORK + ADOPTION));
        } else {
            let checked = result.unwrap();
            assert_eq!(budget.work(), PRIOR_WORK + ADOPTION);
            let retained = checked.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            drop(checked);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), PREFIX + input.1);
            assert_eq!(work.failed_work(), None);
        }
    }
}

#[test]
fn partial_observation_transfer_is_reserved_before_comparison_or_checking() {
    let input = admit(&Module::new("m"));
    for short in [false, true] {
        let observed = prepare(&input);
        let retained = observed.storage().retained_storage();
        let floor = PREFIX + input.1;
        let exact_transfer = floor + retained;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, exact_transfer - usize::from(short));
        budget.charge_work(PRIOR_WORK).unwrap();
        budget.reserve_storage(floor).unwrap();
        let result = finish_observed_v1(&input.0, observed, &mut budget);
        assert_eq!(budget.storage(), floor);
        if short {
            assert!(matches!(
                result,
                Err(KernelIrCheckedOptimizationErrorV1::Resource(
                    Resource::Storage(_)
                ))
            ));
            assert_eq!(budget.work(), PRIOR_WORK);
            assert_eq!(budget.peak_storage(), floor);
            assert_eq!(budget.failed_storage(), Some(exact_transfer));
        } else {
            // This is only the exact observation-transfer boundary. The
            // independent checker still needs additional inventory storage.
            assert!(matches!(
                result,
                Err(KernelIrCheckedOptimizationErrorV1::Check(
                    KirCheckedNeutralOptimizationErrorV1::Inventory(_)
                ))
            ));
            assert!(budget.work() > PRIOR_WORK + 1);
            assert_eq!(budget.peak_storage(), exact_transfer);
            assert!(budget.failed_storage().unwrap() > exact_transfer);
        }
    }
}
