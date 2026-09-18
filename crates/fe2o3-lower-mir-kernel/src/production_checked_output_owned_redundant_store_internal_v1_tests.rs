use super::*;

impl ProductionOwnedUnitLocalRedundantStoreContinuationV1 {
    pub(crate) fn exercise_old_i_report_refusal_v1(
        &mut self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        assert_ne!(self.kernels(), self.prefix().kernels());
        assert_eq!(self.kernels().len(), self.prefix().kernels().len());
        let floor = budget.storage();
        let rows = std::mem::size_of_val(self.prefix().kernels());
        // Both fresh J and hostile copied I rows coexist in this test only.
        budget.reserve_storage(rows * 2).unwrap();
        let old_i = self.prefix().kernels().to_vec().into_boxed_slice();
        let fresh = std::mem::replace(&mut self.data.kernels, old_i);
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(StoreError::Admission(
                crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                )
            ))
        ));
        let stale = std::mem::replace(&mut self.data.kernels, fresh);
        drop(stale);
        budget.release_storage(rows * 2).unwrap();
        assert_eq!(budget.storage(), floor);
        self.verify_equivalence(budget).unwrap();
    }
}

#[test]
fn owning_wrapper_headers_cover_only_new_fields_and_branch_padding() {
    let direct = header::<
        ProductionCheckedOutputOwnerPolicy6V1,
        ProductionOwnedRedundantStoreContinuationV1,
    >()
    .unwrap();
    let erased = header::<
        ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1,
        ProductionOwnedUnitLocalRedundantStoreContinuationV1,
    >()
    .unwrap();
    let fields =
        std::mem::size_of::<Box<[FormalMemoryObligations]>>() + std::mem::size_of::<usize>();
    assert!(direct >= fields);
    assert!(erased >= fields);
    assert_eq!(
        direct
            + std::mem::size_of::<ProductionCheckedOutputOwnerPolicy6V1>()
            + std::mem::size_of::<OwnedRedundantStoreContinuationV1>(),
        std::mem::size_of::<ProductionOwnedRedundantStoreContinuationV1>()
    );
    assert_eq!(
        erased
            + std::mem::size_of::<ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1>()
            + std::mem::size_of::<OwnedRedundantStoreContinuationV1>(),
        std::mem::size_of::<ProductionOwnedUnitLocalRedundantStoreContinuationV1>()
    );
}
