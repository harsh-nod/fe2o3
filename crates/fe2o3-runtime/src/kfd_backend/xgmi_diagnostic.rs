//! Bounded, owner-free diagnostics. Never consulted for execution authority.

use super::{
    KfdNativeXgmiRuntimeBackendV1, KfdRuntimeBackendErrorKindV1, KfdRuntimeBackendErrorV1,
    RuntimeBackendFailureV1,
};
use fe2o3_kfd::{Gfx942NativeXgmiSdmaQueueV1, Gfx942XgmiCopyCallDiagnosticsV1};

const MAX_RECORDS: usize = 40_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdRuntimeXgmiDiagnosticCallV1 {
    Submit,
    Pending,
    Completed,
}

/// One host call after returned custody has been restored to the runtime.
/// This single-packet diagnostic is not the authenticated runtime profiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeXgmiCallObservationV1 {
    pub backend_submission: u64,
    pub source_device: u64,
    pub destination_device: u64,
    pub call: KfdRuntimeXgmiDiagnosticCallV1,
    pub native: Gfx942XgmiCopyCallDiagnosticsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CallIdentity {
    pub direction: usize,
    pub submission: u64,
    pub submit: bool,
}

pub(super) struct Recorder {
    devices: [u64; 2],
    expected: usize,
    limit: usize,
    records: Vec<KfdRuntimeXgmiCallObservationV1>,
    submitted: usize,
    completed: usize,
    in_flight: [Option<u64>; 2],
    pending: Option<CallIdentity>,
    invalid: bool,
}

fn valid_timing(value: Gfx942XgmiCopyCallDiagnosticsV1, submit: bool) -> bool {
    let total = (|| {
        let mut stages = value
            .opening_currentness_ns?
            .checked_add(value.native_call_ns?)?
            .checked_add(value.closing_currentness_ns?)?;
        if submit {
            stages = stages.checked_add(value.preparation_ns?)?;
        } else if value.preparation_ns.is_some() {
            return None;
        }
        Some(stages <= value.total_ns?)
    })();
    total == Some(true)
}

impl Recorder {
    pub(super) fn invalidate(&mut self) {
        self.invalid = true;
    }

    fn new(devices: [u64; 2], expected: usize, limit: usize) -> Result<Self, &'static str> {
        if expected == 0 || limit > MAX_RECORDS || expected.checked_mul(2).is_none_or(|n| n > limit)
        {
            return Err(
                "XGMI diagnostic requires 1..=20000 single-packet submissions and a bounded call roster",
            );
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(limit)
            .map_err(|_| "XGMI diagnostic record reservation")?;
        Ok(Self {
            devices,
            expected,
            limit,
            records,
            submitted: 0,
            completed: 0,
            in_flight: [None; 2],
            pending: None,
            invalid: false,
        })
    }

    /// Arming before the call makes an unwind leave the capture incomplete.
    /// Unsupported shapes execute normally and invalidate only this capture.
    pub(super) fn begin(&mut self, identity: CallIdentity, packets: usize) -> bool {
        if self.invalid
            || self.pending.is_some()
            || packets != 1
            || identity.direction >= 2
            || identity.submission == 0
            || self.records.len() >= self.limit
        {
            self.invalid = true;
            return false;
        }
        let live = self.in_flight[identity.direction];
        if (identity.submit && (live.is_some() || self.submitted == self.expected))
            || (!identity.submit && live != Some(identity.submission))
        {
            self.invalid = true;
            return false;
        }
        self.pending = Some(identity);
        true
    }

    /// Must be called only after native tickets/mappings have been rooted or
    /// settled. A diagnostic failure must never alter a workload's result.
    pub(super) fn finish(
        &mut self,
        identity: CallIdentity,
        observed: Option<(
            KfdRuntimeXgmiDiagnosticCallV1,
            Gfx942XgmiCopyCallDiagnosticsV1,
        )>,
    ) {
        let Some((call, native)) = observed else {
            self.invalid = true;
            return;
        };
        if self.invalid
            || self.pending != Some(identity)
            || identity.submit != (call == KfdRuntimeXgmiDiagnosticCallV1::Submit)
            || !valid_timing(native, identity.submit)
        {
            self.invalid = true;
            return;
        }
        self.pending = None;
        match call {
            KfdRuntimeXgmiDiagnosticCallV1::Submit => {
                self.submitted += 1;
                self.in_flight[identity.direction] = Some(identity.submission);
            }
            KfdRuntimeXgmiDiagnosticCallV1::Completed => {
                self.completed += 1;
                self.in_flight[identity.direction] = None;
            }
            KfdRuntimeXgmiDiagnosticCallV1::Pending => {}
        }
        // Admission reserved the entire bounded roster. This append cannot grow it.
        self.records.push(KfdRuntimeXgmiCallObservationV1 {
            backend_submission: identity.submission,
            source_device: self.devices[identity.direction],
            destination_device: self.devices[1 - identity.direction],
            call,
            native,
        });
    }

    fn complete(&self) -> bool {
        !self.invalid
            && self.pending.is_none()
            && self.in_flight == [None; 2]
            && self.submitted == self.expected
            && self.completed == self.expected
    }
}

fn take_records(
    slot: &mut Option<Recorder>,
    terminal: bool,
    shutdown: bool,
    quiescent: bool,
) -> Result<Vec<KfdRuntimeXgmiCallObservationV1>, KfdRuntimeBackendErrorKindV1> {
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
    Ok(slot.take().expect("validated XGMI recorder").records)
}

impl KfdNativeXgmiRuntimeBackendV1 {
    /// Enables bounded host attribution before any runtime resource exists.
    /// Each submitted batch must contain one packet. Larger batches and native
    /// errors still follow ordinary semantics, but make the capture unavailable.
    pub fn enable_xgmi_copy_diagnostics_v1(
        &mut self,
        expected_submissions: usize,
        max_call_records: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.xgmi_diagnostic.is_some()
            || self.xgmi_aggregate_diagnostic.is_some()
            || self.xgmi_segments_diagnostic.is_some()
            || self.next_handle != 1
            || self.queues.iter().any(Option::is_some)
            || !self.logical_resource_counts().permits_shutdown()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "XGMI diagnosis must begin before resource creation",
            ));
        }
        let devices = self
            .descriptions
            .each_ref()
            .map(|device| device.backend_device);
        let recorder = Recorder::new(devices, expected_submissions, max_call_records)
            .map_err(|error| Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, error))?;
        self.xgmi_diagnostic = Some(recorder);
        Ok(())
    }

    /// Takes one complete capture only after successful logical/native shutdown.
    /// No durations authorize reuse or resource release; all timing is host-only.
    pub fn finish_xgmi_copy_diagnostics_v1(
        &mut self,
    ) -> Result<
        Vec<KfdRuntimeXgmiCallObservationV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
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
        take_records(
            &mut self.xgmi_diagnostic,
            terminal,
            self.shutdown,
            quiescent,
        )
        .map_err(|kind| {
            let detail = match kind {
                KfdRuntimeBackendErrorKindV1::Terminal => "terminal XGMI capture is unavailable",
                KfdRuntimeBackendErrorKindV1::Busy => {
                    "XGMI diagnosis requires complete logical and native teardown"
                }
                KfdRuntimeBackendErrorKindV1::Unsupported => {
                    "XGMI diagnosis was not enabled or was already consumed"
                }
                _ => "XGMI diagnostic capture is incomplete or invalid",
            };
            if kind == KfdRuntimeBackendErrorKindV1::Terminal {
                RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(kind, detail))
            } else {
                Self::rejected(kind, detail)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(direction: usize, submission: u64, submit: bool) -> CallIdentity {
        CallIdentity {
            direction,
            submission,
            submit,
        }
    }

    fn timing(submit: bool) -> Gfx942XgmiCopyCallDiagnosticsV1 {
        Gfx942XgmiCopyCallDiagnosticsV1 {
            opening_currentness_ns: Some(10),
            preparation_ns: submit.then_some(2),
            native_call_ns: Some(3),
            closing_currentness_ns: Some(11),
            total_ns: Some(30),
        }
    }

    fn record(recorder: &mut Recorder, id: CallIdentity, call: KfdRuntimeXgmiDiagnosticCallV1) {
        assert!(recorder.begin(id, 1));
        recorder.finish(id, Some((call, timing(id.submit))));
    }

    #[test]
    fn exact_two_direction_roster_preserves_identity_without_reallocation() {
        use KfdRuntimeXgmiDiagnosticCallV1::*;
        let mut recorder = Recorder::new([71, 93], 2, 5).unwrap();
        let pointer = recorder.records.as_ptr();
        let capacity = recorder.records.capacity();
        record(&mut recorder, identity(0, 4, true), Submit);
        record(&mut recorder, identity(1, 7, true), Submit);
        record(&mut recorder, identity(0, 4, false), Pending);
        record(&mut recorder, identity(1, 7, false), Completed);
        assert!(!recorder.complete());
        record(&mut recorder, identity(0, 4, false), Completed);
        assert!(recorder.complete());
        assert_eq!(recorder.records.as_ptr(), pointer);
        assert_eq!(recorder.records.capacity(), capacity);
        assert_eq!(
            (
                recorder.records[1].source_device,
                recorder.records[1].destination_device
            ),
            (93, 71)
        );
        assert_eq!(recorder.records[4].backend_submission, 4);
        assert!(!recorder.begin(identity(0, 8, true), 1));
        assert!(!recorder.complete());
        assert_eq!(recorder.records.len(), 5);
    }

    #[test]
    fn unsupported_aggregate_invalidates_even_a_previously_complete_capture() {
        use KfdRuntimeXgmiDiagnosticCallV1::*;
        let mut recorder = Recorder::new([71, 93], 1, 2).unwrap();
        record(&mut recorder, identity(0, 4, true), Submit);
        record(&mut recorder, identity(0, 4, false), Completed);
        assert!(recorder.complete());
        recorder.invalidate();
        assert!(!recorder.complete());
        assert_eq!(
            take_records(&mut Some(recorder), false, true, true),
            Err(KfdRuntimeBackendErrorKindV1::InvalidLaunch)
        );
    }

    #[test]
    fn admission_and_unsupported_shape_are_bounded() {
        for (expected, limit) in [(0, 2), (1, 1), (1, MAX_RECORDS + 1), (usize::MAX, 2)] {
            assert!(Recorder::new([1, 2], expected, limit).is_err());
        }
        for (id, packets) in [
            (identity(0, 1, true), 2),
            (identity(2, 1, true), 1),
            (identity(0, 0, true), 1),
            (identity(0, 1, false), 1),
        ] {
            let mut recorder = Recorder::new([1, 2], 1, 2).unwrap();
            assert!(!recorder.begin(id, packets));
            assert!(!recorder.begin(identity(0, 1, true), 1));
            assert!(!recorder.complete());
        }
    }

    #[test]
    fn errors_unwinds_and_wrong_identity_never_yield_a_complete_capture() {
        for case in 0..4 {
            let mut recorder = Recorder::new([1, 2], 1, 3).unwrap();
            let id = identity(0, 1, true);
            assert!(recorder.begin(id, 1));
            match case {
                0 => recorder.finish(id, None),
                1 => {
                    assert!(!recorder.begin(id, 1));
                }
                2 => recorder.finish(
                    identity(1, 1, true),
                    Some((KfdRuntimeXgmiDiagnosticCallV1::Submit, timing(true))),
                ),
                _ => recorder.finish(
                    id,
                    Some((KfdRuntimeXgmiDiagnosticCallV1::Completed, timing(false))),
                ),
            }
            assert!(!recorder.complete());
            assert!(recorder.records.is_empty());
        }
    }

    #[test]
    fn invalid_duration_is_not_zero_or_silently_accepted() {
        assert!(valid_timing(timing(true), true));
        assert!(valid_timing(timing(false), false));
        for case in 0..6 {
            let mut value = timing(true);
            match case {
                0 => value.opening_currentness_ns = None,
                1 => value.preparation_ns = None,
                2 => value.native_call_ns = None,
                3 => value.closing_currentness_ns = Some(u64::MAX),
                4 => value.total_ns = Some(25),
                _ => value.total_ns = None,
            }
            assert!(!valid_timing(value, true));
        }
        assert!(!valid_timing(timing(true), false));
    }

    #[test]
    fn pending_exhaustion_and_foreign_poll_invalidate_without_growing_storage() {
        use KfdRuntimeXgmiDiagnosticCallV1::*;
        for foreign in [false, true] {
            let mut recorder = Recorder::new([71, 93], 1, 2).unwrap();
            let pointer = recorder.records.as_ptr();
            record(&mut recorder, identity(0, 4, true), Submit);
            if !foreign {
                record(&mut recorder, identity(0, 4, false), Pending);
            }
            assert!(!recorder.begin(identity(0, if foreign { 5 } else { 4 }, false), 1));
            assert!(!recorder.complete());
            assert_eq!(recorder.records.as_ptr(), pointer);
            assert!(recorder.records.len() <= 2);
        }
    }

    #[test]
    fn extraction_requires_shutdown_quiescence_and_is_single_use() {
        use KfdRuntimeXgmiDiagnosticCallV1::*;
        let mut recorder = Recorder::new([71, 93], 1, 2).unwrap();
        record(&mut recorder, identity(0, 4, true), Submit);
        record(&mut recorder, identity(0, 4, false), Completed);
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
        assert_eq!(take_records(&mut slot, false, true, true).unwrap().len(), 2);
        assert!(slot.is_none());
        assert_eq!(
            take_records(&mut slot, false, true, true),
            Err(KfdRuntimeBackendErrorKindV1::Unsupported)
        );
        let mut incomplete = Some(Recorder::new([71, 93], 1, 2).unwrap());
        assert_eq!(
            take_records(&mut incomplete, false, true, true),
            Err(KfdRuntimeBackendErrorKindV1::InvalidLaunch)
        );
        assert!(incomplete.is_some());
    }

    #[test]
    fn real_unwind_leaves_pending_and_cannot_be_rehabilitated() {
        let mut recorder = Recorder::new([71, 93], 1, 3).unwrap();
        let id = identity(0, 4, true);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert!(recorder.begin(id, 1));
            std::panic::panic_any(71_u32);
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u32>(), Some(&71));
        assert_eq!(recorder.pending, Some(id));
        assert!(!recorder.complete());
        assert!(!recorder.begin(id, 1));
        recorder.finish(
            id,
            Some((KfdRuntimeXgmiDiagnosticCallV1::Submit, timing(true))),
        );
        assert!(!recorder.complete());
        assert!(recorder.records.is_empty());
    }

    #[test]
    fn production_records_only_after_all_native_custody_branches_settle() {
        // This guards wiring, not live driver behavior or Rust/native refinement.
        let source = include_str!("../kfd_backend.rs");
        let publish = source
            .split("    fn publish_ready_peer_batch(")
            .nth(1)
            .unwrap()
            .split("    fn progress_peer_copy(")
            .next()
            .unwrap();
        let begin = publish.find("recorder.begin(").unwrap();
        let native = publish.find(".submit_batch_diagnostic_v1(").unwrap();
        let finish = publish.find("recorder.finish(").unwrap();
        assert!(begin < native && native < finish);
        assert_eq!(publish.matches("recorder.finish(").count(), 1);
        assert_eq!(
            publish
                .matches("self.active.insert(active.id, active)")
                .count(),
            4
        );
        for required in [
            "active.ticket = Some(ticket)",
            "self.active.insert(active.id, active)",
            "self.restore_mapped_copy_pair(",
            "self.finish_failed(active)",
            "self.terminal_error(",
        ] {
            assert!(publish.rfind(required).unwrap() < finish);
        }
        let poll = source
            .split("    fn progress_peer_copy(")
            .nth(1)
            .unwrap()
            .split("    fn logical_resource_counts(")
            .next()
            .unwrap();
        let begin = poll.find("recorder.begin(").unwrap();
        let native = poll.find(".poll_diagnostic_v1(").unwrap();
        let finish = poll.find("recorder.finish(").unwrap();
        assert!(begin < native && native < finish);
        assert_eq!(poll.matches("recorder.finish(").count(), 1);
        let settlement = &poll[native..finish];
        for required in [
            "active.ticket = Some(ticket)",
            "self.active.insert(active.id, active)",
            "self.restore_mapped_copy_pair(",
            "self.settle_submission(active, status)",
            "self.quarantine_mapping(active.source, source)",
            "self.quarantine_mapping(active.destination, destination)",
        ] {
            assert!(settlement.contains(required));
        }
    }
}
