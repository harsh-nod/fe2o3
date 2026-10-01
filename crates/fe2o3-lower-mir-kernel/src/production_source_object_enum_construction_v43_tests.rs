thread_local! {
    static ENUM_CONSTRUCTION_FIELDS_V43: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ENUM_CONSTRUCTION_OBSERVED_V43: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

include!("production_scoped_storage_constructor_views_v44_tests.rs");
include!("production_source_object_reference_payloads_v44_tests.rs");

#[test]
fn constructor_payload_terminal_requires_exact_enum_type_and_variant() {
    use source_storage_v29::SourceSelectedProjectionV29 as Terminal;
    let ty = SemanticTypeIdV1::from_index(17);
    let other_ty = SemanticTypeIdV1::from_index(18);
    let schema = fe2o3_kernel_ir::StorageLayoutIdV1(31);
    for variant in [0, 2] {
        let payload = Terminal::Payload {
            ty,
            schema,
            variant,
        };
        assert!(source_object_projection_terminal_matches_v46(
            payload,
            ty,
            Some(variant),
        ));
        assert!(!source_object_projection_terminal_matches_v46(
            payload,
            other_ty,
            Some(variant),
        ));
        assert!(!source_object_projection_terminal_matches_v46(
            payload,
            ty,
            Some(variant + 1),
        ));
        assert!(!source_object_projection_terminal_matches_v46(
            payload, ty, None
        ));
    }
}

#[test]
fn constructor_payload_terminal_cannot_substitute_physical_or_omitted_views() {
    use source_storage_v29::SourceSelectedProjectionV29 as Terminal;
    let ty = SemanticTypeIdV1::from_index(17);
    let physical = Terminal::Physical {
        ty,
        schema: fe2o3_kernel_ir::StorageLayoutIdV1(31),
    };
    assert!(source_object_projection_terminal_matches_v46(
        physical, ty, None
    ));
    assert!(!source_object_projection_terminal_matches_v46(
        physical,
        ty,
        Some(0)
    ));
    for expected in [None, Some(0)] {
        assert!(!source_object_projection_terminal_matches_v46(
            Terminal::OmittedNominal { ty, position: 0 },
            ty,
            expected,
        ));
    }
}

fn original_enum_construction_owner_v43() -> ProductionSemanticSsaOwnerV1 {
    let fields = ENUM_CONSTRUCTION_FIELDS_V43.get();
    let base = original_tag_emission_owner();
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let enumeration = helper.locals()[2].ty();
    let (bytes, alignment) = if fields == 0 {
        (1, 1)
    } else {
        (4 + fields as u64 * 4, 4)
    };
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 255),
    );
    let old = &types[enumeration.index() as usize];
    let offsets: Vec<u64> = (0..fields).map(|field| 4 + field as u64 * 4).collect();
    let variants = (0..3)
        .map(|variant| {
            SemanticEnumVariantLayoutV1::from_rustc(
                variant,
                bytes,
                alignment,
                SemanticFieldsShapeV1::arbitrary(offsets.clone(), (0..fields as u32).collect())
                    .unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                alignment,
                720 + u64::from(variant),
                SemanticAggregateLayoutV1::new(
                    offsets.clone(),
                    if fields == 0 {
                        vec![]
                    } else {
                        vec![SemanticPaddingV1::new(1, 3).unwrap()]
                    },
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    types[enumeration.index() as usize] = SemanticTypeDeclV1::new(
        old.identity(),
        old.layout_identity(),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            bytes,
            alignment,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            [3, 17, 250]
                .into_iter()
                .map(|value| {
                    SemanticEnumVariantV1::new(
                        value,
                        SemanticAggregateTypeV1::new(vec![U32; fields]).unwrap(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    );
    let raw = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([173; 32]),
        SemanticLayoutIdentityV1::from_sha256([173; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                enumeration,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = helper.locals().to_vec();
    locals.push(local(219, raw, SemanticLocalRoleV1::Temporary));
    let mut statements = Vec::new();
    for variant in [0, 2] {
        statements.extend([
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
            ),
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(3)),
            ),
            assign(
                place(2, enumeration),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::EnumVariant(variant),
                        (0..fields)
                            .map(|field| {
                                SemanticOperandV1::Constant(SemanticConstantV1::new(
                                    U32,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(101 + field as u128, 4).unwrap(),
                                    ),
                                ))
                            })
                            .collect(),
                    )
                    .unwrap(),
                ),
            ),
            assign(
                place(3, raw),
                SemanticRvalueKindV1::AddressOf {
                    place: place(2, enumeration),
                    mutability: SemanticMutabilityV1::Mutable,
                },
            ),
            assign(
                place(1, U32),
                SemanticRvalueKindV1::Discriminant(place(2, enumeration)),
            ),
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
            ),
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
            ),
        ]);
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

fn observe_original_enum_construction_v43(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let fields = ENUM_CONSTRUCTION_FIELDS_V43.get();
    let mut helpers = std::collections::BTreeSet::new();
    for slot in &slots.slots {
        if let ScopedSlotRepresentationV29::Object {
            bytes, alignment, ..
        } = slot.representation
        {
            assert_eq!(
                (bytes, alignment),
                if fields == 0 {
                    (1, 1)
                } else {
                    (4 + fields as u64 * 4, 4)
                }
            );
            let original = instances.instance(slot.instance).unwrap();
            assert_eq!(original.function().index(), 2);
            let lowered = emitted[slot.instance.index()].as_ref().unwrap();
            let operation = &lowered.function.body.as_ref().unwrap().blocks
                [slot.allocation.block_ordinal]
                .operations[slot.allocation.operation];
            check_scoped_slot_alloca_v29(slot, operation, budget)?;
            helpers.insert(slot.instance.index());
        }
    }
    assert!(!helpers.is_empty());
    for instance in helpers {
        let lowered = emitted[instance].as_ref().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        let mut writes = 0;
        let mut tags = Vec::new();
        let mut reads = 0;
        let mut views = 0;
        for row in &anchors.objects {
            match row.role {
                ScopedObjectRoleV29::Project { projected, .. } => {
                    assert!(matches!(
                        row.operation,
                        ScopedObjectOperationV29::Project {
                            step: ScopedObjectProjectionV29::VariantForWrite { .. }
                                | ScopedObjectProjectionV29::Field(_),
                            ..
                        }
                    ));
                    assert!(matches!(projected.path.count, 1 | 2));
                    views += 1;
                }
                ScopedObjectRoleV29::WriteValue { destination, .. } => {
                    let ScopedObjectSourceV29::AggregateComponent {
                        operand,
                        variant: Some(variant),
                        ..
                    } = destination.source
                    else {
                        panic!("exact original variant field")
                    };
                    assert_eq!(variant, if tags.is_empty() { 0 } else { 2 });
                    assert_eq!(operand as usize, writes);
                    assert_eq!(destination.path.count, 2);
                    writes += 1;
                }
                ScopedObjectRoleV29::SetDiscriminant {
                    variant,
                    origin: ScopedObjectTagOriginV29::Aggregate(_),
                    ..
                } => {
                    assert_eq!(writes, fields, "all payload writes precede the tag commit");
                    assert_eq!(views, if fields == 0 { 0 } else { fields + 1 });
                    writes = 0;
                    views = 0;
                    tags.push(variant);
                }
                ScopedObjectRoleV29::ReadDiscriminant {
                    origin: ScopedObjectTagOriginV29::Statement(_),
                    ..
                } => {
                    assert_eq!(tags.len(), reads + 1);
                    reads += 1;
                }
                _ => panic!("unexpected original constructor effect"),
            }
        }
        assert_eq!(tags, [0, 2]);
        assert_eq!((writes, views, reads), (0, 0, 2));
        ENUM_CONSTRUCTION_OBSERVED_V43.set(ENUM_CONSTRUCTION_OBSERVED_V43.get() + 1);
    }
    Ok(())
}

#[test]
fn original_retained_enum_construction_commits_scalar_payloads_before_the_tag() {
    struct Restore(Option<ScopedSlotObserverV29>, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
            ENUM_CONSTRUCTION_FIELDS_V43.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_original_enum_construction_v43)),
        ENUM_CONSTRUCTION_FIELDS_V43.get(),
    );
    for fields in [0, 1, 3] {
        ENUM_CONSTRUCTION_FIELDS_V43.set(fields);
        ENUM_CONSTRUCTION_OBSERVED_V43.set(0);
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
        assert!(result.is_ok(), "fields={fields}: {result:?}");
        assert!(completed.get());
        assert!(ENUM_CONSTRUCTION_OBSERVED_V43.get() > 0);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
