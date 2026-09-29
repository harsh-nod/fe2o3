fn issued_helper_two_root_owner_v26(nested: bool) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let base = global_expression_helper_owner_v23(nested);
    let source = base.source_semantic();
    let count = source.functions().len() as u32;
    let rebuild = |function: &SemanticFunctionDeclV1, duplicate: bool| {
        let blocks = function
            .blocks()
            .iter()
            .map(|block| {
                let terminator = match block.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) if call.callee().index() >= count => {
                        SemanticTerminatorV1::new(
                            block.terminator().source(),
                            SemanticTerminatorKindV1::Call(
                                SemanticDirectCallV1::new_callable(
                                    SemanticCallableIdV1::from_index(call.callee().index() + 1),
                                    call.arguments().to_vec(),
                                    call.destination().cloned(),
                                    call.unwind(),
                                )
                                .unwrap(),
                            ),
                        )
                    }
                    _ => block.terminator().clone(),
                };
                SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    block.statements().to_vec(),
                    terminator,
                )
                .unwrap()
            })
            .collect();
        let mut result = SemanticFunctionDeclV1::new(
            if duplicate {
                SemanticFunctionIdentityV1::from_sha256([140; 32])
            } else {
                function.identity()
            },
            function.role(),
            if duplicate {
                SemanticItemDefinitionIdentityV1::from_sha256([141; 32])
            } else {
                function.item_definition_identity()
            },
            if duplicate {
                SemanticMonomorphizationIdentityV1::from_sha256([142; 32])
            } else {
                function.monomorphization_identity()
            },
            if duplicate {
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([143; 32])
            } else {
                function.generic_type_arguments_identity()
            },
            if duplicate {
                SemanticConstGenericArgumentsIdentityV1::from_sha256([144; 32])
            } else {
                function.const_generic_arguments_identity()
            },
            function.source(),
            function.abi().clone(),
            function.locals().to_vec(),
            function.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = function.kernel_entry() {
            result = result.with_kernel_entry(if duplicate {
                SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(b"issued_helper_second_root".to_vec()).unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256([145; 32]),
                    entry.source_contract(),
                )
            } else {
                entry.clone()
            });
        }
        result
    };
    let mut functions: Vec<_> = source
        .functions()
        .iter()
        .map(|function| rebuild(function, false))
        .collect();
    functions.push(rebuild(&source.functions()[0], true));
    let mut callables = source.callables().to_vec();
    callables.insert(
        count as usize,
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(count)),
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        callables,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(count),
        ],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn issued_helper_calls_keep_distinct_issuer_and_access_instances_across_nested_roots() {
    for nested in [false, true] {
        for multiple_roots in [false, true] {
            let owner = if multiple_roots {
                issued_helper_two_root_owner_v26(nested)
            } else {
                global_expression_helper_owner_v23(nested)
            };
            let roots = owner.source_semantic().roots().len();
            let abi = issued_descriptor_role_abi_v18(&owner);
            let completed = std::cell::Cell::new(0);
            let result = run_descriptor_role_owner_with_abi_v18(
                owner,
                abi,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                |original, optimized, budget| {
                    for root in 0..roots {
                        let rows = scoped_raw_admission_v29::checked_issued_source_rows_v18(
                            original, root, budget,
                        )?
                        .unwrap();
                        let mut count = 0;
                        for access in rows
                            .accesses
                            .iter()
                            .filter(|row| row.instance != row.issuer_instance)
                        {
                            count += 1;
                            assert!(access.issuer_instance.index() < access.instance.index());
                            assert_eq!(access.access.address_space, AddressSpace::Generic);
                            let issuer = rows
                                .issuers
                                .iter()
                                .find(|issuer| {
                                    issuer.instance == access.issuer_instance
                                        && issuer.definition == access.issuer
                                })
                                .unwrap();
                            assert_ne!(access.pointer, issuer.pointer);
                        }
                        assert!(count > 0, "nested={nested}, root={root}");
                        let ordinal = original.source.root(root, budget)?.1;
                        let recipe = scalar_leaf_collision_recipe_v18(
                            original.inventory.functions()[ordinal].function,
                        );
                        original.with_descriptor_source_roles_v18(
                            optimized,
                            root,
                            &recipe,
                            budget,
                            |roles, budget| {
                                let output = optimized.output_inventory(budget)?;
                                let function = optimized_source_root_function_v18(
                                    original, optimized, root, budget,
                                )?;
                                let mut accesses = 0;
                                for operation in &output.operations()[function.operations.clone()] {
                                    if matches!(
                                        operation.operation.kind,
                                        OperationKind::Load { .. } | OperationKind::Store { .. }
                                    ) {
                                        assert!(
                                            roles.role(operation.coordinate, budget)?.is_some()
                                        );
                                        accesses += 1;
                                    }
                                }
                                assert!(accesses >= 2);
                                Ok(())
                            },
                        )?;
                        completed.set(completed.get() + 1);
                    }
                    Ok(())
                },
            )
            .0;
            assert!(
                result.is_ok(),
                "nested={nested}, multiple_roots={multiple_roots}: {result:?}"
            );
            assert_eq!(completed.get(), roots);
        }
    }
}

#[test]
fn issued_helper_retained_replay_rejects_foreign_issuer_access_instance_and_pointer() {
    for fault in 0..3 {
        let owner = global_expression_helper_owner_v23(true);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let checked = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, _, budget| {
                let rows =
                    scoped_raw_admission_v29::checked_issued_source_rows_v18(original, 0, budget)?
                        .unwrap();
                let floor = budget.storage();
                budget.reserve_storage(
                        std::mem::size_of::<PendingSourceIssuedRolesV29>()
                            + 2 * std::mem::size_of::<
                                SourceOwnedResultV18<PendingSourceIssuedRolesV29>,
                            >(),
                    )?;
                let mut copy = PendingSourceIssuedRolesV29 {
                    sources: emission_vec_v1(rows.sources.len(), budget)
                        .map_err(source_emission_error_v18)?,
                    issuers: emission_vec_v1(rows.issuers.len(), budget)
                        .map_err(source_emission_error_v18)?,
                    accesses: emission_vec_v1(rows.accesses.len(), budget)
                        .map_err(source_emission_error_v18)?,
                };
                budget
                    .charge_work(rows.sources.len() + rows.issuers.len() + rows.accesses.len())?;
                copy.sources.extend_from_slice(&rows.sources);
                copy.issuers.extend_from_slice(&rows.issuers);
                copy.accesses.extend_from_slice(&rows.accesses);
                let access = copy
                    .accesses
                    .iter_mut()
                    .find(|row| row.instance != row.issuer_instance)
                    .unwrap();
                match fault {
                    0 => access.issuer_instance = access.instance,
                    1 => access.instance = access.issuer_instance,
                    2 => access.pointer = rows.issuers[0].length,
                    _ => unreachable!(),
                }
                let error = scoped_raw_admission_v29::test_issued_copied_rows_replay_v26(
                    original, 0, &copy, budget,
                )
                .unwrap_err();
                assert!(
                    matches!(
                        &error,
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "issued immutable receipt differs from its original owner"
                        )
                    ),
                    "{error:?}"
                );
                let owned = budget.storage() - floor;
                drop(copy);
                budget.release_storage(owned)?;
                checked.set(true);
                Err(error)
            },
        )
        .0;
        assert!(result.is_err(), "fault {fault}");
        assert!(checked.get(), "fault {fault}");
    }
}

#[test]
fn issued_global_origin_batches_complete_distinct_actual_roots_with_shared_nested_helpers() {
    for nested in [false, true] {
        let owner = issued_helper_two_root_owner_v26(nested);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                let floor = budget.storage();
                let launches = mixed_native_launches_v26(original, budget)?;
                assert_eq!(launches.len(), 2);
                with_mixed_source_completion_v26(
                    original,
                    optimized,
                    &launches,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    budget,
                    &mut |native, budget| {
                        let premises = native.runtime_premises(budget)?;
                        let occurrences = native.runtime_occurrences(budget)?;
                        for root in 0..2 {
                            assert!(
                                occurrences
                                    .iter()
                                    .any(|row| premises[row.premise_index()].root() == root
                                        && row.memory_access().address_space
                                            == AddressSpace::Generic),
                                "root={root}, nested={nested}"
                            );
                        }
                        assert!(native.source_roles_are_complete());
                        assert!(!native.grants_artifact_or_launch_authority());
                        completed.set(true);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        )
        .0;
        assert!(result.is_ok(), "nested={nested}: {result:?}");
        assert!(completed.get());
    }
}
