use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticRvalueV1;
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/numerical_policy_math_01/ssa_fixture.rs"
    ));
}

#[test]
fn math_bind_sink_requires_exact_replayed_site_and_original_operands() {
    let source = fixture::full_source(true, false);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let facts = MathBorrowSitesV1::new(&source, view, &bindings, 4096).unwrap();
    assert_eq!(facts.binds.len(), 1);
    assert_eq!(facts.pairs.len(), 3);
    let (site, (assignment, locals)) = facts.binds.first_key_value().unwrap();
    assert_ne!(locals[0], locals[1]);
    assert_eq!(
        facts.captured(*site, &SemanticStatementKindV1::Assign(assignment.clone())),
        Some(*locals)
    );
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        panic!()
    };
    let mut operands = aggregate.operands().to_vec();
    operands.swap(0, 1);
    let changed = SemanticAssignmentV1::new(
        assignment.destination().clone(),
        SemanticRvalueV1::new(
            assignment.value().result_type(),
            SemanticRvalueKindV1::aggregate(aggregate.kind().clone(), operands).unwrap(),
        ),
    );
    assert!(
        facts
            .captured(*site, &SemanticStatementKindV1::Assign(changed))
            .is_none()
    );
    assert!(
        facts
            .captured(
                SemanticTransparentBorrowSiteV1 {
                    block: site.block,
                    statement: site.statement + 1
                },
                &SemanticStatementKindV1::Assign(assignment.clone())
            )
            .is_none()
    );
    assert!(
        MathBorrowSitesV1::new(&source, view, &[], 4096)
            .unwrap()
            .binds
            .is_empty()
    );
}

#[test]
fn math_borrow_roster_rejects_foreign_expansion_and_enforces_exact_work_limit() {
    let source = fixture::full_source(false, false);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let required = bindings.len() * 32;
    assert!(MathBorrowSitesV1::new(&source, view, &bindings, required).is_ok());
    assert!(matches!(
        MathBorrowSitesV1::new(&source, view, &bindings, required - 1),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit { .. })
    ));
    let other = fixture::full_source(true, false);
    let other_expansion = SemanticCallExpansionV1::try_new(&other, Default::default()).unwrap();
    assert!(
        MathBorrowSitesV1::new(
            &source,
            other_expansion.root(other.roots()[0]).unwrap(),
            &bindings,
            4096
        )
        .is_err()
    );
}

#[test]
fn math_consumer_transparency_is_closed_to_actual_fp32_receiver() {
    let source = fixture::full_source(false, false);
    let consumer =
        MathConsumerBorrowV1::for_callable(source.types(), &source.callables()[7]).unwrap();
    let SemanticTerminatorKindV1::Call(call) =
        source.functions()[0].blocks()[4].terminator().kind()
    else {
        panic!()
    };
    assert!(consumer.accepts(call, 0, consumer.pair().1));
    assert!(!consumer.accepts(call, 1, consumer.pair().1));
    assert!(!consumer.accepts(call, 0, consumer.pair().0));
    for callable in &source.callables()[..7] {
        assert!(MathConsumerBorrowV1::for_callable(source.types(), callable).is_none());
    }
}
