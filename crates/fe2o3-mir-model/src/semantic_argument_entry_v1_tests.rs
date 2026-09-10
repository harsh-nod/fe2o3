use super::*;
use crate::semantic_mir_v1::*;

fn place(local: u32, ty: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        Vec::new(),
        SemanticTypeIdV1::from_index(ty),
    )
    .unwrap()
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, terminator),
    )
    .unwrap()
}

fn function(
    local_types: &[u32],
    argument: bool,
    entry: u32,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit = SemanticTypeIdV1::from_index(0);
    let arguments = if argument {
        vec![SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(local_types[1]),
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
        )]
    } else {
        Vec::new()
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([240; 32]),
        SemanticLayoutIdentityV1::from_sha256([241; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        arguments,
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = local_types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([10 + index as u8; 32]),
                SemanticTypeIdV1::from_index(*ty),
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else if argument && index == 1 {
                    SemanticLocalRoleV1::Argument(0)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([242; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([243; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([244; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([245; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([246; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(entry),
        blocks,
    )
    .unwrap()
}

fn enum_types() -> Vec<SemanticTypeDeclV1> {
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([30; 32]),
            SemanticLayoutIdentityV1::from_sha256([31; 32]),
            SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([32; 32]),
            SemanticLayoutIdentityV1::from_sha256([33; 32]),
            SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            SemanticTypeShapeV1::Enum {
                discriminant: SemanticTypeIdV1::from_index(2),
                variants: vec![
                    SemanticEnumVariantV1::new(
                        0,
                        SemanticAggregateTypeV1::new(Vec::new()).unwrap(),
                    ),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(0)])
                            .unwrap(),
                    ),
                ]
                .into_boxed_slice(),
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([34; 32]),
            SemanticLayoutIdentityV1::from_sha256([35; 32]),
            SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 8,
            }),
        ),
    ]
}

fn discriminant() -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(2, 2),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(2),
                SemanticRvalueKindV1::Discriminant(place(1, 1)),
            ),
        )),
    )
}

fn enum_switch(zero: u32, one: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(2, 2)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, zero),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, one),
        )
        .unwrap(),
    }
}

fn replacement() -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(1, 1),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::EnumVariant(0),
                    Vec::new(),
                )
                .unwrap(),
            ),
        )),
    )
}

fn argument_branch(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    function(
        &[0, 1, 2],
        true,
        0,
        vec![
            block(40, vec![discriminant()], enum_switch(2, 1)),
            block(41, statements, SemanticTerminatorKindV1::Return),
            block(42, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
}

fn unreachable_prefix(entry: u32) -> Vec<SemanticBasicBlockV1> {
    (0..entry)
        .map(|index| {
            block(
                50 + index as u8,
                Vec::new(),
                SemanticTerminatorKindV1::Return,
            )
        })
        .collect()
}

fn producer_cycle(entry: u32, selected: u32) -> SemanticFunctionDeclV1 {
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        Vec::new(),
        Some(SemanticCallDestinationV1::new(
            place(1, 1),
            edge(SemanticEdgeRoleV1::CallReturn, entry + 2),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let mut blocks = unreachable_prefix(entry);
    blocks.extend([
        block(
            60,
            Vec::new(),
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, entry + 1)),
        ),
        block(61, Vec::new(), SemanticTerminatorKindV1::Call(call)),
        block(62, vec![discriminant()], enum_switch(entry + 4, selected)),
        block(
            63,
            Vec::new(),
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, entry + 2)),
        ),
        block(64, Vec::new(), SemanticTerminatorKindV1::Return),
    ]);
    function(&[0, 1, 2], false, entry, blocks)
}

fn checked_cycle(entry: u32, unsafe_entry: bool) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let scalar = SemanticTypeIdV1::from_index(2);
    let pair = SemanticTypeIdV1::from_index(3);
    let operand = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            scalar,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
        ))
    };
    let unchecked = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(2, 2),
            SemanticRvalueV1::new(
                scalar,
                SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                    SemanticUncheckedBinaryOpV1::Subtract,
                    operand(),
                    operand(),
                )),
            ),
        )),
    );
    let checked = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(1, 3),
            SemanticRvalueV1::new(
                pair,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Subtract,
                    operand(),
                    operand(),
                )),
            ),
        )),
    );
    let overflow = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(1),
                SemanticTypeIdV1::from_index(4),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(4),
    )
    .unwrap();
    let selected = if unsafe_entry { entry } else { entry + 2 };
    let mut blocks = unreachable_prefix(entry);
    blocks.extend([
        block(
            70,
            if unsafe_entry {
                vec![unchecked.clone()]
            } else {
                Vec::new()
            },
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, entry + 1)),
        ),
        block(
            71,
            vec![checked],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(overflow),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, selected),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, entry + 3),
                )
                .unwrap(),
            },
        ),
        block(
            72,
            if unsafe_entry {
                Vec::new()
            } else {
                vec![unchecked]
            },
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, entry + 1)),
        ),
        block(73, Vec::new(), SemanticTerminatorKindV1::Return),
    ]);
    function(&[0, 3, 2], false, entry, blocks)
}

#[test]
fn argument_entry_definition_enables_untouched_payload_and_revokes_reassignment() {
    let types = enum_types();
    let local = SemanticLocalIdV1::from_index(1);
    let untouched =
        SemanticEnumPayloadDominanceV1::analyze(&argument_branch(Vec::new()), &types).unwrap();
    let availability = untouched.availability(local, 1).unwrap();
    assert!(untouched.allows(availability, SemanticBlockIdV1::from_index(1)));
    assert!(!untouched.allows(availability, SemanticBlockIdV1::from_index(0)));
    assert!(!untouched.allows(availability, SemanticBlockIdV1::from_index(2)));
    let mutations = [
        replacement(),
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::SetDiscriminant {
                place: place(1, 1),
                variant_index: 0,
            },
        ),
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Deinitialize(place(1, 1)),
        ),
    ];
    for mutation in mutations {
        let changed =
            SemanticEnumPayloadDominanceV1::analyze(&argument_branch(vec![mutation]), &types)
                .unwrap();
        assert!(changed.availability(local, 1).is_none());
        assert!(changed.availability(local, 0).is_none());
    }
}

#[test]
fn argument_definition_seed_has_exact_fixed_meter_boundaries() {
    const LIMIT: usize = MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1;
    for (statements, expected, counts) in [
        (Vec::new(), 7, [0, 1, 1]),
        (vec![replacement()], 8, [0, 2, 1]),
    ] {
        let function = argument_branch(statements);
        // Three local seeds plus three terminators plus one/two statements.
        let mut budget = WorkBudgetV1::default();
        assert_eq!(
            local_definition_counts(&function, &mut budget).unwrap(),
            counts
        );
        assert_eq!(budget.used, expected);
        let mut exact = WorkBudgetV1 {
            used: LIMIT - expected,
        };
        assert_eq!(
            local_definition_counts(&function, &mut exact).unwrap(),
            counts
        );
        assert_eq!(exact.used, LIMIT);
        let mut one_under = WorkBudgetV1 {
            used: LIMIT - expected + 1,
        };
        assert_eq!(
            local_definition_counts(&function, &mut one_under),
            Err(SemanticOptionDominanceErrorV1::WorkLimit {
                actual: LIMIT + 1,
                limit: LIMIT
            })
        );
        assert_eq!(one_under.used, LIMIT + 1);
        let mut seed_one_under = WorkBudgetV1 { used: LIMIT - 2 };
        assert_eq!(
            local_definition_counts(&function, &mut seed_one_under),
            Err(SemanticOptionDominanceErrorV1::WorkLimit {
                actual: LIMIT + 1,
                limit: LIMIT
            })
        );
        let mut overflow = WorkBudgetV1 { used: usize::MAX };
        assert_eq!(
            local_definition_counts(&function, &mut overflow),
            Err(SemanticOptionDominanceErrorV1::WorkLimit {
                actual: usize::MAX,
                limit: LIMIT
            })
        );
        assert_eq!(overflow.used, usize::MAX);
    }
}

#[test]
fn argument_entry_backedge_does_not_authenticate_the_initial_invocation() {
    for entry in [0, 2] {
        let mut blocks = unreachable_prefix(entry);
        blocks.extend([
            block(
                80,
                Vec::new(),
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, entry + 1)),
            ),
            block(81, vec![discriminant()], enum_switch(entry + 2, entry)),
            block(82, Vec::new(), SemanticTerminatorKindV1::Return),
        ]);
        let function = function(&[0, 1, 2], true, entry, blocks);
        let facts = SemanticEnumPayloadDominanceV1::analyze(&function, &enum_types()).unwrap();
        assert!(
            facts
                .availability(SemanticLocalIdV1::from_index(1), 1)
                .is_none()
        );
        let other = facts
            .availability(SemanticLocalIdV1::from_index(1), 0)
            .unwrap();
        assert!(facts.allows(other, SemanticBlockIdV1::from_index(entry + 2)));
    }
}

#[test]
fn enum_variant_cycle_requires_non_entry_unique_predecessor() {
    for entry in [0, 2] {
        let rejected =
            SemanticEnumPayloadDominanceV1::analyze(&producer_cycle(entry, entry), &enum_types())
                .unwrap();
        assert!(
            rejected
                .availability(SemanticLocalIdV1::from_index(1), 1)
                .is_none()
        );
        let accepted = SemanticEnumPayloadDominanceV1::analyze(
            &producer_cycle(entry, entry + 3),
            &enum_types(),
        )
        .unwrap();
        let availability = accepted
            .availability(SemanticLocalIdV1::from_index(1), 1)
            .unwrap();
        assert!(accepted.allows(availability, SemanticBlockIdV1::from_index(entry + 3)));
        assert!(!accepted.allows(availability, SemanticBlockIdV1::from_index(entry)));
    }
}

#[test]
fn option_some_cycle_requires_non_entry_unique_predecessor() {
    for entry in [0, 2] {
        let producer = SemanticOptionProducerV1::new(
            SemanticLocalIdV1::from_index(1),
            SemanticBlockIdV1::from_index(entry + 2),
        );
        assert!(matches!(
            SemanticOptionDominanceV1::analyze(&producer_cycle(entry, entry), &[producer]),
            Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability Some target is not uniquely controlled by its exact branch"
            ))
        ));
        let accepted =
            SemanticOptionDominanceV1::analyze(&producer_cycle(entry, entry + 3), &[producer])
                .unwrap();
        let availability = accepted
            .availability(SemanticLocalIdV1::from_index(1))
            .unwrap();
        assert!(accepted.allows(availability, SemanticBlockIdV1::from_index(entry + 3)));
        assert!(!accepted.allows(availability, SemanticBlockIdV1::from_index(entry)));
    }
}

#[test]
fn checked_zero_cycle_does_not_authorize_unchecked_arithmetic_at_entry() {
    for entry in [0, 2] {
        let violation = semantic_unchecked_arithmetic_violation_v1(&checked_cycle(entry, true))
            .unwrap()
            .unwrap();
        assert_eq!(violation.operation(), SemanticUncheckedBinaryOpV1::Subtract);
        assert_eq!(violation.block(), SemanticBlockIdV1::from_index(entry));
        assert_eq!(violation.statement(), 0);
        assert_eq!(
            semantic_unchecked_arithmetic_violation_v1(&checked_cycle(entry, false)).unwrap(),
            None
        );
    }
}

#[test]
fn entry_exclusion_does_not_skip_invalid_cfg_reference_validation() {
    let function = producer_cycle(0, 99);
    assert!(matches!(
        SemanticEnumPayloadDominanceV1::analyze(&function, &enum_types()),
        Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(_))
    ));
    assert!(matches!(
        SemanticOptionDominanceV1::analyze(&function, &[]),
        Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(_))
    ));
}
