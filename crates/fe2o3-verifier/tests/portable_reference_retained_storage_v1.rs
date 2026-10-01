//! Public inert records and original signature constructors only.
//! No authenticated reference binding, source provider or proof is constructed.
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1, LogicalStorageLimitsV1};
use fe2o3_mir_model::semantic_mir_v1::{SemanticExternAbiV1, SemanticFunctionSafetyV1};
use fe2o3_verifier::portable_reference_v1::retained_storage_v1::{
    MAX_REFERENCE_RETAINED_EXPRESSION_DEPTH_V1, ReferenceRetainedStorageErrorV1 as Error,
};
use fe2o3_verifier::portable_reference_v1::signature::*;
use fe2o3_verifier::portable_reference_v1::*;
use std::mem::size_of;

fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
    LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: bytes,
        max_items: items,
    })
}
fn leaf() -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }
}
fn binary() -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::Add,
        lhs: Box::new(leaf()),
        rhs: Box::new(leaf()),
        checked: false,
    }
}
fn place(count: usize) -> ReferencePlaceV1 {
    let rows = [
        ReferencePlaceProjectionV1::Dereference,
        ReferencePlaceProjectionV1::Field(1),
        ReferencePlaceProjectionV1::Index(2),
        ReferencePlaceProjectionV1::ConstantIndex {
            offset: 1,
            minimum_length: 8,
            from_end: false,
        },
    ];
    ReferencePlaceV1 {
        local: 1,
        projection: rows[..count].to_vec().into_boxed_slice(),
    }
}
fn input(count: usize) -> ReferenceOperandV1 {
    ReferenceOperandV1::Copy(place(count))
}
fn zero() -> ReferenceOperandV1 {
    ReferenceOperandV1::Constant(ReferenceConstantV1::ZeroSized)
}
fn scalar() -> ReferenceOperandV1 {
    ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::U32,
        bits: 7,
    })
}
fn identity() -> ReferenceFunctionIdentityV1 {
    ReferenceFunctionIdentityV1 {
        def_path_hash: [1; 16],
        function_sha256: [2; 32],
        item_definition_sha256: [3; 32],
        monomorphization_sha256: [4; 32],
        generic_type_arguments_sha256: [5; 32],
        const_generic_arguments_sha256: [6; 32],
        rustc_mir_body_sha256: [7; 32],
    }
}
fn helper() -> ReferenceValueV1 {
    ReferenceValueV1::SafeHelperCall {
        helper: identity(),
        parameters: vec![ReferenceScalarTypeV1::U32; 2].into_boxed_slice(),
        result: ReferenceScalarTypeV1::U32,
        arguments: vec![input(2), zero()].into_boxed_slice(),
        summary: Box::new(binary()),
    }
}
fn complex_write() -> ReferenceOutputWriteV1 {
    ReferenceOutputWriteV1 {
        argument: 1,
        block: 2,
        statement: 3,
        coordinate: ReferenceOutputCoordinateV1::LogicalPoint(
            vec![ReferenceEffectExpressionV1::Binary {
                operation: ReferenceBinaryOpV1::Add,
                lhs: Box::new(leaf()),
                rhs: Box::new(ReferenceEffectExpressionV1::Unary {
                    operation: ReferenceUnaryOpV1::Not,
                    operand: Box::new(leaf()),
                }),
                checked: true,
            }]
            .into_boxed_slice(),
        ),
        guard: ReferencePathPredicateV1 {
            clauses: vec![ReferenceGuardClauseV1 {
                atoms: vec![
                    ReferenceGuardAtomV1::SwitchValueSet {
                        discriminant: ReferenceEffectExpressionV1::InputLoad {
                            reference_argument: 0,
                            index: Box::new(ReferenceEffectExpressionV1::Cast {
                                kind: ReferenceCastKindV1::Integer,
                                source: ReferenceScalarTypeV1::U32,
                                target: ReferenceScalarTypeV1::U64,
                                operand: Box::new(leaf()),
                            }),
                        },
                        values: vec![3, 5].into_boxed_slice(),
                        inside_set: true,
                    },
                    ReferenceGuardAtomV1::Assert {
                        condition: binary(),
                        expected: true,
                    },
                ]
                .into_boxed_slice(),
            }]
            .into_boxed_slice(),
        },
        rhs: ReferenceEffectExpressionV1::Cast {
            kind: ReferenceCastKindV1::Integer,
            source: ReferenceScalarTypeV1::U32,
            target: ReferenceScalarTypeV1::U64,
            operand: Box::new(ReferenceEffectExpressionV1::InputLoad {
                reference_argument: 0,
                index: Box::new(leaf()),
            }),
        },
        value: helper(),
    }
}
fn complex_write_bytes() -> usize {
    // Independent concrete ownership census: 1 coordinate-array expression
    // plus 3 coordinate boxes +4 guard boxes +2 RHS boxes +3 helper-summary boxes.
    13 * size_of::<ReferenceEffectExpressionV1>()
        + size_of::<ReferenceGuardClauseV1>()
        + 2 * size_of::<ReferenceGuardAtomV1>()
        + 2 * size_of::<u128>()
        + 2 * size_of::<ReferenceScalarTypeV1>()
        + 2 * size_of::<ReferenceOperandV1>()
        + 2 * size_of::<ReferencePlaceProjectionV1>()
}
fn ir() -> ReferenceEffectIrV1 {
    let values = vec![
        ReferenceValueV1::Use(input(2)),
        ReferenceValueV1::Binary {
            operation: ReferenceBinaryOpV1::Add,
            lhs: input(1),
            rhs: ReferenceOperandV1::Move(place(2)),
            checked: true,
        },
        ReferenceValueV1::Unary {
            operation: ReferenceUnaryOpV1::Not,
            operand: scalar(),
        },
        ReferenceValueV1::Cast {
            kind: ReferenceCastKindV1::Integer,
            source: ReferenceScalarTypeV1::U32,
            target: ReferenceScalarTypeV1::U64,
            operand: ReferenceOperandV1::Move(place(3)),
        },
        ReferenceValueV1::InputLength {
            reference_argument: 0,
        },
        helper(),
    ];
    let assignments = values
        .into_iter()
        .enumerate()
        .map(|(i, value)| ReferenceAssignmentV1 {
            statement: i as u32,
            destination: place(4),
            value,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    ReferenceEffectIrV1 {
        argument_count: 5,
        local_count: 7,
        relations: vec![
            ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0,
            },
            ReferenceArgumentRelationV1::ScalarInput {
                argument: 1,
                scalar: ReferenceScalarTypeV1::U32,
            },
            ReferenceArgumentRelationV1::SharedSliceInput {
                argument: 2,
                element: ReferenceScalarTypeV1::U32,
            },
            ReferenceArgumentRelationV1::DisjointOutputSlice {
                argument: 3,
                element: ReferenceScalarTypeV1::U32,
            },
            ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                argument: 4,
                element: ReferenceScalarTypeV1::U32,
            },
        ]
        .into_boxed_slice(),
        blocks: vec![
            ReferenceBlockV1 {
                block: 0,
                assignments,
                terminator: ReferenceTerminatorV1::Return,
            },
            ReferenceBlockV1 {
                block: 1,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Goto { target: 0 },
            },
            ReferenceBlockV1 {
                block: 2,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Switch {
                    discriminant: input(1),
                    values: vec![(1, 0), (2, 1)].into_boxed_slice(),
                    otherwise: 0,
                },
            },
            ReferenceBlockV1 {
                block: 3,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Assert {
                    condition: ReferenceOperandV1::Move(place(1)),
                    expected: true,
                    success: 0,
                    bounds_check: Some(ReferenceBoundsCheckV1 {
                        index: input(2),
                        length: scalar(),
                    }),
                },
            },
        ]
        .into_boxed_slice(),
        loop_summaries: vec![
            ReferenceLoopSummaryV2 {
                header: 0,
                latch: 1,
                exit: 2,
                exact_iterations: Some(3),
                maximum_iterations: 3,
                carried_locals: vec![1, 2, 3].into_boxed_slice(),
                initial_state_sha256: [1; 32],
                transition_sha256: [2; 32],
                variant_sha256: [3; 32],
            },
            ReferenceLoopSummaryV2 {
                header: 4,
                latch: 5,
                exit: 6,
                exact_iterations: None,
                maximum_iterations: 8,
                carried_locals: vec![4].into_boxed_slice(),
                initial_state_sha256: [4; 32],
                transition_sha256: [5; 32],
                variant_sha256: [6; 32],
            },
        ]
        .into_boxed_slice(),
        observable_output_effects: vec![complex_write()].into_boxed_slice(),
    }
}
fn ir_bytes() -> usize {
    5 * size_of::<ReferenceArgumentRelationV1>() + 4 * size_of::<ReferenceBlockV1>()
        + 6 * size_of::<ReferenceAssignmentV1>()
        //24 destination +10 value +4 terminator projection rows.
        + 38 * size_of::<ReferencePlaceProjectionV1>()
        + 2 * size_of::<(u128, u32)>()
        + 2 * size_of::<ReferenceLoopSummaryV2>() + 4 * size_of::<u32>()
        + size_of::<ReferenceOutputWriteV1>() + complex_write_bytes()
        // Independent helper value retained in the IR assignment.
        + 3 * size_of::<ReferenceEffectExpressionV1>()
        + 2 * size_of::<ReferenceScalarTypeV1>() + 2 * size_of::<ReferenceOperandV1>()
}
fn signature() -> ReferenceLogicalSignaturePreimageV1 {
    let mut kernel = Vec::with_capacity(21);
    kernel.push(ReferenceSignatureInputV1::Scalar(
        ReferenceScalarTypeV1::U32,
    ));
    let reference = vec![
        ReferenceSignatureInputV1::Scalar(ReferenceScalarTypeV1::Usize),
        ReferenceSignatureInputV1::Scalar(ReferenceScalarTypeV1::U32),
    ];
    ReferenceLogicalSignaturePreimageV1::new(
        kernel.into_boxed_slice(),
        reference.into_boxed_slice(),
        ReferenceReturnShapeV1::Unit,
        SemanticExternAbiV1::Rust,
        SemanticFunctionSafetyV1::Safe,
        false,
    )
    .unwrap()
}

#[test]
fn original_signature_constructor_counts_both_boxes_not_former_vec_capacity() {
    let signature = signature();
    let before = signature.clone();
    signature.derive_relations_v1().unwrap();
    let expected = 3 * size_of::<ReferenceSignatureInputV1>();
    let mut c = counter(Some(expected), 3);
    signature.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 3));
    assert_eq!(signature, before);
}
#[test]
fn signature_exact_shared_limits_and_overflow_are_not_reset() {
    let signature = signature();
    let expected = 3 * size_of::<ReferenceSignatureInputV1>();
    for limit in 0..3 {
        let mut c = counter(None, limit);
        assert_eq!(
            signature.charge_retained_heap_storage_v1(&mut c),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
        assert_eq!(c.items(), limit);
    }
    let mut short = counter(Some(expected - 1), 3);
    assert_eq!(
        signature.charge_retained_heap_storage_v1(&mut short),
        Err(LogicalStorageErrorV1::ByteLimit)
    );
    let mut overflow = counter(None, usize::MAX);
    overflow.charge(usize::MAX, 4).unwrap();
    assert_eq!(
        signature.charge_retained_heap_storage_v1(&mut overflow),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((overflow.bytes(), overflow.items()), (usize::MAX, 5));
}
#[test]
fn complete_output_owns_coordinate_guard_rhs_and_original_value_copies() {
    let output = complex_write();
    let before = output.clone();
    let mut c = counter(Some(complex_write_bytes()), 1000);
    c.charge(0, 1).unwrap(); // caller root visit, header independently omitted
    output.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!(c.bytes(), complex_write_bytes());
    assert_eq!(output, before);
}
#[test]
fn full_ir_all_owner_branches_match_independent_concrete_heap_census() {
    let ir = ir();
    let before = ir.clone();
    let header = size_of::<ReferenceEffectIrV1>();
    let mut c = counter(Some(header + ir_bytes()), 1000);
    c.charge(header, 1).unwrap();
    ir.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!(c.bytes(), header + ir_bytes());
    assert_eq!(ir, before);
}
#[test]
fn independent_output_clone_is_counted_again_not_deduplicated() {
    let ir = ir();
    let duplicate = ir.observable_output_effects.clone();
    assert_ne!(ir.observable_output_effects.as_ptr(), duplicate.as_ptr());
    let expected = ir_bytes() + size_of::<ReferenceOutputWriteV1>() + complex_write_bytes();
    let mut c = counter(Some(expected), 2000);
    ir.charge_retained_heap_storage_v1(&mut c).unwrap();
    c.array::<ReferenceOutputWriteV1>(duplicate.len()).unwrap();
    for output in &duplicate {
        output.charge_retained_heap_storage_v1(&mut c).unwrap();
    }
    assert_eq!(c.bytes(), expected);
}
#[test]
fn each_expression_variant_and_fixed_leaf_is_observed_without_header_duplication() {
    for expression in [
        leaf(),
        ReferenceEffectExpressionV1::KernelScalarArgument { argument: 2 },
        ReferenceEffectExpressionV1::InputLength {
            reference_argument: 3,
        },
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::ZeroSized),
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::U32,
            bits: 9,
        }),
    ] {
        let mut c = counter(Some(0), 2);
        expression.charge_retained_heap_storage_v1(&mut c).unwrap();
        assert_eq!(c.bytes(), 0);
    }
    let mut c = counter(Some(2 * size_of::<ReferenceEffectExpressionV1>()), 5);
    binary().charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!(
        (c.bytes(), c.items()),
        (2 * size_of::<ReferenceEffectExpressionV1>(), 5)
    );
}
#[test]
fn all_remaining_coordinate_shapes_and_empty_predicates_are_complete() {
    for (coordinate, extra) in [
        (ReferenceOutputCoordinateV1::SingleCoordinate, 0),
        (
            ReferenceOutputCoordinateV1::Dynamic(binary()),
            2 * size_of::<ReferenceEffectExpressionV1>(),
        ),
        (
            ReferenceOutputCoordinateV1::Constant {
                offset: 1,
                minimum_length: 8,
                from_end: false,
            },
            0,
        ),
    ] {
        let output = ReferenceOutputWriteV1 {
            argument: 0,
            block: 0,
            statement: 0,
            coordinate,
            guard: ReferencePathPredicateV1::unconditional_v1(),
            rhs: leaf(),
            value: ReferenceValueV1::InputLength {
                reference_argument: 0,
            },
        };
        let mut c = counter(Some(size_of::<ReferenceGuardClauseV1>() + extra), 100);
        output.charge_retained_heap_storage_v1(&mut c).unwrap();
        assert_eq!(c.bytes(), size_of::<ReferenceGuardClauseV1>() + extra);
    }
}
fn comb(depth: usize) -> ReferenceEffectExpressionV1 {
    let mut expression = leaf();
    for _ in 1..depth {
        expression = ReferenceEffectExpressionV1::Binary {
            operation: ReferenceBinaryOpV1::Add,
            lhs: Box::new(expression),
            rhs: Box::new(leaf()),
            checked: false,
        };
    }
    expression
}
#[test]
fn fixed_stack_accepts_exact_depth_and_explicitly_refuses_the_next() {
    assert_eq!(MAX_REFERENCE_RETAINED_EXPRESSION_DEPTH_V1, 128);
    let at_limit = comb(128);
    let expected = 254 * size_of::<ReferenceEffectExpressionV1>();
    let mut c = counter(Some(expected), 509);
    at_limit.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 509));
    let too_deep = comb(129);
    let mut c = counter(None, 2000);
    assert_eq!(
        too_deep.charge_retained_heap_storage_v1(&mut c),
        Err(Error::ExpressionDepthLimit)
    );
}
#[test]
fn shared_item_limits_stop_first_and_late_without_scanning_remaining_trees() {
    let ir = ir();
    let mut full = counter(None, 2000);
    ir.charge_retained_heap_storage_v1(&mut full).unwrap();
    for limit in [0, 1, 2, full.items() - 1] {
        let mut c = counter(None, limit);
        assert_eq!(
            ir.charge_retained_heap_storage_v1(&mut c),
            Err(Error::Counter(LogicalStorageErrorV1::ItemLimit))
        );
        assert_eq!(c.items(), limit);
    }
    let mut c = counter(Some(ir_bytes() - 1), 2000);
    assert_eq!(
        ir.charge_retained_heap_storage_v1(&mut c),
        Err(Error::Counter(LogicalStorageErrorV1::ByteLimit))
    );
}
#[test]
fn empty_ir_retains_exact_array_visits_and_preexisting_counters_are_preserved() {
    let empty = ReferenceEffectIrV1 {
        argument_count: 0,
        local_count: 0,
        relations: Box::default(),
        blocks: Box::default(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    let mut c = counter(Some(11), 7);
    c.charge(11, 2).unwrap();
    empty.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (11, 7));
    let ir = ir();
    let mut c = counter(None, usize::MAX);
    c.charge(usize::MAX, 7).unwrap();
    assert_eq!(
        ir.charge_retained_heap_storage_v1(&mut c),
        Err(Error::Counter(LogicalStorageErrorV1::Arithmetic))
    );
    assert_eq!((c.bytes(), c.items()), (usize::MAX, 8));
}

#[test]
fn depth_refusal_is_independent_of_small_pending_frontier() {
    for depth in [128, 129] {
        // Opposite asymmetric shape: DFS left leaves are consumed immediately,
        // so the pending frontier stays small even while right depth grows.
        let mut expression = leaf();
        for _ in 1..depth {
            expression = ReferenceEffectExpressionV1::Binary {
                operation: ReferenceBinaryOpV1::Add,
                lhs: Box::new(leaf()),
                rhs: Box::new(expression),
                checked: false,
            };
        }
        let mut c = counter(None, 1000);
        let result = expression.charge_retained_heap_storage_v1(&mut c);
        if depth == 128 {
            result.unwrap();
            assert_eq!(
                (c.bytes(), c.items()),
                (254 * size_of::<ReferenceEffectExpressionV1>(), 509)
            );
        } else {
            assert_eq!(result, Err(Error::ExpressionDepthLimit));
        }
    }
}
#[test]
fn absent_assert_bounds_and_unreachable_guard_do_not_invent_heap() {
    let ir = ReferenceEffectIrV1 {
        argument_count: 0,
        local_count: 0,
        relations: Box::default(),
        blocks: vec![ReferenceBlockV1 {
            block: 0,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Assert {
                condition: zero(),
                expected: true,
                success: 0,
                bounds_check: None,
            },
        }]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    let mut c = counter(Some(size_of::<ReferenceBlockV1>()), 10);
    ir.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (size_of::<ReferenceBlockV1>(), 10));
    let output = ReferenceOutputWriteV1 {
        argument: 0,
        block: 0,
        statement: 0,
        coordinate: ReferenceOutputCoordinateV1::SingleCoordinate,
        guard: ReferencePathPredicateV1::unreachable_v1(),
        rhs: leaf(),
        value: ReferenceValueV1::InputLength {
            reference_argument: 0,
        },
    };
    let mut c = counter(Some(0), 6);
    output.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (0, 6));
}
