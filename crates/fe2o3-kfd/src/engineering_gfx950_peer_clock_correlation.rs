//! Bounded raw clock snapshots, never a clock conversion or dispatch capability.

use super::*;
use crate::KfdClockCorrelationObservationV1;

/// An inert sample from one rank of a fresh, idle, fully checked peer group.
/// Different ranks are sampled sequentially, not simultaneously. The KFD
/// counters have not been joined to completion-signal timestamp clock domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerClockObservationV1 {
    group_incarnation: u64,
    rank: usize,
    unique_id: u64,
    queue_epoch: u64,
    counters: KfdClockCorrelationObservationV1,
    sample_started: Instant,
    sample_finished: Instant,
}

impl Gfx950EngineeringPeerClockObservationV1 {
    pub const fn group_incarnation(self) -> u64 {
        self.group_incarnation
    }
    pub const fn rank(self) -> usize {
        self.rank
    }
    pub const fn unique_id(self) -> u64 {
        self.unique_id
    }
    pub const fn queue_epoch(self) -> u64 {
        self.queue_epoch
    }
    pub const fn counters(self) -> KfdClockCorrelationObservationV1 {
        self.counters
    }
    /// Process-local host bracket around the complete device sampler call,
    /// including its full currentness checks, not just the ioctl itself.
    pub const fn sample_started(self) -> Instant {
        self.sample_started
    }
    pub const fn sample_finished(self) -> Instant {
        self.sample_finished
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    unique_id: u64,
    gpu_id: u32,
    queue_epoch: u64,
}

trait ClockBackend {
    fn participants(&self) -> usize;
    fn check(&mut self) -> Result<()>;
    fn identity(&self, rank: usize) -> Identity;
    fn sample(&mut self, rank: usize) -> Result<KfdClockCorrelationObservationV1>;
}

fn sample_group(
    backend: &mut impl ClockBackend,
    group_incarnation: u64,
) -> Result<Vec<Gfx950EngineeringPeerClockObservationV1>> {
    let count = backend.participants();
    if !matches!(count, 2 | 8) || group_incarnation == 0 {
        return Err("invalid clock observation group identity or participant count".into());
    }
    backend.check()?;
    let mut identities: Vec<Identity> = Vec::with_capacity(count);
    for rank in 0..count {
        let identity = backend.identity(rank);
        if identity.unique_id == 0
            || identities.iter().any(|prior| {
                prior.unique_id == identity.unique_id || prior.gpu_id == identity.gpu_id
            })
        {
            return Err("invalid or duplicate clock observation device identity".into());
        }
        identities.push(identity);
    }
    let mut observations = Vec::with_capacity(count);
    for (rank, identity) in identities.iter().enumerate() {
        let sample_started = Instant::now();
        let counters = backend.sample(rank)?;
        let sample_finished = Instant::now();
        if counters.gpu_id() != identity.gpu_id || counters.system_clock_frequency_hz() == 0 {
            return Err("clock observation counter identity or system frequency".into());
        }
        observations.push(Gfx950EngineeringPeerClockObservationV1 {
            group_incarnation,
            rank,
            unique_id: identity.unique_id,
            queue_epoch: identity.queue_epoch,
            counters,
            sample_started,
            sample_finished,
        });
    }
    backend.check()?;
    for (rank, identity) in identities.iter().enumerate() {
        if backend.identity(rank) != *identity {
            return Err("clock observation device identity or queue epoch changed".into());
        }
    }
    Ok(observations)
}

struct NativeClocks<'a>(&'a mut Gfx950EngineeringPeerGroupV1);

impl ClockBackend for NativeClocks<'_> {
    fn participants(&self) -> usize {
        self.0.contexts.len()
    }

    fn check(&mut self) -> Result<()> {
        check_contexts(&mut self.0.contexts, self.0.shared_full_currentness)
    }

    fn identity(&self, rank: usize) -> Identity {
        let context = &self.0.contexts[rank];
        Identity {
            unique_id: context.unique_id,
            gpu_id: context.backend.gpu_id(),
            queue_epoch: context.queue_epoch,
        }
    }

    fn sample(&mut self, rank: usize) -> Result<KfdClockCorrelationObservationV1> {
        self.0.contexts[rank]
            .backend
            .observe_clock_correlation()
            .map_err(|error| error.to_string())
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Samples all 2 or 8 ranks under fresh full-group entry and exit fences.
    /// The full roster is authenticated before the first ioctl. Every device
    /// sampler also performs its own full currentness checks. No queue, signal,
    /// state or performance configuration is changed. A failure quarantines
    /// the group and returns no partial observations.
    ///
    /// Raw counters and process-local host brackets only: no dispatch duration,
    /// calibrated frequency, clock-domain equivalence or cross-GPU alignment.
    pub fn observe_clock_correlation_v1(
        &mut self,
    ) -> Result<Vec<Gfx950EngineeringPeerClockObservationV1>> {
        self.require_active()?;
        let incarnation = self.incarnation;
        let result = sample_group(&mut NativeClocks(self), incarnation);
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_clock_correlation_tests.rs"]
mod tests;
