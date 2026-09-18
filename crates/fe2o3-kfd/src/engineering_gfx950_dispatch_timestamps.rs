//! Explicit serial-only native diagnostics; never a protected capability.

use super::*;
use fe2o3_aql::{
    AMD_QUEUE_PROPERTIES_OFFSET_V1, AmdQueueProfilingPolicyV1, GpuSystemClockBracketV1,
    GpuSystemClockSampleV1, parse_amd_busy_dispatch_timestamp_snapshot_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Identity {
    device: u64,
    gpu: u32,
    epoch: u64,
    packet: u64,
    generation: u64,
}

pub(super) struct PendingTimestamp {
    identity: Identity,
    before: DispatchClockSampleV1,
}

pub(super) struct CanaryOwner {
    pub(super) policy: AmdQueueProfilingPolicyV1,
    generation: u64,
    active: Option<Identity>,
    receipt: Option<DispatchTimestampObservationV1>,
}

impl CanaryOwner {
    pub(super) fn new(policy: AmdQueueProfilingPolicyV1) -> Self {
        Self {
            policy,
            generation: 0,
            active: None,
            receipt: None,
        }
    }

    fn begin(&mut self, device: u64, gpu: u32, epoch: u64, packet: u64) -> Result<Identity> {
        if self.active.is_some() || self.receipt.is_some() {
            return Err("timestamp generation is pending or unconsumed".into());
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("timestamp generation exhausted")?;
        let identity = Identity {
            device,
            gpu,
            epoch,
            packet,
            generation: self.generation,
        };
        self.active = Some(identity);
        Ok(identity)
    }

    fn require_active(&self, identity: Identity) -> Result<()> {
        if self.active != Some(identity) || self.generation != identity.generation {
            return Err("timestamp owner identity changed".into());
        }
        Ok(())
    }

    fn finish(
        &mut self,
        pending: &PendingTimestamp,
        raw: [u8; 64],
        after: DispatchClockSampleV1,
    ) -> Result<()> {
        self.require_active(pending.identity)?;
        let receipt = checked_receipt(self.policy, pending, raw, after)?;
        self.active = None;
        self.receipt = Some(receipt);
        Ok(())
    }

    fn take(&mut self, expected: [u64; 3]) -> Result<DispatchTimestampObservationV1> {
        let receipt = self
            .receipt
            .as_ref()
            .ok_or("no completed timestamp observation")?;
        if self.active.is_some()
            || [
                receipt.queue_epoch,
                receipt.packet_id,
                receipt.signal_generation,
            ] != expected
            || receipt.signal_generation != self.generation
        {
            return Err("timestamp observation identity mismatch".into());
        }
        self.receipt
            .take()
            .ok_or("timestamp observation already consumed".into())
    }
}

pub(super) fn apply_queue_policy(
    bytes: &mut [u8],
    policy: AmdQueueProfilingPolicyV1,
) -> Result<()> {
    if bytes.len() != PAGE_BYTES {
        return Err("timestamp control-page extent".into());
    }
    let range = AMD_QUEUE_PROPERTIES_OFFSET_V1..AMD_QUEUE_PROPERTIES_OFFSET_V1 + 4;
    let supplied = u32::from_le_bytes(bytes[range.clone()].try_into().map_err(explain)?);
    bytes[range].copy_from_slice(&policy.properties(supplied).to_le_bytes());
    Ok(())
}

fn clock_sample(value: crate::KfdClockCorrelationObservationV1) -> DispatchClockSampleV1 {
    DispatchClockSampleV1 {
        gpu_ticks: value.gpu_clock_counter(),
        cpu_ticks: value.cpu_clock_counter(),
        system_ticks: value.system_clock_counter(),
        system_frequency_hz: value.system_clock_frequency_hz(),
        gpu_id: value.gpu_id(),
    }
}

fn clock_value(value: DispatchClockSampleV1) -> Result<GpuSystemClockSampleV1> {
    GpuSystemClockSampleV1::new(
        value.gpu_ticks,
        value.system_ticks,
        value.system_frequency_hz,
    )
    .map_err(explain)
}

fn checked_receipt(
    policy: AmdQueueProfilingPolicyV1,
    pending: &PendingTimestamp,
    raw: [u8; 64],
    after: DispatchClockSampleV1,
) -> Result<DispatchTimestampObservationV1> {
    let identity = pending.identity;
    if pending.before.gpu_id != identity.gpu || after.gpu_id != identity.gpu {
        return Err("timestamp clock device substitution".into());
    }
    let bracket = GpuSystemClockBracketV1::new(clock_value(pending.before)?, clock_value(after)?)
        .map_err(explain)?;
    let enabled = policy == AmdQueueProfilingPolicyV1::EnableDispatchTimestamps;
    let correlated = if enabled {
        let ticks = parse_amd_busy_dispatch_timestamp_snapshot_v1(&raw)
            .map_err(|error| format!("rejected timestamp snapshot: {error:?}; raw={raw:?}"))?;
        let interval = bracket.interpolate(ticks).map_err(explain)?;
        Some(DispatchCorrelatedIntervalV1 {
            start_system_ticks: interval.start_system_ticks(),
            end_system_ticks: interval.end_system_ticks(),
            system_frequency_hz: interval.system_frequency_hz(),
            duration_ns_floor: interval.duration_ns_floor(),
        })
    } else {
        let mut expected = [0; 64];
        expected[0] = 1;
        if raw != expected {
            return Err(format!("rejected profile-off signal snapshot: raw={raw:?}"));
        }
        None
    };
    Ok(DispatchTimestampObservationV1 {
        authority: "none".into(),
        queue_profiling_enabled: enabled,
        queue_properties: policy.properties(0),
        device_unique_id: identity.device,
        gpu_id: identity.gpu,
        queue_epoch: identity.epoch,
        packet_id: identity.packet,
        signal_slot: 0,
        signal_generation: identity.generation,
        acquired_value: 0,
        signal_snapshot: [
            raw[..32].try_into().map_err(explain)?,
            raw[32..].try_into().map_err(explain)?,
        ],
        before: pending.before,
        after,
        correlated,
    })
}

fn admit_command(owner: Option<&CanaryOwner>, command: &CommandV1) -> Result<()> {
    if owner.is_some()
        && matches!(
            command,
            CommandV1::DispatchSequence { .. }
                | CommandV1::DispatchOrderedBatch { .. }
                | CommandV1::DispatchFullForward { .. }
                | CommandV1::RolloverQueue { .. }
                | CommandV1::ConfigurePerformance { .. }
                | CommandV1::PerformanceSnapshot
        )
    {
        return Err("timestamp canary admits only individual serial dispatches".into());
    }
    if owner.is_some_and(|owner| owner.receipt.is_some())
        && !matches!(command, CommandV1::TakeDispatchTimestampObservation { .. })
    {
        return Err("consume exact timestamp observation before another command".into());
    }
    Ok(())
}

impl Context {
    pub(super) fn admit_timestamp_command(&self, command: &CommandV1) -> Result<()> {
        admit_command(self.timestamp_canary.as_ref(), command)
    }

    pub(super) fn begin_timestamp_dispatch(
        &mut self,
        packet: u64,
    ) -> Result<Option<PendingTimestamp>> {
        let Some(owner) = self.timestamp_canary.as_mut() else {
            return Ok(None);
        };
        let properties =
            Backend::observe_engineering_queue_properties(&mut self.internal[CONTROL].mapping)
                .map_err(explain)?;
        if properties != owner.policy.properties(0) {
            return Err("timestamp queue properties changed".into());
        }
        let identity = owner.begin(
            self.unique_id,
            self.backend.gpu_id(),
            self.queue_epoch,
            packet,
        )?;
        Backend::reset_engineering_timestamp_signal_release(&mut self.internal[SIGNAL].mapping)
            .map_err(explain)?;
        let before = clock_sample(
            self.backend
                .observe_engineering_clock_correlation()
                .map_err(explain)?,
        );
        clock_value(before)?;
        if before.gpu_id != identity.gpu {
            return Err("timestamp before-clock device changed".into());
        }
        Ok(Some(PendingTimestamp { identity, before }))
    }

    pub(super) fn require_timestamp_pending(
        &self,
        pending: &PendingTimestamp,
        next: u64,
    ) -> Result<()> {
        let identity = pending.identity;
        if identity.device != self.unique_id
            || identity.gpu != self.backend.gpu_id()
            || identity.epoch != self.queue_epoch
            || identity.packet.checked_add(1) != Some(next)
            || self.ring.write() != next
        {
            return Err("timestamp pending queue identity changed".into());
        }
        self.timestamp_canary
            .as_ref()
            .ok_or("timestamp mode disappeared")?
            .require_active(identity)
    }

    pub(super) fn finish_timestamp_dispatch(
        &mut self,
        pending: &PendingTimestamp,
        next: u64,
        raw: [u8; 64],
    ) -> Result<()> {
        self.require_timestamp_pending(pending, next)?;
        let after = clock_sample(
            self.backend
                .observe_engineering_clock_correlation()
                .map_err(explain)?,
        );
        self.require_timestamp_pending(pending, next)?;
        let properties =
            Backend::observe_engineering_queue_properties(&mut self.internal[CONTROL].mapping)
                .map_err(explain)?;
        if properties
            != self
                .timestamp_canary
                .as_ref()
                .ok_or("timestamp mode disappeared")?
                .policy
                .properties(0)
        {
            return Err("timestamp queue properties changed".into());
        }
        self.timestamp_canary
            .as_mut()
            .ok_or("timestamp mode disappeared")?
            .finish(pending, raw, after)
    }

    pub(super) fn take_timestamp_observation(&mut self, expected: [u64; 3]) -> Result<ResponseV1> {
        if self.timestamp_canary.is_none() {
            return Err("timestamp canary was not selected".into());
        }
        self.check_currentness(true)?;
        self.check_idle()?;
        let observation = self
            .timestamp_canary
            .as_mut()
            .ok_or("timestamp mode disappeared")?
            .take(expected)?;
        if observation.device_unique_id != self.unique_id
            || observation.gpu_id != self.backend.gpu_id()
            || observation.queue_epoch != self.queue_epoch
            || observation.packet_id.checked_add(1) != Some(self.completed_write)
        {
            return Err("timestamp receipt no longer matches its queue".into());
        }
        Ok(ResponseV1::DispatchTimestampObservation { observation })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the pinned existing residual HSACO; host metadata admission only, no GPU"]
    fn timestamp_canary_pinned_residual_metadata() {
        let path = std::env::var_os("FE2O3_TIMESTAMP_CANARY_HSACO").expect("pinned HSACO path");
        let object = std::fs::read(path).unwrap();
        let hash: [u8; 32] = Sha256::digest(&object).into();
        assert_eq!(
            hash.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "6b0889e2834bda81313b3cbda5a60207dd9fa41c40ea80251bbec89d7befef2c"
        );
        let closure = fe2o3_amdhsa_loader::validate(&object, AdmittedProfile::Gfx950XnackOffCov6)
            .unwrap()
            .bind_kernel("ferric_qwen3_tp_batch_residual_bf16_v3")
            .unwrap();
        assert_eq!(
            closure.resources().required_workgroup_size(),
            Some([64, 1, 1])
        );
        let metadata = describe_kernel(closure.selected_kernel(), hash).unwrap();
        println!(
            "TIMESTAMP_CANARY_METADATA={}",
            serde_json::to_string(&metadata).unwrap()
        );
    }

    fn sample(gpu: u64, system: u64) -> DispatchClockSampleV1 {
        DispatchClockSampleV1 {
            gpu_ticks: gpu,
            cpu_ticks: 999,
            system_ticks: system,
            system_frequency_hz: 1000,
            gpu_id: 7,
        }
    }

    #[test]
    fn explicit_canary_is_serial_only_and_legacy_commands_stay_admitted() {
        let owner = CanaryOwner::new(AmdQueueProfilingPolicyV1::Preserve);
        for command in [
            CommandV1::DispatchSequence { dispatches: vec![] },
            CommandV1::DispatchOrderedBatch {
                dispatches: vec![],
                timeout_ms: 1,
            },
            CommandV1::DispatchFullForward {
                dispatch_count: 616,
                plan_bytes: 1,
                kernarg_bytes: 1,
                timeout_ms: 1,
            },
            CommandV1::RolloverQueue {
                expected_epoch: 0,
                expected_completed_packets: 0,
            },
            CommandV1::ConfigurePerformance {
                cache_kernel_admission: false,
                operational_currentness: false,
                profile: true,
            },
            CommandV1::PerformanceSnapshot,
        ] {
            assert!(admit_command(Some(&owner), &command).is_err());
            assert!(admit_command(None, &command).is_ok());
        }
        let (mut owner, pending, raw) = fixture(AmdQueueProfilingPolicyV1::Preserve);
        owner.finish(&pending, raw, sample(20, 200)).unwrap();
        assert!(admit_command(Some(&owner), &CommandV1::Close).is_err());
        assert!(admit_command(Some(&owner), &CommandV1::Allocate { bytes: 4 }).is_err());
        assert!(
            admit_command(
                Some(&owner),
                &CommandV1::TakeDispatchTimestampObservation {
                    expected_queue_epoch: 4,
                    expected_packet_id: 8,
                    expected_signal_generation: 1
                }
            )
            .is_ok()
        );
    }

    #[test]
    fn serial_owner_orders_acquire_snapshot_completion_and_clock_fences() {
        let source = include_str!("engineering_gfx950.rs");
        let poll = source
            .split("fn poll_pending_dispatch")
            .nth(1)
            .unwrap()
            .split("unsafe fn dispatch_sequence")
            .next()
            .unwrap();
        let identity = poll.find("self.require_timestamp_pending").unwrap();
        let snapshot = poll
            .find("Backend::observe_engineering_timestamp_signal_acquire")
            .unwrap();
        let completion = poll.find("let completed = dispatch_completed").unwrap();
        let idle = poll.find("self.check_idle()?").unwrap();
        let finish = poll.find("self.finish_timestamp_dispatch").unwrap();
        let done = poll.find("pending.completed = true").unwrap();
        assert!(
            identity < snapshot
                && snapshot < completion
                && completion < idle
                && idle < finish
                && finish < done
        );
        let publish = source
            .split("unsafe fn publish_prepared_dispatch")
            .nth(1)
            .unwrap()
            .split("fn poll_pending_dispatch")
            .next()
            .unwrap();
        assert!(
            publish.find("self.begin_timestamp_dispatch").unwrap()
                < publish.find("packet.publish_with").unwrap()
        );
        let clock = include_str!("device_gfx950.rs")
            .split("fn observe_engineering_clock_correlation")
            .nth(1)
            .unwrap()
            .split("/// Identifies")
            .next()
            .unwrap();
        assert_eq!(
            clock
                .matches("self.check_engineering_operational_currentness()?")
                .count(),
            2
        );
        assert!(clock.contains("self.currentness_poisoned = true"));
    }

    fn fixture(policy: AmdQueueProfilingPolicyV1) -> (CanaryOwner, PendingTimestamp, [u8; 64]) {
        let mut owner = CanaryOwner::new(policy);
        let identity = owner.begin(123, 7, 4, 8).unwrap();
        let pending = PendingTimestamp {
            identity,
            before: sample(10, 100),
        };
        let mut raw = [0; 64];
        raw[0] = 1;
        if policy == AmdQueueProfilingPolicyV1::EnableDispatchTimestamps {
            raw[32..40].copy_from_slice(&12_u64.to_le_bytes());
            raw[40..48].copy_from_slice(&18_u64.to_le_bytes());
        }
        (owner, pending, raw)
    }

    #[test]
    fn default_control_bytes_identical_and_opt_in_changes_only_bit_three() {
        #[repr(align(4096))]
        struct Page([u8; 4096]);
        let mut baseline = Page([0; 4096]);
        crate::queue::submit::initialize_amd_aql_control(&mut baseline.0).unwrap();
        let mut off = baseline.0;
        apply_queue_policy(&mut off, AmdQueueProfilingPolicyV1::Preserve).unwrap();
        assert_eq!(off, baseline.0);
        let mut on = off;
        apply_queue_policy(&mut on, AmdQueueProfilingPolicyV1::EnableDispatchTimestamps).unwrap();
        let changed: Vec<_> = on
            .iter()
            .zip(off)
            .enumerate()
            .filter(|(_, (a, b))| **a != *b)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(changed, [0xb4]);
        assert_eq!(on[0xb4], 8);
        assert!(apply_queue_policy(&mut [0; 4095], AmdQueueProfilingPolicyV1::Preserve).is_err());
    }

    #[test]
    fn exact_generations_require_completion_and_one_use_consumption() {
        for policy in [
            AmdQueueProfilingPolicyV1::Preserve,
            AmdQueueProfilingPolicyV1::EnableDispatchTimestamps,
        ] {
            let (mut owner, pending, raw) = fixture(policy);
            assert!(owner.take([4, 8, 1]).is_err());
            assert!(owner.begin(123, 7, 4, 9).is_err());
            owner.finish(&pending, raw, sample(20, 200)).unwrap();
            assert!(owner.begin(123, 7, 4, 9).is_err());
            assert!(owner.take([4, 8, 2]).is_err());
            let receipt = owner.take([4, 8, 1]).unwrap();
            assert_eq!(
                receipt.correlated.is_some(),
                policy == AmdQueueProfilingPolicyV1::EnableDispatchTimestamps
            );
            assert!(owner.take([4, 8, 1]).is_err());
            let next = owner.begin(123, 7, 4, 9).unwrap();
            assert_eq!(next.generation, 2);
            assert!(owner.require_active(pending.identity).is_err());
        }
    }

    #[test]
    fn foreign_identity_and_generation_overflow_are_closed() {
        let (owner, pending, _) = fixture(AmdQueueProfilingPolicyV1::Preserve);
        for identity in [
            Identity {
                device: 124,
                ..pending.identity
            },
            Identity {
                gpu: 8,
                ..pending.identity
            },
            Identity {
                epoch: 5,
                ..pending.identity
            },
            Identity {
                packet: 9,
                ..pending.identity
            },
            Identity {
                generation: 2,
                ..pending.identity
            },
        ] {
            assert!(owner.require_active(identity).is_err());
        }
        let mut owner = CanaryOwner::new(AmdQueueProfilingPolicyV1::Preserve);
        owner.generation = u64::MAX;
        assert!(owner.begin(123, 7, 4, 8).is_err());
        assert!(owner.active.is_none());
    }

    #[test]
    fn no_snapshot_or_clock_failure_mints_a_receipt_or_allows_reuse() {
        for offset in [0, 8, 16, 24, 32, 40, 48, 56] {
            let (mut owner, pending, mut raw) = fixture(AmdQueueProfilingPolicyV1::Preserve);
            raw[offset] ^= 1;
            assert!(owner.finish(&pending, raw, sample(20, 200)).is_err());
            assert!(owner.receipt.is_none());
            assert!(owner.begin(123, 7, 4, 9).is_err());
        }
        for after in [
            sample(0, 200),
            sample(10, 200),
            sample(9, 200),
            sample(20, 0),
            sample(20, 100),
            DispatchClockSampleV1 {
                gpu_id: 8,
                ..sample(20, 200)
            },
            DispatchClockSampleV1 {
                system_frequency_hz: 0,
                ..sample(20, 200)
            },
            DispatchClockSampleV1 {
                system_frequency_hz: 999,
                ..sample(20, 200)
            },
        ] {
            let (mut owner, pending, raw) =
                fixture(AmdQueueProfilingPolicyV1::EnableDispatchTimestamps);
            assert!(owner.finish(&pending, raw, after).is_err());
            assert!(owner.receipt.is_none());
        }
    }

    #[test]
    fn profile_on_zero_reverse_outside_and_overflow_have_no_host_fallback() {
        for (start, end) in [(0_u64, 0_u64), (18, 12), (9, 18), (12, 21)] {
            let (mut owner, pending, mut raw) =
                fixture(AmdQueueProfilingPolicyV1::EnableDispatchTimestamps);
            raw[32..40].copy_from_slice(&start.to_le_bytes());
            raw[40..48].copy_from_slice(&end.to_le_bytes());
            assert!(owner.finish(&pending, raw, sample(20, 200)).is_err());
            assert!(owner.receipt.is_none());
        }
        let (mut owner, mut pending, mut raw) =
            fixture(AmdQueueProfilingPolicyV1::EnableDispatchTimestamps);
        pending.before = sample(1, 1);
        raw[32..40].copy_from_slice(&1_u64.to_le_bytes());
        raw[40..48].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(
            owner
                .finish(&pending, raw, sample(u64::MAX, u64::MAX))
                .is_err()
        );
        assert!(owner.receipt.is_none());
    }
}
