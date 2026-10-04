use super::*;

#[test]
fn source_loop_preheaders_unit_local_noop_replays_original_erasure_and_fresh_root_reports() {
    for roots in [1, 2] {
        let (prefix, inherited) = prefix8(roots);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (prefix, promoted) = prefix
            .continue_private_cell_promotion_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(promoted.retained_storage()).unwrap();
        let original = prefix
            .prefix()
            .prefix()
            .prefix()
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes()
            .as_ptr();
        let erased = prefix
            .prefix()
            .prefix()
            .prefix()
            .erased()
            .canonical()
            .canonical_bytes()
            .as_ptr();
        let effects = external_effects(prefix.output().module());
        let floor = budget.storage();
        let (mut owner, added) = prefix.continue_loop_preheaders_v1(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(added.retained_storage()).unwrap();
        assert_eq!(
            original,
            owner
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes()
                .as_ptr()
        );
        assert_eq!(
            erased,
            owner
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .erased()
                .canonical()
                .canonical_bytes()
                .as_ptr()
        );
        assert!(
            owner.origins().is_empty()
                && owner.incoming_origins().is_empty()
                && owner.parameter_origins().is_empty()
        );
        assert_eq!(
            owner.output().canonical().canonical_bytes(),
            owner.prefix().output().canonical().canonical_bytes()
        );
        assert_eq!(external_effects(owner.output().module()), effects);
        assert_eq!(owner.kernels().len(), roots);
        assert!(!owner.grants_artifact_or_launch_authority());
        owner.verify_equivalence(&mut budget).unwrap();
        if roots == 2 {
            owner.exercise_source_preheader_report_order_v1(&mut budget);
        }
    }
}
