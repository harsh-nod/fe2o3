// Constructed genuine semantic source; no ordinary Rust or native-artifact claim.
use super::*;

fn prefix(
    profile: Profile,
    kill: Option<SemanticStatementKindV1>,
    stores: usize,
) -> (Final6, usize) {
    let input = fixture6_from_source(profile, source(kill, stores));
    let floor = input.floor;
    (admit6(input, WORK, STORAGE).0.unwrap(), floor)
}

#[test]
fn consuming_direct_preserves_source_allocations_and_replays_genuine_j_and_noop_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for stores in [1, 4] {
            let (prefix, inherited) = prefix(profile, None, stores);
            let allocations = (
                prefix
                    .source_semantic_kir()
                    .pre_ranked_executable()
                    .unwrap()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                prefix.bound().canonical().canonical_bytes().as_ptr(),
                prefix.output().canonical().canonical_bytes().as_ptr(),
            );
            let inherited_minimum = prefix.retained_input_storage_floor_v1().unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            let floor = inherited + 127;
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let (owner, receipt) = prefix
                .continue_redundant_private_stores_v1(&mut budget)
                .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                receipt.retained_storage(),
                owner.additional_retained_storage_v1()
            );
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            owner.assert_independent_added_receipt_v1(receipt);
            assert_eq!(
                owner.retained_input_storage_floor_v1().unwrap(),
                inherited_minimum + receipt.retained_storage()
            );
            assert_eq!(
                allocations,
                (
                    owner
                        .prefix()
                        .source_semantic_kir()
                        .pre_ranked_executable()
                        .unwrap()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    owner
                        .prefix()
                        .bound()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    owner
                        .prefix()
                        .output()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                )
            );
            assert_eq!(owner.continuation().rows().len(), stores - 1);
            assert_eq!(private_counts(owner.prefix().output().module()).1, stores);
            assert_eq!(private_counts(owner.output().module()).1, 1);
            assert_eq!(
                owner.prefix().output().canonical().canonical_bytes()
                    != owner.output().canonical().canonical_bytes(),
                stores > 1
            );
            assert_eq!(owner.kernels().len(), owner.output().module().kernels.len());
            assert!(!owner.grants_artifact_or_launch_authority());
            owner.verify_equivalence(&mut budget).unwrap();
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(budget.peak_storage() > floor + receipt.retained_storage());
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn consuming_direct_lifetime_restart_refuses_after_real_candidate_without_releasing_inherited_floor()
 {
    let (prefix, inherited) = prefix(
        Profile::Gfx942,
        Some(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(5),
        )),
        2,
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = inherited + 59;
    budget.reserve_storage(floor).unwrap();
    // First demonstrate that the actual I has a genuine local deletion. This
    // independent observation grants no source lifetime admission.
    let candidate = fe2o3_kernel_opt::prepare_owned_redundant_store_continuation_v1(
        prefix.output(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(candidate.rows().len(), 1);
    assert_ne!(
        candidate.output().canonical().canonical_bytes(),
        prefix.output().canonical().canonical_bytes()
    );
    drop(candidate);
    let result = prefix.continue_redundant_private_stores_v1(&mut budget);
    assert!(matches!(
        result,
        Err(StoreError::Admission(
            crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::Unsupported {
                phase: "redundant Store source",
                detail: "no lifetime or Move invalidation between identical Stores",
            }
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn consuming_direct_exact_and_one_short_construction_budgets_keep_the_complete_entry_floor() {
    let run = |work_limit, storage_limit| {
        let (prefix, inherited) = prefix(Profile::Gfx942, None, 3);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        let floor = inherited + 113;
        budget.reserve_storage(floor).unwrap();
        let result = prefix.continue_redundant_private_stores_v1(&mut budget);
        let success = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        (success, budget.work(), budget.peak_storage())
    };
    let (success, work, peak) = run(WORK, STORAGE);
    assert!(success);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    assert!(!run(0, peak).0);
}

#[test]
fn consuming_direct_replay_exact_budgets_and_underfloor_reject_without_new_work() {
    let (prefix, inherited) = prefix(Profile::Gfx942, None, 3);
    let mut initial_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut initial = AssertOriginBudgetV1::new(&mut initial_work, STORAGE);
    initial.reserve_storage(inherited).unwrap();
    let (owner, receipt) = prefix
        .continue_redundant_private_stores_v1(&mut initial)
        .unwrap();
    let floor = inherited + receipt.retained_storage() + 37;
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = owner.verify_equivalence(&mut budget);
        assert_eq!(budget.storage(), floor);
        (result.is_ok(), budget.work(), budget.peak_storage())
    };
    let (success, work, peak) = run(WORK, STORAGE);
    assert!(success);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    let minimum = owner.retained_input_storage_floor_v1().unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut short = AssertOriginBudgetV1::new(&mut work, STORAGE);
    short.reserve_storage(minimum - 1).unwrap();
    assert!(matches!(
        owner.verify_equivalence(&mut short),
        Err(StoreError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(short.work(), 0);
    assert_eq!(short.storage(), minimum - 1);
}
