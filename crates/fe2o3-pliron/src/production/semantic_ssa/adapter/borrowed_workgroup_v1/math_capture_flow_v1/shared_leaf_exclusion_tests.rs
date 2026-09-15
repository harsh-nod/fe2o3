use super::*;

// Private matcher tests. Reuse the canonical fixture and checked route discovery;
// modified types/assignments below are not admitted source or issuance evidence.
fn field(local: u32, index: u32, result: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty(result)).unwrap(),
        ],
        ty(result),
    )
    .unwrap()
}

#[test]
fn projected_move_cannot_extract_a_subcarrier_even_when_its_copy_route_matches() {
    let source = fixture::captured_source(false, false, false, false);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let original = expansion.root(source.roots()[0]).unwrap().body();
    let mut types = source.types().to_vec();
    assert_eq!(types.len(), 13);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([251; 32]),
        SemanticLayoutIdentityV1::from_sha256([251; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(12)]).unwrap()),
    ));
    let mut locals = original.locals().to_vec();
    let local = &locals[12];
    locals[12] = SemanticLocalDeclV1::new(local.identity(), ty(13), local.role(), local.source());
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
    assert_eq!(routes.routes.get(&ty(13)).unwrap().len, 2);
    assert_eq!(routes.routes.get(&ty(12)).unwrap().len, 1);
    for moved in [false, true] {
        let projected = field(12, 0, 12);
        let assignment = SemanticAssignmentV1::new(
            place(13, 12),
            SemanticRvalueV1::new(
                ty(12),
                SemanticRvalueKindV1::Use(if moved {
                    SemanticOperandV1::Move(projected)
                } else {
                    SemanticOperandV1::Copy(projected)
                }),
            ),
        );
        let result = routes
            .source(&assignment, &mut budget(MAX_FLOW_WORK))
            .unwrap();
        assert_eq!(result.is_some(), !moved, "moved={moved}");
    }
}

#[test]
fn aggregate_capture_cannot_hide_a_projected_leaf_move() {
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
    let aggregate = body
        .blocks()
        .iter()
        .flat_map(|b| b.statements())
        .find_map(|s| {
            let SemanticStatementKindV1::Assign(a) = s.kind() else {
                return None;
            };
            let SemanticRvalueKindV1::Aggregate(aggregate) = a.value().kind() else {
                return None;
            };
            (a.destination().local().index() == 12 && a.destination().ty() == ty(12))
                .then_some(aggregate)
        })
        .unwrap();
    for moved in [false, true] {
        let mut operands = aggregate.operands().to_vec();
        let projected = field(13, 0, 10);
        operands[0] = if moved {
            SemanticOperandV1::Move(projected)
        } else {
            SemanticOperandV1::Copy(projected)
        };
        let assignment = SemanticAssignmentV1::new(
            place(12, 12),
            SemanticRvalueV1::new(
                ty(12),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(aggregate.kind().clone(), operands).unwrap(),
                ),
            ),
        );
        let result = routes
            .source(&assignment, &mut budget(MAX_FLOW_WORK))
            .unwrap();
        assert_eq!(result.is_some(), !moved, "moved={moved}");
    }
}

#[test]
fn shared_leaf_move_rechecks_pointer_shape_even_with_an_existing_route_key() {
    let source = fixture::captured_source(false, false, false, true);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let body = expansion.root(source.roots()[0]).unwrap().body();
    let routes = Routes::new(
        body,
        Some(source.types()),
        source.callables(),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let assignment = body.blocks()[receiver_block(body)]
        .statements()
        .iter()
        .find_map(|s| match s.kind() {
            SemanticStatementKindV1::Assign(a) if a.destination().local().index() == 14 => Some(a),
            _ => None,
        })
        .unwrap();
    assert_eq!(routes.owned(ty(10)), Some(&ty(9)));
    assert!(
        routes
            .source(assignment, &mut budget(MAX_FLOW_WORK))
            .unwrap()
            .is_some()
    );
    use SemanticMutabilityV1::{Immutable, Mutable};
    use SemanticPointerKindV1::{Raw, Reference};
    use SemanticPointerMetadataV1::{None as Thin, SliceLength, VTable};
    for (name, kind, mutability, space, bits, metadata, pointee) in [
        ("mutable", Reference, Mutable, 0, 64, Thin, 9),
        ("raw", Raw, Immutable, 0, 64, Thin, 9),
        (
            "slice-metadata",
            Reference,
            Immutable,
            0,
            64,
            SliceLength,
            9,
        ),
        ("vtable-metadata", Reference, Immutable, 0, 64, VTable, 9),
        ("nonzero-space", Reference, Immutable, 1, 64, Thin, 9),
        ("wrong-width", Reference, Immutable, 0, 32, Thin, 9),
        ("foreign-pointee", Reference, Immutable, 0, 64, Thin, 11),
    ] {
        let mut types = source.types().to_vec();
        let original = &types[10];
        types[10] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            original.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(pointee),
                    kind,
                    mutability,
                    space,
                    bits,
                    metadata,
                )
                .unwrap(),
            ),
        );
        // Keep the key present to test the new guard, not an earlier missing route.
        let changed = Routes {
            function: body,
            types: &types,
            routes: routes.routes.clone(),
            shared: routes.shared.clone(),
            secondaries: routes.secondaries.clone(),
            failed_secondaries: routes.failed_secondaries.clone(),
        };
        assert_eq!(changed.owned(ty(10)), Some(&ty(9)));
        assert!(
            changed
                .source(assignment, &mut budget(MAX_FLOW_WORK))
                .unwrap()
                .is_none(),
            "{name}"
        );
    }
}
