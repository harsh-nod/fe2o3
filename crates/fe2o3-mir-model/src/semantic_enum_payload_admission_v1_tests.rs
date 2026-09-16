use super::*;
use crate::semantic_mir_v1::*;

fn payload_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn payload_targets(cases: &[(u128, u32)], otherwise: u32) -> SemanticSwitchTargetsV1 {
    SemanticSwitchTargetsV1::new(
        cases
            .iter()
            .map(|(value, target)| {
                SemanticSwitchTargetV1::new(
                    *value,
                    payload_edge(SemanticEdgeRoleV1::SwitchValue, *target),
                )
            })
            .collect(),
        payload_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
    )
    .unwrap()
}

fn payload_fixture(
    cases: &[(u128, u32)],
    otherwise: u32,
    second_definition: bool,
) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let enumeration = SemanticTypeIdV1::from_index(2);
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([201; 32]),
            SemanticLayoutIdentityV1::from_sha256([202; 32]),
            SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([203; 32]),
            SemanticLayoutIdentityV1::from_sha256([204; 32]),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([206; 32]),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Enum {
                discriminant: scalar,
                variants: (0..3)
                    .map(|variant| {
                        SemanticEnumVariantV1::new(
                            variant,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        )
                    })
                    .collect(),
            },
        ),
    ];
    let source = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |local, ty, kind| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, kind),
            )),
        )
    };
    let define = || {
        assign(
            1,
            enumeration,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![])
                .unwrap(),
        )
    };
    let mut statements = vec![define()];
    if second_definition {
        statements.push(define());
    }
    statements.push(assign(
        2,
        scalar,
        SemanticRvalueKindV1::Discriminant(place(1, enumeration)),
    ));
    let mut blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([207; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, scalar)),
                    targets: payload_targets(cases, otherwise),
                },
            ),
        )
        .unwrap(),
    ];
    for tag in [208, 209, 210] {
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        );
    }
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([211; 32]),
        SemanticLayoutIdentityV1::from_sha256([212; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([213; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([214; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([215; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([216; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([217; 32]),
        source,
        abi,
        [unit, enumeration, scalar]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([218 + index as u8; 32]),
                    ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    (types, function)
}

#[test]
fn three_variant_payload_facts_require_unique_edge_occurrences() {
    let local = SemanticLocalIdV1::from_index(1);
    for (cases, otherwise, expected) in [
        (vec![(0, 1)], 1, [false, false, false]),
        (vec![(0, 1), (1, 1)], 2, [false, false, true]),
        (vec![(0, 1), (1, 2)], 1, [false, true, false]),
        (vec![(0, 1)], 2, [true, false, false]),
        (vec![(0, 1), (1, 2)], 3, [true, true, true]),
    ] {
        let (types, function) = payload_fixture(&cases, otherwise, false);
        let facts = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
        for (variant, expected) in expected.into_iter().enumerate() {
            assert_eq!(
                facts.availability(local, variant as u32).is_some(),
                expected
            );
        }
        assert!(facts.availability(local, 3).is_none());
        assert!(
            facts
                .availability(SemanticLocalIdV1::from_index(99), 0)
                .is_none()
        );
    }
}

#[test]
fn payload_fact_is_not_reused_after_a_second_same_block_definition() {
    let (types, function) = payload_fixture(&[(0, 1), (1, 2)], 3, true);
    let facts = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    for variant in 0..3 {
        assert!(
            facts
                .availability(SemanticLocalIdV1::from_index(1), variant)
                .is_none()
        );
    }
}

#[test]
fn target_occurrence_scan_prepays_exact_and_one_short_work() {
    for (cases, otherwise, target, unique) in [
        (vec![(0, 1)], 1, 1, false),
        (vec![(0, 1), (1, 2)], 3, 1, true),
    ] {
        let targets = payload_targets(&cases, otherwise);
        let cost = cases.len() + 1;
        let limit = MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1;
        let mut exact = WorkBudgetV1 { used: limit - cost };
        assert_eq!(
            enum_payload_target_is_unique_v1(
                &targets,
                SemanticBlockIdV1::from_index(target),
                &mut exact,
            )
            .unwrap(),
            unique
        );
        assert_eq!(exact.used, limit);
        let mut short = WorkBudgetV1 {
            used: limit - cost + 1,
        };
        assert_eq!(
            enum_payload_target_is_unique_v1(
                &targets,
                SemanticBlockIdV1::from_index(target),
                &mut short,
            ),
            Err(SemanticOptionDominanceErrorV1::WorkLimit {
                actual: limit + 1,
                limit
            })
        );
        assert_eq!(short.used, limit + 1);
    }
}
