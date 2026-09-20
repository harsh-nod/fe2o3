//! Full observation sequencing shared by single-device and XGMI pair checks.

#![forbid(unsafe_code)]

use crate::currentness_diagnostic::{Disabled, Mode, Timing};
use crate::device::{DeviceBindingError, validate_apertures};
use crate::topology::{Gfx942XgmiRouteV1, HostTopologySnapshot};
use crate::{CheckedGfx942XnackMinusDevice, KfdUapiVersion};

trait Observation {
    type Process: Copy + Eq;
    type Drm: Copy + Eq;
    type Apertures: Eq;
    type Topology: Eq;

    fn poisoned(&self) -> bool;
    fn set_poisoned(&mut self, value: bool);
    fn retained_process(&self) -> Self::Process;
    fn retained_drm(&self) -> Self::Drm;
    fn retained_uapi(&self) -> KfdUapiVersion;
    fn retained_apertures(&self) -> &Self::Apertures;
    fn retained_topology(&self) -> &Self::Topology;
    fn ensure_opener_process(&mut self) -> Result<(), DeviceBindingError>;
    fn observe_process(&mut self) -> Result<Self::Process, DeviceBindingError>;
    fn check_reset(&mut self) -> Result<(), DeviceBindingError>;
    fn validate_kfd_before(&mut self) -> Result<(), DeviceBindingError>;
    fn validate_kfd_after(&mut self) -> Result<(), DeviceBindingError>;
    fn validate_render(&mut self) -> Result<(), DeviceBindingError>;
    fn observe_uapi(&mut self) -> Result<KfdUapiVersion, DeviceBindingError>;
    fn observe_drm(&mut self) -> Result<Self::Drm, DeviceBindingError>;
    fn observe_xnack(&mut self) -> Result<i32, DeviceBindingError>;
    fn observe_apertures(&mut self) -> Result<Self::Apertures, DeviceBindingError>;
    fn discover<M: Mode>(&mut self) -> Result<(Self::Topology, M::Topology), DeviceBindingError>;
    fn validate_route(
        &mut self,
        snapshot: &Self::Topology,
        route: Gfx942XgmiRouteV1,
    ) -> Result<(), DeviceBindingError>;
    fn vram_lost_counter(drm: Self::Drm) -> u32;
}

struct Before<P, D> {
    process: P,
    drm: D,
}

fn before<O: Observation>(
    observation: &mut O,
) -> Result<Before<O::Process, O::Drm>, DeviceBindingError> {
    observation.ensure_opener_process()?;
    let process = observation.observe_process()?;
    if process != observation.retained_process() {
        return Err(DeviceBindingError::ProcessIncarnationChanged);
    }
    observation.check_reset()?;
    observation.validate_kfd_before()?;
    observation.validate_render()?;
    if observation.observe_uapi()? != observation.retained_uapi() {
        return Err(DeviceBindingError::UapiChanged);
    }
    let drm = observation.observe_drm()?;
    if drm != observation.retained_drm() {
        return Err(DeviceBindingError::ObservableCurrentnessChanged(
            "DRM identity or VRAM-loss counter",
        ));
    }
    if observation.observe_xnack()? != 0 {
        return Err(DeviceBindingError::UnsupportedXnackMode);
    }
    if &observation.observe_apertures()? != observation.retained_apertures() {
        return Err(DeviceBindingError::AperturesChanged);
    }
    Ok(Before { process, drm })
}

fn after<O: Observation>(
    observation: &mut O,
    before: Before<O::Process, O::Drm>,
) -> Result<u32, DeviceBindingError> {
    observation.validate_kfd_after()?;
    observation.validate_render()?;
    let process = observation.observe_process()?;
    if process != before.process || process != observation.retained_process() {
        return Err(DeviceBindingError::ProcessIncarnationChanged);
    }
    if observation.observe_xnack()? != 0 {
        return Err(DeviceBindingError::UnsupportedXnackMode);
    }
    let drm = observation.observe_drm()?;
    if drm != before.drm {
        return Err(DeviceBindingError::ObservableCurrentnessChanged(
            "DRM identity or VRAM-loss counter during currentness check",
        ));
    }
    observation.check_reset()?;
    Ok(O::vram_lost_counter(drm))
}

fn single<O: Observation>(observation: &mut O) -> Result<u32, DeviceBindingError> {
    let opening = before(observation)?;
    if &observation.discover::<Disabled>()?.0 != observation.retained_topology() {
        return Err(DeviceBindingError::TopologySnapshotChanged);
    }
    after(observation, opening)
}

fn exact_route(
    observed: Gfx942XgmiRouteV1,
    expected: Gfx942XgmiRouteV1,
) -> Result<(), DeviceBindingError> {
    if observed != expected {
        return Err(DeviceBindingError::ObservableCurrentnessChanged(
            "directional XGMI topology route",
        ));
    }
    Ok(())
}

fn pair<M: Mode, O: Observation>(
    source: &mut O,
    peer: &mut O,
    route: Gfx942XgmiRouteV1,
) -> Result<M::Pair, DeviceBindingError> {
    let already_poisoned = source.poisoned() || peer.poisoned();
    // The only production implementation uses infallible field assignments.
    // Pre-latching retains both failures and unwinds without a cleanup callback.
    source.set_poisoned(true);
    peer.set_poisoned(true);
    if already_poisoned {
        return Err(DeviceBindingError::CurrentnessFencePoisoned);
    }
    let mut timer = M::Timer::<6>::new();
    let source_before = timer.measure(0, || before(source))?;
    let peer_before = timer.measure(1, || before(peer))?;
    let (snapshot, topology) = timer.measure(2, || source.discover::<M>())?;
    timer.measure(3, || {
        source.validate_route(&snapshot, route)?;
        if &snapshot != source.retained_topology() || &snapshot != peer.retained_topology() {
            return Err(DeviceBindingError::TopologySnapshotChanged);
        }
        Ok(())
    })?;
    timer.measure(4, || after(source, source_before))?;
    timer.measure(5, || after(peer, peer_before))?;
    let diagnostic = M::pair(timer, topology);
    source.set_poisoned(false);
    peer.set_poisoned(false);
    Ok(diagnostic)
}

pub(super) fn check_single(
    device: &mut CheckedGfx942XnackMinusDevice,
) -> Result<super::ObservableDeviceCurrentnessV1, DeviceBindingError> {
    single(device)
        .map(|vram_lost_counter| super::ObservableDeviceCurrentnessV1 { vram_lost_counter })
}

pub(super) fn check_pair<M: Mode>(
    source: &mut CheckedGfx942XnackMinusDevice,
    peer: &mut CheckedGfx942XnackMinusDevice,
    route: Gfx942XgmiRouteV1,
) -> Result<M::Pair, DeviceBindingError> {
    pair::<M, _>(source, peer, route)
}

impl Observation for CheckedGfx942XnackMinusDevice {
    type Process = crate::device::ProcessIncarnationObservation;
    type Drm = crate::device::DrmIdentityObservation;
    type Apertures = Vec<crate::device::ProcessApertureObservation>;
    type Topology = HostTopologySnapshot;

    fn poisoned(&self) -> bool {
        self.currentness_poisoned
    }

    fn set_poisoned(&mut self, value: bool) {
        self.currentness_poisoned = value;
    }

    fn retained_process(&self) -> Self::Process {
        self.process
    }

    fn retained_drm(&self) -> Self::Drm {
        self.observation.drm()
    }

    fn retained_uapi(&self) -> KfdUapiVersion {
        self.kfd.uapi.reported_version()
    }

    fn retained_apertures(&self) -> &Self::Apertures {
        &self.apertures
    }

    fn retained_topology(&self) -> &Self::Topology {
        &self.topology
    }

    fn ensure_opener_process(&mut self) -> Result<(), DeviceBindingError> {
        self.kfd
            .opened
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)
    }

    fn observe_process(&mut self) -> Result<Self::Process, DeviceBindingError> {
        crate::linux::observe_process_incarnation()
    }

    fn check_reset(&mut self) -> Result<(), DeviceBindingError> {
        self.reset_fence.check_clear()
    }

    fn validate_kfd_before(&mut self) -> Result<(), DeviceBindingError> {
        crate::linux::validate_kfd_descriptor_and_sysfs(
            &self.kfd.opened.fd,
            self.kfd.opened.node_observation(),
        )
        .map(|_| ())
    }

    fn validate_kfd_after(&mut self) -> Result<(), DeviceBindingError> {
        crate::linux::revalidate_descriptor(
            &self.kfd.opened.fd,
            self.kfd.opened.node_observation(),
            "KFD currentness fstat",
        )
    }

    fn validate_render(&mut self) -> Result<(), DeviceBindingError> {
        crate::linux::revalidate_render_descriptor(
            &self.render_fd,
            self.observation.render_descriptor(),
        )
    }

    fn observe_uapi(&mut self) -> Result<KfdUapiVersion, DeviceBindingError> {
        crate::linux::observe_uapi(&self.kfd.opened.fd).map_err(DeviceBindingError::Kfd)
    }

    fn observe_drm(&mut self) -> Result<Self::Drm, DeviceBindingError> {
        crate::linux::observe_drm_identity(&self.render_fd)
    }

    fn observe_xnack(&mut self) -> Result<i32, DeviceBindingError> {
        crate::linux::query_xnack_mode(&self.kfd.opened.fd)
    }

    fn observe_apertures(&mut self) -> Result<Self::Apertures, DeviceBindingError> {
        validate_apertures(
            crate::linux::observe_process_apertures(&self.kfd.opened.fd)?,
            &self.topology,
        )
    }

    fn discover<M: Mode>(&mut self) -> Result<(Self::Topology, M::Topology), DeviceBindingError> {
        crate::topology::discover_default_topology_with::<M>().map_err(DeviceBindingError::Topology)
    }

    fn validate_route(
        &mut self,
        snapshot: &Self::Topology,
        route: Gfx942XgmiRouteV1,
    ) -> Result<(), DeviceBindingError> {
        let observed = snapshot
            .topology()
            .admit_gfx942_xgmi_route(route.source_gpu_id(), route.destination_gpu_id())
            .map_err(|_| {
                DeviceBindingError::ObservableCurrentnessChanged("observed XGMI topology route")
            })?;
        exact_route(observed, route)
    }

    fn vram_lost_counter(drm: Self::Drm) -> u32 {
        drm.vram_lost_counter()
    }
}

#[cfg(test)]
mod tests;
