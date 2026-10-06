use super::*;

fn scalar_helper_owner_v55(
    variant: u32,
    moved: bool,
    nested: bool,
) -> ProductionSemanticSsaOwnerV1 {
    enum_helper_owner_transport_case_v55(
        variant,
        moved,
        nested,
        SemanticSourceArgumentOwnershipV1::ByValue,
        true,
        true,
    )
}

fn scalar_helper_transport_v55(
    variant: u32,
    moved: bool,
    nested: bool,
    fault: u8,
) -> Result<(), ProductionSemanticKirErrorV1> {
    run_enum_with_original_demands(
        scalar_helper_owner_v55(variant, moved, nested),
        |plan, budget| {
            let child = helper_instance_v55(plan);
            let incoming = plan.instances.incoming(child).unwrap();
            let node = source_reference_entry_node_v29(
                plan,
                child,
                SemanticLocalIdV1::from_index(1),
                None,
                budget,
            )?
            .unwrap();
            let returned = plan.returns[child.index()].unwrap();
            assert!(!source_reference_node_has_loan_v29(plan, node, budget)?);
            assert!(!source_reference_node_has_selected_pointer_v29(
                plan, node, budget
            )?);
            assert!(!source_descriptor_node_present_v29(
                plan, node, &mut 0, budget
            )?);
            assert!(source_call_requires_captured_carrier_v55(
                plan, node, budget
            )?);
            assert!(source_call_requires_captured_carrier_v55(
                plan, returned, budget
            )?);
            if nested {
                let SourceReferenceNodeKindV29::Aggregate { first, count: 2 } =
                    plan.nodes[node].kind
                else {
                    panic!("captured enum and ordinary sibling");
                };
                assert!(!source_call_requires_captured_carrier_v55(
                    plan,
                    plan.children[first + 1],
                    budget
                )?);
            }
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let signature = execution_function_signature_with_references_v29(
                plan.instances,
                child,
                Some(plan),
                budget,
            )?;
            let expected = vec![Type::Scalar(ScalarType::U64); if nested { 4 } else { 3 }];
            assert_eq!(signature.parameter_types, expected);
            assert_eq!(signature.result_types, expected);
            let call_plan = execution_instance_plan_with_references_v29(
                plan.instances,
                child,
                FunctionId::new("enum.scalar.helper"),
                SemanticEmissionPlacementV1::default(),
                Some(plan),
                budget,
            )?;
            let values = expected
                .iter()
                .enumerate()
                .map(|(index, ty)| ValueDef::new(ValueId(900 + index as u32), ty.clone()))
                .collect::<Vec<_>>();
            let mut binding = source_reference_rebuild_node_v29(
                &references,
                node,
                true,
                &mut [].iter(),
                &mut values.iter(),
                &mut 0,
                budget,
            )?;
            if fault == 1 {
                let enumeration = match &mut binding {
                    SemanticValueBindingV1::Aggregate(fields) => &mut fields[0],
                    enumeration => enumeration,
                };
                let SemanticValueBindingV1::Enum { payloads, .. } = enumeration else {
                    panic!("complete captured enum carrier");
                };
                payloads.get_mut(&variant).unwrap().pop();
            }
            with_execution_call_scope_v29(budget, |scope, budget| {
                let mut origin = PreparedExecutionCallOriginV29 {
                    scope,
                    ledger: budget.work_ledger_identity_v1(),
                    source: ExecutionCallSourceV29::from_instances(plan.instances, budget)?,
                    function: plan
                        .instances
                        .instance(incoming.occurrence().caller)
                        .unwrap()
                        .function(),
                    occurrence: incoming.occurrence(),
                    callee: HELPER,
                    projections: signature.call_arguments,
                    parameter_types: signature.parameter_types,
                };
                match fault {
                    2 => origin.projections[1].component = Some(99),
                    3 => origin.function = HELPER,
                    4 => origin.callee = ROOT,
                    _ => {}
                }
                assert!(source_reference_call_argument_shape_v29(
                    &references,
                    &origin,
                    0,
                    &binding,
                    budget,
                )?);
                let prepared = PreparedDefinedCallArgumentsV1 {
                    arguments: values.iter().map(|value| value.id).collect(),
                    source_bindings: vec![binding],
                    execution: Some(origin),
                };
                let (arguments, parameters) = prepare_execution_parameters_with_references_v29(
                    plan.instances,
                    child,
                    prepared,
                    &call_plan,
                    Some(&references),
                    budget,
                )?;
                assert_eq!(
                    arguments,
                    values.iter().map(|value| value.id).collect::<Vec<_>>()
                );
                let (_, rebuilt) = parameters
                    .locals
                    .iter()
                    .find(|(local, _)| *local == 1)
                    .unwrap();
                let mut rebuilt_values = Vec::new();
                source_reference_values_v29(
                    &references,
                    rebuilt,
                    &mut rebuilt_values,
                    &mut 0,
                    budget,
                )?;
                assert_eq!(rebuilt_values, parameters.values);
                let (leaves, ids) =
                    source_reference_call_shape_v29(&references, node, rebuilt, budget)?;
                assert!(leaves.is_empty());
                assert_eq!(ids, vec![None; expected.len()]);
                let mut returns = rebuilt_values.iter();
                let returned_binding = source_reference_rebuild_node_v29(
                    &references,
                    returned,
                    true,
                    &mut [].iter(),
                    &mut returns,
                    &mut 0,
                    budget,
                )?;
                assert!(returns.next().is_none());
                source_reference_call_shape_v29(&references, returned, &returned_binding, budget)?;
                Ok(())
            })
        },
    )
}

#[test]
fn scalar_enum_helper_transport_preserves_variants_copy_move_and_nested_payloads() {
    for variant in 0..3 {
        for moved in [false, true] {
            for nested in [false, true] {
                scalar_helper_transport_v55(variant, moved, nested, 0).unwrap();
            }
        }
    }
}

#[test]
fn scalar_enum_helper_transport_rejects_missing_payload_and_substituted_call_rosters() {
    for nested in [false, true] {
        scalar_helper_transport_v55(1, true, nested, 0).unwrap();
        for fault in 1..=4 {
            assert!(
                scalar_helper_transport_v55(1, true, nested, fault).is_err(),
                "fault {fault}"
            );
        }
    }
}

#[test]
fn scalar_enum_helper_selector_has_an_independent_exact_work_boundary() {
    let measured = std::cell::Cell::new(0);
    run_enum_with_original_demands(scalar_helper_owner_v55(2, true, true), |plan, budget| {
        let node = plan.returns[helper_instance_v55(plan).index()].unwrap();
        let before = budget.work();
        assert!(source_call_requires_captured_carrier_v55(
            plan, node, budget
        )?);
        measured.set(budget.work() - before);
        Ok(())
    })
    .unwrap();
    assert!(measured.get() > 0);
    for short in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = run_enum_with_original_demands(
            scalar_helper_owner_v55(2, true, true),
            |plan, budget| {
                let node = plan.returns[helper_instance_v55(plan).index()].unwrap();
                let remaining = measured.get() - usize::from(short);
                budget.charge_work(usize::MAX - budget.work() - remaining)?;
                let result = source_call_requires_captured_carrier_v55(plan, node, budget);
                reached.set(true);
                if short {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                    result.map(drop)
                } else {
                    assert!(result?);
                    assert_eq!(budget.work(), usize::MAX);
                    Ok(())
                }
            },
        );
        assert!(reached.get());
        if short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn scalar_enum_helper_selector_refuses_a_foreign_ledger_before_any_debit() {
    let reached = std::cell::Cell::new(false);
    let result =
        run_enum_with_original_demands(scalar_helper_owner_v55(0, false, false), |plan, budget| {
            let node = plan.returns[helper_instance_v55(plan).index()].unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let before = (budget.work(), budget.storage());
            assert!(matches!(
                source_call_requires_captured_carrier_v55(plan, node, &mut foreign),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!((foreign.work(), foreign.storage()), (0, 0));
            assert_eq!((budget.work(), budget.storage()), before);
            reached.set(true);
            Ok(())
        });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}
