//! Fixed diagnostic totals around the real checkpoint callbacks, never authority.
use super::{CheckpointBackend, DeviceBindingError, Instant, ScopedCountsV1, changed};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gfx950EngineeringCurrentnessCallDurationV1 {
    pub calls: u64,
    pub elapsed_ns: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gfx950EngineeringCurrentnessDurationsV1 {
    pub before: Gfx950EngineeringCurrentnessCallDurationV1,
    pub discover: Gfx950EngineeringCurrentnessCallDurationV1,
    pub after: Gfx950EngineeringCurrentnessCallDurationV1,
    pub root_generation: Gfx950EngineeringCurrentnessCallDurationV1,
}

impl Gfx950EngineeringCurrentnessDurationsV1 {
    pub(super) fn validate(&self, counts: &ScopedCountsV1) -> Result<(), DeviceBindingError> {
        if self.before.calls != counts.before_calls
            || self.discover.calls != counts.full_discoveries
            || self.after.calls != counts.after_calls
            || self.root_generation.calls != counts.generation_probes
        {
            return Err(changed(
                "duration callback count differs from scoped census",
            ));
        }
        self.checked_sum_ns()?;
        Ok(())
    }

    pub(crate) fn checked_sum_ns(&self) -> Result<u64, DeviceBindingError> {
        [
            self.before.elapsed_ns,
            self.discover.elapsed_ns,
            self.after.elapsed_ns,
            self.root_generation.elapsed_ns,
        ]
        .into_iter()
        .try_fold(0_u64, |total, value| {
            total
                .checked_add(value)
                .ok_or_else(|| changed("duration category sum overflow"))
        })
    }

    pub(crate) fn elapsed_ns(start: Instant, end: Instant) -> Result<u64, DeviceBindingError> {
        let elapsed = end
            .checked_duration_since(start)
            .ok_or_else(|| changed("duration clock moved backwards"))?;
        u64::try_from(elapsed.as_nanos()).map_err(|_| changed("duration nanosecond overflow"))
    }

    pub(crate) fn bank_interval_ns(
        &self,
        start: Instant,
        end: Instant,
    ) -> Result<u64, DeviceBindingError> {
        let elapsed = Self::elapsed_ns(start, end)?;
        if self.checked_sum_ns()? > elapsed {
            return Err(changed("bank duration callbacks exceed guarded body"));
        }
        Ok(elapsed)
    }
}

impl Gfx950EngineeringCurrentnessCallDurationV1 {
    fn record(&mut self, start: Instant, end: Instant) -> Result<(), DeviceBindingError> {
        let elapsed = Gfx950EngineeringCurrentnessDurationsV1::elapsed_ns(start, end)?;
        let calls = self
            .calls
            .checked_add(1)
            .ok_or_else(|| changed("duration call count overflow"))?;
        let elapsed_ns = self
            .elapsed_ns
            .checked_add(elapsed)
            .ok_or_else(|| changed("duration total overflow"))?;
        self.calls = calls;
        self.elapsed_ns = elapsed_ns;
        Ok(())
    }
}

pub(super) trait Clock {
    fn now(&mut self) -> Instant;
}

pub(super) struct Monotonic;
impl Clock for Monotonic {
    fn now(&mut self) -> Instant {
        Instant::now()
    }
}

fn measure<C: Clock, T>(
    clock: &mut C,
    total: &mut Gfx950EngineeringCurrentnessCallDurationV1,
    operation: impl FnOnce() -> Result<T, DeviceBindingError>,
) -> Result<T, DeviceBindingError> {
    let start = clock.now();
    let result = operation();
    let end = clock.now();
    let value = result?;
    total.record(start, end)?;
    Ok(value)
}

pub(super) struct Measured<'a, B: CheckpointBackend, C: Clock> {
    inner: &'a mut B,
    totals: &'a mut Gfx950EngineeringCurrentnessDurationsV1,
    clock: C,
}

impl<'a, B: CheckpointBackend, C: Clock> Measured<'a, B, C> {
    pub(super) fn new(
        inner: &'a mut B,
        totals: &'a mut Gfx950EngineeringCurrentnessDurationsV1,
        clock: C,
    ) -> Self {
        Self {
            inner,
            totals,
            clock,
        }
    }
}

impl<B: CheckpointBackend, C: Clock> CheckpointBackend for Measured<'_, B, C> {
    type Snapshot = B::Snapshot;

    fn now(&mut self) -> Instant {
        self.inner.now()
    }
    fn participants(&self) -> usize {
        self.inner.participants()
    }
    fn identity(&mut self, rank: usize) -> Result<u64, DeviceBindingError> {
        self.inner.identity(rank)
    }
    fn before(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        measure(&mut self.clock, &mut self.totals.before, || {
            self.inner.before(rank)
        })
    }
    fn discover(&mut self) -> Result<Self::Snapshot, DeviceBindingError> {
        measure(&mut self.clock, &mut self.totals.discover, || {
            self.inner.discover()
        })
    }
    fn compare_retained(
        &mut self,
        rank: usize,
        snapshot: &Self::Snapshot,
    ) -> Result<(), DeviceBindingError> {
        self.inner.compare_retained(rank, snapshot)
    }
    fn after(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        measure(&mut self.clock, &mut self.totals.after, || {
            self.inner.after(rank)
        })
    }
    fn root_generation(&mut self, snapshot: &Self::Snapshot) -> Result<(), DeviceBindingError> {
        measure(&mut self.clock, &mut self.totals.root_generation, || {
            self.inner.root_generation(snapshot)
        })
    }
    fn poison_all(&mut self) {
        self.inner.poison_all();
    }
    fn validate_durations(&self, counts: &ScopedCountsV1) -> Result<(), DeviceBindingError> {
        self.inner.validate_durations(counts)?;
        self.totals.validate(counts)
    }
}
