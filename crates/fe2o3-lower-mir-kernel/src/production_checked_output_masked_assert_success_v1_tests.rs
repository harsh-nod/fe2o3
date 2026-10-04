use super::*;
use crate::production_semantic_kir_v1::helper_source_fixture_v1 as fixture;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::SemanticMaskedShiftLimitsV1;
use fe2o3_mir_model::semantic_mir_v1::*;

const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const FLOOR: usize = 29;
const WORK: usize = 100_000_000;
const STORAGE: usize = 128 << 20;

#[derive(Clone, Copy, Debug)]
enum Shape {
    Exact,
    Raw,
    WrongMask,
    WrongMessage,
    WrongLimit,
    FalseExpected,
    Gap,
    ExtraShift,
    UnreachableShift,
}

fn literal(bits: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        fixture::WORD,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
    ))
}

fn boolean() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    )
}

fn input(shape: Shape, direction: SemanticBinaryOpV1, helper: bool) -> AdmittedInertSemanticMirV1 {
    let original = fixture::source(vec![fixture::function(
        60,
        false,
        1,
        vec![fixture::block(
            61,
            vec![fixture::assign(
                0,
                SemanticRvalueKindV1::Use(fixture::copy(1)),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    )]);
    let mut types = original.types().to_vec();
    types.push(boolean());
    let index = usize::from(helper);
    let source = &original.functions()[index];
    let mut locals = source.locals()[..4].to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([101; 32]),
        BOOL,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    if !helper {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([102; 32]),
            fixture::WORD,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let result_local = if helper { 0 } else { 5 };
    let shift = |right| {
        fixture::assign(
            result_local,
            SemanticRvalueKindV1::Binary {
                operation: direction,
                left: fixture::copy(1),
                right,
            },
        )
    };
    let mut success = Vec::new();
    if matches!(shape, Shape::Gap) {
        success.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Nop,
        ));
    }
    success.push(shift(SemanticOperandV1::Move(fixture::place(3))));
    if matches!(shape, Shape::ExtraShift) {
        success.push(shift(fixture::copy(2)));
    }
    let condition = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![], BOOL).unwrap();
    let mut blocks = vec![
        fixture::block(
            70,
            vec![
                fixture::assign(
                    3,
                    if matches!(shape, Shape::Raw) {
                        SemanticRvalueKindV1::Use(fixture::copy(2))
                    } else {
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::BitAnd,
                            left: fixture::copy(2),
                            right: literal(if matches!(shape, Shape::WrongMask) {
                                63
                            } else {
                                31
                            }),
                        }
                    },
                ),
                SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        condition.clone(),
                        SemanticRvalueV1::new(
                            BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: fixture::copy(3),
                                right: literal(if matches!(shape, Shape::WrongLimit) {
                                    33
                                } else {
                                    32
                                }),
                            },
                        ),
                    )),
                ),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Move(condition),
                expected: !matches!(shape, Shape::FalseExpected),
                message: SemanticAssertMessageV1::Overflow {
                    operation: if matches!(shape, Shape::WrongMessage) {
                        match direction {
                            SemanticBinaryOpV1::ShiftLeft => SemanticBinaryOpV1::ShiftRight,
                            SemanticBinaryOpV1::ShiftRight => SemanticBinaryOpV1::ShiftLeft,
                            _ => unreachable!(),
                        }
                    } else {
                        direction
                    },
                    left: fixture::copy(1),
                    right: fixture::copy(3),
                },
                target: SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::AssertSuccess,
                    SemanticBlockIdV1::from_index(1),
                ),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        fixture::block(71, success, SemanticTerminatorKindV1::Return),
    ];
    if matches!(shape, Shape::UnreachableShift) {
        blocks.push(fixture::block(
            72,
            vec![shift(fixture::copy(2))],
            SemanticTerminatorKindV1::Return,
        ));
    }
    let mut replacement = SemanticFunctionDeclV1::new(
        source.identity(),
        source.role(),
        source.item_definition_identity(),
        source.monomorphization_identity(),
        source.generic_type_arguments_identity(),
        source.const_generic_arguments_identity(),
        source.source(),
        source.abi().clone(),
        locals,
        source.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = source.kernel_entry() {
        replacement = replacement.with_kernel_entry(entry.clone());
    }
    let functions = if helper {
        vec![original.functions()[0].clone(), replacement]
    } else {
        vec![replacement]
    };
    InertSemanticMirRequestV1::new(
        original.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn materialize(source: AdmittedInertSemanticMirV1) -> ProductionPreRankedKirOwnerV1 {
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        &source,
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "helper_value_source",
            [31; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let semantic = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
        source,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = Work::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}

fn assert_refusal(result: R<()>) {
    assert!(
        matches!(
            result,
            Err(E::Unsupported {
                phase: "source",
                detail: "total scalar/global recipe; no unchecked arithmetic",
            })
        ),
        "{result:?}"
    );
}

#[test]
fn assertion_success_census_uses_actual_materialized_roots_and_retained_helpers() {
    for helper in [false, true] {
        for direction in [
            SemanticBinaryOpV1::ShiftLeft,
            SemanticBinaryOpV1::ShiftRight,
        ] {
            let owner = materialize(input(Shape::Exact, direction, helper));
            let source = owner.semantic_ssa().source_semantic();
            let mut work = Work::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            let retained = owner.unit_local_source_storage_floor_v1().unwrap();
            let floor = FLOOR + retained;
            budget.reserve_storage(floor).unwrap();
            assert!(
                !masked_shifts::source(source, usize::from(helper), 1, 0, &mut budget).unwrap()
            );
            source_parts(source, &owner.correspondence, &mut budget).unwrap();
            // Only the existing source-role census retains scratch on success.
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            drop(owner);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn assertion_success_census_preserves_raw_wrong_pattern_and_extra_occurrence_refusals() {
    for shape in [
        Shape::Raw,
        Shape::WrongMask,
        Shape::WrongMessage,
        Shape::WrongLimit,
        Shape::FalseExpected,
        Shape::Gap,
        Shape::ExtraShift,
        Shape::UnreachableShift,
    ] {
        let owner = materialize(input(shape, SemanticBinaryOpV1::ShiftLeft, false));
        let mut work = Work::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        let retained = owner.unit_local_source_storage_floor_v1().unwrap();
        let floor = FLOOR + retained;
        budget.reserve_storage(floor).unwrap();
        assert_refusal(source_parts(
            owner.semantic_ssa().source_semantic(),
            &owner.correspondence,
            &mut budget,
        ));
        budget.release_storage(budget.storage() - floor).unwrap();
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

fn query_scope<'source, 'work, T>(
    source: &'source AdmittedInertSemanticMirV1,
    budget: &mut AssertOriginBudgetV1<'work>,
    run: impl for<'scope> FnOnce(
        &mut crate::ProductionSemanticMaskedShiftQueryV1<'scope, 'source>,
        &mut AssertOriginBudgetV1<'work>,
    ) -> R<T>,
) -> R<T> {
    crate::with_production_semantic_masked_shift_query_v1(
        source,
        SemanticFunctionIdV1::from_index(0),
        SemanticMaskedShiftLimitsV1::default(),
        budget,
        |query, budget| Ok(run(query, budget)),
    )
    .map_err(masked_assertion_query_error_v1)?
}

#[test]
fn assertion_success_census_requires_exact_source_function_and_statement() {
    let source = input(Shape::Exact, SemanticBinaryOpV1::ShiftRight, false);
    let foreign = input(Shape::Exact, SemanticBinaryOpV1::ShiftRight, false);
    let mut work = Work::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    query_scope(&source, &mut budget, |query, budget| {
        assert!(masked_assertion_success_shift_v1(
            &source,
            0,
            1,
            0,
            Some(query),
            budget
        )?);
        for (other, function) in [(&foreign, 0), (&source, 1)] {
            assert!(matches!(
                masked_assertion_success_shift_v1(other, function, 1, 0, Some(query), budget),
                Err(E::Source(
                    ProductionSemanticKirErrorV1::CorrespondenceMismatch
                ))
            ));
        }
        for (block, statement) in [(0, 0), (1, 1)] {
            assert!(!masked_assertion_success_shift_v1(
                &source,
                0,
                block,
                statement,
                Some(query),
                budget,
            )?);
        }
        assert!(!masked_assertion_success_shift_v1(
            &source, 0, 1, 0, None, budget
        )?);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn assertion_success_function_census_has_exact_work_storage_and_one_short_limits() {
    let source = input(Shape::Exact, SemanticBinaryOpV1::ShiftRight, false);
    let profile = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = query_scope(&source, &mut budget, |query, budget| {
            source_function(&source, 0, false, Some(query), budget)
        });
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.storage(),
        )
    };
    let full = profile(WORK, STORAGE);
    full.0.unwrap();
    assert_eq!(full.3, FLOOR);
    let exact = profile(full.1, full.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (full.1, full.2, FLOOR));
    let short = profile(full.1 - 1, full.2);
    assert!(matches!(
        short.0,
        Err(E::Resource(AssertOriginResourceV1::Work(_)))
    ));
    assert_eq!(short.3, FLOOR);
    let short = profile(full.1, full.2 - 1);
    assert!(matches!(
        short.0,
        Err(E::Resource(AssertOriginResourceV1::Storage(_)))
    ));
    assert_eq!(short.3, FLOOR);
}

#[test]
fn assertion_success_query_cleanup_preserves_semantic_refusal_and_unrelated_floor() {
    let source = input(Shape::WrongMask, SemanticBinaryOpV1::ShiftRight, false);
    let mut work = Work::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    assert_refusal(query_scope(&source, &mut budget, |query, budget| {
        source_function(&source, 0, false, Some(query), budget)
    }));
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work() > 0);
}

#[test]
fn assertion_success_query_checks_foreign_budget_before_binding_charge() {
    let source = input(Shape::Exact, SemanticBinaryOpV1::ShiftLeft, false);
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let mut foreign = AssertOriginBudgetV1::new(&mut foreign_work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    foreign.reserve_storage(FLOOR).unwrap();
    let result = query_scope(&source, &mut budget, |query, _| {
        masked_assertion_success_shift_v1(&source, 0, 1, 0, Some(query), &mut foreign).map(|_| ())
    });
    assert!(matches!(
        result,
        Err(E::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!((foreign.work(), foreign.storage()), (0, FLOOR));
    // A wrong-slot observation disables automatic cleanup even on the original
    // ledger. The hostile caller, not the query, performs explicit recovery.
    assert!(budget.storage() > FLOOR);
    budget.release_storage(budget.storage() - FLOOR).unwrap();
}

#[test]
fn assertion_success_census_error_mapping_keeps_query_and_resource_domains() {
    use crate::ProductionSemanticMaskedShiftQueryErrorV1 as Query;
    assert!(matches!(
        masked_assertion_query_error_v1(Query::Resource(AssertOriginResourceV1::Accounting)),
        E::Resource(AssertOriginResourceV1::Accounting)
    ));
    assert!(matches!(
        masked_assertion_query_error_v1(Query::Panicked),
        E::Source(ProductionSemanticKirErrorV1::MaskedAssertionQuery(
            Query::Panicked
        ))
    ));
    assert!(matches!(
        masked_assertion_query_error_v1(Query::Analysis(
            fe2o3_mir_model::SemanticMaskedShiftErrorV1::InvalidModel("sentinel")
        )),
        E::Source(ProductionSemanticKirErrorV1::MaskedAssertionQuery(
            Query::Analysis(fe2o3_mir_model::SemanticMaskedShiftErrorV1::InvalidModel(
                "sentinel"
            ))
        ))
    ));
}
