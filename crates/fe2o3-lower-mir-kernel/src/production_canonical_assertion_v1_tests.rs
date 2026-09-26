use super::super::super::super::assert_origins_v1_tests::{
    Fixture, materialize as original_assertion_owner,
};
use super::*;
use ProductionCanonicalAssertionFailureV1 as Af;
use fe2o3_mir_model::SemanticAssertionProofKindV1 as Proof;
use fe2o3_mir_model::semantic_mir_v1::SemanticCheckedBinaryRvalueV1;

fn literal(expected: bool, shared: bool) -> ProductionPreRankedKirOwnerV1 {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    original_assertion_owner(Fixture::Literal(expected), shared, &mut budget)
}
fn run<T>(
    owner: &ProductionPreRankedKirOwnerV1,
    callback: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalAssertionSourcePoliciesV1<'s, 'm, 'g>,
        &mut Budget<'_>,
    ) -> Result<T, Af>,
) -> Result<T, ProductionCanonicalAssertionChecksErrorV1> {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
            Ok(view.with_assertion_policy_checks_v1(budget, callback))
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}
fn copy_function(
    function: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let out = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    match function.kernel_entry() {
        Some(entry) => out.with_kernel_entry(entry.clone()),
        None => out,
    }
}
fn rebuild(
    owner: &ProductionPreRankedKirOwnerV1,
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> ProductionPreRankedKirOwnerV1 {
    cr_owner_from_ssa(rebuild_ssa(owner, types, functions))
}
fn rebuild_ssa(
    owner: &ProductionPreRankedKirOwnerV1,
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let source = owner.semantic_ssa().source_semantic();
    let admitted = InertSemanticMirRequestV1::new(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        source.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}
fn operand(ty: SemanticTypeIdV1, bits: u128, width: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, width).unwrap()),
    ))
}
fn body(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let provenance = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        provenance,
        statements,
        SemanticTerminatorV1::new(provenance, kind),
    )
    .unwrap()
}
fn assertion(
    condition: SemanticOperandV1,
    expected: bool,
    message: SemanticAssertMessageV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        target: SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::AssertSuccess,
            SemanticBlockIdV1::from_index(target),
        ),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}
fn two_assertions() -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let function = &source.functions()[0];
    let bool_ty = SemanticTypeIdV1::from_index(1);
    let blocks = vec![
        body(
            171,
            vec![],
            assertion(
                operand(bool_ty, 1, 1),
                true,
                SemanticAssertMessageV1::NullPointerDereference,
                1,
            ),
        ),
        body(
            172,
            vec![],
            assertion(
                operand(bool_ty, 0, 1),
                false,
                SemanticAssertMessageV1::NullPointerDereference,
                2,
            ),
        ),
        body(173, vec![], SemanticTerminatorKindV1::Return),
    ];
    rebuild(
        &seed,
        source.types().to_vec(),
        vec![copy_function(function, function.locals().to_vec(), blocks)],
    )
}

fn local(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap(),
    )
}
fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let place = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place,
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn extra_locals(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeIdV1],
) -> Vec<SemanticLocalDeclV1> {
    let mut out = function.locals().to_vec();
    for (i, ty) in types.iter().enumerate() {
        out.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([191 + i as u8; 32]),
            *ty,
            SemanticLocalRoleV1::Temporary,
            function.source(),
        ));
    }
    out
}
fn word_type() -> SemanticTypeDeclV1 {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let owner = original_assertion_owner(Fixture::ElidedBounds, false, &mut budget);
    owner.semantic_ssa().source_semantic().types()[3].clone()
}
fn shift_owner(amount: u128) -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(false, false);
    let source = seed.semantic_ssa().source_semantic();
    let function = &source.functions()[0];
    let word = SemanticTypeIdV1::from_index(2);
    let bool_ty = SemanticTypeIdV1::from_index(1);
    let mut types = source.types().to_vec();
    types.push(word_type());
    let blocks = vec![
        body(
            174,
            vec![assign(
                1,
                bool_ty,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::GreaterOrEqual,
                    left: operand(word, amount, 8),
                    right: operand(word, 64, 8),
                },
            )],
            assertion(
                local(1, bool_ty),
                false,
                SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::ShiftLeft,
                    left: operand(word, 1, 8),
                    right: operand(word, amount, 8),
                },
                1,
            ),
        ),
        body(175, vec![], SemanticTerminatorKindV1::Return),
    ];
    rebuild(
        &seed,
        types,
        vec![copy_function(
            function,
            extra_locals(function, &[bool_ty]),
            blocks,
        )],
    )
}
fn elided_owner() -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let function = &source.functions()[0];
    let word = SemanticTypeIdV1::from_index(2);
    let boolean = SemanticTypeIdV1::from_index(1);
    let mut types = source.types().to_vec();
    types.push(word_type());
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let blocks = vec![
        body(
            176,
            vec![assign(
                1,
                word,
                SemanticRvalueKindV1::Use(operand(word, 8, 8)),
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: local(1, word),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        8,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                )
                .unwrap(),
            },
        ),
        body(
            177,
            vec![
                assign(2, word, SemanticRvalueKindV1::Use(operand(word, 1, 8))),
                assign(
                    3,
                    word,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::BitAnd,
                        left: local(1, word),
                        right: operand(word, 63, 8),
                    },
                ),
                assign(
                    4,
                    boolean,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: local(3, word),
                        right: operand(word, 64, 8),
                    },
                ),
            ],
            assertion(
                local(4, boolean),
                true,
                SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::ShiftLeft,
                    left: local(2, word),
                    right: local(3, word),
                },
                2,
            ),
        ),
        body(
            178,
            vec![assign(
                5,
                word,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::ShiftLeft,
                    left: local(2, word),
                    right: local(3, word),
                },
            )],
            SemanticTerminatorKindV1::Return,
        ),
        body(181, vec![], SemanticTerminatorKindV1::Return),
    ];
    rebuild(
        &seed,
        types,
        vec![copy_function(
            function,
            extra_locals(function, &[word, word, word, boolean, word]),
            blocks,
        )],
    )
}
fn unit_assertion(message: SemanticAssertMessageV1) -> ProductionPreRankedKirOwnerV1 {
    cr_owner_from_ssa(unit_assertion_ssa(message))
}
fn unit_assertion_ssa(message: SemanticAssertMessageV1) -> ProductionSemanticSsaOwnerV1 {
    let erased = erased();
    let seed = erased.original_source();
    let source = seed.semantic_ssa().source_semantic();
    let helper = &source.functions()[1];
    let boolean = SemanticTypeIdV1::from_index(source.types().len() as u32);
    let mut types = source.types().to_vec();
    let literal = literal(true, false);
    let bool_decl = &literal.semantic_ssa().source_semantic().types()[1];
    // The helper's U64 identity is 201; preserve its type index and append Bool.
    let bool_identity = SemanticTypeIdentityV1::from_sha256([203; 32]);
    assert!(types.last().unwrap().identity() < bool_identity);
    // The literal Bool's layout identity 2 already names the seed's Unit layout.
    let bool_layout_identity = SemanticLayoutIdentityV1::from_sha256([204; 32]);
    assert!(
        types
            .iter()
            .all(|ty| ty.layout_identity() != bool_layout_identity)
    );
    types.push(
        SemanticTypeDeclV1::new(
            bool_identity,
            bool_layout_identity,
            bool_decl.layout().clone(),
            bool_decl.shape().clone(),
        )
        .with_rustc_abi_properties(bool_decl.abi_properties())
        .with_rust_type_kind(bool_decl.rust_type_kind()),
    );
    let blocks = vec![
        body(
            179,
            helper.blocks()[0].statements().to_vec(),
            assertion(operand(boolean, 1, 1), true, message, 1),
        ),
        body(180, vec![], SemanticTerminatorKindV1::Return),
    ];
    let mut functions = source.functions().to_vec();
    functions[1] = copy_function(helper, helper.locals().to_vec(), blocks);
    rebuild_ssa(seed, types, functions)
}

fn checked_owner(left: u128, right: u128, message_left: u128) -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(false, false);
    let source = seed.semantic_ssa().source_semantic();
    let function = &source.functions()[0];
    let boolean = SemanticTypeIdV1::from_index(1);
    let word = SemanticTypeIdV1::from_index(2);
    let pair = SemanticTypeIdV1::from_index(3);
    let mut types = source.types().to_vec();
    types.push(word_type());
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([198; 32]),
        SemanticLayoutIdentityV1::from_sha256([199; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![word, boolean]).unwrap()),
    ));
    let flag = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), boolean).unwrap()],
            boolean,
        )
        .unwrap(),
    );
    let blocks = vec![
        body(
            187,
            vec![assign(
                1,
                pair,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    operand(word, left, 8),
                    operand(word, right, 8),
                )),
            )],
            assertion(
                flag,
                false,
                SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: operand(word, message_left, 8),
                    right: operand(word, right, 8),
                },
                1,
            ),
        ),
        body(188, vec![], SemanticTerminatorKindV1::Return),
    ];
    rebuild(
        &seed,
        types,
        vec![copy_function(
            function,
            extra_locals(function, &[pair]),
            blocks,
        )],
    )
}

#[test]
fn assertion_true_and_false_use_actual_original_edges_and_all_nine_reports() {
    for expected in [true, false] {
        let owner = literal(expected, false);
        let before = owner.executable().canonical().canonical_bytes().to_vec();
        run(&owner, |view, budget| {
            let count = view.assertion_count(budget)?;
            assert_eq!(count, 1);
            let assertion = view.assertion(0, budget)?;
            assert_eq!(assertion.proof_kind(), Proof::ExactRange);
            assert_eq!(assertion.binding().expected(), expected);
            let SemanticKirAssertConditionOutcomeV1::Emitted {
                success_edge,
                failure_edge,
                ..
            } = assertion.binding().outcome()
            else {
                panic!("actual retained assertion");
            };
            assert_eq!(success_edge.successor, u32::from(!expected));
            assert_eq!(failure_edge.successor, u32::from(expected));
            let policies = view.policies(budget)?;
            assert!(std::ptr::eq(policies.owner(budget)?, owner.executable()));
            assert_eq!(policies.module_function_count(budget)?, 2);
            assert_eq!(policies.definition_count(budget)?, 1);
            assert_eq!(policies.pair_count(budget)?, 1);
            let coordinate = policies.definition_coordinate(0, budget)?;
            assert_eq!(
                policies.history(0, budget)?.function(),
                coordinate.0 as usize
            );
            let report = policies.report(0, budget)?;
            assert_eq!(report.paired_stage_count(), 9);
            assert_eq!(report.reports().pass_order().len(), 9);
            assert!(report.reports().is_clean());
            assert_eq!(policies.pending_obligations().iter().count(), 19);
            assert!(!view.ranked_verification_is_complete());
            assert!(!view.grants_artifact_or_launch_authority());
            Ok(())
        })
        .unwrap();
        assert_eq!(owner.executable().canonical().canonical_bytes(), before);
    }
}
#[test]
fn shared_sink_requires_both_distinct_assertion_occurrences() {
    let owner = two_assertions();
    run(&owner, |view, budget| {
        assert_eq!(view.assertion_count(budget)?, 2);
        let policies = view.policies(budget)?;
        assert_eq!(policies.pair_count(budget)?, 1);
        assert_eq!(policies.pair(0, budget)?.incoming_edges().len(), 2);
        Ok(())
    })
    .unwrap();
}
#[test]
fn every_root_qualified_shared_helper_alias_is_proved_not_deduplicated() {
    let owner = literal(true, true);
    assert_eq!(owner.assert_origins().source_site_count(), 2);
    assert_eq!(owner.assert_origins().binding_count(), 1);
    run(&owner, |view, budget| {
        assert_eq!(view.assertion_count(budget)?, 2);
        let first = view.assertion(0, budget)?.binding();
        assert_eq!(first, view.assertion(1, budget)?.binding());
        let policies = view.policies(budget)?;
        assert_eq!(policies.definition_count(budget)?, 4);
        assert_eq!(policies.module_function_count(budget)?, 5);
        assert_eq!(policies.pair_count(budget)?, 1);
        assert_eq!(policies.pair(0, budget)?.incoming_edges().len(), 1);
        for ordinal in 0..4 {
            assert_eq!(policies.report(ordinal, budget)?.paired_stage_count(), 9);
            assert_eq!(
                policies.history(ordinal, budget)?.function(),
                policies.definition_coordinate(ordinal, budget)?.0 as usize
            );
        }
        assert_eq!(
            view.callables(budget)?.len(),
            view.metadata(budget)?.function_count(budget)?
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn old_scalar_and_private_entries_remain_closed_on_the_same_assertion_owner() {
    let owner = literal(true, false);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
            assert!(view.with_policy_checks_v1(budget, |_, _| Ok(())).is_err());
            Ok(())
        })
        .unwrap();
    owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
            assert!(
                view.with_private_policy_checks_v1(budget, |_, _| Ok(()))
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
}
#[test]
fn old_private_none_gate_retains_exact_two_work_assertion_refusal() {
    let owner = literal(true, false);
    cr_run_owner(&owner, 1 << 48, S, |source, budget| {
        let before = budget.work();
        let paid = budget.storage();
        assert!(matches!(
            cr_private_source_profile_v1(source, budget),
            Err(ProductionCanonicalRankedPolicyErrorV1::Unsupported {
                requirement: ProductionCanonicalRankedSourceRequirementV1::Assertion,
                ordinal: 0,
            })
        ));
        assert_eq!(budget.work() - before, 2);
        assert_eq!(budget.storage(), paid);
        Ok(())
    })
    .0
    .unwrap();
}
#[test]
fn private_memory_is_not_upgraded_to_empty_or_deterministic_by_new_consumer() {
    let erased = erased();
    run(erased.original_source(), |view, budget| {
        assert_eq!(view.memory_census(budget)?, [1, 2]);
        let rows = view.callables(budget)?;
        assert!(rows.iter().any(|row| row.kind()
            == ProductionCanonicalAssertionCallKindV1::PrivateFrame
            && row.source_decision() == fe2o3_mir_model::SemanticCallableDecisionV1::Rejected));
        assert_eq!(view.assertion_count(budget)?, 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn failure_panic_and_ignored_query_poison_preserve_completed_policy_prefix() {
    for mode in 0..3 {
        let owner = literal(true, false);
        let error = run(&owner, |view, budget| -> Result<(), Af> {
            match mode {
                0 => Err(Af::Binding {
                    span: None,
                    detail: "callback rejection",
                }),
                1 => panic!("assertion callback"),
                _ => {
                    assert!(view.assertion(999, budget).is_err());
                    Ok(())
                }
            }
        })
        .unwrap_err();
        assert!(error.policies.is_some());
        assert!(error.source.is_some());
        match mode {
            0 => assert!(matches!(
                error.failure,
                Af::Binding {
                    detail: "callback rejection",
                    ..
                }
            )),
            1 => assert!(matches!(error.failure, Af::Panicked)),
            _ => assert!(matches!(error.failure, Af::Policy(_))),
        }
    }
}

#[test]
fn unsigned_literal_shift_has_fresh_exact_range_proof() {
    let owner = shift_owner(3);
    run(&owner, |view, budget| {
        assert_eq!(view.assertion_count(budget)?, 1);
        assert_eq!(view.assertion(0, budget)?.proof_kind(), Proof::ExactRange);
        Ok(())
    })
    .unwrap();
}

fn signed_literal_shift_owner(
    width: u16,
    count: u128,
    cast_width: u16,
) -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let function = &source.functions()[0];
    let word = SemanticTypeIdV1::from_index(2);
    let signed = SemanticTypeIdV1::from_index(3);
    let cast = SemanticTypeIdV1::from_index(4);
    let boolean = SemanticTypeIdV1::from_index(1);
    let mut types = source.types().to_vec();
    types.push(word_type());
    for (tag, signed, bits) in [(231, true, width), (233, false, cast_width)] {
        let bytes = u64::from(bits / 8);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                bytes,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                    SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
        ));
    }
    let count = operand(signed, count, (width / 8) as u8);
    let blocks = vec![
        body(
            182,
            vec![
                assign(1, word, SemanticRvalueKindV1::Use(operand(word, 1, 8))),
                assign(
                    2,
                    cast,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: count.clone(),
                    },
                ),
                assign(
                    3,
                    boolean,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: local(2, cast),
                        right: operand(cast, 64, (cast_width / 8) as u8),
                    },
                ),
            ],
            assertion(
                local(3, boolean),
                true,
                SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::ShiftLeft,
                    left: local(1, word),
                    right: count.clone(),
                },
                1,
            ),
        ),
        body(
            183,
            vec![assign(
                4,
                word,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::ShiftLeft,
                    left: local(1, word),
                    right: count,
                },
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ];
    rebuild(
        &seed,
        types,
        vec![copy_function(
            function,
            extra_locals(function, &[word, cast, boolean, word]),
            blocks,
        )],
    )
}

#[test]
fn signed_literal_shift_preserves_the_distinct_rustc_literal_rule() {
    for width in [8, 16, 32, 64] {
        let owner = signed_literal_shift_owner(width, 3, width);
        run(&owner, |view, budget| {
            assert_eq!(view.assertion_count(budget)?, 1);
            let row = view.assertion(0, budget)?;
            assert_eq!(row.proof_kind(), Proof::LiteralShift);
            assert!(row.binding().expected());
            assert!(matches!(
                row.binding().outcome(),
                SemanticKirAssertConditionOutcomeV1::Emitted { .. }
            ));
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn signed_literal_shift_negative_and_wrong_cast_do_not_acquire_a_fact() {
    for (count, cast) in [(u32::MAX.into(), 32), (3, 64)] {
        let owner = signed_literal_shift_owner(32, count, cast);
        let error = run(&owner, |_, _| -> Result<(), Af> {
            panic!("unproved shift cannot enter callback")
        })
        .unwrap_err();
        assert!(matches!(error.failure, Af::NotProved { .. }));
    }
}
#[test]
fn zero_emission_assertion_still_has_fresh_source_proof_and_actual_success_edge() {
    let owner = elided_owner();
    let blocks = || {
        owner
            .executable()
            .module()
            .functions
            .iter()
            .flat_map(|function| function.body.iter())
            .flat_map(|body| &body.blocks)
    };
    assert!(blocks().any(|block| matches!(
        block.terminator,
        Some(fe2o3_kernel_ir::Terminator::Switch { .. })
    )));
    assert!(
        blocks()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                fe2o3_kernel_ir::OperationKind::Binary {
                    op: fe2o3_kernel_ir::BinaryOp::ShiftLeft,
                    ..
                }
            ))
    );
    let mut elided = false;
    cr_run_owner(&owner, 1 << 48, S, |source, budget| {
        elided = matches!(
            source.assertions(budget)?[0].binding().outcome(),
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
        );
        Ok(())
    })
    .0
    .unwrap();
    assert!(
        elided,
        "genuine existing source rule must actually elide the condition"
    );
    run(&owner, |view, budget| {
        assert_eq!(view.assertion_count(budget)?, 1);
        assert_eq!(view.assertion(0, budget)?.proof_kind(), Proof::ExactRange);
        assert!(matches!(
            view.assertion(0, budget)?.binding().outcome(),
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
        ));
        assert_eq!(view.policies(budget)?.pair_count(budget)?, 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn actual_unitlocal_memory_and_supported_retained_helper_assertion_compose() {
    let word = SemanticTypeIdV1::from_index(1);
    let owner = unit_assertion(SemanticAssertMessageV1::BoundsCheck {
        length: operand(word, 8, 8),
        index: operand(word, 0, 8),
    });
    run(&owner, |view, budget| {
        assert_eq!(view.memory_census(budget)?, [1, 2]);
        assert_eq!(view.assertion_count(budget)?, 1);
        assert_eq!(view.assertion(0, budget)?.proof_kind(), Proof::ExactRange);
        assert!(matches!(
            view.assertion(0, budget)?.binding().outcome(),
            SemanticKirAssertConditionOutcomeV1::Emitted { .. }
        ));
        assert_eq!(view.policies(budget)?.pair_count(budget)?, 1);
        assert!(
            view.callables(budget)?
                .iter()
                .any(|row| row.kind() == ProductionCanonicalAssertionCallKindV1::PrivateFrame)
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn actual_checked_arithmetic_keeps_finite_width_source_proof_kind() {
    let owner = checked_owner(2, 3, 2);
    run(&owner, |view, budget| {
        assert_eq!(view.assertion_count(budget)?, 1);
        assert_eq!(
            view.assertion(0, budget)?.proof_kind(),
            Proof::CheckedArithmetic
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn actual_sparse_constants_independently_veto_opposite_polarity() {
    for expected in [true, false] {
        let owner = literal(expected, false);
        cr_run_owner(&owner, 1 << 48, S, |source, budget| {
            let paid = budget.storage();
            canonical_assertion_v1::read_test_sparse(source, budget).unwrap();
            assert_eq!(budget.storage(), paid);
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn assertion_callback_queries_reject_foreign_and_moved_slots_without_debit() {
    let owner = literal(true, false);
    for moved in [false, true] {
        let mut work = Work::new(1 << 48);
        let mut other_work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let mut foreign = Budget::new(&mut other_work, S);
        let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        foreign.reserve_storage(SIBLING).unwrap();
        let result = owner
            .with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                Ok(
                    source.with_assertion_policy_checks_v1(budget, |view, budget| {
                        if moved {
                            std::mem::swap(budget, &mut foreign);
                        }
                        let before = foreign.work();
                        assert!(matches!(
                            view.assertion_count(&mut foreign),
                            Err(Af::Policy(ProductionCanonicalRankedPolicyErrorV1::Source(
                                CrError::Resource(ArgumentResourceV1::Accounting)
                            )))
                        ));
                        assert_eq!(foreign.work(), before);
                        if moved {
                            std::mem::swap(budget, &mut foreign);
                        }
                        Ok(())
                    }),
                )
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            result.failure,
            Af::Policy(ProductionCanonicalRankedPolicyErrorV1::Source(
                CrError::Resource(ArgumentResourceV1::Accounting)
            ))
        ));
        assert!(result.source.is_some());
        assert!(result.policies.is_some());
        assert_eq!(budget.storage(), floor);
        assert_eq!((foreign.storage(), foreign.work()), (SIBLING, 0));
    }
}

#[test]
fn assertion_callback_rejected_result_owns_the_displaced_paid_budget() {
    use std::cell::{Cell, RefCell};
    struct PaidDrop<'a, 'w> {
        original: Option<Budget<'w>>,
        saved: &'a RefCell<Option<Budget<'w>>>,
        observed: &'a Cell<usize>,
        drops: &'a Cell<usize>,
        panic_on_drop: bool,
    }
    impl Drop for PaidDrop<'_, '_> {
        fn drop(&mut self) {
            let original = self.original.take().unwrap();
            self.observed.set(original.storage());
            self.drops.set(self.drops.get() + 1);
            *self.saved.borrow_mut() = Some(original);
            if self.panic_on_drop {
                panic!("rejected assertion callback destructor");
            }
        }
    }
    let owner = literal(true, false);
    for panic_on_drop in [false, true] {
        let mut work = Work::new(1 << 48);
        let mut other_work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let mut foreign = Budget::new(&mut other_work, S);
        foreign.reserve_storage(SIBLING).unwrap();
        let saved = RefCell::new(None);
        let observed = Cell::new(0);
        let drops = Cell::new(0);
        let paid = Cell::new(0);
        let floor = owner.unit_local_source_storage_floor_v1().unwrap()
            + std::mem::size_of::<PaidDrop<'_, '_>>()
            + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result =
            owner.with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                Ok(
                    source.with_assertion_policy_checks_v1(budget, |view, budget| {
                        assert_eq!(view.assertion_count(budget)?, 1);
                        paid.set(budget.storage());
                        Ok(PaidDrop {
                            original: Some(std::mem::replace(budget, foreign)),
                            saved: &saved,
                            observed: &observed,
                            drops: &drops,
                            panic_on_drop,
                        })
                    }),
                )
            });
        assert!(matches!(
            result,
            Err(CrError::Resource(ArgumentResourceV1::Accounting))
                | Err(CrError::Analysis(
                    fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Resource(
                        ArgumentResourceV1::Accounting
                    )
                ))
        ));
        assert_eq!(drops.get(), 1);
        assert_eq!(observed.get(), paid.get());
        assert!(paid.get() > floor);
        assert_eq!((budget.storage(), budget.work()), (SIBLING, 0));
        let original = saved.borrow_mut().take().unwrap();
        assert!(original.work_ledger_identity_v1() == ledger);
        assert_eq!(original.storage(), paid.get());
        let _replacement = std::mem::replace(&mut budget, original);
        budget.release_storage(paid.get() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn assertion_real_panic_payload_drops_once_and_preserves_both_observations() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let owner = literal(true, false);
    let drops = Arc::new(AtomicUsize::new(0));
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap()
        + std::mem::size_of::<Payload>()
        + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let error = owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
            Ok(
                source.with_assertion_policy_checks_v1(budget, |_, _| -> Result<(), Af> {
                    std::panic::panic_any(Payload(Arc::clone(&drops)));
                }),
            )
        })
        .unwrap()
        .unwrap_err();
    assert!(matches!(error.failure, Af::Panicked));
    assert!(error.policies.is_some());
    assert!(error.source.is_some());
    // Real payload destruction is observed; an aliased Budget during Drop is not.
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn assertion_final_three_paid_queries_have_literal_exact_and_one_short_work() {
    const LIMIT: usize = 1 << 48;
    let owner = literal(true, false);
    for remaining in [3, 2] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, S);
        let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let mut entered = false;
        let result = owner
            .with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                Ok(
                    source.with_assertion_policy_checks_v1(budget, |view, budget| {
                        entered = true;
                        let before = budget.work();
                        assert_eq!(view.assertion_count(budget)?, 1);
                        assert_eq!(budget.work() - before, 3);
                        // Own guard, source guard, and C owner are each one paid query.
                        // Other enclosing postflights are checks, not work debits.
                        let target = LIMIT - remaining;
                        budget.charge_work(target.checked_sub(budget.work()).unwrap())?;
                        Ok(())
                    }),
                )
            })
            .unwrap();
        assert!(entered);
        if remaining == 3 {
            result.unwrap();
        } else {
            let error = result.unwrap_err();
            assert!(
                matches!(error.failure, Af::Policy(ProductionCanonicalRankedPolicyErrorV1::Policy(ref error))
                if matches!(error.failure(), fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(ArgumentResourceV1::Work(limit))
                    if limit.actual() == LIMIT + 1 && limit.limit() == LIMIT))
            );
            assert!(error.policies.is_some());
            assert!(error.source.is_some());
        }
        assert_eq!(budget.work(), LIMIT);
        assert_eq!(budget.storage(), floor);
    }
}

mod scalar_assertion_history {
    include!("production_canonical_scalar_assertion_v1_tests.rs");
    mod hostile {
        include!("production_canonical_scalar_assertion_hostile_v1_tests.rs");
    }
}
