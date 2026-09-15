use super::*;

#[path = "../../matrix_borrows_v1/canonical_fixture.rs"]
mod fixture;

fn source() -> (AdmittedInertSemanticMirV1, SemanticCallExpansionV1) {
    let source = fixture::source(fixture::Mutation::None);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    expansion.verify_replay(&source).unwrap();
    (source, expansion)
}

fn assignment_site(view: &SemanticExpandedRootV1) -> (u32, Site) {
    for (block, body) in view.body().blocks().iter().enumerate() {
        for (statement, node) in body.statements().iter().enumerate() {
            if let SemanticStatementKindV1::Assign(a) = node.kind()
                && matches!(a.value().kind(), SemanticRvalueKindV1::Aggregate(aggregate)
                    if aggregate.operands().len() == 4) {
                return (a.destination().local().index(), Site { block: block as u32, statement: statement as u32 });
            }
        }
    }
    panic!("missing fixture assignment");
}

fn selector(view: &SemanticExpandedRootV1, local: u32, site: Site) -> String {
    let identity: String = view.body().identity().as_bytes().iter()
        .map(|b| format!("{b:02x}")).collect();
    format!("{identity}/{local}/{}/{}", site.block, site.statement)
}

#[test]
fn carrier_site_selection_requires_the_exact_body_destination_and_statement() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let (local, site) = assignment_site(view);
    let raw = selector(view, local, site);
    assert_eq!(Observation::with_setting(Some(view), Some(&raw)).selected.unwrap().1, site);
    for raw in [None, Some(""), Some("1"), Some("not/a/valid/selector")] {
        assert!(Observation::with_setting(Some(view), raw).selected.is_none());
    }
    assert!(Observation::with_setting(None, Some(&raw)).selected.is_none());
    let mut wrong_body = raw.clone();
    wrong_body.replace_range(0..1, if &raw[..1] == "0" { "1" } else { "0" });
    for invalid in [wrong_body, format!("{raw}/0"), "0".repeat(98),
        selector(view, u32::MAX, site),
        selector(view, (local + 1) % view.body().locals().len() as u32, site),
        selector(view, local, Site { block: u32::MAX, statement: 0 }),
        selector(view, local, Site { statement: u32::MAX, ..site })] {
        assert!(Observation::with_setting(Some(view), Some(&invalid)).selected.is_none());
    }
}

#[test]
fn carrier_site_renders_retained_shape_and_candidates_without_proof_work() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let (local, site) = assignment_site(view);
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let matrix = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, MAX_FLOW_WORK).unwrap();
    let mut budget = Budget { limit: MAX_FLOW_WORK, remaining: MAX_FLOW_WORK, profile: Default::default() };
    let routes = math_capture_flow_v1::Routes::new_with_matrix(view.body(), Some(source.types()),
        source.callables(), &matrix, &mut budget).unwrap();
    let candidate = SemanticBorrowCandidateV1 {
        site, source_local: local, source_type: view.body().locals()[local as usize].ty(),
        source_reference: Some(local), value_alias: true,
        source_kind: SemanticBorrowCandidateSourceV1::TypedCarrier,
        valid: false, consumers: 2, intrinsic_consumer: false,
    };
    let by_reference = BTreeMap::from([(local, 0)]);
    let before = (budget.remaining, source.canonical_encoding().to_vec());
    let mut out = Output { bytes: [0; MAX_OUTPUT], len: 0, truncated: false };
    let checked_secondary_fields = [
        Some((math_capture_flow_v1::SecondaryRole::Policy, 1)),
        Some((math_capture_flow_v1::SecondaryRole::Workgroup, 17)),
    ];
    Observation::write(&mut out, view, site, "before-carrier-siblings", checked_secondary_fields, &routes,
        &by_reference, &[candidate], matrix.policy_carrier_leaves()).unwrap();
    let text = std::str::from_utf8(&out.bytes[..out.len]).unwrap();
    assert!(text.contains("stage=before-carrier-siblings checked_secondary_fields=[Some((Policy, 1)), Some((Workgroup, 17))]"));
    assert!(text.contains("aggregate_kind=Aggregate operand_count=4"));
    assert!(text.contains("operand_index=3"));
    assert!(text.contains("parent_index=Some(0) source_kind=TypedCarrier value_alias=true valid=false consumers=2"));
    assert!(text.contains("selected_route="));
    assert!(text.contains("shared_route="));
    assert!(text.contains("secondary_roles="));
    assert!(text.contains("checked-policy-pair reference="));
    assert!(text.contains("operand_prefix_truncated=false"));
    assert!(text.contains("type_details_truncated=false"));
    assert!(!out.truncated);
    assert_eq!(before, (budget.remaining, source.canonical_encoding().to_vec()));
    expansion.verify_replay(&source).unwrap();
}

#[test]
fn carrier_site_disabled_and_other_sites_do_not_consume_selection() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let (local, site) = assignment_site(view);
    let mut budget = Budget { limit: MAX_FLOW_WORK, remaining: MAX_FLOW_WORK, profile: Default::default() };
    let routes = math_capture_flow_v1::Routes::new(view.body(), None, &[], &mut budget).unwrap();
    let mut disabled = Observation::with_setting(Some(view), None);
    disabled.emit(site, "disabled", [None; 2], &routes, &BTreeMap::new(), &[], &BTreeMap::new());
    assert!(disabled.selected.is_none());
    let raw = selector(view, local, site);
    let mut enabled = Observation::with_setting(Some(view), Some(&raw));
    enabled.emit(Site { statement: site.statement + 1, ..site }, "other", [None; 2],
        &routes, &BTreeMap::new(), &[], &BTreeMap::new());
    assert_eq!(enabled.selected.unwrap().1, site);
    // A structurally equal clone is not the body retained by the route inventory.
    let clone = view.body().clone();
    let foreign = math_capture_flow_v1::Routes::new(&clone, None, &[], &mut budget).unwrap();
    enabled.emit(site, "foreign", [None; 2], &foreign, &BTreeMap::new(), &[], &BTreeMap::new());
    assert!(enabled.selected.is_none());
    assert_eq!(budget.remaining, MAX_FLOW_WORK);
}

#[test]
fn carrier_site_output_overflow_is_explicit_and_does_not_extend_the_buffer() {
    let mut out = Output { bytes: [0; MAX_OUTPUT], len: MAX_OUTPUT - 1, truncated: false };
    assert!(out.write_all(b"too long").is_err());
    assert!(out.truncated);
    assert_eq!(out.len, MAX_OUTPUT - 1);
}
