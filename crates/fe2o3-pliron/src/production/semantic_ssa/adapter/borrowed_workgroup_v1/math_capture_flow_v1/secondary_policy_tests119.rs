use super::*;
use crate::production::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
    ProductionSemanticSsaSourceQueryErrorV1, ProductionSemanticSsaSourceSiteV1,
};
use fe2o3_mir_model::{SsaResolvedEventV1, SsaVariableIdV1};

#[path = "secondary_policy_fixture119.rs"]
mod fixture;
use fixture::{SecondaryPolicyMutation119 as Mutation, secondary_policy_source119};

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn budget(limit: usize) -> Budget {
    Budget {
        limit,
        remaining: limit,
        profile: FlowWorkProfile::default(),
    }
}

fn expanded(source: &AdmittedInertSemanticMirV1) -> SemanticCallExpansionV1 {
    let expansion = SemanticCallExpansionV1::try_new(source, Default::default()).unwrap();
    expansion.verify_replay(source).unwrap();
    expansion
}

fn owner(source: AdmittedInertSemanticMirV1) -> ProductionSemanticSsaOwnerV1 {
    let identity = *source.semantic_sha256().as_bytes();
    let bytes = source.canonical_encoding().to_vec();
    let root = source.functions()[0].clone();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    assert_eq!(owner.source_semantic_sha256(), &identity);
    assert_eq!(
        owner.source_semantic().canonical_encoding(),
        bytes.as_slice()
    );
    assert_eq!(owner.source_semantic().functions()[0], root);
    assert!(!owner.grants_proof_or_artifact_authority());
    owner
}

fn source_site(
    view: &SemanticExpandedRootV1,
    block: u32,
    statement: Option<u32>,
) -> ProductionSemanticSsaSourceSiteV1 {
    view.block_origins()
        .iter()
        .enumerate()
        .find_map(|(expanded, origin)| {
            if origin.instance().index() != 0
                || origin.function().index() != 0
                || origin.block().index() != block
            {
                return None;
            }
            let statement = match statement {
                Some(statement) => Some(origin.statements().iter().position(|item| {
                    *item == SemanticExpandedStatementOriginV1::Source { statement }
                })? as u32),
                None => None,
            };
            Some(ProductionSemanticSsaSourceSiteV1::new(
                SemanticBlockIdV1::from_index(expanded as u32),
                statement,
            ))
        })
        .expect("original root source site")
}

fn assignment(
    view: &SemanticExpandedRootV1,
    site: ProductionSemanticSsaSourceSiteV1,
) -> &SemanticAssignmentV1 {
    let SemanticStatementKindV1::Assign(assignment) = view.body().blocks()
        [site.block().index() as usize]
        .statements()[site.statement().unwrap() as usize]
        .kind()
    else {
        panic!("original assignment")
    };
    assignment
}

fn borrow_place(
    view: &SemanticExpandedRootV1,
    site: ProductionSemanticSsaSourceSiteV1,
) -> &SemanticPlaceV1 {
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment(view, site).value().kind()
    else {
        panic!("original shared Borrow")
    };
    place
}

fn transparent(site: ProductionSemanticSsaSourceSiteV1) -> SemanticTransparentBorrowSiteV1 {
    SemanticTransparentBorrowSiteV1 {
        block: site.block().index(),
        statement: site.statement().unwrap(),
    }
}

fn assert_events_and_promotion(
    source: &AdmittedInertSemanticMirV1,
    view: &SemanticExpandedRootV1,
    sites: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    promoted: bool,
) {
    let (classified, _, _) = semantic_function_ssa_input_v1(
        view.body(),
        Some(source.types()),
        source.callables(),
        sites,
    );
    let (unclassified, _, _) = semantic_function_ssa_input_v1(
        view.body(),
        Some(source.types()),
        source.callables(),
        &BTreeSet::new(),
    );
    assert_eq!(
        classified.blocks(),
        unclassified.blocks(),
        "original Use/Define/Kill and call-transfer event blocks must not change"
    );
    for (block, statement) in [(7, 1), (4, 0)] {
        let site = source_site(view, block, Some(statement));
        let local = borrow_place(view, site).local().index() as usize;
        assert_eq!(sites.contains(&transparent(site)), promoted);
        assert_eq!(classified.promotable()[local], promoted);
        assert!(!unclassified.promotable()[local]);
    }
}

fn assert_route(route: &Route, reference: u32, owned: u32, fields: &[u32]) {
    assert_eq!(
        (route.reference, route.owned, &route.fields[..route.len]),
        (ty(reference), ty(owned), fields)
    );
}

#[test]
fn secondary_policy119_routes_and_original_operands_use_checked_matrix_bind_seeds() {
    for mutation in [Mutation::Live, Mutation::Reordered] {
        let source = secondary_policy_source119(mutation);
        let expansion = expanded(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let bindings = expansion.defined_capability_bindings(&source).unwrap();
        let matrix_bindings = bindings
            .iter()
            .filter(|binding| {
                matches!(
                    binding.contract(),
                    SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(matrix_bindings.len(), 2);
        let math = MathBorrowSitesV1::new(&source, view, &bindings, MAX_FLOW_WORK).unwrap();
        let matrix =
            MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, MAX_FLOW_WORK).unwrap();
        assert_eq!(math.pairs.get(&ty(7)), Some(&ty(5)));
        assert_eq!(
            matrix.policy_carrier_leaves(),
            &BTreeMap::from([(ty(7), ty(5))])
        );
        assert_eq!(matrix.carrier_leaves(), &BTreeMap::from([(ty(13), ty(12))]));
        let mut work = budget(MAX_FLOW_WORK);
        let mut policy_arguments = BTreeSet::new();
        for binding in matrix_bindings {
            assert_eq!(binding.expansion_identity(), expansion.identity());
            assert_eq!(binding.root_identity(), view.identity());
            let SemanticOperandV1::Copy(policy) = &binding.arguments()[1] else {
                panic!("original Policy argument")
            };
            let origin = view.local_origins()[policy.local().index() as usize];
            assert_eq!(
                (origin.instance().index(), origin.function().index()),
                (0, 0)
            );
            policy_arguments.insert(origin.local().index());
            let site = SemanticTransparentBorrowSiteV1 {
                block: binding.expanded_entry_block().index(),
                statement: 0,
            };
            let kind = view.body().blocks()[site.block as usize].statements()[0].kind();
            let expected = binding
                .callee_arguments()
                .iter()
                .map(|local| local.index())
                .collect::<Vec<_>>();
            assert_eq!(
                matrix
                    .captured(site, kind, &mut |n| work.charge(n))
                    .unwrap()
                    .unwrap(),
                expected.as_slice()
            );
            assert!(
                matrix
                    .captured(site, &kind.clone(), &mut |n| work.charge(n))
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(policy_arguments, BTreeSet::from([6, 20]));
        let barriers = matrix.carrier_barriers().clone();
        let routes = Routes::new_with_matrix(
            view.body(),
            Some(source.types()),
            source.callables(),
            &matrix,
            &mut work,
        )
        .unwrap();
        let (math_field, policy_field) = mutation.leaf_fields();
        assert_route(routes.routes.get(&ty(17)).unwrap(), 10, 9, &[math_field]);
        let secondaries = routes.secondaries.get(&ty(17)).unwrap();
        let (role, policy) = secondaries[0].expect("Policy occupies slot zero");
        assert_eq!(role, SecondaryRole::Policy);
        assert_route(&policy, 7, 5, &[policy_field]);
        assert!(secondaries[1].is_none(), "no Workgroup seed in this source");
        assert_eq!(routes.owned(ty(17)), Some(&ty(9)));
        assert_eq!(
            routes.owned(ty(9)),
            None,
            "Math bound remains an owned barrier"
        );
        assert_eq!(
            routes.owned(ty(14)),
            None,
            "Matrix bound remains an owned barrier"
        );
        assert!(!routes.secondaries.contains_key(&ty(9)));
        assert!(!routes.secondaries.contains_key(&ty(14)));
        assert_eq!(matrix.carrier_barriers(), &barriers);

        let capture = assignment(view, source_site(view, 4, Some(1)));
        let SemanticRvalueKindV1::Aggregate(aggregate) = capture.value().kind() else {
            panic!("original mixed carrier capture")
        };
        let SemanticOperandV1::Copy(math) = &aggregate.operands()[math_field as usize] else {
            panic!("original Math operand")
        };
        assert!(std::ptr::eq(
            routes.source(capture, &mut work).unwrap().unwrap(),
            math
        ));
        let sources = routes.secondary_sources(capture, &mut work).unwrap();
        let (role, field, policy) = sources[0].unwrap();
        assert_eq!(
            (role, field),
            (SecondaryRole::Policy, policy_field as usize)
        );
        let SemanticOperandV1::Copy(expected) = &aggregate.operands()[field] else {
            panic!("original Policy operand")
        };
        assert!(std::ptr::eq(policy.unwrap(), expected));
        assert!(sources[1].is_none());

        // Omitting checked Matrix facts is a private route-constructor control.
        let unseeded = Routes::new(
            view.body(),
            Some(source.types()),
            source.callables(),
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        assert_route(unseeded.routes.get(&ty(17)).unwrap(), 10, 9, &[math_field]);
        assert!(!unseeded.secondaries.contains_key(&ty(17)));
        expansion.verify_replay(&source).unwrap();
    }
}

#[test]
fn secondary_policy119_live_and_reordered_promote_both_original_borrows_with_exact_owner_uses() {
    for mutation in [Mutation::Live, Mutation::Reordered] {
        let owner = owner(secondary_policy_source119(mutation));
        let source = owner.source_semantic();
        let root = source.roots()[0];
        let view = owner.execution_view_for_root(root).unwrap();
        let sites =
            execution_sites(source, owner.execution_expansion(), view, MAX_FLOW_WORK).unwrap();
        assert_events_and_promotion(source, view, &sites, true);
        let query = owner.source_query_for_root(root, view.body()).unwrap();
        let plan = owner.execution_plan_for_root(root).unwrap().plan();
        for (block, statement, original_owner) in [(7, 1, 4), (4, 0, 7)] {
            let site = source_site(view, block, Some(statement));
            let place = borrow_place(view, site);
            let origin = view.local_origins()[place.local().index() as usize];
            assert_eq!(
                (
                    origin.instance().index(),
                    origin.function().index(),
                    origin.local().index()
                ),
                (0, 0, original_owner)
            );
            assert!(
                plan.promoted_variables()
                    .contains(&SsaVariableIdV1::new(place.local().index()))
            );
            let value = query.borrow_place_use(site, place, &mut || true).unwrap();
            assert!(query.resolved_events_at(site, &mut || true).unwrap().iter().any(
                |(_, event)| matches!(event, SsaResolvedEventV1::Use { variable, value: actual }
                    if variable.get() == place.local().index() && *actual == value)
            ));
            assert!(matches!(
                query.borrow_place_use(site, &place.clone(), &mut || true),
                Err(ProductionSemanticSsaSourceQueryErrorV1::OperandOutsideSite)
            ));
            assert!(query.borrow_place_use(site, place, &mut || false).is_err());
        }
        let foreign = view.body().clone();
        assert!(matches!(
            owner.source_query_for_root(root, &foreign),
            Err(ProductionSemanticSsaSourceQueryErrorV1::WrongOwner)
        ));
        let other = self::owner(secondary_policy_source119(mutation));
        assert_eq!(
            owner.source_semantic_sha256(),
            other.source_semantic_sha256()
        );
        assert!(matches!(
            owner.source_query_for_root(root, other.execution_view_for_root(root).unwrap().body()),
            Err(ProductionSemanticSsaSourceQueryErrorV1::WrongOwner)
        ));
        owner.verify_replay().unwrap();
    }
}

#[test]
fn secondary_policy119_replay_and_expansion_owner_gates_reject_foreign_views() {
    let source = secondary_policy_source119(Mutation::Live);
    let reordered = secondary_policy_source119(Mutation::Reordered);
    assert_ne!(source.semantic_sha256(), reordered.semantic_sha256());
    let expansion = expanded(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    assert!(expansion.verify_replay(&reordered).is_err());
    assert!(matches!(
        execution_sites(&reordered, &expansion, view, MAX_FLOW_WORK),
        Err(ProductionSemanticSsaErrorV1::CallExpansion(_))
    ));
    let other_expansion = expanded(&source);
    assert!(matches!(
        execution_sites(
            &source,
            &expansion,
            other_expansion.root(source.roots()[0]).unwrap(),
            MAX_FLOW_WORK
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    expansion.verify_replay(&source).unwrap();
}

#[test]
fn secondary_policy119_duplicate_policy_references_admit_then_withhold_the_component() {
    for mutation in [
        Mutation::DuplicatePolicySameReference,
        Mutation::DuplicatePolicySameReferenceUnmapped,
        Mutation::DuplicatePolicyDistinctReference,
    ] {
        let source = fixture::try_secondary_policy_source119(mutation)
            .expect("duplicate Policy fields must reach classification after source admission");
        let bytes = source.canonical_encoding().to_vec();
        let expansion = expanded(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let capture = assignment(view, source_site(view, 4, Some(1)));
        let SemanticRvalueKindV1::Aggregate(aggregate) = capture.value().kind() else {
            panic!("original duplicate capture")
        };
        assert_eq!(aggregate.operands().len(), 3);
        let mut policy_references = Vec::new();
        for operand in &aggregate.operands()[1..] {
            let SemanticOperandV1::Copy(place) = operand else {
                panic!("Policy copy")
            };
            assert_eq!(place.ty(), ty(7));
            policy_references.push(
                view.local_origins()[place.local().index() as usize]
                    .local()
                    .index(),
            );
        }
        let distinct = mutation == Mutation::DuplicatePolicyDistinctReference;
        let unmapped = mutation == Mutation::DuplicatePolicySameReferenceUnmapped;
        assert_eq!(
            policy_references,
            if distinct { vec![6, 22] } else { vec![6, 6] }
        );
        if distinct || unmapped {
            let first_site = source_site(view, 7, Some(1));
            let second_site = source_site(view, 7, Some(2));
            let first = borrow_place(view, first_site);
            let second = borrow_place(view, second_site);
            assert_eq!(
                first, second,
                "both references borrow the same original Policy owner"
            );
            let definitions = [assignment(view, first_site), assignment(view, second_site)];
            let destinations = definitions.map(|definition| {
                view.local_origins()[definition.destination().local().index() as usize]
                    .local()
                    .index()
            });
            assert_eq!(destinations, if distinct { [6, 22] } else { [6, 6] });
            if unmapped {
                assert_eq!(
                    definitions[0], definitions[1],
                    "duplicate the original local6 Borrow, not a second reference local"
                );
            }
        }
        let bindings = expansion.defined_capability_bindings(&source).unwrap();
        let matrix =
            MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, MAX_FLOW_WORK).unwrap();
        let routes = Routes::new_with_matrix(
            view.body(),
            Some(source.types()),
            source.callables(),
            &matrix,
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        assert_route(routes.routes.get(&ty(17)).unwrap(), 10, 9, &[0]);
        assert!(
            routes
                .secondaries
                .get(&ty(17))
                .is_none_or(|roles| roles[0].is_none())
        );
        let sites = execution_sites(&source, &expansion, view, MAX_FLOW_WORK)
            .expect("admitted duplicates withhold sites without a classification error");
        assert_events_and_promotion(&source, view, &sites, false);
        if distinct || unmapped {
            assert!(!sites.contains(&transparent(source_site(view, 7, Some(2)))));
        }
        assert_eq!(source.canonical_encoding(), bytes.as_slice());
        expansion.verify_replay(&source).unwrap();
    }
}

#[test]
fn secondary_policy119_late_policy_and_math_addresses_withhold_both_connected_roots() {
    for mutation in [
        Mutation::LatePolicyAddressEscape,
        Mutation::LateMathAddressEscape,
    ] {
        let source = fixture::try_secondary_policy_source119(mutation)
            .expect("late AddressOf must be admitted with an exact raw-pointer type");
        let bytes = source.canonical_encoding().to_vec();
        let original = &source.functions()[0];
        let SemanticTerminatorKindV1::Call(matrix) = original.blocks()[4].terminator().kind()
        else {
            panic!("second MatrixBind")
        };
        assert_eq!(matrix.callee().index(), 4);
        assert_eq!(matrix.destination().unwrap().edge().target().index(), 8);
        let SemanticTerminatorKindV1::Call(math) = original.blocks()[8].terminator().kind() else {
            panic!("Math consumer before escape")
        };
        assert_eq!(math.callee().index(), 8);
        assert_eq!(math.destination().unwrap().edge().target().index(), 5);
        let expansion = expanded(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let escape = assignment(view, source_site(view, 5, Some(0)));
        let SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Immutable,
            place,
        } = escape.value().kind()
        else {
            panic!("late original AddressOf")
        };
        let origin = view.local_origins()[place.local().index() as usize];
        assert_eq!(
            Some((origin.local().index(), place.ty().index())),
            mutation.escaped_reference()
        );
        let SemanticTypeShapeV1::Pointer(pointer) = source.types()[18].shape() else {
            panic!("raw-pointer result")
        };
        assert_eq!(pointer.kind(), SemanticPointerKindV1::Raw);
        assert_eq!(pointer.pointee(), place.ty());
        assert_eq!(escape.value().result_type(), ty(18));
        let sites = execution_sites(&source, &expansion, view, MAX_FLOW_WORK).unwrap();
        assert_events_and_promotion(&source, view, &sites, false);
        assert_eq!(source.canonical_encoding(), bytes.as_slice());
        expansion.verify_replay(&source).unwrap();
    }
}

fn assert_budget_failure(error: ProductionSemanticSsaErrorV1, limit: usize) {
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {
        phase_work_units,
        remaining_work_units,
        requested_work_units,
        error,
        ..
    } = error
    else {
        panic!("expected the original Budget failure: {error:?}")
    };
    assert_eq!(
        phase_work_units.iter().sum::<usize>() + remaining_work_units,
        limit
    );
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

#[test]
fn secondary_policy119_route_registry_exact_and_one_short_budget_are_atomic() {
    for mutation in [Mutation::Live, Mutation::Reordered] {
        let source = secondary_policy_source119(mutation);
        let bytes = source.canonical_encoding().to_vec();
        let expansion = expanded(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let bindings = expansion.defined_capability_bindings(&source).unwrap();
        let matrix =
            MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, MAX_FLOW_WORK).unwrap();
        let barriers = matrix.carrier_barriers().clone();
        let construct = |work: &mut Budget| {
            Routes::new_with_matrix(
                view.body(),
                Some(source.types()),
                source.callables(),
                &matrix,
                work,
            )
        };
        let mut ample = budget(MAX_FLOW_WORK);
        let expected = construct(&mut ample).unwrap();
        assert!(expected.secondaries.contains_key(&ty(17)));
        let required = MAX_FLOW_WORK - ample.remaining;
        assert!(required > 1);
        let mut exact = budget(required);
        let actual = construct(&mut exact).unwrap();
        assert_eq!(exact.remaining, 0);
        assert_eq!(actual.routes, expected.routes);
        assert_eq!(actual.secondaries, expected.secondaries);
        let mut short = budget(required - 1);
        let Err(error) = construct(&mut short) else {
            panic!("one-short registry accepted")
        };
        assert_budget_failure(error, required - 1);
        assert_eq!(matrix.carrier_barriers(), &barriers);
        let retried = construct(&mut budget(required)).unwrap();
        assert_eq!(retried.routes, expected.routes);
        assert_eq!(retried.secondaries, expected.secondaries);
        assert_eq!(source.canonical_encoding(), bytes.as_slice());
    }
}

#[test]
fn secondary_policy119_full_classifier_exact_and_one_short_budget_publish_no_partial_sites() {
    for mutation in [Mutation::Live, Mutation::Reordered] {
        let source = secondary_policy_source119(mutation);
        let bytes = source.canonical_encoding().to_vec();
        let expansion = expanded(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let classify = |limit| execution_sites(&source, &expansion, view, limit);
        let expected = classify(MAX_FLOW_WORK).unwrap();
        assert_events_and_promotion(&source, view, &expected, true);
        // Each independent run uses the production classifier's cumulative Budget.
        let (mut low, mut high) = (0, MAX_FLOW_WORK);
        while low < high {
            let middle = low + (high - low) / 2;
            match classify(middle) {
                Ok(sites) => {
                    assert_eq!(sites, expected);
                    high = middle;
                }
                Err(error @ ProductionSemanticSsaErrorV1::BorrowFlowWork { .. }) => {
                    assert_budget_failure(error, middle);
                    low = middle + 1;
                }
                // Checked Bind construction can exhaust its seed allowance first.
                Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    required,
                    limit,
                }) => {
                    assert!(required > limit);
                    assert!(limit <= middle);
                    low = middle + 1;
                }
                Err(error) => panic!("non-budget classification failure: {error:?}"),
            }
        }
        assert!(low > 1);
        assert_eq!(classify(low).unwrap(), expected);
        assert_budget_failure(classify(low - 1).unwrap_err(), low - 1);
        assert_eq!(classify(low).unwrap(), expected);
        assert_eq!(source.canonical_encoding(), bytes.as_slice());
        expansion.verify_replay(&source).unwrap();
    }
}
