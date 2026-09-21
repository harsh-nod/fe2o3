use super::*;
use crate::production_semantic_kir_v1::checked_output_admission_policy3_v1::expanded_source_tests;
use crate::{
    ProductionExpandedHistoryV1 as History, ProductionExpandedPrefixV1 as PrefixView,
    ProductionOwnedExpandedContinuationV1 as Expanded,
};

const EXPANDED_WORK: usize = 4_000_000_000;
const EXPANDED_STORAGE: usize = 256 * 1024 * 1024;

// This is an inert transcript of actual fixture rows, not a backend execution
// witness. Its wire shape is the existing kernel-opt history fixture shape.
fn inert_actual_policy7_record(
    execution: &[u8],
    input: &Graph,
    output: &Graph,
    rows: &[fe2o3_kernel_analysis::CanonicalKirRedundantStoreRowV1],
    retained: &[fe2o3_kernel_analysis::CanonicalKirRedundantStoreRetainedOperationV1],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Vec<u8> {
    let length = 384 + (rows.len() + retained.len()) * 24;
    budget
        .reserve_storage(std::mem::size_of::<Vec<u8>>() + length)
        .unwrap();
    let mut record = Vec::new();
    record.try_reserve_exact(length).unwrap();
    budget.reserve_storage(record.capacity() - length).unwrap();
    budget.charge_work(length).unwrap();
    record.resize(384, 0);
    record[..16].copy_from_slice(b"F2P7EX1\0\x01\0\x07\0\0\x01\0\0");
    record[16..272].copy_from_slice(execution);
    for (offset, graph) in [(272, input), (312, output)] {
        let id = graph.canonical().identity();
        record[offset..offset + 32].copy_from_slice(id.digest());
        record[offset + 32..offset + 40].copy_from_slice(&id.canonical_length().to_le_bytes());
    }
    let mut coordinate = |s: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1| {
        for n in [s.block.function.0, s.block.block, s.operation] {
            record.extend(n.to_le_bytes());
        }
    };
    for row in rows {
        coordinate(row.anchor);
        coordinate(row.removed);
    }
    for row in retained {
        coordinate(row.input);
        coordinate(row.output);
    }
    for (offset, n) in [
        (352, rows.len()),
        (360, retained.len()),
        (368, 1),
        (376, length),
    ] {
        record[offset..offset + 8].copy_from_slice(&(n as u64).to_le_bytes());
    }
    assert_eq!(record.len(), length);
    record
}

fn with_decoded_expanded_history(
    owner: &Expanded,
    budget: &mut AssertOriginBudgetV1<'_>,
    run: impl FnOnce(&fe2o3_kernel_opt::CheckedLoopUnrollHistoryV1<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    use fe2o3_kernel_ir::{
        InertCanonicalKirTransitionGraphIdentityV1 as Identity,
        InertCanonicalKirTransitionReceiptV1 as Transition,
    };
    use fe2o3_kernel_opt::*;
    let floor = budget.storage();
    macro_rules! decode {
        ($u:expr) => {{
            let u = $u;
            let f = u.prefix();
            let r = f.prefix();
            let l = r.prefix();
            let h = l.prefix();
            let p = h.prefix();
            let k = p.prefix();
            let j = k.prefix();
            let i = j.prefix();
            let checked = i.checked_output();
            let p5 = checked.intermediate_policy5();
            let p4 = p5.intermediate_policy4();
            let p4_bytes =
                encode_checked_canonical_policy4_execution_receipt_v1(i.bound(), p4, budget)
                    .unwrap();
            budget
                .reserve_storage(p4_bytes.storage().retained_storage())
                .unwrap();
            let (transition, receipt) = Transition::from_candidate_with_budget(
                p5.owner().canonical().identity(),
                checked.owner().canonical().identity(),
                checked.continuation().occurrences().candidate(),
                budget,
            )
            .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let record = inert_actual_policy7_record(
                checked.execution().canonical_bytes(),
                checked.owner(),
                j.output(),
                j.continuation().rows(),
                j.continuation().retained_operations(),
                budget,
            );
            budget
                .reserve_storage(std::mem::size_of::<
                    CanonicalRefinedForwardingHistoryInputsV1<'_>,
                >())
                .unwrap();
            let inputs = CanonicalRefinedForwardingHistoryInputsV1 {
                prefix: CanonicalPolicy8SemanticInputsV1 {
                    prefix: CanonicalPolicy7SemanticInputsV1 {
                        prefix: CanonicalPolicy6SemanticInputsV1 {
                            prefix: CanonicalPolicy5SemanticInputsV1 {
                                input: i.bound(),
                                intermediate: p4.intermediate_policy3().owner(),
                                stored: p4.owner(),
                                output: p5.owner(),
                                policy4_wire: p4_bytes.canonical_bytes(),
                                policy5_record: p5.execution().canonical_bytes(),
                                load_rows: p5.load_forwarding_rows(),
                            },
                            output: checked.owner(),
                            continuation: CanonicalPolicy6ContinuationClaimsV1 {
                                composition_record: checked.execution().canonical_bytes(),
                                integer_record: checked
                                    .continuation()
                                    .execution()
                                    .canonical_bytes(),
                                transition_wire: transition.canonical_bytes(),
                            },
                        },
                        output: j.output(),
                        continuation: CanonicalPolicy7ContinuationClaimsV1 {
                            execution_record: &record,
                            deletion_rows: j.continuation().rows(),
                            retained_operations: j.continuation().retained_operations(),
                        },
                    },
                    output: k.output(),
                    continuation: CanonicalPolicy8ContinuationClaimsV1 {
                        pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
                        input: Identity::from_verified(j.output().canonical().identity()),
                        output: Identity::from_verified(k.output().canonical().identity()),
                        occurrences: k.continuation().occurrences().candidate(),
                    },
                },
                promoted: p.output(),
                selected_allocations: p.continuation().selected_allocations(),
                promotion_origins: p.continuation().origins(),
                preheaders: h.output(),
                preheader_rows: h.continuation().preheaders(),
                licm: l.output(),
                licm_origins: l.continuation().origins(),
                refined: r.output(),
                refinement_origins: r.continuation().origins(),
                output: f.output(),
                forwarding_origins: f.continuation().origins(),
                limits: CanonicalRefinedForwardingHistoryLimitsV1 {
                    refinement: r.limits(),
                    forwarding: f.limits(),
                },
            };
            let prefix_bytes = encode_refined_forwarding_history_v1(inputs, budget).unwrap();
            budget
                .reserve_storage(prefix_bytes.storage().retained_storage())
                .unwrap();
            let prefix =
                read_refined_forwarding_history_v1(prefix_bytes.canonical_bytes(), budget).unwrap();
            budget
                .reserve_storage(prefix.storage().retained_storage())
                .unwrap();
            let wire = encode_loop_unroll_history_v1(
                LoopUnrollHistoryInputsV1 {
                    prefix: &prefix,
                    output: u.output(),
                    origins: u.continuation().origins(),
                    limits: u.limits(),
                },
                budget,
            )
            .unwrap();
            budget
                .reserve_storage(wire.storage().retained_storage())
                .unwrap();
            let frame = read_loop_unroll_history_v1(wire.canonical_bytes(), budget).unwrap();
            budget
                .reserve_storage(frame.storage().retained_storage())
                .unwrap();
            let decoded = materialize_loop_unroll_history_v1(&frame, budget).unwrap();
            budget
                .reserve_storage(decoded.storage().retained_storage())
                .unwrap();
            let history = decoded.check_semantics(budget).unwrap();
            budget
                .reserve_storage(history.storage().retained_storage())
                .unwrap();
            assert!(!history.authenticates_execution());
            assert!(!history.grants_authority());
            let p3 = history
                .prefix()
                .prefix()
                .policy7_relation()
                .policy6_relation()
                .policy5_relation()
                .policy4_relation()
                .policy3_relation()
                .semantic_receipt();
            assert!(!std::ptr::eq(p3.input(), i.bound()));
            assert!(!std::ptr::eq(
                p3.output(),
                p4.intermediate_policy3().owner()
            ));
            assert_eq!(
                p3.input().canonical().canonical_bytes(),
                i.bound().canonical().canonical_bytes()
            );
            assert_eq!(
                p3.output().canonical().canonical_bytes(),
                p4.intermediate_policy3()
                    .owner()
                    .canonical()
                    .canonical_bytes()
            );
            assert!(!std::ptr::eq(history.output(), u.output()));
            assert_eq!(
                history.output().canonical().canonical_bytes(),
                u.output().canonical().canonical_bytes()
            );
            run(&history, budget);
            drop(history);
            drop(decoded);
            drop(frame);
            drop(wire);
            drop(prefix);
            drop(prefix_bytes);
            drop(record);
            drop(transition);
            drop(p4_bytes);
        }};
    }
    match owner.prefix() {
        PrefixView::Direct(v) => decode!(v),
        PrefixView::Erased(v) => decode!(v),
    }
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn expanded_full_decoded_history_matches_live_source_reports_and_origins() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(0), Some(3), Some(9), None] {
                expanded_fixture(shared, profile, bound, |owner, budget| {
                    with_decoded_expanded_history(owner, budget, |history, budget| {
                        let floor = budget.storage();
                        let ledger = budget.work_ledger_identity_v1();
                        let History::ScalarCleanup(core) = owner.history();
                        let count = owner.origins().len();
                        let mut origins = Vec::new();
                        budget
                            .reserve_storage(
                                std::mem::size_of_val(&origins)
                                    + count
                                        * std::mem::size_of::<
                                            crate::ProductionExpandedSourceOriginV1,
                                        >(),
                            )
                            .unwrap();
                        origins.try_reserve_exact(count).unwrap();
                        budget
                            .reserve_storage(
                                (origins.capacity() - count)
                                    * std::mem::size_of::<crate::ProductionExpandedSourceOriginV1>(
                                    ),
                            )
                            .unwrap();
                        budget
                            .reserve_storage(std::mem::size_of_val(owner.kernels()))
                            .unwrap();
                        let required = budget.storage();
                        let reports = Expanded::check_decoded_source_history_v1(
                            owner.source_anchor(),
                            history,
                            core,
                            &mut origins,
                            required,
                            budget,
                        )
                        .unwrap();
                        assert_eq!(budget.storage(), required);
                        assert_eq!(reports.as_ref(), owner.kernels());
                        assert_eq!(origins, owner.origins());
                        assert!(budget.work_ledger_identity_v1() == ledger);
                        drop(reports);
                        drop(origins);
                        budget.release_storage(budget.storage() - floor).unwrap();
                    });
                });
            }
        }
    }
}

#[test]
fn expanded_full_decoded_history_rejects_short_floor_and_nonempty_source_slots() {
    for shared in [false, true] {
        expanded_fixture(shared, Profile::Gfx942, Some(3), |owner, budget| {
            with_decoded_expanded_history(owner, budget, |history, budget| {
                let floor = budget.storage();
                let before = budget.work();
                let History::ScalarCleanup(core) = owner.history();
                let mut origins = Vec::new();
                let error = Expanded::check_decoded_source_history_v1(
                    owner.source_anchor(),
                    history,
                    core,
                    &mut origins,
                    0,
                    budget,
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    crate::ProductionExpandedPolicyErrorV1::Resource(
                        AssertOriginResourceV1::Accounting
                    )
                ));
                assert_eq!(budget.work(), before);
                assert_eq!(budget.storage(), floor);
                let count = owner.origins().len();
                budget
                    .reserve_storage(
                        std::mem::size_of_val(&origins)
                            + count
                                * std::mem::size_of::<crate::ProductionExpandedSourceOriginV1>(),
                    )
                    .unwrap();
                origins.try_reserve_exact(count).unwrap();
                budget
                    .reserve_storage(
                        (origins.capacity() - count)
                            * std::mem::size_of::<crate::ProductionExpandedSourceOriginV1>(),
                    )
                    .unwrap();
                assert!(count > 0);
                origins.push(owner.origins()[0]);
                let required = budget.storage();
                assert!(
                    Expanded::check_decoded_source_history_v1(
                        owner.source_anchor(),
                        history,
                        core,
                        &mut origins,
                        required,
                        budget,
                    )
                    .is_err()
                );
                assert_eq!(origins.as_slice(), &owner.origins()[..1]);
                assert_eq!(budget.storage(), required);
                drop(origins);
                budget.release_storage(budget.storage() - floor).unwrap();
            });
        });
    }
}

#[test]
fn expanded_full_decoded_history_refuses_foreign_source_and_restores_paid_backing() {
    fn source_identity(
        anchor: crate::CanonicalOutputFormalSourceAnchorV1<'_>,
    ) -> fe2o3_mir_model::semantic_mir_v1::InertSemanticMirSha256V1 {
        match anchor {
            crate::CanonicalOutputFormalSourceAnchorV1::Direct(v) => {
                v.semantic().semantic().semantic_sha256()
            }
            crate::CanonicalOutputFormalSourceAnchorV1::Erased(v) => v
                .original_source()
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256(),
        }
    }
    for shared in [false, true] {
        expanded_fixture(shared, Profile::Gfx942, Some(3), |owner, budget| {
            with_decoded_expanded_history(owner, budget, |history, budget| {
                // Rebinding identical neutral source is valid; a different loop
                // bound changes the actual source body, not merely the target.
                for (profile, bound, same_source) in
                    [(Profile::Gfx950, 3, true), (Profile::Gfx942, 9, false)]
                {
                    expanded_fixture(shared, profile, Some(bound), |foreign, _| {
                        assert_eq!(
                            source_identity(owner.source_anchor())
                                == source_identity(foreign.source_anchor()),
                            same_source,
                        );
                        let floor = budget.storage();
                        let ledger = budget.work_ledger_identity_v1();
                        // Both actual sources, not a detached target flag, coexist.
                        budget
                            .reserve_storage(foreign.retained_input_storage_floor_v1().unwrap())
                            .unwrap();
                        let count = owner.origins().len();
                        let mut origins = Vec::new();
                        budget
                            .reserve_storage(
                                std::mem::size_of_val(&origins)
                                    + count
                                        * std::mem::size_of::<
                                            crate::ProductionExpandedSourceOriginV1,
                                        >(),
                            )
                            .unwrap();
                        origins.try_reserve_exact(count).unwrap();
                        budget
                            .reserve_storage(
                                (origins.capacity() - count)
                                    * std::mem::size_of::<crate::ProductionExpandedSourceOriginV1>(
                                    ),
                            )
                            .unwrap();
                        budget
                            .reserve_storage(std::mem::size_of_val(owner.kernels()))
                            .unwrap();
                        let required = budget.storage();
                        let History::ScalarCleanup(core) = owner.history();
                        let result = Expanded::check_decoded_source_history_v1(
                            foreign.source_anchor(),
                            history,
                            core,
                            &mut origins,
                            required,
                            budget,
                        );
                        if same_source {
                            let reports = result.unwrap();
                            assert_eq!(reports.as_ref(), owner.kernels());
                            assert_eq!(origins, owner.origins());
                            drop(reports);
                            origins.clear();
                        } else {
                            assert!(matches!(
                                result.unwrap_err(),
                                crate::ProductionExpandedPolicyErrorV1::Source(_)
                            ));
                        }
                        assert!(origins.is_empty());
                        assert_eq!(budget.storage(), required);
                        assert!(budget.work_ledger_identity_v1() == ledger);
                        // A refusal does not damage the genuine original receipt.
                        let reports = Expanded::check_decoded_source_history_v1(
                            owner.source_anchor(),
                            history,
                            core,
                            &mut origins,
                            required,
                            budget,
                        )
                        .unwrap();
                        assert_eq!(reports.as_ref(), owner.kernels());
                        assert_eq!(origins, owner.origins());
                        drop(reports);
                        drop(origins);
                        budget.release_storage(budget.storage() - floor).unwrap();
                    });
                }
            });
        });
    }
}

#[test]
fn expanded_full_decoded_history_work_and_storage_refusals_keep_all_caller_slots() {
    for shared in [false, true] {
        expanded_fixture(shared, Profile::Gfx942, Some(3), |owner, budget| {
            with_decoded_expanded_history(owner, budget, |history, budget| {
                let inherited = budget.storage();
                let count = owner.origins().len();
                let mut origins = Vec::new();
                budget
                    .reserve_storage(
                        std::mem::size_of_val(&origins)
                            + count
                                * std::mem::size_of::<crate::ProductionExpandedSourceOriginV1>()
                            + std::mem::size_of_val(owner.kernels())
                            + 61,
                    )
                    .unwrap();
                origins.try_reserve_exact(count).unwrap();
                budget
                    .reserve_storage(
                        (origins.capacity() - count)
                            * std::mem::size_of::<crate::ProductionExpandedSourceOriginV1>(),
                    )
                    .unwrap();
                let floor = budget.storage();
                let canary = [0x3du8; 61];
                let History::ScalarCleanup(core) = owner.history();
                for (work_limit, storage_limit) in [(0, EXPANDED_STORAGE), (EXPANDED_WORK, floor)] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                    let mut denied = AssertOriginBudgetV1::new(&mut work, storage_limit);
                    denied.reserve_storage(floor).unwrap();
                    let ledger = denied.work_ledger_identity_v1();
                    let result = Expanded::check_decoded_source_history_v1(
                        owner.source_anchor(),
                        history,
                        core,
                        &mut origins,
                        floor,
                        &mut denied,
                    );
                    assert!(matches!(
                        result,
                        Err(crate::ProductionExpandedPolicyErrorV1::Resource(_))
                    ));
                    assert!(origins.is_empty());
                    assert_eq!(denied.storage(), floor);
                    assert!(denied.work_ledger_identity_v1() == ledger);
                    assert_eq!(canary, [0x3d; 61]);
                }
                drop(origins);
                budget
                    .release_storage(budget.storage() - inherited)
                    .unwrap();
                assert_eq!(budget.storage(), inherited);
            });
        });
    }
}
fn expanded_fixture(
    shared: bool,
    profile: Profile,
    bound: Option<u32>,
    run: impl FnOnce(&Expanded, &mut AssertOriginBudgetV1<'_>),
) {
    let (prefix, inherited) = forwarded(shared, profile, bound);
    let sibling = [0xabu8; 37];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, EXPANDED_STORAGE);
    budget.charge_work(19).unwrap();
    budget.reserve_storage(inherited + sibling.len()).unwrap();
    let (unrolled, added) = prefix.unroll(Limits::default(), &mut budget).unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let pointer = unrolled.output().canonical().canonical_bytes().as_ptr();
    let (owner, receipt) = match unrolled {
        UnrolledFixture::Direct(v) => v.continue_expanded_production_policy_v1(&mut budget),
        UnrolledFixture::Erased(v) => v.continue_expanded_production_policy_v1(&mut budget),
    }
    .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        receipt.retained_storage(),
        owner.additional_retained_storage_v1()
    );
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let input = match owner.prefix() {
        PrefixView::Direct(v) => v.output(),
        PrefixView::Erased(v) => v.output(),
    };
    assert_eq!(input.canonical().canonical_bytes().as_ptr(), pointer);
    assert_eq!(input.module().kernels, owner.output().module().kernels);
    assert_eq!(owner.kernels().len(), owner.output().module().kernels.len());
    assert!(!owner.grants_artifact_or_launch_authority());
    owner.verify_equivalence(&mut budget).unwrap();
    let History::ScalarCleanup(core) = owner.history();
    assert!(!core.rounds().is_empty());
    assert!(core.rounds().len() <= fe2o3_kernel_opt::SCALAR_FIXED_POINT_MAX_ROUNDS_V1);
    let mut previous = input;
    for (ordinal, round) in core.rounds().iter().enumerate() {
        assert_eq!(round.ordinal() as usize, ordinal);
        assert_eq!(
            round.integer().native_input_audit_bytes(),
            previous.canonical().canonical_bytes()
        );
        assert_eq!(
            round.scalar().native_input_audit_bytes(),
            round.integer().owner().canonical().canonical_bytes()
        );
        let unchanged =
            previous.canonical().canonical_bytes() == round.output().canonical().canonical_bytes();
        assert_eq!(unchanged, ordinal + 1 == core.rounds().len());
        previous = round.output();
    }
    assert!(std::ptr::eq(previous, owner.output()));
    run(&owner, &mut budget);
    assert_eq!(budget.storage(), floor + receipt.retained_storage());
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(sibling, [0xab; 37]);
}

#[test]
fn expanded_source_owner_both_routes_profiles_and_complete_history() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(0), Some(3), Some(9), None] {
                expanded_fixture(shared, profile, bound, |owner, budget| {
                    let History::ScalarCleanup(core) = owner.history();
                    let mut input = match owner.prefix() {
                        PrefixView::Direct(v) => v.output(),
                        PrefixView::Erased(v) => v.output(),
                    };
                    for round in core.rounds() {
                        expanded_source_tests::exercise_transport_rows(
                            input,
                            round.integer().owner(),
                            round.integer().occurrences().candidate(),
                            budget,
                        );
                        expanded_source_tests::exercise_transport_rows(
                            round.integer().owner(),
                            round.output(),
                            round.scalar().occurrences().candidate(),
                            budget,
                        );
                        input = round.output();
                    }
                    assert!(owner.origins().iter().all(|v| !v.grants_authority()));
                    if bound == Some(0) {
                        assert_eq!(
                            core.rounds().len(),
                            1,
                            "zero-trip prefix is a genuine normal no-op"
                        );
                    }
                });
            }
        }
    }
}

#[test]
fn expanded_final_add_keeps_checked_u_proof_after_false_companion_is_deleted() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            expanded_fixture(shared, profile, Some(9), |owner, _| {
                let input = match owner.prefix() {
                    PrefixView::Direct(v) => v.output(),
                    PrefixView::Erased(v) => v.output(),
                };
                let mut witnesses = 0;
                for row in owner.origins() {
                    let Some(old) = row.original_u() else {
                        continue;
                    };
                    let before = &input.module().functions[old.block.function.0 as usize]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[old.block.block as usize]
                        .operations;
                    let now = row.output();
                    let after = &owner.output().module().functions[now.block.function.0 as usize]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[now.block.block as usize]
                        .operations[now.operation as usize];
                    if matches!(
                        after.kind,
                        fe2o3_kernel_ir::OperationKind::Binary {
                            op: fe2o3_kernel_ir::BinaryOp::Add,
                            ..
                        }
                    ) && matches!(
                        before[old.operation as usize].kind,
                        fe2o3_kernel_ir::OperationKind::Binary {
                            op: fe2o3_kernel_ir::BinaryOp::Add,
                            ..
                        }
                    ) {
                        let flag = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                            operation: old.operation + 1,
                            ..old
                        };
                        if before.get(flag.operation as usize).is_some_and(|v| {
                            v.kind
                                == fe2o3_kernel_ir::OperationKind::Constant(
                                    fe2o3_kernel_ir::Constant::Bool(false),
                                )
                        }) && !owner.origins().iter().any(|v| v.original_u() == Some(flag))
                        {
                            witnesses += 1;
                        }
                    }
                }
                assert!(
                    witnesses > 0,
                    "must retain a real admitted Add while deleting its old proof companion"
                );
            });
        }
    }
}

#[test]
fn expanded_decoded_direct_and_erased_use_the_same_source_frontend() {
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_unrolled(shared, profile, Some(3), |owner, budget| {
                macro_rules! inspect {
                    ($owner:expr) => {{
                        let v = $owner;
                        let sixth = v
                            .prefix()
                            .prefix()
                            .prefix()
                            .prefix()
                            .prefix()
                            .prefix()
                            .prefix()
                            .prefix();
                        let checked = sixth
                            .checked_output()
                            .intermediate_policy5()
                            .intermediate_policy4()
                            .intermediate_policy3();
                        expanded_source_tests::exercise_decoded_source_pair(
                            v.expanded_source_anchor_v1(),
                            sixth.bound(),
                            checked,
                            budget,
                        );
                    }};
                }
                match owner {
                    UnrolledFixture::Direct(v) => inspect!(v),
                    UnrolledFixture::Erased(v) => inspect!(v),
                }
            });
        }
    }
}

#[test]
fn expanded_replay_short_floor_refuses_without_releasing_siblings() {
    for shared in [false, true] {
        expanded_fixture(shared, Profile::Gfx942, Some(3), |owner, _| {
            let required = owner.retained_input_storage_floor_v1().unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, EXPANDED_STORAGE);
            budget.reserve_storage(required - 1).unwrap();
            assert!(matches!(
                owner.verify_equivalence(&mut budget),
                Err(crate::ProductionExpandedPolicyErrorV1::Resource(
                    AssertOriginResourceV1::Accounting
                ))
            ));
            assert_eq!(budget.storage(), required - 1);
            assert_eq!(budget.work(), 0);
        });
    }
}

#[test]
fn expanded_closed_schedule_refuses_genuine_nonproduction_unroll_settings() {
    for shared in [false, true] {
        let (prefix, inherited) = forwarded(shared, Profile::Gfx942, Some(3));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, EXPANDED_STORAGE);
        budget.reserve_storage(inherited + 47).unwrap();
        let limits = Limits {
            max_iterations: 1,
            ..Limits::default()
        };
        let (unrolled, receipt) = prefix.unroll(limits, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        unrolled.replay(&mut budget).unwrap();
        let floor = budget.storage();
        let result = match unrolled {
            UnrolledFixture::Direct(v) => v.continue_expanded_production_policy_v1(&mut budget),
            UnrolledFixture::Erased(v) => v.continue_expanded_production_policy_v1(&mut budget),
        };
        assert!(matches!(
            result,
            Err(crate::ProductionExpandedPolicyErrorV1::Policy)
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn expanded_replay_work_and_storage_denials_restore_full_owner_floor() {
    for shared in [false, true] {
        expanded_fixture(shared, Profile::Gfx942, Some(3), |owner, _| {
            let floor = owner.retained_input_storage_floor_v1().unwrap() + 53;
            let mut success = CanonicalKernelIrWorkBudgetV1::new(EXPANDED_WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut success, EXPANDED_STORAGE);
            budget.reserve_storage(floor).unwrap();
            owner.verify_equivalence(&mut budget).unwrap();
            let work = budget.work();
            let peak = budget.peak_storage();
            assert_eq!(budget.storage(), floor);
            // Measured total/peak boundaries only, not an independent exact
            // accepted-interior-prefix oracle or an outer policy work proof.
            for (w, s) in [
                (0, EXPANDED_STORAGE),
                (work - 1, EXPANDED_STORAGE),
                (EXPANDED_WORK, peak - 1),
                (EXPANDED_WORK, floor),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(w);
                let mut denied = AssertOriginBudgetV1::new(&mut work, s);
                denied.reserve_storage(floor).unwrap();
                assert!(owner.verify_equivalence(&mut denied).is_err());
                assert_eq!(denied.storage(), floor);
            }
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn expanded_source_sim_checks_memory_sites_order_and_complete_cleanup() {
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
        CanonicalKirOperationCoordinateV1 as Op, VerifiedCanonicalKernelIrV12,
    };
    use fe2o3_kir_sim::{
        AdmittedSimulationModuleV1, ScalarBitsV1, SimulationArgumentV1,
        SimulationEventKindV1 as Kind, SimulationEventSinkErrorV1, SimulationEventSinkV1,
        SimulationEventV1 as Event, SimulationExecutionOutcomeV1 as Outcome, SimulationLimitsV1,
        SimulationRequestV1, SimulationTargetV1,
    };
    #[derive(Default)]
    struct Events(Vec<Event>);
    impl SimulationEventSinkV1 for Events {
        fn record(&mut self, event: &Event) -> Result<(), SimulationEventSinkErrorV1> {
            if self.0.len() == 524_288 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "expanded trace bound".into(),
                });
            }
            self.0.push(event.clone());
            Ok(())
        }
    }
    let simulate = |graph: &Graph| {
        AdmittedSimulationModuleV1::admit_v12(
            VerifiedCanonicalKernelIrV12::from_canonical_bytes(
                graph.canonical().canonical_bytes().to_vec(),
            )
            .unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap()
    };
    let mut pairs = 0;
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(0), Some(3), Some(9), None] {
                expanded_fixture(shared, profile, bound, |owner, _| {
                    let input = match owner.prefix() {
                        PrefixView::Direct(v) => v.output(),
                        PrefixView::Erased(v) => v.output(),
                    };
                    let before = simulate(input);
                    let after = simulate(owner.output());
                    for kernel in &input.module().kernels {
                        for control in [0, 3] {
                            let request = SimulationRequestV1::new(
                                kernel.id.as_str(),
                                [64, 1, 1],
                                [64, 1, 1],
                                vec![SimulationArgumentV1::Scalar(ScalarBitsV1::u32(control))],
                            );
                            let original_request = request.clone();
                            let mut last = None;
                            for _ in 0..2 {
                                let mut a = Events::default();
                                let mut b = Events::default();
                                let old = before
                                    .simulate_observed_with_sink(
                                        &request,
                                        SimulationTargetV1::amdgpu_64(),
                                        SimulationLimitsV1::default(),
                                        &mut a,
                                    )
                                    .unwrap();
                                let new = after
                                    .simulate_observed_with_sink(
                                        &request,
                                        SimulationTargetV1::amdgpu_64(),
                                        SimulationLimitsV1::default(),
                                        &mut b,
                                    )
                                    .unwrap();
                                assert_eq!(old.arguments(), new.arguments());
                                assert_eq!(old.shared_buffers(), new.shared_buffers());
                                assert_eq!(old.race_assessment(), new.race_assessment());
                                assert_eq!(old.conflict_assessment(), new.conflict_assessment());
                                assert_eq!(
                                    (old.invocations_executed(), new.invocations_executed()),
                                    (64, 64)
                                );
                                let memory =
                                    |graph: &Graph, events: &[Event], final_graph: bool| {
                                        events
                                            .iter()
                                            .filter_map(|event| {
                                                let Kind::MemoryWrite {
                                                    allocation,
                                                    offset,
                                                    bytes,
                                                } = event.kind
                                                else {
                                                    return None;
                                                };
                                                let blocks = &graph.module().functions
                                                    [event.site.function_ordinal]
                                                    .body
                                                    .as_ref()
                                                    .unwrap()
                                                    .blocks;
                                                let at = Op {
                                                    block: Block {
                                                        function: Function(
                                                            event
                                                                .site
                                                                .function_ordinal
                                                                .try_into()
                                                                .unwrap(),
                                                        ),
                                                        block: blocks
                                                            .iter()
                                                            .position(|b| b.id == event.site.block)
                                                            .unwrap()
                                                            .try_into()
                                                            .unwrap(),
                                                    },
                                                    operation: event.site.operation.unwrap(),
                                                };
                                                let at = if final_graph {
                                                    owner
                                                        .origins()
                                                        .iter()
                                                        .find(|v| v.output() == at)
                                                        .unwrap()
                                                        .original_u()
                                                        .unwrap()
                                                } else {
                                                    at
                                                };
                                                assert_eq!((offset, bytes), (0, 4));
                                                Some((
                                                    event.invocation,
                                                    at,
                                                    allocation,
                                                    offset,
                                                    bytes,
                                                ))
                                            })
                                            .collect::<Vec<_>>()
                                    };
                                let old_memory = memory(input, &a.0, false);
                                let new_memory = memory(owner.output(), &b.0, true);
                                assert_eq!(old_memory, new_memory);
                                assert_eq!(
                                    old_memory.len(),
                                    bound.unwrap_or(control) as usize * 64
                                );
                                for events in [&a.0, &b.0] {
                                    assert_eq!(
                                        events
                                            .iter()
                                            .filter(|e| matches!(e.kind, Kind::MemoryRead { .. }))
                                            .count(),
                                        0
                                    );
                                    assert_eq!(
                                        events
                                            .iter()
                                            .filter(|e| matches!(e.kind, Kind::InvocationBegin))
                                            .count(),
                                        64
                                    );
                                    assert_eq!(
                                        events
                                            .iter()
                                            .filter(|e| matches!(
                                                e.kind,
                                                Kind::InvocationEnd {
                                                    outcome: Outcome::Completed
                                                }
                                            ))
                                            .count(),
                                        64
                                    );
                                    assert_eq!(
                                        events
                                            .iter()
                                            .filter(|e| matches!(
                                                e.kind,
                                                Kind::InvocationEnd {
                                                    outcome: Outcome::Failed
                                                }
                                            ))
                                            .count(),
                                        0
                                    );
                                    let created = events
                                        .iter()
                                        .filter_map(|e| {
                                            if let Kind::AllocationCreated { allocation, .. } =
                                                e.kind
                                            {
                                                Some(allocation)
                                            } else {
                                                None
                                            }
                                        })
                                        .collect::<Vec<_>>();
                                    let released = events
                                        .iter()
                                        .filter_map(|e| {
                                            if let Kind::AllocationReleased { allocation } = e.kind
                                            {
                                                Some(allocation)
                                            } else {
                                                None
                                            }
                                        })
                                        .collect::<Vec<_>>();
                                    let created_set =
                                        created.iter().collect::<std::collections::BTreeSet<_>>();
                                    let released_set =
                                        released.iter().collect::<std::collections::BTreeSet<_>>();
                                    assert_eq!(created.len(), created_set.len());
                                    assert_eq!(released.len(), released_set.len());
                                    assert_eq!(created_set, released_set);
                                }
                                if let Some((previous_a, previous_b)) = &last {
                                    assert_eq!(&a.0, previous_a);
                                    assert_eq!(&b.0, previous_b);
                                }
                                last = Some((a.0, b.0));
                                assert_eq!(request, original_request);
                                pairs += 1;
                            }
                        }
                    }
                });
            }
        }
    }
    // Two profiles * (one Direct + two Erased roots) * four shapes * two inputs * two repeats.
    assert_eq!(pairs, 96);
}
