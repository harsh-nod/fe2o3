fn promoted_enum_owner_v47() -> ProductionSemanticSsaOwnerV1 {
    let previous = nested_owner();
    let semantic = previous.source_semantic();
    let mut types = semantic.types().to_vec();
    let inner = SemanticTypeIdV1::from_index((types.len() - 2) as u32);
    let enumeration = SemanticTypeIdV1::from_index(types.len() as u32);
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 255),
    );
    let layouts = [vec![], vec![4, 8, 8]]
        .into_iter()
        .enumerate()
        .map(|(variant, offsets)| {
            SemanticEnumVariantLayoutV1::from_rustc(
                variant as u32,
                12,
                4,
                SemanticFieldsShapeV1::arbitrary(
                    offsets.clone(),
                    (0..offsets.len() as u32).collect(),
                )
                .unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                801 + variant as u64,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([239; 32]),
        SemanticLayoutIdentityV1::from_sha256([239; 32]),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            12,
            4,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                layouts,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            vec![
                SemanticEnumVariantV1::new(3, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(
                    17,
                    SemanticAggregateTypeV1::new(vec![U32, UNIT, inner]).unwrap(),
                ),
            ],
        )
        .unwrap(),
    ));
    let mut functions = semantic.functions().to_vec();
    let original = &functions[0];
    let mut locals = original.locals().to_vec();
    let inner_local = (locals.len() - 2) as u32;
    let target = locals.len() as u32;
    locals.push(local(240, enumeration, SemanticLocalRoleV1::Temporary));
    locals.push(local(241, enumeration, SemanticLocalRoleV1::Temporary));
    locals.push(local(242, U32, SemanticLocalRoleV1::Temporary));
    let mut statements = original.blocks()[0].statements().to_vec();
    statements.extend([
        assign(
            place(target, enumeration),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::EnumVariant(0), vec![])
                    .unwrap(),
            ),
        ),
        assign(
            place(target, enumeration),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::EnumVariant(1),
                    vec![
                        literal(11),
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            UNIT,
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        SemanticOperandV1::Copy(place(inner_local, inner)),
                    ],
                )
                .unwrap(),
            ),
        ),
        assign(
            place(target + 1, enumeration),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(target, enumeration))),
        ),
        assign(
            place(target + 2, U32),
            SemanticRvalueKindV1::Discriminant(place(target + 1, enumeration)),
        ),
    ]);
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

fn check_promoted_enum_v47(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let semantic = relation.source.source_semantic(budget)?;
    let enumeration = SemanticTypeIdV1::from_index((semantic.types().len() - 1) as u32);
    let source_local = (semantic.functions()[0].locals().len() - 3) as u32;
    assert!(
        relation.source.owner.inner.source.owner.plans()[0]
            .plan()
            .promoted_variables()
            .iter()
            .any(|local| local.get() == source_local)
    );
    let rows = &relation
        .source
        .root_row(0)?
        .rvalue_results
        .as_ref()
        .unwrap()
        .values;
    let mut seen = [0usize; 2];
    for row in rows.iter().filter(|row| row.typed.ty == enumeration) {
        let endpoint = relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
        let ProductionSourceSsaCarrierShapeV37::Enum {
            payloads,
            known_variant: Some(variant),
        } = endpoint.carrier_shape(budget)?
        else {
            panic!("original enum constructors retain exact known variants");
        };
        assert_eq!(payloads, 1);
        seen[variant as usize] += 1;
        let tag = endpoint.enum_discriminant_v47(budget)?;
        assert_eq!(tag.source_type(budget)?, U32);
        assert_eq!(
            tag.physical_type(budget)?,
            Some(&Type::Scalar(ScalarType::U32))
        );
        assert!(tag.original_definition(budget)?.is_some());
        assert_eq!(tag.source_local(budget)?, endpoint.source_local(budget)?);
        assert_eq!(
            tag.source_function(budget)?,
            endpoint.source_function(budget)?
        );
        assert_eq!(endpoint.enum_variant_fields_v47(1 - variant, budget)?, None);
        if variant == 0 {
            assert_eq!(endpoint.enum_variant_fields_v47(0, budget)?, Some(0));
        } else {
            assert_eq!(endpoint.enum_variant_fields_v47(1, budget)?, Some(3));
            let scalar = endpoint.enum_field_v47(1, 0, budget)?;
            assert_eq!(scalar.source_type(budget)?, U32);
            assert_eq!(
                scalar.physical_type(budget)?,
                Some(&Type::Scalar(ScalarType::U32))
            );
            let unit = endpoint.enum_field_v47(1, 1, budget)?;
            assert_eq!(unit.source_type(budget)?, UNIT);
            assert_eq!(
                unit.carrier_shape(budget)?,
                ProductionSourceSsaCarrierShapeV37::Unit
            );
            assert_eq!(unit.original_definition(budget)?, None);
            let nested = endpoint.enum_field_v47(1, 2, budget)?;
            assert_eq!(
                nested.carrier_shape(budget)?,
                ProductionSourceSsaCarrierShapeV37::Aggregate { components: 2 }
            );
            assert_eq!(nested.component(0, budget)?.source_type(budget)?, U32);
            assert_eq!(nested.component(1, budget)?.source_type(budget)?, UNIT);
        }
    }
    assert!(
        seen[0] >= 1 && seen[1] >= 2,
        "constructors and ordinary enum Copy reached: {seen:?}"
    );
    Ok(())
}

#[test]
fn original_promoted_enum_carriers_preserve_logical_tags_empty_and_nested_payloads() {
    let completed = std::cell::Cell::new(false);
    probe(
        promoted_enum_owner_v47,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            check_promoted_enum_v47(relation, budget)?;
            completed.set(true);
            Ok(())
        },
    )
    .0
    .unwrap();
    assert!(completed.get());
}

#[test]
fn original_enum_optional_field_carriers_preserve_exact_existing_owner_and_leaf_queries() {
    let completed = std::cell::Cell::new(false);
    probe(
        promoted_enum_owner_v47,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            check_promoted_enum_v47(relation, budget)?;
            let semantic = relation.source.source_semantic(budget)?;
            let enumeration = SemanticTypeIdV1::from_index((semantic.types().len() - 1) as u32);
            let rows = &relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap()
                .values;
            let mut checked = 0usize;
            for row in rows.iter().filter(|row| row.typed.ty == enumeration) {
                let endpoint =
                    relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
                if !matches!(
                    endpoint.carrier_shape(budget)?,
                    ProductionSourceSsaCarrierShapeV37::Enum {
                        known_variant: Some(1),
                        ..
                    }
                ) {
                    continue;
                }
                for field in 0..3 {
                    let original = endpoint.enum_field_v47(1, field, budget)?;
                    let optional = endpoint
                        .enum_field_carrier_v49(1, field, budget)?
                        .expect("genuine initialized scalar, Unit and aggregate retain a carrier");
                    assert_eq!(
                        optional.source_function(budget)?,
                        original.source_function(budget)?
                    );
                    assert_eq!(
                        optional.source_local(budget)?,
                        original.source_local(budget)?
                    );
                    assert_eq!(optional.source_type(budget)?, original.source_type(budget)?);
                    assert_eq!(
                        optional.carrier_shape(budget)?,
                        original.carrier_shape(budget)?
                    );
                    if field < 2 {
                        assert_eq!(
                            optional.original_definition(budget)?,
                            original.original_definition(budget)?
                        );
                        assert_eq!(
                            optional.physical_type(budget)?,
                            original.physical_type(budget)?
                        );
                    }
                    checked += 1;
                }
            }
            assert!(
                checked >= 6,
                "both constructor and whole enum Copy are observed"
            );
            completed.set(true);
            Ok(())
        },
    )
    .0
    .unwrap();
    assert!(completed.get());
}

#[test]
fn original_promoted_enum_carriers_have_exact_and_one_short_complete_resources() {
    let (result, work, storage) = probe(
        promoted_enum_owner_v47,
        MODULE_LIMIT,
        MODULE_LIMIT,
        check_promoted_enum_v47,
    );
    result.unwrap();
    let exact = probe(
        promoted_enum_owner_v47,
        work,
        storage,
        check_promoted_enum_v47,
    );
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    for short_work in [false, true] {
        let result = probe(
            promoted_enum_owner_v47,
            work - usize::from(short_work),
            storage - usize::from(!short_work),
            check_promoted_enum_v47,
        )
        .0;
        let error = entrance_resource(result.expect_err("one-short original enum account"));
        assert!(
            matches!(error, ArgumentResourceV1::Work(_) if short_work)
                || matches!(error, ArgumentResourceV1::Storage(_) if !short_work),
            "{error:?}"
        );
    }
}

#[test]
fn original_promoted_enum_queries_retain_exact_nominal_variant_and_ledger_failures() {
    for fault in 0..5 {
        let reached = std::cell::Cell::new(false);
        let attacked = std::cell::Cell::new(false);
        let result = probe(
            promoted_enum_owner_v47,
            MODULE_LIMIT,
            MODULE_LIMIT,
            |relation, budget| {
                check_promoted_enum_v47(relation, budget)?;
                let row = relation
                    .source
                    .root_row(0)?
                    .rvalue_results
                    .as_ref()
                    .unwrap()
                    .values
                    .iter()
                    .find(|row| {
                        matches!(
                            row.typed.physical,
                            SourceSsaPhysicalV36::Enum {
                                known_variant: Some(1),
                                ..
                            }
                        )
                    })
                    .unwrap();
                let endpoint =
                    relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
                reached.set(true);
                let result = match fault {
                    0 => endpoint
                        .enum_variant_fields_v47(u32::MAX, budget)
                        .map(|_| ()),
                    1 => endpoint.enum_field_v47(1, 3, budget).map(|_| ()),
                    2 => endpoint.enum_field_v47(0, 0, budget).map(|_| ()),
                    3 => endpoint.original_definition(budget).map(|_| ()),
                    4 => {
                        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                        foreign.reserve_storage(budget.storage())?;
                        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                        let result = endpoint.enum_discriminant_v47(&mut foreign).map(|_| ());
                        assert_eq!(
                            before,
                            (foreign.work(), foreign.storage(), foreign.peak_storage())
                        );
                        result
                    }
                    _ => unreachable!(),
                };
                let error = result.expect_err("exact negative query must execute");
                if fault == 4 {
                    assert!(matches!(
                        error,
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                    ));
                } else {
                    assert!(matches!(
                        error,
                        ProductionSourceOwnedViewErrorV18::Binding(_)
                    ));
                }
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                assert!(endpoint.enum_discriminant_v47(budget).is_err());
                assert_eq!(
                    before,
                    (budget.work(), budget.storage(), budget.peak_storage())
                );
                attacked.set(true);
                Ok(())
            },
        )
        .0;
        assert!(
            reached.get() && attacked.get(),
            "fault {fault} did not reach the intended query"
        );
        assert!(
            result.is_err(),
            "retained query failure must prevent publication"
        );
    }
}

#[test]
fn original_promoted_enum_replay_rejects_same_type_tag_and_payload_substitutions() {
    let completed = std::cell::Cell::new(false);
    probe(
        promoted_enum_owner_v47,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            check_promoted_enum_v47(relation, budget)?;
            let original = relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap();
            let row_index = original
                .values
                .iter()
                .position(|row| {
                    matches!(
                        row.typed.physical,
                        SourceSsaPhysicalV36::Enum {
                            known_variant: Some(1),
                            ..
                        }
                    )
                })
                .unwrap();
            let SourceSsaPhysicalV36::Enum {
                discriminant,
                start,
                length: 1,
                ..
            } = original.values[row_index].typed.physical
            else {
                panic!("genuine original enum roster");
            };
            let SourceSsaPhysicalV36::EnumVariant {
                start: fields,
                length: 3,
                ..
            } = original.carriers[start].physical
            else {
                panic!("genuine original variant fields");
            };
            assert_eq!(
                original.carriers[discriminant].ty,
                original.carriers[fields].ty
            );
            assert_ne!(
                original.carriers[discriminant].physical,
                original.carriers[fields].physical
            );
            for fault in 0..6 {
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
                    0 => (),
                    1 => changed.carriers.swap(discriminant, fields),
                    2 => {
                        let SourceSsaPhysicalV36::Enum { known_variant, .. } =
                            &mut changed.values[row_index].typed.physical
                        else {
                            unreachable!()
                        };
                        *known_variant = Some(0);
                    }
                    3 => {
                        let SourceSsaPhysicalV36::EnumVariant { variant, .. } =
                            &mut changed.carriers[start].physical
                        else {
                            unreachable!()
                        };
                        *variant = 0;
                    }
                    4 => changed.carriers[start].ty = U32,
                    5 => {
                        changed.carriers.pop();
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
            completed.set(true);
            Ok(())
        },
    )
    .0
    .unwrap();
    assert!(completed.get());
}
