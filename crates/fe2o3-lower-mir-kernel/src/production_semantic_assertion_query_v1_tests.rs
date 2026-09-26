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

fn fixture(shift: Option<u128>) -> AdmittedInertSemanticMirV1 {
    fixture_with_assertion(shift, false)
}

fn literal_fixture() -> AdmittedInertSemanticMirV1 {
    fixture_with_assertion(None, true)
}

fn fixture_with_assertion(shift: Option<u128>, literal_bool: bool) -> AdmittedInertSemanticMirV1 {
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
    let blocks = if literal_bool {
        [
            (
                vec![],
                SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                        BOOL,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
                    )),
                    expected: true,
                    message: SemanticAssertMessageV1::DivisionByZero(literal(1)),
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        block(1),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            (
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        place(0, WORD),
                        SemanticRvalueV1::new(WORD, SemanticRvalueKindV1::Use(literal(0))),
                    ),
                ))],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    } else {
        [
            (
                vec![
                    statement(SemanticStatementKindV1::StorageLive(
                        SemanticLocalIdV1::from_index(3),
                    )),
                    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(3, WORD),
                        SemanticRvalueV1::new(
                            WORD,
                            SemanticRvalueKindV1::Use(
                                shift.map(literal).unwrap_or_else(|| copy(2, WORD)),
                            ),
                        ),
                    ))),
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
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        block(1),
                    ),
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
    };
    let blocks = blocks
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

fn prepared(work: &mut Work, storage: usize) -> Budget<'_> {
    let mut budget = Budget::new(work, storage);
    budget.charge_work(PREFIX).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    budget
}
fn headers<T>() -> usize {
    std::mem::size_of::<Accounting>()
        + std::mem::size_of::<ProductionSemanticAssertionQueryV1<'_, '_>>()
        + std::mem::size_of::<std::thread::Result<R<T>>>()
        + std::mem::size_of::<Outcome<'_, '_>>()
        + std::mem::size_of::<resources::Cleanup<'_, '_>>()
}
fn scope<'source, 'work, T>(
    source: &'source AdmittedInertSemanticMirV1,
    budget: &mut Budget<'work>,
    run: impl for<'s> FnOnce(
        &mut ProductionSemanticAssertionQueryV1<'s, 'source>,
        &mut Budget<'work>,
    ) -> R<T>,
) -> R<T> {
    with_production_semantic_assertion_query_v1(
        source,
        FUNCTION,
        Limits::new(LIMIT, LIMIT),
        budget,
        run,
    )
}

#[test]
fn live_fact_names_the_actual_source_assertion_and_return_is_not_an_assertion() {
    let source = fixture(Some(8));
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    scope(&source, &mut budget, |query, budget| {
        let Outcome::Proved(fact) = query.assertion(block(0), budget)? else {
            panic!("literal shift assertion was not proved");
        };
        assert!(std::ptr::eq(fact.types(), source.types()));
        assert!(std::ptr::eq(fact.function(), &source.functions()[0]));
        assert_eq!(fact.block(), block(0));
        assert!(fact.expected());
        assert_eq!(fact.success_edge().target(), block(1));
        assert!(std::ptr::eq(
            fact.assertion(),
            source.functions()[0].blocks()[0].terminator().kind()
        ));
        assert!(matches!(
            query.assertion(block(1), budget)?,
            Outcome::NotAnAssertion
        ));
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work() > PREFIX + 2);
    assert!(budget.peak_storage() >= FLOOR + headers::<()>() + std::mem::size_of::<Analysis<'_>>());
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn refuted_and_unknown_predicates_do_not_become_success_facts() {
    for (shift, refuted) in [(Some(32), true), (None, false)] {
        let source = fixture(shift);
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        scope(&source, &mut budget, |query, budget| {
            match query.assertion(block(0), budget)? {
                Outcome::Refuted(fact) if refuted => {
                    assert!(std::ptr::eq(fact.function(), &source.functions()[0]));
                    assert_eq!(fact.block(), block(0));
                }
                Outcome::NotProved(_) if !refuted => {}
                _ => panic!("unproved source assertion became a fact"),
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn invalid_query_poison_survives_an_ignored_error_and_drops_returned_output() {
    let source = fixture(Some(8));
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let drops = Rc::new(Cell::new(0));
    let result = scope(&source, &mut budget, |query, budget| {
        let first = query
            .assertion(block(99), budget)
            .err()
            .expect("invalid block accepted");
        assert!(matches!(
            first,
            ProductionSemanticAssertionQueryErrorV1::Analysis(AnalysisError::InvalidModel(_))
        ));
        let accepted = budget.work();
        assert_eq!(query.assertion(block(0), budget).err(), Some(first));
        assert_eq!(budget.work(), accepted);
        Ok(DropCounter(drops.clone(), false))
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticAssertionQueryErrorV1::Analysis(
            AnalysisError::InvalidModel(_)
        ))
    ));
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn entry_work_and_both_header_reservations_have_source_derived_boundaries() {
    let source = fixture(Some(8));
    let wrapper = headers::<()>();
    let analysis = std::mem::size_of::<Analysis<'_>>();
    for mode in 0..4 {
        let work_limit = if mode == 0 {
            PREFIX + 1
        } else if mode == 3 {
            PREFIX + 2
        } else {
            LIMIT
        };
        let storage_limit = match mode {
            1 => FLOOR + wrapper - 1,
            2 => FLOOR + wrapper + analysis - 1,
            _ => LIMIT,
        };
        let mut work = Work::new(work_limit);
        let mut budget = prepared(&mut work, storage_limit);
        let called = Cell::new(false);
        let result = scope(&source, &mut budget, |_, _| {
            called.set(true);
            Ok(())
        });
        match result {
            Err(ProductionSemanticAssertionQueryErrorV1::Resource(Resource::Work(error)))
                if mode == 0 || mode == 3 =>
            {
                assert_eq!(error.actual(), PREFIX + if mode == 0 { 2 } else { 3 });
                assert_eq!(error.limit(), work_limit);
                assert_eq!(budget.failed_storage(), None);
            }
            Err(ProductionSemanticAssertionQueryErrorV1::Resource(Resource::Storage(error)))
                if mode == 1 || mode == 2 =>
            {
                let attempted = FLOOR + wrapper + if mode == 2 { analysis } else { 0 };
                assert_eq!(error.actual(), attempted);
                assert_eq!(error.limit(), storage_limit);
                assert_eq!(budget.failed_storage(), Some(attempted));
            }
            other => panic!("unexpected entry refusal: {other:?}"),
        }
        assert!(!called.get());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), PREFIX + if mode == 0 { 0 } else { 2 });
    }
}

#[test]
fn local_limits_are_distinct_from_external_meter_denials() {
    let source = fixture(Some(8));
    for storage_denial in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = with_production_semantic_assertion_query_v1(
            &source,
            FUNCTION,
            Limits::new(
                if storage_denial { LIMIT } else { 0 },
                if storage_denial { 0 } else { LIMIT },
            ),
            &mut budget,
            |_, _| Ok(()),
        );
        match result {
            Err(ProductionSemanticAssertionQueryErrorV1::Analysis(
                AnalysisError::StorageLimit { actual, limit: 0 },
            )) if storage_denial => assert_eq!(actual, std::mem::size_of::<Analysis<'_>>()),
            Err(ProductionSemanticAssertionQueryErrorV1::Analysis(AnalysisError::WorkLimit {
                actual: 1,
                limit: 0,
            })) if !storage_denial => {}
            other => panic!("unexpected local refusal: {other:?}"),
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), PREFIX + 2);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn query_debits_the_live_ledger_and_denial_cannot_be_swallowed() {
    let source = fixture(Some(8));
    // Zero remaining denies the facade charge; one denies the engine charge.
    for remaining in [0, 1] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = scope(&source, &mut budget, |query, budget| {
            budget.charge_work(LIMIT - budget.work() - remaining)?;
            assert!(matches!(
                query.assertion(block(0), budget),
                Err(ProductionSemanticAssertionQueryErrorV1::Resource(
                    Resource::Work(_)
                ))
            ));
            Ok(())
        });
        match result {
            Err(ProductionSemanticAssertionQueryErrorV1::Resource(Resource::Work(error))) => {
                assert_eq!(error.actual(), LIMIT + 1);
                assert_eq!(error.limit(), LIMIT);
            }
            other => panic!("query denial lost: {other:?}"),
        }
        assert_eq!(budget.work(), LIMIT);
        assert_eq!(budget.storage(), FLOOR);
    }
}

// B=2, E=1, L=5 and one assignment: CFG backing, definition inventory,
// then checked-assertion rows. These sizes come from source, not a trial run.
fn literal_construction_storage() -> (usize, usize, usize) {
    use fe2o3_mir_model::semantic_assertion_v1::{ScalarAssignmentSiteV1, SemanticAssertionCfgV1};
    use std::mem::size_of;
    let cfg_header = size_of::<Analysis<'_>>()
        + size_of::<SemanticAssertionCfgV1>()
        + 3 * size_of::<Vec<usize>>();
    let cfg =
        cfg_header + 4 * size_of::<Vec<usize>>() + 6 * size_of::<usize>() + 2 * size_of::<bool>();
    let complete = cfg
        + 5 * size_of::<u8>()
        + 5 * size_of::<Option<ScalarAssignmentSiteV1>>()
        + 5 * size_of::<bool>()
        + 2 * size_of::<Vec<usize>>()
        + 4 * size_of::<usize>()
        + 5 * size_of::<Vec<usize>>();
    (cfg_header, cfg, complete)
}

#[test]
fn actual_engine_query_work_limits_preserve_sticky_refusal_and_exact_debits() {
    let source = literal_fixture();
    // Construction: 1 + CFG34 + inventory33 + checked-index9. Query: 1 + 1.
    for limit in [77, 78, 79] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = with_production_semantic_assertion_query_v1(
            &source,
            FUNCTION,
            Limits::new(limit, LIMIT),
            &mut budget,
            |query, budget| {
                assert_eq!(budget.work(), PREFIX + 2 + 77);
                assert_eq!(
                    budget.storage(),
                    FLOOR + headers::<()>() + literal_construction_storage().2
                );
                let outcome = query.assertion(block(0), budget);
                if limit == 79 {
                    assert!(matches!(outcome, Ok(Outcome::Proved(_))));
                } else {
                    let error = outcome.err().expect("engine work limit accepted");
                    assert_eq!(
                        error,
                        ProductionSemanticAssertionQueryErrorV1::Analysis(
                            AnalysisError::WorkLimit {
                                actual: limit + 1,
                                limit
                            }
                        )
                    );
                    let accepted = budget.work();
                    assert_eq!(query.assertion(block(0), budget).err(), Some(error));
                    assert_eq!(budget.work(), accepted);
                }
                Ok(())
            },
        );
        if limit == 79 {
            result.unwrap();
        } else {
            assert_eq!(
                result,
                Err(ProductionSemanticAssertionQueryErrorV1::Analysis(
                    AnalysisError::WorkLimit {
                        actual: limit + 1,
                        limit
                    }
                ))
            );
        }
        assert_eq!(budget.work(), PREFIX + 2 + 1 + limit);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn partial_cfg_and_inventory_allocations_drop_before_restoring_caller_floor() {
    use fe2o3_mir_model::semantic_assertion_v1::ScalarAssignmentSiteV1;
    use std::mem::size_of;
    let source = literal_fixture();
    let (cfg_header, cfg, _) = literal_construction_storage();
    for (accepted, next) in [
        (
            cfg_header + 2 * size_of::<Vec<usize>>(),
            2 * size_of::<Vec<usize>>(),
        ),
        (
            cfg + 5 * size_of::<u8>(),
            5 * size_of::<Option<ScalarAssignmentSiteV1>>(),
        ),
    ] {
        for external in [false, true] {
            let prefix = FLOOR + headers::<()>();
            let mut work = Work::new(LIMIT);
            let mut budget = prepared(&mut work, if external { prefix + accepted } else { LIMIT });
            let called = Cell::new(false);
            let result = with_production_semantic_assertion_query_v1(
                &source,
                FUNCTION,
                Limits::new(LIMIT, if external { LIMIT } else { accepted }),
                &mut budget,
                |_, _| {
                    called.set(true);
                    Ok(())
                },
            );
            match result {
                Err(ProductionSemanticAssertionQueryErrorV1::Analysis(
                    AnalysisError::StorageLimit { actual, limit },
                )) if !external => {
                    assert_eq!((actual, limit), (accepted + next, accepted));
                    assert_eq!(budget.failed_storage(), None);
                }
                Err(ProductionSemanticAssertionQueryErrorV1::Resource(Resource::Storage(
                    error,
                ))) if external => {
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (prefix + accepted + next, prefix + accepted)
                    );
                    assert_eq!(budget.failed_storage(), Some(prefix + accepted + next));
                }
                other => panic!("partial construction denial lost: {other:?}"),
            }
            assert!(!called.get());
            assert_eq!(budget.peak_storage(), prefix + accepted);
            assert_eq!(budget.storage(), FLOOR);
            assert!(budget.work() > PREFIX + 2);
        }
    }
}

#[test]
fn query_scratch_denial_after_paid_construction_is_sticky_and_restores_floor() {
    let source = literal_fixture();
    let storage = literal_construction_storage().2;
    let prefix = FLOOR + headers::<()>();
    for external in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, if external { prefix + storage } else { LIMIT });
        let called = Cell::new(false);
        let result = with_production_semantic_assertion_query_v1(
            &source,
            FUNCTION,
            Limits::new(LIMIT, if external { LIMIT } else { storage }),
            &mut budget,
            |query, budget| {
                called.set(true);
                assert_eq!(budget.work(), PREFIX + 2 + 77);
                assert_eq!(budget.storage(), prefix + storage);
                let error = query
                    .assertion(block(0), budget)
                    .err()
                    .expect("query scratch accepted");
                match error {
                    ProductionSemanticAssertionQueryErrorV1::Analysis(
                        AnalysisError::StorageLimit { actual, limit },
                    ) if !external => {
                        assert_eq!(limit, storage);
                        assert!(actual > storage);
                    }
                    ProductionSemanticAssertionQueryErrorV1::Resource(Resource::Storage(error))
                        if external =>
                    {
                        assert_eq!(error.limit(), prefix + storage);
                        assert!(error.actual() > error.limit());
                        assert_eq!(budget.failed_storage(), Some(error.actual()));
                    }
                    other => panic!("query scratch refusal changed domain: {other:?}"),
                }
                let accepted = budget.work();
                assert_eq!(query.assertion(block(0), budget).err(), Some(error));
                assert_eq!(budget.work(), accepted);
                Ok(())
            },
        );
        assert!(called.get());
        assert!(result.is_err());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), prefix + storage);
    }
}

struct DropCounter(Rc<Cell<usize>>, bool);
impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        assert!(!self.1, "test callback-result destructor panic");
    }
}

#[test]
fn callback_errors_panics_and_rejected_panicking_outputs_restore_the_floor() {
    let source = fixture(Some(8));
    for mode in 0..4 {
        let drops = Rc::new(Cell::new(0));
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result: R<DropCounter> = scope(&source, &mut budget, |_, budget| {
            let output = DropCounter(drops.clone(), mode == 3);
            match mode {
                0 => Err(ProductionSemanticAssertionQueryErrorV1::Analysis(
                    AnalysisError::InvalidModel("callback"),
                )),
                1 => panic!("test callback panic"),
                _ => {
                    budget.reserve_storage(7)?;
                    Ok(output)
                }
            }
        });
        let error = result.err().expect("hostile callback accepted");
        match mode {
            0 => assert!(matches!(
                error,
                ProductionSemanticAssertionQueryErrorV1::Analysis(_)
            )),
            1 => assert_eq!(error, ProductionSemanticAssertionQueryErrorV1::Panicked),
            _ => assert_eq!(error, Resource::Accounting.into()),
        }
        assert_eq!(drops.get(), 1);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn restored_callback_scratch_is_allowed_but_unrestored_scratch_poisons_queries() {
    let source = fixture(Some(8));
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
            assert_eq!(result, Err(Resource::Accounting.into()));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn observed_caller_floor_undercut_disables_refund_even_after_restoration() {
    let source = fixture(Some(8));
    for restore in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let paid = Cell::new(0);
        let result = scope(&source, &mut budget, |query, budget| {
            paid.set(budget.storage() - FLOOR);
            budget.release_storage(paid.get() + 1)?;
            assert_eq!(
                query.assertion(block(0), budget).err(),
                Some(Resource::Accounting.into())
            );
            if restore {
                budget.reserve_storage(paid.get() + 1)?;
            }
            Ok(())
        });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(
            budget.storage(),
            if restore {
                FLOOR + paid.get()
            } else {
                FLOOR - 1
            }
        );
        if restore {
            budget.release_storage(paid.get()).unwrap();
        } else {
            budget.reserve_storage(1).unwrap();
        }
    }
}

#[test]
fn moving_the_budget_slot_never_charges_the_moved_query_ledger() {
    let source = fixture(Some(8));
    let mut work = Work::new(LIMIT);
    let mut other_work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let mut spare = Budget::new(&mut other_work, LIMIT);
    let paid = Cell::new(0);
    let result = scope(&source, &mut budget, |query, budget| {
        paid.set(budget.storage() - FLOOR);
        let accepted = budget.work();
        let identity = budget.work_ledger_identity_v1();
        std::mem::swap(budget, &mut spare);
        assert!(spare.work_ledger_identity_v1() == identity);
        assert_eq!(
            query.assertion(block(0), &mut spare).err(),
            Some(Resource::Accounting.into())
        );
        assert_eq!(spare.work(), accepted);
        std::mem::swap(budget, &mut spare);
        Ok(())
    });
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert_eq!(budget.storage(), FLOOR + paid.get());
    budget.release_storage(paid.get()).unwrap();
}

#[test]
fn substituted_ledger_is_never_charged_or_refunded_on_any_callback_exit() {
    let source = fixture(Some(8));
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut foreign_work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let mut foreign = prepared(&mut foreign_work, LIMIT);
        let foreign_identity = foreign.work_ledger_identity_v1();
        let paid = Cell::new(0);
        let result: R<()> = scope(&source, &mut budget, |query, budget| {
            paid.set(budget.storage() - FLOOR);
            foreign.reserve_storage(paid.get())?;
            std::mem::swap(budget, &mut foreign);
            assert_eq!(
                query.assertion(block(0), budget).err(),
                Some(Resource::Accounting.into())
            );
            match mode {
                0 => Ok(()),
                1 => Err(Resource::Arithmetic.into()),
                _ => panic!("test foreign-ledger callback panic"),
            }
        });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert!(budget.work_ledger_identity_v1() == foreign_identity);
        assert_eq!(
            (budget.work(), budget.storage()),
            (PREFIX, FLOOR + paid.get())
        );
        assert_eq!(foreign.storage(), FLOOR + paid.get());
        budget.release_storage(paid.get()).unwrap();
        foreign.release_storage(paid.get()).unwrap();
    }
}

#[test]
fn recursively_panicking_payload_destructors_cannot_skip_ledger_cleanup() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>, usize);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            if self.1 > 0 {
                std::panic::panic_any(Payload(self.0.clone(), self.1 - 1));
            }
            panic!("test terminal payload destructor panic");
        }
    }
    let source = fixture(Some(8));
    for nested in [false, true] {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = catch_unwind(AssertUnwindSafe(|| {
            scope::<()>(&source, &mut budget, |_, _| {
                std::panic::panic_any(Payload(drops.clone(), usize::from(nested)));
            })
        }));
        if nested {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap(),
                Err(ProductionSemanticAssertionQueryErrorV1::Panicked)
            );
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1 + usize::from(nested));
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work() > PREFIX + 2);
    }
}

#[test]
fn invalid_function_refuses_before_analysis_construction_or_callback() {
    let source = fixture(Some(8));
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let called = Cell::new(false);
    let result = with_production_semantic_assertion_query_v1(
        &source,
        SemanticFunctionIdV1::from_index(99),
        Limits::new(LIMIT, LIMIT),
        &mut budget,
        |_, _| {
            called.set(true);
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticAssertionQueryErrorV1::Analysis(
            AnalysisError::InvalidModel(_)
        ))
    ));
    assert!(!called.get());
    assert_eq!(budget.work(), PREFIX + 2);
    assert_eq!(budget.peak_storage(), FLOOR + headers::<()>());
    assert_eq!(budget.storage(), FLOOR);
}
