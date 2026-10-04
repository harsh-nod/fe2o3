use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

struct FailedCandidate<'a> {
    data: Option<PreheaderData>,
    dropped: &'a Cell<bool>,
}
impl Drop for FailedCandidate<'_> {
    fn drop(&mut self) {
        let data = self.data.take().unwrap();
        assert!(
            !data.origins.blocks().is_empty(),
            "actual admitted source mutation candidate"
        );
        assert_eq!(
            data.kernels.len(),
            data.tail.output().module().kernels.len()
        );
        drop(data);
        self.dropped.set(true);
    }
}

impl ProductionOwnedPrivateCellPromotionContinuationV1 {
    pub(crate) fn exercise_source_preheader_failed_candidate_v1(&self, inherited: usize) {
        let run = |panic: bool| {
            let sibling = vec![0xa5_u8; 113];
            let floor = inherited + sibling.capacity();
            let dropped = Cell::new(false);
            let mut work = Work::new(1_000_000_000);
            let measurements = {
                let mut budget = AssertOriginBudgetV1::new(&mut work, 512 << 20);
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(17).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result: HResult<()> = preheader_scoped(
                    self.retained_input_storage_floor_v1().unwrap(),
                    &mut budget,
                    |budget, binding| {
                        let data = prepare_preheader_data(
                            PromotedPrefix::Direct(self),
                            preheader_header::<Self, ProductionOwnedLoopPreheadersContinuationV1>(
                            )?,
                            budget,
                            binding,
                        )?;
                        let _partial = FailedCandidate {
                            data: Some(data),
                            dropped: &dropped,
                        };
                        if panic {
                            std::panic::panic_any(718_u32);
                        }
                        Err(HError::Origins("injected after actual admitted candidate"))
                    },
                );
                if panic {
                    assert!(matches!(result, Err(HError::Panicked)));
                } else {
                    assert!(matches!(
                        result,
                        Err(HError::Origins("injected after actual admitted candidate"))
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

impl ProductionOwnedUnitLocalLoopPreheadersContinuationV1 {
    pub(crate) fn exercise_source_preheader_report_order_v1(
        &mut self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        assert_eq!(self.data.kernels.len(), 2);
        let floor = budget.storage();
        self.data.kernels.swap(0, 1);
        match self.verify_equivalence(budget) {
            Err(HError::Admission(error)) => assert!(matches!(
                *error,
                PError::Admission(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                ))
            )),
            other => panic!("exact fresh report-order refusal, got {other:?}"),
        }
        assert_eq!(budget.storage(), floor);
        self.data.kernels.swap(0, 1);
        self.verify_equivalence(budget).unwrap();
    }
}

impl ProductionOwnedLoopPreheadersContinuationV1 {
    pub(crate) fn exercise_source_preheader_foreign_inventory_v1(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        let floor = budget.storage();
        preheader_scoped(
            self.retained_input_storage_floor_v1().unwrap(),
            budget,
            |budget, binding| {
                let (pair, receipt) = self
                    .data
                    .tail
                    .replay_against(self.prefix.output(), budget)
                    .map_err(HError::Continuation)?;
                budget.reserve_storage(receipt.retained_storage())?;
                let (foreign, receipt) = StoreOwner::from_module_ref_with_verification_budget_v12(
                    self.output().module(),
                    budget,
                )
                .map_err(fe2o3_kernel_opt::OwnedLoopPreheadersErrorV1::Admission)
                .map_err(HError::Continuation)?;
                budget.reserve_storage(receipt.retained_storage())?;
                assert_eq!(
                    foreign.canonical().canonical_bytes(),
                    self.output().canonical().canonical_bytes()
                );
                let (foreign, receipt) =
                    Inventory::derive(&foreign, budget).map_err(inventory_error)?;
                budget.reserve_storage(receipt.retained_storage())?;
                let result = with_promoted_output_sites(
                    PromotedPrefix::Direct(&self.prefix).p8(),
                    self.prefix.continuation(),
                    budget,
                    binding,
                    |sites, budget, inner| {
                        promotion_sites::check_loop_preheader_sites(
                            sites, &pair, &foreign, budget, inner,
                        )
                    },
                );
                assert!(matches!(
                    result,
                    Err(PError::Admission(E::Unsupported {
                        phase: "neutral loop preheaders",
                        detail: "exact pair/source/output custody",
                    }))
                ));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}
