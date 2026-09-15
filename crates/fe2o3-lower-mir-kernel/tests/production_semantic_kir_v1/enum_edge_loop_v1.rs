use super::*;

fn safe_enum_loop_owner_v1() -> ProductionSemanticMirOwnerV1 {
    let template = promoted_enum_payload_owner(false, false, 1);
    let types = template.semantic().types().to_vec();
    let mut locals = template.semantic().functions()[0].locals().to_vec();
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let enum_ty = SemanticTypeIdV1::from_index(3);
    assert_eq!(locals[0].ty(), unit);
    // Keep the existing exact type catalog closed after replacing its Boolean entry switch.
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(107)),
        bool_ty,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    drop(template);
    let assign = |destination, ty, value| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let constructor = |variant, payload| {
        assign(
            local_place(1, enum_ty),
            enum_ty,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::EnumVariant(variant),
                vec![scalar_constant(u32_ty, payload, 4)],
            )
            .unwrap(),
        )
    };
    let read_payload = |variant, destination| {
        let source = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(variant), enum_ty)
                    .unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), u32_ty).unwrap(),
            ],
            u32_ty,
        )
        .unwrap();
        assign(
            local_place(destination, u32_ty),
            u32_ty,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)),
        )
    };
    let blocks = vec![
        block(
            120,
            vec![constructor(0, 10)],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            121,
            vec![assign(
                local_place(2, u32_ty),
                u32_ty,
                SemanticRvalueKindV1::Discriminant(local_place(1, enum_ty)),
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(local_place(2, u32_ty)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![
                        SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue, 2)),
                        SemanticSwitchTargetV1::new(1, edge(SemanticEdgeRoleV1::SwitchValue, 3)),
                    ],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
                )
                .unwrap(),
            },
        ),
        block(
            122,
            vec![read_payload(0, 3), constructor(1, 20)],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            123,
            vec![read_payload(1, 4)],
            SemanticTerminatorKindV1::Return,
        ),
        block(124, vec![], SemanticTerminatorKindV1::Unreachable),
    ];
    owner_from_parts(types, locals, 0, blocks, b"semantic_safe_enum_loop_test")
}

#[test]
fn admitted_enum_loop_keeps_exact_runtime_discriminant_and_both_payload_paths() {
    let owner = safe_enum_loop_owner_v1();
    owner.verify_equivalence().unwrap();
    let lowered =
        ProductionSemanticKirOwnerV1::try_lower(owner, ProductionSemanticKirLimitsV1::default())
            .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let block = |id| {
        body.blocks
            .iter()
            .find(|block| block.id == BlockId(id))
            .unwrap()
    };
    let header = block(1);
    let [parameter] = header.parameters.as_slice() else {
        panic!("the enum loop header must carry its exact discriminator phi");
    };
    assert_eq!(parameter.ty, Type::Scalar(ScalarType::U32));
    let Some(Terminator::Switch {
        selector,
        cases,
        default_target,
        ..
    }) = &header.terminator
    else {
        panic!("the enum loop header must retain its runtime switch");
    };
    assert_eq!(*selector, parameter.id);
    assert_eq!(
        cases
            .iter()
            .map(|case| (case.value, case.target))
            .collect::<Vec<_>>(),
        vec![(0, BlockId(2)), (1, BlockId(3))]
    );
    assert_eq!(*default_target, BlockId(4));
    for (source, discriminant) in [(0, 0), (2, 1)] {
        let predecessor = block(source);
        let Some(Terminator::Branch { target, arguments }) = &predecessor.terminator else {
            panic!("entry and backedge must reach the discriminator phi");
        };
        assert_eq!(*target, BlockId(1));
        let [argument] = arguments.as_slice() else {
            panic!("entry and backedge must each supply one enum discriminator");
        };
        assert!(predecessor.operations.iter().any(|operation| {
            matches!(operation.results.as_slice(), [result] if result.id == *argument)
                && matches!(operation.kind, OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(value)) if value == discriminant)
        }));
    }
    assert!(matches!(
        block(3).terminator,
        Some(Terminator::Return { .. })
    ));
}
