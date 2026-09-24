use super::*;

pub(super) fn prefix(profile: Profile, mutation: bool) -> (DirectPreheaders, usize) {
    prefix_from_source(source(false, mutation), profile, mutation)
}

pub(super) fn prefix_from_source(
    original: ProductionPreRankedKirOwnerV1,
    profile: Profile,
    mutation: bool,
) -> (DirectPreheaders, usize) {
    let Prepared {
        receipt,
        bound,
        output,
        source_storage,
        bound_storage,
        ..
    } = prepare(array_output_ranked_receipt_v1(original), profile, None);
    drop(output);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + source_storage + bound_storage)
        .unwrap();
    let fifth =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
            .unwrap();
    budget.reserve_storage(fifth.retained_storage()).unwrap();
    let sixth = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
        &bound,
        fifth,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(sixth.retained_storage()).unwrap();
    let prefix = crate::ProductionCheckedOutputOwnerPolicy6V1::try_admit_v1(
        receipt,
        bound,
        sixth,
        &mut budget,
    )
    .unwrap();
    let (prefix, added) = prefix
        .continue_redundant_private_stores_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    let (prefix, added) = prefix
        .continue_commutative_bitwise_cse_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    let (prefix, added) = prefix
        .continue_private_cell_promotion_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    let (prefix, added) = prefix.continue_loop_preheaders_v1(&mut budget).unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    if mutation {
        assert_eq!(prefix.origins().len(), 1);
        assert_eq!(prefix.incoming_origins().len(), 2);
        assert!(!prefix.parameter_origins().is_empty());
    }
    (prefix, budget.storage())
}

#[test]
fn source_licm_direct_replays_actual_motion_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let (prefix, inherited) = prefix(profile, true);
        let pointer = prefix.output().canonical().canonical_bytes().as_ptr();
        let origins = prefix.origins().to_vec();
        let incoming = prefix.incoming_origins().to_vec();
        let parameters = prefix.parameter_origins().to_vec();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, added) = prefix.continue_licm_v1(&mut budget).unwrap();
        assert_eq!(budget.storage(), inherited);
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
            pointer
        );
        assert_eq!(owner.prefix().origins(), origins);
        assert_eq!(owner.prefix().incoming_origins(), incoming);
        assert_eq!(owner.prefix().parameter_origins(), parameters);
        actual_mutation(
            owner.prefix().output(),
            owner.output(),
            owner.operation_origins(),
        );
        assert_eq!(owner.kernels().len(), 1);
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(budget.storage(), inherited + added.retained_storage());
        assert!(!owner.grants_artifact_or_launch_authority());
        owner.exercise_licm_source_join_v1(&mut budget);
    }
}

#[test]
fn source_licm_noops_are_fresh_exact_owners_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let (prefix, inherited) = prefix(profile, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, added) = prefix.continue_licm_v1(&mut budget).unwrap();
        budget.reserve_storage(added.retained_storage()).unwrap();
        assert_eq!(
            owner.output().canonical().canonical_bytes(),
            owner.prefix().output().canonical().canonical_bytes()
        );
        assert!(!std::ptr::eq(owner.output(), owner.prefix().output()));
        assert!(
            owner
                .operation_origins()
                .iter()
                .all(|row| row.hoist.is_none() && row.input == row.output)
        );
        owner.verify_equivalence(&mut budget).unwrap();
    }
}

#[test]
fn source_licm_rejects_prefix_receipts_and_final_report_tampering() {
    let (prefix, inherited) = prefix(Profile::Gfx942, true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (mut owner, added) = prefix.continue_licm_v1(&mut budget).unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    owner.exercise_licm_report_and_receipt_refusals_v1(&mut budget);
}

#[test]
fn source_licm_rejects_equal_bytes_foreign_final_inventory() {
    let (prefix, inherited) = prefix(Profile::Gfx942, true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (owner, added) = prefix.continue_licm_v1(&mut budget).unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    owner.exercise_licm_foreign_inventory_v1(&mut budget);
}
