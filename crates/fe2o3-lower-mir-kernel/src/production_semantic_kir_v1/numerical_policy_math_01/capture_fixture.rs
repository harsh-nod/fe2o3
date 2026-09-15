pub(super) fn captured_source(
    reborrow: bool,
    kill_math: bool,
    forged_bind: bool,
    projected_move: bool,
) -> AdmittedInertSemanticMirV1 {
    let base = full_source(reborrow, kill_math);
    let mut types = base.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([221; 32]),
        SemanticLayoutIdentityV1::from_sha256([221; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 12], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![ty(10), ty(11), ty(11)]).unwrap(),
        ),
    ));
    let root = &base.functions()[0];
    let mut locals = root.locals().to_vec();
    locals.push(local(192, 12, SemanticLocalRoleV1::Temporary));
    locals.push(local(193, 12, SemanticLocalRoleV1::Temporary));
    // Keep the receiver distinct from local 10 in the earlier reborrow chain.
    locals.push(local(194, 10, SemanticLocalRoleV1::Temporary));
    let assign = |destination, value| {
        SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(destination, value)),
        )
    };
    let scalar = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(11),
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(1.0_f32.to_bits().into(), 4).unwrap(),
            ),
        ))
    };
    let mut statements = root.blocks()[4].statements().to_vec();
    if forged_bind {
        statements.insert(
            0,
            assign(
                place(7, 9),
                SemanticRvalueV1::new(
                    ty(9),
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Aggregate,
                        vec![
                            SemanticOperandV1::Copy(place(5, 6)),
                            SemanticOperandV1::Copy(place(6, 7)),
                            SemanticOperandV1::Constant(SemanticConstantV1::new(
                                ty(8),
                                SemanticConstantValueV1::ZeroSized,
                            )),
                        ],
                    )
                    .unwrap(),
                ),
            ),
        );
    }
    statements.push(assign(
        place(12, 12),
        SemanticRvalueV1::new(
            ty(12),
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    SemanticOperandV1::Copy(place(if reborrow { 11 } else { 8 }, 10)),
                    scalar(),
                    scalar(),
                ],
            )
            .unwrap(),
        ),
    ));
    let mut blocks = root.blocks().to_vec();
    blocks[4] = block(
        184,
        statements,
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(6),
        )),
    );
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(13),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(10)).unwrap()],
        ty(10),
    )
    .unwrap();
    blocks.push(block(
        186,
        vec![
            assign(
                place(13, 12),
                SemanticRvalueV1::new(
                    ty(12),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(12, 12))),
                ),
            ),
            SemanticStatementV1::new(
                location(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(12)),
            ),
            assign(
                place(14, 10),
                SemanticRvalueV1::new(
                    ty(10),
                    SemanticRvalueKindV1::Use(if projected_move {
                        SemanticOperandV1::Move(field)
                    } else {
                        SemanticOperandV1::Copy(field)
                    }),
                ),
            ),
        ],
        call(
            7,
            vec![SemanticOperandV1::Copy(place(14, 10)), scalar()],
            9,
            11,
            5,
        ),
    ));
    let mut functions = base.functions().to_vec();
    functions[0] = function(135, root.abi().clone(), locals, blocks, true)
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new_with_callables(
        base.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        base.callables().to_vec(),
        base.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v21(SemanticMirLimitsV1::default())
    .unwrap()
}
