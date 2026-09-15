//! Synthetic transparency components. Numerical/source/loan authority is not
//! manufactured by these fixtures; actual Matrix callbacks remain the oracle.
use super::super::super::closed_lane_flow;
use super::*;

fn leaf_callable(kind: u8, shared: bool) -> SemanticCallableDeclV1 {
    let profile = match kind {
        3 => SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
        4 => SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
        _ => SemanticMfmaProfileV1::Bf16F32M16N16K16,
    };
    let operand = SemanticMfmaOperandContractV1 {
        role: SemanticMfmaOperandRoleV1::A,
        profile,
        register_distribution: if kind >= 3 {
            SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128
        } else {
            SemanticMfmaRegisterDistributionV1::Tile16x16
        },
        wave_width: 64,
    };
    let operation = match kind {
        0 => SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
            lane: ty(2),
            fragment: ty(0),
            contract: SemanticMfmaAccumulatorContractV1 {
                profile,
                distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
                wave_width: 64,
            },
        },
        1 => SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad {
            option_fragment: ty(0),
            view: ty(6),
            lane: ty(2),
            fragment: ty(0),
            contract: operand,
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        },
        2 => SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
            fragment: ty(0),
            view: ty(6),
            lane: ty(2),
            contract: operand,
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        },
        3 => SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixLoadM16K128 {
            fragment: ty(0),
            view: ty(6),
            lane: ty(2),
            contract: operand,
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        },
        4 => SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixLoadM16K128 {
            fragment: ty(0),
            view: ty(6),
            lane: ty(2),
            contract: operand,
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        },
        _ => panic!("fixture consumer"),
    };
    let inputs: &[u32] = if kind == 0 { &[7] } else { &[8, 7, 1, 1] };
    let ownership = inputs
        .iter()
        .map(|id| {
            if shared && matches!(*id, 7 | 8) {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            } else {
                SemanticSourceArgumentOwnershipV1::ByValue
            }
        })
        .collect();
    let source_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([190; 32]),
        SemanticLayoutIdentityV1::from_sha256([191; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        inputs.len() as u32,
        inputs.iter().copied().map(ty).collect(),
        ty(0),
        inputs
            .iter()
            .map(|&id| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty(id),
                    SemanticAbiPassModeV1::Direct(attributes(matches!(id, 7 | 8), false)),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([192; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([192; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([192; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([192; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([192; 32]),
            source(),
            source_abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([192; 32]),
    }
}

fn scalar() -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
    ))
}
fn leaf_call(kind: u8, reference: u32, next: u32) -> SemanticTerminatorKindV1 {
    let lane = SemanticOperandV1::Copy(place(reference, 7));
    let args = if kind == 0 {
        vec![lane]
    } else {
        vec![
            SemanticOperandV1::Copy(place(7, 8)),
            lane,
            scalar(),
            scalar(),
        ]
    };
    call(1, args, place(16, 0), next)
}
fn used_body(kind: u8, reborrow: bool) -> SemanticFunctionDeclV1 {
    let body = fixture(Mutation::None);
    let mut locals = body.locals().to_vec();
    locals.extend([
        local(14, 7, SemanticLocalRoleV1::Temporary),
        local(15, 7, SemanticLocalRoleV1::Temporary),
        local(16, 0, SemanticLocalRoleV1::Temporary),
    ]);
    // The original getter result is transported through a real tuple move and
    // a projected shared-leaf move, followed by an ordinary argument copy.
    let mut statements = vec![
        assign(
            11,
            7,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(9),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), ty(7))
                            .unwrap(),
                    ],
                    ty(7),
                )
                .unwrap(),
            )),
        ),
        alias(14, 11, 7),
    ];
    if reborrow {
        statements.push(borrow(
            15,
            7,
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(14),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(2))
                        .unwrap(),
                ],
                ty(2),
            )
            .unwrap(),
            SemanticBorrowKindV1::Shared,
        ));
    }
    let mut blocks = body.blocks().to_vec();
    blocks[2] = block(
        2,
        statements,
        leaf_call(kind, if reborrow { 15 } else { 14 }, 3),
    );
    blocks.push(block(3, vec![], SemanticTerminatorKindV1::Return));
    function(180, body.abi().clone(), locals, blocks)
}
fn changed(
    body: &SemanticFunctionDeclV1,
    block_index: usize,
    statements: Vec<SemanticStatementV1>,
    terminal: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    let mut blocks = body.blocks().to_vec();
    blocks[block_index] = block(block_index as u8, statements, terminal);
    function(180, body.abi().clone(), body.locals().to_vec(), blocks)
}
fn lane_relations(
    body: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    limit: usize,
) -> Result<
    (
        BTreeMap<SemanticTransparentBorrowSiteV1, closed_lane_flow::ClosedProjection>,
        usize,
    ),
    ProductionSemanticSsaErrorV1,
> {
    let fact = MatrixAccessBorrow::for_callable(types, &callables[0]).unwrap();
    let mut budget = Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    };
    let relations = closed_lane_flow::sites_with_consumers(
        body,
        Some(types),
        &BTreeMap::from([(0, fact)]),
        callables,
        &mut budget,
    )?;
    Ok((relations, limit - budget.remaining))
}
fn projection() -> SemanticTransparentBorrowSiteV1 {
    SemanticTransparentBorrowSiteV1 {
        block: 1,
        statement: 0,
    }
}

#[test]
fn used_lane_getter_capture_move_copy_and_reborrow_keep_every_exact_borrow_site() {
    for kind in 0..5 {
        for reborrow in [false, true] {
            let body = used_body(kind, reborrow);
            let callables = [access(true, 64, true), leaf_callable(kind, true)];
            let (relations, _) = lane_relations(&body, &types(), &callables, 262_144).unwrap();
            assert_eq!(relations.len(), 1, "kind={kind} reborrow={reborrow}");
            let relation = &relations[&projection()];
            assert_eq!(
                (relation.parent, relation.reference, relation.owned),
                (3, ty(4), ty(3))
            );
            let expected = if reborrow {
                vec![
                    projection(),
                    SemanticTransparentBorrowSiteV1 {
                        block: 2,
                        statement: 2,
                    },
                ]
            } else {
                vec![projection()]
            };
            assert_eq!(relation.borrow_sites, expected);
        }
    }
}

#[test]
fn closed_leaf_abi_operation_reference_and_argument_mutations_reject() {
    let body = used_body(0, true);
    let base = leaf_callable(0, true);
    let mut wrong_op = base.clone();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut wrong_op else {
        panic!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
        fragment: ty(7),
        values: ty(0),
    };
    let mut wrong_lane = base.clone();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut wrong_lane else {
        panic!()
    };
    let SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { lane, .. } = operation
    else {
        panic!()
    };
    *lane = ty(3);
    for callable in [
        leaf_callable(0, false),
        wrong_op,
        wrong_lane,
        access(true, 64, true),
    ] {
        assert!(
            lane_relations(
                &body,
                &types(),
                &[access(true, 64, true), callable],
                262_144
            )
            .unwrap()
            .0
            .is_empty()
        );
    }
    for terminal in [
        call(
            1,
            vec![SemanticOperandV1::Copy(place(15, 7)), scalar()],
            place(16, 0),
            3,
        ),
        call(
            1,
            vec![SemanticOperandV1::Copy(place(15, 7))],
            place(13, 1),
            3,
        ),
        call(
            1,
            vec![SemanticOperandV1::Copy(place(15, 8))],
            place(16, 0),
            3,
        ),
        call(
            1,
            vec![SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(15),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(7))
                            .unwrap(),
                    ],
                    ty(7),
                )
                .unwrap(),
            )],
            place(16, 0),
            3,
        ),
    ] {
        let changed = changed(&body, 2, body.blocks()[2].statements().to_vec(), terminal);
        assert!(
            lane_relations(
                &changed,
                &types(),
                &[access(true, 64, true), base.clone()],
                262_144
            )
            .unwrap()
            .0
            .is_empty()
        );
    }
    for (kind, mutability, pointee) in [
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            2,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            2,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            3,
        ),
    ] {
        let mut types = types();
        types[7] = pointer(7, pointee, kind, mutability);
        assert!(
            lane_relations(
                &body,
                &types,
                &[access(true, 64, true), base.clone()],
                262_144
            )
            .unwrap()
            .0
            .is_empty()
        );
    }
}

#[test]
fn accepted_leaf_never_hides_an_escape_write_duplicate_or_root_return() {
    let body = used_body(0, true);
    let callables = [access(true, 64, true), leaf_callable(0, true)];
    for statement in [
        alias(14, 11, 7),
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Deinitialize(place(14, 7)),
        ),
        assign(
            13,
            1,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(14),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(2))
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(1))
                            .unwrap(),
                    ],
                    ty(1),
                )
                .unwrap(),
            )),
        ),
    ] {
        let changed = changed(&body, 3, vec![statement], SemanticTerminatorKindV1::Return);
        assert!(
            lane_relations(&changed, &types(), &callables, 262_144)
                .unwrap()
                .0
                .is_empty()
        );
    }
    let escape = changed(
        &body,
        3,
        vec![],
        call(
            0,
            vec![SemanticOperandV1::Copy(place(14, 7))],
            place(16, 0),
            3,
        ),
    );
    assert!(
        lane_relations(&escape, &types(), &callables, 262_144)
            .unwrap()
            .0
            .is_empty()
    );
    for terminal in [
        SemanticTerminatorKindV1::TailCall(
            SemanticDirectTailCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![SemanticOperandV1::Copy(place(14, 7))],
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::Assert {
            condition: scalar(),
            expected: true,
            message: SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Copy(place(14, 7))),
            target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::AssertSuccess,
                SemanticBlockIdV1::from_index(3),
            ),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    ] {
        let changed = changed(&body, 3, vec![], terminal);
        assert!(
            lane_relations(&changed, &types(), &callables, 262_144)
                .unwrap()
                .0
                .is_empty()
        );
    }
    let mut locals = body.locals().to_vec();
    locals[0] = local(0, 7, SemanticLocalRoleV1::Return);
    let mut blocks = body.blocks().to_vec();
    blocks[3] = block(3, vec![alias(0, 14, 7)], SemanticTerminatorKindV1::Return);
    let returning = function(180, abi(&[3, 0, 8], 7, false), locals, blocks);
    assert!(
        lane_relations(&returning, &types(), &callables, 262_144)
            .unwrap()
            .0
            .is_empty()
    );
}

#[test]
fn borrowed_leaf_requires_exact_mutability_parent_and_capture_path() {
    let body = used_body(0, true);
    let callables = [access(true, 64, true), leaf_callable(0, true)];
    for kind in [SemanticBorrowKindV1::Mutable] {
        let mut statements = body.blocks()[2].statements().to_vec();
        let SemanticStatementKindV1::Assign(a) = statements[2].kind() else {
            panic!()
        };
        let SemanticRvalueKindV1::Borrow { place, .. } = a.value().kind() else {
            panic!()
        };
        statements[2] = borrow(15, 7, place.clone(), kind);
        let changed = changed(
            &body,
            2,
            statements,
            body.blocks()[2].terminator().kind().clone(),
        );
        assert!(
            lane_relations(&changed, &types(), &callables, 262_144)
                .unwrap()
                .0
                .is_empty()
        );
    }
    let mut statements = body.blocks()[2].statements().to_vec();
    statements[0] = assign(
        11,
        7,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(9),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(7)).unwrap()],
                ty(7),
            )
            .unwrap(),
        )),
    );
    let changed = changed(
        &body,
        2,
        statements,
        body.blocks()[2].terminator().kind().clone(),
    );
    assert!(
        lane_relations(&changed, &types(), &callables, 262_144)
            .unwrap()
            .0
            .is_empty()
    );
}

#[test]
fn closed_used_leaf_has_exact_budget_boundary_and_keeps_passive_fast_rejection() {
    let body = used_body(0, true);
    let callables = [access(true, 64, true), leaf_callable(0, true)];
    let (expected, needed) = lane_relations(&body, &types(), &callables, 262_144).unwrap();
    assert_eq!(
        lane_relations(&body, &types(), &callables, needed)
            .unwrap()
            .0[&projection()]
            .borrow_sites,
        expected[&projection()].borrow_sites
    );
    let error = lane_relations(&body, &types(), &callables, needed - 1)
        .err()
        .unwrap();
    let ProductionSemanticSsaErrorV1::BorrowFlowWork { error, .. } = error else {
        panic!("{error:?}")
    };
    assert!(
        matches!(*error,ProductionSemanticSsaErrorV1::AggregateResourceLimit {
        resource:SsaPlannerResourceV1::WorkUnits,required,limit } if required==needed && limit==needed-1)
    );
    let mut statements = vec![];
    statements.extend(std::iter::repeat_n(
        assign(13, 1, SemanticRvalueKindV1::Use(scalar())),
        32_768,
    ));
    let unknown = changed(
        &body,
        2,
        body.blocks()[2].statements().to_vec(),
        call(
            0,
            vec![SemanticOperandV1::Copy(place(15, 7))],
            place(16, 0),
            3,
        ),
    );
    let unknown = changed(&unknown, 3, statements, SemanticTerminatorKindV1::Return);
    let (relations, work) = lane_relations(&unknown, &types(), &callables, 262_144).unwrap();
    assert!(relations.is_empty());
    assert!(work < 100_000, "work={work}");
}

#[path = "used_lane_tests/ssa_tests.rs"]
mod ssa_tests;

#[path = "used_lane_tests/survivor_tests.rs"]
mod survivor_tests;
