use super::*;

mod enum_carriers_v47_tests {
    use super::*;
    include!("production_source_enum_carriers_v47_tests.rs");
    mod spill_origins_v48_tests {
        use super::*;
        include!("production_source_enum_spills_v48_tests.rs");
    }
}

fn nested_owner() -> ProductionSemanticSsaOwnerV1 {
    let previous = typed_root_entry_rhs_owner_v18();
    let semantic = previous.source_semantic();
    let mut types = semantic.types().to_vec();
    let inner = SemanticTypeIdV1::from_index(types.len() as u32);
    let outer = SemanticTypeIdV1::from_index(inner.index() + 1);
    for (identity, size, fields) in [(229, 4, vec![U32, UNIT]), (230, 8, vec![inner, U32])] {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([identity; 32]),
            SemanticLayoutIdentityV1::from_sha256([identity; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                4,
                SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
        ));
    }
    let mut functions = semantic.functions().to_vec();
    let original = &functions[0];
    let mut locals = original.locals().to_vec();
    let first = locals.len() as u32;
    locals.push(local(231, inner, SemanticLocalRoleV1::Temporary));
    locals.push(local(232, outer, SemanticLocalRoleV1::Temporary));
    let mut statements = original.blocks()[0].statements().to_vec();
    statements.push(assign(
        place(first, inner),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![
                    literal(7),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        UNIT,
                        SemanticConstantValueV1::ZeroSized,
                    )),
                ],
            )
            .unwrap(),
        ),
    ));
    statements.push(assign(
        place(first + 1, outer),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![SemanticOperandV1::Copy(place(first, inner)), literal(11)],
            )
            .unwrap(),
        ),
    ));
    functions[0] = function(
        233,
        SemanticFunctionRoleV1::KernelRoot,
        original.abi().clone(),
        locals,
        vec![block(234, statements, SemanticTerminatorKindV1::Return)],
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
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

fn check_nested(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let semantic = relation.source.source_semantic(budget)?;
    let outer = SemanticTypeIdV1::from_index((semantic.types().len() - 1) as u32);
    let rows = &relation
        .source
        .root_row(0)?
        .rvalue_results
        .as_ref()
        .unwrap()
        .values;
    let row = rows
        .iter()
        .find(|row| row.typed.ty == outer)
        .expect("genuine nested original SSA row");
    let endpoint = relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
    assert_eq!(
        endpoint.carrier_shape(budget)?,
        ProductionSourceSsaCarrierShapeV37::Aggregate { components: 2 }
    );
    let inner = endpoint.component(0, budget)?;
    assert_eq!(
        inner.carrier_shape(budget)?,
        ProductionSourceSsaCarrierShapeV37::Aggregate { components: 2 }
    );
    let scalar = inner.component(0, budget)?;
    let unit = inner.component(1, budget)?;
    let last = endpoint.component(1, budget)?;
    assert_eq!(scalar.source_type(budget)?, U32);
    assert_eq!(last.source_type(budget)?, U32);
    assert_eq!(unit.source_type(budget)?, UNIT);
    assert_eq!(
        unit.carrier_shape(budget)?,
        ProductionSourceSsaCarrierShapeV37::Unit
    );
    assert_eq!(unit.original_definition(budget)?, None);
    assert_eq!(
        scalar.physical_type(budget)?,
        Some(&Type::Scalar(ScalarType::U32))
    );
    assert_ne!(
        scalar.original_definition(budget)?,
        last.original_definition(budget)?
    );
    assert_eq!(scalar.source_local(budget)?, endpoint.source_local(budget)?);
    assert_eq!(
        scalar.source_function(budget)?,
        endpoint.source_function(budget)?
    );
    Ok(())
}

#[test]
fn source_typed_carrier_tree_preserves_nested_fields_and_unit_without_flattening() {
    probe(nested_owner, MODULE_LIMIT, MODULE_LIMIT, check_nested)
        .0
        .unwrap();
}

#[test]
fn source_typed_carrier_tree_exact_and_one_short_resources_keep_complete_structure() {
    let (result, work, storage) = probe(nested_owner, MODULE_LIMIT, MODULE_LIMIT, check_nested);
    result.unwrap();
    let exact = probe(nested_owner, work, storage, check_nested);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    for short_work in [false, true] {
        let result = probe(
            nested_owner,
            work - usize::from(short_work),
            storage - usize::from(!short_work),
            check_nested,
        )
        .0;
        let error = entrance_resource(result.expect_err("one-short account must be refused"));
        assert!(
            matches!(error, ArgumentResourceV1::Work(_) if short_work)
                || matches!(error, ArgumentResourceV1::Storage(_) if !short_work),
            "short_work={short_work}, work={work}, storage={storage}, error={error:?}"
        );
    }
}

#[test]
fn source_typed_carrier_tree_replay_rejects_field_order_type_and_child_range_changes() {
    probe(
        nested_owner,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            let original = relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap();
            assert!(original.carriers.len() >= 4);
            for fault in 0..5 {
                let mut changed = OwnedSourceRvaluesV30 {
                    source: original.source,
                    ledger: original.ledger,
                    storage: original.storage,
                    rows: original.rows.clone(),
                    values: original.values.clone(),
                    index_readers: original.index_readers.clone(),
                    enum_spills: original.enum_spills.clone(),
                    carriers: original.carriers.clone(),
                };
                match fault {
                    0 => {}
                    1 => changed.carriers.swap(0, 1),
                    2 => changed.carriers[0].ty = SemanticTypeIdV1::from_index(u32::MAX),
                    3 => {
                        changed.carriers.pop();
                    }
                    4 => {
                        let row = changed
                            .values
                            .iter_mut()
                            .find(|row| {
                                matches!(row.typed.physical, SourceSsaPhysicalV36::Aggregate { .. })
                            })
                            .unwrap();
                        row.typed.physical = SourceSsaPhysicalV36::Aggregate {
                            start: usize::MAX,
                            length: 2,
                        };
                    }
                    _ => unreachable!(),
                }
                assert_eq!(
                    original
                        .matches_replay_v30(&changed, budget)
                        .map_err(source_emission_error_v18)?,
                    fault == 0
                );
            }
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn source_typed_carrier_tree_single_value_and_out_of_range_queries_are_sticky() {
    for fault in 0..2 {
        let result = probe(
            nested_owner,
            MODULE_LIMIT,
            MODULE_LIMIT,
            |relation, budget| {
                let row = relation
                    .source
                    .root_row(0)?
                    .rvalue_results
                    .as_ref()
                    .unwrap()
                    .values
                    .iter()
                    .find(|row| {
                        matches!(row.typed.physical, SourceSsaPhysicalV36::Aggregate { .. })
                    })
                    .unwrap();
                let endpoint =
                    relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
                let failed = if fault == 0 {
                    endpoint.original_definition(budget).is_err()
                } else {
                    endpoint.component(usize::MAX, budget).is_err()
                };
                assert!(failed);
                assert!(endpoint.carrier_shape(budget).is_err());
                Ok(())
            },
        )
        .0;
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
    }
}

#[test]
fn source_typed_carrier_tree_missing_child_refuses_without_hiding_independent_siblings() {
    let reached = std::cell::Cell::new(false);
    let result = probe(
        nested_owner,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            let semantic = relation.source.source_semantic(budget)?;
            let outer = SemanticTypeIdV1::from_index((semantic.types().len() - 1) as u32);
            let archive = relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap();
            let row = archive
                .values
                .iter()
                .find(|row| row.typed.ty == outer)
                .unwrap();
            let endpoint =
                relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
            let SourceSsaPhysicalV36::Aggregate { start, .. } = *endpoint.physical else {
                panic!("genuine outer aggregate");
            };
            let SourceSsaPhysicalV36::Aggregate { start: inner, .. } =
                archive.carriers[start].physical
            else {
                panic!("genuine nested aggregate");
            };
            let mut carriers = archive.carriers.clone();
            // Test-only corruption of one retained child, not a claim that Deinit
            // removes physical carriers or a public endpoint construction path.
            carriers[inner].physical = SourceSsaPhysicalV36::Unmodeled;
            let changed = ProductionSourceSsaEndpointV36 {
                carriers: &carriers,
                ..endpoint
            };
            let sibling = changed.component(1, budget)?;
            assert_eq!(sibling.source_type(budget)?, U32);
            assert_eq!(
                sibling.carrier_shape(budget)?,
                ProductionSourceSsaCarrierShapeV37::Value
            );
            assert!(sibling.original_definition(budget)?.is_some());
            let nested = changed.component(0, budget)?;
            assert_eq!(
                nested.carrier_shape(budget)?,
                ProductionSourceSsaCarrierShapeV37::Aggregate { components: 2 }
            );
            assert_eq!(
                nested.component(1, budget)?.carrier_shape(budget)?,
                ProductionSourceSsaCarrierShapeV37::Unit
            );
            assert!(matches!(
                nested.component(0, budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "original SSA binding has no typed carrier"
                ))
            ));
            assert!(matches!(
                changed.carrier_shape(budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "original SSA binding has no typed carrier"
                ))
            ));
            reached.set(true);
            Ok(())
        },
    )
    .0;
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "original SSA binding has no typed carrier"
        ))
    ));
}

#[test]
fn source_typed_descriptor_wrapper_retains_slice_locator_without_shape_inference() {
    let completed = std::cell::Cell::new(false);
    run_descriptor_roles_v18(
        DescriptorRoleEntranceV18::IssuedDisjointSlice,
        DescriptorRoleSourceV18::Arithmetic,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |relation, _, budget| {
            let semantic = relation.source.source_semantic(budget)?;
            let rows = &relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap()
                .values;
            let mut wrappers = 0;
            for row in rows {
                if !matches!(
                    row.typed.physical,
                    SourceSsaPhysicalV36::Value {
                        ty: SourceSsaCarrierTypeV36::Slice { .. },
                        ..
                    }
                ) {
                    continue;
                }
                let shape = semantic.types()[row.typed.ty.index() as usize].shape();
                if !matches!(shape, SemanticTypeShapeV1::Aggregate(_)) {
                    continue;
                }
                let endpoint =
                    relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
                assert_eq!(
                    endpoint.carrier_shape(budget)?,
                    ProductionSourceSsaCarrierShapeV37::Value
                );
                assert!(matches!(
                    endpoint.physical_type(budget)?,
                    Some(Type::Slice(_))
                ));
                assert_eq!(endpoint.source_type(budget)?, row.typed.ty);
                assert!(endpoint.original_definition(budget)?.is_some());
                wrappers += 1;
            }
            assert!(wrappers > 0, "nominal wrapper must remain a Slice locator");
            completed.set(true);
            Ok(())
        },
    )
    .0
    .unwrap();
    assert!(completed.get(), "typed wrapper consumer must complete");
}

#[test]
fn source_typed_vector_carrier_preserves_element_lanes_layout_and_pointer_target() {
    use fe2o3_kernel_ir::{FixedVectorTypeV12, VectorLayoutV12};
    let vector = FixedVectorTypeV12::new(ScalarType::U32, 4, VectorLayoutV12::Contiguous);
    let expected = SourceSsaCarrierTypeV36::from_type(&Type::vector(vector)).unwrap();
    assert!(expected.matches(&Type::vector(vector)));
    for changed in [
        FixedVectorTypeV12::new(ScalarType::I32, 4, VectorLayoutV12::Contiguous),
        FixedVectorTypeV12::new(ScalarType::U32, 2, VectorLayoutV12::Contiguous),
        vector.with_layout(VectorLayoutV12::Interleaved { factor: 2 }),
    ] {
        assert!(!expected.matches(&Type::vector(changed)));
    }
    assert!(!expected.matches(&Type::Scalar(ScalarType::U32)));
    let pointer = Type::pointer(
        Type::vector(vector),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    assert!(
        SourceSsaCarrierTypeV36::from_type(&pointer)
            .unwrap()
            .matches(&pointer)
    );
}

#[test]
fn source_typed_carrier_field_lookup_has_independent_four_work_and_shape_boundaries() {
    let tuple = SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, UNIT]).unwrap());
    for work in [4, 3] {
        let mut ledger = CanonicalKernelIrWorkBudgetV1::new(work);
        let mut budget = ArgumentBudgetV1::new(&mut ledger, 0);
        let result = source_carrier_field_type_v37(&tuple, 2, 1, &mut budget);
        if work == 4 {
            assert_eq!(result.unwrap(), UNIT);
            assert_eq!(budget.work(), 4);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        }
    }
    let mut ledger = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut ledger, 0);
    assert!(source_carrier_field_type_v37(&tuple, 1, 0, &mut budget).is_err());
    assert!(source_carrier_field_type_v37(&tuple, 2, 2, &mut budget).is_err());
    let array = SemanticTypeShapeV1::Array {
        element: U32,
        length: 2,
    };
    assert_eq!(
        source_carrier_field_type_v37(&array, 2, 1, &mut budget).unwrap(),
        U32
    );
    assert!(source_carrier_field_type_v37(&array, 3, 1, &mut budget).is_err());
}
