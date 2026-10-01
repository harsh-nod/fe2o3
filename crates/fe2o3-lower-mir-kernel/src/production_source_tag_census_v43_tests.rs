include!("production_source_object_completion_gate_v44_tests.rs");

#[test]
fn nonvalue_tag_rows_and_constructor_views_still_require_complete_original_census() {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENUM_CONSTRUCTION_FIELDS_V43.set(self.0);
        }
    }
    let _restore = Restore(ENUM_CONSTRUCTION_FIELDS_V43.get());
    for fields in [0, 1, 3] {
        ENUM_CONSTRUCTION_FIELDS_V43.set(fields);
        for fault in [0, 1, 4, 10] {
            run_tag_census_case_v43(original_enum_construction_owner_v43, fault);
        }
    }
}

thread_local! {
    static TAG_CENSUS_FAULT_V43: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static TAG_CENSUS_VISITS_V43: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn alter_tag_census_v43(
    source_index: &SourceAddressSourceIndexV29<'_>,
    rows: &mut Vec<SourceAddressAccessSourceV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Vec<usize>>(budget)?;
    let mut tags = emission_vec_v1(rows.len(), budget)?;
    for (index, source) in rows.iter().enumerate() {
        budget.charge_work(2)?;
        let sidecar = source_index.sidecar(source.instance, budget)?;
        let anchors = sidecar.scoped_memory_anchors.as_ref().unwrap();
        let row = &anchors.rows[source.anchor];
        if matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_))
            && matches!(
                anchors.object_payload(row, budget)?.operation,
                ScopedObjectOperationV29::ReadDiscriminant { .. }
                    | ScopedObjectOperationV29::SetDiscriminant { .. }
            )
        {
            tags.push(index);
        }
    }
    assert!(
        tags.len() >= 4 && tags.len().is_multiple_of(4),
        "two exact tag writes and two reads per original helper invocation"
    );
    let first = tags[0];
    let last = *tags.last().unwrap();
    budget.charge_work(5)?;
    match TAG_CENSUS_FAULT_V43.get() {
        0 => (),
        1 => {
            budget.charge_work(rows.len())?;
            rows.remove(last);
        }
        2 => {
            assert!(rows.len() < rows.capacity());
            rows.push(rows[first]);
        }
        3 => rows[first].anchor = rows[last].anchor,
        4 => rows[first].direct_object.as_mut().unwrap().generation += 1,
        5 => {
            rows[first].direct_object.as_mut().unwrap().local =
                SemanticLocalIdV1::from_index(u32::MAX)
        }
        6 => {
            rows[first].direct_object.as_mut().unwrap().ty = SemanticTypeIdV1::from_index(u32::MAX)
        }
        7 => rows[first].physical.operation += 1,
        8 => rows[first].instance = ProductionCallInstanceIdV1(usize::MAX),
        9 => rows[first].anchor = usize::MAX,
        10 => {
            for index in tags.into_iter().rev() {
                budget.charge_work(rows.len())?;
                rows.remove(index);
            }
        }
        _ => panic!("unknown tag census mutation"),
    }
    TAG_CENSUS_VISITS_V43.set(TAG_CENSUS_VISITS_V43.get() + 1);
    Ok(())
}

fn run_tag_census_case_v43(factory: fn() -> ProductionSemanticSsaOwnerV1, fault: u8) {
    struct Restore(Option<SourceObjectEffectCensusObserverV29>, u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29.set(self.0);
            TAG_CENSUS_FAULT_V43.set(self.1);
        }
    }
    let _restore = Restore(
        SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29.replace(Some(alter_tag_census_v43)),
        TAG_CENSUS_FAULT_V43.replace(fault),
    );
    TAG_CENSUS_VISITS_V43.set(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(COMPLETE_OBJECT_SOURCE_WORK_LIMIT_V43);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
    let completed = std::cell::Cell::new(false);
    let result =
        prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            completed.set(true);
            Ok(())
        });
    assert!(TAG_CENSUS_VISITS_V43.get() > 0, "fault={fault}: {result:?}");
    assert_eq!(completed.get(), fault == 0, "fault={fault}: {result:?}");
    assert_eq!(result.is_ok(), fault == 0, "fault={fault}: {result:?}");
    if matches!(fault, 1 | 10) {
        assert!(result.unwrap_err().to_string().contains(
            "original typed tag effect is missing or unrelated to its source occurrence"
        ));
    }
    assert_eq!(budget.storage(), MODULE_FLOOR, "fault={fault}");
}

#[test]
fn original_tag_census_requires_exact_complete_unique_occurrences_and_generations() {
    for fault in 0..=10 {
        run_tag_census_case_v43(original_tag_emission_owner, fault);
    }
}

#[test]
fn original_empty_enum_constructors_require_tags_independently_of_payload_completeness() {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENUM_CONSTRUCTION_FIELDS_V43.set(self.0);
        }
    }
    let _restore = Restore(ENUM_CONSTRUCTION_FIELDS_V43.replace(0));
    for fault in [0, 1, 2, 10] {
        run_tag_census_case_v43(original_enum_construction_owner_v43, fault);
    }
}

#[test]
fn original_tag_demand_uses_original_place_identity_role_and_exact_paid_work() {
    let owner = original_tag_emission_owner();
    let function = &owner.source_semantic().functions()[2];
    let mut tested = 0;
    for (block, source_block) in function.blocks().iter().enumerate() {
        for (statement, row) in source_block.statements().iter().enumerate() {
            let (place, access) = match row.kind() {
                SemanticStatementKindV1::SetDiscriminant { place, .. } => {
                    (place, SourceReferenceAccessV29::Write)
                }
                SemanticStatementKindV1::Assign(assignment) => match assignment.value().kind() {
                    SemanticRvalueKindV1::Discriminant(place) => {
                        (place, SourceReferenceAccessV29::ReadDiscriminant)
                    }
                    _ => continue,
                },
                _ => continue,
            };
            let site = SourceReferenceSiteV29 {
                instance: ProductionCallInstanceIdV1(0),
                block: SemanticBlockIdV1::from_index(block as u32),
                statement: Some(statement),
            };
            let key = SourceReferenceAccessKeyV29 {
                site,
                source: place as *const SemanticPlaceV1 as usize,
                access,
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
            let mut budget = ArgumentBudgetV1::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            assert_eq!(
                source_address_original_tag_demand_v43(function, key, &mut budget).unwrap(),
                Some(SourceTagDemandV43::Statement)
            );
            assert_eq!(budget.work(), 4);
            let copied = place.clone();
            assert_eq!(
                source_address_original_tag_demand_v43(
                    function,
                    SourceReferenceAccessKeyV29 {
                        source: &copied as *const SemanticPlaceV1 as usize,
                        ..key
                    },
                    &mut budget
                )
                .unwrap(),
                None
            );
            assert_eq!(
                source_address_original_tag_demand_v43(
                    function,
                    SourceReferenceAccessKeyV29 {
                        access: SourceReferenceAccessV29::Address,
                        ..key
                    },
                    &mut budget
                )
                .unwrap(),
                None
            );
            assert_eq!(budget.storage(), 17);
            let mut short = CanonicalKernelIrWorkBudgetV1::new(3);
            let mut short = ArgumentBudgetV1::new(&mut short, 17);
            short.reserve_storage(17).unwrap();
            assert!(
                matches!(source_address_original_tag_demand_v43(function, key, &mut short),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(limit)))
                    if limit.limit() == 3 && limit.actual() == 4)
            );
            assert_eq!(short.storage(), 17);
            tested += 1;
        }
    }
    assert_eq!(tested, 4);
}

#[test]
fn original_tag_query_headers_have_an_independent_fixed_envelope() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let expected = h::<SourceAddressAccessSourceV29>()
        + h::<Option<SourceAddressAccessSourceV29>>()
        + h::<ScopedObjectEndpointV29>()
        + h::<ScopedObjectTagOriginV29>()
        + h::<SourceDirectObjectOriginV29>()
        + h::<SourceAddressTagAccessV43>()
        + h::<SourceTagDemandV43>()
        + h::<Option<(usize, SourceTagDemandV43)>>()
        + h::<SourceReferenceAccessKeyV29>()
        + h::<SourceReferenceAccessIndexKeyV29>()
        + h::<SourceReferenceSiteV29>()
        + h::<ScopedMemoryFrameV29>()
        + h::<ProductionSemanticSsaFunctionOccurrencesV1<'_>>()
        + h::<ScopedEmittedPointsV29<'_, '_, '_>>()
        + h::<SourceStaticObjectLocationV29>()
        + h::<Option<(u64, u64)>>()
        + 20 * size_of::<usize>()
        + 16 * size_of::<&()>();
    assert_eq!(source_address_tag_query_headers_v43().unwrap(), expected);
}
