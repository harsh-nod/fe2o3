thread_local! {
    static REFERENCE_ENUM_MUTABLE_V44: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static REFERENCE_ENUM_QUERIES_V44: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn original_reference_enum_owner_v44() -> ProductionSemanticSsaOwnerV1 {
    ENUM_CONSTRUCTION_FIELDS_V43.set(0);
    let base = original_enum_construction_owner_v43();
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let enumeration = helper.locals()[2].ty();
    let raw_enum = helper.locals()[3].ty();
    let reference = SemanticTypeIdV1::from_index(types.len() as u32);
    let raw_scalar = SemanticTypeIdV1::from_index(types.len() as u32 + 1);
    let mutable = REFERENCE_ENUM_MUTABLE_V44.get();
    let mutability = if mutable {
        SemanticMutabilityV1::Mutable
    } else {
        SemanticMutabilityV1::Immutable
    };
    let primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let nonnull = SemanticScalarValidityRangeV1::new(1, u64::MAX.into());
    let scalar = SemanticBackendScalarV1::initialized(primitive, nonnull);
    let pointer = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([180; 32]),
        SemanticLayoutIdentityV1::from_sha256([180; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                U32,
                SemanticPointerKindV1::Reference,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    if mutable {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    4,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    types.push(pointer);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([181; 32]),
        SemanticLayoutIdentityV1::from_sha256([181; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                U32,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let source_niche = SemanticLayoutNicheV1::new(0, primitive, nonnull).unwrap();
    let tag =
        SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(1, 0));
    let variants = [false, true]
        .into_iter()
        .enumerate()
        .map(|(index, payload)| {
            let offsets = if payload { vec![0] } else { vec![] };
            SemanticEnumVariantLayoutV1::from_rustc(
                index as u32,
                8,
                8,
                SemanticFieldsShapeV1::arbitrary(
                    offsets.clone(),
                    if payload { vec![0] } else { vec![] },
                )
                .unwrap(),
                if payload {
                    SemanticBackendReprV1::scalar(scalar)
                } else {
                    SemanticBackendReprV1::memory(true)
                },
                payload.then_some(source_niche),
                false,
                None,
                8,
                index as u64,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let old = &types[enumeration.index() as usize];
    types[enumeration.index() as usize] = SemanticTypeDeclV1::new(
        old.identity(),
        old.layout_identity(),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            8,
            8,
            SemanticBackendReprV1::scalar(tag),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Niche(
                    SemanticNicheEnumEncodingV1::new(
                        0,
                        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0)
                            .unwrap(),
                        source_niche,
                        tag,
                        1,
                        0,
                        0,
                        0,
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(
                    1,
                    SemanticAggregateTypeV1::new(vec![reference]).unwrap(),
                ),
            ],
        )
        .unwrap(),
    );
    let mut locals = helper.locals().to_vec();
    for (identity, ty) in [
        (220, U32),
        (221, reference),
        (222, raw_scalar),
        (223, U32),
        (224, reference),
        (225, raw_scalar),
    ] {
        locals.push(local(identity, ty, SemanticLocalRoleV1::Temporary));
    }
    let mut statements = (2..10)
        .map(|local| {
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(local)),
            )
        })
        .collect::<Vec<_>>();
    for (referent, loan, raw) in [(4, 5, 6), (7, 8, 9)] {
        statements.extend([
            assign(
                place(referent, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    U32,
                    SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(100 + u128::from(referent), 4).unwrap(),
                    ),
                ))),
            ),
            assign(
                place(raw, raw_scalar),
                SemanticRvalueKindV1::AddressOf {
                    place: place(referent, U32),
                    mutability: SemanticMutabilityV1::Mutable,
                },
            ),
            assign(
                place(loan, reference),
                SemanticRvalueKindV1::Borrow {
                    kind: if mutable {
                        SemanticBorrowKindV1::Mutable
                    } else {
                        SemanticBorrowKindV1::Shared
                    },
                    place: place(referent, U32),
                },
            ),
        ]);
    }
    for loan in [None, Some(5), Some(8), None] {
        statements.push(assign(
            place(2, enumeration),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::EnumVariant(u32::from(loan.is_some())),
                    loan.map(|local| {
                        if mutable {
                            SemanticOperandV1::Move(place(local, reference))
                        } else {
                            SemanticOperandV1::Copy(place(local, reference))
                        }
                    })
                    .into_iter()
                    .collect(),
                )
                .unwrap(),
            ),
        ));
        statements.push(assign(
            place(3, raw_enum),
            SemanticRvalueKindV1::AddressOf {
                place: place(2, enumeration),
                mutability: SemanticMutabilityV1::Mutable,
            },
        ));
        statements.push(assign(
            place(1, U32),
            SemanticRvalueKindV1::Discriminant(place(2, enumeration)),
        ));
    }
    for local in [3, 2, 5, 8, 6, 9, 4, 7] {
        statements.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local)),
        ));
    }
    functions[2] = function(
        210,
        helper.role(),
        helper.abi().clone(),
        locals,
        vec![block(214, statements, SemanticTerminatorKindV1::Return)],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

fn observe_original_reference_enum_v44(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let references = references.unwrap();
    let plan = references.plan;
    let mut samples = [None, None];
    let mut count = 0;
    let mut sample_instance = None;
    for lowered in emitted.iter().flatten() {
        let instance = lowered.source_call_instance.unwrap();
        let original = instances.instance(instance).unwrap().declaration();
        if original.locals().len() != 10 || sample_instance.is_some_and(|prior| prior != instance) {
            continue;
        }
        let archive = lowered.execution_observation.as_ref().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        for row in &anchors.objects {
            let ScopedObjectRoleV29::WriteValue {
                destination,
                value:
                    ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::Operand {
                        site,
                        role,
                        source: ScopedMemoryOperandSourceV29::Place(occurrence),
                        ..
                    }),
            } = row.role
            else {
                continue;
            };
            if !matches!(
                destination.source,
                ScopedObjectSourceV29::AggregateComponent {
                    variant: Some(1),
                    operand: 0,
                    ..
                }
            ) {
                continue;
            }
            let place = scoped_payload_place_v29(original, site, role).unwrap();
            let ScopedObjectOperationV29::WriteValue { value, .. } = row.operation else {
                panic!("actual reference write")
            };
            assert!(count < 2);
            samples[count] = Some((
                instance,
                archive,
                place,
                site,
                role,
                occurrence,
                destination.projected_schema,
                value,
            ));
            sample_instance = Some(instance);
            count += 1;
        }
    }
    assert_eq!(
        count, 2,
        "both original same-type loans must become distinct stored payloads"
    );
    let (instance, archive, place, site, role, occurrence, schema, stored) = samples[0].unwrap();
    let (_, _, peer_place, peer_site, peer_role, peer_occurrence, peer_schema, peer_stored) =
        samples[1].unwrap();
    let before = budget.storage();
    with_canonical_call_scratch_v1(budget, |budget| {
        let first = source_object_archived_reference_v44(
            plan, archive, instance, site, role, place, occurrence, budget,
        )?;
        let peer = source_object_archived_reference_v44(
            plan,
            archive,
            instance,
            peer_site,
            peer_role,
            peer_place,
            peer_occurrence,
            budget,
        )?;
        assert_eq!(first.source_type, peer.source_type);
        assert_ne!(first.origin, peer.origin);
        let first_value = source_object_reference_value_v44(plan, first, schema, budget)?;
        let peer_value = source_object_reference_value_v44(plan, peer, peer_schema, budget)?;
        assert_eq!(first_value.id, stored);
        assert_eq!(peer_value.id, peer_stored);
        assert_ne!(first_value.id, peer_value.id);
        assert!(matches!(&first_value.ty, Type::Pointer(pointer)
            if pointer.address_space == AddressSpace::Private
                && pointer.access == if REFERENCE_ENUM_MUTABLE_V44.get() { AccessMode::ReadWrite } else { AccessMode::ReadOnly }));
        source_object_reference_same_v44(plan, first, first, budget)?;
        assert!(
            source_object_reference_same_v44(plan, first, peer, budget).is_err(),
            "a same-owner same-type different loan is not the original operand"
        );
        let ScopedMemoryOccurrenceV29::Promoted { event, .. } = occurrence else {
            panic!("promoted reference")
        };
        if REFERENCE_ENUM_MUTABLE_V44.get() {
            let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = occurrence else {
                unreachable!()
            };
            let archived = archive.lookup_original_v29(instances, instance, definition, budget)?;
            assert!(
                source_object_projected_reference_v44(
                    plan,
                    archived,
                    place,
                    place.ty(),
                    true,
                    budget,
                )
                .is_err(),
                "even an authenticated mutable loan cannot satisfy Copy semantics"
            );
        }
        let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = peer_occurrence else {
            panic!("promoted peer")
        };
        assert!(
            source_object_archived_reference_v44(
                plan,
                archive,
                instance,
                site,
                role,
                place,
                ScopedMemoryOccurrenceV29::Promoted { event, definition },
                budget
            )
            .is_err(),
            "the exact event must resolve to this definition"
        );
        assert!(
            source_object_archived_reference_v44(
                plan,
                archive,
                instance,
                site,
                ExecutionOperandV29::Destination,
                place,
                occurrence,
                budget
            )
            .is_err(),
            "a non-operand role cannot grant Copy or Move semantics"
        );
        let foreign = emitted
            .iter()
            .flatten()
            .find(|candidate| {
                candidate.source_call_instance != Some(instance)
                    && candidate.execution_observation.is_some()
            })
            .expect("another genuine call archive");
        assert!(
            source_object_archived_reference_v44(
                plan,
                foreign.execution_observation.as_ref().unwrap(),
                instance,
                site,
                role,
                place,
                occurrence,
                budget
            )
            .is_err(),
            "another instance's archive is not interchangeable"
        );
        Ok(())
    })?;
    assert_eq!(budget.storage(), before);
    REFERENCE_ENUM_QUERIES_V44.set(REFERENCE_ENUM_QUERIES_V44.get() + count);
    Ok(())
}

#[test]
fn original_reference_enum_payloads_bind_exact_loans_and_selected_pointer_cells() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>, bool, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            REFERENCE_ENUM_MUTABLE_V44.set(self.1);
            ENUM_CONSTRUCTION_FIELDS_V43.set(self.2);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe_original_reference_enum_v44)),
        REFERENCE_ENUM_MUTABLE_V44.get(),
        ENUM_CONSTRUCTION_FIELDS_V43.get(),
    );
    for mutable in [false, true] {
        REFERENCE_ENUM_MUTABLE_V44.set(mutable);
        REFERENCE_ENUM_QUERIES_V44.set(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(COMPLETE_OBJECT_SOURCE_WORK_LIMIT_V43);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            scalar_payload_prepared_from_v18(original_reference_enum_owner_v44, &mut budget);
        let completed = std::cell::Cell::new(false);
        let result =
            prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
                completed.set(true);
                Ok(())
            });
        assert!(result.is_ok(), "mutable={mutable}: {result:?}");
        assert!(completed.get());
        assert!(REFERENCE_ENUM_QUERIES_V44.get() >= 2);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

fn reference_enum_prepared_result_v44(
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionPreparedSourceV18> {
    let projection = original_reference_enum_owner_v44();
    let owner = original_reference_enum_owner_v44();
    let (_, launch) = with_module_fixture_view(&owner, ModuleFixture::Ordinary, budget, |_, _| ())?;
    with_module_fixture_view(
        &projection,
        ModuleFixture::Ordinary,
        budget,
        |source, budget| {
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner,
                launch,
                source.input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
        },
    )?
    .0
}

fn reference_enum_resources_v44(
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut completed = false;
    let result = reference_enum_prepared_result_v44(&mut budget).and_then(|prepared| {
        prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            completed = true;
            Ok(())
        })
    });
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn original_reference_enum_payloads_have_exact_and_one_short_transaction_resources() {
    struct Restore(bool, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            REFERENCE_ENUM_MUTABLE_V44.set(self.0);
            ENUM_CONSTRUCTION_FIELDS_V43.set(self.1);
        }
    }
    let _restore = Restore(
        REFERENCE_ENUM_MUTABLE_V44.get(),
        ENUM_CONSTRUCTION_FIELDS_V43.get(),
    );
    for mutable in [false, true] {
        REFERENCE_ENUM_MUTABLE_V44.set(mutable);
        let (result, work, storage, completed) =
            reference_enum_resources_v44(COMPLETE_OBJECT_SOURCE_WORK_LIMIT_V43, MODULE_LIMIT);
        result.unwrap();
        assert!(completed);
        let exact = reference_enum_resources_v44(work, storage);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2, exact.3), (work, storage, true));
        for (work_limit, storage_limit, expect_work) in
            [(work - 1, storage, true), (work, storage - 1, false)]
        {
            let (result, used, peak, _) = reference_enum_resources_v44(work_limit, storage_limit);
            let error = match result {
                Err(ProductionSourceOwnedViewErrorV18::Resource(error)) => error,
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                )) => error,
                other => panic!("exact original resource denial required: {other:?}"),
            };
            if expect_work {
                assert!(matches!(error, ArgumentResourceV1::Work(limit)
                    if limit.limit() == work_limit && limit.actual() == work));
                assert!(used <= work_limit);
            } else {
                assert!(matches!(error, ArgumentResourceV1::Storage(limit)
                    if limit.limit() == storage_limit && limit.actual() == storage));
                assert!(peak <= storage_limit);
            }
        }
    }
}

#[test]
fn original_reference_payload_query_headers_have_independent_return_envelopes() {
    use std::mem::size_of;
    type Operand = (SemanticTypeIdV1, bool, SsaValueV1);
    type Projection = (
        Option<u32>,
        SemanticTypeIdV1,
        Option<usize>,
        [&'static (); 8],
        [usize; 8],
    );
    type Value = (
        SourceReferenceBindingOriginV29,
        fe2o3_kernel_ir::StoragePointerV1,
        [&'static (); 8],
        [usize; 8],
    );
    for (actual, independent) in [
        (
            source_reference_emission_headers_v29::<Operand>().unwrap(),
            size_of::<Operand>() + 2 * size_of::<Result<Operand, ProductionSemanticKirErrorV1>>(),
        ),
        (
            source_reference_emission_headers_v29::<Projection>().unwrap(),
            size_of::<Projection>()
                + 2 * size_of::<Result<Projection, ProductionSemanticKirErrorV1>>(),
        ),
        (
            source_reference_emission_headers_v29::<Value>().unwrap(),
            size_of::<Value>() + 2 * size_of::<Result<Value, ProductionSemanticKirErrorV1>>(),
        ),
    ] {
        assert_eq!(actual, independent);
    }
}
