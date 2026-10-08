//! Fixed host intervals for the private warm-layer engine; never authority.
use super::*;

const LIMIT_NS: u64 = 3_600_000_000_000;

/// Host-only intervals from an already completed scoped warm layer.
/// Order: entry/pre-census, Prefix, MLP/retired seal, hidden/post-census,
/// full exit, commit preparation. Paired intervals are nested in MLP:
/// preflight, consume, reserve, publication, polling, retirement, terminal.
/// Paired timing starts after initial deadline setup and ends after terminal
/// readbacks, before the final deadline timestamp/check and Completion assembly.
/// Those exclusions and retired-arena sealing remain in the enclosing MLP phase.
/// Final metric acceptance is outside these intervals, before owner disarm.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerScopedLayerDurationsV1 {
    pub phase_ns: [u64; 6],
    pub layer_body_ns: u64,
    pub paired_mlp_phase_ns: [u64; 7],
    pub paired_mlp_body_ns: u64,
}

fn sum(values: &[u64]) -> Result<u64> {
    values.iter().try_fold(0_u64, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| "layer duration sum overflow".into())
    })
}

impl Gfx950EngineeringPeerScopedLayerDurationsV1 {
    pub(super) fn validate(&self) -> Result<()> {
        if sum(&self.phase_ns)? != self.layer_body_ns
            || sum(&self.paired_mlp_phase_ns)? != self.paired_mlp_body_ns
            || self.layer_body_ns > LIMIT_NS
            || self.paired_mlp_body_ns > self.phase_ns[2]
        {
            return Err("layer duration extent, sum or containment".into());
        }
        Ok(())
    }
}

pub(super) trait Clock {
    fn now(&mut self) -> Result<Instant>;
}
pub(super) struct Monotonic;
impl Clock for Monotonic {
    fn now(&mut self) -> Result<Instant> {
        Ok(Instant::now())
    }
}

pub(super) struct Recorder<'a, const N: usize> {
    clock: &'a mut dyn Clock,
    start: Option<Instant>,
    last: Option<Instant>,
    phases: [u64; N],
    count: usize,
}
impl<'a, const N: usize> Recorder<'a, N> {
    pub(super) fn new(clock: &'a mut dyn Clock) -> Self {
        Self {
            clock,
            start: None,
            last: None,
            phases: [0; N],
            count: 0,
        }
    }
    pub(super) fn mark(&mut self) -> Result<()> {
        let now = self.clock.now()?;
        if let Some(last) = self.last {
            if self.count == N {
                return Err("extra layer duration boundary".into());
            }
            let value = now
                .checked_duration_since(last)
                .ok_or("layer duration clock moved backwards")?;
            let ns = u64::try_from(value.as_nanos()).map_err(explain)?;
            if ns > LIMIT_NS {
                return Err("layer duration interval bound".into());
            }
            self.phases[self.count] = ns;
            self.count += 1;
        } else {
            self.start = Some(now);
        }
        self.last = Some(now);
        Ok(())
    }
    pub(super) fn finish(&self) -> Result<([u64; N], u64)> {
        if self.count != N {
            return Err("missing layer duration boundary".into());
        }
        let body = self
            .last
            .ok_or("layer duration end absent")?
            .checked_duration_since(self.start.ok_or("layer duration start absent")?)
            .ok_or("layer duration body moved backwards")?;
        let body = u64::try_from(body.as_nanos()).map_err(explain)?;
        if body > LIMIT_NS || sum(&self.phases)? != body {
            return Err("layer duration body bound or sum".into());
        }
        Ok((self.phases, body))
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_layer_duration_v1_tests.rs"]
pub(super) mod tests;
