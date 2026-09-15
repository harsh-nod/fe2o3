use super::super::super::super::{execution_sites, semantic_function_ssa_input_v1};
use super::*;

#[test]
fn captured_policy_has_real_borrow_use_without_rewriting_source_events() {
    let source = fixture::source(Mutation::None);
    let expansion = expand(&source);
    let view = &expansion.roots()[0];
    let sites = execution_sites(&source, &expansion, view, MAX_FLOW_WORK).unwrap();
    let borrow = SemanticTransparentBorrowSiteV1 {
        block: 3,
        statement: 1,
    };
    assert!(sites.contains(&borrow));
    let SemanticStatementKindV1::Assign(a) = view.body().blocks()[3].statements()[1].kind() else {
        panic!()
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = a.value().kind()
    else {
        panic!()
    };
    assert_eq!(place.ty(), ty(3));
    let local = place.local().index() as usize;
    let (with, _, _) = semantic_function_ssa_input_v1(
        view.body(),
        Some(source.types()),
        source.callables(),
        &sites,
    );
    let (without, _, _) = semantic_function_ssa_input_v1(
        view.body(),
        Some(source.types()),
        source.callables(),
        &BTreeSet::new(),
    );
    assert!(with.promotable()[local]);
    assert!(!without.promotable()[local]);
    assert_eq!(
        with.blocks(),
        without.blocks(),
        "original Copy, Move, Kill and call-transfer events are unchanged"
    );
    expansion.verify_replay(&source).unwrap();
}

#[test]
fn complete_policy_component_rejects_duplicate_deinitialize_and_moved_subcarrier() {
    for mutation in [
        Mutation::DuplicateCapture,
        Mutation::DeinitializeCapture,
        Mutation::MoveSubcarrier,
    ] {
        let source = fixture::source(mutation);
        let expansion = expand(&source);
        let view = &expansion.roots()[0];
        let sites = execution_sites(&source, &expansion, view, MAX_FLOW_WORK).unwrap();
        assert!(
            !sites.contains(&SemanticTransparentBorrowSiteV1 {
                block: 3,
                statement: 1
            }),
            "{mutation:?}"
        );
        expansion.verify_replay(&source).unwrap();
    }
}
