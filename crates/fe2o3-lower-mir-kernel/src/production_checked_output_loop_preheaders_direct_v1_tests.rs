// Genuine constructed semantic source through all existing stages, not a
// detached canonical CFG or an authenticated rustc/collector/runtime result.
use super::*;
use crate::{
    ProductionLoopPreheadersErrorV1 as PreheaderError,
    ProductionOwnedPrivateCellPromotionContinuationV1 as Promoted,
};

fn loop_prefix8(profile: Profile) -> (Prefix8, usize) {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            let jump =
                |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
            let switch = |operand, bits, target, otherwise| SemanticTerminatorKindV1::SwitchInt {
                discriminant: operand,
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        bits,
                        edge(SemanticEdgeRoleV1::SwitchValue, target),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
                )
                .unwrap(),
            };
            vec![
                block(
                    31,
                    vec![
                        assignment(
                            2,
                            U64,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::PointerMetadata,
                                operand: value(1, SLICE_REF),
                            },
                        ),
                        assignment(5, U32, SemanticRvalueKindV1::Use(constant(U32, 0, 4))),
                    ],
                    switch(value(2, U64), 0, 2, 1),
                ),
                block(
                    32,
                    vec![assignment(
                        5,
                        U32,
                        SemanticRvalueKindV1::Use(constant(U32, 1, 4)),
                    )],
                    jump(2),
                ),
                block(
                    33,
                    vec![assignment(
                        6,
                        BOOL,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: value(5, U32),
                            right: constant(U32, 3, 4),
                        },
                    )],
                    switch(value(6, BOOL), 1, 3, 4),
                ),
                block(
                    34,
                    vec![assignment(
                        5,
                        U32,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::Add,
                            left: value(5, U32),
                            right: constant(U32, 1, 4),
                        },
                    )],
                    jump(2),
                ),
                block(35, vec![], SemanticTerminatorKindV1::Return),
            ]
        },
        |_| "private_array_relation".to_owned(),
        &[U32, BOOL],
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let input = fixture6_from_source(profile, source);
    let inherited = input.floor;
    let prefix = admit6(input, WORK, STORAGE).0.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (prefix, seven) = prefix
        .continue_redundant_private_stores_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(seven.retained_storage()).unwrap();
    let (prefix, eight) = prefix
        .continue_commutative_bitwise_cse_v1(&mut budget)
        .unwrap();
    (
        prefix,
        inherited + seven.retained_storage() + eight.retained_storage(),
    )
}
fn promoted(profile: Profile, mutation: bool) -> (Promoted, usize) {
    let (prefix, inherited) = if mutation {
        loop_prefix8(profile)
    } else {
        noop_prefix8(profile)
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (owner, added) = prefix
        .continue_private_cell_promotion_v1(&mut budget)
        .unwrap();
    (owner, inherited + added.retained_storage())
}

#[test]
fn source_loop_preheaders_direct_genuine_loop_mutates_and_retains_exact_synthetic_origins_both_profiles()
 {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let (prefix, inherited) = promoted(profile, true);
        let input = prefix.output().canonical().canonical_bytes().as_ptr();
        let p8 = prefix
            .prefix()
            .output()
            .canonical()
            .canonical_bytes()
            .as_ptr();
        let sibling = vec![0xa5_u8; 97];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        let floor = inherited + sibling.capacity();
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let (mut owner, added) = prefix.continue_loop_preheaders_v1(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            added.retained_storage(),
            owner.additional_retained_storage_v1()
        );
        budget.reserve_storage(added.retained_storage()).unwrap();
        assert_eq!(
            owner
                .prefix()
                .output()
                .canonical()
                .canonical_bytes()
                .as_ptr(),
            input
        );
        assert_eq!(
            owner
                .prefix()
                .prefix()
                .output()
                .canonical()
                .canonical_bytes()
                .as_ptr(),
            p8
        );
        assert_ne!(
            owner.prefix().output().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
        assert_eq!(
            owner.origins().len(),
            1,
            "genuine full source pipeline must select a preheader"
        );
        let origin = &owner.origins()[0];
        assert_eq!(origin.incoming().len(), 2);
        assert!(
            !origin.parameters().is_empty(),
            "typed source SSA forwarding, not a detached fixture"
        );
        assert_eq!(owner.incoming_origins().len(), 2);
        assert_ne!(
            owner.incoming_origins()[0].input(),
            owner.incoming_origins()[1].input()
        );
        for incoming in owner.incoming_origins() {
            assert_eq!(incoming.input(), incoming.output());
        }
        assert_eq!(
            owner.continuation().preheaders()[0].header,
            origin.input_header()
        );
        assert_eq!(
            owner.continuation().preheaders()[0].preheader,
            origin.output_preheader()
        );
        assert_eq!(owner.kernels().len(), 1);
        assert!(!owner.grants_artifact_or_launch_authority());
        owner.verify_equivalence(&mut budget).unwrap();
        owner.exercise_source_preheader_origin_refusals_v1(&mut budget);
        owner.exercise_source_preheader_foreign_inventory_v1(&mut budget);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(sibling.iter().all(|byte| *byte == 0xa5));
        drop(owner);
        budget.release_storage(added.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn source_loop_preheaders_direct_noop_is_actual_fresh_output_with_empty_origins_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let (prefix, inherited) = promoted(profile, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited + 31).unwrap();
        let floor = budget.storage();
        let (owner, added) = prefix.continue_loop_preheaders_v1(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(added.retained_storage()).unwrap();
        assert!(
            owner.origins().is_empty()
                && owner.incoming_origins().is_empty()
                && owner.parameter_origins().is_empty()
        );
        assert_eq!(
            owner.output().canonical().canonical_bytes(),
            owner.prefix().output().canonical().canonical_bytes()
        );
        assert!(!std::ptr::eq(owner.output(), owner.prefix().output()));
        owner.verify_equivalence(&mut budget).unwrap();
        let minimum = owner.retained_input_storage_floor_v1().unwrap();
        let mut short_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut short = AssertOriginBudgetV1::new(&mut short_work, STORAGE);
        short.reserve_storage(minimum - 1).unwrap();
        assert!(matches!(
            owner.verify_equivalence(&mut short),
            Err(PreheaderError::Resource(AssertOriginResourceV1::Accounting))
        ));
        assert_eq!((short.storage(), short.work()), (minimum - 1, 0));
    }
}

#[test]
fn source_loop_preheaders_direct_nested_error_and_panic_drop_real_partial_candidates() {
    let (prefix, inherited) = promoted(Profile::Gfx942, true);
    prefix.exercise_source_preheader_failed_candidate_v1(inherited);
}

#[test]
fn source_loop_preheaders_direct_exact_work_and_one_short_preserve_history_and_floor() {
    for mutation in [false, true] {
        let (prefix, _) = promoted(Profile::Gfx942, mutation);
        let minimum = prefix.retained_input_storage_floor_v1().unwrap();
        let mut short_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut short = AssertOriginBudgetV1::new(&mut short_work, STORAGE);
        short.reserve_storage(minimum - 1).unwrap();
        assert!(matches!(
            prefix.continue_loop_preheaders_v1(&mut short),
            Err(PreheaderError::Resource(AssertOriginResourceV1::Accounting))
        ));
        assert_eq!((short.storage(), short.work()), (minimum - 1, 0));
        let run = |limit, storage| {
            let (prefix, inherited) = promoted(Profile::Gfx942, mutation);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let (result, accepted, peak) = {
                let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
                let floor = inherited + 29;
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(17).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result =
                    prefix
                        .continue_loop_preheaders_v1(&mut budget)
                        .map(|(owner, added)| {
                            assert_eq!(
                                owner.additional_retained_storage_v1(),
                                added.retained_storage()
                            );
                            drop(owner);
                        });
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(budget.failed_storage(), None);
                (result, budget.work(), budget.peak_storage())
            };
            (result, accepted, peak, work.failed_work())
        };
        let (result, measured, peak, denied) = run(WORK, STORAGE);
        result.unwrap();
        assert_eq!(denied, None);
        let (result, accepted, exact_peak, denied) = run(measured, peak);
        result.unwrap();
        assert_eq!((accepted, exact_peak, denied), (measured, peak, None));
        let (result, accepted, _, denied) = run(measured - 1, peak);
        match result {
            Err(PreheaderError::Admission(error)) => match *error {
                PromotionError::Admission(
                    crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error),
                    ),
                ) => {
                    assert_eq!(error.actual(), measured);
                    assert_eq!(error.limit(), measured - 1);
                }
                error => panic!("exact final formal-census work refusal, got {error:?}"),
            },
            other => panic!("exact final formal-census work refusal, got {other:?}"),
        }
        assert_eq!((accepted, denied), (measured - 1, Some(measured)));
    }
}

#[test]
fn source_loop_preheaders_direct_one_short_storage_observes_phase_with_live_sibling() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let run = |limit, storage| {
                let (prefix, inherited) = promoted(profile, mutation);
                let sibling = vec![0x6d_u8; 29];
                let floor = inherited + std::mem::size_of_val(&sibling) + sibling.capacity();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let (result, accepted, peak, denied_storage) = {
                    let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
                    budget.reserve_storage(floor).unwrap();
                    budget.charge_work(17).unwrap();
                    let ledger = budget.work_ledger_identity_v1();
                    let result =
                        prefix
                            .continue_loop_preheaders_v1(&mut budget)
                            .map(|(owner, added)| {
                                assert_eq!(
                                    owner.additional_retained_storage_v1(),
                                    added.retained_storage()
                                );
                                if mutation {
                                    assert_eq!(owner.origins().len(), 1);
                                    assert_eq!(owner.incoming_origins().len(), 2);
                                    assert!(!owner.origins()[0].parameters().is_empty());
                                    assert_ne!(
                                        owner.output().canonical().canonical_bytes(),
                                        owner.prefix().output().canonical().canonical_bytes()
                                    );
                                } else {
                                    assert!(owner.origins().is_empty());
                                    assert!(owner.incoming_origins().is_empty());
                                    assert!(owner.parameter_origins().is_empty());
                                    assert_eq!(
                                        owner.output().canonical().canonical_bytes(),
                                        owner.prefix().output().canonical().canonical_bytes()
                                    );
                                    assert!(!std::ptr::eq(owner.output(), owner.prefix().output()));
                                }
                                drop(owner);
                            });
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    assert_eq!(sibling, vec![0x6d; 29]);
                    (
                        result,
                        budget.work(),
                        budget.peak_storage(),
                        budget.failed_storage(),
                    )
                };
                (
                    result,
                    accepted,
                    peak,
                    denied_storage,
                    work.failed_work(),
                    floor,
                )
            };
            let (result, measured_work, measured_storage, denied_storage, denied_work, floor) =
                run(WORK, STORAGE);
            result.unwrap();
            assert_eq!((denied_storage, denied_work), (None, None));
            let (result, accepted, peak, denied_storage, denied_work, exact_floor) =
                run(measured_work, measured_storage);
            result.unwrap();
            assert_eq!(
                (accepted, peak, denied_storage, denied_work),
                (measured_work, measured_storage, None, None)
            );
            assert_eq!(exact_floor, floor);
            let full = preheader_public_frontier(profile, mutation, STORAGE);
            assert!(
                full.result.is_ok(),
                "successful public frontier: {:?}",
                full.result
            );
            assert_eq!((full.failed_storage, full.failed_work), (None, None));
            assert_eq!((full.peak, full.floor), (measured_storage, floor));
            let storage_limit = measured_storage.checked_sub(1).unwrap();
            let expected = preheader_public_frontier(profile, mutation, storage_limit);
            assert_preheader_frontier_storage(&expected.result, measured_storage);
            assert_eq!(
                (expected.failed_storage, expected.failed_work),
                (Some(measured_storage), None)
            );
            assert_eq!(expected.floor, floor);
            let (result, accepted, peak, denied_storage, denied_work, short_floor) =
                run(measured_work, storage_limit);
            assert_eq!(short_floor, floor);
            let error = result.unwrap_err();
            assert_eq!(denied_storage, Some(measured_storage));
            assert_eq!(denied_work, None);
            assert!(peak < measured_storage);
            let PreheaderError::Admission(error) = error else {
                panic!("expected final-source admission Storage refusal")
            };
            let crate::ProductionPrivateCellPromotionContinuationErrorV1::Prefix(error) = *error
            else {
                panic!("expected retained private-cell prefix refusal")
            };
            let crate::ProductionCommutativeContinuationErrorV1::Prefix(error) = *error else {
                panic!("expected retained commutative prefix refusal")
            };
            let crate::ProductionRedundantStoreAdmissionErrorV1::Prefix(error) = *error else {
                panic!("expected retained redundant-store prefix refusal")
            };
            let crate::ProductionCheckedOutputAdmissionErrorPolicy6V1::Optimization(
                fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Map(
                    fe2o3_pliron::KirOptimizationMapErrorV12::Resources(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(
                            error,
                        ),
                    ),
                ),
            ) = *error
            else {
                panic!("expected exact Policy6 replay map Storage refusal")
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (measured_storage, storage_limit)
            );
            assert_eq!((accepted, peak), (expected.work, expected.peak));
        }
    }
}

struct PreheaderFrontier {
    result: Result<(), crate::ProductionCommutativeContinuationErrorV1>,
    floor: usize,
    work: usize,
    peak: usize,
    failed_storage: Option<usize>,
    failed_work: Option<usize>,
}

// Independent paid public stages stop at P8. No final source constructor or
// private origin builder supplies an expected work/storage observation.
fn preheader_public_frontier(profile: Profile, mutation: bool, limit: usize) -> PreheaderFrontier {
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
    let (prefix, inherited) = promoted(profile, mutation);
    if mutation {
        let p6 = prefix.prefix().prefix().prefix();
        for graph in [
            p6.bound(),
            p6.checked_output().intermediate_policy5().owner(),
        ] {
            preheader_comparison_shape_premise(graph);
        }
    }
    let sibling = vec![0x6d_u8; 29];
    let floor = inherited
        .checked_add(std::mem::size_of_val(&sibling))
        .and_then(|n| n.checked_add(sibling.capacity()))
        .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = AssertOriginBudgetV1::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        prefix
            .verify_equivalence(&mut budget)
            .expect("promoted prefix checkpoint");
        let prefix_work = budget.work();
        let tail = fe2o3_kernel_opt::prepare_owned_loop_preheaders_v1(prefix.output(), &mut budget)
            .expect("standalone preheader preparation checkpoint");
        budget.reserve_storage(tail.retained_storage()).unwrap();
        let prepare_work = budget.work();
        let (pair, receipt) = tail
            .replay_against(prefix.output(), &mut budget)
            .expect("actual preheader pair checkpoint");
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let pair_work = budget.work();
        let (input, receipt) = Inventory::derive(prefix.output(), &mut budget)
            .expect("preheader input inventory checkpoint");
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let input_work = budget.work();
        let (output, receipt) = Inventory::derive(tail.output(), &mut budget)
            .expect("preheader output inventory checkpoint");
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let output_work = budget.work();
        let origins = preheader_origin_reference(&pair, &input, &output, &mut budget);
        let origin_work = budget.work();
        for (before, after) in [
            (17, prefix_work),
            (prefix_work, prepare_work),
            (prepare_work, pair_work),
            (pair_work, input_work),
            (input_work, output_work),
            (output_work, origin_work),
        ] {
            assert!(after > before, "each preceding public checkpoint completed");
        }
        budget.charge_work(3).unwrap();
        let result = prefix.prefix().verify_equivalence(&mut budget);
        drop(origins);
        drop((output, input, pair));
        drop(tail);
        budget
            .release_storage(budget.storage().checked_sub(floor).unwrap())
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x6d; 29]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    drop(prefix);
    PreheaderFrontier {
        result,
        floor,
        work: accepted,
        peak,
        failed_storage,
        failed_work: work.failed_work(),
    }
}

fn assert_preheader_frontier_storage(
    result: &Result<(), crate::ProductionCommutativeContinuationErrorV1>,
    peak: usize,
) {
    let Err(crate::ProductionCommutativeContinuationErrorV1::Prefix(store)) = result else {
        panic!("exact public P8 prefix refusal: {result:?}")
    };
    let crate::ProductionRedundantStoreAdmissionErrorV1::Prefix(p6) = store.as_ref() else {
        panic!("exact public P7 prefix refusal: {store:?}")
    };
    let crate::ProductionCheckedOutputAdmissionErrorPolicy6V1::Optimization(
        fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Map(
            fe2o3_pliron::KirOptimizationMapErrorV12::Resources(AssertOriginResourceV1::Storage(
                error,
            )),
        ),
    ) = p6.as_ref()
    else {
        panic!("exact final-source Policy6 map reservation: {p6:?}")
    };
    assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
}

type PreheaderOriginBacking = (
    Vec<crate::ProductionLoopPreheaderOriginV1>,
    Vec<crate::ProductionLoopPreheaderIncomingOriginV1>,
    Vec<crate::ProductionLoopPreheaderParameterOriginV1>,
);

fn preheader_reference_rows<T>(count: usize, budget: &mut AssertOriginBudgetV1<'_>) -> Vec<T> {
    // General source scratch pays six, including its Vec header. It is NOT
    // the later unroll stage's separate four-unit, backing-only helper.
    budget.charge_work(6).unwrap();
    let requested = count.checked_mul(std::mem::size_of::<T>()).unwrap();
    budget
        .reserve_storage(
            requested
                .checked_add(std::mem::size_of::<Vec<T>>())
                .unwrap(),
        )
        .unwrap();
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).unwrap();
    let actual = rows
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .unwrap();
    budget
        .reserve_storage(actual.checked_sub(requested).unwrap())
        .unwrap();
    rows
}

fn preheader_origin_reference(
    pair: &fe2o3_kernel_analysis::CheckedCanonicalKirLoopPreheadersV1<'_>,
    input: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    output: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Option<PreheaderOriginBacking> {
    use crate::{
        ProductionLoopPreheaderIncomingOriginV1 as Incoming,
        ProductionLoopPreheaderOriginV1 as Block,
        ProductionLoopPreheaderParameterOriginV1 as Parameter,
    };
    use std::mem::size_of;
    assert!(std::ptr::eq(pair.input(), input.owner()));
    assert!(std::ptr::eq(pair.output(), output.owner()));
    let start_work = budget.work();
    let start_storage = budget.storage();
    budget.charge_work(4).unwrap();
    let n = pair.preheaders().len();
    if n == 0 {
        assert_eq!(budget.work() - start_work, 4);
        assert_eq!(budget.storage(), start_storage);
        return None;
    }
    let blocks = output.blocks().len();
    let edges = output.edges().len();
    let mut incoming = 0usize;
    let mut parameters = 0usize;
    // This test-only census joins the whole actual pair/graphs, not the
    // production Origins rows or their stored/recomputed storage receipt.
    for selected in pair.preheaders() {
        let before = input
            .blocks()
            .iter()
            .find(|b| b.coordinate == selected.header)
            .unwrap();
        let after = output
            .blocks()
            .iter()
            .find(|b| b.coordinate == selected.preheader)
            .unwrap();
        assert_eq!(before.parameters.len(), after.parameters.len());
        parameters = parameters.checked_add(before.parameters.len()).unwrap();
        let mut count = 0usize;
        for edge in output
            .edges()
            .iter()
            .filter(|e| e.target == selected.preheader)
        {
            let original = input
                .edges()
                .iter()
                .find(|e| e.coordinate == edge.coordinate)
                .unwrap();
            assert_eq!(original.target, selected.header);
            count = count.checked_add(1).unwrap();
        }
        assert!(count > 0);
        incoming = incoming.checked_add(count).unwrap();
    }
    let block_rows = preheader_reference_rows::<Block>(n, budget);
    let selected_rows = preheader_reference_rows::<Option<usize>>(blocks, budget);
    budget.charge_work(blocks).unwrap();
    let cursors = preheader_reference_rows::<usize>(n, budget);
    budget.charge_work(n.checked_mul(19).unwrap()).unwrap();
    budget
        .charge_work(
            edges
                .checked_mul(7)
                .unwrap()
                .checked_add(incoming.checked_mul(9).unwrap())
                .unwrap(),
        )
        .unwrap();
    budget.charge_work(n.checked_mul(4).unwrap()).unwrap();
    let incoming_rows = preheader_reference_rows::<Incoming>(incoming, budget);
    budget.charge_work(incoming).unwrap();
    budget.charge_work(edges.checked_mul(8).unwrap()).unwrap();
    let parameter_rows = preheader_reference_rows::<Parameter>(parameters, budget);
    budget
        .charge_work(n.checked_add(parameters).unwrap().checked_mul(3).unwrap())
        .unwrap();
    let expected_work = 4usize
        .checked_add(5 * 6)
        .unwrap()
        .checked_add(blocks)
        .unwrap()
        .checked_add(n.checked_mul(26).unwrap())
        .unwrap()
        .checked_add(edges.checked_mul(15).unwrap())
        .unwrap()
        .checked_add(incoming.checked_mul(10).unwrap())
        .unwrap()
        .checked_add(parameters.checked_mul(3).unwrap())
        .unwrap();
    assert_eq!(
        budget.work().checked_sub(start_work).unwrap(),
        expected_work
    );
    let mut expected_storage = 0usize;
    for (capacity, cell, header) in [
        (
            block_rows.capacity(),
            size_of::<Block>(),
            size_of::<Vec<Block>>(),
        ),
        (
            selected_rows.capacity(),
            size_of::<Option<usize>>(),
            size_of::<Vec<Option<usize>>>(),
        ),
        (
            cursors.capacity(),
            size_of::<usize>(),
            size_of::<Vec<usize>>(),
        ),
        (
            incoming_rows.capacity(),
            size_of::<Incoming>(),
            size_of::<Vec<Incoming>>(),
        ),
        (
            parameter_rows.capacity(),
            size_of::<Parameter>(),
            size_of::<Vec<Parameter>>(),
        ),
    ] {
        expected_storage = expected_storage
            .checked_add(header)
            .unwrap()
            .checked_add(capacity.checked_mul(cell).unwrap())
            .unwrap();
    }
    assert_eq!(
        budget.storage().checked_sub(start_storage).unwrap(),
        expected_storage
    );
    drop((selected_rows, cursors));
    // The enclosing source scope retains even released scratch's logical
    // charge until all final-source dependents drop. No early refund here.
    Some((block_rows, incoming_rows, parameter_rows))
}

fn preheader_comparison_shape_premise(graph: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12) {
    use fe2o3_kernel_ir::{ComparePredicate, Constant, OperationKind};
    let mut comparisons = 0;
    for function in &graph.module().functions {
        let Some(body) = &function.body else { continue };
        for block in &body.blocks {
            for operation in &block.operations {
                let OperationKind::Compare {
                    predicate,
                    lhs,
                    rhs,
                } = operation.kind
                else {
                    continue;
                };
                comparisons += 1;
                assert_eq!(predicate, ComparePredicate::LessThan);
                let constant = |value| {
                    body.blocks
                        .iter()
                        .flat_map(|b| &b.operations)
                        .find_map(|op| {
                            (op.results.iter().any(|r| r.id == value)).then_some(&op.kind)
                        })
                };
                assert!(matches!(
                    constant(rhs),
                    Some(OperationKind::Constant(Constant::U32(3)))
                ));
                assert!(!matches!(constant(lhs), Some(OperationKind::Constant(_))));
            }
        }
    }
    assert_eq!(comparisons, 1);
    // Complete P6: B/C transition, control index, decoded B/C, then O/I.
    // One full prefix plus the next prefix stopped at the map before O/I
    // enters 4+3 solvers, not necessarily 4+3 boundary evaluations. Fixpoint
    // iterations and root depths are not inferred from the observed delta.
}
