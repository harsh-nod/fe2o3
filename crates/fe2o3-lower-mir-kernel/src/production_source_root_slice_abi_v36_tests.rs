fn run_root_slice_abi_v36(
    exclusive: bool,
    fault: Option<u8>,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, usize) {
    let reached = std::cell::Cell::new(0);
    let consume = |original: &ProductionSourceCorrespondenceV18<'_>,
                   _: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                   budget: &mut ArgumentBudgetV1<'_>| {
        let source = original.source.source_semantic(budget)?;
        let (function, physical) = original.source.root(0, budget)?;
        let declaration = &source.functions()[function.index() as usize];
        let root = if fault == Some(0) { usize::MAX } else { 0 };
        let walks = ROOT_SLICE_ABI_WALKS_V36.get();
        original.with_root_slice_abis_v36(root, budget, |index, budget| {
            assert_eq!(ROOT_SLICE_ABI_WALKS_V36.get(), walks + 1);
            assert_eq!(
                index.argument_count(budget)?,
                declaration.abi().source_input_types().len()
            );
            for (local, row) in declaration.locals().iter().enumerate() {
                let SemanticLocalRoleV1::Argument(argument) = row.role() else {
                    continue;
                };
                let descriptor =
                    original
                        .source
                        .kernel_argument_abi_kind(0, argument as usize, budget)?;
                if !matches!(
                    descriptor,
                    Some(
                        fe2o3_kernel_descriptor::SourceTypeDescriptorV3::SharedSlice(_)
                            | fe2o3_kernel_descriptor::SourceTypeDescriptorV3::DisjointSlice(_)
                    )
                ) {
                    continue;
                }
                let actual_local = SemanticLocalIdV1::from_index(local as u32);
                let baseline = budget.storage();
                reached.set(reached.get() + 1);
                let selected_argument = if fault == Some(1) { u32::MAX } else { argument };
                let selected_local = if fault == Some(2) {
                    SemanticLocalIdV1::from_index(u32::MAX)
                } else {
                    actual_local
                };
                let recipe = index
                    .argument(selected_argument, selected_local, budget)?
                    .expect("genuine captured source slice descriptor");
                assert_eq!(budget.storage(), baseline);
                recipe.check(budget)?;
                assert_eq!(recipe.function(), function);
                assert_eq!(recipe.argument(), argument);
                assert_eq!(recipe.local(), actual_local);
                assert_eq!(recipe.source_type(), row.ty());
                assert_eq!(
                    recipe.source_type_identity(),
                    source.types()[row.ty().index() as usize].identity()
                );
                assert_eq!(recipe.element(), ScalarType::U32);
                assert_eq!(recipe.is_exclusive_contract(), exclusive);
                assert!(recipe.allows_reads());
                assert_eq!(recipe.allows_writes(), exclusive);
                assert_eq!(recipe.metadata_bits(), 64);
                let components = recipe.components();
                assert_eq!((components[0].1, components[0].2), (8, 8));
                assert_eq!((components[1].1, components[1].2), (8, 8));
                assert_eq!(components[0].0 % 8, 0);
                assert_eq!(components[1].0, components[0].0 + 8);
                let RootSliceDefinitionV36::FunctionArgument { function, argument } =
                    recipe.parameter()
                else {
                    panic!("exact original entry parameter");
                };
                assert_eq!(function.0 as usize, physical);
                assert!(std::ptr::eq(
                    recipe.parameter_type(),
                    &original.inventory.functions()[physical]
                        .function
                        .signature
                        .parameters[argument as usize]
                ));
                assert!(!recipe.grants_memory_or_launch_authority());
                if fault == Some(3) {
                    for _ in 0..64 {
                        let repeated = index
                            .argument(recipe.argument(), actual_local, budget)?
                            .unwrap();
                        assert_eq!(repeated.parameter(), recipe.parameter());
                        assert_eq!(ROOT_SLICE_ABI_WALKS_V36.get(), walks + 1);
                    }
                }
            }
            Ok(())
        })
    };
    let (result, used, peak) = if exclusive {
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
        let abi = issued_descriptor_role_abi_v18(&owner);
        run_descriptor_role_owner_with_abi_v18(owner, abi, work, storage, consume)
    } else {
        run_descriptor_role_owner_v18(
            descriptor_source_owner(DescriptorCase::READ),
            work,
            storage,
            consume,
        )
    };
    (result, used, peak, reached.get())
}

#[test]
fn original_root_slice_abi_joins_shared_and_disjoint_descriptors_to_exact_entry_carriers() {
    for exclusive in [false, true] {
        let (result, _, _, reached) = run_root_slice_abi_v36(
            exclusive,
            None,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        result.unwrap();
        assert!(reached > 0);
    }
}

#[test]
fn original_root_slice_abi_refuses_foreign_root_argument_and_local() {
    for exclusive in [false, true] {
        for fault in 0..3 {
            let (result, _, _, reached) = run_root_slice_abi_v36(
                exclusive,
                Some(fault),
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
            );
            assert!(result.is_err(), "exclusive={exclusive}, fault={fault}");
            assert_eq!(reached, usize::from(fault != 0));
        }
    }
}

#[test]
fn original_root_slice_abi_repeated_argument_queries_do_not_repeat_the_source_walk() {
    for exclusive in [false, true] {
        let (result, _, _, reached) = run_root_slice_abi_v36(
            exclusive,
            Some(3),
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        result.unwrap();
        assert!(reached > 0);
    }
}

#[test]
fn original_root_slice_abi_recipe_remembers_transient_callback_undercut() {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(self.0);
        }
    }
    let _restore = Restore(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.replace(None));
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, _, budget| {
            let result = original.with_root_slice_abis_v36(0, budget, |index, budget| {
                let (function, _) = original.source.root(0, budget)?;
                let semantic = original.source.source_semantic(budget)?;
                let declaration = &semantic.functions()[function.index() as usize];
                let (local, declaration) = declaration
                    .locals()
                    .iter()
                    .enumerate()
                    .find(|(_, local)| local.role() == SemanticLocalRoleV1::Argument(0))
                    .unwrap();
                let recipe = index
                    .argument(0, SemanticLocalIdV1::from_index(local as u32), budget)?
                    .expect("actual SharedSlice recipe");
                assert_eq!(recipe.source_type(), declaration.ty());
                let floor = budget.storage();
                let undercut = floor - index.scope.required_storage() + 1;
                budget.release_storage(undercut)?;
                assert!(recipe.check(budget).is_err());
                budget.reserve_storage(undercut)?;
                assert_eq!(budget.storage(), floor);
                assert!(original.source.cleanup.is_denied());
                assert!(recipe.check(budget).is_err());
                reached.set(true);
                Ok(())
            });
            if original.source.cleanup.is_denied() {
                DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(Some(budget.storage()));
            }
            result
        },
    );
    assert!(reached.get(), "{result:?}");
    assert!(matches!(
        source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err()),
        ArgumentResourceV1::Accounting
    ));
}

#[test]
fn original_root_slice_abi_preserves_exact_and_one_short_work_and_storage() {
    for exclusive in [false, true] {
        let (result, used, peak, reached) = run_root_slice_abi_v36(
            exclusive,
            None,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        result.unwrap();
        let (result, exact_used, exact_peak, exact_reached) =
            run_root_slice_abi_v36(exclusive, None, used, peak);
        result.unwrap();
        assert_eq!(
            (exact_used, exact_peak, exact_reached),
            (used, peak, reached)
        );
        let (result, accepted, _, _) = run_root_slice_abi_v36(exclusive, None, used - 1, peak);
        let resource =
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
        assert!(
            matches!(resource, ArgumentResourceV1::Work(error)
            if error.actual() == used && error.limit() == used - 1),
            "{resource:?}"
        );
        assert!(accepted < used);
        let (result, _, accepted, _) = run_root_slice_abi_v36(exclusive, None, used, peak - 1);
        let resource =
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
        assert!(
            matches!(resource, ArgumentResourceV1::Storage(error)
            if error.actual() == peak && error.limit() == peak - 1),
            "{resource:?}"
        );
        assert!(accepted < peak);
    }
}

#[test]
fn original_root_slice_abi_query_headers_have_an_independent_coexisting_frame_oracle() {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    let expected = h::<RootSliceArgumentNodeV36>()
        + h::<Option<RootSliceArgumentNodeV36>>()
        + h::<ProductionSourceRootSliceAbiV36<'_, '_>>()
        + h::<Option<ProductionSourceRootSliceAbiV36<'_, '_>>>()
        + h::<ProductionSourceRootSliceAbisV36<'_, '_>>()
        + h::<Vec<Option<RootSliceArgumentNodeV36>>>()
        + h::<slice_view_v1::DescriptorRoleScopeV18>()
        + h::<&[Option<RootSliceArgumentNodeV36>]>()
        + h::<ArgumentViewDataV18<'_>>()
        + h::<ProductionArgumentNodeV1<'_>>()
        + 2 * h::<ProductionPhysicalArgumentV1<'_>>()
        + h::<Option<ProductionSourceOwnedViewErrorV18>>()
        + h::<&ProductionSourceCorrespondenceV18<'_>>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>()
        + h::<std::iter::Enumerate<std::slice::Iter<'_, Option<RootSliceArgumentNodeV36>>>>()
        + h::<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'_>>()
        + h::<[usize; 6]>()
        + h::<[bool; 4]>()
        + h::<[u32; 2]>()
        + h::<SemanticLocalIdV1>()
        + h::<SemanticTypeIdV1>()
        + h::<RootSliceDefinitionV36>()
        + h::<&Type>()
        + h::<[(u32, u16, u16); 2]>()
        + kernel_argument_abi_v18::slice_entry_abi_headers_v25().unwrap();
    assert_eq!(root_slice_abi_headers_v36().unwrap(), expected);
    for short in [false, true] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 19 + expected - usize::from(short));
        budget.reserve_storage(19).unwrap();
        let result = budget.reserve_storage(root_slice_abi_headers_v36().unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == 19 + expected && error.limit() == 18 + expected));
            assert_eq!(budget.storage(), 19);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), 19 + expected);
            budget.release_storage(expected).unwrap();
            assert_eq!(budget.storage(), 19);
        }
    }
}
