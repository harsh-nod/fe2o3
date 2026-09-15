use super::*;
use crate::production::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
    ProductionSemanticSsaSourceQueryErrorV1, ProductionSemanticSsaSourceSiteV1,
};
use fe2o3_mir_model::SsaResolvedEventV1;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "mixed_math_fixture117.rs"]
mod fixture;
use fixture::{MixedMutation117, mixed_source117};

fn ty(id: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(id)
}

fn budget() -> Budget {
    Budget {
        limit: MAX_FLOW_WORK,
        remaining: MAX_FLOW_WORK,
        profile: FlowWorkProfile::default(),
    }
}

fn owner(mutation: MixedMutation117) -> ProductionSemanticSsaOwnerV1 {
    source_owner(mixed_source117(mutation))
}

fn source_owner(source: AdmittedInertSemanticMirV1) -> ProductionSemanticSsaOwnerV1 {
    let identity = *source.semantic_sha256().as_bytes();
    let root = source.functions()[0].clone();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    assert_eq!(owner.source_semantic_sha256(), &identity);
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

fn borrow_place(
    view: &SemanticExpandedRootV1,
    site: ProductionSemanticSsaSourceSiteV1,
) -> &SemanticPlaceV1 {
    let SemanticStatementKindV1::Assign(assignment) = view.body().blocks()
        [site.block().index() as usize]
        .statements()[site.statement().unwrap() as usize]
        .kind()
    else {
        panic!("original assignment")
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment.value().kind()
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

fn assert_unchanged_events(
    owner: &ProductionSemanticSsaOwnerV1,
    sites: &BTreeSet<SemanticTransparentBorrowSiteV1>,
) {
    let source = owner.source_semantic();
    let view = owner.execution_view_for_root(source.roots()[0]).unwrap();
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
        "original Use/Define/Kill events must not change"
    );
    let bound = borrow_place(view, source_site(view, 4, Some(0)));
    assert!(classified.promotable()[bound.local().index() as usize]);
    assert!(!unclassified.promotable()[bound.local().index() as usize]);
}

#[test]
fn mixed_math_matrix_seed_and_captures_come_from_replayed_bindings() {
    let source = mixed_source117(MixedMutation117::Live);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    expansion.verify_replay(&source).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let math_binding = bindings
        .iter()
        .find(|binding| {
            matches!(
                binding.contract(),
                SemanticDefinedCapabilityContractV1::PolicyMathBind(_)
            )
        })
        .unwrap();
    let matrix_binding = bindings
        .iter()
        .find(|binding| {
            matches!(
                binding.contract(),
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
            )
        })
        .unwrap();
    let math = MathBorrowSitesV1::new(&source, view, &bindings, MAX_FLOW_WORK).unwrap();
    let matrix =
        MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, MAX_FLOW_WORK).unwrap();
    assert_eq!(
        matrix.policy_carrier_leaves(),
        &BTreeMap::from([(ty(7), ty(5))])
    );
    assert_eq!(matrix.carrier_leaves(), &BTreeMap::from([(ty(13), ty(12))]));
    assert_eq!(math.pairs.get(&ty(7)), Some(&ty(5)));
    assert_eq!(math_binding.arguments()[1], matrix_binding.arguments()[1]);
    let SemanticOperandV1::Copy(policy) = &math_binding.arguments()[1] else {
        panic!("original Policy argument")
    };
    let origin = view.local_origins()[policy.local().index() as usize];
    assert_eq!(
        (
            origin.instance().index(),
            origin.function().index(),
            origin.local().index()
        ),
        (0, 0, 6)
    );
    let consumer =
        MathConsumerBorrowV1::for_callable(source.types(), &source.callables()[8]).unwrap();
    assert_eq!(consumer.pair(), (ty(10), ty(9)));
    let mut work = budget();
    for (binding, is_math) in [(math_binding, true), (matrix_binding, false)] {
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
        if is_math {
            assert_eq!(
                math.captured(site, kind).unwrap().as_slice(),
                expected.as_slice()
            );
            let wrong_site = SemanticTransparentBorrowSiteV1 {
                statement: 1,
                ..site
            };
            assert!(math.captured(wrong_site, kind).is_none());
            let SemanticStatementKindV1::Assign(assignment) = kind else {
                panic!()
            };
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                panic!()
            };
            let mut swapped = aggregate.operands().to_vec();
            swapped.swap(0, 1);
            let changed = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                assignment.destination().clone(),
                SemanticRvalueV1::new(
                    assignment.value().result_type(),
                    SemanticRvalueKindV1::aggregate(aggregate.kind().clone(), swapped).unwrap(),
                ),
            ));
            assert!(math.captured(site, &changed).is_none());
        } else {
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
    }
    let original_barriers = matrix.carrier_barriers().clone();
    assert_eq!(original_barriers, BTreeSet::from([ty(14)]));
    // The first Math insertion clones one existing Matrix barrier: 3 + 2*1
    // copy/storage, ordered lookup 2, and entry allowance 2. No input mutation.
    let before_registry = source.callables().len() + 16 + 2;
    for remaining in 0..9 {
        let limit = before_registry + remaining;
        let mut short = Budget { limit, remaining: limit, profile: FlowWorkProfile::default() };
        let Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
            remaining_work_units, requested_work_units, phase_work_units, ..
        }) = Routes::new_with_matrix(view.body(), Some(source.types()), source.callables(),
            &matrix, &mut short) else { panic!("short mixed registry budget accepted") };
        assert_eq!((remaining_work_units, requested_work_units), (remaining, 9));
        assert_eq!(phase_work_units.iter().sum::<usize>(), before_registry);
        assert_eq!(matrix.carrier_barriers(), &original_barriers);
    }
    let routes = Routes::new_with_matrix(
        view.body(),
        Some(source.types()),
        source.callables(),
        &matrix,
        &mut work,
    )
    .unwrap();
    assert_eq!(routes.owned(ty(9)), None);
    assert_eq!(routes.owned(ty(10)), Some(&ty(9)));
    assert_eq!(matrix.carrier_barriers(), &original_barriers);
    // Private classifier control only: omission of the Math barrier exposes the
    // checked Matrix Policy seed. This is not a second production pipeline.
    let mut primary = matrix.carrier_leaves().clone();
    primary.insert(consumer.pair().0, consumer.pair().1);
    let control = Routes::from_pairs_with_policy(
        view.body(),
        source.types(),
        &primary,
        matrix.policy_carrier_leaves(),
        matrix.carrier_barriers(),
        &mut budget(),
    )
    .unwrap();
    let route = control.routes.get(&ty(9)).unwrap();
    assert_eq!(
        (route.reference, route.owned, &route.fields[..route.len]),
        (ty(7), ty(5), &[1][..])
    );
}

#[test]
fn mixed_math_production_sites_promote_the_original_bound_borrow_without_event_changes() {
    let owner = owner(MixedMutation117::Live);
    let source = owner.source_semantic();
    let root = source.roots()[0];
    let view = owner.execution_view_for_root(root).unwrap();
    let sites = execution_sites(source, owner.execution_expansion(), view, MAX_FLOW_WORK).unwrap();
    let query = owner.source_query_for_root(root, view.body()).unwrap();
    for (block, statement, original_owner) in [(3, 0, 3), (7, 1, 4), (4, 0, 7)] {
        let site = source_site(view, block, Some(statement));
        assert!(
            sites.contains(&transparent(site)),
            "original Borrow {block}:{statement}"
        );
        let place = borrow_place(view, site);
        assert_eq!(
            view.local_origins()[place.local().index() as usize]
                .local()
                .index(),
            original_owner
        );
        let value = query.borrow_place_use(site, place, &mut || true).unwrap();
        assert!(
            query
                .resolved_events_at(site, &mut || true)
                .unwrap()
                .iter()
                .any(
                    |(_, event)| matches!(event, SsaResolvedEventV1::Use { variable, value: actual }
                if variable.get() == place.local().index() && *actual == value)
                )
        );
        assert!(matches!(
            query.borrow_place_use(site, &place.clone(), &mut || true),
            Err(ProductionSemanticSsaSourceQueryErrorV1::OperandOutsideSite)
        ));
        assert!(query.borrow_place_use(site, place, &mut || false).is_err());
    }
    assert_unchanged_events(&owner, &sites);
    let foreign = view.body().clone();
    assert!(matches!(
        owner.source_query_for_root(root, &foreign),
        Err(ProductionSemanticSsaSourceQueryErrorV1::WrongOwner)
    ));
}

#[test]
fn mixed_math_absent_matrix_bind_has_no_seed_and_unreachable_bind_is_rejected() {
    assert!(matches!(
        fixture::try_mixed_source117(MixedMutation117::MissingMatrixCall),
        Err(SemanticMirErrorV1::FunctionOutsideRootClosure { function }) if function.index() == 4
    ));
    let owner = source_owner(fixture::full_source(false, false));
    let source = owner.source_semantic();
    let expansion = owner.execution_expansion();
    let view = owner.execution_view_for_root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(source).unwrap();
    assert!(!bindings.iter().any(|binding| matches!(
        binding.contract(),
        SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
    )));
    let matrix =
        MatrixBorrowSitesV1::new(source, expansion, view, &bindings, MAX_FLOW_WORK).unwrap();
    assert!(matrix.policy_carrier_leaves().is_empty());
    assert!(matrix.carrier_leaves().is_empty());
    let sites = execution_sites(source, expansion, view, MAX_FLOW_WORK).unwrap();
    let site = source_site(view, 4, Some(0));
    assert!(sites.contains(&transparent(site)));
    owner
        .source_query_for_root(source.roots()[0], view.body())
        .unwrap()
        .borrow_place_use(site, borrow_place(view, site), &mut || true)
        .unwrap();
    assert_unchanged_events(&owner, &sites);
}

#[test]
fn mixed_math_internal_owner_deaths_retain_the_exact_preterminal_ssa_kills() {
    // This crate can check correspondence, not the lowerer's internal-loan
    // rejection. These variants are not evidence from a failed Matrix lowering.
    for mutation in [
        MixedMutation117::MathOwnerDead,
        MixedMutation117::PolicyOwnerDead,
    ] {
        let owner = owner(mutation);
        let source = owner.source_semantic();
        let original = &source.functions()[0];
        assert_eq!(original.blocks()[4].statements().len(), 2);
        assert!(matches!(original.blocks()[4].statements()[1].kind(),
            SemanticStatementKindV1::StorageDead(local) if Some(local.index()) == mutation.dead_owner()));
        let view = owner.execution_view_for_root(source.roots()[0]).unwrap();
        let sites =
            execution_sites(source, owner.execution_expansion(), view, MAX_FLOW_WORK).unwrap();
        assert_unchanged_events(&owner, &sites);
        let (block, statement) = if mutation == MixedMutation117::MathOwnerDead {
            (3, 0)
        } else {
            (7, 1)
        };
        let origin_site = source_site(view, block, Some(statement));
        assert!(sites.contains(&transparent(origin_site)));
        let place = borrow_place(view, origin_site);
        let query = owner
            .source_query_for_root(source.roots()[0], view.body())
            .unwrap();
        let value = query
            .borrow_place_use(origin_site, place, &mut || true)
            .unwrap();
        let death = source_site(view, 4, Some(1));
        let rows = query.resolved_events_at(death, &mut || true).unwrap();
        assert!(
            matches!(rows, [(_, SsaResolvedEventV1::Kill { variable, previous: Some(previous) })]
            if variable.get() == place.local().index() && *previous == value)
        );
        let terminal = source_site(view, 4, None);
        assert_eq!(death.block(), terminal.block());
        let SemanticTerminatorKindV1::Call(call) = view.body().blocks()
            [terminal.block().index() as usize]
            .terminator()
            .kind()
        else {
            panic!("unchanged Math terminal")
        };
        assert_eq!(call.callee().index(), 8);
        assert!(matches!(
            &source.callables()[8],
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { .. },
                ..
            }
        ));
    }
}
