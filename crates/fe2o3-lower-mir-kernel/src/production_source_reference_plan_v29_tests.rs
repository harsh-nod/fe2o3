use super::*;
#[path = "production_source_reference_call_transfer_v29_tests.rs"]
mod call_transfer_tests;
#[path = "production_source_reference_custody_v29_tests.rs"]
mod custody_tests;
#[path = "production_source_reference_fixture_closure_v29_tests.rs"]
mod fixture_closure;
#[path = "production_source_reference_memo_v29_tests.rs"]
mod memo_credit_tests;
#[path = "production_source_reference_no_return_v29_tests.rs"]
mod no_return_tests;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::*;
// Append inside the existing source_reference_plan_v29_tests child.
// Structural source/SSA evidence only; no rustc or allocation authority claim.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JoinCase {
    SameLoan,
    DifferentOrigins,
    DifferentGeneration,
    BranchWrite,
}

fn fixture_word(value: u128) -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
        WORD,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 8).unwrap()),
    )))
}

fn fixture_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn join_capture(source: u32, kind: SemanticBorrowKindV1) -> Vec<SemanticStatementV1> {
    vec![
        assign(
            place(2, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: place(source, WORD),
            },
        ),
        assign(
            place(3, CAPTURE),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![SemanticOperandV1::Move(place(2, REFERENCE))],
                )
                .unwrap(),
            ),
        ),
        dead(2),
    ]
}

fn joined_owner(case: JoinCase) -> ProductionSemanticSsaOwnerV1 {
    joined_owner_with_direct_discriminant(case, false)
}

fn joined_owner_with_direct_discriminant(
    case: JoinCase,
    direct_discriminant: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let mutable = case == JoinCase::BranchWrite;
    owner_with(
        if mutable {
            Case::UniqueRead
        } else {
            Case::Shared
        },
        |_, functions| {
            let kind = if mutable {
                SemanticBorrowKindV1::Mutable
            } else {
                SemanticBorrowKindV1::Shared
            };
            let mut entry = vec![
                assign(place(5, WORD), fixture_word(23)),
                assign(
                    place(6, WORD),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
                ),
            ];
            if case != JoinCase::DifferentOrigins {
                entry.extend(join_capture(1, kind));
            }
            let mut left = Vec::new();
            let mut right = Vec::new();
            match case {
                JoinCase::SameLoan => {}
                JoinCase::DifferentOrigins => {
                    left.extend(join_capture(1, kind));
                    right.extend(join_capture(5, kind));
                }
                JoinCase::DifferentGeneration => {
                    right.push(statement(SemanticStatementKindV1::StorageLive(
                        SemanticLocalIdV1::from_index(5),
                    )));
                    right.push(assign(place(5, WORD), fixture_word(23)));
                }
                JoinCase::BranchWrite => left.push(assign(
                    projected(
                        3,
                        &[
                            (SemanticProjectionKindV1::Field(0), REFERENCE),
                            (SemanticProjectionKindV1::Dereference, WORD),
                        ],
                    ),
                    fixture_word(17),
                )),
            }
            functions[1] = function(
                20,
                false,
                WORD,
                vec![
                    local(20, UNIT, SemanticLocalRoleV1::Return),
                    local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                    local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                    local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
                    local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
                    local(25, WORD, SemanticLocalRoleV1::Temporary),
                    local(26, WORD, SemanticLocalRoleV1::Temporary),
                ],
                vec![
                    block(
                        20,
                        entry,
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: SemanticOperandV1::Copy(place(
                                if direct_discriminant { 1 } else { 6 },
                                WORD,
                            )),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    0,
                                    fixture_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                                )],
                                fixture_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                            )
                            .unwrap(),
                        },
                    ),
                    block(
                        21,
                        left,
                        SemanticTerminatorKindV1::Goto(fixture_edge(SemanticEdgeRoleV1::Goto, 3)),
                    ),
                    block(
                        22,
                        right,
                        SemanticTerminatorKindV1::Goto(fixture_edge(SemanticEdgeRoleV1::Goto, 3)),
                    ),
                    block(
                        23,
                        vec![],
                        call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 4),
                    ),
                    block(24, vec![], SemanticTerminatorKindV1::Return),
                ],
            );
        },
    )
}

#[test]
fn source_reference_diamond_discriminant_cannot_bypass_live_unique_loan() {
    let result = run_owner(
        joined_owner_with_direct_discriminant(JoinCase::BranchWrite, true),
        |_, _| panic!("direct discriminant read bypasses the unique loan"),
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 1,
            block: Some(0),
            statement: None,
            detail: "source reference access bypasses a live loan",
        })
    ));
}

#[test]
fn source_reference_diamond_retains_same_loan_and_unions_branch_write_effects() {
    for case in [JoinCase::SameLoan, JoinCase::BranchWrite] {
        run_owner(joined_owner(case), |plan, budget| {
            assert_eq!(plan.loans.len(), 2);
            for (index, loan) in plan.loans.iter().enumerate() {
                let origin = &plan.origins[loan.origin];
                assert_eq!(origin.local.index(), 1);
                assert_eq!(origin.generation, 0);
                assert_eq!(loan.site.block.index(), 0);
                let joined = plan.blocks.iter().find(|block| {
                    block.instance == origin.instance && block.block.index() == 3
                }).expect("source diamond join snapshot");
                let state = &plan.states[joined.entry];
                assert!(state[2].node.is_none(), "moved holder is dead at both edges");
                assert!(state[1].node.is_some(), "referent remains live at both edges");
                let aggregate = &plan.nodes[state[3].node.unwrap()];
                let SourceReferenceNodeKindV29::Aggregate { first, count } = aggregate.kind else {
                    panic!("captured loan aggregate survives the join");
                };
                assert_eq!(count, 1);
                assert_eq!(plan.nodes[plan.children[first]].kind, SourceReferenceNodeKindV29::Loan(index));
                assert_eq!(loan.effects.referent_reads, 1);
                assert_eq!(loan.effects.referent_writes, usize::from(case == JoinCase::BranchWrite));
                assert_eq!((loan.effects.payload_reads, loan.effects.payload_writes, loan.effects.address_observations), (0, 0, 0));
                if case == JoinCase::BranchWrite {
                    assert_eq!(loan.representation, SourceReferenceRepresentationV29::NeedsAddressable(SourceReferenceCellNeedV29::ReferentWrite));
                    assert!(matches!(plan.require_promoted(index, budget),
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "source reference requires checked addressable storage and writeback", ..
                        })));
                } else {
                    assert_eq!(plan.require_promoted(index, budget)?, SourceReferenceRepresentationV29::StableReferent);
                }
            }
            assert_ne!(plan.origins[plan.loans[0].origin].instance, plan.origins[plan.loans[1].origin].instance);
            Ok(())
        }).unwrap();
    }
}

#[test]
fn source_reference_diamond_rejects_different_origins_and_storage_generations() {
    for (case, expected) in [
        (
            JoinCase::DifferentOrigins,
            "source reference CFG merge changes loan identity",
        ),
        (
            JoinCase::DifferentGeneration,
            "source reference CFG storage generations differ",
        ),
    ] {
        let result = run_owner(joined_owner(case), |_, _| {
            panic!("ambiguous join must refuse")
        });
        assert!(
            matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }) if detail == expected),
            "{case:?}: {result:?}"
        );
    }
}

fn pair_field_owner(overlap: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::UniqueRead, |types, functions| {
        let pair = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([41; 32]),
            SemanticLayoutIdentityV1::from_sha256([41; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD, WORD]).unwrap()),
        ));
        let reference = |field| projected(2, &[(SemanticProjectionKindV1::Field(field), WORD)]);
        let dereference =
            |holder| projected(holder, &[(SemanticProjectionKindV1::Dereference, WORD)]);
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                local(22, pair, SemanticLocalRoleV1::Temporary),
                local(23, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(25, WORD, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(
                20,
                vec![
                    assign(
                        place(2, pair),
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Tuple,
                                vec![
                                    SemanticOperandV1::Copy(place(1, WORD)),
                                    SemanticOperandV1::Copy(place(1, WORD)),
                                ],
                            )
                            .unwrap(),
                        ),
                    ),
                    assign(
                        place(3, REFERENCE),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: reference(0),
                        },
                    ),
                    assign(
                        place(4, REFERENCE),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: reference(if overlap { 0 } else { 1 }),
                        },
                    ),
                    assign(
                        place(5, WORD),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereference(3))),
                    ),
                    assign(dereference(4), fixture_word(17)),
                    dead(3),
                    dead(4),
                    unit(),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        );
    })
}

#[test]
fn source_reference_disjoint_fields_keep_writeback_effects_on_the_actual_referent() {
    run_owner(pair_field_owner(false), |plan, budget| {
        assert_eq!(plan.loans.len(), 4);
        let mut reads = 0;
        let mut writes = 0;
        for (index, loan) in plan.loans.iter().enumerate() {
            let origin = &plan.origins[loan.origin];
            assert_eq!(origin.local.index(), 2);
            assert_eq!(origin.generation, 0);
            assert_eq!(origin.ty, WORD);
            assert!(loan.parent.is_none());
            let projections = &plan.projections[origin.projections.clone()];
            assert_eq!(projections.len(), 1);
            assert_eq!(projections[0].result_type(), WORD);
            match projections[0].kind() {
                SemanticProjectionKindV1::Field(0) => {
                    reads += 1;
                    assert_eq!(
                        (loan.effects.referent_reads, loan.effects.referent_writes),
                        (1, 0)
                    );
                    assert_eq!(
                        plan.require_promoted(index, budget)?,
                        SourceReferenceRepresentationV29::StableReferent
                    );
                }
                SemanticProjectionKindV1::Field(1) => {
                    writes += 1;
                    assert_eq!(
                        (loan.effects.referent_reads, loan.effects.referent_writes),
                        (0, 1)
                    );
                    assert_eq!(
                        loan.representation,
                        SourceReferenceRepresentationV29::NeedsAddressable(
                            SourceReferenceCellNeedV29::ReferentWrite
                        )
                    );
                    assert!(
                        plan.require_promoted(index, budget).is_err(),
                        "writeback obligation is not cell authority"
                    );
                }
                other => panic!("unexpected source field: {other:?}"),
            }
            assert_eq!(
                (
                    loan.effects.payload_reads,
                    loan.effects.payload_writes,
                    loan.effects.address_observations
                ),
                (0, 0, 0)
            );
        }
        assert_eq!(
            (reads, writes),
            (2, 2),
            "both source calls have separate field loans"
        );
        assert_ne!(
            plan.origins[plan.loans[0].origin].instance,
            plan.origins[plan.loans[2].origin].instance
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_same_field_unique_borrows_are_not_misclassified_as_disjoint() {
    let result = run_owner(pair_field_owner(true), |_, _| {
        panic!("same-field alias must refuse")
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference has conflicting live loans",
            ..
        })
    ));
}

fn union_field_loans_owner_v29(first_has_future_use: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::UniqueRead, |types, functions| {
        let union = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([42; 32]),
            SemanticLayoutIdentityV1::from_sha256([42; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                8,
                8,
                SemanticFieldsShapeV1::union(2).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::union(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                )),
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![WORD, WORD]).unwrap()),
        ));
        functions[0] = function(
            10,
            true,
            union,
            vec![
                local(10, UNIT, SemanticLocalRoleV1::Return),
                local(11, union, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![
                block(
                    10,
                    vec![],
                    call(1, SemanticOperandV1::Copy(place(1, union)), 1),
                ),
                block(11, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"source_reference_union_component".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([10; 32]),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        ));
        functions[1] = function(
            20,
            false,
            union,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, union, SemanticLocalRoleV1::Argument(0)),
                local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(23, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(24, WORD, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(
                20,
                vec![
                    assign(
                        place(2, REFERENCE),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: projected(1, &[(SemanticProjectionKindV1::Field(0), WORD)]),
                        },
                    ),
                    assign(
                        place(3, REFERENCE),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: projected(1, &[(SemanticProjectionKindV1::Field(1), WORD)]),
                        },
                    ),
                    if first_has_future_use {
                        assign(
                            place(4, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                                2,
                                &[(SemanticProjectionKindV1::Dereference, WORD)],
                            ))),
                        )
                    } else {
                        statement(SemanticStatementKindV1::Nop)
                    },
                    dead(2),
                    dead(3),
                    unit(),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        );
    })
}

#[test]
fn source_reference_different_union_fields_still_overlap_the_same_storage() {
    let result = run_owner(union_field_loans_owner_v29(true), |_, _| {
        panic!("simultaneously live union field loans share physical storage")
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference has conflicting live loans",
            ..
        })
    ));
}

#[test]
fn source_reference_dead_union_field_loan_does_not_conflict_with_the_next_borrow() {
    run_owner(union_field_loans_owner_v29(false), |plan, _| {
        assert_eq!(plan.loans.len(), 2);
        assert_eq!(plan.loans[0].kind, SemanticBorrowKindV1::Mutable);
        assert_eq!(plan.loans[1].kind, SemanticBorrowKindV1::Mutable);
        Ok(())
    })
    .unwrap();
}
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const CAPTURE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Shared,
    UniqueRead,
    Writeback,
    DeadReferent,
    Conflict,
    Reborrow,
    SuspendedParent,
    DeadChild,
    Address,
}

impl Case {
    fn mutable(self) -> bool {
        self != Self::Shared && self != Self::Address
    }
}

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn projected(
    local: u32,
    projections: &[(SemanticProjectionKindV1, SemanticTypeIdV1)],
) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        projections
            .iter()
            .map(|&(kind, ty)| SemanticProjectionV1::new(kind, ty).unwrap())
            .collect(),
        projections.last().unwrap().1,
    )
    .unwrap()
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(source(), kind)
}
fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let ty = destination.ty();
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(ty, value),
    )))
}
fn unit() -> SemanticStatementV1 {
    assign(
        place(0, UNIT),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))),
    )
}
fn dead(local: u32) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::StorageDead(
        SemanticLocalIdV1::from_index(local),
    ))
}
fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([tag; 32]),
        ty,
        role,
        source(),
    )
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), kind),
    )
    .unwrap()
}
fn call(callee: u32, argument: SemanticOperandV1, target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            vec![argument],
            Some(SemanticCallDestinationV1::new(
                place(0, UNIT),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(target),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn abi(tag: u8, kernel: bool, input: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    let mode = if input == CAPTURE {
        SemanticAbiPassModeV1::cast(
            false,
            SemanticAbiCastV1::new(
                [None; 8],
                None,
                SemanticAbiUniformV1::new(
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                    8,
                )
                .unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        )
    } else {
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    false,
                    None,
                    false,
                    false,
                    false,
                    input == WORD,
                ),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        )
    };
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if kernel {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if kernel {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            input, mode,
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap()
}
fn function(
    tag: u8,
    kernel: bool,
    input: SemanticTypeIdV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if kernel {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        source(),
        abi(tag, kernel, input),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

// Structural source/SSA component fixtures, not rustc provenance or allocation
// authority. The unchanged managed 24 remain the real source acceptance gate.
fn owner(case: Case) -> ProductionSemanticSsaOwnerV1 {
    owner_with(case, |_, _| {})
}

fn owner_with(
    case: Case,
    mutate: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
) -> ProductionSemanticSsaOwnerV1 {
    try_owner_with(case, mutate).unwrap()
}

fn try_owner_with(
    case: Case,
    mutate: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    let unit_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
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
    );
    let word_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([2; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    let reference_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                WORD,
                SemanticPointerKindV1::Reference,
                if case.mutable() {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    if case.mutable() {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    8,
                    8,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let capture_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([4; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            8,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![REFERENCE]).unwrap()),
    );
    let root = function(
        10,
        true,
        WORD,
        vec![
            local(10, UNIT, SemanticLocalRoleV1::Return),
            local(11, WORD, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![
            block(
                10,
                vec![],
                call(1, SemanticOperandV1::Copy(place(1, WORD)), 1),
            ),
            block(
                11,
                vec![],
                call(1, SemanticOperandV1::Copy(place(1, WORD)), 2),
            ),
            block(12, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"source_reference_component".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([10; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let borrow_kind = if case.mutable() {
        SemanticBorrowKindV1::Mutable
    } else {
        SemanticBorrowKindV1::Shared
    };
    let mut statements = vec![
        assign(
            place(2, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: borrow_kind,
                place: place(1, WORD),
            },
        ),
        assign(
            place(3, CAPTURE),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![SemanticOperandV1::Move(place(2, REFERENCE))],
                )
                .unwrap(),
            ),
        ),
        dead(2),
    ];
    if case == Case::DeadReferent {
        statements.push(dead(1));
    }
    if case == Case::Conflict {
        statements.push(assign(
            place(4, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(1, WORD),
            },
        ));
    }
    let worker = function(
        20,
        false,
        WORD,
        vec![
            local(20, UNIT, SemanticLocalRoleV1::Return),
            local(21, WORD, SemanticLocalRoleV1::Argument(0)),
            local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
            local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
            local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                20,
                statements,
                call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 1),
            ),
            block(21, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let mut statements = vec![assign(
        place(2, REFERENCE),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
            1,
            &[(SemanticProjectionKindV1::Field(0), REFERENCE)],
        ))),
    )];
    if matches!(
        case,
        Case::Reborrow | Case::SuspendedParent | Case::DeadChild
    ) {
        statements.push(assign(
            place(4, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)]),
            },
        ));
    }
    let read = if case == Case::Reborrow { 4 } else { 2 };
    statements.push(assign(
        place(3, WORD),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
            read,
            &[(SemanticProjectionKindV1::Dereference, WORD)],
        ))),
    ));
    if case == Case::SuspendedParent {
        statements.push(assign(
            place(3, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                4,
                &[(SemanticProjectionKindV1::Dereference, WORD)],
            ))),
        ));
    }
    if case == Case::Writeback {
        statements.push(assign(
            projected(
                1,
                &[
                    (SemanticProjectionKindV1::Field(0), REFERENCE),
                    (SemanticProjectionKindV1::Dereference, WORD),
                ],
            ),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                WORD,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(17, 8).unwrap()),
            ))),
        ));
    }
    if case == Case::Reborrow {
        statements.push(dead(4));
        statements.push(assign(
            place(3, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                2,
                &[(SemanticProjectionKindV1::Dereference, WORD)],
            ))),
        ));
    }
    statements.push(unit());
    let closure = function(
        30,
        false,
        CAPTURE,
        vec![
            local(30, UNIT, SemanticLocalRoleV1::Return),
            local(31, CAPTURE, SemanticLocalRoleV1::Argument(0)),
            local(32, REFERENCE, SemanticLocalRoleV1::Temporary),
            local(33, WORD, SemanticLocalRoleV1::Temporary),
            local(34, REFERENCE, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(30, statements, SemanticTerminatorKindV1::Return)],
    );
    let mut types = vec![unit_type, word_type, reference_type, capture_type];
    let mut functions = vec![root, worker, closure];
    if case == Case::Address {
        fixture_closure::add_address_observation(&mut types, &mut functions);
    }
    mutate(&mut types, &mut functions);
    fixture_closure::retain_fixture_closure(&types, &mut functions);
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(
                u32::try_from(index).unwrap(),
            ))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let mut owner =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())?;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    Ok(owner)
}

fn run(
    case: Case,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    run_owner(owner(case), consume)
}

fn run_owner(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = with_source_reference_plan_v29(instances, budget, consume);
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap()
}

#[test]
fn source_reference_moved_capture_keeps_live_referent_and_distinct_instances() {
    for case in [Case::Shared, Case::UniqueRead] {
        run(case, |plan, budget| {
            assert_eq!(plan.loans.len(), 2);
            assert_ne!(plan.loans[0].site.instance, plan.loans[1].site.instance);
            for loan in &plan.loans {
                let origin = &plan.origins[loan.origin];
                let instance = plan.instances.instance(origin.instance).unwrap();
                assert_eq!(instance.declaration().locals()[origin.local.index() as usize].role(), SemanticLocalRoleV1::Argument(0));
                assert_eq!(origin.local.index(), 1);
                assert_eq!(origin.generation, 0);
                assert_eq!(origin.ty, WORD);
                assert_eq!(loan.source_type, REFERENCE);
                assert_eq!(loan.representation, SourceReferenceRepresentationV29::StableReferent);
                assert!(loan.effects.referent_reads > 0);
                let statement = &instance.declaration().blocks()[loan.site.block.index() as usize].statements()[loan.site.statement.unwrap()];
                assert!(matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                    if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow { place, .. }
                        if place.local() == origin.local && place.ty() == WORD)));
                assert!(std::ptr::eq(plan.loan_at(loan.site, budget)?.unwrap(), loan));
            }
            Ok(())
        }).unwrap();
    }
}

#[test]
fn source_reference_reborrow_suspends_then_resumes_parent_without_new_origin() {
    run(Case::Reborrow, |plan, _| {
        assert_eq!(plan.loans.len(), 4);
        for child in plan.loans.iter().filter(|loan| loan.parent.is_some()) {
            let parent = &plan.loans[child.parent.unwrap()];
            let child_origin = &plan.origins[child.origin];
            let parent_origin = &plan.origins[parent.origin];
            assert_eq!(child_origin.instance, parent_origin.instance);
            assert_eq!(child_origin.local, parent_origin.local);
            assert_eq!(child_origin.generation, parent_origin.generation);
            assert_eq!(
                child.representation,
                SourceReferenceRepresentationV29::StableReferent
            );
        }
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        run(Case::SuspendedParent, |_, _| panic!("must refuse")),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference parent is suspended by a live reborrow",
            ..
        })
    ));
}

#[test]
fn source_reference_dead_child_no_longer_suspends_parent() {
    run(Case::DeadChild, |plan, _| {
        assert_eq!(plan.storage, SourceReferenceStorageV29::PromotedOnly);
        assert!(plan.accesses.is_empty());
        let children: Vec<_> = plan
            .loans
            .iter()
            .filter(|loan| loan.parent.is_some())
            .collect();
        assert_eq!(children.len(), 2);
        assert_ne!(children[0].site.instance, children[1].site.instance);
        for child in children {
            let parent = &plan.loans[child.parent.unwrap()];
            let child_origin = &plan.origins[child.origin];
            let origin = &plan.origins[parent.origin];
            assert_eq!(
                (
                    child_origin.instance,
                    child_origin.local,
                    child_origin.generation
                ),
                (origin.instance, origin.local, origin.generation),
            );
            assert_eq!(child.effects.referent_reads, 0);
            assert_eq!(parent.effects.referent_reads, 1);
            let instance = plan.instances.instance(child.site.instance).unwrap();
            assert_eq!(instance.function().index(), 2);
            let statement = &instance.declaration().blocks()[0].statements()[2];
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                panic!("original parent read assignment");
            };
            assert_eq!(
                assignment.value().kind(),
                &SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                    2,
                    &[(SemanticProjectionKindV1::Dereference, WORD),]
                )),)
            );
            assert_eq!(origin.generation, 0);
            assert_eq!(origin.ty, WORD);
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_dead_referent_and_conflicting_unique_loan_refuse() {
    for (case, expected) in [
        (
            Case::DeadReferent,
            "source reference referent storage dies with a live loan",
        ),
        (
            Case::Conflict,
            "source reference has conflicting live loans",
        ),
    ] {
        let result = run(case, |_, _| panic!("must refuse"));
        assert!(
            matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }) if detail == expected),
            "{case:?}: {result:?}"
        );
    }
}

#[test]
fn source_reference_mutation_and_address_identity_require_cells_not_copies() {
    for (case, need) in [
        (Case::Writeback, SourceReferenceCellNeedV29::ReferentWrite),
        (
            Case::Address,
            SourceReferenceCellNeedV29::AddressObservation,
        ),
    ] {
        run_owner_with_storage(owner(case), |plan, budget| {
            assert_eq!(plan.loans.len(), 2);
            for (index, loan) in plan.loans.iter().enumerate() {
                assert_eq!(
                    loan.representation,
                    SourceReferenceRepresentationV29::NeedsAddressable(need)
                );
                assert!(plan.require_promoted(index, budget).is_err());
            }
            Ok(())
        })
        .unwrap();
    }
}

pub(super) fn run_owner_with_storage(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    run_owner_with_storage_limits(owner, usize::MAX, usize::MAX, consume)
}

pub(super) fn run_owner_with_storage_limits(
    owner: ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = with_source_reference_storage_scope_v29(
                instances,
                SourceReferenceStorageV29::PromotedOnly,
                Some(&mut layouts),
                budget,
                |plan, root, budget| {
                    assert!(root.is_some());
                    consume(plan, budget).map_err(Into::into)
                },
            );
            assert!(budget.storage() >= floor);
            let extra = budget.storage() - floor;
            if layouts.permits_root_emission_refund(&owner, extra, budget) {
                budget.release_storage(extra).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap();
    let settled = layouts.permits_root_emission_refund(&owner, 0, &budget);
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = if settled {
        demands.discard(&mut budget)
    } else {
        drop(demands);
        Err(ArgumentResourceV1::Accounting.into())
    };
    result.and(cleanup).and(demand_cleanup)
}

#[test]
fn source_reference_unknown_effect_is_not_a_pointer_admission_fallback() {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            budget
                .reserve_storage(source_reference_headers_v29::<()>().unwrap())
                .unwrap();
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            let call = instances.calls(instances.root()).unwrap()[0].source();
            let result =
                builder.intrinsic(&SemanticCallableDeclV1::defined(ROOT), &[], call, budget);
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference call has unknown external effects",
                    ..
                })
            ));
            drop(builder);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[test]
fn source_reference_owner_mismatch_and_budget_denial_restore_floor() {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let mismatch = with_source_reference_plan_v29(instances, budget, |plan, _| {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut other = ArgumentBudgetV1::new(&mut work, usize::MAX);
                assert!(plan.check_owner(instances, &mut other).is_err());
                Ok(())
            });
            assert!(
                mismatch.is_err(),
                "an ignored custody failure poisons callback success"
            );
            for (work_limit, storage_limit) in [(0, usize::MAX), (usize::MAX, 0), (usize::MAX, 1)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut restricted = ArgumentBudgetV1::new(&mut work, storage_limit);
                let result: Result<(), _> =
                    with_source_reference_plan_v29(instances, &mut restricted, |_, _| {
                        panic!("must deny")
                    });
                assert!(result.is_err());
                assert_eq!(restricted.storage(), 0);
            }
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[path = "production_source_reference_cfg_v29_tests.rs"]
mod cfg_tests;
