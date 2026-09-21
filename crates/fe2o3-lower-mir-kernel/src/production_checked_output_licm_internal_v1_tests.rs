use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

fn exercise_join(
    prefix: PreheaderPrefix<'_>,
    data: &LicmData,
    budget: &mut AssertOriginBudgetV1<'_>,
) {
    let floor = budget.storage();
    licm_scoped(
        prefix.floor().unwrap() + data.added,
        budget,
        |budget, binding| {
            prefix.replay(budget)?;
            let (licm, storage) = data
                .tail
                .replay_against(prefix.output(), budget)
                .map_err(LError::Continuation)?;
            budget.reserve_storage(storage.retained_storage())?;
            let promoted = prefix.promoted();
            let (preheaders, storage) = prefix
                .tail()
                .replay_against(promoted.output(), budget)
                .map_err(|e| LError::Prefix(Box::new(HError::Continuation(e))))?;
            budget.reserve_storage(storage.retained_storage())?;
            let (input, storage) = Inventory::derive(prefix.output(), budget)
                .map_err(inventory_error)
                .map_err(PError::from)?;
            budget.reserve_storage(storage.retained_storage())?;
            let (output, storage) = Inventory::derive(data.tail.output(), budget)
                .map_err(inventory_error)
                .map_err(PError::from)?;
            budget.reserve_storage(storage.retained_storage())?;
            with_promoted_output_sites(
                promoted.p8(),
                promoted.tail(),
                budget,
                binding,
                |sites, budget, inner| {
                    promotion_sites::exercise_licm_source_sites(
                        sites,
                        &preheaders,
                        &licm,
                        &input,
                        &output,
                        budget,
                        inner,
                    )
                },
            )?;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

impl ProductionOwnedLicmContinuationV1 {
    pub(crate) fn exercise_licm_source_join_v1(&self, budget: &mut AssertOriginBudgetV1<'_>) {
        exercise_join(PreheaderPrefix::Direct(&self.prefix), &self.data, budget);
    }

    pub(crate) fn exercise_licm_report_and_receipt_refusals_v1(
        &mut self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        let floor = budget.storage();
        let kernels = std::mem::take(&mut self.data.kernels);
        match self.verify_equivalence(budget) {
            Err(LError::Resource(AssertOriginResourceV1::Accounting)) => (),
            other => {
                panic!("changed report backing must first invalidate its owning receipt: {other:?}")
            }
        }
        self.data.added -= std::mem::size_of_val(kernels.as_ref());
        match self.verify_equivalence(budget) {
            Err(LError::Admission(error)) => assert!(matches!(
                *error,
                PError::Admission(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                ))
            )),
            other => panic!("exact fresh final census refusal: {other:?}"),
        }
        self.data.added += std::mem::size_of_val(kernels.as_ref());
        self.data.kernels = kernels;
        self.data.added += 1;
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(LError::Resource(AssertOriginResourceV1::Accounting))
        ));
        self.data.added -= 1;
        // Retained prefix accounting is replayed before preparing another tail.
        self.prefix.data.added += 1;
        match self.verify_equivalence(budget) {
            Err(LError::Prefix(error)) => assert!(matches!(
                *error,
                HError::Resource(AssertOriginResourceV1::Accounting)
            )),
            other => panic!("exact retained prefix accounting refusal: {other:?}"),
        }
        self.prefix.data.added -= 1;
        self.verify_equivalence(budget).unwrap();
        assert_eq!(budget.storage(), floor);
    }

    pub(crate) fn exercise_licm_foreign_inventory_v1(&self, budget: &mut AssertOriginBudgetV1<'_>) {
        let floor = budget.storage();
        licm_scoped(
            self.retained_input_storage_floor_v1().unwrap(),
            budget,
            |budget, binding| {
                let prefix = PreheaderPrefix::Direct(&self.prefix);
                prefix.replay(budget)?;
                let (licm, storage) = self
                    .data
                    .tail
                    .replay_against(prefix.output(), budget)
                    .map_err(LError::Continuation)?;
                budget.reserve_storage(storage.retained_storage())?;
                let promoted = prefix.promoted();
                let (preheaders, storage) = prefix
                    .tail()
                    .replay_against(promoted.output(), budget)
                    .map_err(|e| LError::Prefix(Box::new(HError::Continuation(e))))?;
                budget.reserve_storage(storage.retained_storage())?;
                let (input, storage) = Inventory::derive(prefix.output(), budget)
                    .map_err(inventory_error)
                    .map_err(PError::from)?;
                budget.reserve_storage(storage.retained_storage())?;
                let (foreign, storage) = StoreOwner::from_module_ref_with_verification_budget_v12(
                    self.output().module(),
                    budget,
                )
                .map_err(fe2o3_kernel_opt::OwnedLicmErrorV1::Admission)
                .map_err(LError::Continuation)?;
                budget.reserve_storage(storage.retained_storage())?;
                assert_eq!(
                    foreign.canonical().canonical_bytes(),
                    self.output().canonical().canonical_bytes()
                );
                let (foreign, storage) = Inventory::derive(&foreign, budget)
                    .map_err(inventory_error)
                    .map_err(PError::from)?;
                budget.reserve_storage(storage.retained_storage())?;
                let result = with_promoted_output_sites(
                    promoted.p8(),
                    promoted.tail(),
                    budget,
                    binding,
                    |sites, budget, inner| {
                        promotion_sites::check_licm_after_preheaders_sites(
                            sites,
                            &preheaders,
                            &licm,
                            &input,
                            &foreign,
                            budget,
                            inner,
                        )
                    },
                );
                assert!(matches!(
                    result,
                    Err(PError::Admission(E::Unsupported {
                        phase: "total-integer LICM",
                        detail: "connected actual owners and complete sites"
                    }))
                ));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

impl ProductionOwnedUnitLocalLicmContinuationV1 {
    pub(crate) fn exercise_licm_source_join_v1(&self, budget: &mut AssertOriginBudgetV1<'_>) {
        exercise_join(PreheaderPrefix::Erased(&self.prefix), &self.data, budget);
    }

    pub(crate) fn exercise_licm_report_order_v1(&mut self, budget: &mut AssertOriginBudgetV1<'_>) {
        assert_eq!(self.data.kernels.len(), 2);
        let floor = budget.storage();
        self.data.kernels.swap(0, 1);
        match self.verify_equivalence(budget) {
            Err(LError::Admission(error)) => assert!(matches!(
                *error,
                PError::Admission(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                ))
            )),
            other => panic!("exact fresh final report-order refusal: {other:?}"),
        }
        assert_eq!(budget.storage(), floor);
        self.data.kernels.swap(0, 1);
        self.verify_equivalence(budget).unwrap();
    }
}

struct FailedCandidate<'a> {
    data: Option<LicmData>,
    dropped: &'a Cell<bool>,
}
impl Drop for FailedCandidate<'_> {
    fn drop(&mut self) {
        let data = self.data.take().unwrap();
        assert!(data.tail.origins().iter().any(|row| row.hoist.is_some()));
        assert_eq!(
            data.kernels.len(),
            data.tail.output().module().kernels.len()
        );
        drop(data);
        self.dropped.set(true);
    }
}
impl ProductionOwnedLoopPreheadersContinuationV1 {
    pub(crate) fn exercise_licm_failed_candidate_v1(&self, inherited: usize) {
        let run = |panic: bool| {
            let sibling = vec![0xa5_u8; 113];
            let floor = inherited + std::mem::size_of_val(&sibling) + sibling.capacity();
            let dropped = Cell::new(false);
            let mut work = Work::new(1_000_000_000);
            let measurements = {
                let mut budget = AssertOriginBudgetV1::new(&mut work, 512 << 20);
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(17).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result: LResult<()> = licm_scoped(
                    self.retained_input_storage_floor_v1().unwrap(),
                    &mut budget,
                    |budget, binding| {
                        let data = prepare_licm_data(
                            PreheaderPrefix::Direct(self),
                            licm_header::<Self, ProductionOwnedLicmContinuationV1>()?,
                            budget,
                            binding,
                        )?;
                        let _partial = FailedCandidate {
                            data: Some(data),
                            dropped: &dropped,
                        };
                        if panic {
                            std::panic::panic_any(761_u32);
                        }
                        Err(AssertOriginResourceV1::Accounting.into())
                    },
                );
                if panic {
                    assert!(matches!(result, Err(LError::Panicked)));
                } else {
                    assert!(matches!(
                        result,
                        Err(LError::Resource(AssertOriginResourceV1::Accounting))
                    ));
                }
                assert!(dropped.get());
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(budget.failed_storage(), None);
                assert!(sibling.iter().all(|byte| *byte == 0xa5));
                (budget.work(), budget.peak_storage())
            };
            assert_eq!(work.failed_work(), None);
            measurements
        };
        assert_eq!(run(false), run(true));
    }
}
