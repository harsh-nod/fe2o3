// White-box queries live beside the private installer. Only these test-only
// commands cross to the fixture module; no private production types escape.
thread_local! {
    static DESCRIPTOR_ROWS_ATTEMPT_PROBE_V18: std::cell::Cell<Option<(usize, usize, usize, usize)>> = const { std::cell::Cell::new(None) };
}

fn test_descriptor_rows_attempt_header_v18(budget: &mut ArgumentBudgetV1<'_>) {
    DESCRIPTOR_ROWS_ATTEMPT_PROBE_V18.with(|probe| {
        if let Some((remaining, _, _, calls)) = probe.get() {
            assert_eq!(calls, 0, "the probe must reach one descriptor row attempt");
            let floor = budget.storage();
            let padding = budget.storage_limit() - floor - remaining;
            budget.reserve_storage(padding).unwrap();
            probe.set(Some((remaining, padding, floor, calls + 1)));
        }
    });
}

pub(super) fn test_descriptor_rows_attempt_boundary_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    short: bool,
    completed: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let outer_floor = budget.storage();
    type Capture<'a, 's> = (
        &'a ProductionSourceCorrespondenceV18<'s>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'s>,
        &'a usize,
        &'a ProductionOptimizedSourceScalarLeavesV18<'s>,
    );
    let helper = scoped_source_attempt_header_oracle_v29::<
        (Vec<DescriptorSourceRoleRowV18>, usize),
        ProductionSourceOwnedViewErrorV18,
        Capture<'_, '_>,
    >();
    let explicit = descriptor_role_headers_v18::<(), ProductionSourceOwnedViewErrorV18>()?;
    DESCRIPTOR_ROWS_ATTEMPT_PROBE_V18.set(Some((helper - usize::from(short), 0, 0, 0)));
    let first: SourceOwnedResultV18<()> =
        original.with_descriptor_source_roles_v18(optimized, 0, recipe, budget, |_, _| {
            panic!("row construction must refuse before publication")
        });
    let (_, padding, row_floor, calls) = DESCRIPTOR_ROWS_ATTEMPT_PROBE_V18.replace(None).unwrap();
    assert_eq!(
        calls, 1,
        "outer descriptor and both scalar scopes must have succeeded"
    );
    assert!(row_floor > outer_floor);
    let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(bound))) =
        first.as_ref()
    else {
        panic!("the actual descriptor row attempt must refuse: {first:?}");
    };
    let limit = budget.storage_limit();
    assert_eq!(
        (bound.actual(), bound.limit()),
        (limit + if short { 1 } else { explicit }, limit)
    );
    assert_eq!(budget.storage(), outer_floor + padding);
    budget.release_storage(padding)?;
    let before = (budget.work(), budget.storage());
    let replay: SourceOwnedResultV18<()> =
        original.with_descriptor_source_roles_v18(optimized, 0, recipe, budget, |_, _| {
            panic!("the row helper failure forbids retry")
        });
    assert_eq!(format!("{first:?}"), format!("{replay:?}"));
    assert_eq!((budget.work(), budget.storage()), before);
    completed.set(true);
    first
}

pub(super) fn test_issued_output_substitution_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    fault: usize,
    completed: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let expected = if fault == 0 {
        "issued output definition lost its original producer"
    } else {
        "issued output operand changed occurrence or value"
    };
    let floor = budget.storage();
    let refused: SourceOwnedResultV18<()> =
        scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
            original.source.retain_construction(|| {
                budget.reserve_storage(issued_output_headers_v18()?)?;
                let rows =
                    scoped_raw_admission_v29::checked_issued_source_rows_v18(original, 0, budget)?
                        .unwrap();
                assert_eq!(rows.issuers.len(), 1);
                let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
                let actual = SourceIssuedActualV29::from_function(function.function, budget)
                    .map_err(source_emission_error_v18)?;
                let fact = issued_output_issuer_v18(
                    original,
                    optimized,
                    0,
                    &rows.issuers[0],
                    function,
                    &actual,
                    budget,
                )?
                .unwrap();
                issued_output_definition_v18(
                    original,
                    optimized,
                    SliceDefinition::Result {
                        operation: fact.input[0],
                        result: 0,
                    },
                    SliceDefinition::Result {
                        operation: fact.output[0],
                        result: 0,
                    },
                    budget,
                )?;
                issued_output_operand_v18(
                    original,
                    optimized,
                    fact.input[0],
                    fact.output[0],
                    0,
                    fact.receiver,
                    budget,
                )?;
                // Only the proposed query coordinate/value changes; both immutable
                // graphs and their checked relation stay exact.
                let refused = match fault {
                    0 => issued_output_definition_v18(
                        original,
                        optimized,
                        SliceDefinition::Result {
                            operation: fact.input[0],
                            result: 0,
                        },
                        SliceDefinition::Result {
                            operation: fact.output[2],
                            result: 0,
                        },
                        budget,
                    ),
                    1 => issued_output_operand_v18(
                        original,
                        optimized,
                        fact.input[0],
                        fact.output[2],
                        0,
                        fact.receiver,
                        budget,
                    ),
                    2 => {
                        let other = function.function.body.as_ref().unwrap().parameters[1];
                        assert_ne!(other, fact.root_input);
                        assert_eq!(
                            actual
                                .value(other, budget)
                                .map_err(source_emission_error_v18)?
                                .ty,
                            actual
                                .value(fact.receiver, budget)
                                .map_err(source_emission_error_v18)?
                                .ty
                        );
                        issued_output_operand_v18(
                            original,
                            optimized,
                            fact.input[0],
                            fact.output[0],
                            0,
                            other,
                            budget,
                        )
                    }
                    _ => panic!("unknown issued output test fault"),
                };
                let error = refused.unwrap_err();
                assert!(
                    matches!(error, ProductionSourceOwnedViewErrorV18::Binding(detail)
                if detail == expected),
                    "{error:?}"
                );
                completed.set(true);
                Err(error)
            })
        });
    assert_eq!(budget.storage(), floor);
    refused
}

pub(super) fn test_issued_installer_header_cut_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    cut: usize,
    completed: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let refused: SourceOwnedResultV18<()> = scoped_source_attempt_v29(
        original.source.cleanup,
        budget,
        floor,
        |budget| {
            original.source.retain_construction(|| {
            budget.reserve_storage(size_of::<Vec<DescriptorSourceRoleRowV18>>())?;
            let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
            let output = optimized.output_inventory(budget)?;
            let mut roles = emission_vec_v1(function.operations.len(), budget).map_err(source_emission_error_v18)?;
            for operation in &output.operations()[function.operations.clone()] {
                budget.charge_work(1)?;
                roles.push(DescriptorSourceRoleRowV18 { output: operation.coordinate,
                    input: None, instance: None, site: None, role: None, write_recipe_pending: false, global: None });
            }
            let limit = budget.storage_limit();
            // The installer captures three source/leaf references, the root slot,
            // and the borrowed role slice until its explicit debit completes.
            type Attempt<'a, 's> = (
                &'a ProductionSourceCorrespondenceV18<'s>,
                &'a ProductionOptimizedSourceCorrespondenceV18<'s>,
                &'a usize,
                &'a ProductionOptimizedSourceScalarLeavesV18<'s>,
                &'a mut [DescriptorSourceRoleRowV18],
            );
            let explicit = issued_output_headers_v18()?;
            let attempt = scoped_source_attempt_header_oracle_v29::<usize,
                    ProductionSourceOwnedViewErrorV18, Attempt<'_, '_>>();
            let remaining = match cut { 0 => attempt - 1, 1 => attempt, _ => attempt + explicit - 1 };
            let excess = if cut == 1 { explicit } else { 1 };
            let padding = limit.checked_sub(budget.storage()).unwrap().checked_sub(remaining).unwrap();
            budget.reserve_storage(padding)?;
            let padded = budget.storage();
            let error = install_optimized_issued_roles_v18(original, optimized, 0, leaves,
                &mut roles, budget).unwrap_err();
            assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(bound)) if bound.actual() == limit + excess && bound.limit() == limit),
                "{error:?}");
            assert_eq!(budget.storage(), padded);
            assert!(roles.iter().all(|row| row.role.is_none() && row.input.is_none()
                && row.instance.is_none() && row.site.is_none() && !row.write_recipe_pending));
            budget.release_storage(padding)?;
            let before = (budget.work(), budget.storage());
            assert!(matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(bound))) if bound.actual() == limit + excess && bound.limit() == limit));
            assert_eq!((budget.work(), budget.storage()), before);
            let repeated = install_optimized_issued_roles_v18(original, optimized, 0, leaves, &mut roles, budget).unwrap_err();
            assert_eq!(format!("{error:?}"), format!("{repeated:?}"));
            assert_eq!((budget.work(), budget.storage()), before);
            assert!(roles.iter().all(|row| row.role.is_none() && row.input.is_none()
                && row.instance.is_none() && row.site.is_none() && !row.write_recipe_pending));
            completed.set(true);
            Err(error)
        })
        },
    );
    assert_eq!(budget.storage(), floor);
    refused
}
