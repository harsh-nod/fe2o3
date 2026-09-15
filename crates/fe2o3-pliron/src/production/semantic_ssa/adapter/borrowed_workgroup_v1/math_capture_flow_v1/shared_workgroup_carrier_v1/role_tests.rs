//! Classifier and common-graph tests. The pair maps below are inert test inputs,
//! not authenticated issuers. Existing actual BF16 owner/loan callbacks stay exact.
use super::*;

#[test]
fn replayed_non_workgroup_roster_does_not_seed_a_workgroup_role() {
    let source = fixture::source(Mutation::None);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    expansion.verify_replay(&source).unwrap();
    let view = &expansion.roots()[0];
    let matrix = MatrixBorrowSitesV1::new(
        &source,
        &expansion,
        view,
        &expansion.defined_capability_bindings(&source).unwrap(),
        MAX_FLOW_WORK,
    )
    .unwrap();
    assert!(!matrix.policy_carrier_leaves().is_empty());
    let routes = Routes::new_with_matrix(
        view.body(),
        Some(source.types()),
        source.callables(),
        &matrix,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert!(
        routes.secondaries.values().all(|roles| roles[1].is_none()),
        "other checked roles are not Workgroup issuers"
    );
}

fn place(local: u32, ty_: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty_).unwrap()
}

fn assignment(
    local: u32,
    ty_: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticAssignmentV1 {
    SemanticAssignmentV1::new(place(local, ty_), SemanticRvalueV1::new(ty_, value))
}

fn fixture() -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    SemanticTypeIdV1,
    u32,
) {
    let source = fixture::source(Mutation::None);
    let mut types = source.types().to_vec();
    let outer = pointer(
        &mut types,
        14,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::None,
    );
    let start = source.functions()[0].locals().len() as u32;
    let body = locals(
        &source.functions()[0],
        [ty(14), outer, ty(5), ty(6), ty(5), ty(6)],
    );
    (types, body, outer, start)
}

fn routes<'a>(types: &'a [SemanticTypeDeclV1], body: &'a SemanticFunctionDeclV1) -> Routes<'a> {
    Routes::from_pairs_with_workgroups(
        body,
        types,
        &Pairs::from([(ty(5), ty(2))]),
        &Pairs::new(),
        &BTreeSet::new(),
        &Pairs::from([(ty(6), ty(3))]),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap()
}

fn capture(start: u32) -> SemanticAssignmentV1 {
    assignment(
        start,
        ty(14),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Aggregate,
            vec![
                SemanticOperandV1::Copy(place(start + 2, ty(5))),
                SemanticOperandV1::Move(place(start + 3, ty(6))),
            ],
        )
        .unwrap(),
    )
}

fn projected(
    start: u32,
    field: u32,
    result: SemanticTypeIdV1,
    deref: SemanticTypeIdV1,
) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(start + 1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, deref).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), result).unwrap(),
        ],
        result,
    )
    .unwrap()
}

#[test]
fn primary_is_preserved_and_both_exact_fields_have_original_operand_edges() {
    let (types, body, outer, start) = fixture();
    let routes = routes(&types, &body);
    let primary = routes.routes[&ty(14)];
    assert_eq!(
        (
            primary.reference,
            primary.owned,
            &primary.fields[..primary.len]
        ),
        (ty(5), ty(2), &[0][..])
    );
    let (role, wg) = routes.secondaries[&ty(14)][1].unwrap();
    assert_eq!(role, SecondaryRole::Workgroup);
    assert_eq!(
        (wg.reference, wg.owned, &wg.fields[..wg.len]),
        (ty(6), ty(3), &[1][..])
    );
    assert_eq!(
        routes
            .shared_owned(outer, &mut budget(MAX_FLOW_WORK))
            .unwrap(),
        Some(ty(3))
    );
    let borrow = assignment(
        start + 1,
        outer,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(start, ty(14)),
        },
    );
    let SemanticRvalueKindV1::Borrow { place, .. } = borrow.value().kind() else {
        unreachable!()
    };
    assert!(std::ptr::eq(
        routes
            .source(&borrow, &mut budget(MAX_FLOW_WORK))
            .unwrap()
            .unwrap(),
        place
    ));
    for (field, reference, destination) in [(0, ty(5), start + 4), (1, ty(6), start + 5)] {
        for moved in [false, true] {
            let p = projected(start, field, reference, ty(14));
            let operand = if moved {
                SemanticOperandV1::Move(p)
            } else {
                SemanticOperandV1::Copy(p)
            };
            let a = assignment(destination, reference, SemanticRvalueKindV1::Use(operand));
            assert!(
                routes
                    .source(&a, &mut budget(MAX_FLOW_WORK))
                    .unwrap()
                    .is_some()
            );
            assert!(
                !routes
                    .disjoint_field_use(&a, &mut budget(MAX_FLOW_WORK))
                    .unwrap(),
                "neither checked capability leaf may be hidden as a disjoint sibling"
            );
        }
    }
}

#[test]
fn role_projection_rejects_wrong_deref_field_type_and_bounds() {
    let (types, body, _, start) = fixture();
    let routes = routes(&types, &body);
    for (field, reference, deref) in [
        (0, ty(6), ty(14)),
        (1, ty(5), ty(14)),
        (2, ty(6), ty(14)),
        (1, ty(6), ty(13)),
    ] {
        let a = assignment(
            start + 5,
            reference,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                start, field, reference, deref,
            ))),
        );
        assert!(
            routes
                .source(&a, &mut budget(MAX_FLOW_WORK))
                .unwrap()
                .is_none()
        );
        assert!(
            !routes
                .disjoint_field_use(&a, &mut budget(MAX_FLOW_WORK))
                .unwrap()
        );
    }
    assert!(
        routes.secondaries.get(&ty(17)).is_none_or(|roles| roles[1].is_none()),
        "two Workgroup leaves stay ambiguous"
    );
}

#[test]
fn checked_role_order_is_not_a_fixed_field_number() {
    let (types, body, outer, _) = fixture();
    let routes = Routes::from_pairs_with_workgroups(
        &body,
        &types,
        &Pairs::from([(ty(6), ty(3))]),
        &Pairs::new(),
        &BTreeSet::new(),
        &Pairs::from([(ty(5), ty(2))]),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert_eq!(routes.routes[&ty(14)].fields[0], 1);
    let (role, wg) = routes.secondaries[&ty(14)][1].unwrap();
    assert_eq!(role, SecondaryRole::Workgroup);
    assert_eq!(wg.fields[0], 0);
    assert_eq!(
        routes
            .shared_owned(outer, &mut budget(MAX_FLOW_WORK))
            .unwrap(),
        Some(ty(2))
    );
}

#[test]
fn the_secondary_role_cannot_join_a_mutable_raw_or_noncanonical_primary_reference() {
    let (types, body, _, _) = fixture();
    for (kind, mutability, space, bits, metadata) in [
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
        ),
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            1,
            64,
            SemanticPointerMetadataV1::None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            32,
            SemanticPointerMetadataV1::None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::SliceLength,
        ),
    ] {
        let mut types = types.clone();
        let old = &types[5];
        types[5] = SemanticTypeDeclV1::new(
            old.identity(),
            old.layout_identity(),
            old.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(2),
                    kind,
                    mutability,
                    space,
                    bits,
                    metadata,
                )
                .unwrap(),
            ),
        );
        let routes = routes(&types, &body);
        assert_eq!(
            routes.routes[&ty(14)].reference,
            ty(5),
            "primary selection is unchanged"
        );
        assert!(routes.secondaries.get(&ty(14)).is_none_or(|roles| roles[1].is_none()));
    }
}

fn candidate(
    site: u32,
    source: u32,
    owned: u32,
    parent: Option<u32>,
    alias: bool,
) -> SemanticBorrowCandidateV1 {
    SemanticBorrowCandidateV1 {
        site: SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement: site,
        },
        source_local: source,
        source_type: ty(owned),
        source_reference: parent,
        value_alias: alias, source_kind: SemanticBorrowCandidateSourceV1::Direct,
        valid: true,
        consumers: 0,
        intrinsic_consumer: false,
    }
}

fn nodes(
    start: u32,
) -> (
    Vec<SemanticBorrowCandidateV1>,
    BTreeMap<u32, usize>,
    Vec<Vec<usize>>,
) {
    (
        vec![
            candidate(0, 200, 2, None, false),
            candidate(1, 201, 3, None, false),
            candidate(2, start + 2, 2, Some(start + 2), true),
            candidate(3, start, 3, Some(start), false),
            candidate(4, start + 1, 2, Some(start + 1), true),
            candidate(5, start + 1, 3, Some(start + 1), true),
        ],
        BTreeMap::from([
            (start + 2, 0),
            (start + 3, 1),
            (start, 2),
            (start + 1, 3),
            (start + 4, 4),
            (start + 5, 5),
        ]),
        vec![vec![2], vec![], vec![3], vec![4, 5], vec![], vec![]],
    )
}

fn run(
    failed: Option<usize>,
    rejected_root: Option<usize>,
    work: &mut Budget,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    let (types, body, _, start) = fixture();
    let routes = routes(&types, &body);
    let a = capture(start);
    let (mut candidates, by_reference, mut children) = nodes(start);
    let mut joins = workgroup_role_joins_v1::Joins::default();
    invalidate_reference_uses_in_statement_v1(
        &SemanticStatementKindV1::Assign(a.clone()),
        candidates[2].site,
        &by_reference,
        &mut candidates,
    );
    let checked_secondary_fields = joins.connect(&routes, &a, 2, &by_reference, &mut candidates, work)?;
    assert_eq!(checked_secondary_fields.into_iter().flatten().collect::<Vec<_>>(),
        vec![(SecondaryRole::Workgroup, 1)]);
    routes.invalidate_siblings_checked(&a, &by_reference, &mut candidates, checked_secondary_fields, work)?;
    assert!(candidates.iter().all(|c| c.valid));
    joins.start(&mut children, work)?;
    if let Some(failed) = failed {
        candidates[failed].valid = false;
    }
    let mut accepted = BTreeSet::new();
    for root in 0..2 {
        // Models a root whose separate ordered/lifetime proof failed, although
        // the syntactic candidate component itself is otherwise valid.
        if rejected_root == Some(root) {
            continue;
        }
        if let Ok(visited) =
            borrow_components_v1::members(&candidates, &children, root, work, |_| true)?
        {
            joins.record(&visited, work)?;
            accepted.extend(
                visited
                    .into_iter()
                    .filter(|&i| !candidates[i].value_alias)
                    .map(|i| candidates[i].site),
            );
        }
    }
    joins.retain(&candidates, &children, &[], &mut accepted, work)?;
    Ok(accepted)
}

#[test]
fn either_failed_root_or_descendant_rejects_both_roles_and_the_shared_borrow() {
    assert_eq!(
        run(None, None, &mut budget(MAX_FLOW_WORK)).unwrap(),
        [0, 1, 3]
            .map(|statement| SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement
            })
            .into_iter()
            .collect()
    );
    for failed in 0..6 {
        assert!(
            run(Some(failed), None, &mut budget(MAX_FLOW_WORK))
                .unwrap()
                .is_empty(),
            "failed node {failed}"
        );
    }
    for root in 0..2 {
        assert!(
            run(None, Some(root), &mut budget(MAX_FLOW_WORK))
                .unwrap()
                .is_empty(),
            "a separately rejected owner proof must veto the shared component"
        );
    }
}

#[test]
fn absent_or_self_parent_is_not_a_checked_second_input() {
    let (types, body, _, start) = fixture();
    let routes = routes(&types, &body);
    for self_parent in [false, true] {
        let (mut candidates, mut by_reference, _) = nodes(start);
        if self_parent {
            by_reference.insert(start + 3, 2);
        } else {
            by_reference.remove(&(start + 3));
        }
        let mut joins = workgroup_role_joins_v1::Joins::default();
        assert_eq!(
            joins
                .connect(
                    &routes,
                    &capture(start),
                    2,
                    &by_reference,
                    &mut candidates,
                    &mut budget(MAX_FLOW_WORK)
                )
                .unwrap(),
            [None; 2]
        );
        assert!(!candidates[2].valid);
    }
}

#[test]
fn a_third_tracked_input_is_rejected_instead_of_ignored() {
    let (mut types, body, _, start) = fixture();
    let third = pointer(
        &mut types,
        8,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::None,
    );
    let triple = aggregate(&mut types, vec![ty(5), ty(6), third]);
    let triple_local = body.locals().len() as u32;
    let body = locals(&body, [triple, third]);
    let routes = routes(&types, &body);
    let a = assignment(
        triple_local,
        triple,
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Aggregate,
            vec![
                SemanticOperandV1::Copy(place(start + 2, ty(5))),
                SemanticOperandV1::Move(place(start + 3, ty(6))),
                SemanticOperandV1::Copy(place(triple_local + 1, third)),
            ],
        )
        .unwrap(),
    );
    let (mut candidates, mut by_reference, _) = nodes(start);
    candidates[2].source_reference = Some(start + 2);
    by_reference.insert(triple_local, 2);
    by_reference.insert(triple_local + 1, candidates.len());
    candidates.push(candidate(6, 202, 8, None, false));
    let mut joins = workgroup_role_joins_v1::Joins::default();
    let checked_secondary_fields = joins
        .connect(
            &routes,
            &a,
            2,
            &by_reference,
            &mut candidates,
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
    assert_eq!(checked_secondary_fields.into_iter().flatten().collect::<Vec<_>>(),
        vec![(SecondaryRole::Workgroup, 1)]);
    routes
        .invalidate_siblings_checked(
            &a,
            &by_reference,
            &mut candidates,
            checked_secondary_fields,
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
    assert!(!candidates[2].valid, "the joined carrier must reject");
    assert!(
        !candidates[6].valid,
        "the third input's old escape rejection must remain"
    );
}

fn check_join_budget(
    run: impl Fn(&mut Budget) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1>,
) {
    let mut measured = budget(MAX_FLOW_WORK);
    let expected = run(&mut measured).unwrap();
    let required = MAX_FLOW_WORK - measured.remaining;
    for remaining in [0, 1, required - 1, required] {
        let limit = remaining + 17;
        let mut work = budget(limit);
        work.charge(17).unwrap();
        match run(&mut work) {
            Ok(actual) => {
                assert_eq!(remaining, required);
                assert_eq!(work.remaining, 0);
                assert_eq!(actual, expected);
            }
            Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
                remaining_work_units,
                requested_work_units,
                error,
                ..
            }) => {
                assert!(remaining < required);
                assert_eq!(remaining_work_units, work.remaining);
                assert!(requested_work_units > remaining_work_units);
                assert_eq!(
                    *error,
                    ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                        resource: SsaPlannerResourceV1::WorkUnits,
                        required: limit + 1,
                        limit,
                    }
                );
            }
            other => panic!("wrong result: {other:?}"),
        }
    }
}

#[test]
fn role_join_shared_budget_is_exact_and_one_less_has_no_successful_result() {
    check_join_budget(|work| run(None, None, work));
}

struct SecondaryFixture {
    types: Vec<SemanticTypeDeclV1>,
    body: SemanticFunctionDeclV1,
    carrier: SemanticTypeIdV1,
    shared: SemanticTypeIdV1,
    references: [SemanticTypeIdV1; 3],
    fields: [u32; 3],
    start: u32,
}

impl SecondaryFixture {
    fn new(fields: [u32; 3]) -> Self {
        let (mut types, body, _, _) = fixture();
        let policy = pointer(&mut types, 8, SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable, 0, 64, SemanticPointerMetadataV1::None);
        let references = [ty(5), policy, ty(6)];
        let mut field_types = vec![ty(4); *fields.iter().max().unwrap() as usize + 2];
        for (&field, &reference) in fields.iter().zip(&references) {
            field_types[field as usize] = reference;
        }
        let carrier = aggregate(&mut types, field_types);
        let shared = pointer(&mut types, carrier.index(), SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable, 0, 64, SemanticPointerMetadataV1::None);
        let start = body.locals().len() as u32;
        let body = locals(&body, [carrier, carrier, shared, ty(5), policy, ty(6), ty(4),
            ty(5), policy, ty(6)]);
        Self { types, body, carrier, shared, references, fields, start }
    }

    fn routes(&self) -> Routes<'_> {
        Routes::from_pairs_with_workgroups(&self.body, &self.types,
            &Pairs::from([(self.references[0], ty(2))]),
            &Pairs::from([(self.references[1], ty(8))]), &BTreeSet::new(),
            &Pairs::from([(self.references[2], ty(3))]), &mut budget(MAX_FLOW_WORK)).unwrap()
    }

    fn capture(&self) -> SemanticAssignmentV1 {
        let mut operands = vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(4), SemanticConstantValueV1::ZeroSized));
            *self.fields.iter().max().unwrap() as usize + 2];
        for (role, &field) in self.fields.iter().enumerate() {
            operands[field as usize] = SemanticOperandV1::Copy(
                place(self.start + 3 + role as u32, self.references[role]));
        }
        self.capture_with(operands)
    }

    fn capture_with(&self, operands: Vec<SemanticOperandV1>) -> SemanticAssignmentV1 {
        assignment(self.start, self.carrier,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, operands).unwrap())
    }

    fn leaf(&self, field: u32, result: SemanticTypeIdV1, shared: bool) -> SemanticPlaceV1 {
        let mut projections = Vec::new();
        if shared {
            projections.push(SemanticProjectionV1::new(
                SemanticProjectionKindV1::Dereference, self.carrier).unwrap());
        }
        projections.push(SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), result).unwrap());
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(self.start + if shared { 2 } else { 1 }),
            projections, result).unwrap()
    }

    fn nodes(&self) -> (Vec<SemanticBorrowCandidateV1>, BTreeMap<u32, usize>, Vec<Vec<usize>>) {
        let start = self.start;
        let mut candidates = vec![
            candidate(0, 200, 2, None, false),
            candidate(1, 201, 8, None, false),
            candidate(2, 202, 3, None, false),
            candidate(3, start + 3, 2, Some(start + 3), true),
            candidate(4, start, 3, Some(start), false),
            candidate(5, start + 2, 2, Some(start + 2), true),
            candidate(6, start + 2, 8, Some(start + 2), true),
            candidate(7, start + 2, 3, Some(start + 2), true),
        ];
        // The normal source edge is already counted when secondary joins run.
        candidates[0].consumers = 1;
        (candidates, BTreeMap::from([
            (start + 3, 0), (start + 4, 1), (start + 5, 2), (start, 3),
            (start + 2, 4), (start + 7, 5), (start + 8, 6), (start + 9, 7),
        ]), vec![vec![3], vec![], vec![], vec![4], vec![5, 6, 7], vec![], vec![], vec![]])
    }

    fn checked_fields(&self) -> CheckedFields {
        [Some((SecondaryRole::Policy, self.fields[1] as usize)),
            Some((SecondaryRole::Workgroup, self.fields[2] as usize))]
    }
}

#[test]
fn three_roles_keep_exact_direct_and_shared_extractions_and_only_markers_are_disjoint() {
    for fields in [[0, 1, 2], [2, 1, 0], [17, 0, 16]] {
        let f = SecondaryFixture::new(fields);
        let routes = f.routes();
        assert_eq!(routes.routes[&f.carrier].reference, f.references[0]);
        assert_eq!(routes.routes[&f.carrier].fields[0], fields[0]);
        for (slot, role) in [SecondaryRole::Policy, SecondaryRole::Workgroup].into_iter().enumerate() {
            let (actual, route) = routes.secondaries[&f.carrier][slot].unwrap();
            assert_eq!((actual, route.reference, route.fields[0]),
                (role, f.references[slot + 1], fields[slot + 1]));
        }
        assert_eq!(routes.shared_owned(f.shared, &mut budget(MAX_FLOW_WORK)).unwrap(), Some(ty(3)));
        let capture = f.capture();
        let SemanticRvalueKindV1::Aggregate(aggregate) = capture.value().kind() else { unreachable!() };
        let SemanticOperandV1::Copy(primary) = &aggregate.operands()[fields[0] as usize] else { unreachable!() };
        assert!(std::ptr::eq(routes.source(&capture, &mut budget(MAX_FLOW_WORK)).unwrap().unwrap(), primary));
        let (mut candidates, by_reference, _) = f.nodes();
        let checked = workgroup_role_joins_v1::Joins::default().connect(
            &routes, &capture, 3, &by_reference, &mut candidates, &mut budget(MAX_FLOW_WORK)).unwrap();
        assert_eq!(checked, f.checked_fields());
        routes.invalidate_siblings_checked(&capture, &by_reference, &mut candidates, checked, &mut budget(MAX_FLOW_WORK)).unwrap();
        assert!(candidates.iter().all(|c| c.valid));
        for (role, field, source) in routes.secondary_sources(&capture, &mut budget(MAX_FLOW_WORK))
            .unwrap().into_iter().flatten()
        {
            let index = match role { SecondaryRole::Policy => 1, SecondaryRole::Workgroup => 2 };
            assert_eq!(field, fields[index] as usize);
            let SemanticOperandV1::Copy(original) = &aggregate.operands()[field] else { unreachable!() };
            assert!(std::ptr::eq(source.unwrap(), original));
        }
        let borrow = assignment(f.start + 2, f.shared, SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared, place: place(f.start + 1, f.carrier),
        });
        let SemanticRvalueKindV1::Borrow { place: original, .. } = borrow.value().kind() else { unreachable!() };
        assert!(std::ptr::eq(routes.source(&borrow, &mut budget(MAX_FLOW_WORK)).unwrap().unwrap(), original));
        assert_eq!(routes.secondary_sources(&borrow, &mut budget(MAX_FLOW_WORK)).unwrap(), [None; 2]);
        let forward = assignment(f.start, f.carrier,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(f.start + 1, f.carrier))));
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(original)) = forward.value().kind() else { unreachable!() };
        assert!(std::ptr::eq(routes.source(&forward, &mut budget(MAX_FLOW_WORK)).unwrap().unwrap(), original));
        assert_eq!(routes.secondary_sources(&forward, &mut budget(MAX_FLOW_WORK)).unwrap(), [None; 2]);
        assert!(!routes.disjoint_field_use(&forward, &mut budget(MAX_FLOW_WORK)).unwrap());
        for shared in [false, true] {
            for role in 0..3 {
                for moved in [false, true] {
                    let p = f.leaf(fields[role], f.references[role], shared);
                    let operand = if moved { SemanticOperandV1::Move(p) } else { SemanticOperandV1::Copy(p) };
                    let a = assignment(f.start + 7 + role as u32, f.references[role], SemanticRvalueKindV1::Use(operand));
                    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(original) | SemanticOperandV1::Move(original))
                        = a.value().kind() else { unreachable!() };
                    assert!(std::ptr::eq(routes.source(&a, &mut budget(MAX_FLOW_WORK)).unwrap().unwrap(), original));
                    assert!(!routes.disjoint_field_use(&a, &mut budget(MAX_FLOW_WORK)).unwrap());
                }
            }
            let marker = f.leaf(*fields.iter().max().unwrap() + 1, ty(4), shared);
            let a = assignment(f.start + 6, ty(4), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(marker)));
            assert!(routes.source(&a, &mut budget(MAX_FLOW_WORK)).unwrap().is_none());
            assert!(routes.disjoint_field_use(&a, &mut budget(MAX_FLOW_WORK)).unwrap());
        }
    }
}

fn run_secondary_roots(
    failed: Option<usize>,
    rejected_root: Option<usize>,
    work: &mut Budget,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    let f = SecondaryFixture::new([0, 1, 2]);
    let routes = f.routes();
    let a = f.capture();
    let (mut candidates, by_reference, mut children) = f.nodes();
    let mut joins = workgroup_role_joins_v1::Joins::default();
    let checked = joins.connect(&routes, &a, 3, &by_reference, &mut candidates, work)?;
    assert_eq!(checked, f.checked_fields());
    routes.invalidate_siblings_checked(&a, &by_reference, &mut candidates, checked, work)?;
    assert!(candidates.iter().all(|c| c.valid));
    assert_eq!(candidates[..3].iter().map(|c| c.consumers).collect::<Vec<_>>(), vec![1; 3]);
    joins.start(&mut children, work)?;
    assert_eq!(&children[..3], &[vec![3], vec![3], vec![3]]);
    if let Some(failed) = failed { candidates[failed].valid = false; }
    let lane_sites: Vec<_> = (0..candidates.len()).map(|node| vec![SemanticTransparentBorrowSiteV1 {
        block: 1, statement: node as u32,
    }]).collect();
    let mut accepted = BTreeSet::new();
    for root in 0..3 {
        if rejected_root == Some(root) { continue }
        if let Ok(visited) = borrow_components_v1::members(&candidates, &children, root, work, |_| true)? {
            joins.record(&visited, work)?;
            for node in visited {
                if !candidates[node].value_alias { accepted.insert(candidates[node].site); }
                accepted.extend(lane_sites[node].iter().copied());
            }
        }
    }
    joins.retain(&candidates, &children, &lane_sites, &mut accepted, work)?;
    Ok(accepted)
}

#[test]
fn distinct_secondary_parents_require_every_root_and_descendant_for_borrow_and_lane_sites() {
    let expected: BTreeSet<_> = [0, 1, 2, 4].into_iter().map(|statement| SemanticTransparentBorrowSiteV1 { block: 0, statement })
        .chain((0..8).map(|statement| SemanticTransparentBorrowSiteV1 { block: 1, statement })).collect();
    assert_eq!(run_secondary_roots(None, None, &mut budget(MAX_FLOW_WORK)).unwrap(), expected);
    for failed in 0..8 {
        assert!(run_secondary_roots(Some(failed), None, &mut budget(MAX_FLOW_WORK)).unwrap().is_empty(),
            "failed node {failed} must veto all connected Borrow and lane sites");
    }
    for root in 0..3 {
        assert!(run_secondary_roots(None, Some(root), &mut budget(MAX_FLOW_WORK)).unwrap().is_empty(),
            "unrecorded owner proof {root} must veto all connected Borrow and lane sites");
    }
}

#[test]
fn secondary_roles_reuse_one_parent_edge_without_recounting_secondary_or_primary_consumers() {
    let f = SecondaryFixture::new([0, 1, 2]);
    let routes = f.routes();
    for primary_too in [false, true] {
        let capture = f.capture();
        let SemanticRvalueKindV1::Aggregate(aggregate) = capture.value().kind() else { unreachable!() };
        let mut operands = aggregate.operands().to_vec();
        for role in if primary_too { 0..3 } else { 1..3 } {
            operands[f.fields[role] as usize] = SemanticOperandV1::Copy(f.leaf(f.fields[role], f.references[role], false));
        }
        let a = f.capture_with(operands);
        let (mut candidates, mut by_reference, mut children) = f.nodes();
        by_reference.remove(&(f.start + 4));
        by_reference.remove(&(f.start + 5));
        by_reference.insert(f.start + 1, 1);
        // This carrier's primary describes neither of the joined secondary owners.
        candidates[1].source_type = ty(2);
        if primary_too {
            by_reference.remove(&(f.start + 3));
            candidates[3].source_local = f.start + 1;
            candidates[3].source_reference = Some(f.start + 1);
            candidates[0].consumers = 0;
            candidates[1].consumers = 1;
            children[0].clear();
            children[1].push(3);
        }
        let mut joins = workgroup_role_joins_v1::Joins::default();
        for _ in 0..2 {
            let checked = joins.connect(&routes, &a, 3, &by_reference, &mut candidates, &mut budget(MAX_FLOW_WORK)).unwrap();
            assert_eq!(checked, f.checked_fields());
            assert_eq!(candidates[1].consumers, 1);
            assert_eq!(candidates[0].consumers, if primary_too { 0 } else { 1 });
            routes.invalidate_siblings_checked(&a, &by_reference, &mut candidates, checked, &mut budget(MAX_FLOW_WORK)).unwrap();
            assert!(candidates.iter().all(|c| c.valid));
        }
        joins.start(&mut children, &mut budget(MAX_FLOW_WORK)).unwrap();
        assert_eq!(children[1], vec![3], "a repeated parent must add at most one edge");
        assert!(children[2].is_empty());
        assert_eq!(borrow_components_v1::members(&candidates, &children, 1, &mut budget(MAX_FLOW_WORK), |_| true).unwrap(),
            Ok(BTreeSet::from([1, 3, 4, 5, 6, 7])));
    }
}

#[test]
fn secondary_join_diamonds_still_reject_repeated_nodes() {
    let f = SecondaryFixture::new([0, 1, 2]);
    let routes = f.routes();
    let (mut candidates, by_reference, mut children) = f.nodes();
    candidates[1].source_reference = Some(f.start + 3);
    children[0].push(1);
    let mut joins = workgroup_role_joins_v1::Joins::default();
    let checked = joins.connect(&routes, &f.capture(), 3, &by_reference, &mut candidates, &mut budget(MAX_FLOW_WORK)).unwrap();
    assert_eq!(checked, f.checked_fields());
    joins.start(&mut children, &mut budget(MAX_FLOW_WORK)).unwrap();
    assert_eq!(borrow_components_v1::members(&candidates, &children, 0, &mut budget(MAX_FLOW_WORK), |_| true).unwrap(),
        Err(3), "two paths to a joined carrier must retain repeated-node rejection");
}

#[test]
fn three_role_join_shared_budget_is_exact_and_one_less_has_no_successful_result() {
    check_join_budget(|work| run_secondary_roots(None, None, work));
}

#[test]
fn ambiguous_roles_with_unmapped_inputs_veto_carriers_and_shared_wrappers() {
    // Inert route/graph inputs. The separate source test exercises a duplicate
    // Policy definition through actual admission and candidate discovery.
    for (duplicate, omit_primary) in [(0, false), (1, false), (2, false), (1, true)] {
        let mut f = SecondaryFixture::new([0, 1, 2]);
        let mut fields = f.references.to_vec();
        fields.push(f.references[duplicate]);
        if omit_primary { fields[0] = ty(4); }
        let carrier = aggregate(&mut f.types, fields.clone());
        let shared = pointer(&mut f.types, carrier.index(), SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable, 0, 64, SemanticPointerMetadataV1::None);
        let destination = f.body.locals().len() as u32;
        f.body = locals(&f.body, [carrier, shared]);
        let routes = f.routes();
        assert!(routes.routes.contains_key(&carrier));
        assert!(!routes.secondary_complete(carrier, &mut budget(MAX_FLOW_WORK)).unwrap());
        if duplicate != 2 {
            assert!(routes.shared.contains_key(&shared));
            assert!(!routes.secondary_complete(shared, &mut budget(MAX_FLOW_WORK)).unwrap());
        }
        let operands = fields.into_iter().map(|ty_| {
            match f.references.iter().position(|&reference| reference == ty_) {
                Some(role) => SemanticOperandV1::Copy(place(f.start + 3 + role as u32, ty_)),
                None => SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty_, SemanticConstantValueV1::ZeroSized)),
            }
        }).collect();
        let capture = assignment(destination, carrier,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, operands).unwrap());
        let (mut candidates, mut by_reference, _) = f.nodes();
        by_reference.remove(&(f.start + 3 + duplicate as u32));
        by_reference.remove(&f.start);
        by_reference.insert(destination, 3);
        assert!(!by_reference.contains_key(&(f.start + 3 + duplicate as u32)));
        let checked = workgroup_role_joins_v1::Joins::default().connect(
            &routes, &capture, 3, &by_reference, &mut candidates, &mut budget(MAX_FLOW_WORK)).unwrap();
        assert_eq!(checked, [None; 2]);
        assert!(!candidates[3].valid, "missing role {duplicate}, omit_primary={omit_primary}");
        if duplicate != 2 {
            candidates[3].valid = true;
            let borrow = assignment(destination + 1, shared, SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared, place: place(destination, carrier),
            });
            assert_eq!(workgroup_role_joins_v1::Joins::default().connect(
                &routes, &borrow, 3, &by_reference, &mut candidates, &mut budget(MAX_FLOW_WORK)).unwrap(), [None; 2]);
            assert!(!candidates[3].valid);
        }
    }
}

#[test]
fn secondary_complete_has_an_independent_four_unit_boundary_for_one_failure_and_wrapper() {
    let f = SecondaryFixture::new([0, 1, 2]);
    let mut routes = f.routes();
    routes.shared.retain(|&ty, _| ty == f.shared);
    routes.failed_secondaries = BTreeSet::from([f.carrier]);
    assert_eq!(routes.shared.len(), 1);
    assert_eq!(routes.shared[&f.shared].pointee, f.carrier);
    assert_eq!(routes.failed_secondaries.len(), 1);

    // Each one-entry lookup costs two units, independently of measured work.
    for (query, complete) in [(f.carrier, false), (f.shared, false), (f.references[0], true)] {
        for spent in [0, 17] {
            let mut exact = budget(spent + 4);
            exact.charge(spent).unwrap();
            assert_eq!(routes.secondary_complete(query, &mut exact).unwrap(), complete);
            assert_eq!(exact.remaining, 0);

            let mut short = budget(spent + 3);
            short.charge(spent).unwrap();
            let Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
                stage,
                phase_work_units,
                ordered_proof_calls,
                remaining_work_units,
                requested_work_units,
                error,
            }) = routes.secondary_complete(query, &mut short) else {
                panic!("three units must not return a completeness result");
            };
            assert_eq!(stage, "facts");
            assert_eq!(ordered_proof_calls, 0);
            assert_eq!((remaining_work_units, requested_work_units), (3, 4));
            assert_eq!(short.remaining, 3);
            assert_eq!(short.limit, spent + 3);
            let mut phases = [0; 11];
            phases[0] = spent;
            assert_eq!(phase_work_units, phases);
            assert_eq!(
                *error,
                ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    required: spent + 4,
                    limit: spent + 3,
                }
            );

            let mut retry = budget(4);
            assert_eq!(routes.secondary_complete(query, &mut retry).unwrap(), complete);
            assert_eq!(retry.remaining, 0);
        }
    }
    assert_eq!(routes.failed_secondaries, BTreeSet::from([f.carrier]));
}

#[test]
fn either_missing_secondary_source_parent_or_self_parent_is_not_exempt_and_the_other_role_still_joins() {
    let f = SecondaryFixture::new([0, 1, 2]);
    let routes = f.routes();
    for role in 1..3 {
        for failure in 0..3 {
            let capture = f.capture();
            let SemanticRvalueKindV1::Aggregate(aggregate) = capture.value().kind() else { unreachable!() };
            let mut operands = aggregate.operands().to_vec();
            let (mut candidates, mut by_reference, mut children) = f.nodes();
            match failure {
                0 => {
                    // Projected aggregate Moves remain forbidden even for an exact leaf.
                    operands[f.fields[role] as usize] = SemanticOperandV1::Move(f.leaf(f.fields[role], f.references[role], false));
                    by_reference.insert(f.start + 1, role);
                }
                1 => { by_reference.remove(&(f.start + 3 + role as u32)); }
                2 => { by_reference.insert(f.start + 3 + role as u32, 3); }
                _ => unreachable!(),
            }
            let a = f.capture_with(operands);
            let mut joins = workgroup_role_joins_v1::Joins::default();
            let checked = joins.connect(&routes, &a, 3, &by_reference, &mut candidates, &mut budget(MAX_FLOW_WORK)).unwrap();
            let successful = 3 - role;
            assert_eq!(checked.into_iter().flatten().collect::<Vec<_>>(),
                vec![f.checked_fields()[successful - 1].unwrap()]);
            assert!(!candidates[3].valid, "role {role}, failure {failure}");
            assert_eq!(candidates[role].consumers, 0);
            assert_eq!(candidates[successful].consumers, 1);
            routes.invalidate_siblings_checked(&a, &by_reference, &mut candidates, checked, &mut budget(MAX_FLOW_WORK)).unwrap();
            if failure == 0 { assert!(!candidates[role].valid, "a failed source gets no sibling exemption"); }
            assert!(candidates[successful].valid);
            joins.start(&mut children, &mut budget(MAX_FLOW_WORK)).unwrap();
            assert!(children[role].is_empty());
            assert_eq!(children[successful], vec![3]);
        }
    }
}
