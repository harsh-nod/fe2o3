use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiIdentityV1, SemanticAbiPassModeV1, SemanticAbiValueV1, SemanticBasicBlockV1,
    SemanticBlockIdentityV1, SemanticCanonAbiV1, SemanticConstGenericArgumentsIdentityV1,
    SemanticConstantV1, SemanticConstantValueV1, SemanticControlFlowEdgeV1, SemanticExternAbiV1,
    SemanticFunctionAbiV1, SemanticFunctionIdentityV1, SemanticFunctionRoleV1,
    SemanticGenericTypeArgumentsIdentityV1, SemanticItemDefinitionIdentityV1,
    SemanticLayoutIdentityV1, SemanticLocalDeclV1, SemanticLocalIdentityV1, SemanticLocalRoleV1,
    SemanticMemoryLoadV1, SemanticMonomorphizationIdentityV1, SemanticMutabilityV1,
    SemanticProjectionV1, SemanticRvalueV1, SemanticSourceProvenanceV1, SemanticStatementV1,
    SemanticTerminatorV1, SemanticTypeIdV1, SemanticVolatilityV1,
};

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(0),
    )
    .unwrap()
}

fn operand(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local))
}

fn audit_statement(kind: SemanticStatementKindV1) -> bool {
    let mut work = 0;
    let mut audit = Audit {
        selected: [
            SemanticLocalIdV1::from_index(1),
            SemanticLocalIdV1::from_index(2),
        ],
        work: &mut work,
    };
    audit.statement(&kind).is_ok()
}

fn audit_terminator(kind: SemanticTerminatorKindV1) -> bool {
    let mut work = 0;
    let mut audit = Audit {
        selected: [
            SemanticLocalIdV1::from_index(1),
            SemanticLocalIdV1::from_index(2),
        ],
        work: &mut work,
    };
    audit.terminator(&kind).is_ok()
}

fn assertion(message: SemanticAssertMessageV1, condition: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: operand(condition),
        expected: true,
        message,
        target: SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::AssertSuccess,
            SemanticBlockIdV1::from_index(0),
        ),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}

#[test]
fn unrelated_assertion_does_not_observe_snapshot() {
    assert!(audit_terminator(assertion(
        SemanticAssertMessageV1::BoundsCheck {
            length: operand(4),
            index: operand(5)
        },
        3
    )));
    assert!(audit_terminator(assertion(
        SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: operand(4),
            right: operand(5)
        },
        3
    )));
    assert!(audit_terminator(assertion(
        SemanticAssertMessageV1::NullPointerDereference,
        3
    )));
}

#[test]
fn assertion_conditions_and_diagnostic_operands_cannot_observe_snapshot() {
    for local in [1, 2] {
        assert!(!audit_terminator(assertion(
            SemanticAssertMessageV1::NullPointerDereference,
            local
        )));
        for message in [
            SemanticAssertMessageV1::BoundsCheck {
                length: operand(local),
                index: operand(5),
            },
            SemanticAssertMessageV1::BoundsCheck {
                length: operand(4),
                index: operand(local),
            },
            SemanticAssertMessageV1::Overflow {
                operation: SemanticBinaryOpV1::Multiply,
                left: operand(local),
                right: operand(5),
            },
            SemanticAssertMessageV1::Overflow {
                operation: SemanticBinaryOpV1::Multiply,
                left: operand(4),
                right: operand(local),
            },
            SemanticAssertMessageV1::DivisionByZero(operand(local)),
            SemanticAssertMessageV1::RemainderByZero(operand(local)),
            SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment: operand(local),
                found_alignment: operand(5),
            },
            SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment: operand(4),
                found_alignment: operand(local),
            },
        ] {
            assert!(!audit_terminator(assertion(message, 3)));
        }
    }
}

#[test]
fn hidden_index_use_and_place_observers_are_rejected() {
    for local in [1, 2] {
        let indexed = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(local)),
                    SemanticTypeIdV1::from_index(0),
                )
                .unwrap(),
            ],
            SemanticTypeIdV1::from_index(0),
        )
        .unwrap();
        let kinds = [
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(indexed)),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(local))),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(local),
            },
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(local),
            },
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(local),
            },
            SemanticRvalueKindV1::Length(place(local)),
            SemanticRvalueKindV1::Discriminant(place(local)),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                place(local),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ];
        for kind in kinds {
            assert!(!audit_statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    place(5),
                    SemanticRvalueV1::new(SemanticTypeIdV1::from_index(0), kind)
                )
            )));
        }
        assert!(!audit_statement(SemanticStatementKindV1::Deinitialize(
            place(local)
        )));
        assert!(!audit_statement(SemanticStatementKindV1::Assume(operand(
            local
        ))));
    }
}

#[test]
fn use_audit_exhaustion_is_an_error_not_a_closed_chain() {
    let mut work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1;
    let mut audit = Audit {
        selected: [
            SemanticLocalIdV1::from_index(1),
            SemanticLocalIdV1::from_index(2),
        ],
        work: &mut work,
    };
    assert!(matches!(
        audit.terminator(&SemanticTerminatorKindV1::Return),
        Err(Rejected::Resource(_))
    ));
}

fn fixture(extra: Option<SemanticStatementKindV1>) -> SemanticFunctionDeclV1 {
    // This fixture tests an inert use visitor, not type/initialization custody.
    let ty = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut statements = (1..=3)
        .map(|local| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(local),
                    SemanticRvalueV1::new(
                        ty,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(ty, SemanticConstantValueV1::ZeroSized),
                        )),
                    ),
                )),
            )
        })
        .collect::<Vec<_>>();
    if let Some(extra) = extra {
        statements.push(SemanticStatementV1::new(source, extra));
    }
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([2; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([3; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([4; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([5; 32]),
        source,
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([6; 32]),
            SemanticLayoutIdentityV1::from_sha256([7; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap(),
        (0..=5)
            .map(|local| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([10 + local; 32]),
                    ty,
                    if local == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([8; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(
                    source,
                    assertion(SemanticAssertMessageV1::NullPointerDereference, 5),
                ),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn allowed(function: &SemanticFunctionDeclV1) -> [&SemanticAssignmentV1; 3] {
    [0, 1, 2].map(|index| {
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[0].statements()[index].kind()
        else {
            panic!()
        };
        assignment
    })
}

#[test]
fn closed_audit_requires_exact_retained_assignments_and_no_extra_observer() {
    let selected = [
        SemanticLocalIdV1::from_index(1),
        SemanticLocalIdV1::from_index(2),
    ];
    let function = fixture(None);
    assert!(closed(&function, allowed(&function), selected, &mut 0).unwrap());
    let foreign = function.clone();
    assert!(!closed(&function, allowed(&foreign), selected, &mut 0).unwrap());
    let mut duplicate = allowed(&function);
    duplicate[1] = duplicate[0];
    assert!(!closed(&function, duplicate, selected, &mut 0).unwrap());
    let observed = fixture(Some(SemanticStatementKindV1::Assume(operand(2))));
    assert!(!closed(&observed, allowed(&observed), selected, &mut 0).unwrap());
    let mut exhausted = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1;
    assert!(closed(&function, allowed(&function), selected, &mut exhausted).is_err());
}
