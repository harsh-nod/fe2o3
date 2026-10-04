use super::*;
use crate::{
    DebugKirIdentityV1, DebugTerminalFaultV1, DebugWaveWidthV1, DebuggerLimitsV1,
    TranscriptCollectorV1,
};
use fe2o3_kir_sim::{DivergentWorkgroupBarrierV2, SimulationDebugSinkV1, SimulationOutOfBoundsV2};

/// Wrap the actual legacy collector; never duplicate its record-cost policy.
pub(super) struct OriginCollector {
    inner: TranscriptCollectorV1,
    retained: Retention,
}

impl OriginCollector {
    pub(super) fn new(debugger: DebuggerLimitsV1, limits: Option<Limits>) -> Self {
        Self::with_retention(debugger, Retention::new(limits))
    }

    pub(super) fn with_retention(debugger: DebuggerLimitsV1, retained: Retention) -> Self {
        Self {
            inner: TranscriptCollectorV1::new(debugger),
            retained,
        }
    }

    pub(super) fn record_count(&self) -> usize {
        self.inner.records.len()
    }

    fn deliver(&mut self, record: SimulationDebugRecordV1, context: Context) -> Control {
        let pending = self
            .retained
            .prepare(self.inner.records.len(), &record, context);
        let control = self.inner.record(record);
        self.retained
            .commit(pending, control, self.inner.records.len());
        control
    }

    pub(super) fn finish(
        self,
        identity: DebugKirIdentityV1,
        width: DebugWaveWidthV1,
        fault: Option<DebugTerminalFaultV1>,
    ) -> ObservedTranscript {
        self.retained
            .seal(self.inner.into_transcript(identity, width, fault))
    }
}

impl SimulationDebugSinkV1 for OriginCollector {
    fn record(&mut self, record: SimulationDebugRecordV1) -> Control {
        self.deliver(record, Context::Unavailable(RuntimeMissing::NotRequested))
    }

    fn wants_operation_origin_v1(&self) -> bool {
        self.retained.limits.is_some()
    }

    fn record_with_operation_origin_v1(
        &mut self,
        record: SimulationDebugRecordV1,
        origin: Context,
    ) -> Control {
        self.deliver(record, origin)
    }

    fn terminal_out_of_bounds_v2(&mut self, detail: SimulationOutOfBoundsV2) {
        self.inner.terminal_out_of_bounds_v2(detail);
    }

    fn terminal_barrier_divergence_v2(&mut self, detail: DivergentWorkgroupBarrierV2) {
        self.inner.terminal_barrier_divergence_v2(detail);
    }
}
