use super::*;

#[derive(Clone, Copy)]
enum BitAndSubject {
    Unsigned,
    Signed,
    Unmasked,
    Reassigned,
    Escaped,
    MismatchedType,
}

fn bitand_subject(
    width: u16,
    mask: u128,
    swapped: bool,
    subject: BitAndSubject,
) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let pointer = SemanticTypeIdV1::from_index(2);
    let other_word = SemanticTypeIdV1::from_index(3);
    let mut declarations = types();
    declarations[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([210; 32]),
        SemanticLayoutIdentityV1::from_sha256([211; 32]),
        SemanticTypeLayoutV1::new(Some(u64::from(width / 8)), u64::from(width / 8)).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: matches!(subject, BitAndSubject::Signed),
            bits: width,
        }),
    );
    declarations[2] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([212; 32]),
        SemanticLayoutIdentityV1::from_sha256([213; 32]),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                U8,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    declarations.push(declarations[0].clone());
    let word = |ty, bits| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(bits, (width / 8) as u8).unwrap(),
            ),
        ))
    };
    let input = copy(1, U8);
    let mask = word(
        if matches!(subject, BitAndSubject::MismatchedType) {
            other_word
        } else {
            U8
        },
        mask,
    );
    let (left, right) = if swapped {
        (mask, input)
    } else {
        (input, mask)
    };
    let mut statements = vec![assign(
        2,
        U8,
        if matches!(subject, BitAndSubject::Unmasked) {
            SemanticRvalueKindV1::Use(copy(1, U8))
        } else {
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitAnd,
                left,
                right,
            }
        },
    )];
    if matches!(subject, BitAndSubject::Reassigned) {
        statements.push(assign(2, U8, SemanticRvalueKindV1::Use(copy(1, U8))));
    }
    if matches!(subject, BitAndSubject::Escaped) {
        statements.push(assign(
            4,
            pointer,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(2, U8),
            },
        ));
    }
    statements.push(assign(
        3,
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: copy(2, U8),
            right: word(U8, u128::from(width)),
        },
    ));
    let seed = function(
        214,
        vec![block(215, vec![], SemanticTerminatorKindV1::Return)],
    );
    let direct = |ty| {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
        )
    };
    let function = SemanticFunctionDeclV1::new(
        seed.identity(),
        seed.role(),
        seed.item_definition_identity(),
        seed.monomorphization_identity(),
        seed.generic_type_arguments_identity(),
        seed.const_generic_arguments_identity(),
        seed.source(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([216; 32]),
            SemanticLayoutIdentityV1::from_sha256([217; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct(U8)],
            direct(U8),
        )
        .unwrap(),
        [U8, U8, U8, BOOL, pointer]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([220 + index as u8; 32]),
                    ty,
                    match index {
                        0 => SemanticLocalRoleV1::Return,
                        1 => SemanticLocalRoleV1::Argument(0),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                    seed.source(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                218,
                statements,
                SemanticTerminatorKindV1::Assert {
                    condition: copy(3, BOOL),
                    expected: true,
                    message: SemanticAssertMessageV1::Overflow {
                        operation: SemanticBinaryOpV1::ShiftLeft,
                        left: copy(1, U8),
                        right: copy(2, U8),
                    },
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(
                219,
                vec![assign(
                    0,
                    U8,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::ShiftLeft,
                        left: copy(1, U8),
                        right: copy(2, U8),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .unwrap();
    (declarations, function)
}

#[test]
fn unsigned_bitand_proves_actual_masked_source_assertions_at_every_width() {
    for width in [8, 16, 32, 64, 128] {
        for mask in [0, u128::from(width - 1)] {
            for swapped in [false, true] {
                let (types, function) =
                    bitand_subject(width, mask, swapped, BitAndSubject::Unsigned);
                let mut meter = Meter::default();
                let mut analysis = SemanticAssertionAnalysisV1::new_metered(
                    &types,
                    &function,
                    limits(),
                    &mut meter,
                )
                .unwrap();
                let SemanticAssertionOutcomeV1::Proved(fact) = analysis
                    .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
                    .unwrap()
                else {
                    panic!("actual unsigned masked source must be proved");
                };
                assert!(std::ptr::eq(fact.function(), &function));
                assert!(std::ptr::eq(fact.types(), types.as_slice()));
                assert_eq!(fact.condition(), &copy(3, BOOL));
                assert!(fact.expected());
                assert_eq!(fact.proof_kind(), SemanticAssertionProofKindV1::ExactRange);
            }
        }
    }
}

#[test]
fn unsigned_bitand_does_not_reuse_a_mask_after_source_subject_changes() {
    for subject in [
        BitAndSubject::Signed,
        BitAndSubject::Unmasked,
        BitAndSubject::Reassigned,
        BitAndSubject::Escaped,
        BitAndSubject::MismatchedType,
    ] {
        let (types, function) = bitand_subject(32, 31, false, subject);
        let mut meter = Meter::default();
        let mut analysis =
            SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter)
                .unwrap();
        assert!(matches!(
            analysis
                .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter,)
                .unwrap(),
            SemanticAssertionOutcomeV1::NotProved(_)
        ));
    }
    let (types, function) = bitand_subject(32, u32::MAX.into(), false, BitAndSubject::Unsigned);
    let mut meter = Meter::default();
    let mut analysis =
        SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter).unwrap();
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter,)
            .unwrap(),
        SemanticAssertionOutcomeV1::NotProved(_)
    ));
}

#[test]
fn unsigned_bitand_interval_transfer_is_bounded_and_refuses_unknown_or_invalid_ranges() {
    let transfer = |left, right, maximum| {
        SemanticAssertionRecipeQueriesV1::<Meter>::range_of_binary(
            SemanticBinaryOpV1::BitAnd,
            left,
            right,
            maximum,
        )
        .unwrap()
    };
    for maximum in [
        u8::MAX as u128,
        u16::MAX as u128,
        u32::MAX as u128,
        u64::MAX as u128,
        u128::MAX,
    ] {
        let full = Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum,
        });
        for mask in [0, 7, maximum] {
            let narrow = Some(UnsignedRangeProofV1::exact(mask));
            let expected = Some(UnsignedRangeProofV1 {
                minimum: 0,
                maximum: mask,
            });
            assert_eq!(transfer(full, narrow, Some(maximum)), expected);
            assert_eq!(transfer(narrow, full, Some(maximum)), expected);
        }
        assert_eq!(transfer(None, full, Some(maximum)), None);
        assert_eq!(transfer(full, None, Some(maximum)), None);
        assert_eq!(transfer(full, full, None), None);
    }
    let exact = Some(UnsignedRangeProofV1::exact(7));
    for maximum in [0, 1, 127, 256, 65534] {
        assert_eq!(transfer(exact, exact, Some(maximum)), None);
    }
    for invalid in [
        UnsignedRangeProofV1 {
            minimum: 9,
            maximum: 8,
        },
        UnsignedRangeProofV1::exact(256),
    ] {
        assert_eq!(transfer(Some(invalid), exact, Some(255)), None);
        assert_eq!(transfer(exact, Some(invalid), Some(255)), None);
    }
    for left in 0u128..=255 {
        for right in 0u128..=255 {
            let range = transfer(
                Some(UnsignedRangeProofV1::exact(left)),
                Some(UnsignedRangeProofV1::exact(right)),
                Some(255),
            )
            .unwrap();
            let actual = left & right;
            assert!(range.minimum <= actual && actual <= range.maximum);
        }
    }
}
pub(super) const U8: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
pub(super) const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
pub(super) const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

#[derive(Default)]
pub(super) struct Meter {
    pub work: usize,
    pub storage: usize,
}
impl SemanticAssertionMeterV1 for Meter {
    type Error = std::convert::Infallible;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.work = self.work.checked_add(amount).unwrap();
        Ok(())
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.storage = self.storage.checked_add(bytes).unwrap();
        Ok(())
    }
}
pub(super) fn limits() -> SemanticAssertionLimitsV1 {
    SemanticAssertionLimitsV1::new(usize::MAX, usize::MAX)
}
pub(super) fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
pub(super) fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
pub(super) fn constant(ty: SemanticTypeIdV1, bits: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 1).unwrap()),
    ))
}
pub(super) fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
pub(super) fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}
pub(super) fn assign(
    local: u32,
    ty: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
pub(super) fn types() -> Vec<SemanticTypeDeclV1> {
    let scalar = |tag, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            shape,
        )
    };
    vec![
        scalar(
            1,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 8,
            }),
        ),
        scalar(2, SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(2),
                1,
                SemanticAggregateLayoutV1::new(vec![0, 1], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U8, BOOL]).unwrap()),
        ),
    ]
}
pub(super) fn function(tag: u8, blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let direct = |ty| {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
        )
    };
    let locals = [U8, BOOL, BOOL, PAIR, U8]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([index as u8 + 20; 32]),
                ty,
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct(BOOL)],
            direct(U8),
        )
        .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
pub(super) fn assertion_function(
    condition: SemanticOperandV1,
    message: SemanticAssertMessageV1,
) -> SemanticFunctionDeclV1 {
    function(
        50,
        vec![
            block(
                60,
                vec![],
                SemanticTerminatorKindV1::Assert {
                    condition,
                    expected: true,
                    message,
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(
                61,
                vec![assign(0, U8, SemanticRvalueKindV1::Use(constant(U8, 0)))],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
}
pub(super) fn ordinary_message() -> SemanticAssertMessageV1 {
    SemanticAssertMessageV1::DivisionByZero(constant(U8, 1))
}

#[test]
fn exact_fact_and_refutation_borrow_the_actual_source_subject() {
    let types = types();
    for bit in [0, 1] {
        let function = assertion_function(constant(BOOL, bit), ordinary_message());
        let mut meter = Meter::default();
        let mut analysis =
            SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter)
                .unwrap();
        match analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
            .unwrap()
        {
            SemanticAssertionOutcomeV1::Proved(fact) => {
                assert_eq!(bit, 1);
                assert!(std::ptr::eq(fact.function(), &function));
                assert!(std::ptr::eq(fact.types(), types.as_slice()));
                assert!(std::ptr::eq(
                    fact.assertion(),
                    function.blocks()[0].terminator().kind()
                ));
                assert!(fact.expected());
                assert_eq!(fact.block(), SemanticBlockIdV1::from_index(0));
                assert_eq!(
                    fact.success_edge().target(),
                    SemanticBlockIdV1::from_index(1)
                );
                assert_eq!(fact.proof_kind(), SemanticAssertionProofKindV1::ExactRange);
            }
            SemanticAssertionOutcomeV1::Refuted(fact) => {
                assert_eq!(bit, 0);
                assert!(std::ptr::eq(fact.function(), &function));
                assert_eq!(fact.condition(), &constant(BOOL, 0));
                assert!(fact.expected());
            }
            _ => panic!("exact bool must be classified"),
        }
    }
}
#[test]
fn bounds_deferred_is_legacy_bookkeeping_never_a_positive_fact() {
    let types = types();
    let function = assertion_function(
        copy(1, BOOL),
        SemanticAssertMessageV1::BoundsCheck {
            length: constant(U8, 8),
            index: constant(U8, 3),
        },
    );
    let mut meter = Meter::default();
    let mut analysis =
        SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter).unwrap();
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
            .unwrap(),
        SemanticAssertionOutcomeV1::NotProved(
            SemanticAssertionNotProvedV1::BoundsNeedsIndependentRule
        )
    ));
    assert_eq!(
        analysis
            .legacy_recipe_assertions_v1(false, &mut meter)
            .unwrap()[0],
        SemanticAssertionLegacyDispositionV1::BoundsDeferred
    );
    assert_eq!(
        analysis
            .legacy_recipe_assertions_v1(true, &mut meter)
            .unwrap()[0],
        SemanticAssertionLegacyDispositionV1::NotProved
    );
}
#[test]
fn exact_bounds_predicate_can_be_genuinely_proved_without_deferral() {
    let types = types();
    let function = assertion_function(
        constant(BOOL, 1),
        SemanticAssertMessageV1::BoundsCheck {
            length: constant(U8, 8),
            index: constant(U8, 3),
        },
    );
    let mut meter = Meter::default();
    let mut analysis =
        SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter).unwrap();
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
            .unwrap(),
        SemanticAssertionOutcomeV1::Proved(_)
    ));
}
#[test]
fn unknown_non_assertion_and_invalid_subject_are_distinct() {
    let types = types();
    let function = assertion_function(copy(1, BOOL), ordinary_message());
    let mut meter = Meter::default();
    let mut analysis =
        SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter).unwrap();
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
            .unwrap(),
        SemanticAssertionOutcomeV1::NotProved(SemanticAssertionNotProvedV1::Unknown)
    ));
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(1), &mut meter)
            .unwrap(),
        SemanticAssertionOutcomeV1::NotAnAssertion
    ));
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(2), &mut meter)
            .err()
            .unwrap(),
        SemanticAssertionMeteredErrorV1::Analysis(SemanticAssertionErrorV1::InvalidModel(_))
    ));
}
#[test]
fn checked_u8_addition_retains_no_wrap_and_exact_message_requirements() {
    let types = types();
    for (left, right, message_left, proved) in
        [(2, 3, 2, true), (255, 1, 255, false), (2, 3, 4, false)]
    {
        let flag = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(3),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), BOOL).unwrap()],
                BOOL,
            )
            .unwrap(),
        );
        let function = function(
            51,
            vec![
                block(
                    62,
                    vec![assign(
                        3,
                        PAIR,
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            SemanticCheckedBinaryOpV1::Add,
                            constant(U8, left),
                            constant(U8, right),
                        )),
                    )],
                    SemanticTerminatorKindV1::Assert {
                        condition: flag,
                        expected: false,
                        message: SemanticAssertMessageV1::Overflow {
                            operation: SemanticBinaryOpV1::Add,
                            left: constant(U8, message_left),
                            right: constant(U8, right),
                        },
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    63,
                    vec![assign(0, U8, SemanticRvalueKindV1::Use(constant(U8, 0)))],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
        let mut meter = Meter::default();
        let mut analysis =
            SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter)
                .unwrap();
        assert_eq!(
            matches!(
                analysis
                    .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
                    .unwrap(),
                SemanticAssertionOutcomeV1::Proved(_)
            ),
            proved
        );
    }
}
#[test]
fn profile_success_cannot_bypass_unknown_or_refuted_source_assertion() {
    let types = types();
    for (condition, accepted) in [
        (constant(BOOL, 1), true),
        (constant(BOOL, 0), false),
        (copy(1, BOOL), false),
    ] {
        let function = assertion_function(condition, ordinary_message());
        let mut meter = Meter::default();
        let mut profile =
            SemanticCallableProfileQueriesV1::new_metered(&types, &function, limits(), &mut meter)
                .unwrap();
        assert!(
            profile
                .terminator_profile(1, &[], function.blocks()[0].terminator().kind())
                .unwrap()
                .empty_eligible()
        );
        let functions = [function];
        let callables = [SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )];
        let summary = SemanticDefinedCallableSummariesV1::new_metered(
            &types,
            &functions,
            &callables,
            limits(),
            &mut meter,
        )
        .unwrap();
        assert!(std::ptr::eq(summary.functions(), functions.as_slice()));
        assert!(std::ptr::eq(summary.types(), types.as_slice()));
        assert!(std::ptr::eq(summary.callables(), callables.as_slice()));
        assert_eq!(
            summary.decision(SemanticFunctionIdV1::from_index(0)),
            Some(if accepted {
                SemanticCallableDecisionV1::ExactEmptyDeterministicScalar
            } else {
                SemanticCallableDecisionV1::Rejected
            })
        );
    }
}
#[test]
fn every_defined_callee_and_recursive_dependency_remains_in_the_closure() {
    let types = types();
    for cycle in [false, true] {
        let calling = function(
            52,
            vec![
                block(
                    65,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(if cycle { 0 } else { 1 }),
                            vec![copy(1, BOOL)],
                            Some(SemanticCallDestinationV1::new(
                                place(0, U8),
                                edge(SemanticEdgeRoleV1::CallReturn, 1),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(66, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        let functions = [
            calling,
            assertion_function(copy(1, BOOL), ordinary_message()),
        ];
        let callables = [
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ];
        let summary = SemanticDefinedCallableSummariesV1::new_metered(
            &types,
            &functions,
            &callables,
            limits(),
            &mut Meter::default(),
        )
        .unwrap();
        assert_eq!(
            summary.decision(SemanticFunctionIdV1::from_index(0)),
            Some(SemanticCallableDecisionV1::Rejected)
        );
        assert_eq!(
            summary.decision(SemanticFunctionIdV1::from_index(1)),
            Some(SemanticCallableDecisionV1::Rejected)
        );
    }
}
