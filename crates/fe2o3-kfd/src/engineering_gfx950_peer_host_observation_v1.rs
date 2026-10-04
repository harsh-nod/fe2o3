//! Optional host observations, independent of publication/currentness policy.

use super::*;

/// Shared backend calls counted once per group, not once per participant.
/// Durations are inclusive host elapsed nanoseconds, not GPU time.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Gfx950EngineeringSharedHostCountersV1 {
    pub group_full_checks: u64,
    pub group_full_ns: u64,
    pub publication_full_checks: u64,
    pub publication_full_ns: u64,
}

#[derive(Clone, Copy)]
pub(super) enum SharedScope {
    GroupFence,
    Publication,
}

impl Gfx950EngineeringSharedHostCountersV1 {
    fn record(&mut self, scope: SharedScope, elapsed: Duration) -> Result<()> {
        let ns = u64::try_from(elapsed.as_nanos()).map_err(explain)?;
        let (count, total) = match scope {
            SharedScope::GroupFence => (&mut self.group_full_checks, &mut self.group_full_ns),
            SharedScope::Publication => (
                &mut self.publication_full_checks,
                &mut self.publication_full_ns,
            ),
        };
        let next_count = count
            .checked_add(1)
            .ok_or("host observation count exhausted")?;
        let next_total = total
            .checked_add(ns)
            .ok_or("host observation time exhausted")?;
        *count = next_count;
        *total = next_total;
        Ok(())
    }

    fn difference(&self, earlier: &Self) -> Result<Self> {
        Ok(Self {
            group_full_checks: difference(self.group_full_checks, earlier.group_full_checks)?,
            group_full_ns: difference(self.group_full_ns, earlier.group_full_ns)?,
            publication_full_checks: difference(
                self.publication_full_checks,
                earlier.publication_full_checks,
            )?,
            publication_full_ns: difference(self.publication_full_ns, earlier.publication_full_ns)?,
        })
    }
}

pub(in crate::engineering_gfx950) struct HostObservationState {
    queue_epoch: u64,
    shared: Gfx950EngineeringSharedHostCountersV1,
}

impl HostObservationState {
    fn require_epoch(&self, queue_epoch: u64) -> Result<()> {
        if self.queue_epoch != queue_epoch {
            return Err("host observation queue epoch changed".into());
        }
        Ok(())
    }
}

pub(in crate::engineering_gfx950) fn timers_enabled(
    policy: Option<PerformanceOptions>,
    observation: bool,
) -> bool {
    observation || policy.is_some_and(|options| options.profile)
}

pub(in crate::engineering_gfx950) fn require_observational_policy(
    observation: bool,
    policy: Option<PerformanceOptions>,
) -> Result<()> {
    if observation
        && policy.is_some_and(|options| options.profile || options.operational_currentness)
    {
        return Err(
            "host observation requires unchanged full currentness and legacy profile off".into(),
        );
    }
    Ok(())
}

fn require_fresh_observation(
    enabled: bool,
    policy: Option<PerformanceOptions>,
    next_buffer: u64,
    next_kernel: u64,
    write: u64,
) -> Result<()> {
    require_fresh_configuration(enabled, next_buffer, next_kernel, write)?;
    require_observational_policy(true, policy)
}

pub(super) fn shared_currentness(contexts: &mut [Context], scope: SharedScope) -> Result<()> {
    let started = contexts
        .first()
        .and_then(|context| context.host_observation.as_ref().map(|_| Instant::now()));
    let result = {
        let mut devices = contexts
            .iter_mut()
            .map(|context| context.backend.engineering_peer_device())
            .collect::<Vec<_>>();
        crate::device::check_engineering_group_currentness(&mut devices).map_err(explain)
    };
    if let Some(started) = started {
        contexts[0]
            .host_observation
            .as_mut()
            .ok_or("missing host observer")?
            .shared
            .record(scope, started.elapsed())?;
    }
    result
}

/// One rank's cumulative host counters, or its counters in a checked delta.
/// Prepare, publication, wait, currentness, admission and I/O scopes are nested
/// and inclusive. In particular wait includes host scheduling and other ranks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerHostParticipantV1 {
    rank: usize,
    unique_id: u64,
    queue_epoch: u64,
    cache_kernel_admission: bool,
    raw_timestamp_queue: bool,
    counters: PerformanceCountersV1,
}

impl Gfx950EngineeringPeerHostParticipantV1 {
    pub const fn rank(&self) -> usize {
        self.rank
    }
    pub const fn unique_id(&self) -> u64 {
        self.unique_id
    }
    pub const fn queue_epoch(&self) -> u64 {
        self.queue_epoch
    }
    pub const fn cache_kernel_admission(&self) -> bool {
        self.cache_kernel_admission
    }
    pub const fn raw_timestamp_queue(&self) -> bool {
        self.raw_timestamp_queue
    }
    pub fn counters(&self) -> &PerformanceCountersV1 {
        &self.counters
    }
}

/// Host-only snapshot. It does not observe a device or certify completion.
/// These inclusive scopes cannot be added or subtracted to infer GPU duration,
/// calibrated overlap, or throughput. Successful external reporting must also
/// retain workload validation and healthy Close; snapshots alone prove neither.
#[derive(Clone, Debug)]
pub struct Gfx950EngineeringPeerHostObservationV1 {
    group: u64,
    observed_at: Instant,
    shared_full_currentness: bool,
    participants: Vec<Gfx950EngineeringPeerHostParticipantV1>,
    shared: Gfx950EngineeringSharedHostCountersV1,
}

/// Checked host counter differences between two snapshots of the same group,
/// rank roster, queue epochs and policy. Host elapsed is not summed counter time.
#[derive(Clone, Debug)]
pub struct Gfx950EngineeringPeerHostDeltaV1 {
    group: u64,
    host_elapsed_ns: u64,
    participants: Vec<Gfx950EngineeringPeerHostParticipantV1>,
    shared: Gfx950EngineeringSharedHostCountersV1,
}

impl Gfx950EngineeringPeerHostObservationV1 {
    pub const fn group_incarnation(&self) -> u64 {
        self.group
    }
    pub const fn shared_full_currentness(&self) -> bool {
        self.shared_full_currentness
    }
    pub fn participants(&self) -> &[Gfx950EngineeringPeerHostParticipantV1] {
        &self.participants
    }
    pub fn shared_counters(&self) -> &Gfx950EngineeringSharedHostCountersV1 {
        &self.shared
    }

    pub fn checked_delta(&self, earlier: &Self) -> Result<Gfx950EngineeringPeerHostDeltaV1> {
        if self.group != earlier.group
            || self.shared_full_currentness != earlier.shared_full_currentness
            || self.participants.len() != earlier.participants.len()
        {
            return Err("host observation group or policy changed".into());
        }
        let elapsed = self
            .observed_at
            .checked_duration_since(earlier.observed_at)
            .ok_or("host snapshots are reversed")?;
        let mut participants = Vec::with_capacity(self.participants.len());
        for (now, before) in self.participants.iter().zip(&earlier.participants) {
            if now.rank != before.rank
                || now.unique_id != before.unique_id
                || now.queue_epoch != before.queue_epoch
                || now.cache_kernel_admission != before.cache_kernel_admission
                || now.raw_timestamp_queue != before.raw_timestamp_queue
            {
                return Err("host observation participant identity or policy changed".into());
            }
            let mut participant = now.clone();
            participant.counters = counter_difference(&now.counters, &before.counters)?;
            participants.push(participant);
        }
        Ok(Gfx950EngineeringPeerHostDeltaV1 {
            group: self.group,
            host_elapsed_ns: u64::try_from(elapsed.as_nanos()).map_err(explain)?,
            participants,
            shared: self.shared.difference(&earlier.shared)?,
        })
    }
}

impl Gfx950EngineeringPeerHostDeltaV1 {
    pub const fn group_incarnation(&self) -> u64 {
        self.group
    }
    pub const fn host_elapsed_ns(&self) -> u64 {
        self.host_elapsed_ns
    }
    pub fn participants(&self) -> &[Gfx950EngineeringPeerHostParticipantV1] {
        &self.participants
    }
    pub fn shared_counters(&self) -> &Gfx950EngineeringSharedHostCountersV1 {
        &self.shared
    }
}

fn difference(now: u64, earlier: u64) -> Result<u64> {
    now.checked_sub(earlier)
        .ok_or_else(|| "host counters decreased".into())
}

fn counter_difference(
    now: &PerformanceCountersV1,
    earlier: &PerformanceCountersV1,
) -> Result<PerformanceCountersV1> {
    macro_rules! subtract {
        ($($field:ident),+ $(,)?) => {
            PerformanceCountersV1 { $($field: difference(now.$field, earlier.$field)?),+ }
        };
    }
    Ok(subtract!(
        commands,
        command_ns,
        full_currentness_checks,
        full_currentness_ns,
        operational_currentness_checks,
        operational_currentness_ns,
        kernel_admissions,
        kernel_admission_ns,
        dispatches,
        dispatch_prepare_ns,
        dispatch_publish_ns,
        dispatch_wait_ns,
        completion_polls,
        reads,
        read_bytes,
        read_ns,
        writes,
        write_bytes,
        write_ns
    ))
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Enables host timers once, before any user allocation/kernel/packet.
    /// Does not change performance options, publication sharing, queue mode or
    /// full currentness checks. Disabled by default; no implicit environment flag.
    /// Timer/counter overhead is part of the observed execution. Every failure
    /// remains terminal under the group's existing disposable-process contract.
    pub fn enable_host_observation_v1(&mut self) -> Result<()> {
        self.require_active()?;
        let result = (|| {
            if !matches!(self.contexts.len(), 2 | 8)
                || self.next_buffer != 1
                || !self.buffers.is_empty()
            {
                return Err("host observer requires a fresh peer group".into());
            }
            for context in &self.contexts {
                require_fresh_observation(
                    context.host_observation.is_some(),
                    context.performance,
                    context.next_buffer,
                    context.next_kernel,
                    context.ring.write(),
                )?;
                if context.counters != PerformanceCountersV1::default() {
                    return Err("host observer requires unprofiled counters".into());
                }
            }
            check_contexts(&mut self.contexts, self.shared_full_currentness)?;
            for context in &mut self.contexts {
                context.host_observation = Some(HostObservationState {
                    queue_epoch: context.queue_epoch,
                    shared: Gfx950EngineeringSharedHostCountersV1::default(),
                });
            }
            Ok(())
        })();
        self.finish(result)
    }

    /// Copies bounded host data only: no native call, sampling, counter reset or
    /// additional currentness check. Refuses disabled, poisoned/closed groups or
    /// queue rollover since activation. This does not change queue ownership.
    pub fn host_observation_v1(&mut self) -> Result<Gfx950EngineeringPeerHostObservationV1> {
        self.require_active()?;
        let result = (|| {
            if !matches!(self.contexts.len(), 2 | 8) {
                return Err("host observer participant roster".into());
            }
            let mut participants = Vec::with_capacity(self.contexts.len());
            for (rank, context) in self.contexts.iter().enumerate() {
                let observer = context
                    .host_observation
                    .as_ref()
                    .ok_or("host observation disabled")?;
                observer.require_epoch(context.queue_epoch)?;
                require_observational_policy(true, context.performance)?;
                participants.push(Gfx950EngineeringPeerHostParticipantV1 {
                    rank,
                    unique_id: context.unique_id,
                    queue_epoch: context.queue_epoch,
                    cache_kernel_admission: context
                        .performance
                        .is_some_and(|p| p.cache_kernel_admission),
                    raw_timestamp_queue: context.raw_timestamps_enabled,
                    counters: context.counters.clone(),
                });
            }
            Ok(Gfx950EngineeringPeerHostObservationV1 {
                group: self.incarnation,
                observed_at: Instant::now(),
                shared_full_currentness: self.shared_full_currentness,
                participants,
                shared: self.contexts[0]
                    .host_observation
                    .as_ref()
                    .ok_or("host observation disabled")?
                    .shared
                    .clone(),
            })
        })();
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_host_observation_v1_tests.rs"]
mod tests;
