use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::{cell::Cell, rc::Rc};

const FUNCTION: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const FLOOR: usize = 37;
const PREFIX: usize = 11;
const LIMIT: usize = 1_000_000;

fn block(index: u32) -> SemanticBlockIdV1 {
    SemanticBlockIdV1::from_index(index)
}
fn place(index: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap()
}
fn copy(index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(index, ty))
}
fn literal(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        WORD,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn assign(
    index: u32,
    ty: SemanticTypeIdV1,
    operation: SemanticBinaryOpV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(index, ty),
        SemanticRvalueV1::new(
            ty,
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            },
        ),
    )))
}
fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn fixture(mask: u128) -> AdmittedInertSemanticMirV1 {
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let types = [(32, u128::from(u32::MAX)), (8, 1)]
        .into_iter()
        .enumerate()
        .map(|(index, (bits, maximum))| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([1 + index as u8 * 2; 32]),
                SemanticLayoutIdentityV1::from_sha256([2 + index as u8 * 2; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(bits / 8),
                    bits / 8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, bits as u16, bits / 8),
                        SemanticScalarValidityRangeV1::new(0, maximum),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(if index == 0 {
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    }
                } else {
                    SemanticScalarTypeV1::Bool
                }),
            )
        })
        .collect();
    let locals = [WORD, WORD, WORD, WORD, BOOL]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([20 + index as u8; 32]),
                ty,
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    2 => SemanticLocalRoleV1::Argument(1),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                provenance,
            )
        })
        .collect();
    let blocks = [
        (
            vec![
                statement(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(3),
                )),
                assign(
                    3,
                    WORD,
                    SemanticBinaryOpV1::BitAnd,
                    copy(2, WORD),
                    literal(mask),
                ),
                assign(
                    4,
                    BOOL,
                    SemanticBinaryOpV1::LessThan,
                    copy(3, WORD),
                    literal(32),
                ),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Move(place(4, BOOL)),
                expected: true,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::ShiftLeft,
                    left: copy(1, WORD),
                    right: copy(3, WORD),
                },
                target: SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::AssertSuccess, block(1)),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        (
            vec![
                assign(
                    0,
                    WORD,
                    SemanticBinaryOpV1::ShiftLeft,
                    copy(1, WORD),
                    SemanticOperandV1::Move(place(3, WORD)),
                ),
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(3),
                )),
            ],
            SemanticTerminatorKindV1::Return,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (statements, terminator))| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([40 + index as u8; 32]),
            provenance,
            statements,
            SemanticTerminatorV1::new(provenance, terminator),
        )
        .unwrap()
    })
    .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([50; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([51; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([52; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([53; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([54; 32]),
        provenance,
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([55; 32]),
            SemanticLayoutIdentityV1::from_sha256([56; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct(WORD), direct(WORD)],
            direct(WORD),
        )
        .unwrap(),
        locals,
        block(0),
        blocks,
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([57; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![FUNCTION],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn measured(source: &AdmittedInertSemanticMirV1) -> (usize, usize) {
    let index = SemanticMaskedShiftIndexV1::analyze(
        source,
        FUNCTION,
        SemanticMaskedShiftLimitsV1::default(),
    )
    .unwrap();
    (index.work_units(), index.storage_bytes())
}
fn prepared(work: &mut Work, storage: usize) -> Budget<'_> {
    let mut budget = Budget::new(work, storage);
    budget.charge_work(PREFIX).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    budget
}
fn scope<'source, 'work, T>(
    source: &'source AdmittedInertSemanticMirV1,
    budget: &mut Budget<'work>,
    run: impl for<'s> FnOnce(
        &mut ProductionSemanticMaskedShiftQueryV1<'s, 'source>,
        &mut Budget<'work>,
    ) -> R<T>,
) -> R<T> {
    with_production_semantic_masked_shift_query_v1(
        source,
        FUNCTION,
        SemanticMaskedShiftLimitsV1::default(),
        budget,
        run,
    )
}

#[test]
fn exact_live_constructor_and_lookup_limits_preserve_inherited_floor_and_fact_borrow() {
    let source = fixture(31);
    let (constructor, storage) = measured(&source);
    let mut work = Work::new(PREFIX + constructor + 3);
    let mut budget = prepared(&mut work, FLOOR + storage);
    let fact = scope(&source, &mut budget, |query, budget| {
        assert_eq!(budget.storage(), FLOOR + storage);
        assert!(query.assertion(block(0), budget)?.is_some());
        query.shift(block(1), 0, budget)
    })
    .unwrap()
    .unwrap();
    assert!(std::ptr::eq(fact.owner(), &source));
    assert_eq!(fact.function(), FUNCTION);
    assert_eq!(fact.assertion_block(), block(0));
    assert_eq!(fact.successor_block(), block(1));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + storage);
    assert_eq!(budget.work(), PREFIX + constructor + 3);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn constructor_one_short_work_and_storage_report_exact_external_errors_and_cleanup() {
    let source = fixture(31);
    let (constructor, storage) = measured(&source);
    for short_storage in [false, true] {
        let work_limit = PREFIX + constructor - usize::from(!short_storage);
        let storage_limit = FLOOR + storage - usize::from(short_storage);
        let mut work = Work::new(work_limit);
        let mut budget = prepared(&mut work, storage_limit);
        let called = Cell::new(false);
        let result = scope(&source, &mut budget, |_, _| {
            called.set(true);
            Ok(())
        });
        match result {
            Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(ResourceError::Storage(
                error,
            ))) if short_storage => {
                assert_eq!(error.actual(), FLOOR + storage);
                assert_eq!(error.limit(), storage_limit);
                assert_eq!(budget.failed_storage(), Some(FLOOR + storage));
            }
            Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(ResourceError::Work(
                error,
            ))) if !short_storage => {
                assert_eq!(error.actual(), PREFIX + constructor);
                assert_eq!(error.limit(), work_limit);
                assert_eq!(budget.failed_storage(), None);
            }
            _ => panic!("unexpected constructor outcome {result:?}"),
        }
        assert!(!called.get());
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work() >= PREFIX && budget.work() <= work_limit);
        assert!(budget.peak_storage() <= storage_limit);
    }
}

#[test]
fn lookup_one_short_failures_are_not_missing_facts_and_cannot_be_swallowed() {
    let source = fixture(31);
    let (constructor, storage) = measured(&source);
    for shift in [false, true] {
        let lookup = if shift { 2 } else { 1 };
        let limit = PREFIX + constructor + lookup - 1;
        let mut work = Work::new(limit);
        let mut budget = prepared(&mut work, FLOOR + storage);
        let dropped = Rc::new(Cell::new(0));
        let result = scope(&source, &mut budget, |query, budget| {
            let result = if shift {
                query.shift(block(1), 0, budget)
            } else {
                query.assertion(block(0), budget)
            };
            let error = match result {
                Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(ResourceError::Work(
                    error,
                ))) => error,
                _ => panic!("resource failure became a fact"),
            };
            assert_eq!(error.actual(), PREFIX + constructor + lookup);
            assert_eq!(error.limit(), limit);
            assert_eq!(budget.work(), PREFIX + constructor);
            Ok(DropCounter(dropped.clone(), false))
        });
        assert!(matches!(
            result,
            Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(
                ResourceError::Work(_)
            ))
        ));
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), PREFIX + constructor);
    }
}

#[test]
fn local_limits_and_absent_facts_do_not_change_live_error_domains() {
    let source = fixture(31);
    let (constructor, storage) = measured(&source);
    for local_storage_short in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = with_production_semantic_masked_shift_query_v1(
            &source,
            FUNCTION,
            SemanticMaskedShiftLimitsV1::new(
                constructor - usize::from(!local_storage_short),
                storage - usize::from(local_storage_short),
            ),
            &mut budget,
            |_, _| Ok(()),
        );
        match result {
            Err(ProductionSemanticMaskedShiftQueryErrorV1::Analysis(
                SemanticMaskedShiftErrorV1::StorageLimit { .. },
            )) if local_storage_short => {}
            Err(ProductionSemanticMaskedShiftQueryErrorV1::Analysis(
                SemanticMaskedShiftErrorV1::WorkLimit { .. },
            )) if !local_storage_short => {}
            _ => panic!("wrong local limit domain: {result:?}"),
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
    let hostile = fixture(30);
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    scope(&hostile, &mut budget, |query, budget| {
        assert!(query.assertion(block(0), budget)?.is_none());
        assert!(query.shift(block(1), 0, budget)?.is_none());
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

struct DropCounter(Rc<Cell<usize>>, bool);
impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        assert!(!self.1, "test returned-owner destructor panic");
    }
}

#[test]
fn callback_error_and_panic_drop_locals_and_restore_floor_without_resetting_work() {
    let source = fixture(31);
    let (constructor, storage) = measured(&source);
    for panic in [false, true] {
        let dropped = Rc::new(Cell::new(0));
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let error = ProductionSemanticMaskedShiftQueryErrorV1::Analysis(
            SemanticMaskedShiftErrorV1::InvalidModel("callback sentinel"),
        );
        let result: R<()> = scope(&source, &mut budget, |_, budget| {
            let _local = DropCounter(dropped.clone(), false);
            budget.charge_work(5)?;
            if panic {
                panic!("test callback panic");
            }
            Err(error)
        });
        assert_eq!(
            result,
            Err(if panic {
                ProductionSemanticMaskedShiftQueryErrorV1::Panicked
            } else {
                error
            })
        );
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), PREFIX + constructor + 5);
        assert_eq!(budget.peak_storage(), FLOOR + storage);
    }
}

#[test]
fn callback_extra_reservation_rejects_and_drops_output_before_cleanup_even_if_drop_panics() {
    let source = fixture(31);
    let (_, storage) = measured(&source);
    for panic_drop in [false, true] {
        let dropped = Rc::new(Cell::new(0));
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = scope(&source, &mut budget, |_, budget| {
            budget.reserve_storage(13)?;
            Ok(DropCounter(dropped.clone(), panic_drop))
        });
        assert!(matches!(
            result,
            Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(
                ResourceError::Accounting
            ))
        ));
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + storage + 13);
    }
}

#[test]
fn panicking_panic_payload_destructor_cannot_skip_query_cleanup() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("test panic payload destructor");
        }
    }
    let source = fixture(31);
    let (constructor, storage) = measured(&source);
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let drops = Arc::new(AtomicUsize::new(0));
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _: R<()> = scope(&source, &mut budget, |_, _| {
            std::panic::panic_any(Payload(drops.clone()));
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + storage);
    assert_eq!(budget.work(), PREFIX + constructor);
}

#[test]
fn temporary_callback_storage_must_be_restored_before_query_and_exit() {
    let source = fixture(31);
    for restore in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = scope(&source, &mut budget, |query, budget| {
            budget.reserve_storage(7)?;
            if restore {
                budget.release_storage(7)?;
            }
            query.assertion(block(0), budget).map(|_| ())
        });
        if restore {
            result.unwrap();
        } else {
            assert_eq!(result, Err(ResourceError::Accounting.into()));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn entry_floor_undercut_disables_cleanup_even_after_attempted_restoration() {
    let source = fixture(31);
    let (_, storage) = measured(&source);
    for restore in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = scope(&source, &mut budget, |query, budget| {
            budget.release_storage(storage + 1)?;
            assert!(matches!(
                query.assertion(block(0), budget),
                Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(
                    ResourceError::Accounting
                ))
            ));
            if restore {
                budget.reserve_storage(storage + 1)?;
            }
            Ok(())
        });
        assert_eq!(result, Err(ResourceError::Accounting.into()));
        assert_eq!(
            budget.storage(),
            if restore { FLOOR + storage } else { FLOOR - 1 }
        );
        // Deliberate hostile ownership violation; recovery belongs to this caller.
        if restore {
            budget.release_storage(storage).unwrap();
        } else {
            budget.reserve_storage(1).unwrap();
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn exit_only_floor_undercut_drops_callback_output_without_releasing_current_storage() {
    let source = fixture(31);
    let (_, storage) = measured(&source);
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let dropped = Rc::new(Cell::new(0));
    let result = scope(&source, &mut budget, |_, budget| {
        budget.release_storage(storage + 1)?;
        Ok(DropCounter(dropped.clone(), false))
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(
            ResourceError::Accounting
        ))
    ));
    assert_eq!(dropped.get(), 1);
    assert_eq!(budget.storage(), FLOOR - 1);
    budget.reserve_storage(1).unwrap();
}

#[test]
fn wrong_budget_slot_with_the_same_work_ledger_is_rejected_without_metering_it() {
    let source = fixture(31);
    let (_, storage) = measured(&source);
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let mut spare_work = Work::new(LIMIT);
    let mut spare = Budget::new(&mut spare_work, LIMIT);
    let result = scope(&source, &mut budget, |query, budget| {
        let ledger = budget.work_ledger_identity_v1();
        let current_work = budget.work();
        // Moving the original Budget changes only its slot; its Work is unchanged.
        std::mem::swap(budget, &mut spare);
        assert!(spare.work_ledger_identity_v1() == ledger);
        assert!(matches!(
            query.assertion(block(0), &mut spare),
            Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(
                ResourceError::Accounting
            ))
        ));
        assert_eq!(spare.work(), current_work);
        assert_eq!(spare.storage(), FLOOR + storage);
        std::mem::swap(budget, &mut spare);
        Ok(())
    });
    assert_eq!(result, Err(ResourceError::Accounting.into()));
    assert_eq!(budget.storage(), FLOOR + storage);
    budget.release_storage(storage).unwrap();
}

#[test]
fn replaced_work_ledger_is_never_charged_or_released_on_ok_error_or_panic() {
    let source = fixture(31);
    let (_, storage) = measured(&source);
    for (exit, query_after_swap) in (0..3).flat_map(|exit| [false, true].map(|query| (exit, query)))
    {
        let mut original_work = Work::new(LIMIT);
        let mut foreign_work = Work::new(LIMIT);
        let mut budget = prepared(&mut original_work, LIMIT);
        let original_ledger = budget.work_ledger_identity_v1();
        let mut foreign = prepared(&mut foreign_work, LIMIT);
        foreign.reserve_storage(storage).unwrap();
        let foreign_ledger = foreign.work_ledger_identity_v1();
        assert!(original_ledger != foreign_ledger);
        let result: R<()> = scope(&source, &mut budget, |query, budget| {
            std::mem::swap(budget, &mut foreign);
            if query_after_swap {
                assert!(matches!(
                    query.shift(block(1), 0, budget),
                    Err(ProductionSemanticMaskedShiftQueryErrorV1::Resource(
                        ResourceError::Accounting
                    ))
                ));
            }
            match exit {
                0 => Ok(()),
                1 => Err(ProductionSemanticMaskedShiftQueryErrorV1::Analysis(
                    SemanticMaskedShiftErrorV1::InvalidModel("callback"),
                )),
                _ => panic!("test replaced ledger panic"),
            }
        });
        assert_eq!(result, Err(ResourceError::Accounting.into()));
        assert!(budget.work_ledger_identity_v1() == foreign_ledger);
        assert!(foreign.work_ledger_identity_v1() == original_ledger);
        assert_eq!((budget.work(), budget.storage()), (PREFIX, FLOOR + storage));
        assert_eq!(foreign.storage(), FLOOR + storage);
        budget.release_storage(storage).unwrap();
        foreign.release_storage(storage).unwrap();
    }
}
