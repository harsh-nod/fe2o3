use super::*;

fn padded(mutation: Mutation, escape: Option<SemanticStatementKindV1>) -> SemanticFunctionDeclV1 {
    let original = fixture(mutation);
    let mut blocks = original.blocks().to_vec();
    for (index, body) in blocks.iter_mut().enumerate() {
        let mut statements = body.statements().to_vec();
        for _ in 0..64 {
            // Local13 is not the tracked subgroup or lane carrier.
            statements.extend([
                statement(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(13),
                )),
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(13),
                )),
                statement(SemanticStatementKindV1::Nop),
            ]);
        }
        if index == 1
            && let Some(kind) = &escape
        {
            statements.push(statement(kind.clone()));
        }
        *body = block(index as u8, statements, body.terminator().kind().clone());
    }
    function(
        180,
        original.abi().clone(),
        original.locals().to_vec(),
        blocks,
    )
}

#[test]
fn non_assignment_lane_dispatch_preserves_sparse_sites_and_existing_rejections() {
    let types = types();
    let callables = [access(true, 64, true)];
    for mutation in [
        Mutation::None,
        Mutation::Fork,
        Mutation::Field,
        Mutation::Mutable,
        Mutation::Escape,
        Mutation::Duplicate,
        Mutation::ParentEscape,
    ] {
        let expected = classify(&fixture(mutation), &types, &callables, MAX_FLOW_WORK).unwrap();
        let actual = classify(&padded(mutation, None), &types, &callables, MAX_FLOW_WORK).unwrap();
        assert_eq!(
            actual, expected,
            "neutral frame padding changed {mutation:?}"
        );
        assert_eq!(
            actual.contains(&root()),
            matches!(mutation, Mutation::None | Mutation::Fork)
        );
    }
}

#[test]
fn non_assignment_lane_dispatch_never_hides_a_parent_or_leaf_escape() {
    let types = types();
    let callables = [access(true, 64, true)];
    for (local, ty) in [(3, 4), (6, 7), (9, 9)] {
        for kind in [
            SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(place(local, ty))),
            SemanticStatementKindV1::Deinitialize(place(local, ty)),
        ] {
            let sites = classify(
                &padded(Mutation::None, Some(kind)),
                &types,
                &callables,
                MAX_FLOW_WORK,
            )
            .unwrap();
            assert!(
                !sites.contains(&root()),
                "non-assignment escape of local{local}"
            );
        }
    }
}
