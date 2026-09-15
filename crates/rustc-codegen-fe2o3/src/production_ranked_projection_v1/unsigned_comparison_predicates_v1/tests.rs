use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

mod race_consumer;
#[path = "../source_unsigned_switch_v1/tests.rs"]
mod source_comparison;

const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const SIGNED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const NARROW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn constant(ty: SemanticTypeIdV1, value: u64, bytes: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), bytes).unwrap()),
    ))
}
fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn edge(role: SemanticEdgeRoleV1, target: usize) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target as u32))
}
fn block(
    id: usize,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([id as u8 + 20; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}
fn switch(ty: SemanticTypeIdV1, zero: usize, one: usize) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: copy(5, ty),
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
fn abi(inputs: Vec<SemanticTypeIdV1>, output: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    let direct = |ty| {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
        )
    };
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([8; 32]),
        SemanticLayoutIdentityV1::from_sha256([9; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs.into_iter().map(direct).collect(),
        direct(output),
    )
    .unwrap()
}
fn callable(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}
fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    local: u32,
    ty: SemanticTypeIdV1,
    target: usize,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(local, ty),
                edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
    locals: Vec<SemanticTypeIdV1>,
    blocks: Vec<SemanticBasicBlockV1>,
}
impl Fixture {
    fn new(operation: SemanticBinaryOpV1, bound: u64, reversed: bool) -> Self {
        let scalar = |signed, bits| {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits })
        };
        let shape = |tag, bytes, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::new(Some(bytes), bytes.max(1)).unwrap(),
                shape,
            )
        };
        let types = vec![
            shape(1, 8, scalar(false, 64)),
            shape(
                2,
                1,
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
            ),
            shape(
                3,
                0,
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            ),
            shape(
                4,
                8,
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![WORD]).unwrap()),
            ),
            shape(
                5,
                8,
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        WITNESS,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
            shape(6, 8, scalar(true, 64)),
            shape(7, 4, scalar(false, 32)),
        ];
        let callables = vec![
            callable(
                10,
                abi(vec![], WITNESS),
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                    index_witness: WITNESS,
                    raw_index: WORD,
                },
            ),
            callable(
                11,
                abi(vec![REFERENCE], WORD),
                SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                    index_witness: WITNESS,
                    raw_index: WORD,
                },
            ),
        ];
        let (left, right) = if reversed {
            (constant(WORD, bound, 8), copy(4, WORD))
        } else {
            (copy(4, WORD), constant(WORD, bound, 8))
        };
        Self {
            types,
            callables,
            locals: vec![UNIT, WITNESS, REFERENCE, WORD, WORD, BOOL],
            blocks: vec![
                block(0, vec![], call(0, vec![], 1, WITNESS, 1)),
                block(
                    1,
                    vec![assign(
                        2,
                        REFERENCE,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(1, WITNESS),
                        },
                    )],
                    call(1, vec![copy(2, REFERENCE)], 3, WORD, 2),
                ),
                block(
                    2,
                    vec![
                        assign(
                            4,
                            WORD,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitAnd,
                                left: copy(3, WORD),
                                right: constant(WORD, 63, 8),
                            },
                        ),
                        assign(
                            5,
                            BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation,
                                left,
                                right,
                            },
                        ),
                    ],
                    switch(BOOL, 3, 4),
                ),
                block(3, vec![], SemanticTerminatorKindV1::Return),
                block(4, vec![], SemanticTerminatorKindV1::Return),
            ],
        }
    }
    fn function(&self) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([30; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([31; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([32; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([33; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([34; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi(vec![], UNIT),
            self.locals
                .iter()
                .enumerate()
                .map(|(i, &ty)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([i as u8 + 40; 32]),
                        ty,
                        if i == 0 {
                            SemanticLocalRoleV1::Return
                        } else {
                            SemanticLocalRoleV1::Temporary
                        },
                        SemanticSourceProvenanceV1::unavailable(),
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            self.blocks.clone(),
        )
        .unwrap()
    }
    fn replace_block(
        &mut self,
        index: usize,
        statements: Vec<SemanticStatementV1>,
        terminal: SemanticTerminatorKindV1,
    ) {
        self.blocks[index] = block(index, statements, terminal);
    }
    fn project(
        &self,
        spent: usize,
    ) -> Result<
        (
            Vec<Option<GuardPredicateV1>>,
            Vec<ProductionRankedOperationV1>,
        ),
        ProductionRankedProjectionErrorV1,
    > {
        let function = self.function();
        let retained = function.clone();
        let inventory = assertion_definition_inventory(&function)?;
        let mut proof = SemanticAssertProofsV1::new(&self.types, &function)?;
        // The requested pre-consumer total includes the real constructor debit.
        proof
            .charge(spent.saturating_sub(proof.work))
            .expect("fixture budget fits before the comparison consumer");
        let mut indices = vec![None; self.locals.len()];
        let root = ProductionRankedValueIdV1::new(0);
        for slot in &mut indices[1..=3] {
            *slot = Some(ProjectedDisjointIndexV1 {
                value: ProductionRankedValueV1::Local(root),
                mapping: SemanticDisjointIndexSpaceV1::Index1d,
                precondition: None,
                availability: None,
            });
        }
        let constants = vec![None; self.locals.len()];
        let mut arguments = vec![None; self.locals.len()];
        let mut next_argument = 0;
        let mut operations = vec![ProductionRankedOperationV1::InvocationIndex {
            result: root,
            dimension: 0,
            launch_extent: 1024,
        }];
        let mut next_value = 1;
        let mut predicates = vec![None; self.locals.len()];
        TotalUnsignedIndexProjectorV1::new(
            &self.types,
            &function,
            &constants,
            &inventory.counts,
            &inventory.address_escaped,
            &inventory.assignments,
            &mut proof,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )?
        .with_invocation_roots(&self.callables, &indices, 1024)?
        .retain_unsigned_comparison_predicates_v1(&mut predicates)?;
        assert_eq!(function, retained);
        assert_eq!(next_argument, 0);
        Ok((predicates, operations))
    }
}

fn evaluate(
    operations: &[ProductionRankedOperationV1],
    value: ProductionRankedValueV1,
    invocation: u64,
) -> u64 {
    let mut values = HashMap::new();
    for operation in operations {
        let (result, value) = match operation {
            ProductionRankedOperationV1::InvocationIndex { result, .. } => (*result, invocation),
            ProductionRankedOperationV1::IndexConstant { result, value } => (*result, *value),
            ProductionRankedOperationV1::IndexBinary {
                result,
                kind,
                lhs,
                rhs,
            } => {
                let left = values[lhs];
                let right = values[rhs];
                (
                    *result,
                    match kind {
                        IndexBinaryKindAttr::Add => left + right,
                        IndexBinaryKindAttr::Multiply => left * right,
                        IndexBinaryKindAttr::Divide => left / right,
                        IndexBinaryKindAttr::Remainder => left % right,
                    },
                )
            }
            _ => panic!("comparison produced a non-total dependency operation"),
        };
        values.insert(ProductionRankedValueV1::Local(result), value);
    }
    values[&value]
}

#[test]
fn unsigned_comparison_retains_real_masked_ge_polarity_and_exact_active_domain() {
    let fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    let (predicates, operations) = fixture.project(0).unwrap();
    let [(lhs, rhs)] = predicates[5].as_ref().unwrap().comparisons.as_slice() else {
        panic!()
    };
    let mut writes = HashMap::new();
    for invocation in 0..1024 {
        let inactive =
            evaluate(&operations, *lhs, invocation) < evaluate(&operations, *rhs, invocation);
        assert_eq!(inactive, invocation & 63 >= 16);
        if !inactive {
            assert!(
                writes
                    .insert((invocation / 64) * 16 + (invocation & 63), invocation)
                    .is_none()
            );
        }
    }
    assert_eq!(writes.len(), 256);
    assert_eq!(writes[&16], 64);
}

#[test]
fn unsigned_comparison_wrong_threshold_and_missing_guard_keep_collision() {
    let fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 63, false);
    let (predicates, operations) = fixture.project(0).unwrap();
    let [(lhs, rhs)] = predicates[5].as_ref().unwrap().comparisons.as_slice() else {
        panic!()
    };
    for invocation in [16, 64] {
        assert!(evaluate(&operations, *lhs, invocation) >= evaluate(&operations, *rhs, invocation));
        assert_eq!((invocation / 64) * 16 + (invocation & 63), 16);
    }
    let mut missing = fixture;
    missing.replace_block(
        2,
        missing.blocks[2].statements()[..1].to_vec(),
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
    );
    assert!(missing.project(0).unwrap().0[5].is_none());
}

#[test]
fn unsigned_comparison_all_orderings_preserve_boolean_values_without_overflow() {
    for operation in [
        SemanticBinaryOpV1::LessThan,
        SemanticBinaryOpV1::LessOrEqual,
        SemanticBinaryOpV1::GreaterThan,
        SemanticBinaryOpV1::GreaterOrEqual,
    ] {
        for reversed in [false, true] {
            for bound in [1, 16, 63] {
                let fixture = Fixture::new(operation, bound, reversed);
                let (predicates, operations) = fixture.project(0).unwrap();
                let [(lhs, rhs)] = predicates[5].as_ref().unwrap().comparisons.as_slice() else {
                    panic!()
                };
                for invocation in 0..128 {
                    let (left, right) = if reversed {
                        (bound, invocation & 63)
                    } else {
                        (invocation & 63, bound)
                    };
                    let expected = match operation {
                        SemanticBinaryOpV1::LessThan => left < right,
                        SemanticBinaryOpV1::LessOrEqual => left <= right,
                        SemanticBinaryOpV1::GreaterThan => left > right,
                        SemanticBinaryOpV1::GreaterOrEqual => left >= right,
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        evaluate(&operations, *lhs, invocation)
                            < evaluate(&operations, *rhs, invocation),
                        expected
                    );
                }
            }
        }
    }
    for (operation, bound, reversed) in [
        (SemanticBinaryOpV1::GreaterOrEqual, 0, false),
        (SemanticBinaryOpV1::LessOrEqual, u64::MAX, false),
        (SemanticBinaryOpV1::GreaterOrEqual, u64::MAX, true),
        (SemanticBinaryOpV1::LessOrEqual, 0, true),
    ] {
        assert!(
            Fixture::new(operation, bound, reversed)
                .project(0)
                .unwrap()
                .0[5]
                .is_none()
        );
    }
}

#[test]
fn unsigned_comparison_reassignment_deinitialization_and_projected_writes_kill_authority() {
    for local in [3, 4, 5] {
        for kind in 0..3 {
            let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
            let ty = fixture.locals[local];
            let mutation = match kind {
                0 => assign(
                    local as u32,
                    ty,
                    SemanticRvalueKindV1::Use(constant(ty, 0, if ty == BOOL { 1 } else { 8 })),
                ),
                1 => SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Deinitialize(place(local as u32, ty)),
                ),
                _ => SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(local as u32),
                            vec![
                                SemanticProjectionV1::new(SemanticProjectionKindV1::Subtype, ty)
                                    .unwrap(),
                            ],
                            ty,
                        )
                        .unwrap(),
                        SemanticRvalueV1::new(
                            ty,
                            SemanticRvalueKindV1::Use(constant(
                                ty,
                                0,
                                if ty == BOOL { 1 } else { 8 },
                            )),
                        ),
                    )),
                ),
            };
            let mut statements = fixture.blocks[2].statements().to_vec();
            statements.insert(1, mutation);
            fixture.replace_block(2, statements, switch(BOOL, 3, 4));
            assert!(
                fixture.project(0).unwrap().0[5].is_none(),
                "local={local}, kind={kind}"
            );
        }
    }
}

#[test]
fn unsigned_comparison_rejects_signed_mismatched_width_and_nonboolean_types() {
    for ty in [SIGNED, NARROW, BOOL] {
        let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
        let mut statements = fixture.blocks[2].statements().to_vec();
        statements[1] = assign(
            5,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::GreaterOrEqual,
                left: copy(4, ty),
                right: constant(
                    ty,
                    if ty == BOOL { 1 } else { 16 },
                    if ty == NARROW {
                        4
                    } else if ty == BOOL {
                        1
                    } else {
                        8
                    },
                ),
            },
        );
        fixture.replace_block(2, statements, switch(BOOL, 3, 4));
        assert!(fixture.project(0).unwrap().0[5].is_none());
    }
    for ty in [WORD, NARROW] {
        let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
        fixture.replace_block(2, fixture.blocks[2].statements().to_vec(), switch(ty, 3, 4));
        assert!(fixture.project(0).unwrap().0[5].is_none());
    }
}

#[test]
fn unsigned_comparison_requires_every_use_and_operand_definition_to_be_dominated() {
    let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    fixture.replace_block(0, vec![], switch(BOOL, 1, 4));
    assert!(fixture.project(0).unwrap().0[5].is_none());
    let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    let statements = fixture.blocks[2].statements().to_vec();
    fixture.replace_block(
        2,
        vec![statements[1].clone(), statements[0].clone()],
        switch(BOOL, 3, 4),
    );
    assert!(fixture.project(0).unwrap().0[5].is_none());
    let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    fixture.replace_block(
        3,
        vec![assign(
            4,
            WORD,
            SemanticRvalueKindV1::Use(constant(WORD, 0, 8)),
        )],
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
    );
    assert!(fixture.project(0).unwrap().0[5].is_none());
}

#[test]
fn unsigned_comparison_rejects_non_total_masks_and_opaque_producers() {
    let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    let mut statements = fixture.blocks[2].statements().to_vec();
    statements[0] = assign(
        4,
        WORD,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::BitAnd,
            left: copy(3, WORD),
            right: constant(WORD, 10, 8),
        },
    );
    fixture.replace_block(2, statements, switch(BOOL, 3, 4));
    assert!(fixture.project(0).unwrap().0[5].is_none());
    let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    fixture.callables[0] = callable(
        12,
        abi(vec![], WITNESS),
        SemanticCompilerIntrinsicOperationV1::Trap,
    );
    assert!(fixture.project(0).unwrap().0[5].is_none());
}

#[test]
fn unsigned_comparison_charges_existing_request_budget() {
    let fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    assert!(matches!(
        fixture.project(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 1),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
}

#[test]
fn unsigned_comparison_preserves_copy_and_move_but_rejects_wrong_constant_width() {
    let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    let mut statements = fixture.blocks[2].statements().to_vec();
    statements[1] = assign(
        5,
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::GreaterOrEqual,
            left: SemanticOperandV1::Move(place(4, WORD)),
            right: constant(WORD, 16, 8),
        },
    );
    let SemanticTerminatorKindV1::SwitchInt { targets, .. } = switch(BOOL, 3, 4) else {
        panic!()
    };
    fixture.replace_block(
        2,
        statements.clone(),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Move(place(5, BOOL)),
            targets,
        },
    );
    assert!(fixture.project(0).unwrap().0[5].is_some());
    statements[1] = assign(
        5,
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::GreaterOrEqual,
            left: copy(4, WORD),
            right: constant(WORD, 16, 4),
        },
    );
    fixture.replace_block(2, statements, switch(BOOL, 3, 4));
    assert!(fixture.project(0).unwrap().0[5].is_none());
}

#[test]
fn unsigned_comparison_unwind_and_mutable_borrow_do_not_create_invocation_authority() {
    for producer in [0, 1] {
        let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
        let original = fixture.blocks[producer].clone();
        let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
            panic!()
        };
        fixture.replace_block(
            producer,
            original.statements().to_vec(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    call.arguments().to_vec(),
                    call.destination().cloned(),
                    SemanticUnwindActionV1::Continue,
                )
                .unwrap(),
            ),
        );
        assert!(fixture.project(0).unwrap().0[5].is_none());
    }
    let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    fixture.locals.push(REFERENCE);
    let mut statements = fixture.blocks[2].statements().to_vec();
    statements.insert(
        1,
        assign(
            6,
            REFERENCE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(4, WORD),
            },
        ),
    );
    fixture.replace_block(2, statements, switch(BOOL, 3, 4));
    assert!(fixture.project(0).unwrap().0[5].is_none());
}

#[test]
fn unsigned_comparison_one_bad_switch_use_cannot_share_another_uses_fact() {
    let mut fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    assert!(fixture.project(0).unwrap().0[5].is_some());
    // Keep the complete valid producer/consumer graph, but add another switch
    // use not dominated by the comparison. No local-global success cache applies.
    fixture.blocks.push(block(5, vec![], switch(BOOL, 3, 4)));
    assert!(fixture.project(0).unwrap().0[5].is_none());
}

#[test]
fn unsigned_comparison_cannot_borrow_a_different_function_or_type_owner() {
    let fixture = Fixture::new(SemanticBinaryOpV1::GreaterOrEqual, 16, false);
    let function = fixture.function();
    let other_function = function.clone();
    let other_types = fixture.types.clone();
    let inventory = assertion_definition_inventory(&function).unwrap();
    for (types, body) in [(&fixture.types, &other_function), (&other_types, &function)] {
        let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
        let constants = vec![None; fixture.locals.len()];
        let mut arguments = vec![None; fixture.locals.len()];
        let mut next_argument = 0;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let result = TotalUnsignedIndexProjectorV1::new(
            types,
            body,
            &constants,
            &inventory.counts,
            &inventory.address_escaped,
            &inventory.assignments,
            &mut proof,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        );
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "pure uniform index tables do not match the semantic local table"
            ))
        ));
    }
}

#[test]
fn unsigned_comparison_switch_storage_accounts_heads_and_reserved_entries() {
    let blocks = 8;
    let locals = MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 - 2 * blocks;
    let uses = SwitchUsesV1::new(locals, blocks).unwrap();
    assert_eq!(uses.heads.len(), locals);
    assert!(uses.heads.iter().all(|head| *head == usize::MAX));
    assert!(uses.entries.is_empty());
    assert!(uses.entries.capacity() >= blocks);
    assert!(
        uses.heads.capacity() + 2 * uses.entries.capacity()
            <= MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1
    );
    for (locals, blocks) in [(locals + 1, blocks), (usize::MAX, 1), (0, usize::MAX)] {
        assert!(matches!(
            SwitchUsesV1::new(locals, blocks),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "unsigned comparison switch-use storage exceeds the state limit"
            ))
        ));
    }
}
