use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[test]
fn canonical_capture_cursor_preserves_every_constructor_getter_and_normal_edge() {
    let source = canonical_fixture::source(canonical_fixture::Mutation::None);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    expansion.verify_replay(&source).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, 65_536).unwrap();
    let mut cursor = facts.capture_cursor(&mut |_| Ok(())).unwrap();
    let mut foreign_nodes = facts.capture_cursor(&mut |_| Ok(())).unwrap();
    let mut counts = [0, 0];
    for (block, body) in view.body().blocks().iter().enumerate() {
        for (statement, source_node) in body.statements().iter().enumerate() {
            let site = SemanticTransparentBorrowSiteV1 {
                block: block as u32,
                statement: statement as u32,
            };
            let expected = facts
                .captured(site, source_node.kind(), &mut |_| Ok(()))
                .unwrap();
            assert_eq!(
                cursor
                    .captured(site, source_node.kind(), &mut |_| Ok(()))
                    .unwrap(),
                expected
            );
            assert!(
                foreign_nodes
                    .captured(site, &source_node.kind().clone(), &mut |_| Ok(()))
                    .unwrap()
                    .is_none()
            );
            if let Some(locals) = expected {
                assert!(!locals.is_empty());
                let SemanticStatementKindV1::Assign(assignment) = source_node.kind() else {
                    unreachable!()
                };
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Aggregate(_) => counts[0] += 1,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => {
                        assert_eq!(place.projections().len(), 2);
                        counts[1] += 1;
                    }
                    _ => panic!("unexpected registered capture"),
                }
            }
        }
    }
    assert_eq!(
        counts,
        [2, 1],
        "Bind, normal-return Narrow, and its exact getter"
    );
}

#[test]
fn canonical_capture_cursor_adds_no_record_for_missing_or_foreign_source_owners() {
    let source = canonical_fixture::source(canonical_fixture::Mutation::None);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let absent = MatrixBorrowSitesV1::new(&source, &expansion, view, &[], 65_536).unwrap();
    let mut cursor = absent
        .capture_cursor(&mut |_| panic!("empty inventory work"))
        .unwrap();
    for (block, body) in view.body().blocks().iter().enumerate() {
        for (statement, node) in body.statements().iter().enumerate() {
            assert!(
                cursor
                    .captured(
                        SemanticTransparentBorrowSiteV1 {
                            block: block as u32,
                            statement: statement as u32
                        },
                        node.kind(),
                        &mut |_| panic!("empty dispatch work")
                    )
                    .unwrap()
                    .is_none()
            );
        }
    }
    let other_expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    assert!(matches!(
        MatrixBorrowSitesV1::new(
            &source,
            &expansion,
            other_expansion.root(source.roots()[0]).unwrap(),
            &bindings,
            65_536
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}
