//! Bounded diagnostic capture, independent of completion and profiler authority.

use super::{
    ActiveSdmaCopyV1, DirectionalSdmaCompletedOwnerV1, KfdRuntimeBackendErrorKindV1,
    KfdRuntimeBackendErrorV1, KfdRuntimeBackendV1, RuntimeBackendFailureV1,
};
use fe2o3_kfd::{
    Gfx942PersistentSdmaDirectionV1, Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
    Gfx942SdmaPersistentWaitDiagnosticsV1,
};

const MAX_RECORDS: usize = 40_000;

/// Logical offsets and owner-free measurements, not authenticated completion evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeDirectionalWaitObservationV1 {
    pub backend_submission: u64,
    pub direction: Gfx942PersistentSdmaDirectionV1,
    pub completed_prefix_bytes: u64,
    pub host_offset: u64,
    pub device_offset: u64,
    pub window_bytes: u32,
    pub packet_count: usize,
    pub native: Gfx942SdmaPersistentWaitDiagnosticsV1,
}

impl KfdRuntimeDirectionalWaitObservationV1 {
    pub(super) fn new(
        active: &ActiveSdmaCopyV1,
        completed: &DirectionalSdmaCompletedOwnerV1,
        native: Gfx942SdmaPersistentWaitDiagnosticsV1,
    ) -> Self {
        Self {
            backend_submission: active.id,
            direction: completed.direction(),
            completed_prefix_bytes: active.completed_bytes,
            host_offset: completed.host_offset(),
            device_offset: completed.device_offset(),
            window_bytes: completed.copy_bytes(),
            packet_count: completed.packet_count(),
            native,
        }
    }
}

pub(super) struct DirectionalWaitRecorderV1 {
    policy: Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
    expected: usize,
    records: Vec<KfdRuntimeDirectionalWaitObservationV1>,
    incomplete: bool,
}

impl DirectionalWaitRecorderV1 {
    fn new(
        policy: Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
        expected: usize,
    ) -> Result<Self, &'static str> {
        if expected == 0 || expected > MAX_RECORDS {
            return Err("diagnostic record count outside 1..=40000");
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(expected)
            .map_err(|_| "diagnostic record reservation")?;
        Ok(Self {
            policy,
            expected,
            records,
            incomplete: false,
        })
    }

    pub(super) fn policy(&self) -> Gfx942SdmaPersistentDiagnosticSleepCeilingV1 {
        self.policy
    }

    pub(super) fn record(&mut self, observed: Option<KfdRuntimeDirectionalWaitObservationV1>) {
        let Some(observed) = observed else {
            self.incomplete = true;
            return;
        };
        if self.records.len() >= self.expected || observed.native.sleep_ceiling != self.policy {
            self.incomplete = true;
            return;
        }
        // Admission reserved this exact bound; observation never allocates or changes status.
        self.records.push(observed);
    }

    fn complete(&self) -> bool {
        !self.incomplete && self.records.len() == self.expected
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn record_directional_wait_settlement_v1(
        &mut self,
        settled: bool,
        observation: Option<KfdRuntimeDirectionalWaitObservationV1>,
    ) {
        if settled && let Some(recorder) = self.directional_wait_diagnostic.as_mut() {
            recorder.record(observation);
        }
    }

    /// Enables a closed diagnostic before any logical resource or native queue exists.
    /// Unsupported copy shapes still complete normally but invalidate this capture.
    pub fn enable_directional_sdma_wait_diagnostics_v1(
        &mut self,
        policy: Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
        expected_records: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.directional_wait_diagnostic.is_some()
            || self.next_handle != 1
            || self.queue_retired
            || self.queue.is_some()
            || !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || !self.submissions.is_empty()
            || !self.events.is_empty()
            || self.any_compute_active_v1()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "directional wait diagnosis must begin before runtime resource creation",
            ));
        }
        let recorder =
            DirectionalWaitRecorderV1::new(policy, expected_records).map_err(Self::capacity)?;
        self.directional_wait_diagnostic = Some(recorder);
        Ok(())
    }

    /// Consumes only a complete capture after logical and native teardown.
    /// This is not the authenticated runtime profiler and conveys no authority.
    pub fn finish_directional_sdma_wait_diagnostics_v1(
        &mut self,
    ) -> Result<
        Vec<KfdRuntimeDirectionalWaitObservationV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        self.require_live()?;
        if !self.queue_retired
            || self.queue.is_some()
            || !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || !self.submissions.is_empty()
            || !self.events.is_empty()
            || self.any_compute_active_v1()
            || !self.active_sdma.is_empty()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "directional wait diagnosis requires complete logical and native teardown",
            ));
        }
        let recorder = self.directional_wait_diagnostic.as_ref().ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "directional wait diagnosis was not enabled",
            )
        })?;
        if !recorder.complete() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "directional wait diagnostic capture is incomplete or invalid",
            ));
        }
        Ok(self
            .directional_wait_diagnostic
            .take()
            .expect("validated diagnostic recorder")
            .records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kfd::Gfx942SdmaPersistentWaitCpuV1;

    fn rejected<T>(
        result: Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
        kind: KfdRuntimeBackendErrorKindV1,
    ) {
        assert!(
            matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == kind)
        );
    }

    fn observation() -> KfdRuntimeDirectionalWaitObservationV1 {
        KfdRuntimeDirectionalWaitObservationV1 {
            backend_submission: 1,
            direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
            completed_prefix_bytes: 0,
            host_offset: 0,
            device_offset: 0,
            window_bytes: 8,
            packet_count: 1,
            native: Gfx942SdmaPersistentWaitDiagnosticsV1 {
                sleep_ceiling: Gfx942SdmaPersistentDiagnosticSleepCeilingV1::Micros25,
                packet_count: 1,
                counters: None,
                scan_ns: None,
                cpu: Gfx942SdmaPersistentWaitCpuV1::Unavailable,
            },
        }
    }

    #[test]
    fn recorder_admission_is_bounded_and_append_does_not_reallocate() {
        let policy = observation().native.sleep_ceiling;
        assert!(DirectionalWaitRecorderV1::new(policy, 0).is_err());
        assert!(DirectionalWaitRecorderV1::new(policy, MAX_RECORDS + 1).is_err());
        let mut recorder = DirectionalWaitRecorderV1::new(policy, 2).unwrap();
        let pointer = recorder.records.as_ptr();
        let capacity = recorder.records.capacity();
        assert!(!recorder.complete());
        recorder.record(Some(observation()));
        assert!(!recorder.complete());
        recorder.record(Some(observation()));
        assert!(recorder.complete());
        recorder.record(Some(observation()));
        assert!(!recorder.complete());
        assert_eq!(recorder.records.len(), 2);
        assert_eq!(recorder.records.capacity(), capacity);
        assert_eq!(recorder.records.as_ptr(), pointer);
    }

    #[test]
    fn missing_or_wrong_policy_observations_permanently_invalidate_capture() {
        for missing in [true, false] {
            let observed = observation();
            let mut recorder =
                DirectionalWaitRecorderV1::new(observed.native.sleep_ceiling, 1).unwrap();
            let mut wrong = observed;
            wrong.native.sleep_ceiling = Gfx942SdmaPersistentDiagnosticSleepCeilingV1::Millis1;
            recorder.record(if missing { None } else { Some(wrong) });
            recorder.record(Some(observed));
            assert_eq!(recorder.records.len(), 1);
            assert!(!recorder.complete());
        }
    }

    #[test]
    fn extraction_requires_teardown_and_is_single_use() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let observed = observation();
        rejected(
            backend.finish_directional_sdma_wait_diagnostics_v1(),
            KfdRuntimeBackendErrorKindV1::Busy,
        );
        backend
            .enable_directional_sdma_wait_diagnostics_v1(observed.native.sleep_ceiling, 1)
            .unwrap();
        rejected(
            backend.enable_directional_sdma_wait_diagnostics_v1(observed.native.sleep_ceiling, 1),
            KfdRuntimeBackendErrorKindV1::Busy,
        );
        backend
            .directional_wait_diagnostic
            .as_mut()
            .unwrap()
            .record(Some(observed));
        rejected(
            backend.finish_directional_sdma_wait_diagnostics_v1(),
            KfdRuntimeBackendErrorKindV1::Busy,
        );
        backend.shutdown_native_v1().unwrap();
        assert_eq!(
            backend
                .finish_directional_sdma_wait_diagnostics_v1()
                .unwrap(),
            [observed]
        );
        rejected(
            backend.finish_directional_sdma_wait_diagnostics_v1(),
            KfdRuntimeBackendErrorKindV1::Unsupported,
        );
        rejected(
            backend.enable_directional_sdma_wait_diagnostics_v1(observed.native.sleep_ceiling, 1),
            KfdRuntimeBackendErrorKindV1::Busy,
        );
    }

    #[test]
    fn incomplete_capture_stays_unavailable_after_teardown() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .enable_directional_sdma_wait_diagnostics_v1(observation().native.sleep_ceiling, 1)
            .unwrap();
        backend.shutdown_native_v1().unwrap();
        rejected(
            backend.finish_directional_sdma_wait_diagnostics_v1(),
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
        );
        assert!(backend.directional_wait_diagnostic.is_some());
    }

    #[test]
    fn bad_capacity_does_not_install_a_recorder() {
        let mut backend = KfdRuntimeBackendV1::mock();
        for count in [0, MAX_RECORDS + 1] {
            rejected(
                backend.enable_directional_sdma_wait_diagnostics_v1(
                    observation().native.sleep_ceiling,
                    count,
                ),
                KfdRuntimeBackendErrorKindV1::Capacity,
            );
            assert!(backend.directional_wait_diagnostic.is_none());
        }
        backend.shutdown_native_v1().unwrap();
    }
}
