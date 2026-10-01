fn source_object_endpoints_run_v39(
    work: usize,
    storage: usize,
    fault: u8,
) -> (SourceOwnedResultV18<()>, usize, usize, usize) {
    let mut ledger = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut ledger, storage);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut visited = 0;
    let mut refused_storage = None;
    let result = (|| {
        let prepared = physical_prepared_result_v29(&mut budget)?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        source_scalar_normalization_scratch_v18(
                            source.cleanup,
                            budget,
                            0,
                            |budget| {
                                let mut classes = [0usize; 2];
                                let mut roles = [0usize; 2];
                                for root in 0..source.root_count(budget)? {
                                    for instance in 0..source.instance_count(root, budget)? {
                                        for ordinal in 0..source.memory_anchor_count(root, instance, budget)? {
                                            let anchors = source.sidecar(root, instance, budget)?
                                                .scoped_memory_anchors.as_ref().unwrap();
                                            let archived = &anchors.rows[ordinal];
                                            let Some(recipe) = relation.memory_object_recipe_v39(
                                                root, instance, ordinal, budget,
                                            )? else {
                                                assert!(!matches!(archived.kind, ScopedMemoryAnchorKindV29::Object(_)));
                                                continue;
                                            };
                                            visited += 1;
                                            if fault != 0 {
                                                let endpoint = recipe.endpoint(0, budget)?;
                                                let before = (budget.work(), budget.storage());
                                                let error = match fault {
                                                    1 => {
                                                        let credit = before.1 - recipe.required + 1;
                                                        budget.release_storage(credit)?;
                                                        let error = endpoint.source_types(budget).unwrap_err();
                                                        budget.reserve_storage(credit)?;
                                                        assert_eq!(budget.storage(), before.1);
                                                        error
                                                    }
                                                    2 => {
                                                        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(work);
                                                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, storage);
                                                        foreign.reserve_storage(before.1)?;
                                                        let error = endpoint.source_types(&mut foreign).unwrap_err();
                                                        assert_eq!((foreign.work(), foreign.storage()), (0, before.1));
                                                        error
                                                    }
                                                    3 => recipe.endpoint(usize::MAX, budget).err().expect("wrong endpoint refuses"),
                                                    _ => unreachable!(),
                                                };
                                                if fault != 3 {
                                                    refused_storage = Some(before.1);
                                                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)));
                                                } else {
                                                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding("object endpoint ordinal")));
                                                }
                                                let stopped = budget.work();
                                                assert!(endpoint.source_types(budget).is_err());
                                                assert!(recipe.original_operation(budget).is_err());
                                                assert_eq!(budget.work(), stopped);
                                                return Err(error);
                                            }
                                            assert!(std::ptr::eq(recipe.object.anchor, archived));
                                            let actual = source_operation_row_v18(inventory, recipe.original_operation(budget)?, budget)?;
                                            assert_eq!(actual.operation.kind, OperationKind::Storage(recipe.physical_operation(budget)?.0));
                                            let source_endpoints: Vec<_> = match &recipe.object.source.role {
                                                ScopedObjectRoleV29::Project { source, projected, .. } => vec![(source, ProductionSourceObjectEndpointRoleV39::ProjectionBase), (projected, ProductionSourceObjectEndpointRoleV39::ProjectionResult)],
                                                ScopedObjectRoleV29::ReadValue { source, .. } => { roles[0] += 1; vec![(source, ProductionSourceObjectEndpointRoleV39::Read)] },
                                                ScopedObjectRoleV29::WriteValue { destination, .. } => { roles[1] += 1; vec![(destination, ProductionSourceObjectEndpointRoleV39::Write)] },
                                                ScopedObjectRoleV29::CopyObject { source, destination, .. } => vec![(source, ProductionSourceObjectEndpointRoleV39::CopySource), (destination, ProductionSourceObjectEndpointRoleV39::CopyDestination)],
                                                ScopedObjectRoleV29::ReadDiscriminant { source, .. } => vec![(source, ProductionSourceObjectEndpointRoleV39::DiscriminantRead)],
                                                ScopedObjectRoleV29::SetDiscriminant { destination, .. } => vec![(destination, ProductionSourceObjectEndpointRoleV39::DiscriminantWrite)],
                                            };
                                            assert_eq!(recipe.endpoint_count(budget)?, source_endpoints.len());
                                            for (index, (archived, role)) in source_endpoints.into_iter().enumerate() {
                                                let endpoint = recipe.endpoint(index, budget)?;
                                                assert!(std::ptr::eq(endpoint.endpoint, archived));
                                                assert_eq!(endpoint.role(budget)?, role);
                                                assert_eq!(endpoint.source_types(budget)?, (archived.root_type, archived.projected_type));
                                                assert_eq!(endpoint.storage_layouts(budget)?, (archived.root_schema, archived.projected_schema));
                                                match archived.object {
                                                    ScopedObjectIdentityV29::Local { instance, local, generation } => {
                                                        classes[0] += 1;
                                                        assert_eq!(endpoint.object_class(budget)?, ProductionSourceObjectClassV39::Local);
                                                        assert_eq!(endpoint.local_identity(budget)?, Some((instance.index(), local, generation)));
                                                    }
                                                    _ => {
                                                        classes[1] += 1;
                                                        assert_ne!(endpoint.object_class(budget)?, ProductionSourceObjectClassV39::Local);
                                                        assert_eq!(endpoint.local_identity(budget)?, None);
                                                    }
                                                }
                                                let expected_place = match archived.source {
                                                    ScopedObjectSourceV29::Place { site, role, local, prefix } => Some((site, role, local, prefix)),
                                                    _ => None,
                                                };
                                                assert_eq!(endpoint.original_place(budget)?, expected_place);
                                                let path = anchors.object_path(archived.source_path, budget)
                                                    .map_err(|error| source_attachment_error_v18(error.into()))?;
                                                assert_eq!(endpoint.original_projection_count(budget)?, path.len());
                                                for (position, component) in path.iter().enumerate() {
                                                    let ScopedObjectComponentV29::Original { projection, selector } = component else {
                                                        panic!("source path must retain original projections");
                                                    };
                                                    let selector = selector.map(|row| match row {
                                                        ScopedMemoryOccurrenceV29::Promoted { event, definition } => (event, Some(definition)),
                                                        ScopedMemoryOccurrenceV29::Retained { event } => (event, None),
                                                    });
                                                    assert_eq!(endpoint.original_projection(position, budget)?, (*projection, selector));
                                                }
                                            }
                                        }
                                    }
                                }
                                assert!(visited > 0);
                                assert!(classes.into_iter().all(|count| count > 0), "local and nonlocal original endpoints");
                                assert!(roles.into_iter().all(|count| count > 0), "actual original reads and writes");
                                Ok(())
                            },
                        )
                    })
                })
            })
        })
    })();
    assert_eq!(
        budget.storage(),
        refused_storage.unwrap_or(MODULE_FLOOR),
        "{result:?}"
    );
    (result, budget.work(), budget.peak_storage(), visited)
}

#[test]
fn source_object_endpoints_preserve_complete_original_types_paths_and_generations() {
    let result = source_object_endpoints_run_v39(MODULE_LIMIT, MODULE_LIMIT, 0);
    result.0.unwrap();
    assert!(result.3 > 0);
}

#[test]
fn source_object_endpoints_reject_restored_undercut_foreign_ledger_and_bad_ordinal() {
    for fault in 1..=3 {
        let result = source_object_endpoints_run_v39(MODULE_LIMIT, MODULE_LIMIT, fault);
        assert_eq!(result.3, 1, "the authentic first object is reached");
        assert!(result.0.is_err());
    }
}

#[test]
fn source_object_endpoints_keep_exact_and_one_short_complete_resources() {
    let run = |work, storage| source_object_endpoints_run_v39(work, storage, 0);
    let (result, work, storage, visited) = run(MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    assert!(visited > 0);
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, storage, visited));
    for (work_limit, storage_limit, work_failure) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _, _) = run(work_limit, storage_limit);
        match (
            work_failure,
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!((error.actual(), error.limit()), (work, work_limit))
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!((error.actual(), error.limit()), (storage, storage_limit))
            }
            other => panic!("exact object endpoint resource refusal: {other:?}"),
        }
    }
}
