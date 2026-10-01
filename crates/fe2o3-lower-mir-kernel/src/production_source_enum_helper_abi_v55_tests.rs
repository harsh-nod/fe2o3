use super::*;

const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const HELPER: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(3);

fn enum_helper_owner_v55(variant: u32, moved: bool, nested: bool) -> ProductionSemanticSsaOwnerV1 {
    enum_helper_owner_with_ownership_v55(
        variant,
        moved,
        nested,
        SemanticSourceArgumentOwnershipV1::ByValue,
    )
}

fn enum_helper_owner_with_ownership_v55(
    variant: u32,
    moved: bool,
    nested: bool,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |types, functions| {
        types.push(enum_declaration());
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([202; 32]),
            SemanticLayoutIdentityV1::from_sha256([202; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(32),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 24], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![ENUM, WORD]).unwrap()),
        ));
        let input = if nested { PAIR } else { ENUM };
        let mut locals = functions[2].locals().to_vec();
        locals.push(local(201, ENUM, SemanticLocalRoleV1::Temporary));
        locals.push(local(202, PAIR, SemanticLocalRoleV1::Temporary));
        let mut initial = functions[2].blocks()[0].statements().to_vec();
        initial.push(construct(variant, 2));
        if nested {
            initial.push(assign(
                place(6, PAIR),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![
                            SemanticOperandV1::Move(place(5, ENUM)),
                            SemanticOperandV1::Copy(place(3, WORD)),
                        ],
                    )
                    .unwrap(),
                ),
            ));
        }
        let argument = place(if nested { 6 } else { 5 }, input);
        functions[2] = function(
            30,
            false,
            CAPTURE,
            locals,
            vec![
                block(
                    200,
                    initial,
                    call(
                        3,
                        if moved {
                            SemanticOperandV1::Move(argument)
                        } else {
                            SemanticOperandV1::Copy(argument)
                        },
                        1,
                    ),
                ),
                block(201, vec![unit()], SemanticTerminatorKindV1::Return),
            ],
        );
        let mode = SemanticAbiPassModeV1::Indirect {
            attributes: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesNone),
                    true,
                    false,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                if nested { 32 } else { 24 },
                Some(8),
            )
            .unwrap(),
            metadata_attributes: None,
            on_stack: false,
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([203; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                input, mode,
            ))],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![ownership])
        .unwrap();
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([203; 32]),
                SemanticFunctionRoleV1::InternalHelper,
                SemanticItemDefinitionIdentityV1::from_sha256([203; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([203; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([203; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([203; 32]),
                source(),
                abi,
                vec![
                    local(203, UNIT, SemanticLocalRoleV1::Return),
                    local(204, input, SemanticLocalRoleV1::Argument(0)),
                ],
                SemanticBlockIdV1::from_index(0),
                vec![block(202, vec![unit()], SemanticTerminatorKindV1::Return)],
            )
            .unwrap(),
        );
    })
}

fn helper_instance_v55(plan: &SourceReferencePlanV29<'_, '_>) -> ProductionCallInstanceIdV1 {
    let index = plan
        .instances
        .instances()
        .iter()
        .position(|row| row.function() == HELPER)
        .unwrap();
    plan.instances.id_at(index).unwrap()
}

#[test]
fn scoped_enum_helper_abi_preserves_variants_moves_nested_carriers_and_kernel_refusal() {
    for variant in 0..3 {
        for moved in [false, true] {
            for nested in [false, true] {
                run_enum_with_original_demands(
                    enum_helper_owner_v55(variant, moved, nested),
                    |plan, budget| {
                        let instance = helper_instance_v55(plan);
                        let function = plan.instances.instance(instance).unwrap().declaration();
                        let types = plan.instances.owner().source_semantic().types();
                        let signature = execution_function_signature_with_references_v29(
                            plan.instances,
                            instance,
                            Some(plan),
                            budget,
                        )?;
                        assert_eq!(
                            signature.parameter_types,
                            vec![Type::Scalar(ScalarType::U64); if nested { 4 } else { 3 }]
                        );
                        assert_eq!(
                            signature.parameter_semantic_types,
                            [if nested { PAIR } else { ENUM }]
                        );
                        assert!(signature.result_types.is_empty());
                        // This representation is still not external kernel packing.
                        assert!(
                            source_arguments_v1::scoped_v18::source_kernel_parameter_components_v18(
                                types, function, 0, if nested { PAIR } else { ENUM },
                            )
                            .is_err()
                        );
                        Ok(())
                    },
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn scoped_enum_helper_abi_rejects_foreign_admitted_owner_and_root_occurrence() {
    for nested in [false, true] {
        let foreign = enum_helper_owner_v55(0, false, nested);
        run_enum_with_original_demands(enum_helper_owner_v55(0, false, nested), |plan, budget| {
            let instance = helper_instance_v55(plan);
            let node = source_reference_entry_node_v29(
                plan,
                instance,
                SemanticLocalIdV1::from_index(1),
                None,
                budget,
            )?
            .unwrap();
            let original = plan
                .instances
                .owner()
                .source_semantic()
                .logical_arguments_v1(HELPER)
                .unwrap();
            let mapped = original.adjusted_arguments().next().unwrap();
            assert!(check_source_enum_helper_parameter_v55(
                plan, instance, mapped, node, budget
            )?);
            let other = foreign
                .source_semantic()
                .logical_arguments_v1(HELPER)
                .unwrap();
            let other = other.adjusted_arguments().next().unwrap();
            assert!(
                check_source_enum_helper_parameter_v55(plan, instance, other, node, budget)
                    .is_err()
            );
            assert!(
                check_source_enum_helper_parameter_v55(
                    plan,
                    plan.instances.root(),
                    mapped,
                    node,
                    budget
                )
                .is_err()
            );
            assert!(
                check_source_enum_helper_parameter_v55(plan, instance, mapped, usize::MAX, budget)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn scoped_enum_helper_abi_cannot_admit_adjustment_pointee_or_ownership_substitutions() {
    let owner = enum_helper_owner_v55(0, false, false);
    let semantic = owner.source_semantic();
    for fault in 0..2 {
        let old = &semantic.functions()[HELPER.index() as usize];
        let original = old.abi().arguments()[0].value();
        let value = match fault {
            0 => SemanticAbiValueV1::new_with_adjusted_type(
                ENUM,
                SemanticAbiAdjustedTypeV1::new(
                    ENUM,
                    semantic.types()[ENUM.index() as usize].layout_identity(),
                    semantic.types()[ENUM.index() as usize].layout().clone(),
                ),
                original.mode().clone(),
            ),
            1 => original.clone().with_pointee_override(
                SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 24, 8).unwrap(),
            ),
            _ => original.clone(),
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            old.abi().identity(),
            old.abi().layout_identity(),
            old.abi().canon_abi(),
            old.abi().extern_abi(),
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(value)],
            old.abi().return_value().clone(),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap();
        let mut functions = semantic.functions().to_vec();
        functions[HELPER.index() as usize] = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            abi,
            old.locals().to_vec(),
            old.entry(),
            old.blocks().to_vec(),
        )
        .unwrap();
        let request = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            semantic.types().to_vec(),
            semantic.allocations().to_vec(),
            semantic.statics().to_vec(),
            semantic.vtables().to_vec(),
            functions,
            semantic.callables().to_vec(),
            semantic.roots().to_vec(),
        )
        .unwrap();
        assert!(
            request
                .admit_current_production(SemanticMirLimitsV1::default())
                .is_err(),
            "foreign ABI fact {fault} must fail before scoped helper transport"
        );
    }
    for nested in [false, true] {
        let owner = enum_helper_owner_with_ownership_v55(
            0,
            false,
            nested,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        );
        assert!(
            run_enum_with_original_demands(owner, |plan, budget| {
                execution_function_signature_with_references_v29(
                    plan.instances,
                    helper_instance_v55(plan),
                    Some(plan),
                    budget,
                )
                .map(drop)
            })
            .is_err()
        );
    }
}

#[test]
fn scoped_enum_helper_abi_refuses_a_foreign_budget_before_any_debit() {
    let reached = std::cell::Cell::new(false);
    let result =
        run_enum_with_original_demands(enum_helper_owner_v55(0, false, true), |plan, budget| {
            let instance = helper_instance_v55(plan);
            let original = plan
                .instances
                .owner()
                .source_semantic()
                .logical_arguments_v1(HELPER)
                .unwrap();
            let mapped = original.adjusted_arguments().next().unwrap();
            let node =
                source_reference_entry_node_v29(plan, instance, mapped.local(), None, budget)?
                    .unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let before = (budget.work(), budget.storage());
            assert!(matches!(
                check_source_enum_helper_parameter_v55(plan, instance, mapped, node, &mut foreign),
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
