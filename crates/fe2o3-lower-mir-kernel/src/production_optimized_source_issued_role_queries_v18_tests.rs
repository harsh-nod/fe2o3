// White-box queries live beside the private installer. Only these test-only
// commands cross to the fixture module; no private production types escape.
pub(super) fn test_issued_output_substitution_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    fault: usize,
    completed: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let expected = if fault == 0 { "issued output definition lost its original producer" }
        else { "issued output operand changed occurrence or value" };
    let floor = budget.storage();
    let refused: SourceOwnedResultV18<()> = scoped_source_attempt_v29(
        original.source.cleanup, budget, floor, |budget| original.source.retain_construction(|| {
            budget.reserve_storage(issued_output_headers_v18()?)?;
            let rows = scoped_raw_admission_v29::checked_issued_source_rows_v18(original, 0, budget)?.unwrap();
            assert_eq!(rows.issuers.len(), 1);
            let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
            let actual = SourceIssuedActualV29::from_function(function.function, budget)
                .map_err(source_emission_error_v18)?;
            let fact = issued_output_issuer_v18(original, optimized, 0, &rows.issuers[0],
                function, &actual, budget)?.unwrap();
            issued_output_definition_v18(original, optimized,
                SliceDefinition::Result { operation: fact.input[0], result: 0 },
                SliceDefinition::Result { operation: fact.output[0], result: 0 }, budget)?;
            issued_output_operand_v18(original, optimized, fact.input[0], fact.output[0],
                0, fact.receiver, budget)?;
            // Only the proposed query coordinate/value changes; both immutable
            // graphs and their checked relation stay exact.
            let refused = match fault {
                0 => issued_output_definition_v18(original, optimized,
                    SliceDefinition::Result { operation: fact.input[0], result: 0 },
                    SliceDefinition::Result { operation: fact.output[2], result: 0 }, budget),
                1 => issued_output_operand_v18(original, optimized, fact.input[0], fact.output[2],
                    0, fact.receiver, budget),
                2 => {
                    let other = function.function.body.as_ref().unwrap().parameters[1];
                    assert_ne!(other, fact.root_input);
                    assert_eq!(actual.value(other, budget).map_err(source_emission_error_v18)?.ty,
                        actual.value(fact.receiver, budget).map_err(source_emission_error_v18)?.ty);
                    issued_output_operand_v18(original, optimized, fact.input[0], fact.output[0],
                        0, other, budget)
                }
                _ => panic!("unknown issued output test fault"),
            };
            let error = refused.unwrap_err();
            assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(detail)
                if detail == expected), "{error:?}");
            completed.set(true);
            Err(error)
        }));
    assert_eq!(budget.storage(), floor);
    refused
}

pub(super) fn test_issued_installer_header_cut_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    completed: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let refused: SourceOwnedResultV18<()> = scoped_source_attempt_v29(
        original.source.cleanup, budget, floor, |budget| original.source.retain_construction(|| {
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
            let headers = issued_output_headers_v18()?;
            let padding = limit.checked_sub(budget.storage()).unwrap().checked_sub(headers - 1).unwrap();
            budget.reserve_storage(padding)?;
            let padded = budget.storage();
            let error = install_optimized_issued_roles_v18(original, optimized, 0, leaves,
                &mut roles, budget).unwrap_err();
            assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(bound)) if bound.actual() == limit + 1 && bound.limit() == limit),
                "{error:?}");
            assert_eq!(budget.storage(), padded);
            assert!(roles.iter().all(|row| row.role.is_none() && row.input.is_none()
                && row.instance.is_none() && row.site.is_none() && !row.write_recipe_pending));
            budget.release_storage(padding)?;
            let before = (budget.work(), budget.storage());
            assert!(matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(bound))) if bound.actual() == limit + 1 && bound.limit() == limit));
            assert_eq!((budget.work(), budget.storage()), before);
            completed.set(true);
            Err(error)
        }));
    assert_eq!(budget.storage(), floor);
    refused
}
