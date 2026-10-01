use super::*;
#[path = "production_source_carrier_tree_v37_tests.rs"]
mod carrier_tree_tests;

fn slice_owner() -> ProductionSemanticSsaOwnerV1 {
    descriptor_source_owner(DescriptorCase::READ)
}

fn probe(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
    consume: impl FnOnce(
        &ProductionSourceCorrespondenceV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = private_memory_prepared_v18(factory, &mut budget).and_then(|prepared| {
        prepared.with_checked_source_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, consume)
                })
            })
        })
    });
    if !matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ) {
        assert_eq!(budget.storage(), 0, "{result:?}");
    }
    (result, budget.work(), budget.peak_storage())
}

fn checked_rows(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<[usize; 4]> {
    let mut counts = [0; 4];
    let source = relation.source.source_ssa(budget)?;
    for root in 0..relation.source.root_count(budget)? {
        let retained = relation
            .source
            .root_row(root)?
            .rvalue_results
            .as_ref()
            .unwrap();
        for row in &retained.values {
            if row.typed.physical == SourceSsaPhysicalV36::Unmodeled {
                continue;
            }
            let endpoint =
                relation.ssa_typed_endpoint_v36(root, row.instance, row.original, budget)?;
            let function = endpoint.source_function(budget)?;
            let local = endpoint.source_local(budget)?;
            let ty = endpoint.source_type(budget)?;
            assert_eq!(
                relation.source.instance(root, row.instance, budget)?.0,
                function
            );
            assert_eq!(
                source.source_semantic().functions()[function.index() as usize].locals()
                    [local.index() as usize]
                    .ty(),
                ty
            );
            let occurrences = source.occurrences_v1().unwrap().function(function).unwrap();
            // An independent test-only scan, not the production dense lookup.
            let expected_local = match row.original {
                SsaValueV1::BlockArgument { variable, .. } => variable.get(),
                SsaValueV1::Definition(_) => {
                    let mut found = Vec::new();
                    for entry in occurrences.entry_definitions() {
                        if entry.value() == Some(row.original) {
                            found.push(entry.variable().get());
                        }
                    }
                    for event in occurrences.events() {
                        if let Some(fe2o3_mir_model::SsaResolvedEventV1::Define {
                            variable,
                            value,
                        }) = event.resolved()
                        {
                            if value == row.original {
                                found.push(variable.get());
                            }
                        }
                    }
                    for edge in occurrences.edge_definitions() {
                        if edge.value() == Some(row.original) {
                            found.push(edge.variable().get());
                        }
                    }
                    assert_eq!(found.len(), 1);
                    found[0]
                }
            };
            assert_eq!(local.index(), expected_local);
            match row.typed.physical {
                SourceSsaPhysicalV36::Unit => {
                    counts[0] += 1;
                    assert_eq!(endpoint.original_definition(budget)?, None);
                    assert_eq!(endpoint.physical_type(budget)?, None);
                    assert!(matches!(
                        source.source_semantic().types()[ty.index() as usize].shape(),
                        SemanticTypeShapeV1::Unit
                    ));
                }
                SourceSsaPhysicalV36::Aggregate { length, .. } => {
                    assert_eq!(
                        endpoint.carrier_shape(budget)?,
                        ProductionSourceSsaCarrierShapeV37::Aggregate { components: length }
                    );
                    for field in 0..length {
                        endpoint.component(field, budget)?;
                    }
                }
                SourceSsaPhysicalV36::Enum {
                    length,
                    known_variant,
                    ..
                } => {
                    assert_eq!(
                        endpoint.carrier_shape(budget)?,
                        ProductionSourceSsaCarrierShapeV37::Enum {
                            payloads: length,
                            known_variant
                        }
                    );
                    endpoint.enum_discriminant_v47(budget)?;
                }
                SourceSsaPhysicalV36::Value {
                    value,
                    ty: carrier,
                    loan,
                } => {
                    let index = endpoint.original_definition(budget)?.unwrap();
                    let actual = &relation.inventory.definitions()[index];
                    assert_eq!(actual.value, Some(value));
                    assert_eq!(endpoint.physical_type(budget)?, Some(actual.ty));
                    assert!(carrier.matches(actual.ty));
                    match carrier {
                        SourceSsaCarrierTypeV36::Pointer { .. } => counts[1] += 1,
                        SourceSsaCarrierTypeV36::Slice { .. } => counts[2] += 1,
                        SourceSsaCarrierTypeV36::Scalar(_) => {}
                        SourceSsaCarrierTypeV36::Vector(_) => {}
                    }
                    counts[3] += usize::from(loan.is_some());
                }
                SourceSsaPhysicalV36::EnumVariant { .. } | SourceSsaPhysicalV36::Unmodeled => {
                    unreachable!()
                }
            }
        }
    }
    Ok(counts)
}

#[test]
fn source_typed_endpoints_join_genuine_raw_pointer_unit_and_scalar_locals() {
    probe(
        typed_root_entry_rhs_owner_v18,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            let counts = checked_rows(relation, budget)?;
            assert!(counts[0] > 0);
            assert!(counts[1] > 0);
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn source_typed_endpoints_join_genuine_single_carrier_slice() {
    probe(
        slice_owner,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            assert!(checked_rows(relation, budget)?[2] > 0);
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn source_typed_endpoints_join_genuine_checked_thin_reference_and_repeated_instances() {
    probe(
        safe_private_mutable_owner_v18,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            assert!(checked_rows(relation, budget)?[3] > 0);
            assert!(
                relation
                    .source
                    .root_row(0)?
                    .rvalue_results
                    .as_ref()
                    .unwrap()
                    .values
                    .iter()
                    .any(|row| row.instance > 0)
            );
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn source_typed_endpoint_replay_detects_local_type_carrier_loan_and_instance_mutations() {
    probe(
        safe_private_mutable_owner_v18,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            let original = relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap();
            let at = original
                .values
                .iter()
                .position(|row| {
                    matches!(
                        row.typed.physical,
                        SourceSsaPhysicalV36::Value { loan: Some(_), .. }
                    )
                })
                .unwrap();
            for fault in 0..10 {
                let mut altered = OwnedSourceRvaluesV30 {
                    source: original.source,
                    ledger: original.ledger,
                    storage: original.storage,
                    rows: original.rows.clone(),
                    values: original.values.clone(),
                    carriers: original.carriers.clone(),
                    index_readers: original.index_readers.clone(),
                    enum_spills: original.enum_spills.clone(),
                };
                let row = &mut altered.values[at];
                match fault {
                    0 => {}
                    1 => row.instance = usize::MAX,
                    2 => row.typed.local = SemanticLocalIdV1::from_index(u32::MAX),
                    3 => row.typed.ty = SemanticTypeIdV1::from_index(u32::MAX),
                    4 => row.typed.physical = SourceSsaPhysicalV36::Unit,
                    _ => {
                        let SourceSsaPhysicalV36::Value { value, ty, loan } =
                            &mut row.typed.physical
                        else {
                            unreachable!()
                        };
                        match fault {
                            5 => *value = ValueId(u32::MAX),
                            6 => *ty = SourceSsaCarrierTypeV36::Scalar(ScalarType::U32),
                            7 => loan.as_mut().unwrap().loan = usize::MAX,
                            8 => loan.as_mut().unwrap().origin_generation ^= 1,
                            9 => {
                                loan.as_mut().unwrap().origin_local =
                                    SemanticLocalIdV1::from_index(u32::MAX)
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                assert_eq!(
                    original
                        .matches_replay_v30(&altered, budget)
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
fn source_typed_endpoints_reject_foreign_original_and_instance_with_sticky_refusal() {
    for fault in 0..2 {
        let result = probe(
            typed_root_entry_rhs_owner_v18,
            MODULE_LIMIT,
            MODULE_LIMIT,
            |relation, budget| {
                let row = relation
                    .source
                    .root_row(0)?
                    .rvalue_results
                    .as_ref()
                    .unwrap()
                    .values[0];
                let (instance, original) = if fault == 0 {
                    (
                        row.instance,
                        SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(u32::MAX)),
                    )
                } else {
                    (usize::MAX, row.original)
                };
                assert!(
                    relation
                        .ssa_typed_endpoint_v36(0, instance, original, budget)
                        .is_err()
                );
                assert!(
                    relation
                        .ssa_typed_endpoint_v36(0, row.instance, row.original, budget)
                        .is_err()
                );
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
fn source_typed_endpoint_getter_rejects_funded_foreign_ledger_without_charging() {
    let result = probe(
        typed_root_entry_rhs_owner_v18,
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
                .find(|row| row.typed.physical != SourceSsaPhysicalV36::Unmodeled)
                .unwrap();
            let endpoint =
                relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
            foreign.reserve_storage(budget.storage())?;
            let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
            assert!(matches!(
                endpoint.physical_type(&mut foreign),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(
                before,
                (foreign.work(), foreign.storage(), foreign.peak_storage())
            );
            assert!(matches!(
                endpoint.source_type(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            Ok(())
        },
    )
    .0;
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
}

#[test]
fn source_typed_endpoint_capture_and_query_exact_and_one_short_complete_resources() {
    fn consume(
        relation: &ProductionSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        assert!(checked_rows(relation, budget)?[1] > 0);
        Ok(())
    }
    let (result, work, storage) = probe(
        typed_root_entry_rhs_owner_v18,
        MODULE_LIMIT,
        MODULE_LIMIT,
        consume,
    );
    result.unwrap();
    let exact = probe(typed_root_entry_rhs_owner_v18, work, storage, consume);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    for short_work in [true, false] {
        let result = probe(
            typed_root_entry_rhs_owner_v18,
            work - usize::from(short_work),
            storage - usize::from(!short_work),
            consume,
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
fn source_typed_endpoint_carrier_identity_rejects_relabel_nested_pointer_and_vector() {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Generic, AccessMode::ReadOnly);
    let slice = Type::slice(scalar, AddressSpace::Generic, AccessMode::ReadOnly);
    let expected = SourceSsaCarrierTypeV36::from_type(&pointer).unwrap();
    assert!(expected.matches(&pointer));
    for changed in [
        slice,
        Type::vector(fe2o3_kernel_ir::FixedVectorTypeV12::new(
            ScalarType::U32,
            4,
            fe2o3_kernel_ir::VectorLayoutV12::Contiguous,
        )),
        Type::Scalar(ScalarType::U32),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Private,
            AccessMode::ReadOnly,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Generic,
            AccessMode::ReadWrite,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U64),
            AddressSpace::Generic,
            AccessMode::ReadOnly,
        ),
    ] {
        assert!(!expected.matches(&changed));
    }
    assert_eq!(
        SourceSsaCarrierTypeV36::from_type(&Type::pointer(
            pointer,
            AddressSpace::Generic,
            AccessMode::ReadOnly
        )),
        None
    );
    assert_eq!(SourceSsaCarrierTypeV36::from_type(&Type::Unit), None);
}

#[test]
fn source_typed_definition_local_join_has_independent_three_work_boundary() {
    for limit in [3, 2] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let mut definitions = [None];
        let result = source_ssa_record_local_v36(
            &mut definitions,
            SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(0)),
            fe2o3_mir_model::SsaVariableIdV1::new(7),
            &mut budget,
        );
        if limit == 3 {
            result.unwrap();
            assert_eq!(definitions, [Some(SemanticLocalIdV1::from_index(7))]);
            assert_eq!(budget.work(), 3);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
            assert_eq!(definitions, [None]);
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let mut definitions = [None];
    let value = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(0));
    source_ssa_record_local_v36(
        &mut definitions,
        value,
        fe2o3_mir_model::SsaVariableIdV1::new(0),
        &mut budget,
    )
    .unwrap();
    assert!(
        source_ssa_record_local_v36(
            &mut definitions,
            value,
            fe2o3_mir_model::SsaVariableIdV1::new(0),
            &mut budget
        )
        .is_err()
    );
}
