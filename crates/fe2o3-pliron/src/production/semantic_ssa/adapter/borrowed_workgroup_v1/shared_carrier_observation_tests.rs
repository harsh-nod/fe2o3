use super::super::super::all_use_observation_v1::Observation;
use super::*;
use std::ffi::OsStr;

fn observed_nodes(view: &SemanticExpandedRootV1) -> Vec<SemanticBorrowCandidateV1> {
    // Observation inputs only. Production acceptance below comes independently
    // from execution_sites and the original owner, never from these records.
    fixture::original_borrows(view.body())
        .into_iter()
        .map(|(site, place)| SemanticBorrowCandidateV1 {
            site,
            source_local: place.local().index(),
            source_type: ty(4),
            source_reference: Some(place.local().index()),
            value_alias: false, source_kind: SemanticBorrowCandidateSourceV1::Direct,
            valid: false,
            consumers: 0,
            intrinsic_consumer: false,
        })
        .collect()
}

#[test]
fn retained_carrier_routes_and_absent_candidate_are_distinct_observations() {
    let source = fixture::admitted();
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    expansion.verify_replay(&source).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let mut work = budget(MAX_FLOW_WORK);
    let routes = math_capture_flow_v1::Routes::new(
        view.body(),
        Some(source.types()),
        source.callables(),
        &mut work,
    )
    .unwrap();
    let remaining = work.remaining;
    let selected = execution_sites(&source, &expansion, view, MAX_FLOW_WORK).unwrap();
    let original = source.canonical_encoding().to_vec();
    for include_nodes in [false, true] {
        let nodes = if include_nodes {
            observed_nodes(view)
        } else {
            vec![]
        };
        let observation = Observation::with_workgroup_role(
            Some(view),
            source.callables(),
            &nodes,
            Some(OsStr::new("1")),
            Some(OsStr::new("workgroup")),
        );
        let mut out = Vec::new();
        observation
            .write_with_routes(
                &mut out,
                view,
                &nodes,
                &selected,
                |ty| routes.owned(ty).copied(),
                Some((&routes, &BTreeMap::new())),
            )
            .unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("scope=workgroup"));
        assert!(out.contains("workgroup-borrow-census same_body=true"));
        assert!(out.contains("carrier-type ty=11 selected_route=None shared_route=Some((10,"));
        assert!(out.contains("carrier-fields count=2 prefix=[SemanticTypeIdV1(5), SemanticTypeIdV1(1)] truncated=false"));
        assert!(out.contains("shared-borrow-source block=Some("));
        assert!(out.contains("scan_complete=true truncated=false"));
        if include_nodes {
            assert!(out.contains("candidate_count=1 first_candidate=Some("));
            assert!(out.contains("initial_valid=false final_valid=false"));
            assert!(nodes.iter().all(|node| !node.valid && node.consumers == 0));
        } else {
            assert!(out.contains("candidate_count=0 first_candidate=None"));
        }
    }
    assert_eq!(work.remaining, remaining);
    assert_eq!(source.canonical_encoding().to_vec(), original);
    assert_eq!(
        execution_sites(&source, &expansion, view, MAX_FLOW_WORK).unwrap(),
        selected
    );
    let owner = owner(source).unwrap();
    owner.verify_replay().unwrap();
    let view = owner
        .execution_view_for_root(owner.source_semantic().roots()[0])
        .unwrap();
    let query = owner
        .source_query_for_root(view.root(), view.body())
        .unwrap();
    for (site, place) in fixture::original_borrows(view.body()) {
        let site = ProductionSemanticSsaSourceSiteV1::new(
            SemanticBlockIdV1::from_index(site.block),
            Some(site.statement),
        );
        assert!(query.borrow_place_use(site, place, &mut || true).is_ok());
        assert!(matches!(
            query.borrow_place_use(site, &place.clone(), &mut || true),
            Err(ProductionSemanticSsaSourceQueryErrorV1::OperandOutsideSite)
        ));
    }
}

#[test]
fn absent_seed_is_reported_without_fabricating_a_selected_owner() {
    let source = fixture::admitted();
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let mut work = budget(MAX_FLOW_WORK);
    let routes =
        math_capture_flow_v1::Routes::new(view.body(), Some(source.types()), &[], &mut work)
            .unwrap();
    let observation = Observation::with_workgroup_role(
        Some(view),
        &[],
        &[],
        Some(OsStr::new("1")),
        Some(OsStr::new("workgroup")),
    );
    let mut out = Vec::new();
    observation
        .write_with_routes(
            &mut out,
            view,
            &[],
            &BTreeSet::new(),
            |_| None,
            Some((&routes, &BTreeMap::new())),
        )
        .unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("selected=0 owned_types=[]"));
    assert!(out.contains("carrier-type ty=11 selected_route=None shared_route=None"));
    assert!(out.contains("candidate_count=0 first_candidate=None"));
    assert!(!out.contains("accepted_borrow=true"));
}

#[test]
fn disabled_or_foreign_route_observation_cannot_inspect_another_body() {
    let source = fixture::admitted();
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let other = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let foreign = other.root(source.roots()[0]).unwrap();
    assert!(!std::ptr::eq(view.body(), foreign.body()));
    let mut work = budget(MAX_FLOW_WORK);
    let routes = math_capture_flow_v1::Routes::new(
        view.body(),
        Some(source.types()),
        source.callables(),
        &mut work,
    )
    .unwrap();
    for (setting, role) in [
        (None, Some("workgroup")),
        (Some("0"), Some("workgroup")),
        (Some("true"), Some("workgroup")),
        (Some("1"), None),
        (Some("1"), Some("matrix")),
    ] {
        let observation = Observation::with_workgroup_role(
            Some(view),
            source.callables(),
            &[],
            setting.map(OsStr::new),
            role.map(OsStr::new),
        );
        observation.emit_with_routes(
            view.body(),
            &[],
            &BTreeSet::new(),
            |_| panic!("disabled route lookup"),
            &routes,
            &BTreeMap::new(),
        );
        let mut out = Vec::new();
        observation
            .write_with_routes(
                &mut out,
                view,
                &[],
                &BTreeSet::new(),
                |_| None,
                Some((&routes, &BTreeMap::new())),
            )
            .unwrap();
        assert!(
            !String::from_utf8(out)
                .unwrap()
                .contains("workgroup-borrow-census")
        );
    }
    let observation = Observation::with_workgroup_role(
        Some(foreign),
        source.callables(),
        &[],
        Some(OsStr::new("1")),
        Some(OsStr::new("workgroup")),
    );
    let mut out = Vec::new();
    observation
        .write_with_routes(
            &mut out,
            foreign,
            &[],
            &BTreeSet::new(),
            |_| None,
            Some((&routes, &BTreeMap::new())),
        )
        .unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("workgroup-borrow-census same_body=false"));
    assert!(!out.contains("shared-borrow site="));
    assert!(!out.contains("carrier-type"));
}

#[test]
fn type_observation_charges_before_lookup_and_never_spends_proof_work() {
    let source = fixture::admitted();
    let mut work = budget(MAX_FLOW_WORK);
    let routes = math_capture_flow_v1::Routes::new(
        &source.functions()[0],
        Some(source.types()),
        source.callables(),
        &mut work,
    )
    .unwrap();
    let proof_remaining = work.remaining;
    let mut out = Vec::new();
    let mut remaining = 1000;
    assert!(
        routes
            .write_type_observation(&mut out, ty(CARRIER_REF), &mut remaining)
            .unwrap()
    );
    let required = 1000 - remaining;
    assert!(required > 0);
    let expected = out;
    for allowance in [required - 1, required] {
        let mut out = Vec::new();
        let mut remaining = allowance;
        assert_eq!(
            routes
                .write_type_observation(&mut out, ty(CARRIER_REF), &mut remaining)
                .unwrap(),
            allowance == required
        );
        assert_eq!(remaining, 0);
        if allowance == required {
            assert_eq!(out, expected);
        } else {
            assert!(out.is_empty());
        }
    }
    assert_eq!(work.remaining, proof_remaining);
}
