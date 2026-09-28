// Genuine nominal DisjointSlice ABI and original issued-pointer source. The
// unsupported raw UniqueBorrow factory remains a separate boundary negative.
fn issued_descriptor_role_owner_v18(mode: DescriptorRoleSourceV18) -> ProductionSemanticSsaOwnerV1 {
    issued_descriptor_role_owner_with_access_v18(mode, true)
}

fn issued_descriptor_role_owner_with_access_v18(
    mode: DescriptorRoleSourceV18,
    used: bool,
) -> ProductionSemanticSsaOwnerV1 {
    assert!(used || matches!(mode, DescriptorRoleSourceV18::Constant));
    let base = source_issued_pointer_source_tests_v29::owner_with_access(false);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let scalar = SemanticTypeIdV1::from_index(1);
    assert_eq!(original.locals().len(), 8);
    let pointer = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(7),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar).unwrap()],
        scalar,
    )
    .unwrap();
    let temporary =
        |index| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], scalar).unwrap();
    let constant = |bits| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            scalar,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
        ))
    };
    let mut locals = original.locals().to_vec();
    let mut statements = original.blocks()[3].statements().to_vec();
    assert_eq!(
        statements.len(),
        1,
        "original Some payload extraction remains exact"
    );
    if used && !matches!(mode, DescriptorRoleSourceV18::VolatileOnly) {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([80; 32]),
            scalar,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
        statements.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                temporary(8),
                SemanticRvalueV1::new(
                    scalar,
                    SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                        pointer.clone(),
                        if matches!(mode, DescriptorRoleSourceV18::VolatileRead) {
                            SemanticVolatilityV1::Volatile
                        } else {
                            SemanticVolatilityV1::NonVolatile
                        },
                        None,
                    )),
                ),
            )),
        ));
    }
    let operand = match mode {
        DescriptorRoleSourceV18::ReadValue => SemanticOperandV1::Copy(temporary(8)),
        DescriptorRoleSourceV18::Arithmetic => {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([81; 32]),
                scalar,
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ));
            statements.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    temporary(9),
                    SemanticRvalueV1::new(
                        scalar,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::Add,
                            left: SemanticOperandV1::Copy(temporary(8)),
                            right: constant(1),
                        },
                    ),
                )),
            ));
            SemanticOperandV1::Copy(temporary(9))
        }
        _ => constant(17),
    };
    if used {
        statements.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                pointer,
                operand,
                if matches!(
                    mode,
                    DescriptorRoleSourceV18::Volatile | DescriptorRoleSourceV18::VolatileOnly
                ) {
                    SemanticVolatilityV1::Volatile
                } else {
                    SemanticVolatilityV1::NonVolatile
                },
                None,
            )),
        ));
    }
    let mut blocks = original.blocks().to_vec();
    blocks[3] = SemanticBasicBlockV1::new(
        blocks[3].identity(),
        blocks[3].source(),
        statements,
        blocks[3].terminator().clone(),
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        vec![root],
        source.callables().to_vec(),
        source.roots().to_vec(),
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

fn issued_descriptor_role_abi_v18(
    owner: &ProductionSemanticSsaOwnerV1,
) -> kernel_argument_abi_v18::tests::FixtureKernelAbiV18 {
    use fe2o3_kernel_descriptor::{
        DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName,
    };
    let mut abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(owner);
    let semantic = owner.source_semantic();
    let arguments = abi.arguments_mut(0);
    assert_eq!(arguments.len(), 2);
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    for (ordinal, argument) in arguments.iter_mut().enumerate() {
        assert_eq!(
            argument.semantic_type_identity,
            semantic.types()[4].identity()
        );
        argument.kind = ProductionKernelArgumentAbiKindV18::Descriptor {
            source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
            argument: LogicalArgumentV1::disjoint_slice(
                ordinal as u16,
                ValidName::new(format!("issued{ordinal}")).unwrap(),
                &source,
                &layout,
                fe2o3_kernel_descriptor::AccessMode::ReadWrite,
                (ordinal * 16) as u32,
            )
            .unwrap(),
        };
    }
    abi
}

#[test]
fn original_issued_descriptor_fixture_correction_records_distinct_source_identities() {
    for mode in [
        DescriptorRoleSourceV18::Constant,
        DescriptorRoleSourceV18::ReadValue,
        DescriptorRoleSourceV18::Arithmetic,
        DescriptorRoleSourceV18::Volatile,
        DescriptorRoleSourceV18::VolatileRead,
        DescriptorRoleSourceV18::VolatileOnly,
    ] {
        let old = descriptor_role_owner_v18(mode);
        let corrected = issued_descriptor_role_owner_v18(mode);
        let old_source = old.source_semantic();
        let corrected_source = corrected.source_semantic();
        let old_argument = old_source.functions()[0].locals()[1].ty();
        let corrected_argument = corrected_source.functions()[0].locals()[1].ty();
        assert!(
            matches!(old_source.types()[old_argument.index() as usize].shape(),
            SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.metadata() == SemanticPointerMetadataV1::SliceLength
                && pointer.mutability() == SemanticMutabilityV1::Mutable)
        );
        assert!(matches!(
            corrected_source.types()[corrected_argument.index() as usize].shape(),
            SemanticTypeShapeV1::Aggregate(_)
        ));
        assert_ne!(
            old.source_semantic_sha256(),
            corrected.source_semantic_sha256()
        );
        eprintln!(
            "descriptor fixture correction {mode:?}: original UniqueBorrow source {:02x?}; admitted nominal DisjointSlice source {:02x?}",
            old.source_semantic_sha256(),
            corrected.source_semantic_sha256()
        );
    }
}

#[test]
fn original_issued_descriptor_roles_check_unused_address_dependencies() {
    let owner =
        issued_descriptor_role_owner_with_access_v18(DescriptorRoleSourceV18::Constant, false);
    let abi = issued_descriptor_role_abi_v18(&owner);
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            let input_root = original.source.root(0, budget)?.1;
            let recipe = scalar_leaf_collision_recipe_v18(
                original.inventory.functions()[input_root].function,
            );
            original.with_descriptor_source_roles_v18(
                optimized,
                0,
                &recipe,
                budget,
                |roles, budget| {
                    let output = optimized.output_inventory(budget)?;
                    let function =
                        optimized_source_root_function_v18(original, optimized, 0, budget)?;
                    let mut counts = [0; 3];
                    for operation in &output.operations()[function.operations.clone()] {
                        let role = roles.role(operation.coordinate, budget)?;
                        match operation.operation.kind {
                            OperationKind::SliceLength { .. } => {
                                assert_eq!(role, Some(DescriptorSourceRoleV18::Length));
                                counts[0] += 1;
                            }
                            OperationKind::SliceData { .. } => {
                                assert_eq!(role, Some(DescriptorSourceRoleV18::Data));
                                counts[1] += 1;
                            }
                            OperationKind::GetElementPointer { .. } => {
                                assert_eq!(role, Some(DescriptorSourceRoleV18::Address));
                                counts[2] += 1;
                            }
                            OperationKind::Load { .. } | OperationKind::Store { .. } => {
                                panic!("unused original issuer has no access")
                            }
                            _ => assert!(role.is_none()),
                        }
                    }
                    assert_eq!(counts, [1, 1, 1]);
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    )
    .0;
    result.unwrap();
    assert!(completed.get());
}

#[test]
fn original_issued_descriptor_store_recipe_checks_both_unchanged_endpoints() {
    for output in [false, true] {
        let expected = if output {
            "actual optimized scalar expression differs from its original source value"
        } else {
            "actual scalar expression differs from its original source value"
        };
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_roles_v18(DescriptorRoleEntranceV18::IssuedDisjointSlice, DescriptorRoleSourceV18::Constant,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT, |original, optimized, budget| {
                check_descriptor_roles_v18(original, optimized, true, budget)?;
                let root = original.source.root(0, budget)?.1;
                let recipe = scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
                original.with_optimized_scalar_leaves_v18(optimized, 0, &recipe, budget, |leaves, budget| {
                    let count = leaves.visit_store_inputs(budget, |disposition, budget| -> SourceOwnedResultV18<()> {
                        let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) = disposition
                            else { panic!("genuine original Store must be retained before recipe mutation"); };
                        let exact = ProductionSemanticExpressionV2::Constant { scalar: request.scalar(budget)?, bits: 17 };
                        request.original.check_expression(&exact, budget)?;
                        request.check_expression(&exact, budget)
                    })?;
                    assert_eq!(count, 1, "complete unchanged source Store census precedes the hostile query");
                    leaves.visit_store_inputs(budget, |disposition, budget| -> SourceOwnedResultV18<()> {
                        let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) = disposition
                            else { return Ok(()); };
                        let scalar = request.scalar(budget)?;
                        let exact = ProductionSemanticExpressionV2::Constant { scalar, bits: 17 };
                        request.original.check_expression(&exact, budget)?;
                        request.check_expression(&exact, budget)?;
                        // This mutates the requested recipe, not the immutable
                        // graph. Actual wrong-producer coverage is separate.
                        let wrong = ProductionSemanticExpressionV2::Constant { scalar, bits: 18 };
                        let query_floor = budget.storage();
                        assert!(query_floor > MODULE_FLOOR);
                        let refused = if output { request.check_expression(&wrong, budget) }
                            else { request.original.check_expression(&wrong, budget) };
                        let error = refused.unwrap_err();
                        assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(detail)
                            if detail == expected), "{error:?}");
                        assert_eq!(budget.storage(), query_floor,
                            "dropped normalization scratch is not an owned refusal");
                        let work = budget.work();
                        for again in [request.original.check_expression(&exact, budget),
                            request.check_expression(&exact, budget)] {
                            assert!(matches!(again, Err(ProductionSourceOwnedViewErrorV18::Binding(detail))
                                if detail == expected), "{again:?}");
                        }
                        assert_eq!((budget.work(), budget.storage()), (work, query_floor),
                            "the exact first query refusal is retained without further debits");
                        completed.set(true);
                        Err(error)
                    }).map(|count| assert_eq!(count, 1))
                })
            }).0;
        assert!(
            completed.get(),
            "the exact same candidate precedes the copied recipe mutation: {result:?}"
        );
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail))
            if detail == expected)
        );
    }
}

#[test]
fn original_issued_descriptor_scalar_scratch_refunds_after_fixed_and_partial_denials() {
    // This unit boundary control is separate from the genuine same-candidate
    // expression mismatch above. It never fabricates a scalar/source proof.
    for fault in 0..5 {
        const WORK: usize = 256;
        const LIMIT: usize = 4096;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let entered = std::cell::Cell::new(false);
        let dropped = std::cell::Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_scoped_source_cleanup_v29(&mut budget, MODULE_FLOOR, |cleanup, budget| {
                let floor = budget.storage();
                let fixed = size_of::<[u64; 7]>();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    source_scalar_normalization_scratch_v18(cleanup, budget, fixed, |budget| {
                        assert!(budget.storage() >= floor + fixed);
                        entered.set(true);
                        if fault == 2 {
                            // The denial occurs after the whole fixed scratch
                            // debit, not at the source query entrance.
                            budget.charge_work(WORK - budget.work())?;
                            budget.charge_work(1)?;
                        }
                        struct Dropped<'a>(&'a std::cell::Cell<bool>, Box<[u8; 32]>);
                        impl Drop for Dropped<'_> {
                            fn drop(&mut self) {
                                self.0.set(true);
                            }
                        }
                        budget.reserve_storage(size_of::<Dropped<'_>>() + size_of::<[u8; 32]>())?;
                        let owned = Dropped(&dropped, Box::new([0; 32]));
                        assert_eq!(owned.1[0], 0);
                        if fault == 3 {
                            // A denied second allocation must also settle the
                            // accepted first allocation after its owner drops.
                            budget.reserve_storage(LIMIT - budget.storage() + 1)?;
                        }
                        if fault == 4 {
                            std::panic::panic_any("scalar scratch sentinel");
                        }
                        if fault == 1 {
                            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "scalar scratch sentinel",
                            ));
                        }
                        Ok(())
                    })
                }));
                assert_eq!(budget.storage(), floor, "fault {fault}");
                assert!(!cleanup.is_denied());
                if fault != 2 {
                    assert!(dropped.get());
                }
                match outcome {
                    Ok(value) => value,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            })
        }));
        assert!(entered.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
        match fault {
            0 => result.unwrap().unwrap(),
            1 => assert!(matches!(
                result.unwrap(),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar scratch sentinel"
                ))
            )),
            2 => assert!(
                matches!(result.unwrap(), Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Work(error))) if error.limit() == WORK && error.actual() == WORK + 1)
            ),
            3 => assert!(
                matches!(result.unwrap(), Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(error))) if error.limit() == LIMIT && error.actual() == LIMIT + 1)
            ),
            4 => assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"scalar scratch sentinel")
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn original_issued_descriptor_scalar_scratch_denies_refund_below_its_live_fixed_floor() {
    for panic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(256);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4096);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let retained = std::cell::Cell::new(None);
        let completed = std::cell::Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_scoped_source_cleanup_v29(&mut budget, MODULE_FLOOR, |cleanup, budget| {
                let outer = budget.storage();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    source_scalar_normalization_scratch_v18(
                        cleanup,
                        budget,
                        size_of::<[u64; 7]>(),
                        |budget| {
                            budget.release_storage(1)?;
                            assert!(budget.storage() > outer);
                            retained.set(Some(budget.storage()));
                            if panic {
                                std::panic::panic_any("scalar scratch floor sentinel");
                            }
                            Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "scalar scratch floor sentinel",
                            ))
                        },
                    )
                }));
                assert!(
                    cleanup.is_denied(),
                    "the higher fixed scratch floor is mandatory on Err and unwind"
                );
                assert_eq!(Some(budget.storage()), retained.get());
                completed.set(true);
                match result {
                    Ok(value) => value,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            })
        }));
        assert!(completed.get());
        assert_eq!(
            Some(budget.storage()),
            retained.get(),
            "no containing scope may refund the lost custody"
        );
        if panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"scalar scratch floor sentinel")
            );
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar scratch floor sentinel"
                ))
            ));
        }
    }
}

#[test]
fn original_issued_descriptor_checked_output_rejects_foreign_result_use_and_same_typed_input() {
    for fault in 0..3 {
        let completed = std::cell::Cell::new(false);
        let expected = if fault == 0 {
            "issued output definition lost its original producer"
        } else {
            "issued output operand changed occurrence or value"
        };
        let result = run_descriptor_roles_v18(
            DescriptorRoleEntranceV18::IssuedDisjointSlice,
            DescriptorRoleSourceV18::Constant,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                check_descriptor_roles_v18(original, optimized, true, budget)?;
                slice_view_v1::test_issued_output_substitution_v18(
                    original, optimized, fault, &completed, budget,
                )
            },
        )
        .0;
        assert!(completed.get(), "fault {fault}: {result:?}");
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail))
            if detail == expected)
        );
    }
}

#[test]
fn original_issued_descriptor_installer_header_cut_precedes_all_role_publication() {
    for cut in 0..3 {
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_roles_v18(
            DescriptorRoleEntranceV18::IssuedDisjointSlice,
            DescriptorRoleSourceV18::Constant,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                check_descriptor_roles_v18(original, optimized, true, budget)?;
                let root = original.source.root(0, budget)?.1;
                let recipe =
                    scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
                original.with_optimized_scalar_leaves_v18(
                    optimized,
                    0,
                    &recipe,
                    budget,
                    |leaves, budget| {
                        slice_view_v1::test_issued_installer_header_cut_v18(
                            original, optimized, leaves, cut, &completed, budget,
                        )
                    },
                )
            },
        )
        .0;
        assert!(completed.get(), "{result:?}");
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(_)
            ))
        ));
    }
}

fn run_ordered_issued_roles_v18(
    mode: DescriptorRoleSourceV18,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    run_descriptor_roles_v18(
        DescriptorRoleEntranceV18::IssuedDisjointSlice,
        mode,
        work,
        storage,
        |original, optimized, budget| {
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            original.with_descriptor_source_roles_v18(
                optimized,
                0,
                &recipe,
                budget,
                |roles, budget| {
                    let output = optimized.output_inventory(budget)?;
                    let function =
                        optimized_source_root_function_v18(original, optimized, 0, budget)?;
                    let (mut ordered, mut ordinary, mut dependencies) = (0, 0, 0);
                    for operation in &output.operations()[function.operations.clone()] {
                        let role = roles.role(operation.coordinate, budget)?;
                        match operation.operation.kind {
                            OperationKind::Store { access, .. } => {
                                assert!(access.volatile);
                                assert_eq!(role, None, "an exact ordered access is still pending");
                                ordered += 1;
                            }
                            OperationKind::Load { access, .. } => {
                                assert!(!access.volatile);
                                assert_eq!(role, Some(DescriptorSourceRoleV18::Read));
                                ordinary += 1;
                            }
                            OperationKind::SliceLength { .. }
                            | OperationKind::SliceData { .. }
                            | OperationKind::GetElementPointer { .. } => {
                                if role.is_some() {
                                    dependencies += 1;
                                }
                            }
                            _ => assert!(role.is_none()),
                        }
                    }
                    assert_eq!(ordered, 1);
                    if matches!(mode, DescriptorRoleSourceV18::VolatileOnly) {
                        assert_eq!((ordinary, dependencies), (0, 0));
                    } else {
                        assert_eq!((ordinary, dependencies), (1, 3));
                    }
                    Ok(())
                },
            )
        },
    )
}

#[test]
fn original_issued_ordered_only_and_mixed_users_have_exact_transaction_boundaries() {
    for mode in [
        DescriptorRoleSourceV18::VolatileOnly,
        DescriptorRoleSourceV18::Volatile,
    ] {
        let (positive, work, storage) =
            run_ordered_issued_roles_v18(mode, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        positive.unwrap();
        let exact = run_ordered_issued_roles_v18(mode, work, storage);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (work, storage));
        let work_error = source_slot_tests::original_repeated_source_resource_v29(
            run_ordered_issued_roles_v18(mode, work - 1, storage)
                .0
                .unwrap_err(),
        );
        assert!(matches!(work_error, ArgumentResourceV1::Work(error)
            if error.limit() == work - 1 && error.actual() > work - 1));
        let storage_error = source_slot_tests::original_repeated_source_resource_v29(
            run_ordered_issued_roles_v18(mode, work, storage - 1)
                .0
                .unwrap_err(),
        );
        assert!(matches!(storage_error, ArgumentResourceV1::Storage(error)
            if error.limit() == storage - 1 && error.actual() > storage - 1));
    }
}
