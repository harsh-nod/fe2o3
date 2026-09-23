//! The sole constructor of sealed observed owners; one accepted legacy-record counter.
use super::*;
use crate::{
    DebugKirIdentityV1, DebugTerminalFaultV1, DebugWaveWidthV1, DebuggerLimitsV1,
    TranscriptCollectorV1,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, DivergentWorkgroupBarrierV2, NoopSimulationEventSinkV1,
    ObservationExecutionOptionsV1, SimulationAllocationTransitionV1,
    SimulationAllocationWatermarkV1, SimulationDebugCaptureLimitsV1,
    SimulationDebugCheckpointFramesV1, SimulationDebugFrameOriginUnavailableV1,
    SimulationDebugObservationContextV1, SimulationDebugSinkV1, SimulationErrorV1,
    SimulationLimitsV1, SimulationOutOfBoundsV2, SimulationRequestV1, SimulationScheduleRequestV1,
    SimulationTargetV1,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CAPTURE_INSTANCE: AtomicU64 = AtomicU64::new(1);
fn capture_instance() -> Result<u64, RuntimeObservationConfigErrorV1> {
    NEXT_CAPTURE_INSTANCE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| RuntimeObservationConfigErrorV1::CaptureInstanceExhausted)
}

struct ObservationCollector {
    inner: TranscriptCollectorV1,
    origins: Retention,
    frames: FrameRetention,
    allocations: AllocationRetention,
    capture_instance: u64,
    options: RuntimeObservationOptionsV1,
}
impl ObservationCollector {
    fn new(
        limits: DebuggerLimitsV1,
        options: RuntimeObservationOptionsV1,
    ) -> Result<Self, RuntimeObservationConfigErrorV1> {
        let capture_instance = capture_instance()?;
        Ok(Self {
            inner: TranscriptCollectorV1::new(limits),
            origins: Retention::new(options.origins.limits()),
            frames: FrameRetention::new(options.frames.limits()),
            allocations: AllocationRetention::new(options.allocations.limits()),
            capture_instance,
            options,
        })
    }
    fn deliver(
        &mut self,
        record: SimulationDebugRecordV1,
        origin: Context,
        frames: SimulationDebugCheckpointFramesV1<'_>,
        watermark: SimulationAllocationWatermarkV1,
    ) -> Control {
        let index = self.inner.records.len();
        let pending_origin = self.origins.prepare(index, &record, origin);
        let pending_frames = self.frames.prepare(index, &record, frames);
        let pending_allocation = self.allocations.prepare(index, &record, watermark);
        let control = self.inner.record(record);
        let accepted = self.inner.records.len();
        self.origins.commit(pending_origin, control, accepted);
        self.frames.commit(pending_frames, control, accepted);
        self.allocations
            .commit(pending_allocation, control, accepted);
        control
    }
    fn finish(
        mut self,
        identity: DebugKirIdentityV1,
        width: DebugWaveWidthV1,
        fault: Option<DebugTerminalFaultV1>,
    ) -> DebugObservedTranscriptV1 {
        let len = self.inner.records.len();
        self.origins.validate_seal(len);
        self.frames.validate_seal(len);
        self.allocations.validate_seal(len);
        DebugObservedTranscriptV1 {
            transcript: self.inner.into_transcript(identity, width, fault),
            retained: self.origins,
            frames: self.frames,
            allocations: self.allocations,
            capture_instance: self.capture_instance,
            options: self.options,
        }
    }
}
impl SimulationDebugSinkV1 for ObservationCollector {
    fn record(&mut self, record: SimulationDebugRecordV1) -> Control {
        self.deliver(
            record,
            Context::Unavailable(RuntimeMissing::NotRequested),
            SimulationDebugCheckpointFramesV1::Unavailable(
                SimulationDebugFrameOriginUnavailableV1::NotRequested,
            ),
            SimulationAllocationWatermarkV1::NotRequested,
        )
    }
    fn wants_operation_origin_v1(&self) -> bool {
        self.origins.limits.is_some()
    }
    fn wants_checkpoint_frames_v1(&self) -> bool {
        self.frames.coverage != Coverage::Disabled
    }
    fn wants_allocation_lifecycle_v1(&self) -> bool {
        self.allocations.enabled()
    }
    fn wants_observation_context_v1(&self) -> bool {
        self.origins.limits.is_some()
            || self.frames.coverage != Coverage::Disabled
            || self.allocations.enabled()
    }
    fn record_with_observation_context_v1(
        &mut self,
        record: SimulationDebugRecordV1,
        context: SimulationDebugObservationContextV1<'_>,
    ) -> Control {
        self.deliver(
            record,
            context.operation_origin(),
            context.checkpoint_frames(),
            context.allocation_lifecycle(),
        )
    }
    fn allocation_lifecycle_v1(&mut self, transition: SimulationAllocationTransitionV1) -> Control {
        self.allocations.receive(transition)
    }
    fn terminal_out_of_bounds_v2(&mut self, detail: SimulationOutOfBoundsV2) {
        self.inner.terminal_out_of_bounds_v2(detail);
    }
    fn terminal_barrier_divergence_v2(&mut self, detail: DivergentWorkgroupBarrierV2) {
        self.inner.terminal_barrier_divergence_v2(detail);
    }
}

/// Fresh ordinary execution. No caller can attach metadata to an old transcript.
#[allow(clippy::too_many_arguments)]
pub fn capture_debugger_observed_run_v1(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    simulation_limits: SimulationLimitsV1,
    capture_limits: SimulationDebugCaptureLimitsV1,
    debugger_limits: DebuggerLimitsV1,
    wave_width: DebugWaveWidthV1,
    options: RuntimeObservationOptionsV1,
) -> Result<DebugObservedRunV1, RuntimeObservationConfigErrorV1> {
    capture(
        module,
        request,
        target,
        simulation_limits,
        capture_limits,
        debugger_limits,
        wave_width,
        options,
        None,
    )
}

/// Canonical/seeded/replay schedules pass through the existing checked simulator.
#[allow(clippy::too_many_arguments)]
pub fn capture_debugger_observed_scheduled_run_v1(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    simulation_limits: SimulationLimitsV1,
    capture_limits: SimulationDebugCaptureLimitsV1,
    debugger_limits: DebuggerLimitsV1,
    wave_width: DebugWaveWidthV1,
    options: RuntimeObservationOptionsV1,
    schedule: SimulationScheduleRequestV1<'_>,
) -> Result<DebugObservedRunV1, RuntimeObservationConfigErrorV1> {
    capture(
        module,
        request,
        target,
        simulation_limits,
        capture_limits,
        debugger_limits,
        wave_width,
        options,
        Some(schedule),
    )
}

#[allow(clippy::too_many_arguments)]
fn capture(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    simulation_limits: SimulationLimitsV1,
    capture_limits: SimulationDebugCaptureLimitsV1,
    debugger_limits: DebuggerLimitsV1,
    wave_width: DebugWaveWidthV1,
    options: RuntimeObservationOptionsV1,
    schedule: Option<SimulationScheduleRequestV1<'_>>,
) -> Result<DebugObservedRunV1, RuntimeObservationConfigErrorV1> {
    let identity = DebugKirIdentityV1 {
        digest: *module.identity().digest(),
        canonical_len: module.identity().canonical_length(),
    };
    let mut collector = ObservationCollector::new(debugger_limits, options)?;
    let mut execution_options = ObservationExecutionOptionsV1::new(capture_limits);
    if let Some(policy) = options.allocation_reuse {
        execution_options = execution_options.with_allocation_reuse(policy);
    }
    let execution = match schedule {
        Some(schedule) => module.simulate_debugged_scheduled_with_observation_options(
            request,
            target,
            simulation_limits,
            schedule,
            execution_options,
            &mut NoopSimulationEventSinkV1,
            &mut collector,
        ),
        None => module.simulate_debugged_with_observation_options(
            request,
            target,
            simulation_limits,
            execution_options,
            &mut NoopSimulationEventSinkV1,
            &mut collector,
        ),
    };
    let fault = match &execution {
        Err(SimulationErrorV1::Execution(error)) => Some(DebugTerminalFaultV1 {
            ordinal: collector
                .inner
                .records
                .last()
                .map_or(0, |record| record.ordinal.saturating_add(1)),
            invocation: error.invocation,
            site: error.site.clone(),
            kind: error.kind.clone(),
        }),
        _ => None,
    };
    Ok(DebugObservedRunV1 {
        execution,
        transcript: collector.finish(identity, wave_width, fault),
    })
}
