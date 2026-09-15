#[test]
fn ordinary_enum_alias_observation_reports_existing_transport_and_keeps_proof_state() {
    let fixture = Fixture::new(true);
    let (_, lowering) = fixture.emit().unwrap();
    let before = (
        lowering.enum_analysis_budget.work,
        lowering.enum_analysis_budget.storage,
        lowering.enum_payload_aliases.clone(),
    );
    let rows =
        lowering.enum_alias_failure_evidence_v1(SemanticBlockIdV1::from_index(6), local(3), 1, 0);
    assert!(
        rows.iter()
            .any(|s| s.contains("plan initialized=true selected_owner=Some(2)")),
        "{rows:?}"
    );
    assert!(
        rows.iter()
            .any(|s| s.contains("state local=3 ssa=true promoted=false")),
        "{rows:?}"
    );
    assert!(
        rows.iter()
            .any(|s| s.contains("state local=2 ssa=true promoted=true structural=true")),
        "{rows:?}"
    );
    assert!(
        rows.iter()
            .any(|s| s.contains("event local=2 block=1 statement=1 kind=constructor")),
        "{rows:?}"
    );
    assert!(rows.last().unwrap().contains("complete=true"));
    assert_eq!(
        before,
        (
            lowering.enum_analysis_budget.work,
            lowering.enum_analysis_budget.storage,
            lowering.enum_payload_aliases.clone()
        )
    );
}

#[test]
fn ordinary_enum_alias_observation_does_not_initialize_or_authorize_a_failed_alias() {
    let mut fixture = Fixture::new(true);
    let mut statements = fixture.function.blocks()[3].statements().to_vec();
    statements.push(some(2, constant(99)));
    fixture.replace_block(3, statements, goto(4));
    require_missing_payload(&fixture);
    let lowering = fixture.lowering();
    let before = (
        lowering.enum_analysis_budget.work,
        lowering.enum_analysis_budget.storage,
    );
    let rows =
        lowering.enum_alias_failure_evidence_v1(SemanticBlockIdV1::from_index(6), local(3), 1, 0);
    assert!(
        rows.iter()
            .any(|s| s.contains("plan initialized=false selected_owner=None"))
    );
    assert!(
        rows.iter()
            .any(|s| s.contains("event local=2 block=3 statement=1 kind=constructor")),
        "{rows:?}"
    );
    assert!(rows.last().unwrap().contains("complete=true"));
    assert!(lowering.enum_payload_aliases.is_none());
    assert_eq!(
        before,
        (
            lowering.enum_analysis_budget.work,
            lowering.enum_analysis_budget.storage
        )
    );
}

#[test]
fn ordinary_enum_alias_observation_visit_exhaustion_is_explicit_and_bounded() {
    let mut fixture = Fixture::new(false);
    fixture.replace_block(
        5,
        vec![SemanticStatementV1::new(source(), SemanticStatementKindV1::Nop); 8192],
        SemanticTerminatorKindV1::Return,
    );
    let lowering = fixture.lowering();
    let rows =
        lowering.enum_alias_failure_evidence_v1(SemanticBlockIdV1::from_index(6), local(3), 1, 0);
    assert!(rows.len() <= 25);
    assert!(rows.iter().all(|r| r.len() < 512));
    assert!(
        rows.last().unwrap().contains("complete=false visits=8192"),
        "{rows:?}"
    );
    assert!(lowering.enum_payload_aliases.is_none());
}
