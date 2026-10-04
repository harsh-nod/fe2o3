use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, CastKind,
    Constant, Function, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId, verify_module_ref, verify_module_with_capabilities,
};

const DIAGNOSTIC_ROW: usize =
    std::mem::size_of::<crate::Diagnostic>().div_ceil(std::mem::size_of::<usize>());

struct Observation<'module> {
    result: Result<VerifiedKernelIrModuleV1<'module>, BorrowedKernelIrVerificationErrorV1>,
    work: usize,
    current: usize,
    peak: usize,
    failed_storage: Option<usize>,
}

fn run<'module>(module: &'module Module, work: usize, storage: usize) -> Observation<'module> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage);
    let result = verify_module_ref_with_budget_v1(module, None, &mut budget);
    Observation {
        result,
        work: budget.work(),
        current: budget.storage(),
        peak: budget.peak_storage(),
        failed_storage: budget.failed_storage(),
    }
}

fn nested_pointer(depth: usize) -> Type {
    (0..depth).fold(Type::Scalar(ScalarType::U32), |ty, _| {
        Type::pointer(ty, AddressSpace::Private, AccessMode::ReadWrite)
    })
}

fn depth_module(depth: usize) -> Module {
    let mut module = Module::new("module");
    module.functions.push(Function::external_import(
        "function",
        Signature::new(vec![nested_pointer(depth)], vec![]),
    ));
    module
}

fn diagnostics(error: BorrowedKernelIrVerificationErrorV1) -> VerificationErrors {
    match error {
        BorrowedKernelIrVerificationErrorV1::Verification(error) => error,
        BorrowedKernelIrVerificationErrorV1::Resource(error) => {
            panic!("unexpected resource failure: {error}");
        }
    }
}

fn assert_work_prefixes(module: &Module, charges: &[usize], storage: usize) {
    let exact: usize = charges.iter().sum();
    for limit in 0..exact {
        let mut accepted = 0;
        let mut attempted = 0;
        for charge in charges {
            attempted = accepted + charge;
            if attempted > limit {
                break;
            }
            accepted = attempted;
        }
        let observed = run(module, limit, storage);
        assert!(
            matches!(
                observed.result,
                Err(BorrowedKernelIrVerificationErrorV1::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                )) if error.actual() == attempted && error.limit() == limit
            ),
            "limit={limit}, accepted={accepted}, attempted={attempted}"
        );
        assert_eq!(observed.work, accepted, "limit={limit}");
        assert_eq!(observed.current, 0, "limit={limit}");
        assert!(observed.peak <= storage);
    }
}

#[test]
fn borrowed_verifier_exact_empty_boundary_binds_the_borrowed_owner() {
    // One preflight module visit, then the existing five-field location debit.
    let module = Module::new("module");
    let clone = module.clone();
    let observed = run(&module, 6, 0);
    let verified = observed.result.unwrap();
    assert!(std::ptr::eq(verified.module(), &module));
    assert!(!std::ptr::eq(verified.module(), &clone));
    assert_eq!((observed.work, observed.current, observed.peak), (6, 0, 0));
    assert_work_prefixes(&module, &[1, 5], 0);
}

#[test]
fn borrowed_verifier_invalid_identity_preserves_exact_diagnostics_and_prefixes() {
    // Preflight1, count44, materialization111 plus empty-ID copy7, finish1.
    const WORK: usize = 164;
    const STORAGE: usize = DIAGNOSTIC_ROW + 33;
    let module = Module::new("");
    let observed = run(&module, WORK, STORAGE);
    assert_eq!(
        diagnostics(observed.result.unwrap_err()),
        verify_module_ref(&module).unwrap_err()
    );
    assert_eq!(
        (observed.work, observed.current, observed.peak),
        (WORK, 0, STORAGE)
    );
    assert_work_prefixes(&module, &[1, 5, 1, 33, 5, 5, 1, 33, 7, 67, 5, 1], STORAGE);

    let denied = run(&module, WORK, STORAGE - 1);
    assert!(matches!(
        denied.result,
        Err(BorrowedKernelIrVerificationErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        )) if error.actual() == STORAGE && error.limit() == STORAGE - 1
    ));
    assert_eq!(
        (denied.work, denied.current, denied.peak),
        (158, 0, DIAGNOSTIC_ROW)
    );
    assert_eq!(denied.failed_storage, Some(STORAGE));
}

#[test]
fn borrowed_verifier_depth_error_has_literal_publication_and_denial_boundaries() {
    // Module/function/signature visits 3, depth loop 66, location fields 5.
    // Materialization costs 1 + 55 + (5 + 14 + 2*2) + 111 + 1.
    // Both source identifier buffers coexist with the 55-byte message and row.
    const WORK: usize = 265;
    const STORAGE: usize = DIAGNOSTIC_ROW + 14 + 55;
    let module = depth_module(65);
    let observed = run(&module, WORK, STORAGE);
    assert_eq!(
        diagnostics(observed.result.unwrap_err()),
        verify_module_ref(&module).unwrap_err()
    );
    assert_eq!(
        (observed.work, observed.current, observed.peak),
        (WORK, 0, STORAGE)
    );
    let mut charges = vec![1; 69];
    charges.extend_from_slice(&[5, 1, 55, 23, 111, 1]);
    assert_work_prefixes(&module, &charges, STORAGE);

    let denied = run(&module, WORK, STORAGE - 1);
    assert!(matches!(
        denied.result,
        Err(BorrowedKernelIrVerificationErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        )) if error.actual() == STORAGE && error.limit() == STORAGE - 1
    ));
    assert_eq!(
        (denied.work, denied.current, denied.peak),
        (WORK - 1, 0, DIAGNOSTIC_ROW)
    );
    assert_eq!(denied.failed_storage, Some(STORAGE));

    let denied = run(&module, WORK, DIAGNOSTIC_ROW - 1);
    assert_eq!((denied.work, denied.current, denied.peak), (74, 0, 0));
    assert_eq!(denied.failed_storage, Some(DIAGNOSTIC_ROW));
    assert!(matches!(
        denied.result,
        Err(BorrowedKernelIrVerificationErrorV1::Resource(_))
    ));
}

#[test]
fn borrowed_verifier_exact_depth_and_ordinary_verification_match_the_existing_engine() {
    let module = depth_module(64);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let expected = verify_depth_bounded_module_with_budget_v1(&module, None, &mut budget).unwrap();
    assert!(std::ptr::eq(expected.module(), &module));
    let engine_work = budget.work();
    let engine_peak = budget.peak_storage();
    let observed = run(&module, engine_work + 68, engine_peak);
    assert!(std::ptr::eq(observed.result.unwrap().module(), &module));
    assert_eq!(observed.work, engine_work + 68);
    assert_eq!((observed.current, observed.peak), (0, engine_peak));
}

#[test]
fn borrowed_verifier_preserves_preflight_and_capability_error_order() {
    let supported = BTreeSet::from([TargetCapability::DynamicWorkgroupMemory]);
    for module in [Module::new("module"), depth_module(65)] {
        let expected = verify_module_with_capabilities(&module, &supported).unwrap_err();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
        let result = verify_module_ref_with_budget_v1(&module, Some(&supported), &mut budget);
        assert_eq!(diagnostics(result.unwrap_err()), expected);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn borrowed_verifier_shares_recursive_type_owner_order_and_short_circuiting() {
    let deep = nested_pointer(65);
    let mut cases = Vec::new();
    for (results, kind) in [
        (
            vec![ValueDef::new(ValueId(0), deep.clone())],
            OperationKind::Constant(Constant::U32(0)),
        ),
        (
            vec![],
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(0),
                to: deep.clone(),
            },
        ),
        (
            vec![ValueDef::new(ValueId(0), deep.clone())],
            OperationKind::Alloca {
                element: deep.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
    ] {
        let mut module = Module::new("module");
        module.functions.push(Function::internal_helper(
            "function",
            Signature::new(vec![], vec![]),
            vec![],
            vec![BasicBlock {
                id: BlockId(9),
                parameters: vec![],
                operations: vec![Operation::new(results, kind)],
                terminator: Some(Terminator::Return { values: vec![] }),
            }],
        ));
        cases.push(module);
    }
    for module in &cases {
        let observed = run(module, usize::MAX, usize::MAX);
        assert_eq!(
            diagnostics(observed.result.unwrap_err()),
            verify_module_ref(module).unwrap_err()
        );
        assert_eq!(observed.current, 0);
        assert_eq!(observed.work, 244 + 5 + 14 + 2 * 2);
    }
    // The result owner is visited first. Its excessive depth suppresses the
    // Alloca element walk, just as the existing short-circuit disjunction does.
}

#[test]
fn borrowed_verifier_restores_caller_checkpoint_after_every_failure_phase() {
    for (module, limit, storage) in [
        (Module::new("module"), 6, 11),
        (Module::new(""), 164, 11 + DIAGNOSTIC_ROW + 33),
        (Module::new(""), 163, 11 + DIAGNOSTIC_ROW + 33),
        (Module::new(""), 164, 11 + DIAGNOSTIC_ROW + 32),
        (depth_module(65), 73, 74),
        (depth_module(65), 265, 11 + DIAGNOSTIC_ROW + 14 + 54),
        (depth_module(65), 265, 11 + DIAGNOSTIC_ROW + 14 + 55),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit + 7);
        work.charge_work(7).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage);
        budget.reserve_storage(11).unwrap();
        let expected = run(&module, limit, storage - 11);
        let result = verify_module_ref_with_budget_v1(&module, None, &mut budget);
        match (result, expected.result) {
            (Ok(actual), Ok(_)) => assert!(std::ptr::eq(actual.module(), &module)),
            (
                Err(BorrowedKernelIrVerificationErrorV1::Verification(actual)),
                Err(BorrowedKernelIrVerificationErrorV1::Verification(expected)),
            ) => assert_eq!(actual, expected),
            (
                Err(BorrowedKernelIrVerificationErrorV1::Resource(_)),
                Err(BorrowedKernelIrVerificationErrorV1::Resource(_)),
            ) => {}
            (actual, expected) => panic!("different result categories: {actual:?}, {expected:?}"),
        }
        assert_eq!(budget.work(), 7 + expected.work);
        assert_eq!(budget.storage(), 11);
        assert_eq!(budget.peak_storage(), 11 + expected.peak);
        assert_eq!(
            budget.failed_storage(),
            expected.failed_storage.map(|actual| actual + 11)
        );
    }
}

#[test]
fn borrowed_verifier_preserves_existing_saturating_overflow_and_caller_usage() {
    let module = Module::new("module");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    work.charge_work(usize::MAX).unwrap();
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(matches!(
        verify_module_ref_with_budget_v1(&module, None, &mut budget),
        Err(BorrowedKernelIrVerificationErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Work(error)
        )) if error.actual() == usize::MAX && error.limit() == usize::MAX
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (usize::MAX, 0, 0)
    );

    let module = depth_module(65);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(usize::MAX).unwrap();
    assert!(matches!(
        verify_module_ref_with_budget_v1(&module, None, &mut budget),
        Err(BorrowedKernelIrVerificationErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        )) if error.actual() == usize::MAX && error.limit() == usize::MAX
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (74, usize::MAX, usize::MAX)
    );
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
}
