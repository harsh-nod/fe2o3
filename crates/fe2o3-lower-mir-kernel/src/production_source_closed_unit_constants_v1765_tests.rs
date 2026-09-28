#[derive(Clone, Copy)]
enum ClosedUnitCaseV1765 {
    Unit,
    ChangedAndUnit,
    ScalarZeroSized,
    AggregateZeroSized,
}

fn closed_unit_request_v1765(case: ClosedUnitCaseV1765) -> InertSemanticMirRequestV1 {
    let original = closed_owner_v1760(ClosedCaseV1760::Noop);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let aggregate = SemanticTypeIdV1::from_index(types.len() as u32);
    if matches!(case, ClosedUnitCaseV1765::AggregateZeroSized) {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([210; 32]),
            SemanticLayoutIdentityV1::from_sha256([210; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![UNIT]).unwrap()),
        ));
    }
    let functions = semantic
        .functions()
        .iter()
        .enumerate()
        .map(|(ordinal, prior)| {
            let tag = if ordinal == 0 { 60 } else { 150 };
            let mut locals = prior.locals().to_vec();
            let mut statements = Vec::new();
            if matches!(case, ClosedUnitCaseV1765::ChangedAndUnit) {
                statements.push(assign(
                    place(1, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            U32,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(7, 4).unwrap(),
                            ),
                        ),
                    )),
                ));
            }
            match case {
                ClosedUnitCaseV1765::ScalarZeroSized => statements.push(assign(
                    place(1, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(U32, SemanticConstantValueV1::ZeroSized),
                    )),
                )),
                ClosedUnitCaseV1765::AggregateZeroSized => {
                    let destination = locals.len() as u32;
                    locals.push(local(tag + 10, aggregate, SemanticLocalRoleV1::Temporary));
                    statements.push(assign(
                        place(destination, aggregate),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(aggregate, SemanticConstantValueV1::ZeroSized),
                        )),
                    ));
                }
                _ => (),
            }
            // Preserve the real MIR return assignment, rather than normalizing it
            // away while assembling the source fixture.
            statements.push(assign(
                place(0, UNIT),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ));
            function(
                tag,
                prior.role(),
                prior.abi().clone(),
                locals,
                vec![block(tag + 4, statements, SemanticTerminatorKindV1::Return)],
            )
            .with_kernel_entry(prior.kernel_entry().unwrap().clone())
        })
        .collect();
    InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
}

fn closed_unit_owner_v1765(case: ClosedUnitCaseV1765) -> ProductionSemanticSsaOwnerV1 {
    let admitted = closed_unit_request_v1765(case)
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn closed_unit_prepared_v1765(
    case: ClosedUnitCaseV1765,
    budget: &mut ArgumentBudgetV1<'_>,
) -> (ProductionPreparedSourceV18, OriginalKernelAbiFixtureV18) {
    with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        budget,
        || closed_unit_owner_v1765(case),
        |owner, launch, input, _, budget| {
            let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
            let roots = fixture.roots();
            let prepared =
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap();
            (prepared, fixture)
        },
    )
}

#[test]
fn closed_scalar_unit_return_assignment_preserves_real_noop_and_changed_outputs() {
    for (case, changed) in [
        (ClosedUnitCaseV1765::Unit, false),
        (ClosedUnitCaseV1765::ChangedAndUnit, true),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = closed_unit_prepared_v1765(case, &mut budget);
        let roots = fixture.roots();
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let semantic = source.source_semantic(budget)?;
                for function in semantic.functions() {
                    let last = function.blocks()[0].statements().last().unwrap();
                    assert!(
                        matches!(last.kind(), SemanticStatementKindV1::Assign(assignment)
                    if assignment.destination().local().index() == 0
                        && assignment.destination().ty() == UNIT
                        && matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(
                            SemanticOperandV1::Constant(value)) if value.ty() == UNIT
                                && matches!(value.value(), SemanticConstantValueV1::ZeroSized)))
                    );
                }
                let original = source.canonical(budget)?;
                let original_ops: usize = original
                    .module()
                    .functions
                    .iter()
                    .map(|function| {
                        function
                            .body
                            .as_ref()
                            .unwrap()
                            .blocks
                            .iter()
                            .map(|block| block.operations.len())
                            .sum::<usize>()
                    })
                    .sum();
                assert_eq!(
                    original_ops,
                    if changed {
                        semantic.functions().len()
                    } else {
                        0
                    }
                );
                let handoff = source.checked_closed_scalar_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                let output = handoff.output(budget)?;
                assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                assert_eq!(
                    output.owner().canonical_bytes() != output.input_audit_bytes(),
                    changed
                );
                assert!(output.owner().module().functions.iter().all(|function| {
                    function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .all(|block| block.operations.is_empty())
                }));
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), floor);
                Ok::<_, ProductionClosedScalarHandoffErrorV18>(())
            })
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn closed_scalar_zero_sized_scalar_is_rejected_by_exact_mir_admission() {
    let error = closed_unit_request_v1765(ClosedUnitCaseV1765::ScalarZeroSized)
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .err()
        .unwrap();
    assert!(
        matches!(
            error,
            SemanticMirErrorV1::InvalidTypeOperation {
                operation: SemanticTypeOperationV1::Constant,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn closed_scalar_admitted_zero_sized_aggregate_is_not_unit_authority() {
    let owner = closed_unit_owner_v1765(ClosedUnitCaseV1765::AggregateZeroSized);
    let aggregate = owner.source_semantic().types().last().unwrap();
    assert_eq!(aggregate.layout().size_bytes(), Some(0));
    assert!(matches!(aggregate.shape(), SemanticTypeShapeV1::Tuple(_)));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        closed_unit_prepared_v1765(ClosedUnitCaseV1765::AggregateZeroSized, &mut budget);
    let roots = fixture.roots();
    prepared
        .with_checked_source_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let error = source
                .checked_closed_scalar_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )
                .err()
                .unwrap();
            assert!(
                matches!(
                    error,
                    ProductionClosedScalarHandoffErrorV18::Check(
                        ProductionClosedScalarCheckErrorV18::Unsupported(
                            "memory or non-scalar type"
                        )
                    )
                ),
                "{error:?}"
            );
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
