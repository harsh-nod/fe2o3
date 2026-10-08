//! Census-only hooks around the existing closed warm-layer ordering engine.
use super::*;
use crate::engineering_gfx950::peer::capacity_v1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerScopedCapacityCensusObservationV1 {
    pub owner_counts: [u64; 2],
    pub preflights: u32,
    pub rank_checkpoints: u32,
}

#[derive(Debug)]
pub struct Gfx950EngineeringPeerScopedCensusWarmLayerObservationV1 {
    pub layer: Gfx950EngineeringPeerScopedWarmLayerObservationV1,
    pub census: Gfx950EngineeringPeerScopedCapacityCensusObservationV1,
}

struct Sample<S> {
    snapshot: S,
    owner_counts: [u64; 2],
    rank_checkpoints: u32,
}
impl<S> Sample<S> {
    fn validate(&self, expected: [usize; 2]) -> Result<()> {
        let expected = [
            u64::try_from(expected[0]).map_err(|_| "census assertion conversion")?,
            u64::try_from(expected[1]).map_err(|_| "census assertion conversion")?,
        ];
        if self.owner_counts != expected || self.rank_checkpoints != 8 {
            return Err("scoped census owner assertion or boundary count changed".into());
        }
        Ok(())
    }
}

trait CensusBackend: ClosedBackend {
    type Snapshot: Eq;
    fn census(&mut self) -> Result<Sample<Self::Snapshot>>;
}
impl CensusBackend for NativeLayer<'_, '_> {
    type Snapshot = capacity_v1::Snapshot;
    fn census(&mut self) -> Result<Sample<Self::Snapshot>> {
        let (snapshot, rank_checkpoints) = capacity_v1::scoped_preflight(
            self.op.group,
            &mut scoped_currentness::Currentness::Scoped(
                self.window.as_mut().ok_or("scoped census window absent")?,
            ),
        )?;
        Ok(Sample {
            owner_counts: snapshot.owner_counts()?,
            snapshot,
            rank_checkpoints,
        })
    }
}

struct CensusLayer<B: CensusBackend> {
    inner: B,
    expected: [usize; 2],
    before: Option<Sample<B::Snapshot>>,
    observation: Option<Gfx950EngineeringPeerScopedCapacityCensusObservationV1>,
}
impl<B: CensusBackend> ClosedBackend for CensusLayer<B> {
    type Prefix = B::Prefix;
    type Pending = B::Pending;
    type Hidden = B::Hidden;
    type Counts = B::Counts;
    type Output = (
        B::Output,
        Gfx950EngineeringPeerScopedCapacityCensusObservationV1,
    );
    fn enter(&mut self) -> Result<()> {
        self.inner.enter()?;
        let before = self.inner.census()?;
        before.validate(self.expected)?;
        self.before = Some(before);
        Ok(())
    }
    fn prefix(&mut self) -> Result<Self::Prefix> {
        self.inner.prefix()
    }
    fn mlp(&mut self) -> Result<Self::Pending> {
        self.inner.mlp()
    }
    fn hidden(&mut self) -> Result<Self::Hidden> {
        let hidden = self.inner.hidden()?;
        let after = self.inner.census()?;
        after.validate(self.expected)?;
        let before = self
            .before
            .take()
            .ok_or("scoped census before snapshot absent")?;
        if before.snapshot != after.snapshot || before.owner_counts != after.owner_counts {
            return Err("scoped layer complete allocation accounting changed".into());
        }
        let rank_checkpoints = before
            .rank_checkpoints
            .checked_add(after.rank_checkpoints)
            .ok_or("scoped census total checkpoint overflow")?;
        self.observation = Some(Gfx950EngineeringPeerScopedCapacityCensusObservationV1 {
            owner_counts: after.owner_counts,
            preflights: 2,
            rank_checkpoints,
        });
        Ok(hidden)
    }
    fn exit(&mut self) -> Result<Self::Counts> {
        self.inner.exit()
    }
    fn commit(
        &mut self,
        prefix: Self::Prefix,
        pending: Self::Pending,
        hidden: Self::Hidden,
        counts: Self::Counts,
    ) -> Result<Self::Output> {
        // Prepare every fallible census result before NativeLayer can commit.
        let census = self
            .observation
            .take()
            .ok_or("scoped census observation absent")?;
        let layer = self.inner.commit(prefix, pending, hidden, counts)?;
        Ok((layer, census))
    }
    fn quarantine(&mut self) {
        self.inner.quarantine();
    }
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    fn accept_layer_durations(
        &mut self,
        output: &mut Self::Output,
        phase_ns: [u64; 6],
        body_ns: u64,
    ) -> Result<()> {
        self.inner
            .accept_layer_durations(&mut output.0, phase_ns, body_ns)
    }
}

pub(super) fn run(
    native: NativeLayer<'_, '_>,
    expected: [usize; 2],
) -> Result<Gfx950EngineeringPeerScopedCensusWarmLayerObservationV1> {
    let (layer, census) = closed_layer(&mut CensusLayer {
        inner: native,
        expected,
        before: None,
        observation: None,
    })?;
    Ok(Gfx950EngineeringPeerScopedCensusWarmLayerObservationV1 { layer, census })
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_scoped_census_layer_v1_tests.rs"]
mod tests;
