impl PlironSession {
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
    use pliron::{
        graph::dominance::DomInfo,
        pass::{PassManager, PassResult},
    };
    struct Observed<'a, 'budget, 'work> {
        ledger: &'a mut crate::fixed_policy_v3::CseLedger<'budget, 'work>,
        observer: Option<Box<dyn pliron::irbuild::observer::RewriteObserver>>,
    }
    impl Pass for Observed<'_, '_, '_> {
        fn name(&self) -> &str {
            "gpu-integer-neutral-v1"
        }
        fn run(
            &mut self,
            root: Ptr<Operation>,
            context: &mut pliron::context::Context,
            _analyses: &mut AnalysisManager,
        ) -> pliron::result::Result<PassResult> {
            let observer = self.observer.take().expect("one fixed integer invocation");
            let changed = dialect_gpu::integer_identity_v1::integer_identity_canonicalization_with_observer_v1(
                root, context, self.ledger, observer,
            ).map_err(|error| {
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
        &mut Observed {
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
