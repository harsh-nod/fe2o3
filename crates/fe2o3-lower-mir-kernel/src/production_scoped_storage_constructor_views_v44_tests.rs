thread_local! {
    static STORAGE_CONSTRUCTOR_FAULT_V44: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static STORAGE_CONSTRUCTOR_QUERIES_V44: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

const STORAGE_CONSTRUCTOR_STOP_V44: &str = "exact constructor payload movement query completed";

fn observe_storage_constructor_views_v44(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.unwrap().plan;
    let lowered = emitted
        .iter()
        .flatten()
        .find(|row| {
            row.scoped_memory_anchors.as_ref().is_some_and(|anchors| {
                anchors.objects.iter().any(|row| {
                    matches!(
                        row.operation,
                        ScopedObjectOperationV29::Project {
                            step: ScopedObjectProjectionV29::VariantForWrite { .. },
                            ..
                        }
                    )
                })
            })
        })
        .expect("genuine retained constructor views");
    let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
    let mut views = anchors
        .objects
        .iter()
        .filter_map(|row| match (row.role, row.operation) {
            (
                ScopedObjectRoleV29::Project { source, projected },
                ScopedObjectOperationV29::Project {
                    step: ScopedObjectProjectionV29::VariantForWrite { .. },
                    ..
                },
            ) => Some((source, projected)),
            _ => None,
        });
    let (root, original) = views.next().unwrap();
    let (_, peer) = views.next().unwrap();
    assert!(views.next().is_none());
    assert_eq!(original.root_type, peer.root_type);
    assert_eq!(original.root_schema, peer.root_schema);
    assert_ne!(original.projected_schema, peer.projected_schema);
    let floor = budget.storage();
    let fault = STORAGE_CONSTRUCTOR_FAULT_V44.get();
    with_canonical_call_scratch_v1(budget, |budget| {
        let mut scratch = 0;
        let parts = ScopedDeferredScalarViewV29::for_instance(
            instances,
            anchors.subject.instance,
            lowered,
            budget,
        )?;
        let index = call_splice_index_with_deferred_parts_v29(
            &lowered.function,
            Some(&parts),
            budget,
            &mut scratch,
        )
        .map_err(source_address_call_error_v29)?;
        let mut construction_views = 0;
        let mut field_views = 0;
        for anchor in &anchors.rows {
            let ScopedMemoryAnchorKindV29::Object(_) = anchor.kind else {
                continue;
            };
            let payload = anchors.object_payload(anchor, budget)?;
            let (
                ScopedObjectRoleV29::Project { source, projected },
                ScopedObjectOperationV29::Project { step, .. },
            ) = (payload.role, payload.operation)
            else {
                continue;
            };
            match step {
                ScopedObjectProjectionV29::VariantForWrite { .. } => construction_views += 1,
                ScopedObjectProjectionV29::Field(_) if source.path.count == 1 => field_views += 1,
                _ => continue,
            }
            let operation = &lowered
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == anchor.block)
                .unwrap()
                .operations[anchor.position];
            let (inputs, result) =
                scoped_storage_operand_types_v29(plan, anchors, payload, operation, &index, budget)
                    .map_err(source_address_call_error_v29)?;
            let Some(ScopedStorageTypeV29::Pointer(schema, space, access)) = result else {
                panic!("constructor projection must retain its exact pointer result")
            };
            assert_eq!(schema, projected.projected_schema);
            assert_eq!(access, AccessMode::WriteOnly);
            assert!(matches!(
                inputs[0],
                Some((_, ScopedStorageTypeV29::Pointer(_, _, base_access)))
                    if base_access == if matches!(step, ScopedObjectProjectionV29::VariantForWrite { .. }) {
                        AccessMode::ReadWrite
                    } else {
                        AccessMode::WriteOnly
                    }
            ));
            for changed_access in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
                let mut results = call_splice_vec_v1(1, budget, &mut scratch)
                    .map_err(source_address_call_error_v29)?;
                call_splice_charge_storage_v1(std::mem::size_of::<Type>(), budget, &mut scratch)
                    .map_err(source_address_call_error_v29)?;
                results.push(ValueDef::new(
                    operation.results[0].id,
                    Type::pointer(Type::StorageObject(schema), space, changed_access),
                ));
                let changed = Operation::new(results, OperationKind::Storage(payload.operation));
                assert!(matches!(
                    scoped_storage_operand_types_v29(
                        plan, anchors, payload, &changed, &index, budget
                    ),
                    Err(CallInstanceEmissionErrorV1::StorageTransport)
                ));
            }
        }
        assert_eq!(construction_views, 2);
        assert_eq!(field_views, 6);
        assert!(!scoped_storage_constructor_schema_v44(
            plan, anchors, root, budget
        )?);
        assert!(scoped_storage_constructor_schema_v44(
            plan, anchors, original, budget
        )?);
        assert!(scoped_storage_constructor_schema_v44(
            plan, anchors, peer, budget
        )?);
        if fault != 0 {
            let mut changed = original;
            match fault {
                1 => changed.projected_schema = peer.projected_schema,
                2 => changed.source = peer.source,
                3 => changed.path = peer.path,
                4 => {
                    let ScopedObjectSourceV29::Place {
                        site,
                        local,
                        prefix,
                        ..
                    } = changed.source
                    else {
                        unreachable!()
                    };
                    changed.source = ScopedObjectSourceV29::Place {
                        site,
                        local,
                        prefix,
                        role: ExecutionOperandV29::RvalueOperand(0),
                    };
                }
                5 => {
                    let ScopedObjectIdentityV29::Local {
                        local, generation, ..
                    } = changed.object
                    else {
                        unreachable!()
                    };
                    changed.object = ScopedObjectIdentityV29::Local {
                        instance: instances
                            .id_at(
                                (anchors.subject.instance.index() + 1)
                                    % instances.instances().len(),
                            )
                            .unwrap(),
                        local,
                        generation,
                    };
                }
                6 => changed.root_schema = peer.projected_schema,
                7 => {
                    let layouts = plan
                        .storage_root
                        .as_ref()
                        .unwrap()
                        .source_layouts(instances, budget)?;
                    assert!(
                        matches!(
                            layouts.check_selected_schema(
                                instances.owner(),
                                original.projected_type,
                                original.projected_schema,
                                budget
                            ),
                            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                        ),
                        "a constructor payload must not become an ordinary type schema"
                    );
                }
                _ => unreachable!(),
            }
            if fault != 7 {
                assert!(
                    matches!(
                        scoped_storage_constructor_schema_v44(plan, anchors, changed, budget),
                        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                    ),
                    "fault={fault}"
                );
            }
        }
        Ok(())
    })?;
    assert_eq!(budget.storage(), floor);
    STORAGE_CONSTRUCTOR_QUERIES_V44.set(STORAGE_CONSTRUCTOR_QUERIES_V44.get() + 2);
    if fault == 0 {
        Ok(())
    } else {
        Err(source_reference_error_v29(STORAGE_CONSTRUCTOR_STOP_V44))
    }
}

fn run_storage_constructor_views_v44(fault: u8) -> (SourceOwnedResultV18<()>, bool, usize, usize) {
    struct Restore(Option<ScopedSlotCustodyObserverV29>, u8, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            STORAGE_CONSTRUCTOR_FAULT_V44.set(self.1);
            ENUM_CONSTRUCTION_FIELDS_V43.set(self.2);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe_storage_constructor_views_v44)),
        STORAGE_CONSTRUCTOR_FAULT_V44.replace(fault),
        ENUM_CONSTRUCTION_FIELDS_V43.replace(3),
    );
    STORAGE_CONSTRUCTOR_QUERIES_V44.set(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(COMPLETE_OBJECT_SOURCE_WORK_LIMIT_V43);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        scalar_payload_prepared_from_v18(original_enum_construction_owner_v43, &mut budget);
    let completed = std::cell::Cell::new(false);
    let result =
        prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            completed.set(true);
            Ok(())
        });
    (
        result,
        completed.get(),
        STORAGE_CONSTRUCTOR_QUERIES_V44.get(),
        budget.storage(),
    )
}

#[test]
fn scoped_storage_constructor_views_retain_exact_original_variant_rows() {
    let (result, completed, observed, storage) = run_storage_constructor_views_v44(0);
    assert!(result.is_ok(), "{result:?}");
    assert!(completed);
    assert!(observed >= 2);
    assert_eq!(storage, MODULE_FLOOR);
}

#[test]
fn scoped_storage_constructor_views_refuse_wrong_schemas_paths_sites_and_instances() {
    for fault in 1..=7 {
        let (result, completed, observed, storage) = run_storage_constructor_views_v44(fault);
        assert!(result.is_err(), "fault={fault}");
        assert!(!completed);
        assert!(observed >= 2, "fault={fault}: {result:?}");
        assert_eq!(storage, MODULE_FLOOR);
    }
}

#[test]
fn scoped_storage_constructor_view_headers_have_an_independent_fixed_envelope() {
    type Fields = (
        [SemanticProjectionV1; 1],
        source_storage_v29::SourceSelectedProjectionV29,
        bool,
    );
    let expected = std::mem::size_of::<Fields>()
        + 2 * std::mem::size_of::<Result<Fields, ProductionSemanticKirErrorV1>>();
    assert_eq!(scoped_storage_constructor_headers_v44().unwrap(), expected);
}

#[test]
fn scoped_storage_construction_access_is_write_only_without_strengthening_other_views() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    for space in [
        AddressSpace::Private,
        AddressSpace::Global,
        AddressSpace::Constant,
    ] {
        for access in [
            AccessMode::ReadOnly,
            AccessMode::WriteOnly,
            AccessMode::ReadWrite,
        ] {
            let before = budget.work();
            let result = scoped_storage_project_access_v48(
                ScopedObjectProjectionV29::VariantForWrite { index: 1 },
                space,
                access,
                &mut budget,
            );
            let expected = if space == AddressSpace::Constant || access == AccessMode::ReadOnly {
                Err(CallInstanceEmissionErrorV1::StorageTransport)
            } else {
                Ok(AccessMode::WriteOnly)
            };
            assert_eq!(result, expected);
            assert_eq!(budget.work() - before, 2);
            for step in [
                ScopedObjectProjectionV29::Field(0),
                ScopedObjectProjectionV29::ArrayIndex(ValueId(0)),
                ScopedObjectProjectionV29::Variant {
                    index: 1,
                    access: fe2o3_kernel_ir::MemoryAccess::new(space, 1),
                },
            ] {
                let before = budget.work();
                assert_eq!(
                    scoped_storage_project_access_v48(step, space, access, &mut budget),
                    Ok(access)
                );
                assert_eq!(budget.work(), before);
            }
        }
    }
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 17);
}

#[test]
fn scoped_storage_construction_access_has_exact_work_and_no_storage_charge() {
    for limit in [2, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        let result = scoped_storage_project_access_v48(
            ScopedObjectProjectionV29::VariantForWrite { index: 0 },
            AddressSpace::Private,
            AccessMode::ReadWrite,
            &mut budget,
        );
        if limit == 2 {
            assert_eq!(result, Ok(AccessMode::WriteOnly));
            assert_eq!(budget.work(), 2);
        } else {
            let Err(CallInstanceEmissionErrorV1::Resource(ArgumentResourceV1::Work(error))) =
                result
            else {
                panic!("exact work refusal: {result:?}")
            };
            assert_eq!(error.limit(), 1);
            assert_eq!(error.actual(), 2);
        }
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.peak_storage(), 17);
    }
}
