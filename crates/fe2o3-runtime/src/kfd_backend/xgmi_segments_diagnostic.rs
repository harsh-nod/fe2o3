//! Success-only host attribution for a fresh, single-wait ordered XGMI list.

use super::xgmi_batch_diagnostic::Phase;
use std::time::Instant;

#[cfg(feature = "hardware-diagnostic")]
use super::{
    KfdNativeXgmiRuntimeBackendV1, KfdRuntimeBackendErrorKindV1, KfdRuntimeBackendErrorV1,
    RuntimeBackendFailureV1,
};
#[cfg(feature = "hardware-diagnostic")]
use fe2o3_kfd::Gfx942XgmiPairCurrentnessDiagnosticsV1;

const PHASE_COUNT: usize = 7;

/// Repeated submission/wait spans are summed without allocating per descriptor.
/// Const-disabled instances perform no clock reads or recording.
pub(super) struct Timer<const ENABLED: bool> {
    elapsed: [u64; PHASE_COUNT],
    calls: [u32; PHASE_COUNT],
    invalid: bool,
    #[cfg(feature = "hardware-diagnostic")]
    pub(super) currentness: [Option<Gfx942XgmiPairCurrentnessDiagnosticsV1>; 2],
}

impl<const ENABLED: bool> Timer<ENABLED> {
    pub(super) const fn new() -> Self {
        Self {
            elapsed: [0; PHASE_COUNT],
            calls: [0; PHASE_COUNT],
            invalid: false,
            #[cfg(feature = "hardware-diagnostic")]
            currentness: [None; 2],
        }
    }

    #[inline]
    fn start_with(&self, clock: impl FnOnce() -> Instant) -> Option<Instant> {
        if ENABLED { Some(clock()) } else { None }
    }

    #[inline]
    pub(super) fn start(&self) -> Option<Instant> {
        self.start_with(Instant::now)
    }

    #[inline]
    pub(super) fn end(&mut self, phase: Phase, start: Option<Instant>) {
        if ENABLED {
            self.record(
                phase,
                start.and_then(|start| u64::try_from(start.elapsed().as_nanos()).ok()),
            );
        }
    }

    #[inline]
    pub(super) fn measure<T>(&mut self, phase: Phase, operation: impl FnOnce() -> T) -> T {
        let start = self.start();
        let result = operation();
        self.end(phase, start);
        result
    }

    fn record(&mut self, phase: Phase, elapsed: Option<u64>) {
        let index = phase as usize;
        let next = elapsed
            .and_then(|value| self.elapsed[index].checked_add(value))
            .zip(self.calls[index].checked_add(1));
        match next {
            Some((elapsed, calls))
                if calls == 1 || matches!(phase, Phase::Submission | Phase::Wait) =>
            {
                self.elapsed[index] = elapsed;
                self.calls[index] = calls;
            }
            _ => self.invalid = true,
        }
    }

    #[cfg(feature = "hardware-diagnostic")]
    pub(super) fn finish(
        self,
        start: Instant,
        descriptors: u32,
    ) -> Option<KfdRuntimeXgmiSegmentsTimingV1> {
        self.finish_ns(u64::try_from(start.elapsed().as_nanos()).ok()?, descriptors)
    }

    #[cfg(feature = "hardware-diagnostic")]
    fn finish_ns(self, total_ns: u64, descriptors: u32) -> Option<KfdRuntimeXgmiSegmentsTimingV1> {
        if self.invalid
            || descriptors == 0
            || self.calls != [1, 1, 1, descriptors, descriptors, 1, 1]
            || self
                .elapsed
                .iter()
                .try_fold(0u64, |sum, value| sum.checked_add(*value))?
                > total_ns
        {
            return None;
        }
        let [Some(opening), Some(closing)] = self.currentness else {
            return None;
        };
        if !opening.is_complete()
            || !closing.is_complete()
            || opening.total_ns? > self.elapsed[Phase::Opening as usize]
            || closing.total_ns? > self.elapsed[Phase::Closing as usize]
        {
            return None;
        }
        Some(KfdRuntimeXgmiSegmentsTimingV1 {
            admission_validation_ns: self.elapsed[Phase::Admission as usize],
            preparation_ns: self.elapsed[Phase::Preparation as usize],
            opening_currentness_ns: self.elapsed[Phase::Opening as usize],
            submission_ns: self.elapsed[Phase::Submission as usize],
            wait_ns: self.elapsed[Phase::Wait as usize],
            closing_currentness_ns: self.elapsed[Phase::Closing as usize],
            settlement_ns: self.elapsed[Phase::Settlement as usize],
            submission_calls: self.calls[Phase::Submission as usize],
            wait_calls: self.calls[Phase::Wait as usize],
            total_ns,
            opening,
            closing,
        })
    }
}

/// Host intervals, not GPU timings or ownership/release authority. Submission
/// and wait are sums over the entire list. Nested currentness intervals overlap
/// their containing opening/closing phases and must not be added to them.
#[cfg(feature = "hardware-diagnostic")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeXgmiSegmentsTimingV1 {
    pub admission_validation_ns: u64,
    pub preparation_ns: u64,
    pub opening_currentness_ns: u64,
    pub submission_ns: u64,
    pub wait_ns: u64,
    pub closing_currentness_ns: u64,
    pub settlement_ns: u64,
    pub submission_calls: u32,
    pub wait_calls: u32,
    pub total_ns: u64,
    pub opening: Gfx942XgmiPairCurrentnessDiagnosticsV1,
    pub closing: Gfx942XgmiPairCurrentnessDiagnosticsV1,
}

/// Owner-free observation available only after complete logical/native teardown.
#[cfg(feature = "hardware-diagnostic")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeXgmiSegmentsObservationV1 {
    pub backend_submission: u64,
    pub source_device: u64,
    pub destination_device: u64,
    pub descriptor_count: u32,
    pub useful_bytes: u64,
    pub host: KfdRuntimeXgmiSegmentsTimingV1,
}

#[cfg(feature = "hardware-diagnostic")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Identity {
    pub submission: u64,
    pub direction: usize,
    pub descriptors: u32,
    pub useful_bytes: u64,
    pub fresh: bool,
}

#[cfg(feature = "hardware-diagnostic")]
pub(super) struct Recorder {
    devices: [u64; 2],
    expected: usize,
    records: Vec<KfdRuntimeXgmiSegmentsObservationV1>,
    pending: Option<Identity>,
    invalid: bool,
}

#[cfg(feature = "hardware-diagnostic")]
impl Recorder {
    fn new(devices: [u64; 2], expected: usize) -> Result<Self, &'static str> {
        if !(1..=40_000).contains(&expected) {
            return Err("ordered XGMI diagnosis requires 1..=40000 successful lists");
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(expected)
            .map_err(|_| "ordered XGMI diagnostic reservation")?;
        Ok(Self {
            devices,
            expected,
            records,
            pending: None,
            invalid: false,
        })
    }

    pub(super) fn begin(&mut self, identity: Option<Identity>, one_step: bool) -> bool {
        let Some(identity) = identity else {
            self.invalidate();
            return false;
        };
        if self.invalid
            || self.pending.is_some()
            || one_step
            || !identity.fresh
            || identity.direction >= 2
            || identity.submission == 0
            || !(1..=4096).contains(&identity.descriptors)
            || identity.useful_bytes == 0
            || self.records.len() >= self.expected
            || self.records.len() >= self.records.capacity()
            || self
                .records
                .last()
                .is_some_and(|last| identity.submission <= last.backend_submission)
        {
            self.invalidate();
            return false;
        }
        self.pending = Some(identity);
        true
    }

    pub(super) fn finish(
        &mut self,
        identity: Identity,
        observed: Option<KfdRuntimeXgmiSegmentsTimingV1>,
    ) {
        let Some(host) = observed else {
            self.invalidate();
            return;
        };
        if self.invalid
            || self.pending != Some(identity)
            || !valid_timing(host)
            || host.submission_calls != identity.descriptors
            || host.wait_calls != identity.descriptors
            || self.records.len() >= self.expected
            || self.records.len() >= self.records.capacity()
        {
            self.invalidate();
            return;
        }
        self.pending = None;
        self.records.push(KfdRuntimeXgmiSegmentsObservationV1 {
            backend_submission: identity.submission,
            source_device: self.devices[identity.direction],
            destination_device: self.devices[1 - identity.direction],
            descriptor_count: identity.descriptors,
            useful_bytes: identity.useful_bytes,
            host,
        });
    }

    pub(super) fn invalidate(&mut self) {
        self.invalid = true;
    }

    fn complete(&self) -> bool {
        !self.invalid && self.pending.is_none() && self.records.len() == self.expected
    }
}

#[cfg(feature = "hardware-diagnostic")]
fn valid_timing(host: KfdRuntimeXgmiSegmentsTimingV1) -> bool {
    let sum = [
        host.admission_validation_ns,
        host.preparation_ns,
        host.opening_currentness_ns,
        host.submission_ns,
        host.wait_ns,
        host.closing_currentness_ns,
        host.settlement_ns,
    ]
    .into_iter()
    .try_fold(0u64, u64::checked_add);
    sum.is_some_and(|sum| sum <= host.total_ns)
        && host.opening.is_complete()
        && host.closing.is_complete()
        && host
            .opening
            .total_ns
            .is_some_and(|inner| inner <= host.opening_currentness_ns)
        && host
            .closing
            .total_ns
            .is_some_and(|inner| inner <= host.closing_currentness_ns)
}

#[cfg(feature = "hardware-diagnostic")]
fn take_records(
    slot: &mut Option<Recorder>,
    terminal: bool,
    shutdown: bool,
    quiescent: bool,
) -> Result<Vec<KfdRuntimeXgmiSegmentsObservationV1>, KfdRuntimeBackendErrorKindV1> {
    if terminal {
        return Err(KfdRuntimeBackendErrorKindV1::Terminal);
    }
    if !shutdown || !quiescent {
        return Err(KfdRuntimeBackendErrorKindV1::Busy);
    }
    let recorder = slot
        .as_ref()
        .ok_or(KfdRuntimeBackendErrorKindV1::Unsupported)?;
    if !recorder.complete() {
        return Err(KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    }
    Ok(slot
        .take()
        .expect("validated ordered diagnostic recorder")
        .records)
}

#[cfg(feature = "hardware-diagnostic")]
impl KfdNativeXgmiRuntimeBackendV1 {
    /// Enables bounded host attribution before any resource creation. Only fresh
    /// lists completed in one backend wait (or drain) call in increasing backend
    /// submission order qualify. Poll, flush, ordinary/aggregate progress,
    /// accepted cancellation, pending, retry and failure invalidate capture but
    /// do not change execution. Recorder enrollment/append, facade enqueue and
    /// settlement, and submission release are outside the measured intervals.
    pub fn enable_xgmi_segments_diagnostics_v1(
        &mut self,
        expected_lists: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.xgmi_segments_diagnostic.is_some()
            || self.xgmi_diagnostic.is_some()
            || self.xgmi_aggregate_diagnostic.is_some()
            || self.next_handle != 1
            || self.queues.iter().any(Option::is_some)
            || !self.logical_resource_counts().permits_shutdown()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "ordered XGMI diagnosis must begin before resource creation",
            ));
        }
        let devices = self
            .descriptions
            .each_ref()
            .map(|device| device.backend_device);
        self.xgmi_segments_diagnostic = Some(
            Recorder::new(devices, expected_lists)
                .map_err(|error| Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, error))?,
        );
        Ok(())
    }

    /// Consumes a complete capture after explicit successful shutdown. A failed
    /// extraction leaves the recorder intact; no durations authorize teardown.
    pub fn finish_xgmi_segments_diagnostics_v1(
        &mut self,
    ) -> Result<
        Vec<KfdRuntimeXgmiSegmentsObservationV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let (terminal, quiescent) = self.xgmi_diagnostic_teardown_state();
        take_records(&mut self.xgmi_segments_diagnostic, terminal, self.shutdown, quiescent)
            .map_err(|kind| {
                let detail = "ordered XGMI diagnosis requires an enabled, complete capture and successful logical/native teardown";
                if kind == KfdRuntimeBackendErrorKindV1::Terminal {
                    RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(kind, detail))
                } else { Self::rejected(kind, detail) }
            })
    }
}

#[cfg(test)]
mod tests;
