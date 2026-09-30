impl PlironSession {
    fn fixedpoint_presentation_v18(
        &mut self,
        root: Ptr<Operation>,
        limit: usize,
        ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
    ) -> Result<Vec<u8>, PlironOptimizationErrorV1> {
        use pliron::printable::Printable;
        use std::fmt::Write as _;
        struct Bounded<'a, 'budget, 'work> {
            ledger: &'a mut crate::fixed_policy_v3::CseLedger<'budget, 'work>,
            bytes: Option<&'a mut Vec<u8>>,
            length: usize,
            limit: usize,
        }
        impl fmt::Write for Bounded<'_, '_, '_> {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                self.ledger
                    .admit_fixedpoint_round(value.len().checked_add(1).ok_or(fmt::Error)?, 0)
                    .map_err(|_| fmt::Error)?;
                let length = self
                    .length
                    .checked_add(value.len())
                    .filter(|n| *n <= self.limit)
                    .ok_or(fmt::Error)?;
                if let Some(bytes) = &mut self.bytes {
                    if length > bytes.capacity() {
                        return Err(fmt::Error);
                    }
                    bytes.extend_from_slice(value.as_bytes());
                }
                self.length = length;
                Ok(())
            }
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut count = Bounded {
                ledger: &mut *ledger,
                bytes: None,
                length: 0,
                limit,
            };
            write!(&mut count, "{}", root.disp(&self.context))?;
            let length = count.length;
            ledger
                .admit_fixedpoint_round(0, length)
                .map_err(|_| fmt::Error)?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(length).map_err(|_| {
                ledger.record_core_error(
                    dialect_gpu::dominance_cse_v1::DominanceCseErrorV1::Allocation,
                );
                fmt::Error
            })?;
            ledger
                .admit_fixedpoint_round(0, bytes.capacity().checked_sub(length).ok_or(fmt::Error)?)
                .map_err(|_| fmt::Error)?;
            let mut output = Bounded {
                ledger: &mut *ledger,
                bytes: Some(&mut bytes),
                length: 0,
                limit: length,
            };
            write!(&mut output, "{}", root.disp(&self.context))?;
            if output.length != length {
                return Err(fmt::Error);
            }
            Ok::<_, fmt::Error>(bytes)
        }));
        match result {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(_)) => {
                self.poisoned = true;
                Err(PlironOptimizationErrorV1::GraphAccountingMismatch)
            }
            Err(_) => {
                self.poisoned = true;
                Err(PlironOptimizationErrorV1::UpstreamPanicked { during: None })
            }
        }
    }

    pub(crate) fn execute_fixed_mixed_fixedpoint_v18(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
        capture: &crate::kir_optimization_map_v12::CaptureV12,
        occurrences: &crate::kir_occurrence_capture_v1::Capture,
        ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
        rounds: crate::fixed_policy_v3::FixedpointRoundResourcesV18,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        if plan.passes.as_slice() != crate::fixed_policy_v3::POLICY11_PASSES {
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        }
        self.execute_optimization_impl_v1(
            root,
            plan,
            Some(capture),
            Some(occurrences),
            Some(ledger),
            Some(rounds),
        )
    }

    pub(crate) fn execute_fixed_mixed_pure_cse_v18(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
        capture: &crate::kir_optimization_map_v12::CaptureV12,
        occurrences: &crate::kir_occurrence_capture_v1::Capture,
        ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        if plan.passes.as_slice() != crate::fixed_policy_v3::POLICY10_PASSES {
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        }
        self.execute_optimization_impl_v1(
            root,
            plan,
            Some(capture),
            Some(occurrences),
            Some(ledger),
            None,
        )
    }

    pub(crate) fn execute_fixed_integer_worklist_v18(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
        capture: &crate::kir_optimization_map_v12::CaptureV12,
        occurrences: &crate::kir_occurrence_capture_v1::Capture,
        ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        if plan.passes.as_slice() != crate::fixed_policy_v3::POLICY9_PASSES {
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        }
        self.execute_optimization_impl_v1(
            root,
            plan,
            Some(capture),
            Some(occurrences),
            Some(ledger),
            None,
        )
    }

    pub(crate) fn execute_fixed_integer_continuation_v1(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
        capture: &crate::kir_optimization_map_v12::CaptureV12,
        occurrences: &crate::kir_occurrence_capture_v1::Capture,
        ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        if plan.passes.as_slice()
            != crate::fixed_integer_continuation_v1::INTEGER_CONTINUATION_PASSES
        {
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        }
        self.execute_optimization_impl_v1(
            root,
            plan,
            Some(capture),
            Some(occurrences),
            Some(ledger),
            None,
        )
    }
}

fn run_observed_integer_identity_v1(
    pointer: Ptr<Operation>,
    context: &mut pliron::context::Context,
    analyses: &mut AnalysisManager,
    observer: Box<dyn pliron::irbuild::observer::RewriteObserver>,
    ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
) -> Result<bool, TrustedPassFailure> {
    run_observed_integer_identity::<false>(pointer, context, analyses, observer, ledger)
}

fn run_observed_integer_identity<const WORKLIST: bool>(
    pointer: Ptr<Operation>,
    context: &mut pliron::context::Context,
    analyses: &mut AnalysisManager,
    observer: Box<dyn pliron::irbuild::observer::RewriteObserver>,
    ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
) -> Result<bool, TrustedPassFailure> {
    use pliron::{
        graph::dominance::DomInfo,
        pass::{PassManager, PassResult},
    };
    struct Observed<'a, 'budget, 'work, const WORKLIST: bool> {
        ledger: &'a mut crate::fixed_policy_v3::CseLedger<'budget, 'work>,
        observer: Option<Box<dyn pliron::irbuild::observer::RewriteObserver>>,
    }
    impl<const WORKLIST: bool> Pass for Observed<'_, '_, '_, WORKLIST> {
        fn name(&self) -> &str {
            if WORKLIST {
                "gpu-integer-neutral-worklist-v2"
            } else {
                "gpu-integer-neutral-v1"
            }
        }
        fn run(
            &mut self,
            root: Ptr<Operation>,
            context: &mut pliron::context::Context,
            _analyses: &mut AnalysisManager,
        ) -> pliron::result::Result<PassResult> {
            let observer = self.observer.take().expect("one fixed integer invocation");
            let changed = if WORKLIST {
                dialect_gpu::integer_identity_v2::integer_identity_canonicalization_with_observer_v2(
                    root, context, self.ledger, observer,
                )
            } else {
                dialect_gpu::integer_identity_v1::integer_identity_canonicalization_with_observer_v1(
                    root, context, self.ledger, observer,
                )
            }.map_err(|error| {
                let error = self.ledger.record_integer_error(error);
                pliron::input_error_noloc!(error)
            })?;
            self.ledger
                .finish()
                .map_err(|error| pliron::input_error_noloc!(error))?;
            let mut result = PassResult::default();
            result.ir_changed = changed;
            result.set_preserved::<DomInfo>();
            Ok(result)
        }
    }
    let result = <Passes as PassManager>::run_pass(
        &mut Observed::<WORKLIST> {
            ledger,
            observer: Some(observer),
        },
        pointer,
        context,
        analyses,
    )
    .map_err(|_| TrustedPassFailure)?;
    analyses.retain_preserved(&result);
    Ok(result.ir_changed == IRStatus::Changed)
}
