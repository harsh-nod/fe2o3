//! Independent expectations for the pinned raw classifier fixture. This is not
//! an issuer oracle or a substitute for the original source-use/owner tests.
use super::*;

pub(super) struct Expected {
    roles: usize,
    shared8: bool,
}

fn snapshot(routes: &Routes<'_>) -> BTreeMap<SemanticTypeIdV1, (SemanticTypeIdV1, Route)> {
    routes
        .shared
        .iter()
        .map(|(&ty, c)| (ty, (c.pointee, c.route)))
        .collect()
}

fn fixture_fields(types: &[SemanticTypeDeclV1], id: u32) -> &[SemanticTypeIdV1] {
    let SemanticTypeShapeV1::Aggregate(a) = types[id as usize].shape() else {
        panic!("fixture carrier {id} is no longer an aggregate");
    };
    a.fields()
}

/// The complete possible two-role leaves are enumerated from the fixture's
/// pinned fields, not from the production walker or its secondary map.
pub(super) fn assert_maps(old: &Routes<'_>, new: &Routes<'_>, wg: &Pairs) -> Expected {
    assert_maps_with_policy(old, new, &Pairs::new(), &Pairs::new(), wg)
}

pub(super) fn assert_maps_with_policy(
    old: &Routes<'_>,
    new: &Routes<'_>,
    primary_pairs: &Pairs,
    policy: &Pairs,
    wg: &Pairs,
) -> Expected {
    assert!(std::ptr::eq(old.function, new.function));
    assert!(std::ptr::eq(old.types, new.types));
    assert!(
        old.secondaries.is_empty(),
        "the cold oracle must remain legacy"
    );
    assert_eq!(
        old.routes, new.routes,
        "primary selection is never replaced"
    );
    assert_eq!(fixture_fields(old.types, 7), &[ty(5), ty(6), ty(4), ty(4)]);
    assert_eq!(fixture_fields(old.types, 14), &[ty(5), ty(6)]);
    assert_eq!(fixture_fields(old.types, 13), &[ty(6), ty(4)]);
    assert_eq!(fixture_fields(old.types, 15), &[ty(6), ty(6)]);
    assert_eq!(fixture_fields(old.types, 16), &[ty(5), ty(5), ty(6)]);
    assert_eq!(fixture_fields(old.types, 17), &[ty(6), ty(6), ty(5)]);
    assert_eq!(fixture_fields(old.types, 18), &[ty(13), ty(4)]);
    assert!(policy.is_empty() || *policy == Pairs::from([(ty(6), ty(3))]));
    for (reference, owned) in [(5, 2), (6, 3)] {
        let SemanticTypeShapeV1::Pointer(p) = old.types[reference as usize].shape() else {
            panic!()
        };
        assert_eq!(
            (
                p.kind(),
                p.mutability(),
                p.address_space(),
                p.pointer_width_bits(),
                p.metadata(),
                p.pointee()
            ),
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
                ty(owned)
            )
        );
    }
    // Only these two pinned carriers have a unique Policy beside a primary.
    // Types15/17 duplicate Policy, type16 duplicates primary, and types13/18
    // contain only Policy, which stays primary instead of becoming secondary.
    let mut policy_roles = BTreeMap::new();
    if !policy.is_empty()
        && primary_pairs.get(&ty(5)) == Some(&ty(2))
        && !primary_pairs.contains_key(&ty(6))
    {
        for carrier in [7, 14] {
            if let Some(primary) = old.routes.get(&ty(carrier)) {
                if primary.reference == ty(5) {
                    assert_eq!(
                        (primary.owned, primary.len, primary.fields[0]),
                        (ty(2), 1, 0)
                    );
                    let mut fields = [0; MAX_FIELDS];
                    fields[0] = 1;
                    policy_roles.insert(
                        ty(carrier),
                        Route {
                            reference: ty(6),
                            owned: ty(3),
                            fields,
                            len: 1,
                        },
                    );
                }
            }
        }
    }
    let leaves: Vec<_> = [(5, 2, 0), (6, 3, 1)]
        .into_iter()
        .filter(|&(reference, owned, _)| wg.get(&ty(reference)) == Some(&ty(owned)))
        .collect();
    let mut roles = BTreeMap::new();
    if let [(reference, owned, field)] = leaves.as_slice() {
        for carrier in [7, 14] {
            if let Some(primary) = old.routes.get(&ty(carrier)) {
                if primary.reference == ty(*reference) {
                    continue;
                }
                assert!(matches!(primary.reference.index(), 5 | 6));
                assert_eq!(primary.len, 1);
                assert_eq!(primary.fields[0], u32::from(primary.reference == ty(6)));
                // The fixture's Policy and Workgroup seeds may name the same
                // leaf; that one path cannot be stored under both roles.
                if *reference == 6 && policy_roles.contains_key(&ty(carrier)) {
                    continue;
                }
                let mut fields = [0; MAX_FIELDS];
                fields[0] = *field;
                roles.insert(
                    ty(carrier),
                    Route {
                        reference: ty(*reference),
                        owned: ty(*owned),
                        fields,
                        len: 1,
                    },
                );
            }
        }
    }
    for slots in new.secondaries.values() {
        assert!(
            slots.iter().any(Option::is_some),
            "no empty secondary entries"
        );
        if let Some((role, _)) = slots[0] {
            assert_eq!(role, SecondaryRole::Policy, "slot0 is Policy");
        }
        if let Some((role, _)) = slots[1] {
            assert_eq!(role, SecondaryRole::Workgroup, "slot1 is Workgroup");
        }
        if policy.is_empty() {
            assert!(
                slots[0].is_none(),
                "fixtures without Policy must keep slot0 empty"
            );
        }
    }
    let projected_policy: BTreeMap<_, _> = new.secondaries.iter()
        .filter_map(|(&ty, slots)| slots[0].map(|(_, route)| (ty, route)))
        .collect();
    let projected_workgroup: BTreeMap<_, _> = new.secondaries.iter()
        .filter_map(|(&ty, slots)| slots[1].map(|(_, route)| (ty, route)))
        .collect();
    assert_eq!(
        projected_policy, policy_roles,
        "complete independent Policy roster"
    );
    assert_eq!(
        projected_workgroup, roles,
        "complete independent Workgroup roster"
    );
    let mut shared = snapshot(old);
    let shared8 = roles.get(&ty(7)).copied();
    if let Some(route) = shared8 {
        assert!(!old.routes.contains_key(&ty(8)));
        assert!(shared.insert(ty(8), (ty(7), route)).is_none());
    }
    // The ordering fixture adds other pointer types, but none is another
    // canonical shared pointer around either newly mixed carrier.
    for local in old.function.locals() {
        if let Some(SemanticTypeShapeV1::Pointer(p)) = old
            .types
            .get(local.ty().index() as usize)
            .map(SemanticTypeDeclV1::shape)
        {
            if matches!(p.pointee().index(), 7 | 14)
                && p.kind() == SemanticPointerKindV1::Reference
                && p.mutability() == SemanticMutabilityV1::Immutable
                && p.address_space() == 0
                && p.pointer_width_bits() == 64
                && p.metadata() == SemanticPointerMetadataV1::None
            {
                assert_eq!(
                    local.ty(),
                    ty(8),
                    "new fixture requires an explicit oracle extension"
                );
            }
        }
    }
    assert_eq!(snapshot(new), shared, "no unlisted shared-route additions");
    if roles.is_empty() && policy_roles.is_empty() {
        same(old, new);
    }
    Expected {
        roles: roles.len() + policy_roles.len(),
        shared8: shared8.is_some(),
    }
}

pub(super) fn assert_root_inventory(body: &SemanticFunctionDeclV1) {
    assert_eq!(body.locals().len(), 22);
    assert_eq!(
        body.locals()[16].ty(),
        ty(14),
        "type14 is retained as local16"
    );
    assert_eq!(body.locals()[14].ty(), ty(13), "local14 is not type14");
    for block in body.blocks() {
        for statement in block.statements() {
            if let SemanticStatementKindV1::Assign(a) = statement.kind() {
                assert_ne!(a.destination().local().index(), 16);
                a.value()
                    .kind()
                    .try_visit_operands::<std::convert::Infallible>(|operand| {
                        if let SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) = operand {
                            assert_ne!(p.local().index(), 16);
                        }
                        Ok(())
                    })
                    .unwrap();
                if let SemanticRvalueKindV1::Borrow { place, .. } = a.value().kind() {
                    assert_ne!(place.local().index(), 16);
                }
            }
        }
    }
    let SemanticStatementKindV1::Assign(a) = body.blocks()[4].statements()[0].kind() else {
        panic!()
    };
    assert_original_borrow(a);
}

fn assert_original_borrow(a: &SemanticAssignmentV1) -> &SemanticPlaceV1 {
    assert_eq!(
        (a.destination().local().index(), a.destination().ty()),
        (8, ty(8))
    );
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = a.value().kind()
    else {
        panic!()
    };
    assert_eq!((place.local().index(), place.ty()), (7, ty(7)));
    assert!(place.projections().is_empty());
    place
}

impl Expected {
    pub(super) fn has_roles(&self) -> bool {
        self.roles != 0
    }
    pub(super) fn new_shared_borrow(&self) -> bool {
        self.shared8
    }

    /// Every assignment is compared. Only the original pinned borrow changes
    /// meaning; the newly nonempty shared map also adds a debit on missing keys.
    pub(super) fn assert_root_query(
        &self,
        old: &Routes<'_>,
        new: &Routes<'_>,
        site: (usize, usize),
        a: &SemanticAssignmentV1,
    ) -> bool {
        let (mut aw, mut bw) = (budget(MAX_FLOW_WORK), budget(MAX_FLOW_WORK));
        let ap = old.source(a, &mut aw).unwrap();
        let bp = new.source(a, &mut bw).unwrap();
        let changed = self.shared8 && site == (4, 0);
        let extra = if changed {
            let place = assert_original_borrow(a);
            assert!(ap.is_none());
            assert!(std::ptr::eq(bp.unwrap(), place));
            assert_eq!(aw.remaining, MAX_FLOW_WORK);
            // shared lookup2 + copy21 + projection1 + role lookup(2 or3) + copy21.
            match self.roles {
                1 => 47,
                2 => 48,
                _ => panic!("unexpected fixture role count"),
            }
        } else {
            assert_eq!(ap, bp, "unaffected source query at {site:?}");
            if let (Some(ap), Some(bp)) = (ap, bp) {
                assert!(std::ptr::eq(ap, bp));
            }
            if self.shared8 && !old.routes.contains_key(&a.destination().ty()) {
                assert!(old.shared.is_empty());
                assert_eq!(new.shared.len(), 1);
                assert!(ap.is_none() && bp.is_none());
                assert!(a.destination().projections().is_empty());
                assert_eq!(a.destination().ty(), a.value().result_type());
                2 // One ordered lookup in the newly nonempty shared map.
            } else {
                0
            }
        };
        assert_eq!(
            aw.remaining,
            bw.remaining + extra,
            "exact source debit delta at {site:?}"
        );
        assert_eq!(
            old.disjoint_field_use(a, &mut aw).unwrap(),
            new.disjoint_field_use(a, &mut bw).unwrap(),
            "original disjoint result at {site:?}"
        );
        assert_eq!(
            aw.remaining,
            bw.remaining + extra,
            "no unaccounted disjoint debit at {site:?}"
        );
        changed
    }
}

fn getter(outer: bool, field: u32, reference: u32, moved: bool) -> SemanticAssignmentV1 {
    let mut projections = Vec::new();
    if outer {
        projections
            .push(SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(7)).unwrap());
    }
    projections.push(
        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty(reference)).unwrap(),
    );
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(if outer { 8 } else { 7 }),
        projections,
        ty(reference),
    )
    .unwrap();
    SemanticAssignmentV1::new(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(reference),
            vec![],
            ty(reference),
        )
        .unwrap(),
        SemanticRvalueV1::new(
            ty(reference),
            SemanticRvalueKindV1::Use(if moved {
                SemanticOperandV1::Move(place)
            } else {
                SemanticOperandV1::Copy(place)
            }),
        ),
    )
}

fn key_units(entries: usize) -> usize {
    if entries == 0 {
        1
    } else {
        2 + entries.ilog2() as usize
    }
}

#[test]
fn independent_projection_truth_table_and_exact_debits_cover_the_deliberate_extension() {
    let source = fixture::source(Mutation::None);
    let body = &source.functions()[0];
    assert_root_inventory(body);
    let primary = Pairs::from([(ty(5), ty(2))]);
    let wg = Pairs::from([(ty(6), ty(3))]);
    let old = cold(
        body,
        source.types(),
        &primary,
        &Pairs::new(),
        &BTreeSet::new(),
        &wg,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let new = Routes::from_pairs_with_workgroups(
        body,
        source.types(),
        &primary,
        &Pairs::new(),
        &BTreeSet::new(),
        &wg,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let expected = assert_maps(&old, &new, &wg);
    assert_eq!((expected.roles, expected.shared8), (2, true));
    let keys = key_units(old.routes.len());
    // Direct field0 remains primary; field1 is the new independent WG leaf.
    // The outer pointer was wholly unsupported by the legacy route selection.
    // Direct alternates cost lookup3 + copy21; disjoint checks reuse the typed
    // projection and add comparison2. Outer alternates cost 3+2+3+keys+21.
    for (
        outer,
        field,
        reference,
        old_source,
        new_source,
        old_disjoint,
        new_disjoint,
        old_cost,
        new_cost,
        old_d,
        new_d,
        move_cost,
    ) in [
        (false, 0, 5, true, true, false, false, 2, 2, 2, 2, 1),
        (false, 1, 6, false, true, true, false, 2, 26, 2, 28, 1),
        (
            true,
            0,
            5,
            false,
            true,
            false,
            false,
            3,
            55 + keys,
            3,
            57 + keys,
            1,
        ),
        (true, 1, 6, false, true, false, false, 3, 26, 3, 26, 1),
        // Scalar/marker sibling: same rejection as a source and same disjoint
        // result, but the new map/second-role checks perform additional work.
        (false, 2, 4, false, false, true, true, 0, 2, 2, 28, 0),
        // Wrong retained field type is rejected even in the extended domain.
        (false, 2, 6, false, false, false, false, 2, 26, 2, 2, 1),
    ] {
        for moved in [false, true] {
            let a = getter(outer, field, reference, moved);
            let (mut aw, mut bw) = (budget(MAX_FLOW_WORK), budget(MAX_FLOW_WORK));
            let ap = old.source(&a, &mut aw).unwrap();
            let bp = new.source(&a, &mut bw).unwrap();
            assert_eq!(
                ap.is_some(),
                old_source,
                "cold source outer={outer} field={field} reference={reference} moved={moved}"
            );
            assert_eq!(
                bp.is_some(),
                new_source,
                "new source outer={outer} field={field} reference={reference} moved={moved}"
            );
            let SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            ) = a.value().kind()
            else {
                unreachable!()
            };
            for selected in [ap, bp].into_iter().flatten() {
                assert!(std::ptr::eq(selected, place));
            }
            assert_eq!(
                MAX_FLOW_WORK - aw.remaining,
                old_cost + usize::from(moved) * move_cost,
                "cold source work outer={outer} field={field} reference={reference} moved={moved}"
            );
            assert_eq!(
                MAX_FLOW_WORK - bw.remaining,
                new_cost + usize::from(moved) * move_cost,
                "new source work outer={outer} field={field} reference={reference} moved={moved}"
            );
            let (mut aw, mut bw) = (budget(MAX_FLOW_WORK), budget(MAX_FLOW_WORK));
            assert_eq!(old.disjoint_field_use(&a, &mut aw).unwrap(), old_disjoint);
            assert_eq!(new.disjoint_field_use(&a, &mut bw).unwrap(), new_disjoint);
            assert_eq!(
                MAX_FLOW_WORK - aw.remaining,
                old_d,
                "cold disjoint work outer={outer} field={field} reference={reference}"
            );
            assert_eq!(
                MAX_FLOW_WORK - bw.remaining,
                new_d,
                "new disjoint work outer={outer} field={field} reference={reference}"
            );
        }
    }
}

#[test]
fn original_borrow_extension_has_an_independent_exact_work_boundary() {
    let source = fixture::source(Mutation::None);
    let body = &source.functions()[0];
    let primary = Pairs::from([(ty(5), ty(2))]);
    let wg = Pairs::from([(ty(6), ty(3))]);
    let old = cold(
        body,
        source.types(),
        &primary,
        &Pairs::new(),
        &BTreeSet::new(),
        &wg,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let new = Routes::from_pairs_with_workgroups(
        body,
        source.types(),
        &primary,
        &Pairs::new(),
        &BTreeSet::new(),
        &wg,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let expected = assert_maps(&old, &new, &wg);
    assert_eq!((expected.roles, expected.shared8), (2, true));
    assert_root_inventory(body);
    let SemanticStatementKindV1::Assign(a) = body.blocks()[4].statements()[0].kind() else {
        panic!()
    };
    let place = assert_original_borrow(a);
    // Independently calculated above: 2 +21 +1 +3 +21 =48, not a debit
    // measured from a successful production invocation and reused as an oracle.
    for remaining in [47, 48] {
        let limit = remaining + 17;
        let mut work = budget(limit);
        work.charge(17).unwrap();
        let result = new.source(a, &mut work);
        if remaining == 48 {
            assert!(std::ptr::eq(result.unwrap().unwrap(), place));
            assert_eq!(work.remaining, 0);
        } else {
            let Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
                remaining_work_units,
                requested_work_units,
                error,
                ..
            }) = result
            else {
                panic!("expected exact work rejection without a source result")
            };
            assert_eq!((remaining_work_units, requested_work_units), (20, 21));
            assert_eq!(work.remaining, 20);
            assert_eq!(
                *error,
                ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    required: limit + 1,
                    limit,
                }
            );
        }
        let mut cold_work = budget(limit);
        cold_work.charge(17).unwrap();
        assert!(old.source(a, &mut cold_work).unwrap().is_none());
        assert_eq!(
            cold_work.remaining, remaining,
            "legacy rejection did not perform the new query"
        );
    }
}
