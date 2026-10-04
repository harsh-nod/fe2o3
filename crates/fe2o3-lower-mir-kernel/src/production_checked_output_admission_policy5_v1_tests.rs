// Nested in genuine direct private-memory fixtures; no fabricated source owner.
use super::*;
type Final5 = crate::ProductionCheckedOutputOwnerPolicy5V1;

#[test]
fn direct_policy5_preserves_complete_source_and_existing_store_forwarding() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let Prepared {
            receipt,
            bound,
            output,
            source_storage,
            bound_storage,
            ..
        } = prepare(
            array_output_ranked_receipt_v1(retained_scalar_source_with_reads(3)),
            profile,
            None,
        );
        drop(output);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget
            .reserve_storage(FLOOR + source_storage + bound_storage)
            .unwrap();
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
                .unwrap();
        assert_eq!(checked.intermediate_policy4().forwarding_rows().len(), 3);
        assert!(checked.load_forwarding_rows().is_empty());
        budget.reserve_storage(checked.retained_storage()).unwrap();
        let floor = budget.storage();
        let owner = Final5::try_admit_v1(receipt, bound, checked, &mut budget).unwrap();
        assert_eq!(
            private_counts(owner.source_semantic_kir().module()),
            (1, 1, 3)
        );
        assert_eq!(private_counts(owner.output().module()), (1, 1, 0));
        assert!(!owner.grants_artifact_or_launch_authority());
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}
