use super::*;

#[derive(Clone, Copy, Debug)]
enum TagDestination {
    Local,
    Field,
    Dereference,
    FieldDereference,
}

impl TagDestination {
    fn behind_pointer(self) -> bool {
        matches!(self, Self::Dereference | Self::FieldDereference)
    }

    fn field(self) -> bool {
        matches!(self, Self::Field | Self::FieldDereference)
    }
}

fn tag_destination_owner(destination: TagDestination) -> ProductionSemanticSsaOwnerV1 {
    let original = tag_only_activation_owner_v29(1);
    let enumeration = original.source_semantic().types()[ENUM.index() as usize].clone();
    owner_with(Case::Shared, |types, functions| {
        assert_eq!(types.len(), ENUM.index() as usize);
        types.push(enumeration);
        let raw = SemanticTypeIdV1::from_index(5);
        if destination.behind_pointer() {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([172; 32]),
                SemanticLayoutIdentityV1::from_sha256([172; 32]),
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
                        ENUM,
                        SemanticPointerKindV1::Raw,
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ));
        }
        let field = if destination.behind_pointer() {
            raw
        } else {
            ENUM
        };
        let holder = if destination.field() {
            let holder = SemanticTypeIdV1::from_index(types.len() as u32);
            let (size, alignment) = if destination.behind_pointer() {
                (8, 8)
            } else {
                (1, 1)
            };
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([173; 32]),
                SemanticLayoutIdentityV1::from_sha256([173; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(size),
                    alignment,
                    SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![field]).unwrap()),
            ));
            holder
        } else {
            field
        };
        let mut locals = vec![
            local(30, UNIT, SemanticLocalRoleV1::Return),
            local(31, CAPTURE, SemanticLocalRoleV1::Argument(0)),
            local(172, holder, SemanticLocalRoleV1::Temporary),
        ];
        let mut statements = Vec::new();
        if destination.behind_pointer() {
            locals.push(local(173, ENUM, SemanticLocalRoleV1::Temporary));
            statements.push(assign(
                place(3, ENUM),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::EnumVariant(0), vec![])
                        .unwrap(),
                ),
            ));
            let pointer_local = if destination.field() {
                locals.push(local(174, raw, SemanticLocalRoleV1::Temporary));
                4
            } else {
                2
            };
            statements.push(assign(
                place(pointer_local, raw),
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: place(3, ENUM),
                },
            ));
            if destination.field() {
                statements.push(assign(
                    place(2, holder),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Tuple,
                            vec![SemanticOperandV1::Copy(place(pointer_local, raw))],
                        )
                        .unwrap(),
                    ),
                ));
            }
        }
        let mut projections = Vec::new();
        if destination.field() {
            projections.push(
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), field).unwrap(),
            );
        }
        if destination.behind_pointer() {
            projections.push(
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ENUM).unwrap(),
            );
        }
        statements.push(statement(SemanticStatementKindV1::SetDiscriminant {
            place: SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), projections, ENUM)
                .unwrap(),
            variant_index: 1,
        }));
        statements.push(unit());
        functions[2] = function(
            30,
            false,
            CAPTURE,
            locals,
            vec![block(170, statements, SemanticTerminatorKindV1::Return)],
        );
    })
}

#[test]
fn original_tag_destinations_select_only_the_actual_local_allocation() {
    for destination in [
        TagDestination::Local,
        TagDestination::Field,
        TagDestination::Dereference,
        TagDestination::FieldDereference,
    ] {
        let owner = tag_destination_owner(destination);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(59).unwrap();
        let demands =
            source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)
                .unwrap();
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let (requests, _) = demands
                    .root_lens(&owner, 0, budget)
                    .unwrap()
                    .requests(instances, budget)
                    .unwrap();
                let mut checked = 0;
                for (ordinal, instance) in instances.instances().iter().enumerate() {
                    if instance.function().index() != 2 {
                        continue;
                    }
                    let id = instances.id_at(ordinal).unwrap();
                    let target = SemanticLocalIdV1::from_index(2);
                    let flags =
                        scoped_slot_candidates_v29(instance.declaration(), instance.ssa(), budget)
                            .unwrap();
                    assert_eq!(
                        flags[2],
                        if destination.behind_pointer() { 1 } else { 2 },
                        "{destination:?}"
                    );
                    let found = requests
                        .iter()
                        .filter(|row| row.instance == id && row.local == target)
                        .collect::<Vec<_>>();
                    if destination.behind_pointer() {
                        assert!(
                            found.is_empty(),
                            "pointee tag writes do not allocate their pointer holder"
                        );
                        assert!(
                            source_backing_original_request_v29(
                                requests,
                                id,
                                SemanticLocalIdV1::from_index(3),
                                budget
                            )
                            .unwrap(),
                            "the separate original AddressOf still owns its pointee backing"
                        );
                    } else {
                        assert_eq!(found.len(), 1);
                        assert_eq!(
                            found[0].kind,
                            source_storage_demands_v29::DemandKindV29::WholeBackingCandidate
                        );
                        assert_eq!(found[0].ty, instance.declaration().locals()[2].ty());
                        assert!(found[0].path.is_empty());
                        assert!(
                            source_backing_original_request_v29(requests, id, target, budget)
                                .unwrap()
                        );
                    }
                    checked += 1;
                    let retained = flags.capacity() * std::mem::size_of::<u8>();
                    drop(flags);
                    budget.release_storage(retained).unwrap();
                }
                assert_eq!(
                    checked, 2,
                    "both original helper call instances are required"
                );
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
        demands.discard(&mut budget).unwrap();
        assert_eq!(budget.storage(), 59);
    }
}

#[test]
fn original_promoted_enum_holder_is_not_forced_into_local_backing() {
    let owner = enum_owner(EnumCase::CorrelatedLoans);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let (requests, _) = demands
                .root_lens(&owner, 0, budget)
                .unwrap()
                .requests(instances, budget)
                .unwrap();
            let mut checked = 0;
            for (ordinal, instance) in instances.instances().iter().enumerate() {
                if instance.function().index() != 2 {
                    continue;
                }
                let local = SemanticLocalIdV1::from_index(5);
                assert_eq!(instance.declaration().locals()[5].ty(), ENUM);
                assert!(
                    instance
                        .ssa()
                        .plan()
                        .promoted_variables()
                        .iter()
                        .any(|variable| variable.get() == 5)
                );
                assert!(
                    !source_backing_original_request_v29(
                        requests,
                        instances.id_at(ordinal).unwrap(),
                        local,
                        budget
                    )
                    .unwrap()
                );
                checked += 1;
            }
            assert_eq!(checked, 2);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
    demands.discard(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn tag_destination_visitor_has_independent_exact_and_one_short_work() {
    for destination in [
        TagDestination::Local,
        TagDestination::Field,
        TagDestination::Dereference,
        TagDestination::FieldDereference,
    ] {
        let owner = tag_destination_owner(destination);
        let function = &owner.source_semantic().functions()[2];
        // One block, each original statement, one terminator; each local root
        // adds visitor3 + callback2. Tag paths pay every projection exactly once.
        let exact = match destination {
            TagDestination::Local => 1 + 2 + 1 + 3 + 2,
            TagDestination::Field => 1 + 2 + 1 + 1 + 3 + 2,
            TagDestination::Dereference => 1 + 4 + 1 + 3 + 2 + 1,
            TagDestination::FieldDereference => 1 + 5 + 1 + 3 + 2 + 2,
        };
        for short in [false, true] {
            let limit = exact - usize::from(short);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 71);
            budget.reserve_storage(71).unwrap();
            let mut roots = Vec::new();
            let result = visit_private_slot_roots_v1(function, &mut budget, |local, budget| {
                budget.charge_work(2)?;
                roots.push(local);
                Ok(())
            });
            assert_eq!(result.is_err(), short, "{destination:?}");
            assert_eq!(
                budget.work(),
                limit,
                "the one-short denial is the final terminator visit"
            );
            assert_eq!(budget.storage(), 71);
            assert_eq!(roots, [if destination.behind_pointer() { 3 } else { 2 }]);
            if short {
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
    }
}
