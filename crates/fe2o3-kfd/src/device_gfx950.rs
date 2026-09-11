//! Separate MI350 checked observations; no gfx942 or execution authority.

use super::*;

#[cfg(feature = "engineering-gfx950")]
#[path = "device_gfx950_group_currentness.rs"]
mod group_currentness;
#[cfg(feature = "engineering-gfx950")]
pub(crate) use group_currentness::check_engineering_group_currentness;

pub const GFX950_ADMITTED_KERNEL_RELEASE_V1: &str = "6.8.0-124-generic";
pub const GFX950_ADMITTED_AMDGPU_MODULE_VERSION_V1: &str = "6.16.13";
pub const GFX950_ADMITTED_AMDGPU_MODULE_SRCVERSION_V1: &str = "703B1127E578BC5D4BD6615";

/// Exact checked-observation profile. Source hashes name reviewed contracts;
/// they do not authenticate the loaded driver, firmware, or hardware.
pub const GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1: &str = concat!(
    "profile_id=fe2o3-linux-x86_64-mi350x-gfx950-xnack-minus-spx-nps1-observation-v1\n",
    "kfd_schema_sha256=e4aad5d8e3177ea6d70298adab7741c377cb091373553ce689f3525e7514d9b4\n",
    "drm_schema_sha256=800569fe9b467b389bcfc6e5d65b23d66a0386a90fc2a669fac8c83800e76d8b\n",
    "kernel_release=6.8.0-124-generic\n",
    "amdgpu_module=6.16.13\n",
    "amdgpu_srcversion=703B1127E578BC5D4BD6615\n",
    "source.kfd_ioctl.h=b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d\n",
    "source.amdgpu_drm.h=9d7ff60a211d2aa73a6c15b2da49e050cebe518fc059ee93e31d61288f7b60dc\n",
    "source.core_drm.h=3ab6ac01bf91067aed96b70d7fa7847a86e7f726d74278151f085143688659cc\n",
    "source.core_drm_mode.h=6f1e99012854f40c59e62ba9ab031aa6e0f7354f41f25d0a9d23e6dfc6bd370b\n",
    "source.kfd_chardev.c=f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba\n",
    "source.kfd_smi_events.c=2d786562fe1e97b8257841b755106c8bce47658a2aa3b439ce4e0178323004bd\n",
    "source.amdgpu_kms.c=ef2375c3f35ad4a24b560326b55676a907d6d2ba248e469a62e84e877435101c\n",
    "source.soc15.c=e51e3c71fd479abe29b1ab667474de1a8416243e6a83bc9ce743306b1b17aea8\n",
    "source.amdgpu_discovery.c=f931372a3632e2c753897c80e90c300f570ebeb51a2c8021bda311cdbdde5c85\n",
    "uapi=kfd:1.18,drm:3.64.0\n",
    "target=gfx950:90500,wavefront:64,simd:1024,xcc:8\n",
    "pci=vendor:1002,device:75a0,revision:00\n",
    "drm_device=chip_rev:0,external_rev:80,family:141,acceleration:1\n",
    "partition=SPX/NPS1\n",
    "firmware_observation=compute:41,sdma:12\n",
    "xnack=disabled-query-only,no-setter,no-no-queue-barrier-claim\n",
    "apertures=max:16,count-fill-count,complete-topology-inventory,page-aligned,inclusive,record-disjoint\n",
    "currentness=process,retained-fds,full-topology,xnack,apertures,prospective-smi-reset-events,vram-loss-counter,poison-on-error,no-all-reset-or-aba-proof\n",
    "authority=checked-observation-only,no-model-admission,no-explicit-vm-acquisition,no-vm-authority,no-memory,no-queue,no-dispatch,no-gfx942-conversion\n",
);

/// SHA-256 of the canonical checked-observation profile.
pub fn gfx950_device_observation_profile_sha256_v1() -> [u8; 32] {
    Sha256::digest(GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1).into()
}

/// Retained, process-bound MI350 observations, distinct from gfx942 authority.
///
/// Binding performs read-only identity/XNACK/aperture queries and subscribes a
/// process-owned reset-event descriptor. It does not set XNACK, establish a
/// no-existing-queue barrier, or grant a runtime-model device/VM generation.
/// Dropping closes its descriptors. There is no memory, queue, or conversion
/// API. These observations are contracted, not an all-reset or ABA proof.
///
/// ```compile_fail
/// use fe2o3_kfd::{CheckedGfx942XnackMinusDevice, CheckedGfx950XnackMinusDevice};
/// fn cannot_relabel(device: CheckedGfx950XnackMinusDevice) {
///     let _: CheckedGfx942XnackMinusDevice = device;
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::CheckedGfx950XnackMinusDevice;
/// fn cannot_allocate(device: CheckedGfx950XnackMinusDevice) {
///     let _ = device.acquire_host_visible_memory_session();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kfd::CheckedGfx950XnackMinusDevice;
/// fn cannot_transfer(device: CheckedGfx950XnackMinusDevice) {
///     std::thread::spawn(move || drop(device));
/// }
/// ```
pub struct CheckedGfx950XnackMinusDevice {
    kfd: KfdWithAdmittedUapi,
    render_fd: OwnedFd,
    render_path: PathBuf,
    topology: HostTopologySnapshot,
    apertures: Vec<ProcessApertureObservation>,
    observation: DeviceBindingObservation,
    process: ProcessIncarnationObservation,
    reset_fence: crate::currentness::ResetEventFence,
    currentness_poisoned: bool,
    not_send_sync: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl fmt::Debug for CheckedGfx950XnackMinusDevice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx950XnackMinusDevice")
            .field("observation", &self.observation)
            .field("process", &self.process)
            .field("currentness_poisoned", &self.currentness_poisoned)
            .finish_non_exhaustive()
    }
}

impl CheckedGfx950XnackMinusDevice {
    #[cfg(feature = "engineering-gfx950")]
    pub(crate) fn kfd_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        use std::os::fd::AsFd;
        self.kfd.opened.fd.as_fd()
    }

    #[cfg(feature = "engineering-gfx950")]
    pub(crate) fn render_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        use std::os::fd::AsFd;
        self.render_fd.as_fd()
    }

    pub fn topology_snapshot(&self) -> &HostTopologySnapshot {
        &self.topology
    }
    pub fn process_apertures(&self) -> &[ProcessApertureObservation] {
        &self.apertures
    }
    pub const fn observation(&self) -> &DeviceBindingObservation {
        &self.observation
    }
    pub fn render_opening_path(&self) -> &Path {
        &self.render_path
    }
    pub const fn process_incarnation(&self) -> ProcessIncarnationObservation {
        self.process
    }

    /// Reports retained descriptors without exposing raw descriptor access.
    pub const fn descriptor_count(&self) -> usize {
        3
    }

    /// Rechecks every retained observation. Any failure permanently poisons
    /// this token; subsequent calls cannot recover or grant authority.
    pub fn check_observable_currentness(&mut self) -> Result<(), DeviceBindingError> {
        if self.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let result = self.check_currentness_inner();
        if result.is_err() {
            self.currentness_poisoned = true;
        }
        result
    }

    /// Engineering-only retained-queue fence. Full topology/aperture checks
    /// remain mandatory at lifecycle boundaries; no public observation API
    /// changes its contract. Any reset, descriptor, mode, or process failure
    /// permanently invalidates the same token used by the full fence.
    #[cfg(feature = "engineering-gfx950")]
    pub(crate) fn check_engineering_operational_currentness(
        &mut self,
    ) -> Result<(), DeviceBindingError> {
        if self.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let result = (|| {
            self.kfd
                .opened
                .ensure_process(std::process::id())
                .map_err(DeviceBindingError::Kfd)?;
            if crate::linux::observe_process_incarnation()? != self.process {
                return Err(DeviceBindingError::ProcessIncarnationChanged);
            }
            self.reset_fence.check_clear()?;
            crate::linux::revalidate_descriptor(
                &self.kfd.opened.fd,
                self.kfd.opened.node_observation(),
                "gfx950 engineering operational currentness fstat",
            )?;
            crate::linux::revalidate_render_descriptor(
                &self.render_fd,
                self.observation.render_descriptor(),
            )?;
            if crate::linux::observe_uapi(&self.kfd.opened.fd)? != self.kfd.uapi.reported_version()
            {
                return Err(DeviceBindingError::UapiChanged);
            }
            if crate::linux::query_xnack_mode(&self.kfd.opened.fd)? != 0 {
                return Err(DeviceBindingError::UnsupportedXnackMode);
            }
            if crate::linux::observe_drm_identity(&self.render_fd)? != self.observation.drm() {
                return Err(DeviceBindingError::ObservableCurrentnessChanged(
                    "DRM identity or VRAM-loss counter",
                ));
            }
            if crate::linux::observe_process_incarnation()? != self.process {
                return Err(DeviceBindingError::ProcessIncarnationChanged);
            }
            self.reset_fence.check_clear()
        })();
        if result.is_err() {
            self.currentness_poisoned = true;
        }
        result
    }

    fn check_currentness_inner(&mut self) -> Result<(), DeviceBindingError> {
        self.check_currentness_before_topology()?;
        let topology = topology::discover_default_topology_for_target(topology::GfxTarget::Gfx950)?;
        if topology != self.topology {
            return Err(DeviceBindingError::TopologySnapshotChanged);
        }
        self.check_currentness_after_topology()
    }

    fn check_currentness_before_topology(&mut self) -> Result<(), DeviceBindingError> {
        self.kfd
            .opened
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)?;
        let process_before = crate::linux::observe_process_incarnation()?;
        if process_before != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        // Never consume an inherited reset FIFO before validating its process.
        self.reset_fence.check_clear()?;
        crate::linux::validate_kfd_descriptor_and_sysfs(
            &self.kfd.opened.fd,
            self.kfd.opened.node_observation(),
        )?;
        crate::linux::revalidate_render_descriptor(
            &self.render_fd,
            self.observation.render_descriptor(),
        )?;
        if crate::linux::observe_uapi(&self.kfd.opened.fd)? != self.kfd.uapi.reported_version() {
            return Err(DeviceBindingError::UapiChanged);
        }
        let drm = crate::linux::observe_drm_identity(&self.render_fd)?;
        if drm != self.observation.drm() {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "DRM identity or VRAM-loss counter",
            ));
        }
        if crate::linux::query_xnack_mode(&self.kfd.opened.fd)? != 0 {
            return Err(DeviceBindingError::UnsupportedXnackMode);
        }
        let apertures = validate_apertures(
            crate::linux::observe_process_apertures(&self.kfd.opened.fd)?,
            &self.topology,
        )?;
        if apertures != self.apertures {
            return Err(DeviceBindingError::AperturesChanged);
        }
        Ok(())
    }

    fn check_currentness_after_topology(&mut self) -> Result<(), DeviceBindingError> {
        crate::linux::revalidate_descriptor(
            &self.kfd.opened.fd,
            self.kfd.opened.node_observation(),
            "KFD currentness fstat",
        )?;
        crate::linux::revalidate_render_descriptor(
            &self.render_fd,
            self.observation.render_descriptor(),
        )?;
        if crate::linux::observe_process_incarnation()? != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        if crate::linux::query_xnack_mode(&self.kfd.opened.fd)? != 0 {
            return Err(DeviceBindingError::UnsupportedXnackMode);
        }
        if crate::linux::observe_drm_identity(&self.render_fd)? != self.observation.drm() {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "DRM identity during currentness check",
            ));
        }
        self.reset_fence.check_clear()
    }
}

impl KfdWithAdmittedUapi {
    /// Checks an explicitly selected MI350 without setting XNACK, explicitly
    /// acquiring a VM, allocating or mapping GPU memory, or creating a queue.
    /// Opening descriptors may create driver-internal per-process state; this
    /// token grants no VM, gfx942, runtime-model, code, or dispatch authority.
    pub fn bind_gfx950_xnack_minus(
        self,
        selector: DeviceSelector,
    ) -> Result<CheckedGfx950XnackMinusDevice, DeviceBindingError> {
        let _admission_transaction = match DEVICE_ADMISSION_LEASE.try_lock() {
            Ok(lease) => lease,
            Err(TryLockError::WouldBlock) => return Err(DeviceBindingError::AdmissionInProgress),
            Err(TryLockError::Poisoned(_)) => {
                return Err(DeviceBindingError::AdmissionLeasePoisoned);
            }
        };
        self.opened
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)?;
        let process_before = crate::linux::observe_process_incarnation()?;
        crate::linux::validate_kfd_descriptor_and_sysfs(
            &self.opened.fd,
            self.opened.node_observation(),
        )?;
        let snapshot = topology::discover_default_topology_for_target(topology::GfxTarget::Gfx950)?;
        validate_platform(
            snapshot.kernel_release().as_str(),
            snapshot.amdgpu_module().version(),
            snapshot.amdgpu_module().srcversion(),
        )?;
        let (gpu, render_sysfs) = select_unique(&snapshot, selector)?;
        let gpu_id = u32::try_from(gpu.gpu_id())
            .map_err(|_| DeviceBindingError::GpuIdOutOfRange(gpu.gpu_id()))?;
        if crate::linux::observe_uapi(&self.opened.fd)? != self.uapi.reported_version() {
            return Err(DeviceBindingError::UapiChanged);
        }
        let render = crate::linux::open_and_observe_render(gpu.drm_render_minor())?;
        let facts = ProfileFacts::observe(gpu, render_sysfs, &render);
        validate_profile(facts)?;
        let mut reset_fence =
            crate::currentness::ResetEventFence::subscribe(&self.opened.fd, gpu_id)?;
        if crate::linux::observe_drm_identity(&render.fd)? != render.drm {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "DRM identity across reset subscription",
            ));
        }
        if crate::linux::query_xnack_mode(&self.opened.fd)? != 0 {
            return Err(DeviceBindingError::UnsupportedXnackMode);
        }
        let apertures = validate_apertures(
            crate::linux::observe_process_apertures(&self.opened.fd)?,
            &snapshot,
        )?;
        if crate::linux::query_xnack_mode(&self.opened.fd)? != 0 {
            return Err(DeviceBindingError::UnsupportedXnackMode);
        }
        let reobserved_apertures = validate_apertures(
            crate::linux::observe_process_apertures(&self.opened.fd)?,
            &snapshot,
        )?;
        if apertures != reobserved_apertures {
            return Err(DeviceBindingError::AperturesChanged);
        }
        if topology::discover_default_topology_for_target(topology::GfxTarget::Gfx950)? != snapshot
        {
            return Err(DeviceBindingError::TopologySnapshotChanged);
        }
        crate::linux::revalidate_descriptor(
            &self.opened.fd,
            self.opened.node_observation(),
            "KFD",
        )?;
        crate::linux::revalidate_render_descriptor(&render.fd, render.descriptor)?;
        if crate::linux::observe_process_incarnation()? != process_before {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        if crate::linux::observe_uapi(&self.opened.fd)? != self.uapi.reported_version() {
            return Err(DeviceBindingError::UapiChanged);
        }
        if crate::linux::observe_drm_identity(&render.fd)? != render.drm {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "DRM identity at device commit",
            ));
        }
        let aperture = apertures
            .iter()
            .find(|aperture| aperture.gpu_id == gpu_id)
            .copied()
            .ok_or(DeviceBindingError::SelectedApertureMissing(gpu_id))?;
        reset_fence.check_clear()?;
        let observation = DeviceBindingObservation {
            topology_node_id: gpu.node_id(),
            kfd_gpu_id: gpu_id,
            unique_id: gpu.unique_id(),
            pci: render_sysfs.pci_address(),
            render_minor: gpu.drm_render_minor(),
            render_descriptor: render.descriptor,
            drm: render.drm,
            aperture,
        };
        Ok(CheckedGfx950XnackMinusDevice {
            kfd: self,
            render_fd: render.fd,
            render_path: render.path,
            topology: snapshot,
            apertures,
            observation,
            process: process_before,
            reset_fence,
            currentness_poisoned: false,
            not_send_sync: std::marker::PhantomData,
        })
    }
}

fn validate_platform(
    kernel: &str,
    version: Option<&str>,
    source: Option<&str>,
) -> Result<(), DeviceBindingError> {
    if kernel != GFX950_ADMITTED_KERNEL_RELEASE_V1 {
        return Err(DeviceBindingError::UnsupportedKernelRelease(
            kernel.to_owned(),
        ));
    }
    if version != Some(GFX950_ADMITTED_AMDGPU_MODULE_VERSION_V1) {
        return Err(DeviceBindingError::UnsupportedModuleVersion(
            version.map(str::to_owned),
        ));
    }
    if source != Some(GFX950_ADMITTED_AMDGPU_MODULE_SRCVERSION_V1) {
        return Err(DeviceBindingError::UnsupportedModuleSourceVersion(
            source.map(str::to_owned),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ProfileFacts {
    target: topology::GfxTarget,
    device_id: u16,
    revision: u8,
    firmware: u32,
    sdma_firmware: u32,
    wavefront: u32,
    simds: u32,
    xccs: u32,
    partition: bool,
    render_major: u32,
    render_minor: u32,
    topology_render_minor: u16,
    drm: DrmIdentityObservation,
}

impl ProfileFacts {
    fn observe(
        gpu: &topology::GpuTopologyNode,
        sysfs: &topology::RenderNodeObservation,
        render: &OpenedRender,
    ) -> Self {
        Self {
            target: gpu.target(),
            device_id: gpu.pci_device_id(),
            revision: sysfs.pci_revision(),
            firmware: gpu.fw_version(),
            sdma_firmware: gpu.sdma_fw_version(),
            wavefront: gpu.capacity().wavefront_size(),
            simds: gpu.capacity().simd_count(),
            xccs: gpu.capacity().xcc_count(),
            partition: sysfs.partition() == V1_PARTITION_PROFILE,
            render_major: render.descriptor.major,
            render_minor: render.descriptor.minor,
            topology_render_minor: gpu.drm_render_minor(),
            drm: render.drm,
        }
    }
}

fn validate_profile(facts: ProfileFacts) -> Result<(), DeviceBindingError> {
    if facts.target != topology::GfxTarget::Gfx950 {
        return Err(DeviceBindingError::Topology(
            TopologyError::UnsupportedTarget {
                node_id: 0,
                encoded: u64::from(facts.target.encoded_version()),
            },
        ));
    }
    if facts.device_id != 0x75a0 {
        return Err(DeviceBindingError::UnsupportedPciDevice(facts.device_id));
    }
    if facts.revision != 0 {
        return Err(DeviceBindingError::UnsupportedPciRevision(facts.revision));
    }
    if facts.firmware != 41 || facts.sdma_firmware != 12 {
        return Err(DeviceBindingError::UnsupportedFirmware {
            compute: facts.firmware,
            sdma: facts.sdma_firmware,
        });
    }
    if facts.wavefront != 64 || facts.simds != 1024 || facts.xccs != 8 {
        return Err(DeviceBindingError::UnsupportedCapacity {
            wavefront: facts.wavefront,
            simds: facts.simds,
            xccs: facts.xccs,
        });
    }
    if !facts.partition {
        return Err(DeviceBindingError::UnsupportedPartition);
    }
    if facts.render_major != DRM_DEVICE_MAJOR
        || facts.render_minor != u32::from(facts.topology_render_minor)
    {
        return Err(DeviceBindingError::RenderDescriptorMismatch);
    }
    if facts.drm.driver_version != AMDGPU_DRM_DRIVER_VERSION {
        return Err(DeviceBindingError::UnsupportedDrmVersion(
            facts.drm.driver_version,
        ));
    }
    if facts.drm.acceleration_working != 1 {
        return Err(DeviceBindingError::AccelerationUnavailable(
            facts.drm.acceleration_working,
        ));
    }
    let device = facts.drm.device;
    if device.device_id != u32::from(facts.device_id)
        || device.pci_rev != u32::from(facts.revision)
        || device.family != AMDGPU_FAMILY_AI
        || device.chip_rev != 0
        || device.external_rev != 80
    {
        return Err(DeviceBindingError::DrmDeviceMismatch(device));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> ProfileFacts {
        ProfileFacts {
            target: topology::GfxTarget::Gfx950,
            device_id: 0x75a0,
            revision: 0,
            firmware: 41,
            sdma_firmware: 12,
            wavefront: 64,
            simds: 1024,
            xccs: 8,
            partition: true,
            render_major: 226,
            render_minor: 128,
            topology_render_minor: 128,
            drm: DrmIdentityObservation {
                driver_version: AMDGPU_DRM_DRIVER_VERSION,
                acceleration_working: 1,
                device: DrmAmdgpuDeviceIdentityV1 {
                    device_id: 0x75a0,
                    chip_rev: 0,
                    external_rev: 80,
                    pci_rev: 0,
                    family: AMDGPU_FAMILY_AI,
                },
                vram_lost_counter: 0,
            },
        }
    }

    #[test]
    fn exact_mi350_profile_is_separate_from_mi300() {
        validate_profile(facts()).unwrap();
        let mut gfx942 = facts();
        gfx942.target = topology::GfxTarget::Gfx942;
        assert!(matches!(
            validate_profile(gfx942),
            Err(DeviceBindingError::Topology(
                TopologyError::UnsupportedTarget { .. }
            ))
        ));
        assert!(
            validate_platform(
                GFX950_ADMITTED_KERNEL_RELEASE_V1,
                Some(GFX950_ADMITTED_AMDGPU_MODULE_VERSION_V1),
                Some(ADMITTED_AMDGPU_MODULE_SRCVERSION_V1)
            )
            .is_err()
        );
    }

    #[test]
    fn every_exact_profile_field_is_fail_closed() {
        let mutations: &[fn(&mut ProfileFacts)] = &[
            |v| v.device_id = 0x74a1,
            |v| v.revision = 1,
            |v| v.firmware = 192,
            |v| v.sdma_firmware = 25,
            |v| v.wavefront = 32,
            |v| v.simds = 1216,
            |v| v.xccs = 4,
            |v| v.partition = false,
            |v| v.render_major = 225,
            |v| v.render_minor = 129,
            |v| v.drm.driver_version.minor += 1,
            |v| v.drm.acceleration_working = 0,
            |v| v.drm.device.device_id = 0x74a1,
            |v| v.drm.device.pci_rev = 1,
            |v| v.drm.device.family = 0,
            |v| v.drm.device.chip_rev = 1,
            |v| v.drm.device.external_rev = 71,
        ];
        for (index, mutation) in mutations.iter().enumerate() {
            let mut input = facts();
            mutation(&mut input);
            assert!(
                validate_profile(input).is_err(),
                "accepted mutation {index}"
            );
        }
    }

    #[test]
    fn platform_observations_must_all_be_exact() {
        let kernel = GFX950_ADMITTED_KERNEL_RELEASE_V1;
        let version = Some(GFX950_ADMITTED_AMDGPU_MODULE_VERSION_V1);
        let source = Some(GFX950_ADMITTED_AMDGPU_MODULE_SRCVERSION_V1);
        validate_platform(kernel, version, source).unwrap();
        for changed in ["", "6.8.0-123-generic", "6.8.0-124-generic-extra"] {
            assert!(validate_platform(changed, version, source).is_err());
        }
        for changed in [None, Some("6.16.12"), Some("")] {
            assert!(validate_platform(kernel, changed, source).is_err());
        }
        for changed in [None, Some(ADMITTED_AMDGPU_MODULE_SRCVERSION_V1), Some("")] {
            assert!(validate_platform(kernel, version, changed).is_err());
        }
    }

    #[test]
    fn profile_explicitly_grants_no_execution_or_model_authority() {
        assert_ne!(
            gfx950_device_observation_profile_sha256_v1(),
            DEVICE_ADMISSION_PROFILE_SHA256_BYTES_V1
        );
        assert!(GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1.contains("no-model-admission,no-explicit-vm-acquisition,no-vm-authority,no-memory,no-queue,no-dispatch,no-gfx942-conversion"));
        assert!(
            GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1
                .contains("no-setter,no-no-queue-barrier-claim")
        );
        assert!(
            GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1.contains(
                KFD_UAPI_SCHEMA_MANIFEST_SHA256_BYTES
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    .as_str()
            )
        );
        assert!(
            GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1.contains(
                DRM_UAPI_SCHEMA_MANIFEST_SHA256_BYTES
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    .as_str()
            )
        );
    }
}
