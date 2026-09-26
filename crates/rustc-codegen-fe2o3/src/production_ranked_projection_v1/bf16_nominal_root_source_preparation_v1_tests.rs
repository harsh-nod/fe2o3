//! Inert source/resource controls only; never a manufactured production owner.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::cell::Cell;

const LIMIT: usize = 16 * 1024 * 1024;
const FLOOR: usize = 37;

fn source_fixture(dead_block: bool) -> AdmittedInertSemanticMirV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit = SemanticTypeIdV1::from_index(0);
    let ty = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([11; 32]),
        SemanticLayoutIdentityV1::from_sha256([12; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(Vec::new(), Vec::new()).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([13; 32]),
        SemanticLayoutIdentityV1::from_sha256([14; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        Vec::new(),
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let block = |tag| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            Vec::new(),
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap()
    };
    let mut blocks = vec![block(15)];
    if dead_block {
        blocks.push(block(16));
    }
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([17; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([18; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([19; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([21; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([22; 32]),
            unit,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([23; 32])),
        vec![ty],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}
fn run<'w, R, F>(
    source: &AdmittedInertSemanticMirV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&NominalRootSourceTablesV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    // Component-only rich factory: this is explicitly not the owner-bound API.
    with_rich_tables_for_test_v1(
        source.callables(),
        source.types(),
        &source.functions()[0],
        budget,
        move |rich, budget| {
            with_root_report_scope(
                source,
                SemanticFunctionIdV1::from_index(0),
                rich,
                budget,
                inspect,
            )
        },
    )
}

#[test]
fn joined_tables_and_real_report_match_the_actual_source_without_authority() {
    let source = source_fixture(false);
    let expected = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
        &source,
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    run(&source, &mut budget, |view, budget| {
        budget.charge_work(128)?;
        assert!(std::ptr::eq(view.rich().function(), &source.functions()[0]));
        assert_eq!(view.induction_report(), &expected);
        assert!(view.induction_report().work_units() > 0);
        assert!(!view.induction_report().grants_authority());
        assert!(!view.induction_report().authorizes_compiler_transform());
        assert!(!view.induction_report().uses_reachable_scope_v2());
        assert_eq!(
            view.rich().scalar_counts().len(),
            source.functions()[0].locals().len()
        );
        assert!(budget.storage() > FLOOR);
        assert!(budget.work_ledger_identity_v1() == identity);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == identity);
}
fn probe(work_limit: usize, storage_limit: usize) -> (bool, usize, usize, bool, bool) {
    let source = source_fixture(false);
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = run(&source, &mut budget, |view, budget| {
        budget.charge_work(9)?;
        assert!(view.induction_report().work_units() > 0);
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    (
        result.is_ok(),
        budget.work(),
        budget.peak_storage(),
        budget.failed_work().is_some(),
        budget.failed_storage().is_some(),
    )
}
#[test]
fn joined_exact_work_storage_and_one_short_are_fail_closed() {
    let measured = probe(LIMIT, LIMIT);
    assert!(measured.0);
    assert_eq!(probe(measured.1, measured.2), measured);
    let work = probe(measured.1 - 1, measured.2);
    assert!(!work.0 && work.3 && !work.4);
    let storage = probe(measured.1, measured.2 - 1);
    assert!(!storage.0 && !storage.3 && storage.4);
}
#[test]
fn joined_callback_success_error_panic_preserve_only_callback_surplus() {
    let source = source_fixture(false);
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let entered = Cell::new(false);
        let result = run(&source, &mut budget, |_, budget| -> Result<()> {
            entered.set(true);
            budget.reserve_storage(23)?;
            budget.charge_work(17)?;
            match mode {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable("joined callback refusal")),
                _ => panic!("joined callback unwind"),
            }
        });
        assert!(entered.get());
        assert_eq!(
            result,
            match mode {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable("joined callback refusal")),
                _ => Err(QueryError::CallbackPanicked),
            }
        );
        assert_eq!(budget.storage(), FLOOR + 23);
        budget.release_storage(23).unwrap();
    }
}
#[test]
fn joined_partial_analysis_refusal_drops_owned_report_scratch_before_refund() {
    let source = source_fixture(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let entered = Cell::new(false);
    let result = run(&source, &mut budget, |_, _| {
        entered.set(true);
        Ok(())
    });
    assert_eq!(
        result,
        Err(QueryError::Unavailable("actual source preparation refused"))
    );
    assert!(!entered.get());
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.peak_storage() > FLOOR);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
}
#[test]
fn equal_source_identity_cannot_replace_the_exact_rich_function_object() {
    let source = source_fixture(false);
    let foreign = source_fixture(false);
    assert_eq!(source.semantic_sha256(), foreign.semantic_sha256());
    assert!(!std::ptr::eq(
        &source.functions()[0],
        &foreign.functions()[0]
    ));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let entered = Cell::new(false);
    let result = with_rich_tables_for_test_v1(
        foreign.callables(),
        foreign.types(),
        &foreign.functions()[0],
        &mut budget,
        |rich, budget| {
            with_root_report_scope(
                &source,
                SemanticFunctionIdV1::from_index(0),
                rich,
                budget,
                |_, _| {
                    entered.set(true);
                    Ok(())
                },
            )
        },
    );
    assert_eq!(
        result,
        Err(QueryError::Unavailable(
            "joined root rich function differs from actual source"
        ))
    );
    assert!(!entered.get());
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn missing_source_caller_never_invokes_the_joined_callback() {
    let source = source_fixture(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_rich_tables_for_test_v1(
        source.callables(),
        source.types(),
        &source.functions()[0],
        &mut budget,
        |rich, budget| {
            with_root_report_scope::<(), _>(
                &source,
                SemanticFunctionIdV1::from_index(99),
                rich,
                budget,
                |_, _| panic!("missing caller cannot lend a report"),
            )
        },
    );
    assert_eq!(
        result,
        Err(QueryError::Unavailable(
            "joined root caller absent from actual source"
        ))
    );
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn joined_ignored_denials_remain_sticky_and_cannot_report_success() {
    let source = source_fixture(false);
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = run(&source, &mut budget, |_, budget| {
            if storage {
                let _ = budget.reserve_storage(LIMIT + 1);
            } else {
                let _ = budget.charge_work(LIMIT + 1);
            }
            Ok(())
        });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage().is_some(), storage);
        assert_eq!(budget.failed_work().is_some(), !storage);
    }
}
#[test]
fn joined_floor_corruption_is_not_repaired_by_outer_scope_refunds() {
    let source = source_fixture(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = run(&source, &mut budget, |_, budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert!(budget.storage() > FLOOR);
}
#[test]
fn joined_original_ledger_replacement_is_not_repaired() {
    let source = source_fixture(false);
    let mut original = Work::new(LIMIT);
    let mut foreign = Work::new(LIMIT);
    let replacement = Budget::new(&mut foreign, LIMIT);
    let mut budget = Budget::new(&mut original, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    assert_eq!(
        run(&source, &mut budget, |_, budget| {
            *budget = replacement;
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert!(budget.work_ledger_identity_v1() != identity);
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (0, 0, 0)
    );
}
#[test]
fn joined_preexisting_denial_does_not_run_preparation_or_callback() {
    let source = source_fixture(false);
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        if storage {
            assert!(budget.reserve_storage(LIMIT + 1).is_err());
        } else {
            assert!(budget.charge_work(LIMIT + 1).is_err());
        }
        assert_eq!(
            run::<(), _>(&source, &mut budget, |_, _| panic!("denied")),
            Err(Resource::Accounting.into())
        );
        assert_eq!((budget.storage(), budget.work()), (FLOOR, 0));
    }
}
struct LargeCapture<'a> {
    bytes: [u8; 8192],
    dropped: &'a Cell<bool>,
}
impl Drop for LargeCapture<'_> {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}
#[test]
fn joined_large_callback_and_copy_result_are_paid_and_drop_before_refund() {
    assert!(root_header::<[u8; 8192]>(size_of::<LargeCapture<'_>>()).unwrap() > 4 * 8192);
    let source = source_fixture(false);
    let dropped = Cell::new(false);
    let capture = LargeCapture {
        bytes: [7; 8192],
        dropped: &dropped,
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = run(&source, &mut budget, move |_, budget| {
        let bytes = capture.bytes;
        drop(capture);
        assert!(budget.storage() > FLOOR);
        Ok(bytes)
    })
    .unwrap();
    assert_eq!(result, [7; 8192]);
    assert!(dropped.get());
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn joined_large_callback_drops_on_partial_analysis_refusal_without_a_loan() {
    let source = source_fixture(true);
    let dropped = Cell::new(false);
    let capture = LargeCapture {
        bytes: [9; 8192],
        dropped: &dropped,
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = run(&source, &mut budget, move |_, _| {
        let bytes = capture.bytes;
        drop(capture);
        Ok(bytes)
    });
    assert_eq!(
        result,
        Err(QueryError::Unavailable("actual source preparation refused"))
    );
    assert!(dropped.get());
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn joined_generic_header_arithmetic_refuses_before_preparation() {
    assert_eq!(
        root_header::<()>(usize::MAX),
        Err(Resource::Arithmetic.into())
    );
}
