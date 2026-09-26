//! Distinct two-frame BF16 helper CPU observation; old root-only profile unchanged.
use super::*;
#[path = "budgeted_v12_bf16_call_profile_v1.rs"]
mod call_profile;

/// Closed one-call profile. Only lower step/record ceilings can be selected.
/// Source, LLVM, native and hardware authority are not created by CPU execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Bf16CallCpuObservationOptionsV1 {
    base: V12CpuObservationOptionsV1,
}
impl Bf16CallCpuObservationOptionsV1 {
    /// Reduce, never increase, the existing finite execution-step ceiling.
    pub fn with_step_limit(mut self, steps: u64) -> Result<Self, V12CpuObservationProfileErrorV1> {
        self.base = self.base.with_step_limit(steps)?;
        Ok(self)
    }
    /// Reduce, never increase, the existing finite complete-record ceiling.
    pub fn with_record_limit(
        mut self,
        records: usize,
    ) -> Result<Self, V12CpuObservationProfileErrorV1> {
        self.base = self.base.with_record_limit(records)?;
        Ok(self)
    }
    /// Same graph/resident ceilings, with exactly two permitted active frames.
    /// The old V12 profile still permits only one.
    pub fn simulation_limits(self) -> SimulationLimitsV1 {
        let mut limits = self.base.simulation_limits();
        limits.max_call_depth = 2;
        limits
    }
}
impl AdmittedSimulationModuleV1 {
    /// Run the actual admitted V12 graph with one independently checked nominal-
    /// shaped scalar helper, using the caller's original cumulative ledger.
    ///
    /// This validates canonical structure, not Rust source authenticity. The
    /// ordinary engine still enforces call/frame types, full-wave/site convergence
    /// and the exact-integer MFMA domain. Every Result exit restores the incoming
    /// storage floor; result/error is dropped before scope cleanup, including unwind.
    /// Successful observer delivery does not itself mean successful execution:
    /// inspect the borrowed Result. No decoded bytes are source custody.
    pub fn with_bf16_call_cpu_observation_v1<R: Copy + 'static, E: Copy + 'static>(
        &self,
        input: V12CpuObservationInputV1<'_>,
        options: Bf16CallCpuObservationOptionsV1,
        budget: &mut Budget<'_>,
        sinks: (
            &mut impl SimulationEventSinkV1,
            &mut impl SimulationDebugSinkV1,
        ),
        observe: impl for<'run, 'work> FnOnce(
            Result<&'run SimulationExecutionV1, &'run SimulationErrorV1>,
            &mut Budget<'work>,
        ) -> Result<R, E>,
    ) -> Result<R, V12CpuObservationErrorV1<E>> {
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(V12CpuObservationErrorV1::Profile(
                V12CpuObservationProfileErrorV1::Accounting,
            ));
        }
        let floor = budget.storage();
        let work = call_profile::work(options.base.records)?;
        budget.with_prepaid_scope(floor, 1, work, profile::STORAGE, |budget| {
            call_profile::check(self, input).map_err(V12CpuObservationErrorV1::Profile)?;
            if !call_profile::scratch_fits() {
                return Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::Accounting,
                ));
            }
            let mut debug = BoundedDebug {
                sink: sinks.1,
                count: 0,
                limit: options.base.records,
                incomplete: false,
            };
            let capture =
                SimulationDebugCaptureLimitsV1::new(2, 512, 128, 65536).map_err(|_| {
                    V12CpuObservationErrorV1::Profile(V12CpuObservationProfileErrorV1::Limits)
                })?;
            let result = self.simulate_debugged_with_observation_options(
                input.request,
                SimulationTargetV1::amdgpu_64(),
                options.simulation_limits(),
                ObservationExecutionOptionsV1::new(capture),
                sinks.0,
                &mut debug,
            );
            if debug.incomplete {
                drop(result);
                return Err(V12CpuObservationErrorV1::IncompleteObservation);
            }
            let observed =
                observe(result.as_ref(), budget).map_err(V12CpuObservationErrorV1::Observer);
            drop(result);
            if budget.failed_work().is_some() || budget.failed_storage().is_some() {
                return Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::Accounting,
                ));
            }
            observed
        })
    }
}
