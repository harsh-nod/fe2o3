use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "../policy_carrier_v1/fixture.rs"]
mod fixture;
use fixture::{Mutation, ty};
include!("cold_oracle.rs");

#[path = "role_tests.rs"]
mod role_tests;

#[path = "role_oracle83.rs"]
mod role_oracle83;

type Pairs = BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>;

fn budget(limit: usize) -> Budget {
    Budget {
        limit,
        remaining: limit,
        profile: FlowWorkProfile::default(),
    }
}

fn cold<'a>(
    body: &'a SemanticFunctionDeclV1,
    types: &'a [SemanticTypeDeclV1],
    primary: &Pairs,
    policy: &Pairs,
    barriers: &BTreeSet<SemanticTypeIdV1>,
    wg: &Pairs,
    budget: &mut Budget,
) -> Result<Routes<'a>, ProductionSemanticSsaErrorV1> {
    let mut routes = cold_primary(body, types, primary, policy, barriers, budget)?;
    cold_workgroup(&mut routes, wg, barriers, budget)?;
    cold_shared(&mut routes, wg, budget)?;
    Ok(routes)
}

fn same(a: &Routes<'_>, b: &Routes<'_>) {
    assert!(std::ptr::eq(a.function, b.function));
    assert!(std::ptr::eq(a.types, b.types));
    assert_eq!(a.routes, b.routes);
    assert_eq!(a.secondaries, b.secondaries);
    let snapshot = |routes: &Routes<'_>| {
        routes
            .shared
            .iter()
            .map(|(&ty, c)| (ty, c.pointee, c.route))
            .collect::<Vec<_>>()
    };
    assert_eq!(snapshot(a), snapshot(b));
}

fn locals(
    body: &SemanticFunctionDeclV1,
    extra: impl IntoIterator<Item = SemanticTypeIdV1>,
) -> SemanticFunctionDeclV1 {
    let mut locals = body.locals().to_vec();
    for ty in extra {
        let index = u32::try_from(locals.len()).unwrap();
        let mut identity = [0xa5; 32];
        identity[..4].copy_from_slice(&index.to_le_bytes());
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity),
            ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let result = SemanticFunctionDeclV1::new(
        body.identity(),
        body.role(),
        body.item_definition_identity(),
        body.monomorphization_identity(),
        body.generic_type_arguments_identity(),
        body.const_generic_arguments_identity(),
        body.source(),
        body.abi().clone(),
        locals,
        body.entry(),
        body.blocks().to_vec(),
    )
    .unwrap();
    match body.kernel_entry() {
        Some(entry) => result.with_kernel_entry(entry.clone()),
        None => result,
    }
}

fn pointer(
    types: &mut Vec<SemanticTypeDeclV1>,
    pointee: u32,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    space: u32,
    bits: u16,
    metadata: SemanticPointerMetadataV1,
) -> SemanticTypeIdV1 {
    let id = u32::try_from(types.len()).unwrap();
    let mut identity = [0xb5; 32];
    identity[..4].copy_from_slice(&id.to_le_bytes());
    // Raw classifier inputs only. These declarations do not mint an admitted
    // source owner or a Workgroup issuer, and negative layouts are not admitted.
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity),
        SemanticLayoutIdentityV1::from_sha256(identity),
        types[5].layout().clone(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(pointee),
                kind,
                mutability,
                space,
                bits,
                metadata,
            )
            .unwrap(),
        ),
    ));
    ty(id)
}

fn aggregate(
    types: &mut Vec<SemanticTypeDeclV1>,
    fields: Vec<SemanticTypeIdV1>,
) -> SemanticTypeIdV1 {
    let id = u32::try_from(types.len()).unwrap();
    let mut identity = [0xc5; 32];
    identity[..4].copy_from_slice(&id.to_le_bytes());
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity),
        SemanticLayoutIdentityV1::from_sha256(identity),
        types[13].layout().clone(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    ));
    ty(id)
}

#[test]
fn cold_domain_and_exact_role_delta_for_all_priority_tiers_barriers_and_mutations() {
    let mut cases = 0;
    let mut extended = 0;
    let mut changed_borrows = 0;
    for mutation in [
        Mutation::None,
        Mutation::DuplicateCapture,
        Mutation::DeinitializeCapture,
        Mutation::MoveSubcarrier,
    ] {
        let source = fixture::source(mutation);
        for primary in [
            Pairs::new(),
            Pairs::from([(ty(5), ty(2))]),
            Pairs::from([(ty(5), ty(2)), (ty(6), ty(3))]),
        ] {
            for policy in [Pairs::new(), Pairs::from([(ty(6), ty(3))])] {
                for wg in [
                    Pairs::new(),
                    Pairs::from([(ty(5), ty(2))]),
                    Pairs::from([(ty(6), ty(3))]),
                    Pairs::from([(ty(5), ty(2)), (ty(6), ty(3))]),
                ] {
                    for barriers in [
                        BTreeSet::new(),
                        BTreeSet::from([ty(7), ty(9)]),
                        BTreeSet::from([ty(14)]),
                    ] {
                        let body = &source.functions()[0];
                        let a = cold(
                            body,
                            source.types(),
                            &primary,
                            &policy,
                            &barriers,
                            &wg,
                            &mut budget(MAX_FLOW_WORK),
                        )
                        .unwrap();
                        let b = Routes::from_pairs_with_workgroups(
                            body,
                            source.types(),
                            &primary,
                            &policy,
                            &barriers,
                            &wg,
                            &mut budget(MAX_FLOW_WORK),
                        )
                        .unwrap();
                        let expected = role_oracle83::assert_maps_with_policy(
                            &a, &b, &primary, &policy, &wg,
                        );
                        cases += 1;
                        extended += usize::from(expected.has_roles());
                        let mut changed_here = 0;
                        role_oracle83::assert_root_inventory(body);
                        for (block, body) in body.blocks().iter().enumerate() {
                            for (statement, source) in body.statements().iter().enumerate() {
                                if let SemanticStatementKindV1::Assign(assignment) = source.kind() {
                                    changed_here += usize::from(expected.assert_root_query(
                                        &a, &b, (block, statement), assignment));
                                }
                            }
                        }
                        assert_eq!(changed_here, usize::from(expected.new_shared_borrow()));
                        changed_borrows += changed_here;
                    }
                }
            }
        }
    }
    assert_eq!((cases, extended, changed_borrows), (288, 72, 16));
}

#[test]
fn canonical_type_order_preserves_outer_reference_priority_and_pointer_rejections() {
    let source = fixture::source(Mutation::None);
    let mut types = source.types().to_vec();
    let valid = pointer(
        &mut types,
        13,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::None,
    );
    let nested = pointer(
        &mut types,
        18,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::None,
    );
    let ambiguous = pointer(
        &mut types,
        15,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::None,
    );
    let raw = pointer(
        &mut types,
        13,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::None,
    );
    let mutable = pointer(
        &mut types,
        13,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
        0,
        64,
        SemanticPointerMetadataV1::None,
    );
    let space = pointer(
        &mut types,
        13,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        1,
        64,
        SemanticPointerMetadataV1::None,
    );
    let narrow = pointer(
        &mut types,
        13,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        32,
        SemanticPointerMetadataV1::None,
    );
    let thick = pointer(
        &mut types,
        13,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::SliceLength,
    );
    let twice = pointer(
        &mut types,
        valid.index(),
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::None,
    );
    let order = [
        valid, nested, ambiguous, raw, mutable, space, narrow, thick, twice,
    ];
    let wg = Pairs::from([(ty(6), ty(3))]);
    for order in [
        order.to_vec(),
        order.into_iter().rev().collect(),
        order.repeat(4),
    ] {
        let body = locals(&source.functions()[0], order);
        for primary in [
            Pairs::new(),
            Pairs::from([(ty(5), ty(2))]),
            Pairs::from([(valid, ty(3))]),
        ] {
            let a = cold(
                &body,
                &types,
                &primary,
                &Pairs::new(),
                &BTreeSet::new(),
                &wg,
                &mut budget(MAX_FLOW_WORK),
            )
            .unwrap();
            let b = Routes::from_pairs_with_workgroups(
                &body,
                &types,
                &primary,
                &Pairs::new(),
                &BTreeSet::new(),
                &wg,
                &mut budget(MAX_FLOW_WORK),
            )
            .unwrap();
            role_oracle83::assert_maps(&a, &b, &wg);
            if primary.contains_key(&valid) {
                assert!(b.routes.contains_key(&valid));
                assert!(!b.shared.contains_key(&valid));
            } else {
                assert!(b.shared.contains_key(&valid));
            }
            assert!(b.shared.contains_key(&nested));
            for invalid in [ambiguous, raw, mutable, space, narrow, thick, twice] {
                assert!(
                    !b.shared.contains_key(&invalid),
                    "invalid outer reference {invalid:?}"
                );
            }
        }
    }
}

#[test]
fn no_workgroup_or_policy_seed_preserves_every_old_debit_and_failure() {
    let source = fixture::source(Mutation::None);
    let body = locals(&source.functions()[0], [ty(5); 64]);
    for primary in [Pairs::new(), Pairs::from([(ty(5), ty(2))])] {
        let policy = Pairs::new();
        for limit in 0..512 {
            let (mut aw, mut bw) = (budget(limit), budget(limit));
            let a = cold(
                &body,
                source.types(),
                &primary,
                &policy,
                &BTreeSet::new(),
                &Pairs::new(),
                &mut aw,
            );
            let b = Routes::from_pairs_with_workgroups(
                &body,
                source.types(),
                &primary,
                &policy,
                &BTreeSet::new(),
                &Pairs::new(),
                &mut bw,
            );
            assert_eq!(aw.remaining, bw.remaining);
            match (a, b) {
                (Ok(a), Ok(b)) => same(&a, &b),
                (Err(a), Err(b)) => assert_eq!(format!("{a:?}"), format!("{b:?}")),
                _ => panic!("empty Workgroup/Policy path changed its work boundary"),
            }
        }
    }
}

#[test]
fn no_workgroup_seed_checks_policy_state_and_exact_added_work() {
    let source = fixture::source(Mutation::None);
    role_oracle83::assert_root_inventory(&source.functions()[0]);
    let body = locals(&source.functions()[0], [ty(5); 64]);
    let policy = Pairs::from([(ty(6), ty(3))]);
    assert_eq!(body.locals().len(), 86);
    assert_eq!(
        body.locals().iter().map(|local| local.ty()).collect::<BTreeSet<_>>(),
        (0..19).map(ty).collect::<BTreeSet<_>>()
    );
    // Census: path16 + locals86 + distinct types19. Policy-only visits 19 roots
    // and 22 children (41 nodes/lookups). Its 10 Policy observations include
    // two each in types15/17; only types6/7/13/14/16/18 retain unique routes.
    // Mixed stops type16 at its second primary: 40 nodes, minus six primary
    // visits and four post-primary nodes =30 Policy lookups. Observations in
    // types6/13/15/17/18 total7; types5/6/7/13/14/17/18 retain seven routes.
    // Each lookup costs2; each observation and retained route costs20.
    let census = 16 + 86 + 19;
    let policy_only_work = census + 41 + 2 * 41 + 20 * (10 + 6); // 564
    let mixed_work = census + 40 + 2 * 30 + 20 * (7 + 7); // 501
    for (primary, old_required, policy_delta) in [
        (Pairs::new(), policy_only_work, 0),
        (Pairs::from([(ty(5), ty(2))]), mixed_work, 211),
    ] {
        let (mut aw, mut bw) = (budget(MAX_FLOW_WORK), budget(MAX_FLOW_WORK));
        let old = cold(
            &body, source.types(), &primary, &policy, &BTreeSet::new(),
            &Pairs::new(), &mut aw,
        ).unwrap();
        let new = Routes::from_pairs_with_workgroups(
            &body, source.types(), &primary, &policy, &BTreeSet::new(),
            &Pairs::new(), &mut bw,
        ).unwrap();
        let expected = role_oracle83::assert_maps_with_policy(
            &old, &new, &primary, &policy, &Pairs::new(),
        );
        assert_eq!(expected.has_roles(), !primary.is_empty());
        assert!(!expected.new_shared_borrow(), "Policy cannot mint a shared wrapper");
        // Mixed type7 adds three lookups and one copy; type14 adds one lookup
        // and one copy: 4*2 + 2*20 =48. Admission/storage is 33+3+43 and 34+44
        // for the first and second inline entries: another157. Type17's known
        // ambiguous Policy adds header3 + lookup1 + entry2: total211.
        assert_eq!(new.failed_secondaries,
            if primary.is_empty() { BTreeSet::new() } else { BTreeSet::from([ty(17)]) });
        let new_required = old_required + policy_delta;
        assert_eq!(MAX_FLOW_WORK - aw.remaining, old_required, "frozen Policy debit");
        assert_eq!(MAX_FLOW_WORK - bw.remaining, new_required, "explicit Policy delta");
        for limit in 0..=new_required + 1 {
            let (mut aw, mut bw) = (budget(limit), budget(limit));
            let a = cold(
                &body, source.types(), &primary, &policy, &BTreeSet::new(),
                &Pairs::new(), &mut aw,
            );
            let b = Routes::from_pairs_with_workgroups(
                &body, source.types(), &primary, &policy, &BTreeSet::new(),
                &Pairs::new(), &mut bw,
            );
            if policy_delta == 0 {
                assert_eq!(aw.remaining, bw.remaining);
                if let (Err(a), Err(b)) = (&a, &b) {
                    assert_eq!(format!("{a:?}"), format!("{b:?}"));
                }
            }
            for (result, work, required, expected) in [
                (a, &aw, old_required, &old),
                (b, &bw, new_required, &new),
            ] {
                if limit >= required {
                    let actual = result.unwrap();
                    same(expected, &actual);
                    assert_eq!(expected.failed_secondaries, actual.failed_secondaries);
                    assert_eq!(work.remaining, limit - required);
                } else {
                    let Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
                        stage, phase_work_units, ordered_proof_calls,
                        remaining_work_units, requested_work_units, error,
                    }) = result else {
                        panic!("expected Policy work rejection without partial routes");
                    };
                    assert_eq!(stage, "facts");
                    assert_eq!(ordered_proof_calls, 0);
                    assert_eq!(remaining_work_units, work.remaining);
                    assert!(requested_work_units > remaining_work_units);
                    let mut phases = [0; 11];
                    phases[0] = limit - remaining_work_units;
                    assert_eq!(phase_work_units, phases);
                    assert_eq!(
                        *error,
                        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                            resource: SsaPlannerResourceV1::WorkUnits,
                            required: limit + 1,
                            limit,
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn repeated_local_types_remove_real_scans_on_primary_and_workgroup_only_paths() {
    let source = fixture::source(Mutation::None);
    let wg = Pairs::from([(ty(6), ty(3))]);
    for primary in [Pairs::new(), Pairs::from([(ty(5), ty(2))])] {
        let body = locals(
            &source.functions()[0],
            (0..4096).map(|i| [ty(6), ty(13), ty(18)][i % 3]),
        );
        let (mut aw, mut bw) = (budget(MAX_FLOW_WORK), budget(MAX_FLOW_WORK));
        let a = cold(
            &body,
            source.types(),
            &primary,
            &Pairs::new(),
            &BTreeSet::new(),
            &wg,
            &mut aw,
        )
        .unwrap();
        let b = Routes::from_pairs_with_workgroups(
            &body,
            source.types(),
            &primary,
            &Pairs::new(),
            &BTreeSet::new(),
            &wg,
            &mut bw,
        )
        .unwrap();
        role_oracle83::assert_maps(&a, &b, &wg);
        let (old, new) = (MAX_FLOW_WORK - aw.remaining, MAX_FLOW_WORK - bw.remaining);
        assert!(
            new * 4 < old * 3,
            "logical work old={old} new={new}, primary={}",
            !primary.is_empty()
        );
        if !primary.is_empty() {
            let shorter = locals(
                &source.functions()[0],
                (0..2048).map(|i| [ty(6), ty(13), ty(18)][i % 3]),
            );
            let mut short_work = budget(MAX_FLOW_WORK);
            Routes::from_pairs_with_workgroups(
                &shorter,
                source.types(),
                &primary,
                &Pairs::new(),
                &BTreeSet::new(),
                &wg,
                &mut short_work,
            )
            .unwrap();
            assert_eq!(
                new - (MAX_FLOW_WORK - short_work.remaining),
                2048,
                "only the existing primary per-local census grows with repeated types"
            );
        }
        eprintln!(
            "workgroup-type-reuse locals={} primary={} old_work={old} new_work={new}",
            body.locals().len(),
            !primary.is_empty()
        );
    }
}

#[test]
fn shared_budget_exact_and_one_less_never_publish_partial_routes_or_refund_work() {
    let source = fixture::source(Mutation::None);
    let primary = Pairs::from([(ty(5), ty(2))]);
    let wg = Pairs::from([(ty(6), ty(3))]);
    let body = locals(&source.functions()[0], [ty(6); 128]);
    let mut measured = budget(MAX_FLOW_WORK);
    let expected = Routes::from_pairs_with_workgroups(
        &body,
        source.types(),
        &primary,
        &Pairs::new(),
        &BTreeSet::new(),
        &wg,
        &mut measured,
    )
    .unwrap();
    let required = MAX_FLOW_WORK - measured.remaining;
    assert!(required > MAX_FIELDS);
    for remaining in [0, 1, required - 1, required] {
        let limit = remaining + 17;
        let mut work = budget(limit);
        work.charge(17).unwrap();
        assert_eq!(work.remaining, remaining);
        let result = Routes::from_pairs_with_workgroups(
            &body,
            source.types(),
            &primary,
            &Pairs::new(),
            &BTreeSet::new(),
            &wg,
            &mut work,
        );
        assert_eq!(work.limit, limit);
        if remaining == required {
            same(&expected, &result.unwrap());
            assert_eq!(work.remaining, 0);
        } else {
            let Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
                stage,
                phase_work_units,
                ordered_proof_calls,
                remaining_work_units,
                requested_work_units,
                error,
            }) = result else {
                panic!("expected exact shared work-budget rejection, not a partial result");
            };
            assert_eq!(stage, "facts");
            assert_eq!(ordered_proof_calls, 0);
            assert_eq!(remaining_work_units, work.remaining);
            assert!(work.remaining <= remaining);
            assert!(requested_work_units > remaining_work_units);
            let mut expected_phases = [0; 11];
            expected_phases[0] = limit - remaining_work_units;
            assert_eq!(phase_work_units, expected_phases, "includes the pre-spent17 units");
            assert_eq!(
                *error,
                ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    required: limit + 1,
                    limit,
                }
            );
            if remaining < MAX_FIELDS {
                assert_eq!(remaining_work_units, remaining);
                assert_eq!(requested_work_units, MAX_FIELDS);
                assert_eq!(phase_work_units[0], 17);
            } else {
                // With one unit missing, the unchanged final positive debit
                // is denied without consuming the leftover allowance.
                assert_eq!(remaining, required - 1);
                assert_eq!(requested_work_units, remaining_work_units + 1);
            }
        }
    }
}

#[test]
fn bounds_cycles_and_nonlocal_shape_nodes_keep_the_cold_walk_result() {
    let source = fixture::source(Mutation::None);
    let mut types = source.types().to_vec();
    let mut nested = ty(6);
    let mut roots = Vec::new();
    for _ in 0..=MAX_FIELDS {
        nested = aggregate(&mut types, vec![nested]);
        roots.push(nested);
    }
    let mut exact_fields = vec![ty(4); MAX_SHAPE_NODES - 1];
    exact_fields[0] = ty(6);
    let exact_nodes = aggregate(&mut types, exact_fields.clone());
    exact_fields.push(ty(4));
    let wide = aggregate(&mut types, exact_fields);
    let cycle_id = ty(types.len() as u32);
    let cycle = aggregate(&mut types, vec![cycle_id]);
    assert_eq!(cycle, cycle_id);
    let body = locals(
        &source.functions()[0],
        [
            roots[MAX_FIELDS - 1],
            roots[MAX_FIELDS],
            exact_nodes,
            wide,
            cycle,
            ty(u32::MAX),
        ],
    );
    let wg = Pairs::from([(ty(6), ty(3))]);
    let a = cold(
        &body,
        &types,
        &Pairs::new(),
        &Pairs::new(),
        &BTreeSet::new(),
        &wg,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let b = Routes::from_pairs_with_workgroups(
        &body,
        &types,
        &Pairs::new(),
        &Pairs::new(),
        &BTreeSet::new(),
        &wg,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    same(&a, &b);
    assert!(b.routes.contains_key(&roots[MAX_FIELDS - 1]));
    assert!(b.routes.contains_key(&exact_nodes));
    for rejected in [roots[MAX_FIELDS], wide, cycle, ty(u32::MAX)] {
        assert!(!b.routes.contains_key(&rejected));
    }
    assert!(
        !b.routes.contains_key(&roots[0]),
        "nested type without a local does not become a new route root"
    );
    assert_eq!(MAX_FIELDS, 16);
    assert_eq!(MAX_SHAPE_NODES, 256);
    assert_eq!(MAX_FLOW_WORK, 262_144);
}
