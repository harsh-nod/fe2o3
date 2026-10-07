//! Private entry/checkpoint/exit observations; no execution authority or owner escape.
use super::*;
use std::time::Instant;

fn changed(message: &'static str) -> DeviceBindingError {
    DeviceBindingError::ObservableCurrentnessChanged(message)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ScopedCountsV1 {
    pub(crate) full_discoveries: u64,
    pub(crate) local_checkpoints: u64,
    pub(crate) before_calls: u64,
    pub(crate) after_calls: u64,
    pub(crate) generation_probes: u64,
}

fn increment(value: &mut u64) -> Result<(), DeviceBindingError> {
    *value = value
        .checked_add(1)
        .ok_or_else(|| changed("scoped counter overflow"))?;
    Ok(())
}

trait CheckpointBackend {
    type Snapshot: Eq;
    fn now(&mut self) -> Instant;
    fn participants(&self) -> usize;
    fn identity(&mut self, rank: usize) -> Result<u64, DeviceBindingError>;
    fn before(&mut self, rank: usize) -> Result<(), DeviceBindingError>;
    fn discover(&mut self) -> Result<Self::Snapshot, DeviceBindingError>;
    fn compare_retained(
        &mut self,
        rank: usize,
        snapshot: &Self::Snapshot,
    ) -> Result<(), DeviceBindingError>;
    fn after(&mut self, rank: usize) -> Result<(), DeviceBindingError>;
    fn root_generation(&mut self, snapshot: &Self::Snapshot) -> Result<(), DeviceBindingError>;
    fn poison_all(&mut self);
}

struct Attempt<'a, B: CheckpointBackend> {
    backend: &'a mut B,
    poisoned: &'a mut bool,
    committed: bool,
}

impl<B: CheckpointBackend> Drop for Attempt<'_, B> {
    fn drop(&mut self) {
        if !self.committed {
            *self.poisoned = true;
            self.backend.poison_all();
        }
    }
}

fn deadline<B: CheckpointBackend>(
    backend: &mut B,
    until: Instant,
) -> Result<(), DeviceBindingError> {
    if backend.now() >= until {
        return Err(changed("scoped currentness deadline expired"));
    }
    Ok(())
}

fn identities<B: CheckpointBackend>(backend: &mut B) -> Result<[u64; 2], DeviceBindingError> {
    if backend.participants() != 2 {
        return Err(changed(
            "scoped currentness requires exactly two participants",
        ));
    }
    let ids = [backend.identity(0)?, backend.identity(1)?];
    if ids.contains(&0) || ids[0] == ids[1] {
        return Err(changed("scoped currentness participant identities"));
    }
    Ok(ids)
}

fn selected_identities<B: CheckpointBackend>(
    backend: &mut B,
    expected: &[u64],
) -> Result<(), DeviceBindingError> {
    if !matches!(expected.len(), 1 | 2) || backend.participants() != expected.len() {
        return Err(changed("scoped selected participant census"));
    }
    for (rank, expected) in expected.iter().enumerate() {
        if *expected == 0 || backend.identity(rank)? != *expected {
            return Err(changed("scoped selected participant substitution"));
        }
    }
    Ok(())
}

fn before<B: CheckpointBackend>(
    backend: &mut B,
    counts: &mut ScopedCountsV1,
    until: Instant,
) -> Result<(), DeviceBindingError> {
    for rank in 0..backend.participants() {
        deadline(backend, until)?;
        increment(&mut counts.before_calls)?;
        backend.before(rank)?;
        deadline(backend, until)?;
    }
    Ok(())
}

fn after<B: CheckpointBackend>(
    backend: &mut B,
    counts: &mut ScopedCountsV1,
    until: Instant,
) -> Result<(), DeviceBindingError> {
    for rank in 0..backend.participants() {
        deadline(backend, until)?;
        increment(&mut counts.after_calls)?;
        backend.after(rank)?;
        deadline(backend, until)?;
    }
    Ok(())
}

fn generation<B: CheckpointBackend>(
    backend: &mut B,
    snapshot: &B::Snapshot,
    counts: &mut ScopedCountsV1,
    until: Instant,
) -> Result<(), DeviceBindingError> {
    deadline(backend, until)?;
    increment(&mut counts.generation_probes)?;
    backend.root_generation(snapshot)?;
    deadline(backend, until)
}

fn full<B: CheckpointBackend>(
    backend: &mut B,
    counts: &mut ScopedCountsV1,
    until: Instant,
) -> Result<B::Snapshot, DeviceBindingError> {
    before(backend, counts, until)?;
    increment(&mut counts.full_discoveries)?;
    let snapshot = backend.discover()?;
    deadline(backend, until)?;
    for rank in 0..2 {
        backend.compare_retained(rank, &snapshot)?;
        deadline(backend, until)?;
    }
    after(backend, counts, until)?;
    generation(backend, &snapshot, counts, until)?;
    Ok(snapshot)
}

// Owned observation data, not a borrow of Context devices and not a certificate.
// The future closed LayerOperation must quarantine its owners on abandonment.
struct Window<S> {
    entry: S,
    identities: [u64; 2],
    until: Instant,
    poisoned: bool,
    closed: bool,
    counts: ScopedCountsV1,
}

impl<S: Eq> Window<S> {
    fn enter<B: CheckpointBackend<Snapshot = S>>(
        backend: &mut B,
        until: Instant,
    ) -> Result<Self, DeviceBindingError> {
        let mut poisoned = false;
        let mut attempt = Attempt {
            backend,
            poisoned: &mut poisoned,
            committed: false,
        };
        deadline(attempt.backend, until)?;
        let identities = identities(attempt.backend)?;
        let mut counts = ScopedCountsV1::default();
        let entry = full(attempt.backend, &mut counts, until)?;
        // Identity is checked again after the complete entry observation.
        if identities != self::identities(attempt.backend)? {
            return Err(changed("scoped entry roster changed"));
        }
        deadline(attempt.backend, until)?;
        attempt.committed = true;
        Ok(Self {
            entry,
            identities,
            until,
            poisoned: false,
            closed: false,
            counts,
        })
    }

    fn checkpoint<B: CheckpointBackend<Snapshot = S>>(
        &mut self,
        backend: &mut B,
    ) -> Result<(), DeviceBindingError> {
        self.checkpoint_selected(backend, None)
    }

    fn checkpoint_rank<B: CheckpointBackend<Snapshot = S>>(
        &mut self,
        backend: &mut B,
        rank: usize,
    ) -> Result<(), DeviceBindingError> {
        self.checkpoint_selected(backend, Some(rank))
    }

    fn checkpoint_selected<B: CheckpointBackend<Snapshot = S>>(
        &mut self,
        backend: &mut B,
        selected: Option<usize>,
    ) -> Result<(), DeviceBindingError> {
        let was_poisoned = self.poisoned;
        let mut attempt = Attempt {
            backend,
            poisoned: &mut self.poisoned,
            committed: false,
        };
        if was_poisoned || self.closed {
            return Err(changed("scoped currentness is closed or poisoned"));
        }
        let expected = match selected {
            None => &self.identities[..],
            Some(rank) => self
                .identities
                .get(
                    rank..rank
                        .checked_add(1)
                        .ok_or_else(|| changed("scoped rank overflow"))?,
                )
                .ok_or_else(|| changed("scoped participant rank"))?,
        };
        deadline(attempt.backend, self.until)?;
        selected_identities(attempt.backend, expected)?;
        generation(attempt.backend, &self.entry, &mut self.counts, self.until)?;
        before(attempt.backend, &mut self.counts, self.until)?;
        after(attempt.backend, &mut self.counts, self.until)?;
        generation(attempt.backend, &self.entry, &mut self.counts, self.until)?;
        selected_identities(attempt.backend, expected)?;
        deadline(attempt.backend, self.until)?;
        increment(&mut self.counts.local_checkpoints)?;
        attempt.committed = true;
        Ok(())
    }

    fn finish<B: CheckpointBackend<Snapshot = S>>(
        &mut self,
        backend: &mut B,
    ) -> Result<ScopedCountsV1, DeviceBindingError> {
        let was_poisoned = self.poisoned;
        let mut attempt = Attempt {
            backend,
            poisoned: &mut self.poisoned,
            committed: false,
        };
        if was_poisoned || self.closed {
            return Err(changed("scoped currentness is closed or poisoned"));
        }
        deadline(attempt.backend, self.until)?;
        if identities(attempt.backend)? != self.identities {
            return Err(changed("scoped exit roster changed"));
        }
        generation(attempt.backend, &self.entry, &mut self.counts, self.until)?;
        let observed = full(attempt.backend, &mut self.counts, self.until)?;
        if observed != self.entry || identities(attempt.backend)? != self.identities {
            return Err(changed("scoped exit full observation changed"));
        }
        deadline(attempt.backend, self.until)?;
        self.closed = true;
        attempt.committed = true;
        Ok(self.counts)
    }
}

struct Native<'a, 'device> {
    devices: &'a mut [&'device mut CheckedGfx950XnackMinusDevice],
}

impl CheckpointBackend for Native<'_, '_> {
    type Snapshot = HostTopologySnapshot;
    fn now(&mut self) -> Instant {
        Instant::now()
    }
    fn participants(&self) -> usize {
        self.devices.len()
    }
    fn identity(&mut self, rank: usize) -> Result<u64, DeviceBindingError> {
        let device = &self.devices[rank];
        if device.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        Ok(device.observation.unique_id())
    }
    fn before(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        if self.devices[rank].currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        self.devices[rank].check_currentness_before_topology()
    }
    fn discover(&mut self) -> Result<Self::Snapshot, DeviceBindingError> {
        topology::discover_default_topology_for_target(topology::GfxTarget::Gfx950)
            .map_err(Into::into)
    }
    fn compare_retained(
        &mut self,
        rank: usize,
        snapshot: &Self::Snapshot,
    ) -> Result<(), DeviceBindingError> {
        if *snapshot != self.devices[rank].topology {
            return Err(DeviceBindingError::TopologySnapshotChanged);
        }
        Ok(())
    }
    fn after(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        self.devices[rank].check_currentness_after_topology()
    }
    fn root_generation(&mut self, snapshot: &Self::Snapshot) -> Result<(), DeviceBindingError> {
        topology::check_scoped_topology_root_generation(snapshot).map_err(Into::into)
    }
    fn poison_all(&mut self) {
        for device in &mut *self.devices {
            device.currentness_poisoned = true;
        }
    }
}

/// Internal two-rank observation state. No queue/idle/owner/retirement authority.
/// The retaining Group operation must poison all owners on drop without finish.
pub(crate) struct ScopedCurrentnessV1(Window<HostTopologySnapshot>);

impl ScopedCurrentnessV1 {
    pub(crate) fn enter(
        devices: &mut [&mut CheckedGfx950XnackMinusDevice],
        until: Instant,
    ) -> Result<Self, DeviceBindingError> {
        Window::enter(&mut Native { devices }, until).map(Self)
    }
    pub(crate) fn checkpoint(
        &mut self,
        devices: &mut [&mut CheckedGfx950XnackMinusDevice],
    ) -> Result<(), DeviceBindingError> {
        self.0.checkpoint(&mut Native { devices })
    }
    pub(crate) fn checkpoint_rank(
        &mut self,
        rank: usize,
        device: &mut CheckedGfx950XnackMinusDevice,
    ) -> Result<(), DeviceBindingError> {
        self.0.checkpoint_rank(
            &mut Native {
                devices: &mut [device],
            },
            rank,
        )
    }
    pub(crate) fn finish(
        &mut self,
        devices: &mut [&mut CheckedGfx950XnackMinusDevice],
    ) -> Result<ScopedCountsV1, DeviceBindingError> {
        self.0.finish(&mut Native { devices })
    }
    pub(crate) fn poison(&mut self, devices: &mut [&mut CheckedGfx950XnackMinusDevice]) {
        self.0.poisoned = true;
        Native { devices }.poison_all();
    }
    pub(crate) fn poison_devices(devices: &mut [&mut CheckedGfx950XnackMinusDevice]) {
        Native { devices }.poison_all();
    }
}

#[cfg(test)]
#[path = "device_gfx950_scoped_currentness_v1_tests.rs"]
mod tests;
