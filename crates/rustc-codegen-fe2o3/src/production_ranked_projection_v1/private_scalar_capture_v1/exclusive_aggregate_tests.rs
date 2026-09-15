// Included beside the existing checked-owner fixtures.
#[derive(Clone, Copy)]
pub(in crate::production_ranked_projection_v1) enum ExclusiveAggregateChange {
    None,
    Write,
    RawEscape,
}

pub(in crate::production_ranked_projection_v1) fn exclusive_aggregate_owner(
    change: ExclusiveAggregateChange,
) -> ProductionSemanticSsaOwnerV1 {
    let baseline = capture_owner(Change::None).unwrap();
    let semantic = baseline.source_semantic();
    let mut types = semantic.types().to_vec();
    let integer = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(230)),
        SemanticLayoutIdentityV1::from_sha256(bytes(230)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            neutral_scalar_backend_v1(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                u64::MAX.into(),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    let aggregate = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(231)),
        SemanticLayoutIdentityV1::from_sha256(bytes(231)),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![integer, integer, UNIT, UNIT]).unwrap(),
        ),
    ));
    let reference = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(reference_type(
        232,
        aggregate,
        SemanticMutabilityV1::Mutable,
        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
        16,
        8,
    ));
    let raw = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(neutral_pointer_type_v1(
        233,
        aggregate,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Mutable,
        0,
    ));
    let original = &semantic.functions()[0];
    let first = original.locals().len() as u32;
    let mut locals = original.locals().to_vec();
    for (offset, ty) in [
        aggregate, reference, integer, reference, raw, integer, aggregate,
    ]
    .into_iter()
    .enumerate()
    {
        locals.push(local(
            230 + offset as u8,
            ty,
            SemanticLocalRoleV1::Temporary,
        ));
    }
    let marker = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))
    };
    let construct = || {
        typed_assignment(
            first,
            aggregate,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    typed_constant(integer, 64, 8),
                    typed_constant(integer, 31, 8),
                    marker(),
                    marker(),
                ],
            )
            .unwrap(),
        )
    };
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    statements.extend([
        construct(),
        typed_assignment(
            first + 1,
            reference,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(first, aggregate),
            },
        ),
    ]);
    match change {
        ExclusiveAggregateChange::None => {}
        ExclusiveAggregateChange::Write => statements.push(construct()),
        ExclusiveAggregateChange::RawEscape => statements.push(typed_assignment(
            first + 4,
            raw,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Pointer,
                operand: typed_operand(first + 1, reference),
            },
        )),
    }
    for (field, destination) in [(0, first + 2), (1, first + 5)] {
        statements.push(typed_assignment(
            destination,
            integer,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(first + 1),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, aggregate)
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), integer)
                            .unwrap(),
                    ],
                    integer,
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
    let admitted = InertSemanticMirRequestV1::new_with_callables(
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
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let mir = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        mir,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn private_exclusive_aggregate_original_owner_two_u64_fields_keep_source() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let before = owner.source_semantic().canonical_encoding().to_vec();
    assert_eq!(
        classified(&owner),
        3,
        "two private u64 reads plus the original independent scalar read"
    );
    assert_eq!(owner.source_semantic().canonical_encoding(), before);
    owner.verify_replay().unwrap();
}

#[test]
fn private_exclusive_aggregate_source_write_and_raw_escape_poison_both_reads() {
    for change in [
        ExclusiveAggregateChange::Write,
        ExclusiveAggregateChange::RawEscape,
    ] {
        let owner = exclusive_aggregate_owner(change);
        owner.verify_replay().unwrap();
        assert_eq!(
            classified(&owner),
            1,
            "the independent shared scalar read is unchanged"
        );
    }
}

#[test]
fn private_exclusive_aggregate_ranked_consumer_requires_exact_owner_and_statement() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let other = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let function = view.body();
    let types = owner.source_semantic().types();
    let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
    let foreign = PrivateScalarReads::for_root(&other, root).unwrap();
    let (block, read) = function
        .blocks()
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.statements()
                .iter()
                .find(|statement| {
                    reads.contains(function, statement)
                        && matches!(statement.kind(),
                SemanticStatementKindV1::Assign(assignment)
                if matches!(types[assignment.destination().ty().index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits: 64, .. })))
                })
                .map(|statement| (block, statement))
        })
        .unwrap();
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
    audit(read, Some(&reads)).unwrap();
    for result in [
        audit(read, None),
        audit(read, Some(&foreign)),
        audit(&read.clone(), Some(&reads)),
    ] {
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::UnrankedDereference(_))
        ));
    }
}
