//! Bounded host attribution for explicit aggregate XGMI progress.

use std::time::Instant;

#[cfg(feature = "hardware-diagnostic")]
use super::{
    KfdNativeXgmiRuntimeBackendV1, KfdRuntimeBackendErrorKindV1, KfdRuntimeBackendErrorV1,
    RuntimeBackendFailureV1,
};
#[cfg(feature = "hardware-diagnostic")]
use fe2o3_kfd::{Gfx942NativeXgmiSdmaQueueV1, Gfx942XgmiPairCurrentnessDiagnosticsV1};

#[cfg(feature = "hardware-diagnostic")]
const MAX_RECORDS: usize = 40_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Admission,
    Preparation,
    Opening,
    Submission,
    Wait,
    Closing,
    Settlement,
}

const PHASE_COUNT: usize = 7;

/// Const-disabled instances execute the original operation directly, without
/// clock reads, allocation, or callbacks.
pub(super) struct CallTimer<const ENABLED: bool> {
    elapsed: [Option<u64>; PHASE_COUNT],
    visited: [bool; PHASE_COUNT],
    #[cfg(feature = "hardware-diagnostic")]
    pub(super) currentness: [Option<Gfx942XgmiPairCurrentnessDiagnosticsV1>; 2],
}

impl<const ENABLED: bool> CallTimer<ENABLED> {
    pub(super) const fn new() -> Self {
        Self {
            elapsed: [None; PHASE_COUNT],
            visited: [false; PHASE_COUNT],
            #[cfg(feature = "hardware-diagnostic")]
            currentness: [None; 2],
        }
    }

    #[inline]
    pub(super) fn start(&self) -> Option<Instant> {
        if ENABLED { Some(Instant::now()) } else { None }
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
        self.elapsed[index] = if self.visited[index] { None } else { elapsed };
        self.visited[index] = true;
    }

    #[cfg(feature = "hardware-diagnostic")]
    pub(super) fn finish(self, start: Instant) -> KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
        KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
            admission_validation_ns: self.elapsed[Phase::Admission as usize],
            preparation_ns: self.elapsed[Phase::Preparation as usize],
            opening_currentness_ns: self.elapsed[Phase::Opening as usize],
            submission_ns: self.elapsed[Phase::Submission as usize],
            wait_ns: self.elapsed[Phase::Wait as usize],
            closing_currentness_ns: self.elapsed[Phase::Closing as usize],
            settlement_ns: self.elapsed[Phase::Settlement as usize],
            total_ns: u64::try_from(start.elapsed().as_nanos()).ok(),
        }
    }
}

/// Host-only timing for one successful backend aggregate-progress call.
///
/// These durations are neither device timings nor execution, completion, or
/// cleanup authority. They exclude facade enqueue, facade settlement, and
/// submission release outside the backend call. `None` makes the observation
/// invalid and unavailable for extraction.
#[cfg(feature = "hardware-diagnostic")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
    pub admission_validation_ns: Option<u64>,
    pub preparation_ns: Option<u64>,
    pub opening_currentness_ns: Option<u64>,
    pub submission_ns: Option<u64>,
    pub wait_ns: Option<u64>,
    pub closing_currentness_ns: Option<u64>,
    pub settlement_ns: Option<u64>,
    pub total_ns: Option<u64>,
}

/// One owner-free observation published only after successful aggregate
/// progress and complete runtime/native teardown.
#[cfg(feature = "hardware-diagnostic")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeXgmiAggregateCallObservationV1 {
    pub backend_submission: u64,
    pub source_device: u64,
    pub destination_device: u64,
    pub host: KfdRuntimeXgmiAggregateCallDiagnosticsV1,
}

/// Joined host intervals for one successful aggregate call. Available only
/// after complete teardown; nested intervals grant no execution authority.
#[cfg(feature = "hardware-diagnostic")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeXgmiAggregateCurrentnessObservationV1 {
    pub aggregate: KfdRuntimeXgmiAggregateCallObservationV1,
    pub opening: Gfx942XgmiPairCurrentnessDiagnosticsV1,
    pub closing: Gfx942XgmiPairCurrentnessDiagnosticsV1,
}

#[cfg(feature = "hardware-diagnostic")]
enum Records {
    Aggregate(Vec<KfdRuntimeXgmiAggregateCallObservationV1>),
    Currentness(Vec<KfdRuntimeXgmiAggregateCurrentnessObservationV1>),
}

#[cfg(feature = "hardware-diagnostic")]
impl Records {
    fn len(&self) -> usize {
        match self {
            Self::Aggregate(records) => records.len(),
            Self::Currentness(records) => records.len(),
        }
    }

    fn capacity(&self) -> usize {
        match self {
            Self::Aggregate(records) => records.capacity(),
            Self::Currentness(records) => records.capacity(),
        }
    }

    fn is_currentness(&self) -> bool {
        matches!(self, Self::Currentness(_))
    }

    #[cfg(test)]
    fn aggregate(&self) -> &Vec<KfdRuntimeXgmiAggregateCallObservationV1> {
        match self {
            Self::Aggregate(records) => records,
            Self::Currentness(_) => panic!("not aggregate records"),
        }
    }
}

#[cfg(feature = "hardware-diagnostic")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CallIdentity {
    pub direction: usize,
    pub submission: u64,
    pub published: bool,
}

#[cfg(feature = "hardware-diagnostic")]
pub(super) struct Recorder {
    devices: [u64; 2],
    expected: usize,
    records: Records,
    last_submission: Option<u64>,
    pending: Option<CallIdentity>,
    invalid: bool,
}

#[cfg(feature = "hardware-diagnostic")]
fn valid_timing(value: KfdRuntimeXgmiAggregateCallDiagnosticsV1) -> bool {
    let total = (|| {
        let stages = value
            .admission_validation_ns?
            .checked_add(value.preparation_ns?)?
            .checked_add(value.opening_currentness_ns?)?
            .checked_add(value.submission_ns?)?
            .checked_add(value.wait_ns?)?
            .checked_add(value.closing_currentness_ns?)?
            .checked_add(value.settlement_ns?)?;
        Some(stages <= value.total_ns?)
    })();
    total == Some(true)
}

#[cfg(feature = "hardware-diagnostic")]
impl Recorder {
    pub(super) fn new(devices: [u64; 2], expected: usize) -> Result<Self, &'static str> {
        Self::with_currentness(devices, expected, false)
    }

    fn with_currentness(
        devices: [u64; 2],
        expected: usize,
        currentness: bool,
    ) -> Result<Self, &'static str> {
        if expected == 0 || expected > MAX_RECORDS {
            return Err("XGMI aggregate diagnostic requires 1..=40000 successful calls");
        }
        fn reserve<T>(expected: usize) -> Result<Vec<T>, &'static str> {
            let mut records = Vec::new();
            records
                .try_reserve_exact(expected)
                .map_err(|_| "XGMI aggregate diagnostic record reservation")?;
            Ok(records)
        }
        let records = if currentness {
            Records::Currentness(reserve(expected)?)
        } else {
            Records::Aggregate(reserve(expected)?)
        };
        Ok(Self {
            devices,
            expected,
            records,
            last_submission: None,
            pending: None,
            invalid: false,
        })
    }

    /// Arms one first-attempt depth-one call before timed work. Unsupported
    /// shapes continue normally but permanently invalidate this capture.
    pub(super) fn begin(&mut self, identity: CallIdentity, packets: usize) -> bool {
        if self.invalid
            || self.pending.is_some()
            || packets != 1
            || identity.direction >= 2
            || identity.submission == 0
            || identity.published
            || self.records.len() >= self.expected
            || self.records.len() >= self.records.capacity()
            || self
                .last_submission
                .is_some_and(|last| identity.submission <= last)
        {
            self.invalid = true;
            return false;
        }
        self.pending = Some(identity);
        true
    }

    /// Records only after every native owner has returned to backend custody
    /// and logical settlement has succeeded. Diagnostic failure never changes
    /// the workload result.
    pub(super) fn finish(
        &mut self,
        identity: CallIdentity,
        observed: Option<KfdRuntimeXgmiAggregateCallDiagnosticsV1>,
        currentness: Option<[Gfx942XgmiPairCurrentnessDiagnosticsV1; 2]>,
    ) {
        let Some(host) = observed else {
            self.invalid = true;
            return;
        };
        if self.invalid
            || self.pending != Some(identity)
            || !valid_timing(host)
            || !match (&self.records, currentness) {
                (Records::Aggregate(_), None) => true,
                (Records::Currentness(_), Some([opening, closing])) => {
                    opening.is_complete()
                        && closing.is_complete()
                        && matches!((opening.total_ns, host.opening_currentness_ns), (Some(inner), Some(outer)) if inner <= outer)
                        && matches!((closing.total_ns, host.closing_currentness_ns), (Some(inner), Some(outer)) if inner <= outer)
                }
                _ => false,
            }
            || self.records.len() >= self.expected
            || self.records.len() >= self.records.capacity()
        {
            self.invalid = true;
            return;
        }
        self.pending = None;
        self.last_submission = Some(identity.submission);
        let aggregate = KfdRuntimeXgmiAggregateCallObservationV1 {
            backend_submission: identity.submission,
            source_device: self.devices[identity.direction],
            destination_device: self.devices[1 - identity.direction],
            host,
        };
        match &mut self.records {
            Records::Aggregate(records) => records.push(aggregate),
            Records::Currentness(records) => {
                let [opening, closing] = currentness.expect("validated currentness intervals");
                records.push(KfdRuntimeXgmiAggregateCurrentnessObservationV1 {
                    aggregate,
                    opening,
                    closing,
                });
            }
        }
    }

    pub(super) fn is_currentness(&self) -> bool {
        self.records.is_currentness()
    }

    pub(super) fn invalidate(&mut self) {
        self.invalid = true;
    }

    fn complete(&self) -> bool {
        !self.invalid && self.pending.is_none() && self.records.len() == self.expected
    }
}

#[cfg(feature = "hardware-diagnostic")]
fn take_records(
    slot: &mut Option<Recorder>,
    terminal: bool,
    shutdown: bool,
    quiescent: bool,
) -> Result<Vec<KfdRuntimeXgmiAggregateCallObservationV1>, KfdRuntimeBackendErrorKindV1> {
    match take_storage(slot, terminal, shutdown, quiescent, false)? {
        Records::Aggregate(records) => Ok(records),
        Records::Currentness(_) => unreachable!("validated recorder mode"),
    }
}

#[cfg(feature = "hardware-diagnostic")]
fn take_storage(
    slot: &mut Option<Recorder>,
    terminal: bool,
    shutdown: bool,
    quiescent: bool,
    currentness: bool,
) -> Result<Records, KfdRuntimeBackendErrorKindV1> {
    if terminal {
        return Err(KfdRuntimeBackendErrorKindV1::Terminal);
    }
    if !shutdown || !quiescent {
        return Err(KfdRuntimeBackendErrorKindV1::Busy);
    }
    let recorder = slot
        .as_ref()
        .ok_or(KfdRuntimeBackendErrorKindV1::Unsupported)?;
    if recorder.is_currentness() != currentness {
        return Err(KfdRuntimeBackendErrorKindV1::Unsupported);
    }
    if !recorder.complete() {
        return Err(KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    }
    Ok(slot
        .take()
        .expect("validated XGMI aggregate diagnostic recorder")
        .records)
}

#[cfg(feature = "hardware-diagnostic")]
impl KfdNativeXgmiRuntimeBackendV1 {
    /// Enables bounded success-only aggregate attribution before any runtime
    /// resource exists. Only unpublished depth-one calls executed in strictly
    /// increasing backend submission-ID order are recordable. This diagnostic
    /// profile is narrower than the aggregate execution API; unsupported calls
    /// still execute normally but invalidate the entire capture. A pending
    /// result, retry, or error also invalidates capture; only calls returning
    /// `RuntimePeerCopyBatchPollV1::Succeeded` contribute observations.
    pub fn enable_xgmi_aggregate_diagnostics_v1(
        &mut self,
        expected_calls: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.enable_aggregate_diagnostics(expected_calls, false)
    }

    /// Enables joined opening/closing full-currentness intervals under the
    /// same bounded, success-only, depth-one rules as aggregate attribution.
    pub fn enable_xgmi_aggregate_currentness_diagnostics_v1(
        &mut self,
        expected_calls: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.enable_aggregate_diagnostics(expected_calls, true)
    }

    fn enable_aggregate_diagnostics(
        &mut self,
        expected_calls: usize,
        currentness: bool,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.xgmi_aggregate_diagnostic.is_some()
            || self.xgmi_diagnostic.is_some()
            || self.next_handle != 1
            || self.queues.iter().any(Option::is_some)
            || !self.logical_resource_counts().permits_shutdown()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "XGMI aggregate diagnosis must begin before resource creation",
            ));
        }
        let devices = self
            .descriptions
            .each_ref()
            .map(|device| device.backend_device);
        let recorder = if currentness {
            Recorder::with_currentness(devices, expected_calls, true)
        } else {
            Recorder::new(devices, expected_calls)
        }
        .map_err(|error| Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, error))?;
        self.xgmi_aggregate_diagnostic = Some(recorder);
        Ok(())
    }

    /// Takes a complete owner-free capture only after successful logical and
    /// native shutdown. No duration authorizes reuse or resource release.
    pub fn finish_xgmi_aggregate_diagnostics_v1(
        &mut self,
    ) -> Result<
        Vec<KfdRuntimeXgmiAggregateCallObservationV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let (terminal, quiescent) = self.aggregate_diagnostic_teardown_state();
        take_records(
            &mut self.xgmi_aggregate_diagnostic,
            terminal,
            self.shutdown,
            quiescent,
        )
        .map_err(Self::aggregate_diagnostic_failure)
    }

    /// Takes the complete joined capture only after logical and native teardown.
    /// Calling the wrong extractor leaves the capture available to its own mode.
    pub fn finish_xgmi_aggregate_currentness_diagnostics_v1(
        &mut self,
    ) -> Result<
        Vec<KfdRuntimeXgmiAggregateCurrentnessObservationV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let (terminal, quiescent) = self.aggregate_diagnostic_teardown_state();
        match take_storage(
            &mut self.xgmi_aggregate_diagnostic,
            terminal,
            self.shutdown,
            quiescent,
            true,
        )
        .map_err(Self::aggregate_diagnostic_failure)?
        {
            Records::Currentness(records) => Ok(records),
            Records::Aggregate(_) => unreachable!("validated recorder mode"),
        }
    }

    fn aggregate_diagnostic_teardown_state(&self) -> (bool, bool) {
        let terminal = self.terminal
            || self
                .queue_creation_roots
                .iter()
                .any(|root| !root.is_vacant())
            || self
                .queues
                .iter()
                .flatten()
                .any(Gfx942NativeXgmiSdmaQueueV1::has_terminal_retirement_v1);
        let quiescent = self.queues.iter().all(Option::is_none)
            && self.logical_resource_counts().permits_shutdown();
        (terminal, quiescent)
    }

    fn aggregate_diagnostic_failure(
        kind: KfdRuntimeBackendErrorKindV1,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        let detail = match kind {
            KfdRuntimeBackendErrorKindV1::Terminal => {
                "terminal XGMI aggregate capture is unavailable"
            }
            KfdRuntimeBackendErrorKindV1::Busy => {
                "XGMI aggregate diagnosis requires complete logical and native teardown"
            }
            KfdRuntimeBackendErrorKindV1::Unsupported => {
                "XGMI aggregate diagnosis was not enabled or was already consumed"
            }
            _ => "XGMI aggregate diagnostic capture is incomplete or invalid",
        };
        if kind == KfdRuntimeBackendErrorKindV1::Terminal {
            RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(kind, detail))
        } else {
            Self::rejected(kind, detail)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_timer_runs_each_operation_once_without_recording() {
        let mut timer = CallTimer::<false>::new();
        assert_eq!(timer.start(), None);
        timer.end(Phase::Admission, Some(Instant::now()));
        let mut calls = 0;
        assert_eq!(
            timer.measure(Phase::Opening, || {
                calls += 1;
                Err::<(), _>("original error")
            }),
            Err("original error")
        );
        assert_eq!(calls, 1);
        assert_eq!(timer.elapsed, [None; PHASE_COUNT]);
        assert_eq!(timer.visited, [false; PHASE_COUNT]);
    }

    #[test]
    fn enabled_missing_or_duplicate_span_is_permanently_unavailable() {
        let mut missing = CallTimer::<true>::new();
        missing.end(Phase::Preparation, None);
        let start = missing.start();
        missing.end(Phase::Preparation, start);
        assert_eq!(missing.elapsed[Phase::Preparation as usize], None);
        assert!(missing.visited[Phase::Preparation as usize]);

        let mut duplicate = CallTimer::<true>::new();
        let start = duplicate.start();
        duplicate.end(Phase::Settlement, start);
        assert!(duplicate.elapsed[Phase::Settlement as usize].is_some());
        let start = duplicate.start();
        duplicate.end(Phase::Settlement, start);
        assert_eq!(duplicate.elapsed[Phase::Settlement as usize], None);
        assert!(duplicate.visited[Phase::Settlement as usize]);
    }

    #[test]
    fn enabled_timer_preserves_results_and_all_phase_slots() {
        let mut timer = CallTimer::<true>::new();
        for (index, phase) in [
            Phase::Admission,
            Phase::Preparation,
            Phase::Opening,
            Phase::Submission,
            Phase::Wait,
            Phase::Closing,
            Phase::Settlement,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(timer.measure(phase, || index), index);
        }
        assert!(timer.elapsed.iter().all(Option::is_some));
        assert_eq!(timer.visited, [true; PHASE_COUNT]);
    }

    #[test]
    fn timer_preserves_error_and_original_panic_identity() {
        let mut timer = CallTimer::<true>::new();
        assert_eq!(
            timer.measure(Phase::Admission, || Err::<(), _>(37_u32)),
            Err(37)
        );
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            timer.measure(Phase::Wait, || std::panic::panic_any(53_u32));
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u32>(), Some(&53));
        assert!(!timer.visited[Phase::Wait as usize]);
    }

    #[test]
    fn duplicate_phase_is_permanently_unavailable() {
        let mut timer = CallTimer::<true>::new();
        timer.record(Phase::Closing, Some(7));
        timer.record(Phase::Closing, Some(8));
        timer.record(Phase::Closing, Some(9));
        assert_eq!(timer.elapsed[Phase::Closing as usize], None);
        assert!(timer.visited[Phase::Closing as usize]);
    }

    #[cfg(feature = "hardware-diagnostic")]
    fn identity(submission: u64) -> CallIdentity {
        CallIdentity {
            direction: (submission % 2) as usize,
            submission,
            published: false,
        }
    }

    #[cfg(feature = "hardware-diagnostic")]
    fn timing() -> KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
        KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
            admission_validation_ns: Some(2),
            preparation_ns: Some(3),
            opening_currentness_ns: Some(5),
            submission_ns: Some(7),
            wait_ns: Some(11),
            closing_currentness_ns: Some(13),
            settlement_ns: Some(17),
            total_ns: Some(61),
        }
    }

    #[cfg(feature = "hardware-diagnostic")]
    fn record(recorder: &mut Recorder, id: CallIdentity) {
        assert!(recorder.begin(id, 1));
        recorder.finish(id, Some(timing()), None);
    }

    #[cfg(feature = "hardware-diagnostic")]
    #[test]
    fn recorder_preallocates_exact_roster_and_preserves_identity_order() {
        let mut recorder = Recorder::new([71, 93], 3).unwrap();
        let pointer = recorder.records.aggregate().as_ptr();
        let capacity = recorder.records.capacity();
        for submission in [4, 7, 11] {
            record(&mut recorder, identity(submission));
        }
        assert!(recorder.complete());
        assert_eq!(recorder.records.aggregate().as_ptr(), pointer);
        assert_eq!(recorder.records.capacity(), capacity);
        assert!(capacity >= 3);
        assert_eq!(
            recorder
                .records
                .aggregate()
                .iter()
                .map(|record| record.backend_submission)
                .collect::<Vec<_>>(),
            [4, 7, 11]
        );
        assert_eq!(
            (
                recorder.records.aggregate()[1].source_device,
                recorder.records.aggregate()[1].destination_device
            ),
            (93, 71)
        );
        assert!(!recorder.begin(identity(12), 1));
        assert!(!recorder.complete());
        assert_eq!(recorder.records.aggregate().as_ptr(), pointer);
        assert_eq!(recorder.records.capacity(), capacity);
    }

    #[cfg(feature = "hardware-diagnostic")]
    #[test]
    fn recorder_rejects_unsupported_or_noncanonical_calls() {
        for (id, packets) in [
            (identity(1), 0),
            (identity(1), 2),
            (
                CallIdentity {
                    direction: 2,
                    ..identity(1)
                },
                1,
            ),
            (
                CallIdentity {
                    submission: 0,
                    ..identity(1)
                },
                1,
            ),
            (
                CallIdentity {
                    published: true,
                    ..identity(1)
                },
                1,
            ),
        ] {
            let mut recorder = Recorder::new([1, 2], 1).unwrap();
            assert!(!recorder.begin(id, packets));
            assert!(!recorder.complete());
        }

        let mut duplicate = Recorder::new([1, 2], 2).unwrap();
        record(&mut duplicate, identity(7));
        assert!(!duplicate.begin(identity(7), 1));
        assert!(!duplicate.complete());

        let mut descending = Recorder::new([1, 2], 2).unwrap();
        record(&mut descending, identity(7));
        assert!(!descending.begin(identity(6), 1));
        assert!(!descending.complete());
    }

    #[cfg(feature = "hardware-diagnostic")]
    #[test]
    fn errors_wrong_identity_and_incomplete_calls_never_record_success() {
        for case in 0..4 {
            let mut recorder = Recorder::new([1, 2], 1).unwrap();
            let id = identity(5);
            assert!(recorder.begin(id, 1));
            match case {
                0 => recorder.finish(id, None, None),
                1 => recorder.finish(identity(6), Some(timing()), None),
                2 => assert!(!recorder.begin(id, 1)),
                _ => {}
            }
            assert!(!recorder.complete());
            assert_eq!(recorder.records.len(), 0);
        }
    }

    #[cfg(feature = "hardware-diagnostic")]
    #[test]
    fn missing_overflow_and_impossible_totals_are_invalid() {
        assert!(valid_timing(timing()));
        assert!(valid_timing(KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
            total_ns: Some(58),
            ..timing()
        }));
        for case in 0..3 {
            let mut value = timing();
            match case {
                0 => value.wait_ns = None,
                1 => {
                    value.admission_validation_ns = Some(u64::MAX);
                    value.preparation_ns = Some(1);
                    value.total_ns = Some(u64::MAX);
                }
                _ => value.total_ns = Some(57),
            }
            assert!(!valid_timing(value));
            let mut recorder = Recorder::new([1, 2], 1).unwrap();
            let id = identity(3);
            assert!(recorder.begin(id, 1));
            recorder.finish(id, Some(value), None);
            assert!(!recorder.complete());
        }
        assert!(Recorder::new([1, 2], 0).is_err());
        assert!(Recorder::new([1, 2], MAX_RECORDS + 1).is_err());
    }

    #[cfg(feature = "hardware-diagnostic")]
    #[test]
    fn extraction_requires_complete_teardown_and_is_single_use() {
        let mut recorder = Recorder::new([71, 93], 1).unwrap();
        record(&mut recorder, identity(4));
        let mut slot = Some(recorder);
        for (terminal, shutdown, quiescent, error) in [
            (false, false, true, KfdRuntimeBackendErrorKindV1::Busy),
            (false, true, false, KfdRuntimeBackendErrorKindV1::Busy),
            (true, true, true, KfdRuntimeBackendErrorKindV1::Terminal),
        ] {
            assert_eq!(
                take_records(&mut slot, terminal, shutdown, quiescent),
                Err(error)
            );
            assert!(slot.is_some());
        }
        let records = take_records(&mut slot, false, true, true).unwrap();
        assert_eq!(records.len(), 1);
        assert!(slot.is_none());
        assert_eq!(
            take_records(&mut slot, false, true, true),
            Err(KfdRuntimeBackendErrorKindV1::Unsupported)
        );
    }
}

#[cfg(all(test, feature = "hardware-diagnostic"))]
mod currentness_tests;
