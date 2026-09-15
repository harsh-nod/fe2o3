use super::*;

fn projected(local: u32, path: &[(u32, u32)], result: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        path.iter()
            .map(|&(index, result)| {
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty(result))
                    .unwrap()
            })
            .collect(),
        ty(result),
    )
    .unwrap()
}

fn assignment(place: SemanticPlaceV1, moved: bool) -> SemanticAssignmentV1 {
    SemanticAssignmentV1::new(
        super::place(9, place.ty().index()),
        SemanticRvalueV1::new(
            place.ty(),
            SemanticRvalueKindV1::Use(if moved {
                SemanticOperandV1::Move(place)
            } else {
                SemanticOperandV1::Copy(place)
            }),
        ),
    )
}

#[test]
fn disjoint_copy_and_move_share_exact_types_and_existing_work_charge() {
    let source = fixture::captured_source(false, false, false, false);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let body = expansion.root(source.roots()[0]).unwrap().body();
    let routes = Routes::new(
        body,
        Some(source.types()),
        source.callables(),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert_eq!(
        fields(source.types(), ty(12)),
        Some([ty(10), ty(11), ty(11)].as_slice())
    );
    for moved in [false, true] {
        let valid = assignment(projected(13, &[(1, 11)], 11), moved);
        let mut measured = budget(MAX_FLOW_WORK);
        assert!(routes.disjoint_field_use(&valid, &mut measured).unwrap());
        let required = MAX_FLOW_WORK - measured.remaining;
        assert_eq!(
            required, 2,
            "one typed field and root lookup, moved={moved}"
        );
        assert!(
            routes
                .disjoint_field_use(&valid, &mut budget(required))
                .unwrap()
        );
        assert!(matches!(
            routes.disjoint_field_use(&valid, &mut budget(required - 1)),
            Err(ProductionSemanticSsaErrorV1::BorrowFlowWork { .. })
        ));
        assert!(
            routes
                .disjoint_field_use(
                    &assignment(projected(13, &[(2, 11)], 11), moved),
                    &mut budget(MAX_FLOW_WORK),
                )
                .unwrap()
        );
        for rejected in [
            projected(13, &[], 12),
            projected(13, &[(0, 10)], 10),
            projected(13, &[(1, 10)], 10),
            projected(13, &[(3, 11)], 11),
            projected(u32::MAX, &[(1, 11)], 11),
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(13),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(11))
                        .unwrap(),
                ],
                ty(11),
            )
            .unwrap(),
        ] {
            assert!(
                !routes
                    .disjoint_field_use(&assignment(rejected, moved), &mut budget(MAX_FLOW_WORK))
                    .unwrap()
            );
        }
        let mismatched = SemanticAssignmentV1::new(
            place(9, 10),
            SemanticRvalueV1::new(ty(10), valid.value().kind().clone()),
        );
        assert!(
            !routes
                .disjoint_field_use(&mismatched, &mut budget(MAX_FLOW_WORK))
                .unwrap()
        );
    }
}

#[test]
fn nested_paths_require_typed_divergence_not_scalar_or_reference_shape_authority() {
    let source = fixture::captured_source(false, false, false, false);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let original = expansion.root(source.roots()[0]).unwrap().body();
    // Private path-predicate mutations only, not admitted source or new issuers.
    // The ordinary sibling can be a scalar or an unrelated reference.
    for sibling in [11, 2] {
        let mut types = source.types().to_vec();
        assert_eq!(types.len(), 13);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([252; 32]),
            SemanticLayoutIdentityV1::from_sha256([252; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(24),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![ty(12), ty(sibling)]).unwrap(),
            ),
        ));
        let mut locals = original.locals().to_vec();
        let old = &locals[12];
        locals[12] = SemanticLocalDeclV1::new(old.identity(), ty(13), old.role(), old.source());
        let body = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original.abi().clone(),
            locals,
            original.entry(),
            original.blocks().to_vec(),
        )
        .unwrap();
        let routes = Routes::new(
            &body,
            Some(&types),
            source.callables(),
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        let route = routes.routes.get(&ty(13)).unwrap();
        assert_eq!(&route.fields[..route.len], &[0, 0]);
        for moved in [false, true] {
            for (path, result, expected) in [
                (vec![], 13, false),
                (vec![(0, 12)], 12, false),
                (vec![(0, 12), (0, 10)], 10, false),
                (vec![(0, 12), (1, 11)], 11, true),
                (vec![(1, sibling)], sibling, true),
                (vec![(0, 12), (1, 10)], 10, false),
                (vec![(1, sibling), (0, 11)], 11, false),
            ] {
                let value = assignment(projected(12, &path, result), moved);
                assert_eq!(
                    routes
                        .disjoint_field_use(&value, &mut budget(MAX_FLOW_WORK))
                        .unwrap(),
                    expected,
                    "sibling={sibling} moved={moved} path={path:?}"
                );
                if expected {
                    assert!(
                        routes
                            .source(&value, &mut budget(MAX_FLOW_WORK))
                            .unwrap()
                            .is_none(),
                        "the sibling receives no tracked leaf edge"
                    );
                }
            }
        }
    }
}

#[test]
fn disjoint_sibling_use_does_not_hide_a_destination_write_into_the_carrier() {
    let source = fixture::captured_source(false, false, false, false);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let body = expansion.root(source.roots()[0]).unwrap().body();
    let (site, _) = bound_borrow(body);
    assert!(classify(&source, &expansion, body).contains(&site));
    let block = receiver_block(body);
    for moved in [false, true] {
        let mut statements = body.blocks()[block].statements().to_vec();
        let sibling = projected(13, &[(1, 11)], 11);
        statements.push(assign(
            sibling.clone(),
            SemanticRvalueKindV1::Use(if moved {
                SemanticOperandV1::Move(sibling)
            } else {
                SemanticOperandV1::Copy(sibling)
            }),
        ));
        let changed = changed_block(
            body,
            block,
            statements,
            body.blocks()[block].terminator().kind().clone(),
        );
        assert!(
            !classify(&source, &expansion, &changed).contains(&site),
            "moved={moved}"
        );
    }
}
