//! Inert per-dispatch observations, not clock calibration or production authority.

use super::{PendingDispatch, Result, require_pending_dispatch_identity};

/// Raw ticks written to one retained AQL completion signal on one GPU.
///
/// Process-local identity is (group incarnation, rank, GPU UID, queue epoch,
/// packet ID); external evidence must also retain its owning process identity.
/// Signal generation is the one-based dispatch frontier within that epoch.
/// No timestamp frequency, cross-GPU alignment, wrap correction, overlap,
/// kernel-active-cycle interpretation, or throughput claim is supplied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringRawTimestampObservationV1 {
    group_incarnation: u64,
    rank: usize,
    unique_id: u64,
    queue_epoch: u64,
    packet_id: u64,
    signal_generation: u64,
    start_tick: u64,
    end_tick: u64,
    host_elapsed_ns: u64,
}

impl Gfx950EngineeringRawTimestampObservationV1 {
    pub const fn group_incarnation(&self) -> u64 {
        self.group_incarnation
    }
    pub const fn rank(&self) -> usize {
        self.rank
    }
    pub const fn unique_id(&self) -> u64 {
        self.unique_id
    }
    pub const fn queue_epoch(&self) -> u64 {
        self.queue_epoch
    }
    pub const fn packet_id(&self) -> u64 {
        self.packet_id
    }
    pub const fn signal_generation(&self) -> u64 {
        self.signal_generation
    }
    pub const fn start_tick(&self) -> u64 {
        self.start_tick
    }
    pub const fn end_tick(&self) -> u64 {
        self.end_tick
    }
    /// Existing submission-to-observed-completion host interval, not GPU time.
    pub const fn host_elapsed_ns(&self) -> u64 {
        self.host_elapsed_ns
    }
}

pub(super) fn require_capture_mode(enabled: bool, capture: bool) -> Result<()> {
    if enabled != capture {
        return Err("dispatch API does not match fresh queue timestamp mode".into());
    }
    Ok(())
}

pub(super) fn require_ticks(ticks: [u64; 2]) -> Result<()> {
    // Zero is the per-dispatch unwritten sentinel. Reversed values, including
    // an unmodelled clock wrap, reject; equal nonzero ticks remain observable.
    if ticks.contains(&0) || ticks[1] < ticks[0] {
        return Err("unwritten or reversed raw completion timestamps".into());
    }
    Ok(())
}

pub(super) fn completed_observation(
    incarnation: u64,
    rank: usize,
    retained: [u64; 3],
    pending: &mut PendingDispatch,
    host_elapsed_ns: u64,
) -> Result<Gfx950EngineeringRawTimestampObservationV1> {
    if incarnation == 0 || rank >= 8 || retained[0] == 0 || !pending.completed {
        return Err("raw timestamp completion identity or phase".into());
    }
    require_pending_dispatch_identity(
        retained,
        [pending.unique_id, pending.queue_epoch, pending.next],
        false,
    )?;
    let packet_id = pending
        .next
        .checked_sub(1)
        .ok_or("raw timestamp zero generation")?;
    let ticks = pending
        .raw_timestamps
        .take()
        .ok_or("raw timestamp absent or already consumed")?;
    require_ticks(ticks)?;
    Ok(Gfx950EngineeringRawTimestampObservationV1 {
        group_incarnation: incarnation,
        rank,
        unique_id: retained[0],
        queue_epoch: retained[1],
        packet_id,
        signal_generation: pending.next,
        start_tick: ticks[0],
        end_tick: ticks[1],
        host_elapsed_ns,
    })
}

/// Called only after the shared round coordinator's final full exit fence.
pub(super) fn finish_round(
    capture: bool,
    ranks: &[usize],
    elapsed: &[u64],
    mut observations: [Option<Gfx950EngineeringRawTimestampObservationV1>; 8],
) -> Result<Vec<Gfx950EngineeringRawTimestampObservationV1>> {
    if ranks.is_empty() || ranks.len() > 8 || ranks.len() != elapsed.len() {
        return Err("raw timestamp round extent".into());
    }
    let mut result = Vec::with_capacity(if capture { ranks.len() } else { 0 });
    for (&rank, &host_ns) in ranks.iter().zip(elapsed) {
        let slot = observations
            .get_mut(rank)
            .ok_or("raw timestamp rank bound")?;
        if capture {
            let value = slot
                .take()
                .ok_or("raw timestamp missing or duplicated rank")?;
            if value.rank != rank || value.host_elapsed_ns != host_ns {
                return Err("raw timestamp host/rank join".into());
            }
            result.push(value);
        }
    }
    if observations.iter().any(Option::is_some) {
        return Err("unexpected raw timestamp completion".into());
    }
    Ok(result)
}

#[cfg(test)]
#[path = "engineering_gfx950_raw_timestamps_tests.rs"]
mod tests;
