//! Scripted outcomes use no native receipt constructors or GPU execution.

#![cfg(test)]

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScriptedCompletionStepV1 {
    PollPending,
    PollReady,
    PollError,
    PollUnwind,
    PollOuterErrorPending,
    PollOuterUnwindPending,
    PollOuterErrorReady,
    PollOuterUnwindReady,
    RecycleRetry,
    RecycleReady,
    RecycleError,
    RecycleUnwind,
    RecycleOuterErrorRetry,
    RecycleOuterUnwindRetry,
    RecycleOuterErrorReady,
    RecycleOuterUnwindReady,
    ProfileUnwind,
}

impl KfdRuntimeBackendV1 {
    pub(super) fn observe_scripted_materialized_v1(
        &mut self,
        target: MaterializedCompletionTargetV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        use ScriptedCompletionStepV1::*;
        for operation in [MaterializedConsumeV1::Poll, MaterializedConsumeV1::Recycle] {
            let current = execution_phase(target.owner(self).unwrap().execution.as_ref());
            if operation == MaterializedConsumeV1::Poll
                && current == Some(RuntimeComputePipelinePhaseV1::Completed)
            {
                continue;
            }
            let id = target.owner(self).unwrap().id;
            let Some((expected_id, step)) = self
                .scripted_materialized_completion
                .as_mut()
                .and_then(|steps| steps.pop_front())
            else {
                return Err(
                    self.terminal_error("scripted ordinary completion needs an exact outcome")
                );
            };
            let valid = match operation {
                MaterializedConsumeV1::Poll => matches!(
                    step,
                    PollPending
                        | PollReady
                        | PollError
                        | PollUnwind
                        | PollOuterErrorPending
                        | PollOuterUnwindPending
                        | PollOuterErrorReady
                        | PollOuterUnwindReady
                ),
                MaterializedConsumeV1::Recycle => matches!(
                    step,
                    RecycleRetry
                        | RecycleReady
                        | RecycleError
                        | RecycleUnwind
                        | RecycleOuterErrorRetry
                        | RecycleOuterUnwindRetry
                        | RecycleOuterErrorReady
                        | RecycleOuterUnwindReady
                ),
            };
            if expected_id != id || !valid {
                return Err(self.terminal_error("scripted ordinary completion outcome mismatch"));
            }
            let (active, phase) = target.parts_mut(&mut self.active, &mut self.compute_pipeline);
            let started = Instant::now();
            active.execution = Some(ActiveComputeExecutionV1::Materialized(
                MaterializedCompletionReceiptV1::Consuming(operation),
            ));
            if matches!(step, PollUnwind | RecycleUnwind) {
                panic!("scripted ordinary completion unwind");
            }
            if matches!(step, PollError | RecycleError) {
                return Err(self.terminal_error("scripted ordinary completion consumed error"));
            }
            let next = match step {
                PollPending | PollOuterErrorPending | PollOuterUnwindPending => {
                    ActiveComputeExecutionV1::ScriptedMaterialized
                }
                PollReady
                | PollOuterErrorReady
                | PollOuterUnwindReady
                | RecycleRetry
                | RecycleOuterErrorRetry
                | RecycleOuterUnwindRetry => {
                    ActiveComputeExecutionV1::ScriptedMaterializedCompleted
                }
                RecycleReady | RecycleOuterErrorReady | RecycleOuterUnwindReady => {
                    ActiveComputeExecutionV1::ScriptedMaterializedRetired
                }
                _ => unreachable!(),
            };
            active.execution = Some(next);
            let new_phase = execution_phase(active.execution.as_ref()).unwrap();
            if let Some(phase) = phase {
                *phase = new_phase;
            }
            if operation == MaterializedConsumeV1::Poll
                && new_phase == RuntimeComputePipelinePhaseV1::Completed
            {
                active.performance.publish_to_completion = active.published_at.elapsed();
            }
            if new_phase == RuntimeComputePipelinePhaseV1::PhysicallyRetired {
                active.performance.completed_readback = Duration::ZERO;
                active.performance.completion_detach_restore = Duration::ZERO;
            }
            if matches!(
                step,
                PollOuterUnwindPending
                    | PollOuterUnwindReady
                    | RecycleOuterUnwindRetry
                    | RecycleOuterUnwindReady
            ) {
                panic!("scripted ordinary completion outer unwind");
            }
            if matches!(
                step,
                PollOuterErrorPending
                    | PollOuterErrorReady
                    | RecycleOuterErrorRetry
                    | RecycleOuterErrorReady
            ) {
                return Err(self.terminal_error("scripted ordinary completion outer error"));
            }
            if operation == MaterializedConsumeV1::Recycle {
                active.performance.completion_signal_recycle += started.elapsed();
            }
            if new_phase != RuntimeComputePipelinePhaseV1::Completed {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn scripted_materialized_profile_step_v1(&mut self, id: u64) {
        use ScriptedCompletionStepV1::ProfileUnwind;
        if self
            .scripted_materialized_completion
            .as_ref()
            .and_then(|steps| steps.front())
            == Some(&(id, ProfileUnwind))
        {
            self.scripted_materialized_completion
                .as_mut()
                .unwrap()
                .pop_front();
            panic!("scripted ordinary completion profile unwind");
        }
    }
}
