const ORIGINAL_ACCESS_FLOOR: usize = 113;
const ORIGINAL_LOAD_WORK: usize = 2 + 2 + 3 + 3;

fn original_load_peak() -> usize {
    // Destination place, operand visitor, and load place each prepay the
    // existing unit local plus both constructor/caller result envelopes.
    ORIGINAL_ACCESS_FLOOR
        + 3 * (std::mem::size_of::<()>()
            + 2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>())
}

fn with_original_access_source(
    flow: AddressFlow,
    consume: impl FnOnce(&SemanticFunctionDeclV1, ProductionCallInstanceIdV1),
) {
    let mut owner = address_owner(flow);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(20_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 20_000_000);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, _| {
        let root = instances.root();
        consume(instances.instance(root).unwrap().declaration(), root);
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
}

fn original_load_source(function: &SemanticFunctionDeclV1) -> &SemanticPlaceV1 {
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[0].statements()[4].kind()
    else {
        panic!("original load assignment");
    };
    let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
        panic!("original load source");
    };
    load.source()
}

#[test]
fn original_access_queries_release_finished_envelopes_at_constant_prepaid_peak() {
    with_original_access_source(AddressFlow::Read, |function, root| {
        let source = original_load_source(function);
        let site = SourceReferenceSiteV29 {
            instance: root,
            block: SemanticBlockIdV1::from_index(0),
            statement: Some(4),
        };
        for count in [1, 32, 64, 128] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(count * ORIGINAL_LOAD_WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, original_load_peak());
            budget.reserve_storage(ORIGINAL_ACCESS_FLOOR).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            for _ in 0..count {
                let result =
                    source_reference_raw_original_access_v29(function, site, source, &mut budget);
                assert!(
                    matches!(result, Ok(Some(SourceReferenceAccessV29::Read))),
                    "{result:?}"
                );
                assert_eq!(budget.storage(), ORIGINAL_ACCESS_FLOOR);
                assert!(budget.work_ledger_identity_v1() == ledger);
            }
            assert_eq!(budget.work(), count * ORIGINAL_LOAD_WORK);
            assert_eq!(budget.peak_storage(), original_load_peak());
        }
    });
}

#[test]
fn original_access_query_exact_and_short_limits_restore_outer_storage() {
    with_original_access_source(AddressFlow::Read, |function, root| {
        let source = original_load_source(function);
        let site = SourceReferenceSiteV29 {
            instance: root,
            block: SemanticBlockIdV1::from_index(0),
            statement: Some(4),
        };
        for available in 0..=ORIGINAL_LOAD_WORK {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(available);
            let mut budget = ArgumentBudgetV1::new(&mut work, original_load_peak());
            budget.reserve_storage(ORIGINAL_ACCESS_FLOOR).unwrap();
            let result =
                source_reference_raw_original_access_v29(function, site, source, &mut budget);
            if available == ORIGINAL_LOAD_WORK {
                assert!(
                    matches!(result, Ok(Some(SourceReferenceAccessV29::Read))),
                    "{result:?}"
                );
                assert_eq!(budget.work(), ORIGINAL_LOAD_WORK);
                assert_eq!(budget.failed_work(), None);
            } else {
                let expected = [2, 4, 7, 10]
                    .into_iter()
                    .find(|&next| next > available)
                    .unwrap();
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.actual() == expected && error.limit() == available),
                    "available={available}: {result:?}"
                );
                assert_eq!(budget.failed_work(), Some(expected));
            }
            assert_eq!(budget.storage(), ORIGINAL_ACCESS_FLOOR);
            assert_eq!(budget.failed_storage(), None);
            if available >= ORIGINAL_LOAD_WORK - 1 {
                assert_eq!(budget.peak_storage(), original_load_peak());
            }
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(ORIGINAL_LOAD_WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, original_load_peak() - 1);
        budget.reserve_storage(ORIGINAL_ACCESS_FLOOR).unwrap();
        let result = source_reference_raw_original_access_v29(function, site, source, &mut budget);
        assert!(
            matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            ArgumentResourceV1::Storage(error)))
            if error.actual() == original_load_peak() && error.limit() == original_load_peak() - 1),
            "{result:?}"
        );
        assert_eq!(
            budget.work(),
            ORIGINAL_LOAD_WORK - 3,
            "the final place envelope must be paid before its work and source comparison"
        );
        assert_eq!(budget.storage(), ORIGINAL_ACCESS_FLOOR);
        assert_eq!(budget.failed_storage(), Some(original_load_peak()));
        assert_eq!(budget.failed_work(), None);
    });
}

#[test]
fn original_access_query_keeps_exact_borrowed_source_and_site_checks() {
    with_original_access_source(AddressFlow::Read, |function, root| {
        let source = original_load_source(function);
        let cloned = source.clone();
        let foreign_owner = address_owner(AddressFlow::Read);
        let foreign_function = &foreign_owner.source_semantic().functions()[0];
        let foreign_source = original_load_source(foreign_function);
        let site = SourceReferenceSiteV29 {
            instance: root,
            block: SemanticBlockIdV1::from_index(0),
            statement: Some(4),
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4_096);
        budget.reserve_storage(ORIGINAL_ACCESS_FLOOR).unwrap();
        for fault in 0..6 {
            assert!(matches!(
                source_reference_raw_original_access_v29(function, site, source, &mut budget),
                Ok(Some(SourceReferenceAccessV29::Read))
            ));
            assert_eq!(budget.storage(), ORIGINAL_ACCESS_FLOOR);
            let result = source_reference_raw_original_access_v29(
                if fault == 2 {
                    foreign_function
                } else {
                    function
                },
                match fault {
                    3 => SourceReferenceSiteV29 {
                        statement: Some(3),
                        ..site
                    },
                    4 => SourceReferenceSiteV29 {
                        block: SemanticBlockIdV1::from_index(u32::MAX),
                        ..site
                    },
                    5 => SourceReferenceSiteV29 {
                        statement: Some(usize::MAX),
                        ..site
                    },
                    _ => site,
                },
                match fault {
                    0 => &cloned,
                    1 => foreign_source,
                    _ => source,
                },
                &mut budget,
            );
            if fault < 4 {
                assert!(matches!(result, Ok(None)), "fault={fault}: {result:?}");
            } else {
                assert!(
                    matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ),
                    "fault={fault}: {result:?}"
                );
            }
            assert_eq!(budget.storage(), ORIGINAL_ACCESS_FLOOR);
        }
    });
}

#[test]
fn original_access_query_preserves_destination_address_operand_and_borrow_roles() {
    for flow in [AddressFlow::Read, AddressFlow::ReferenceCast] {
        with_original_access_source(flow, |function, root| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 4_096);
            budget.reserve_storage(ORIGINAL_ACCESS_FLOOR).unwrap();
            let mut seen = [false; 4];
            for (index, statement) in function.blocks()[0].statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let site = SourceReferenceSiteV29 {
                    instance: root,
                    block: SemanticBlockIdV1::from_index(0),
                    statement: Some(index),
                };
                assert!(matches!(
                    source_reference_raw_original_access_v29(
                        function,
                        site,
                        assignment.destination(),
                        &mut budget
                    ),
                    Ok(Some(SourceReferenceAccessV29::Write))
                ));
                seen[0] = true;
                let (source, expected, kind) = match assignment.value().kind() {
                    SemanticRvalueKindV1::AddressOf { place, .. } => {
                        (place, SourceReferenceAccessV29::Address, 1)
                    }
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => {
                        (place, SourceReferenceAccessV29::Read, 2)
                    }
                    SemanticRvalueKindV1::Borrow { kind, place } => {
                        (place, SourceReferenceAccessV29::Borrow(*kind), 3)
                    }
                    _ => continue,
                };
                let result =
                    source_reference_raw_original_access_v29(function, site, source, &mut budget)
                        .unwrap();
                assert_eq!(result, Some(expected));
                assert_eq!(budget.storage(), ORIGINAL_ACCESS_FLOOR);
                seen[kind] = true;
            }
            assert_eq!(
                seen,
                if flow == AddressFlow::Read {
                    [true, true, true, false]
                } else {
                    [true, false, true, true]
                }
            );
            assert_eq!(budget.storage(), ORIGINAL_ACCESS_FLOOR);
        });
    }
}
