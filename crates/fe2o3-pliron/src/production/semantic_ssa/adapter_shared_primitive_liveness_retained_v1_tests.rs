//! Inert constructor controls only: model DATA is never admitted ownership.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 19;

fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(tag)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn cfg_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn projection_function_with_locals(
    blocks: Vec<SemanticBasicBlockV1>,
    locals: Vec<SemanticLocalDeclV1>,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(10)),
        SemanticLayoutIdentityV1::from_sha256(bytes(10)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(11)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(12)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(13)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(14)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(15)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SCALAR_TYPE,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

fn bytes(value: u8) -> [u8; 32] {
    [value; 32]
}

fn place(index: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], SCALAR_TYPE).unwrap()
}

fn assign(index: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(index),
        SemanticRvalueV1::new(SCALAR_TYPE, value),
    )))
}

fn fixture(cycle: bool) -> SemanticFunctionDeclV1 {
    let second = if cycle {
        SemanticTerminatorKindV1::FalseEdge {
            real_target: cfg_edge(SemanticEdgeRoleV1::FalseEdgeReal, 1),
            imaginary_target: cfg_edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 2),
        }
    } else {
        SemanticTerminatorKindV1::Assert {
            condition: SemanticOperandV1::Copy(place(3)),
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: constant(4),
                index: SemanticOperandV1::Copy(place(3)),
            },
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 2),
            unwind: SemanticUnwindActionV1::Unreachable,
        }
    };
    projection_function_with_locals(
        vec![
            block(
                80,
                vec![
                    statement(SemanticStatementKindV1::StorageLive(
                        SemanticLocalIdV1::from_index(1),
                    )),
                    assign(2, SemanticRvalueKindV1::Use(constant(7))),
                    statement(SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(
                        place(2),
                    ))),
                ],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(
                81,
                vec![statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(1),
                ))],
                second,
            ),
            block(82, vec![], SemanticTerminatorKindV1::Return),
        ],
        (0..4)
            .map(|index| {
                local(
                    100 + index,
                    SCALAR_TYPE,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                )
            })
            .collect(),
    )
}
struct OriginalMeter<'a, 'w>(&'a mut Budget<'w>);
impl BorrowWork for OriginalMeter<'_, '_> {
    fn work(&mut self, units: usize) -> Result<()> {
        self.0
            .charge_work(units)
            .map_err(|_| Error::ResourceOverflow)
    }
}
fn rows(schedule: &Schedule) -> Vec<(usize, usize, bool)> {
    schedule
        .locals
        .iter()
        .map(|row| (row.block, row.statement, row.pinned))
        .collect()
}
fn payload(owner: &RetainedSharedLivenessV1<'_>) -> usize {
    owner.schedule.locals.capacity() * size_of::<Last>()
        + (owner.schedule.indegrees.capacity() + owner.ready.capacity()) * size_of::<usize>()
}
fn counts(function: &SemanticFunctionDeclV1) -> (usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut retained = RetainedSharedLivenessV1::new();
    retained
        .prepare_into(function, &mut budget, &mut owned)
        .unwrap();
    let result = (budget.work(), budget.storage());
    assert_eq!(owned, frame().unwrap() + payload(&retained));
    drop(retained);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}
fn complete_oracle(function: &SemanticFunctionDeclV1) {
    let mut old_work = Work::new(LIMIT);
    let mut old_budget = Budget::new(&mut old_work, LIMIT);
    let original = Schedule::new(function, &mut OriginalMeter(&mut old_budget)).unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut retained = RetainedSharedLivenessV1::new();
    retained
        .prepare_into(function, &mut budget, &mut owned)
        .unwrap();
    let current = retained.completed_for(function, &budget, &owned).unwrap();
    assert_eq!(rows(current), rows(&original));
    assert_eq!(current.indegrees, original.indegrees);
    // The original Kahn loop has exhausted the logical queue on return.
    // This component deliberately retains its otherwise-dropped allocation.
    assert!(retained.ready.is_empty());
    assert!(retained.ready.capacity() >= function.blocks().len());
    assert_eq!(budget.work(), old_budget.work() + 32);
    assert_eq!(owned, frame().unwrap() + payload(&retained));
    assert_eq!(budget.storage(), FLOOR + owned);
    assert_eq!(budget.peak_storage(), budget.storage());
    assert!(retained.resource_failure().is_none());
    assert!(retained.entry.unwrap().ledger == budget.work_ledger_identity_v1());
    assert!(retained.held.unwrap() == snapshot(&budget, &owned));
    drop(retained);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn retained_liveness_dag_matches_full_original_data_and_work() {
    let function = fixture(false);
    complete_oracle(&function);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let original = Schedule::new(&function, &mut OriginalMeter(&mut budget)).unwrap();
    assert_eq!(
        rows(&original),
        vec![
            (usize::MAX, usize::MAX, true),
            (1, 0, true),
            (0, 2, false),
            (1, usize::MAX, true),
        ]
    );
    assert_eq!(original.indegrees, vec![0, 0, 0]);
}
#[test]
fn retained_liveness_cycles_and_tails_match_original() {
    let function = fixture(true);
    complete_oracle(&function);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let original = Schedule::new(&function, &mut OriginalMeter(&mut budget)).unwrap();
    assert_eq!(original.indegrees, vec![0, 1, 1]);
}
#[test]
fn retained_liveness_empty_and_unreferenced_owners_are_real_data() {
    let empty = projection_function_with_locals(vec![], vec![]);
    complete_oracle(&empty);
    let unused = projection_function_with_locals(
        vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        vec![
            local(1, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(2, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    );
    complete_oracle(&unused);
}
#[test]
fn retained_liveness_all_work_cuts_preserve_original_prefix_and_attached_state() {
    let function = fixture(false);
    let (needed, _) = counts(&function);
    let mut saw_populated = false;
    for limit in 0..needed {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut retained = RetainedSharedLivenessV1::new();
        let error = retained
            .prepare_into(&function, &mut budget, &mut owned)
            .err()
            .unwrap();
        let RetainedLivenessErrorV1::Resource(Resource::Work(first)) = error else {
            panic!("wrong work refusal");
        };
        assert_eq!(retained.resource_failure(), Some(Resource::Work(first)));
        assert_eq!(budget.failed_work(), Some(first.actual()));
        assert_eq!(budget.storage(), FLOOR + owned);
        assert_eq!(retained.phase, Phase::Terminal);
        assert!(std::ptr::eq(retained.source.unwrap(), &function));
        if limit >= 32 {
            let mut old_work = Work::new(limit - 32);
            let mut old_budget = Budget::new(&mut old_work, LIMIT);
            assert!(Schedule::new(&function, &mut OriginalMeter(&mut old_budget)).is_err());
            assert_eq!(budget.work(), old_budget.work() + 32);
            assert_eq!(first.actual(), old_budget.failed_work().unwrap() + 32);
        } else {
            assert_eq!(budget.work(), 0);
            assert_eq!(owned, 0);
        }
        saw_populated |= retained.schedule.locals.len() == function.locals().len()
            && retained.schedule.indegrees.len() == function.blocks().len()
            && retained.ready.capacity() >= function.blocks().len();
        let before = (
            rows(&retained.schedule),
            retained.schedule.indegrees.clone(),
            retained.ready.clone(),
            payload(&retained),
            budget.work(),
            budget.storage(),
        );
        assert!(retained.completed_for(&function, &budget, &owned).is_err());
        assert_eq!(
            before,
            (
                rows(&retained.schedule),
                retained.schedule.indegrees.clone(),
                retained.ready.clone(),
                payload(&retained),
                budget.work(),
                budget.storage()
            )
        );
        drop(retained);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_work(), Some(first.actual()));
    }
    assert!(saw_populated);
}
#[test]
fn retained_liveness_all_storage_cuts_keep_partial_vector_owners() {
    let function = fixture(false);
    let (_, needed) = counts(&function);
    let mut stages = [false; 3];
    for limit in FLOOR..needed {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut retained = RetainedSharedLivenessV1::new();
        let error = retained
            .prepare_into(&function, &mut budget, &mut owned)
            .err()
            .unwrap();
        let RetainedLivenessErrorV1::Resource(Resource::Storage(first)) = error else {
            panic!("wrong storage refusal");
        };
        assert_eq!(retained.resource_failure(), Some(Resource::Storage(first)));
        assert_eq!(budget.failed_storage(), Some(first.actual()));
        assert_eq!(budget.storage(), FLOOR + owned);
        if retained.schedule.locals.capacity() == 0 {
            stages[0] = true;
        }
        if retained.schedule.locals.capacity() > 0 && retained.schedule.indegrees.capacity() == 0 {
            stages[1] = true;
        }
        if retained.schedule.indegrees.capacity() > 0 && retained.ready.capacity() == 0 {
            stages[2] = true;
        }
        assert!(retained.completed_for(&function, &budget, &owned).is_err());
        let peak = budget.peak_storage();
        drop(retained);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), peak);
        assert_eq!(budget.failed_storage(), Some(first.actual()));
    }
    assert!(stages.into_iter().all(|stage| stage));
}
#[test]
fn retained_liveness_malformed_local_and_edge_match_original_refusal() {
    for edge in [false, true] {
        let function = projection_function_with_locals(
            vec![block(
                1,
                if edge {
                    vec![]
                } else {
                    vec![statement(SemanticStatementKindV1::StorageLive(
                        SemanticLocalIdV1::from_index(999),
                    ))]
                },
                if edge {
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 999))
                } else {
                    SemanticTerminatorKindV1::Return
                },
            )],
            vec![local(1, SCALAR_TYPE, SemanticLocalRoleV1::Return)],
        );
        let mut old_work = Work::new(LIMIT);
        let mut old_budget = Budget::new(&mut old_work, LIMIT);
        let old = Schedule::new(&function, &mut OriginalMeter(&mut old_budget))
            .err()
            .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut retained = RetainedSharedLivenessV1::new();
        let error = retained
            .prepare_into(&function, &mut budget, &mut owned)
            .err()
            .unwrap();
        let RetainedLivenessErrorV1::Original(error) = error else {
            panic!("resource substitution");
        };
        assert_eq!(format!("{error:?}"), format!("{old:?}"));
        assert_eq!(budget.work(), old_budget.work() + 32);
        assert!(retained.resource_failure().is_none());
        assert_eq!(retained.schedule.locals.len(), 1);
        assert_eq!(retained.schedule.indegrees.len(), 1);
        assert!(retained.ready.capacity() >= 1);
        assert!(retained.completed_for(&function, &budget, &owned).is_err());
        drop(retained);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn retained_liveness_source_counter_and_one_shot_identity_are_exact() {
    let function = fixture(false);
    let other = fixture(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut retained = RetainedSharedLivenessV1::new();
    assert!(retained.completed_for(&function, &budget, &owned).is_err());
    retained
        .prepare_into(&function, &mut budget, &mut owned)
        .unwrap();
    assert!(retained.completed_for(&other, &budget, &owned).is_err());
    let counterfeit_counter = owned;
    assert!(
        retained
            .completed_for(&function, &budget, &counterfeit_counter)
            .is_err()
    );
    assert!(retained.completed_for(&function, &budget, &owned).is_ok());
    let before = (budget.work(), budget.storage(), owned, payload(&retained));
    assert!(
        retained
            .prepare_into(&function, &mut budget, &mut owned)
            .is_err()
    );
    assert_eq!(
        before,
        (budget.work(), budget.storage(), owned, payload(&retained))
    );
    assert!(retained.completed_for(&function, &budget, &owned).is_err());
    drop(retained);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_liveness_foreign_ledger_in_same_budget_slot_is_rejected() {
    let function = fixture(false);
    let mut work = Work::new(LIMIT);
    let mut other_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    let mut owned = 0;
    let mut retained = RetainedSharedLivenessV1::new();
    retained
        .prepare_into(&function, &mut budget, &mut owned)
        .unwrap();
    other.charge_work(budget.work()).unwrap();
    other.reserve_storage(budget.storage()).unwrap();
    let original_ledger = budget.work_ledger_identity_v1();
    std::mem::swap(&mut budget, &mut other);
    assert!(retained.completed_for(&function, &budget, &owned).is_err());
    assert!(budget.work_ledger_identity_v1() != original_ledger);
    std::mem::swap(&mut budget, &mut other);
    assert!(retained.completed_for(&function, &budget, &owned).is_ok());
    drop(retained);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_liveness_held_floor_rejects_coupled_rollback_but_accepts_coupled_growth() {
    let function = fixture(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut retained = RetainedSharedLivenessV1::new();
    retained
        .prepare_into(&function, &mut budget, &mut owned)
        .unwrap();
    budget.release_storage(1).unwrap();
    owned -= 1;
    assert!(retained.completed_for(&function, &budget, &owned).is_err());
    budget.reserve_storage(1).unwrap();
    owned += 1;
    assert!(retained.completed_for(&function, &budget, &owned).is_ok());
    budget.reserve_storage(7).unwrap();
    owned += 7;
    budget.charge_work(3).unwrap();
    assert!(retained.completed_for(&function, &budget, &owned).is_ok());
    drop(retained);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_liveness_entry_and_postflight_sticky_denials_are_not_cleared() {
    let function = fixture(false);
    for entry in [false, true] {
        for storage in [false, true] {
            let needed = counts(&function).0;
            let mut work = Work::new(if storage { LIMIT } else { needed });
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let mut retained = RetainedSharedLivenessV1::new();
            if !entry {
                retained
                    .prepare_into(&function, &mut budget, &mut owned)
                    .unwrap();
            }
            if storage {
                assert!(budget.reserve_storage(LIMIT + 1).is_err());
            } else {
                assert!(budget.charge_work(LIMIT + 1).is_err());
            }
            let denied = (budget.failed_work(), budget.failed_storage());
            if entry {
                assert!(
                    retained
                        .prepare_into(&function, &mut budget, &mut owned)
                        .is_err()
                );
            }
            assert!(retained.completed_for(&function, &budget, &owned).is_err());
            assert_eq!((budget.failed_work(), budget.failed_storage()), denied);
            drop(retained);
            budget.release_storage(owned).unwrap();
            assert_eq!((budget.failed_work(), budget.failed_storage()), denied);
        }
    }
}
#[test]
fn retained_liveness_caller_error_and_unwind_keep_owners_until_drop_before_refund() {
    let function = fixture(false);
    for unwind in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut retained = RetainedSharedLivenessV1::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            retained
                .prepare_into(&function, &mut budget, &mut owned)
                .unwrap();
            if unwind {
                panic!("inert caller control");
            }
            Err::<(), &'static str>("inert caller error")
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err("inert caller error"))));
        }
        assert!(retained.completed_for(&function, &budget, &owned).is_ok());
        assert_eq!(budget.storage(), FLOOR + owned);
        assert!(retained.ready.capacity() >= function.blocks().len());
        let peak = budget.peak_storage();
        drop(retained);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), peak);
    }
}
#[test]
fn retained_liveness_arithmetic_and_first_failure_carriers_are_preserved() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut failure = None;
    let mut values = Vec::<Last>::new();
    {
        let mut meter = RetainedMeter {
            budget: &mut budget,
            owned: &mut owned,
            failure: &mut failure,
        };
        assert!(meter.capacity(&mut values, usize::MAX).is_err());
        assert_eq!(meter.accept_work(1), Err(Resource::Arithmetic));
    }
    assert_eq!(failure, Some(Resource::Arithmetic));
    assert_eq!(owned, 0);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.failed_work(), Some(1));
    assert!(values.is_empty());
}
#[test]
fn retained_liveness_original_completed_consumer_observes_identical_data_and_work() {
    fn consume(
        schedule: &Schedule,
        function: &SemanticFunctionDeclV1,
    ) -> (usize, usize, usize, usize, bool, usize) {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut meter = OriginalMeter(&mut budget);
        let mut observer = NoReads;
        let site = SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement: 2,
        };
        let mut analysis = Analysis {
            function,
            types: &[],
            meter: &mut meter,
            observer: &mut observer,
            site: Some(site),
            candidates: vec![Candidate {
                site,
                source: 2,
                pointee: SCALAR_TYPE,
                reference: SCALAR_TYPE,
                live: 1,
                valid: true,
                read: false,
            }],
            holders: [(
                2,
                vec![Alias {
                    candidate: 0,
                    fields: vec![],
                    live: true,
                }],
            )]
            .into_iter()
            .collect(),
            active: [(2, [0usize].into_iter().collect())].into_iter().collect(),
            alias_words: 1,
            cap: 100,
            tree_work: 2,
            exhausted: false,
        };
        schedule
            .completed(
                &mut analysis,
                site,
                function.blocks()[0].statements()[2].kind(),
            )
            .unwrap();
        let result = (
            analysis.holders.len(),
            analysis.active.len(),
            analysis.alias_words,
            analysis.candidates[0].live,
            analysis.candidates[0].valid,
        );
        drop(analysis);
        (
            result.0,
            result.1,
            result.2,
            result.3,
            result.4,
            budget.work(),
        )
    }
    for cycle in [false, true] {
        let function = fixture(cycle);
        let mut old_work = Work::new(LIMIT);
        let mut old_budget = Budget::new(&mut old_work, LIMIT);
        let original = Schedule::new(&function, &mut OriginalMeter(&mut old_budget)).unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut retained = RetainedSharedLivenessV1::new();
        retained
            .prepare_into(&function, &mut budget, &mut owned)
            .unwrap();
        let current = retained.completed_for(&function, &budget, &owned).unwrap();
        assert_eq!(consume(current, &function), consume(&original, &function));
        assert_eq!(consume(current, &function).0, 0);
        drop(retained);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_liveness_all_assert_message_operands_reuse_original_walker() {
    for message in [
        SemanticAssertMessageV1::BoundsCheck {
            length: constant(8),
            index: SemanticOperandV1::Copy(place(1)),
        },
        SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: SemanticOperandV1::Copy(place(1)),
            right: constant(1),
        },
        SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Copy(place(1))),
        SemanticAssertMessageV1::RemainderByZero(SemanticOperandV1::Copy(place(1))),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment: constant(4),
            found_alignment: SemanticOperandV1::Copy(place(1)),
        },
        SemanticAssertMessageV1::NullPointerDereference,
        SemanticAssertMessageV1::ResumedAfterReturn,
        SemanticAssertMessageV1::ResumedAfterPanic,
    ] {
        let function = projection_function_with_locals(
            vec![
                block(
                    1,
                    vec![],
                    SemanticTerminatorKindV1::Assert {
                        condition: constant(1),
                        expected: true,
                        message,
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(2, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(1, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(2, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        complete_oracle(&function);
    }
}
#[test]
fn retained_liveness_source_order_and_no_ordinary_activation() {
    let text = include_str!("adapter_shared_primitive_liveness_retained_v1.rs");
    let start = text.find("fn build_into(").unwrap();
    let end = text[start..].find("fn frame()").unwrap() + start;
    let body = &text[start..end];
    let markers = [
        "meter.work(",
        "meter.capacity(&mut schedule.locals",
        "schedule.locals.push(",
        "meter.capacity(&mut schedule.indegrees",
        "schedule.indegrees.resize(",
        "meter.capacity(ready",
        "for (block, body)",
        "for (block, degree)",
        "while let Some(block)",
        "Ok(())",
    ];
    let mut position = 0;
    for marker in markers {
        position += body[position..].find(marker).unwrap() + marker.len();
    }
    assert!(!text.contains("release_storage("));
    assert!(!text.contains("Budget::new("));
    let parent = include_str!("adapter_shared_primitive_liveness_v1.rs");
    assert!(parent.contains("pub(super) fn new("));
    assert!(parent.contains("pub(super) fn completed"));
    assert!(parent.contains("let mut ready = Vec::with_capacity(function.blocks().len());"));
}
