//! Admitted inert-source component controls; no manufactured production owner.
//! Genuine owner/true-entry-floor controls are in the separate genuine leaf.
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
    let block = |tag, kind| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            Vec::new(),
            SemanticTerminatorV1::new(source, kind),
        )
        .unwrap()
    };
    let edge = |target| {
        SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        )
    };
    let mut blocks = vec![
        block(15, SemanticTerminatorKindV1::Goto(edge(1))),
        block(16, SemanticTerminatorKindV1::Goto(edge(2))),
        block(17, SemanticTerminatorKindV1::Return),
    ];
    if dead_block {
        blocks.push(block(18, SemanticTerminatorKindV1::Return));
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
    assertion_source: &AdmittedInertSemanticMirV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&NominalRootAssertionSourceV1<'s>, &mut Budget<'w>) -> Result<R>,
{
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
                move |root, budget| {
                    with_graph_scope(
                        source,
                        SemanticFunctionIdV1::from_index(0),
                        root,
                        budget,
                        move |cfg, budget| {
                            with_assertion_scope(
                                assertion_source,
                                SemanticFunctionIdV1::from_index(0),
                                cfg,
                                budget,
                                inspect,
                            )
                        },
                    )
                },
            )
        },
    )
}
#[test]
fn actual_whole_mask_retains_exact_source_graph_types_and_logical_work() {
    let source = source_fixture(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    run(&source, &source, &mut budget, |view, budget| {
        budget.charge_work(128)?;
        assert!(std::ptr::eq(view.cfg().function(), &source.functions()[0]));
        assert_eq!(view.cfg().types().as_ptr(), source.types().as_ptr());
        assert_eq!(view.cfg().types().len(), source.types().len());
        assert_eq!(view.cfg().graph().successors[0], [1]);
        assert_eq!(view.cfg().graph().reachable, [true; 3]);
        assert_eq!(view.decisions(), [false; 3]);
        // This admitted fixture has no Assert terminators. It tests custody and
        // real mask construction, not positive evaluator branch coverage.
        assert_eq!(view.logical_work(), 0);
        assert_eq!(
            view.cfg()
                .source_tables()
                .induction_report()
                .semantic_mir_sha256(),
            source.semantic_sha256()
        );
        assert!(
            !view
                .cfg()
                .source_tables()
                .induction_report()
                .grants_authority()
        );
        assert!(budget.storage() > FLOOR);
        assert!(budget.work_ledger_identity_v1() == identity);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
fn probe(work_limit: usize, storage_limit: usize) -> (bool, usize, usize, bool, bool) {
    let source = source_fixture(false);
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = run(&source, &source, &mut budget, |view, budget| {
        budget.charge_work(9)?;
        assert_eq!(view.decisions(), [false; 3]);
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
fn exact_and_one_short_whole_component_route_boundaries_are_fail_closed() {
    let exact = probe(LIMIT, LIMIT);
    assert!(exact.0);
    assert_eq!(probe(exact.1, exact.2), exact);
    let short_work = probe(exact.1 - 1, exact.2);
    assert!(!short_work.0 && short_work.3 && !short_work.4);
    let short_storage = probe(exact.1, exact.2 - 1);
    assert!(!short_storage.0 && !short_storage.3 && short_storage.4);
}
fn preparation_probe(work_limit: usize, storage_limit: usize) -> (bool, bool, usize, usize) {
    let source = source_fixture(false);
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let entered = Cell::new(false);
    let result = run(&source, &source, &mut budget, |view, _| {
        entered.set(true);
        assert_eq!(view.decisions(), [false; 3]);
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    (
        result.is_ok(),
        entered.get(),
        budget.work(),
        budget.peak_storage(),
    )
}
#[test]
fn last_preloan_work_or_storage_denial_has_zero_observer() {
    let exact = preparation_probe(LIMIT, LIMIT);
    assert!(exact.0 && exact.1);
    assert_eq!(preparation_probe(exact.2, exact.3), exact);
    let work_short = preparation_probe(exact.2 - 1, exact.3);
    let storage_short = preparation_probe(exact.2, exact.3 - 1);
    assert!(!work_short.0 && !work_short.1);
    assert!(!storage_short.0 && !storage_short.1);
}
#[test]
fn success_error_and_panic_preserve_only_callback_surplus() {
    let source = source_fixture(false);
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let entered = Cell::new(false);
        let result = run(&source, &source, &mut budget, |_, budget| -> Result<()> {
            entered.set(true);
            budget.reserve_storage(23)?;
            budget.charge_work(17)?;
            match mode {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable("assertion callback refusal")),
                _ => panic!("assertion callback unwind"),
            }
        });
        assert!(entered.get());
        assert_eq!(
            result,
            match mode {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable("assertion callback refusal")),
                _ => Err(QueryError::CallbackPanicked),
            }
        );
        assert_eq!(budget.storage(), FLOOR + 23);
        budget.release_storage(23).unwrap();
    }
}
#[test]
fn equal_but_separate_admitted_source_cannot_supply_the_mask() {
    let source = source_fixture(false);
    let foreign = source_fixture(false);
    assert_eq!(source.semantic_sha256(), foreign.semantic_sha256());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let entered = Cell::new(false);
    assert_eq!(
        run(&source, &foreign, &mut budget, |_, _| {
            entered.set(true);
            Ok(())
        }),
        Err(QueryError::Unavailable(
            "root assertion source binding differs"
        ))
    );
    assert!(!entered.get());
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn partial_underlying_report_refusal_never_lends_a_mask() {
    let source = source_fixture(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let entered = Cell::new(false);
    assert_eq!(
        run(&source, &source, &mut budget, |_, _| {
            entered.set(true);
            Ok(())
        }),
        Err(QueryError::Unavailable("actual source preparation refused"))
    );
    assert!(!entered.get());
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.peak_storage() > FLOOR);
}
#[test]
fn ignored_denials_are_sticky_after_mask_observation() {
    let source = source_fixture(false);
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        assert_eq!(
            run(&source, &source, &mut budget, |_, budget| {
                if storage {
                    let _ = budget.reserve_storage(LIMIT + 1);
                } else {
                    let _ = budget.charge_work(LIMIT + 1);
                }
                Ok(())
            }),
            Err(Resource::Accounting.into())
        );
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage().is_some(), storage);
        assert_eq!(budget.failed_work().is_some(), !storage);
    }
}
#[test]
fn assertion_own_floor_erosion_is_not_repaired_before_outer_refunds() {
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
            with_root_report_scope(
                &source,
                SemanticFunctionIdV1::from_index(0),
                rich,
                budget,
                |root, budget| {
                    with_graph_scope(
                        &source,
                        SemanticFunctionIdV1::from_index(0),
                        root,
                        budget,
                        |cfg, budget| {
                            let observed = Cell::new(0);
                            let result = with_assertion_scope(
                                &source,
                                SemanticFunctionIdV1::from_index(0),
                                cfg,
                                budget,
                                |_, budget| {
                                    observed.set(budget.storage());
                                    budget.release_storage(1)?;
                                    Ok(())
                                },
                            );
                            assert_eq!(result, Err(Resource::Accounting.into()));
                            assert!(observed.get() > FLOOR);
                            assert_eq!(budget.storage(), observed.get() - 1);
                            result
                        },
                    )
                },
            )
        },
    );
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert!(budget.storage() > FLOOR);
}
#[test]
fn original_ledger_replacement_is_not_repaired() {
    let source = source_fixture(false);
    let mut original = Work::new(LIMIT);
    let mut foreign = Work::new(LIMIT);
    let replacement = Budget::new(&mut foreign, LIMIT);
    let mut budget = Budget::new(&mut original, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    assert_eq!(
        run(&source, &source, &mut budget, |_, budget| {
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
fn preexisting_denial_never_enters_any_mask_observer() {
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
            run::<(), _>(&source, &source, &mut budget, |_, _| panic!("denied")),
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
fn complete_callback_and_copy_result_frames_are_paid_and_dropped() {
    assert!(assertion_header::<[u8; 8192]>(size_of::<LargeCapture<'_>>()).unwrap() > 4 * 8192);
    let source = source_fixture(false);
    let dropped = Cell::new(false);
    let capture = LargeCapture {
        bytes: [7; 8192],
        dropped: &dropped,
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = run(&source, &source, &mut budget, move |view, budget| {
        budget.charge_work(8192 + 32)?;
        assert_eq!(view.decisions(), [false; 3]);
        let bytes = capture.bytes;
        drop(capture);
        Ok(bytes)
    })
    .unwrap();
    assert_eq!(result, [7; 8192]);
    assert!(dropped.get());
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn large_capture_drops_on_partial_refusal_and_callback_unwind() {
    for partial in [false, true] {
        let source = source_fixture(partial);
        let dropped = Cell::new(false);
        let capture = LargeCapture {
            bytes: [9; 8192],
            dropped: &dropped,
        };
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = run(&source, &source, &mut budget, move |_, _| -> Result<()> {
            assert_eq!(capture.bytes[0], 9);
            // Keep the Drop capture live through the unwind.
            let _capture = capture;
            panic!("assertion owned callback capture unwind")
        });
        assert_eq!(
            result,
            if partial {
                Err(QueryError::Unavailable("actual source preparation refused"))
            } else {
                Err(QueryError::CallbackPanicked)
            }
        );
        assert!(dropped.get());
        assert_eq!(budget.storage(), FLOOR);
    }
}
#[test]
fn checked_generic_header_overflow_refuses_without_reservation() {
    assert_eq!(
        assertion_header::<()>(usize::MAX),
        Err(Resource::Arithmetic.into())
    );
}
