//! Bounded borrowed-result observation on the caller's original canonical ledger.
use crate::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, VerifiedCanonicalKernelIrModuleV12,
};
use std::{error::Error, fmt};

#[path = "budgeted_v12_observation_profile.rs"]
mod profile;

/// Two immutable borrows, not source custody or a reconstructible execution permit.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::Module;
/// use fe2o3_kir_sim::{V12CpuObservationInputV1, SimulationRequestV1};
/// fn raw(module: &Module, request: &SimulationRequestV1) {
///     V12CpuObservationInputV1::new(module, request);
/// }
/// ```
#[derive(Clone, Copy)]
pub struct V12CpuObservationInputV1<'a> {
    owner: &'a VerifiedCanonicalKernelIrModuleV12,
    request: &'a SimulationRequestV1,
}
impl<'a> V12CpuObservationInputV1<'a> {
    /// Caller retains and accounts for both immutable inputs throughout observation.
    pub const fn new(
        owner: &'a VerifiedCanonicalKernelIrModuleV12,
        request: &'a SimulationRequestV1,
    ) -> Self {
        Self { owner, request }
    }
}

/// Closed finite profile; options can only reduce the fixed step/record ceilings.
/// No schedule, physical capture, dynamic LDS or allocation-reuse authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct V12CpuObservationOptionsV1 {
    steps: u64,
    records: usize,
}
impl Default for V12CpuObservationOptionsV1 {
    fn default() -> Self {
        Self {
            steps: profile::STEPS as u64,
            records: profile::RECORDS,
        }
    }
}
impl V12CpuObservationOptionsV1 {
    /// A lower ceiling changes refusal behavior, never numerical semantics.
    pub fn with_step_limit(mut self, steps: u64) -> Result<Self, V12CpuObservationProfileErrorV1> {
        if steps == 0 || steps > profile::STEPS as u64 {
            return Err(V12CpuObservationProfileErrorV1::Limits);
        }
        self.steps = steps;
        Ok(self)
    }
    /// Includes every generic debug callback, not just selected Matrix checkpoints.
    pub fn with_record_limit(
        mut self,
        records: usize,
    ) -> Result<Self, V12CpuObservationProfileErrorV1> {
        if records == 0 || records > profile::RECORDS {
            return Err(V12CpuObservationProfileErrorV1::Limits);
        }
        self.records = records;
        Ok(self)
    }
    /// Exact limits shared by admission and this bounded observed execution.
    pub fn simulation_limits(self) -> SimulationLimitsV1 {
        SimulationLimitsV1 {
            max_canonical_bytes: 16 * 1024,
            max_reachable_functions: 2,
            max_reachable_operations: 1024,
            max_invocations: 64,
            max_workgroups: 1,
            max_scheduled_slots: 64,
            max_steps: self.steps,
            max_call_depth: 1,
            max_ssa_values: 512,
            max_allocations: 128,
            max_allocation_bytes: 4096,
            max_total_bytes: 65536,
            max_resident_bytes: 64 * 1024 * 1024,
            max_events: 65536,
            max_memory_access_records: 4096,
        }
    }
}

/// Compact closed refusal coordinates; no copied graph or user data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V12CpuObservationProfileErrorV1 {
    Limits,
    OwnerMismatch,
    FunctionRoster,
    CanonicalBytes,
    Counts,
    Type,
    Operation,
    Matrix,
    Launch,
    Request,
    Accounting,
}

/// A borrowed simulator error is observed while charged, never boxed or copied here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V12CpuObservationErrorV1<E> {
    Resource(Resource),
    Profile(V12CpuObservationProfileErrorV1),
    /// Delivery stopped or exceeded its finite bound; execution may have continued.
    IncompleteObservation,
    Observer(E),
}
impl<E> From<Resource> for V12CpuObservationErrorV1<E> {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl<E: fmt::Debug> fmt::Display for V12CpuObservationErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bounded V12 CPU observation: {self:?}")
    }
}
impl<E: fmt::Debug> Error for V12CpuObservationErrorV1<E> {}

struct BoundedDebug<'a, S> {
    sink: &'a mut S,
    count: usize,
    limit: usize,
    incomplete: bool,
}
fn complete_checkpoint(
    stack: &SimulationDebugCollectionV1<SimulationDebugFrameV1>,
    memory: &SimulationDebugCollectionV1<SimulationDebugAllocationV1>,
) -> bool {
    let complete_stack = match stack {
        SimulationDebugCollectionV1::Captured(frames) => frames
            .iter()
            .all(|frame| matches!(&frame.values, SimulationDebugCollectionV1::Captured(_))),
        SimulationDebugCollectionV1::Unavailable { .. } => false,
    };
    complete_stack && matches!(memory, SimulationDebugCollectionV1::Captured(_))
}
impl<S: SimulationDebugSinkV1> SimulationDebugSinkV1 for BoundedDebug<'_, S> {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        if self.count == self.limit {
            self.incomplete = true;
            return SimulationDebugSinkControlV1::DropAndStop;
        }
        self.count += 1;
        if let SimulationDebugRecordKindV1::Checkpoint { stack, memory, .. } = &record.kind
            && !complete_checkpoint(stack, memory)
        {
            self.incomplete = true;
            return SimulationDebugSinkControlV1::DropAndStop;
        }
        let control = self.sink.record(record);
        if control != SimulationDebugSinkControlV1::Continue {
            self.incomplete = true;
        }
        control
    }
    // Deliberately no context/lifecycle/physical opt-ins in this finite adapter.
}

#[cfg(test)]
mod tests {
    use super::*;
    fn unavailable<T>() -> SimulationDebugCollectionV1<T> {
        SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::AllocationFailure,
            required: 1,
        }
    }
    #[test]
    fn unavailable_outer_or_nested_snapshot_never_is_complete() {
        let frames = SimulationDebugCollectionV1::Captured(Vec::new());
        let memory = SimulationDebugCollectionV1::Captured(Vec::new());
        assert!(complete_checkpoint(&frames, &memory));
        assert!(!complete_checkpoint(&unavailable(), &memory));
        assert!(!complete_checkpoint(&frames, &unavailable()));
        let nested = SimulationDebugCollectionV1::Captured(vec![SimulationDebugFrameV1 {
            depth: 0,
            function_ordinal: 0,
            block: fe2o3_kernel_ir::BlockId(0),
            next_operation: Some(0),
            values: unavailable(),
        }]);
        assert!(!complete_checkpoint(&nested, &memory));
    }
}

impl AdmittedSimulationModuleV1 {
    /// Observes the actual immutable V12 graph with the ordinary CPU executor.
    ///
    /// Prepayment includes allocating resident calculators, Engine, ephemeral
    /// snapshots, result/error coexistence and fixed logical work. This is not
    /// an RSS or wall-clock bound. Inputs and caller-retained Copy observations
    /// must already be accounted for. The result/error is dropped before the
    /// scope releases its reservation on normal return, error or unwind.
    ///
    /// A successful continuation is NOT necessarily successful execution:
    /// the observer receives the exact borrowed Result and must inspect it.
    /// No source, artifact, native or GPU authority is granted.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_kir_sim::*;
    /// fn escape<'a>(view: &AdmittedSimulationModuleV1, input: V12CpuObservationInputV1<'_>, b: &mut Budget<'_>) -> &'a SimulationExecutionV1 {
    ///     view.with_v12_cpu_observation_v1(input, V12CpuObservationOptionsV1::default(), b,
    ///         (&mut NoopSimulationEventSinkV1, &mut NoopSimulationDebugSinkV1),
    ///         |result, _| Ok::<_, ()>(result.unwrap())).unwrap()
    /// }
    /// ```
    pub fn with_v12_cpu_observation_v1<R: Copy + 'static, E: Copy + 'static>(
        &self,
        input: V12CpuObservationInputV1<'_>,
        options: V12CpuObservationOptionsV1,
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
        let work = profile::work(options.records)?;
        budget.with_prepaid_scope(floor, 1, work, profile::STORAGE, |budget| {
            profile::check(self, input).map_err(V12CpuObservationErrorV1::Profile)?;
            // This guard is allocation-free and runs before existing calculators.
            if !profile::scratch_fits() {
                return Err(V12CpuObservationErrorV1::Profile(
                    V12CpuObservationProfileErrorV1::Accounting,
                ));
            }
            let mut debug = BoundedDebug {
                sink: sinks.1,
                count: 0,
                limit: options.records,
                incomplete: false,
            };
            let capture =
                SimulationDebugCaptureLimitsV1::new(1, 512, 128, 65536).map_err(|_| {
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
