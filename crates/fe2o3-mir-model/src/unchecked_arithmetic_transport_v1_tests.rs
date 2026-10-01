use super::*;
use crate::semantic_mir_v1::*;

const INT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const PTR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Change {
    None,
    Direct,
    WrongOrigin,
    Swapped,
    WrongOperation,
    Cast,
    Duplicate,
    Exposed,
    DeadBeforeRead,
    Revived,
    LateDefinition,
    Bypass,
    ReassignedReturn,
    Unwind,
    Cycle,
    WrongZero,
    HintFalse,
    HintEffect,
    HintCycle,
    HintWrongArgument,
    InvalidHelper,
    Moves,
    SnapshotMove,
    StaleMove,
    CallMovedArgument,
    SameLocationMove,
    CheckedSameLocationMove,
    HintStaleMove,
    ProjectedMove,
    LegacyExposed,
    MixedExposed,
}

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn moved(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(local, ty))
}
fn constant(value: bool) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        BOOL,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(u128::from(value), 1).unwrap()),
    ))
}
fn edge(role: SemanticEdgeRoleV1, block: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(source(), kind)
}
fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(local, ty),
        SemanticRvalueV1::new(ty, value),
    )))
}
fn use_value(local: u32, ty: SemanticTypeIdV1, value: SemanticOperandV1) -> SemanticStatementV1 {
    assign(local, ty, SemanticRvalueKindV1::Use(value))
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    end: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 1; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), end),
    )
    .unwrap()
}
fn switch(discriminant: SemanticOperandV1, zero: u32, otherwise: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, zero),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
        )
        .unwrap(),
    }
}
fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        if ty == UNIT {
            SemanticAbiPassModeV1::Ignore
        } else {
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain())
        },
    )
}
fn abi(tag: u8, arguments: &[SemanticTypeIdV1], output: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        arguments.iter().copied().map(direct).collect(),
        direct(output),
    )
    .unwrap()
}
fn function(
    tag: u8,
    local_types: &[SemanticTypeIdV1],
    arguments: usize,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let locals = local_types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([index as u8 + 1; 32]),
                *ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else if index <= arguments {
                    SemanticLocalRoleV1::Argument(index as u32 - 1)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source(),
            )
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if tag == 60 {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        source(),
        abi(tag, &local_types[1..=arguments], local_types[0]),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn types(bits: u16, signed: bool) -> Vec<SemanticTypeDeclV1> {
    let size = u64::from(bits / 8);
    let alignment = size.min(8);
    let tuple_size = (size + alignment) & !(alignment - 1);
    let scalar = |tag, ty, bytes, align, max| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                align,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(
                        matches!(ty, SemanticScalarTypeV1::Integer { signed: true, .. }),
                        (bytes * 8) as u16,
                        align,
                    ),
                    SemanticScalarValidityRangeV1::new(0, max),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(ty),
        )
    };
    vec![
        scalar(
            1,
            SemanticScalarTypeV1::Integer { signed, bits },
            size,
            alignment,
            if bits == 128 {
                u128::MAX
            } else {
                (1u128 << bits) - 1
            },
        ),
        scalar(2, SemanticScalarTypeV1::Bool, 1, 1, 1),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(tuple_size),
                alignment,
                SemanticAggregateLayoutV1::new(
                    vec![0, size],
                    if tuple_size == size + 1 {
                        vec![]
                    } else {
                        vec![SemanticPaddingV1::new(size + 1, tuple_size - size - 1).unwrap()]
                    },
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![INT, BOOL]).unwrap()),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([4; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([5; 32]),
            SemanticLayoutIdentityV1::from_sha256([5; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    INT,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ]
}
struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
}
impl Fixture {
    fn result(
        &self,
    ) -> Result<Option<SemanticUncheckedArithmeticViolationV1>, SemanticOptionDominanceErrorV1>
    {
        semantic_unchecked_arithmetic_in_module_v1(
            &self.functions[0],
            &self.types,
            &self.functions,
            &self.callables,
        )
    }
    fn request(&self) -> InertSemanticMirRequestV1 {
        InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([90; 32])),
            self.types.clone(),
            vec![],
            vec![],
            vec![],
            self.functions.clone(),
            self.callables.clone(),
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
    }
}
fn fixture(change: Change, bits: u16, signed: bool) -> Fixture {
    let read = if change == Change::Moves { moved } else { copy };
    let same_operands = matches!(
        change,
        Change::SameLocationMove | Change::CheckedSameLocationMove
    );
    let legacy_exposed = matches!(change, Change::LegacyExposed | Change::MixedExposed);
    let mut before = vec![
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(3),
        )),
        use_value(3, INT, copy(1, INT)),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(4),
        )),
        use_value(4, INT, copy(2, INT)),
        assign(
            5,
            PAIR,
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                SemanticCheckedBinaryOpV1::Add,
                if change == Change::CheckedSameLocationMove {
                    moved(3, INT)
                } else {
                    read(if legacy_exposed { 1 } else { 3 }, INT)
                },
                read(
                    if legacy_exposed {
                        2
                    } else if same_operands {
                        3
                    } else {
                        4
                    },
                    INT,
                ),
            )),
        ),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(3),
        )),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(4),
        )),
        use_value(
            6,
            BOOL,
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(5),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), BOOL)
                            .unwrap(),
                    ],
                    BOOL,
                )
                .unwrap(),
            ),
        ),
    ];
    if change == Change::ProjectedMove {
        let field = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(5),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), BOOL).unwrap()],
            BOOL,
        )
        .unwrap();
        before.insert(7, use_value(13, BOOL, SemanticOperandV1::Move(field)));
    }
    if change == Change::Duplicate {
        before.push(use_value(3, INT, copy(1, INT)));
    }
    if change == Change::Exposed || legacy_exposed {
        before.push(assign(
            11,
            PTR,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(1, INT),
            },
        ));
    }
    let hint = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![if change == Change::CallMovedArgument {
                moved(6, BOOL)
            } else {
                read(6, BOOL)
            }],
            Some(SemanticCallDestinationV1::new(
                place(7, BOOL),
                edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            if change == Change::Unwind {
                SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 2))
            } else {
                SemanticUnwindActionV1::Unreachable
            },
        )
        .unwrap(),
    );
    let mut safe = vec![
        use_value(
            8,
            INT,
            copy(if change == Change::WrongOrigin { 2 } else { 1 }, INT),
        ),
        use_value(9, INT, copy(2, INT)),
    ];
    if change == Change::Cast {
        safe[0] = assign(
            8,
            INT,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: copy(1, INT),
            },
        );
    }
    if matches!(change, Change::DeadBeforeRead | Change::Revived) {
        safe.push(statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(8),
        )));
        if change == Change::Revived {
            safe.push(statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(8),
            )));
        }
    }
    if matches!(change, Change::StaleMove | Change::SnapshotMove) {
        safe.push(use_value(12, INT, moved(8, INT)));
    }
    safe.push(assign(
        10,
        INT,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            if change == Change::WrongOperation {
                SemanticUncheckedBinaryOpV1::Subtract
            } else {
                SemanticUncheckedBinaryOpV1::Add
            },
            if change == Change::SameLocationMove {
                moved(8, INT)
            } else {
                read(
                    if legacy_exposed {
                        1
                    } else if change == Change::Swapped {
                        9
                    } else if change == Change::SnapshotMove {
                        12
                    } else {
                        8
                    },
                    INT,
                )
            },
            read(
                if legacy_exposed {
                    2
                } else if change == Change::Swapped || same_operands {
                    8
                } else {
                    9
                },
                INT,
            ),
        )),
    ));
    if change == Change::MixedExposed {
        safe.push(assign(
            12,
            INT,
            SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                SemanticUncheckedBinaryOpV1::Add,
                copy(8, INT),
                copy(9, INT),
            )),
        ));
    }
    if change == Change::LateDefinition {
        let definition = safe.remove(0);
        safe.push(definition);
    }
    let mut root_blocks = vec![
        block(
            0,
            before,
            if change == Change::Bypass {
                switch(constant(false), 4, 1)
            } else {
                hint.clone()
            },
        ),
        block(
            1,
            if change == Change::ReassignedReturn {
                vec![use_value(7, BOOL, constant(false))]
            } else {
                vec![]
            },
            switch(
                read(
                    if matches!(change, Change::Direct | Change::CallMovedArgument)
                        || legacy_exposed
                    {
                        6
                    } else {
                        7
                    },
                    BOOL,
                ),
                if change == Change::WrongZero { 3 } else { 2 },
                if change == Change::WrongZero { 2 } else { 3 },
            ),
        ),
        block(
            2,
            safe,
            if change == Change::Cycle {
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))
            } else {
                SemanticTerminatorKindV1::Return
            },
        ),
        block(3, vec![], SemanticTerminatorKindV1::Return),
    ];
    if change == Change::Bypass {
        root_blocks.push(block(4, vec![], hint));
    }
    let root = function(
        60,
        &[
            UNIT, INT, INT, INT, INT, PAIR, BOOL, BOOL, INT, INT, INT, PTR, INT, BOOL,
        ],
        2,
        root_blocks,
    );
    let mut hint_true = vec![];
    if change == Change::HintEffect {
        hint_true.push(statement(SemanticStatementKindV1::Assume(constant(true))));
    }
    let helper = function(
        61,
        &[
            BOOL,
            if change == Change::InvalidHelper {
                INT
            } else {
                BOOL
            },
            BOOL,
            UNIT,
        ],
        1,
        vec![
            block(
                0,
                vec![use_value(
                    2,
                    BOOL,
                    if change == Change::HintWrongArgument {
                        constant(false)
                    } else if change == Change::HintStaleMove {
                        moved(1, BOOL)
                    } else {
                        read(1, BOOL)
                    },
                )],
                switch(
                    read(
                        if change == Change::HintStaleMove {
                            1
                        } else {
                            2
                        },
                        BOOL,
                    ),
                    3,
                    1,
                ),
            ),
            block(
                1,
                hint_true,
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(2),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            place(3, UNIT),
                            edge(SemanticEdgeRoleV1::CallReturn, 2),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(
                2,
                vec![use_value(0, BOOL, constant(change != Change::HintFalse))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(
                3,
                vec![use_value(0, BOOL, constant(false))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(
                4,
                vec![],
                if change == Change::HintCycle {
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0))
                } else {
                    SemanticTerminatorKindV1::Return
                },
            ),
        ],
    );
    Fixture {
        types: types(bits, signed),
        functions: vec![root, helper],
        callables: vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256([62; 32]),
                    SemanticItemDefinitionIdentityV1::from_sha256([62; 32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([62; 32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([62; 32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([62; 32]),
                    source(),
                    abi(62, &[], UNIT),
                ),
                operation: SemanticCompilerIntrinsicOperationV1::ColdPath,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([63; 32]),
            },
        ],
    }
}

#[test]
fn actual_shaped_bool_call_and_distinct_scalar_copies_preserve_all_integer_widths() {
    for signed in [false, true] {
        for bits in [8, 16, 32, 64, 128] {
            for change in [
                Change::None,
                Change::Moves,
                Change::SnapshotMove,
                Change::ProjectedMove,
            ] {
                let fixture = fixture(change, bits, signed);
                assert!(
                    super::super::semantic_unchecked_arithmetic_violation_v1(&fixture.functions[0])
                        .unwrap()
                        .is_some()
                );
                assert_eq!(
                    fixture.result().unwrap(),
                    None,
                    "{change:?}/{signed}/{bits}"
                );
                fixture
                    .request()
                    .admit(SemanticMirLimitsV1::default())
                    .unwrap();
            }
        }
    }
}

#[test]
fn scalar_copy_refinement_does_not_require_a_hint_call() {
    assert_eq!(fixture(Change::Direct, 32, false).result().unwrap(), None);
    let legacy = fixture(Change::LegacyExposed, 32, false);
    assert_eq!(
        super::super::semantic_unchecked_arithmetic_violation_v1(&legacy.functions[0]).unwrap(),
        None
    );
    assert_eq!(legacy.result().unwrap(), None);
    let mixed = fixture(Change::MixedExposed, 32, false);
    let old = super::super::semantic_unchecked_arithmetic_violation_v1(&mixed.functions[0])
        .unwrap()
        .unwrap();
    assert_eq!((old.block.index(), old.statement), (2, 3));
    let conservative = mixed.result().unwrap().unwrap();
    assert_eq!((conservative.block.index(), conservative.statement), (2, 2));
}

#[test]
fn scalar_origins_preserve_operand_order_operation_types_and_definitions() {
    for change in [
        Change::WrongOrigin,
        Change::Swapped,
        Change::WrongOperation,
        Change::Cast,
        Change::Duplicate,
        Change::Exposed,
    ] {
        assert!(
            fixture(change, 32, false).result().unwrap().is_some(),
            "{change:?}"
        );
    }
}

#[test]
fn caller_storage_generations_and_normal_edge_scope_cannot_be_bypassed() {
    for change in [
        Change::DeadBeforeRead,
        Change::Revived,
        Change::LateDefinition,
        Change::Bypass,
        Change::ReassignedReturn,
        Change::Unwind,
        Change::Cycle,
        Change::WrongZero,
        Change::StaleMove,
        Change::CallMovedArgument,
        Change::SameLocationMove,
        Change::CheckedSameLocationMove,
    ] {
        assert!(
            fixture(change, 32, false).result().unwrap().is_some(),
            "{change:?}"
        );
    }
    let call = function(
        60,
        &[UNIT, INT],
        1,
        vec![
            block(
                0,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![moved(1, INT), copy(1, INT)],
                        Some(SemanticCallDestinationV1::new(
                            place(0, UNIT),
                            edge(SemanticEdgeRoleV1::CallReturn, 1),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    assert_eq!(
        operand_uses(
            &call,
            Location {
                block: 0,
                statement: 0
            },
            SemanticLocalIdV1::from_index(1),
            &mut WorkBudgetV1::default()
        )
        .unwrap(),
        (2, true)
    );
}

#[test]
fn helper_identity_comes_from_the_complete_boolean_domain_not_its_name() {
    for change in [
        Change::HintFalse,
        Change::HintEffect,
        Change::HintCycle,
        Change::HintWrongArgument,
        Change::InvalidHelper,
        Change::HintStaleMove,
    ] {
        assert!(
            fixture(change, 32, false).result().unwrap().is_some(),
            "{change:?}"
        );
    }
    let mut missing = fixture(Change::None, 32, false);
    missing.callables[1] = missing.callables[2].clone();
    assert!(missing.result().unwrap().is_some());
}

#[test]
fn all_referenced_function_bodies_validate_before_any_cross_function_proof() {
    let failure = fixture(Change::InvalidHelper, 32, false)
        .request()
        .admit(SemanticMirLimitsV1::default())
        .unwrap_err();
    assert!(
        !matches!(
            failure,
            SemanticMirErrorV1::UnprovenUncheckedArithmetic { .. }
        ),
        "invalid helper must fail structural validation, not become a proof: {failure:?}"
    );
}

#[test]
fn actual_transport_work_limit_is_exact_and_one_short_fails_closed() {
    let fixture = fixture(Change::None, 8, false);
    let run = |budget: &mut WorkBudgetV1<'_>| {
        analyze(
            &fixture.functions[0],
            &fixture.types,
            &fixture.functions,
            &fixture.callables,
            budget,
        )
    };
    let mut baseline = WorkBudgetV1::default();
    assert_eq!(run(&mut baseline).unwrap(), None);
    let used = baseline.used;
    assert!(used > 0 && used < MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1);
    let mut exact = WorkBudgetV1 {
        used: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1 - used,
        meter: None,
    };
    assert_eq!(run(&mut exact).unwrap(), None);
    let mut short = WorkBudgetV1 {
        used: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1 - used + 1,
        meter: None,
    };
    assert!(matches!(
        run(&mut short),
        Err(SemanticOptionDominanceErrorV1::WorkLimit { .. })
    ));
}

#[test]
fn actual_transport_storage_reservations_preserve_meter_exhaustion() {
    struct Meter {
        bytes: usize,
        maximum: usize,
    }
    impl enum_payload_resources::InternalMeter for Meter {
        fn work(&mut self, _: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
            Ok(())
        }
        fn storage(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
            self.bytes = self
                .bytes
                .checked_add(amount)
                .ok_or(SemanticOptionDominanceErrorV1::Storage)?;
            if self.bytes > self.maximum {
                Err(SemanticOptionDominanceErrorV1::Storage)
            } else {
                Ok(())
            }
        }
        fn failure(&mut self, _: bool) {}
    }
    let fixture = fixture(Change::None, 8, false);
    let run = |meter: &mut Meter| {
        analyze(
            &fixture.functions[0],
            &fixture.types,
            &fixture.functions,
            &fixture.callables,
            &mut WorkBudgetV1 {
                used: 0,
                meter: Some(meter),
            },
        )
    };
    let mut baseline = Meter {
        bytes: 0,
        maximum: usize::MAX,
    };
    assert_eq!(run(&mut baseline).unwrap(), None);
    let mut exact = Meter {
        bytes: 0,
        maximum: baseline.bytes,
    };
    assert_eq!(run(&mut exact).unwrap(), None);
    let mut short = Meter {
        bytes: 0,
        maximum: baseline.bytes - 1,
    };
    assert_eq!(
        run(&mut short),
        Err(SemanticOptionDominanceErrorV1::Storage)
    );
}
