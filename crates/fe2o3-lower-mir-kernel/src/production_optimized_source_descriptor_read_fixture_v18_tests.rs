fn check_shared_descriptor_roles_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let root = original.source.root(0, budget)?.1;
    let recipe = scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
    original.with_descriptor_source_roles_v18(optimized, 0, &recipe, budget, |roles, budget| {
        let output = optimized.output_inventory(budget)?;
        let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
        let (mut reads, mut addresses, mut lengths, mut data) = (0, 0, 0, 0);
        for operation in &output.operations()[function.operations.clone()] {
            let role = roles.role(operation.coordinate, budget)?;
            match operation.operation.kind {
                OperationKind::Load { .. } => {
                    assert_eq!(role, Some(DescriptorSourceRoleV18::Read));
                    reads += 1;
                }
                OperationKind::Store { .. } => panic!("the original shared source has no write"),
                OperationKind::GetElementPointer { .. } => {
                    if role.is_some() {
                        assert_eq!(role, Some(DescriptorSourceRoleV18::Address));
                        addresses += 1;
                    }
                }
                OperationKind::SliceData { .. } => {
                    if role.is_some() {
                        assert_eq!(role, Some(DescriptorSourceRoleV18::Data));
                        data += 1;
                    }
                }
                OperationKind::SliceLength { .. } => {
                    assert_eq!(role, Some(DescriptorSourceRoleV18::Length));
                    lengths += 1;
                }
                _ => assert!(role.is_none(), "unrelated source roles remain pending"),
            }
        }
        assert!(reads > 0 && addresses > 0 && lengths > 0 && data > 0);
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    })
}

fn shared_descriptor_role_boundary_v18(
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let completed = std::cell::Cell::new(false);
    let (result, work, storage) = run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        work_limit,
        storage_limit,
        |original, optimized, budget| {
            check_shared_descriptor_roles_v18(original, optimized, budget)?;
            completed.set(true);
            Ok(())
        },
    );
    (result, work, storage, completed.get())
}

#[test]
fn original_shared_descriptor_roles_use_the_admitted_source_abi_and_complete_read_census() {
    let result = shared_descriptor_role_boundary_v18(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(result.0.is_ok() && result.3, "{:?}", result.0);
}

#[test]
fn original_shared_descriptor_roles_complete_transaction_exact_and_one_short() {
    let (result, work, storage, completed) =
        shared_descriptor_role_boundary_v18(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(completed);
    let exact = shared_descriptor_role_boundary_v18(work, storage);
    exact.0.unwrap();
    assert!(exact.3);
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let short = shared_descriptor_role_boundary_v18(work_limit, storage_limit);
        // Callback visitation does not imply successful final postflight.
        match (
            is_work,
            source_slot_tests::original_repeated_source_resource_v29(short.0.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert!(error.actual() > work_limit);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert!(error.actual() > storage_limit);
            }
            other => panic!("shared descriptor boundary {other:?}, visited={}", short.3),
        }
    }
}

#[test]
fn original_shared_descriptor_roles_require_same_candidate_data_extent_and_guard() {
    for fault in [
        DescriptorFault::Data,
        DescriptorFault::Extent,
        DescriptorFault::Guard,
    ] {
        let positive =
            shared_descriptor_role_boundary_v18(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(
            positive.0.is_ok() && positive.3,
            "{fault:?}: {:?}",
            positive.0
        );
        let _restore = DescriptorObservers::install(fault);
        let visited = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_v18(
            descriptor_source_owner(DescriptorCase::READ),
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                visited.set(true);
                check_shared_descriptor_roles_v18(original, optimized, budget)
            },
        )
        .0;
        assert!(
            !visited.get(),
            "same emitted candidate must refuse before the role view"
        );
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 1);
        // This genuine SharedSlice source rejects the changed guard at its
        // descriptor correspondence check, before the optimized role callback.
        let expected = "source runtime slice descriptor/index/extent correspondence differs";
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == expected),
            "{fault:?}: {result:?}"
        );
    }
}

#[test]
fn original_descriptor_roles_do_not_relabel_unique_borrow_as_a_supported_kernel_abi() {
    let positive =
        shared_descriptor_role_boundary_v18(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    let visited = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        DescriptorRoleEntranceV18::OriginalUniqueBorrow,
        DescriptorRoleSourceV18::Constant,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |_, _, _| {
            visited.set(true);
            Ok(())
        },
    )
    .0;
    assert!(!visited.get());
    assert!(
        matches!(result, Err(ProductionSourceOwnedViewErrorV18::Source(
        ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
            function: 0, block: None, statement: None, detail,
        }))) if detail == "kernel argument ABI profile differs from the complete original descriptor/source contract"),
        "{result:?}"
    );
}
