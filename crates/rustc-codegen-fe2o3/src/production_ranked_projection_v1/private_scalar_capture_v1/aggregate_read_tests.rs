// Include inside private_capture_tests so existing checked-owner fixtures and
// their exact ABI/type constructors are shared, not duplicated.

#[derive(Clone, Copy)]
pub(in crate::production_ranked_projection_v1) enum AggregateChange {
    None,
    Write,
    RawEscape,
    SharedResult,
    ScalarResult,
}

pub(in crate::production_ranked_projection_v1) fn aggregate_capture_owner(
    change: AggregateChange,
    field: u32,
) -> ProductionSemanticSsaOwnerV1 {
    let admitted = aggregate_capture_request(change, field, |_, _| {})
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .expect("the retained aggregate source must reach private-read analysis")
}

fn aggregate_capture_request(
    change: AggregateChange,
    field: u32,
    mutate_types: impl FnOnce(&mut [SemanticTypeDeclV1], SemanticTypeIdV1),
) -> InertSemanticMirRequestV1 {
    let baseline = capture_owner(Change::None).unwrap();
    let semantic = baseline.source_semantic();
    let mut types = semantic.types().to_vec();
    let integer = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(230)),
        SemanticLayoutIdentityV1::from_sha256(bytes(230)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            neutral_scalar_backend_v1(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                u32::MAX.into(),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    ));
    let dimension = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(231)),
        SemanticLayoutIdentityV1::from_sha256(bytes(231)),
        SemanticTypeLayoutV1::aggregate(
            Some(12),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![integer; 3]).unwrap()),
    ));
    let geometry = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(232)),
        SemanticLayoutIdentityV1::from_sha256(bytes(232)),
        SemanticTypeLayoutV1::aggregate(
            Some(48),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 12, 24, 36, 48, 48], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![
                dimension, dimension, dimension, dimension, UNIT, UNIT,
            ])
            .unwrap(),
        ),
    ));
    let reference = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(reference_type(
        233,
        geometry,
        SemanticMutabilityV1::Immutable,
        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
        48,
        4,
    ));
    let raw = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(neutral_pointer_type_v1(
        234,
        geometry,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Immutable,
        0,
    ));
    let original = &semantic.functions()[0];
    let mut locals = original.locals().to_vec();
    let first = locals.len() as u32;
    for (offset, ty) in [dimension, geometry, reference, dimension, raw, integer, REF]
        .into_iter()
        .enumerate()
    {
        locals.push(local(
            230 + offset as u8,
            ty,
            SemanticLocalRoleV1::Temporary,
        ));
    }
    let make_geometry = || {
        typed_assignment(
            first + 1,
            geometry,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    typed_operand(first, dimension),
                    typed_operand(first, dimension),
                    typed_operand(first, dimension),
                    typed_operand(first, dimension),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        UNIT,
                        SemanticConstantValueV1::ZeroSized,
                    )),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        UNIT,
                        SemanticConstantValueV1::ZeroSized,
                    )),
                ],
            )
            .unwrap(),
        )
    };
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    statements.extend([
        typed_assignment(
            first,
            dimension,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    typed_constant(integer, 1, 4),
                    typed_constant(integer, 2, 4),
                    typed_constant(integer, 3, 4),
                ],
            )
            .unwrap(),
        ),
        make_geometry(),
        typed_assignment(
            first + 2,
            reference,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(first + 1, geometry),
            },
        ),
    ]);
    match change {
        AggregateChange::Write => statements.push(make_geometry()),
        AggregateChange::RawEscape => statements.push(typed_assignment(
            first + 4,
            raw,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Pointer,
                operand: typed_operand(first + 2, reference),
            },
        )),
        _ => {}
    }
    if !matches!(change, AggregateChange::SharedResult) {
        let mut projections = vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, geometry).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), dimension).unwrap(),
        ];
        let (result, destination) = if matches!(change, AggregateChange::ScalarResult) {
            projections.push(
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(2), integer).unwrap(),
            );
            (integer, first + 5)
        } else {
            (dimension, first + 3)
        };
        statements.push(typed_assignment(
            destination,
            result,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(first + 2),
                    projections,
                    result,
                )
                .unwrap(),
            )),
        ));
    }
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    let mut functions = semantic.functions().to_vec();
    functions[0] = typed_global_fixture_with_body_v1(original, locals, blocks)
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    // The existing leaf already contains copy((*shared_env).field0) -> &f32;
    // retain that exact source instead of inserting an ill-typed root borrow.
    mutate_types(&mut types, geometry);
    InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

#[test]
fn private_aggregate_capture_scalar_and_three_component_fields_preserve_source() {
    for (change, field) in [
        (AggregateChange::None, 1),
        (AggregateChange::None, 2),
        (AggregateChange::None, 3),
        (AggregateChange::ScalarResult, 1),
    ] {
        let owner = aggregate_capture_owner(change, field);
        let before = owner.source_semantic().canonical_encoding().to_vec();
        assert_eq!(
            classified(&owner),
            2,
            "one existing scalar read plus one private field read"
        );
        assert_eq!(owner.source_semantic().canonical_encoding(), before);
        owner.verify_replay().unwrap();
    }
}

#[test]
fn private_aggregate_capture_write_and_raw_escape_do_not_authorize_field_read() {
    for change in [AggregateChange::Write, AggregateChange::RawEscape] {
        let owner = aggregate_capture_owner(change, 1);
        assert_eq!(
            classified(&owner),
            1,
            "only the original independent scalar read remains"
        );
    }
}

#[test]
fn private_aggregate_capture_reference_field_is_not_numeric_data() {
    let owner = aggregate_capture_owner(AggregateChange::SharedResult, 1);
    assert_eq!(classified(&owner), 1);
}

#[test]
fn private_aggregate_capture_requires_same_owner_function_and_statement() {
    let owner = aggregate_capture_owner(AggregateChange::None, 1);
    let other = aggregate_capture_owner(AggregateChange::None, 1);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let other_view = other.execution_view_for_root(root).unwrap();
    let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
    let statement = view.body().blocks().iter().flat_map(|block| block.statements())
        .find(|statement| reads.contains(view.body(), statement) && matches!(statement.kind(),
            SemanticStatementKindV1::Assign(assignment)
                if matches!(owner.source_semantic().types()[assignment.destination().ty().index() as usize].shape(), SemanticTypeShapeV1::Aggregate(_))
        )).unwrap();
    assert!(!reads.contains(view.body(), &statement.clone()));
    assert!(!reads.contains(other_view.body(), statement));
}

#[test]
fn private_aggregate_capture_ranked_audit_consumes_only_the_exact_private_read() {
    let owner = aggregate_capture_owner(AggregateChange::None, 2);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let function = view.body();
    let types = owner.source_semantic().types();
    let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
    let (block, read) = function.blocks().iter().enumerate().find_map(|(block, body)| {
        body.statements().iter().find(|statement| {
            reads.contains(function, statement) && matches!(statement.kind(),
                SemanticStatementKindV1::Assign(assignment)
                    if matches!(types[assignment.destination().ty().index() as usize].shape(), SemanticTypeShapeV1::Aggregate(_))
            )
        }).map(|statement| (block, statement))
    }).unwrap();
    let contracts = ProjectionLocalContractsV1 {
        checked_references: CheckedReferencesV1 {
            origins: vec![None; function.locals().len()],
            option_dominance: SemanticOptionDominanceV1::analyze(function, &[]).unwrap(),
            enum_payload_dominance: SemanticEnumPayloadDominanceV1::analyze(function, types)
                .unwrap(),
        },
        allocations: vec![None; function.locals().len()],
        allocation_provenance: vec![None; function.locals().len()],
    };
    let audit = |statement, certificate| {
        let mut operations = Vec::new();
        let mut sources = Vec::new();
        let result = project_statement_accesses(
            types,
            function,
            block,
            &[],
            statement,
            &ProjectedGlobalSemanticUsesV1::default(),
            certificate,
            &vec![None; function.locals().len()],
            &contracts,
            &[],
            &mut Vec::new(),
            &mut vec![None; function.locals().len()],
            &mut operations,
            &mut sources,
            &mut 0,
            &mut String::new(),
        );
        assert!(operations.is_empty() && sources.is_empty());
        result
    };
    assert!(matches!(
        audit(read, None),
        Err(ProductionRankedProjectionErrorV1::UnrankedDereference(_))
    ));
    audit(read, Some(&reads)).unwrap();
    assert!(matches!(
        audit(&read.clone(), Some(&reads)),
        Err(ProductionRankedProjectionErrorV1::UnrankedDereference(_))
    ));
}

#[test]
fn private_aggregate_capture_malformed_field_layout_is_rejected_before_analysis() {
    aggregate_capture_owner(AggregateChange::None, 1)
        .verify_replay()
        .unwrap();
    for offset in [1, 4] {
        let request = aggregate_capture_request(AggregateChange::None, 1, |types, geometry| {
            let old = &types[geometry.index() as usize];
            types[geometry.index() as usize] = SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                SemanticTypeLayoutV1::aggregate(
                    Some(48),
                    4,
                    SemanticAggregateLayoutV1::new(vec![0, offset, 24, 36, 48, 48], vec![])
                        .unwrap(),
                )
                .unwrap(),
                old.shape().clone(),
            );
        });
        // Offset 1 misaligns the selected dimension; offset 4 overlaps field0.
        assert!(
            request
                .admit_current_production(SemanticMirLimitsV1::default())
                .is_err()
        );
    }
}
