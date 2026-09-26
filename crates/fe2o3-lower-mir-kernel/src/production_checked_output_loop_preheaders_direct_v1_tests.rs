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
                (result, accepted, peak, denied_storage, work.failed_work())
            };
            let (result, measured_work, measured_storage, denied_storage, denied_work) =
                run(WORK, STORAGE);
            result.unwrap();
            assert_eq!((denied_storage, denied_work), (None, None));
            let (result, accepted, peak, denied_storage, denied_work) =
                run(measured_work, measured_storage);
            result.unwrap();
            assert_eq!(
                (accepted, peak, denied_storage, denied_work),
                (measured_work, measured_storage, None, None)
            );
            let storage_limit = measured_storage.checked_sub(1).unwrap();
            let (result, accepted, peak, denied_storage, denied_work) =
                run(measured_work, storage_limit);
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
            // Both targets use the same canonical fixture and exact replay schedule.
            // Add the actual inline optional nominal-helper header to the
            // historical V20 baseline for this one retained source owner.
            // Work and the exact nested first-denial phase stay unchanged.
            let nominal_header = std::mem::size_of::<Option<Box<SealedBf16CallRelationV1>>>();
            let expected = if mutation {
                (845_738, 2_685_874 + nominal_header)
            } else {
                (65_552, 1_655_457 + nominal_header)
            };
            assert_eq!((accepted, peak), expected);
        }
    }
}
