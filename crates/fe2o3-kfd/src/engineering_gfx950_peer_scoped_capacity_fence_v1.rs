//! Four ordered rank checkpoints per zero-add capacity fence.
use super::*;

pub(in crate::engineering_gfx950::peer) trait RankBackend {
    fn check(&mut self) -> Result<()>;
    fn idle(&mut self) -> Result<()>;
}
pub(in crate::engineering_gfx950::peer) trait FenceBackend {
    type Rank<'rank>: RankBackend
    where
        Self: 'rank;
    fn rank(&mut self, rank: usize) -> Result<Self::Rank<'_>>;
}

pub(in crate::engineering_gfx950::peer) fn fence(backend: &mut impl FenceBackend) -> Result<u32> {
    let mut observed = 0_u32;
    for rank in 0..2 {
        let mut route = backend.rank(rank)?;
        route.check()?;
        observed = observed
            .checked_add(1)
            .ok_or("capacity fence count overflow")?;
        // idle retains its own second rank checkpoint before queue predicates.
        route.idle()?;
        observed = observed
            .checked_add(1)
            .ok_or("capacity fence count overflow")?;
    }
    Ok(observed)
}

struct NativeFence<'group, 'call> {
    group: &'group mut Gfx950EngineeringPeerGroupV1,
    currentness: &'group mut Currentness<'call>,
}
struct NativeRank<'rank> {
    context: &'rank mut Context,
    route: RankCurrentness<'rank>,
}
impl RankBackend for NativeRank<'_> {
    fn check(&mut self) -> Result<()> {
        self.route.check(self.context, false)
    }
    fn idle(&mut self) -> Result<()> {
        self.route.idle(self.context)
    }
}
impl FenceBackend for NativeFence<'_, '_> {
    type Rank<'rank>
        = NativeRank<'rank>
    where
        Self: 'rank;
    fn rank(&mut self, rank: usize) -> Result<Self::Rank<'_>> {
        let route = self.currentness.rank(self.group, rank)?;
        let context = self
            .group
            .contexts
            .get_mut(rank)
            .ok_or("capacity census rank absent")?;
        Ok(NativeRank { context, route })
    }
}

pub(super) fn run(
    group: &mut Gfx950EngineeringPeerGroupV1,
    currentness: &mut Currentness<'_>,
) -> Result<u32> {
    fence(&mut NativeFence { group, currentness })
}
