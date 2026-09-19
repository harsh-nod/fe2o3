// Included beside existing private session lifecycle helpers. No old pass tag.
impl PlironSession {
    pub(crate) fn execute_commutative_owner_v1(
        &mut self,
        root: &OperationHandle,
        capture: &crate::kir_occurrence_capture_v1::CommutativeCapture,
        ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
    ) -> Result<crate::CommutativeBitwiseExecutionV1, crate::commutative_cse_owner_v1::Error> {
        use crate::commutative_cse_owner_v1::Error as E;
        const GRAPH_CAP: usize = 32_768;
        const WORK_CAP: usize = 25_268_224;
        let pointer = self
            .with_operation(root, |pointer, _| pointer)
            .map_err(|e| E::Execution(PlironOptimizationErrorV1::Operation(e)))?;
        if self.operation_roots.get(&root.identity).copied() != Some(root.identity) {
            return Err(E::Execution(PlironOptimizationErrorV1::RootHandleRequired));
        }
        let charged = *self
            .owned_tree_work
            .get(&root.identity)
            .ok_or(E::Execution(
                PlironOptimizationErrorV1::GraphAccountingMismatch,
            ))?;
        let handles = self
            .operation_roots
            .values()
            .filter(|owner| **owner == root.identity)
            .count();
        enforce_graph_limit(charged, GRAPH_CAP).map_err(E::Execution)?;
        self.operation_tree_work
            .checked_sub(charged)
            .and_then(|n| n.checked_add(GRAPH_CAP))
            .filter(|n| *n <= HARD_MAX_SESSION_OPERATION_TREE_ITEMS)
            .ok_or(E::Execution(
                PlironOptimizationErrorV1::SessionGraphCapacityExceeded,
            ))?;
        let work =
            optimization_work_preflight(charged, 1, GRAPH_CAP, handles).map_err(E::Execution)?;
        if work > WORK_CAP {
            return Err(E::Execution(PlironOptimizationErrorV1::WorkLimitExceeded {
                required: work,
                limit: WORK_CAP,
            }));
        }
        let (input_work, _) = self
            .inspect_optimization_graph(pointer, None)
            .map_err(E::Execution)?;
        if input_work != charged {
            self.poisoned = true;
            return Err(E::Execution(
                PlironOptimizationErrorV1::GraphAccountingMismatch,
            ));
        }
        self.verify_optimization_graph(pointer, None)
            .map_err(E::Execution)?;
        let before = self
            .analyze_operation_graph_v1(root)
            .map_err(|e| E::Execution(PlironOptimizationErrorV1::Operation(e)))?
            .replay_identity();
        let snapshot = self
            .operation_graph_snapshot_v1(root)
            .map_err(|e| E::Execution(PlironOptimizationErrorV1::Operation(e)))?;
        if !capture.begin(snapshot.epoch()) {
            self.poisoned = true;
            return Err(E::Capture(
                capture
                    .failure()
                    .unwrap_or(crate::KirOptimizationMapErrorV12::Lifecycle),
            ));
        }
        let transaction = self
            .begin_checked_operation_graph_mutation_v1(root)
            .map_err(|e| E::Execution(PlironOptimizationErrorV1::Operation(e)))?;
        let mut analyses = AnalysisManager::default();
        // An unwind deliberately propagates to the outer owning cleanup scope;
        // its payload cannot be dropped while graph/capture reservations are live.
        let raw = run_observed_commutative_owner_v1(
            pointer,
            &mut self.context,
            &mut analyses,
            capture.observer(),
            ledger,
        );
        if let Some(error) = ledger.failure() {
            self.poisoned = true;
            return Err(E::Resource(error));
        }
        if let Some(error) = capture.failure() {
            self.poisoned = true;
            return Err(E::Capture(error));
        }
        let changed = match raw {
            Ok(changed) => changed,
            Err(_) => {
                self.poisoned = true;
                return Err(E::PassRejected);
            }
        };
        let (output_work, operations) = self
            .inspect_optimization_graph(pointer, None)
            .map_err(E::Execution)?;
        enforce_graph_limit_after_mutation(self, output_work, GRAPH_CAP).map_err(E::Execution)?;
        if output_work > input_work {
            self.poisoned = true;
            return Err(E::Capture(
                crate::KirOptimizationMapErrorV12::UnsupportedMutation,
            ));
        }
        self.verify_optimization_graph(pointer, None)
            .map_err(E::Execution)?;
        let commit = self
            .commit_checked_operation_graph_mutation_v1(transaction, changed)
            .map_err(|e| E::Execution(PlironOptimizationErrorV1::Operation(e)))?;
        if !capture.end(&self.context, commit.snapshot().epoch()) {
            self.poisoned = true;
            return Err(E::Capture(
                capture
                    .failure()
                    .unwrap_or(crate::KirOptimizationMapErrorV12::Lifecycle),
            ));
        }
        self.reconcile_optimized_root(root, pointer, charged, output_work, &operations)
            .map_err(E::Execution)?;
        let after = self
            .analyze_operation_graph_v1(root)
            .map_err(|e| E::Execution(PlironOptimizationErrorV1::Operation(e)))?
            .replay_identity();
        Ok(crate::CommutativeBitwiseExecutionV1 {
            before,
            after,
            changed,
            input_work,
            output_work,
            dynamic_work: 0,
        })
    }
}

fn run_observed_commutative_owner_v1(
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
    struct Observed<'a, 'b, 'w> {
        ledger: &'a mut crate::fixed_policy_v3::CseLedger<'b, 'w>,
        observer: Option<Box<dyn pliron::irbuild::observer::RewriteObserver>>,
    }
    impl Pass for Observed<'_, '_, '_> {
        fn name(&self) -> &str {
            "gpu-commutative-bitwise-dominance-cse-v1"
        }
        fn run(
            &mut self,
            root: Ptr<Operation>,
            context: &mut pliron::context::Context,
            analyses: &mut AnalysisManager,
        ) -> pliron::result::Result<PassResult> {
            let mut dominance = analyses.get_analysis_mut::<DomInfo>(root, context)?;
            let observer = self
                .observer
                .take()
                .expect("one closed commutative invocation");
            let changed = dialect_gpu::commutative_bitwise_cse_v1::commutative_bitwise_dominance_cse_with_observer_v1(
                root, context, &mut dominance, self.ledger, observer,
            ).map_err(|error| {
                let error = self.ledger.record_core_error(error);
                pliron::input_error_noloc!(error)
            })?;
            self.ledger
                .finish()
                .map_err(|e| pliron::input_error_noloc!(e))?;
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
