//! Redacted, authority-free facts from the direct-KFD queue boundary.

use core::fmt;

use fe2o3_kfd_uapi::{
    KFD_GFX942_DOORBELL_BYTES, KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES,
    KFD_INTERNAL_SIGNAL_PAGE_SLOT_COUNT, admit_kfd_aql_queue_ring_size,
};
use sha2::{Digest, Sha256};

use crate::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueObservationV1, DEVICE_ADMISSION_PROFILE_SHA256_V1,
    DeviceBindingObservation, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1,
    GFX942_MAX_ADMITTED_RING_BYTES_V1, GFX942_MIN_ROCR_RING_BYTES_V1,
};

const EXPECTED_CWSR_SHADOW_PAGES: u8 = 8;
const EXPECTED_RELEASED_QUEUE_RESOURCES: u8 = 4;
const ZERO_SCOPE: [u8; 32] = [0; 32];

/// Canonical claim boundary for direct-KFD semantic observations.
///
/// Its digest identifies this schema. It does not authenticate KFD, the
/// kernel, firmware, hardware, or any dispatch.
pub const KFD_SEMANTIC_OBSERVATION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-direct-kfd-semantic-observation-r3-v1\n",
    "source.device_admission_sha256=e12ea33b259666e7928612403109640b03b0d637b893a2c15b87d17a4211c8de\n",
    "source.queue_session_sha256=367158d1f4b1edd6b09aaf8a993dae6d30ab7624d63a5400e858bd98c86c8ac5\n",
    "input=detached-device-binding-optional,detached-live-queue,detached-destroyed-queue-optional\n",
    "bounds=fixed-size,no-input-read,no-variable-allocation,no-device-enumeration\n",
    "identity=sha256-domain-separated-canonical-little-endian,caller-supplied-nonzero-scope,opaque-correlation-not-authentication-or-secrecy\n",
    "redaction=no-raw-queue-id,event-id,doorbell-offset,gpu-id,unique-id,pci-location,aperture,address,fd,handle-or-pointer-in-report\n",
    "observed=optional-device-binding,queue-lifecycle,ring-bytes,doorbell-slice-bytes,cwsr-shadow-page-count\n",
    "unavailable=queue-exception,dispatch-submission,dispatch-completion,dispatch-timing,kir,workgroup,wave,lane,memory-access,register-value\n",
    "destroy=confirmed-kfd-queue-event-runtime-and-resource-teardown,not-dispatch-completion-or-kernel-success\n",
    "trace=no-semantic-trace-v1-emission-without-authenticated-dispatch-and-completion\n",
    "authority=inert-read-only-report,no-fd,address,handle,queue,event,mmio,launch,wait-or-completion-authority\n",
);

/// SHA-256 of [`KFD_SEMANTIC_OBSERVATION_MANIFEST_V1`].
pub const KFD_SEMANTIC_OBSERVATION_MANIFEST_SHA256_V1: &str =
    "e595755fb337dccfd3ed2e0b02732c715ef00883920ae9568ad1caeeb804b0c0";

/// A caller-controlled correlation scope for one observation domain.
///
/// The scope prevents identities from being globally stable by default. It is
/// consumed only as hash input and is never returned by a report. It is not a
/// secret-key type and does not make low-entropy inputs confidential if the
/// caller publishes or reuses its bytes.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct KfdObservationScopeV1([u8; 32]);

impl KfdObservationScopeV1 {
    /// Creates a nonzero correlation scope.
    pub fn new(bytes: [u8; 32]) -> Result<Self, KfdSemanticObservationErrorV1> {
        if bytes == ZERO_SCOPE {
            return Err(KfdSemanticObservationErrorV1::ZeroObservationScope);
        }
        Ok(Self(bytes))
    }
}

impl fmt::Debug for KfdObservationScopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("KfdObservationScopeV1")
            .field(&"<redacted>")
            .finish()
    }
}

/// Stable, inert pseudonymous identity within a caller-selected scope.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KfdOpaqueIdentityV1([u8; 32]);

impl KfdOpaqueIdentityV1 {
    /// Returns the opaque digest bytes. They grant no KFD authority.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for KfdOpaqueIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KfdOpaqueIdentityV1(")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str(")")
    }
}

/// One queryable debugger/profiler fact class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdSemanticCapabilityV1 {
    /// Correlated device-binding evidence.
    DeviceBinding,
    /// KFD queue lifecycle.
    QueueLifecycle,
    /// Safe queue resource geometry.
    QueueResourceShape,
    /// Delivered queue exception evidence.
    QueueException,
    /// Authenticated dispatch submission.
    DispatchSubmission,
    /// Authenticated dispatch completion.
    DispatchCompletion,
    /// Dispatch timing interval.
    DispatchTiming,
    /// Kernel IR or artifact binding.
    KernelIr,
    /// Workgroup execution facts.
    Workgroups,
    /// Wave execution facts.
    Waves,
    /// Lane execution facts.
    Lanes,
    /// Memory-access facts.
    MemoryAccesses,
    /// Register or value facts.
    Registers,
}

impl KfdSemanticCapabilityV1 {
    /// Every capability in stable schema order.
    pub const ALL: [Self; 13] = [
        Self::DeviceBinding,
        Self::QueueLifecycle,
        Self::QueueResourceShape,
        Self::QueueException,
        Self::DispatchSubmission,
        Self::DispatchCompletion,
        Self::DispatchTiming,
        Self::KernelIr,
        Self::Workgroups,
        Self::Waves,
        Self::Lanes,
        Self::MemoryAccesses,
        Self::Registers,
    ];
}

/// Why a semantic fact is unavailable from this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdUnavailableReasonV1 {
    /// The caller supplied no detached device observation.
    NoDeviceBindingSupplied,
    /// The direct-KFD public boundary exposes no detached fact of this class.
    NotExposedByDirectKfdBoundary,
    /// No observation authenticates both a dispatch and its completion.
    NoAuthenticatedDispatch,
    /// Queue lifecycle evidence does not capture this semantic class.
    NotCapturedByQueueLifecycle,
}

/// Availability of one fact class in a report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdFactAvailabilityV1 {
    /// The report carries this class of fact.
    Observed,
    /// The report does not carry this class of fact.
    Unavailable(KfdUnavailableReasonV1),
}

/// Fixed capability view for a direct-KFD observation report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdSemanticCapabilitiesV1 {
    device_binding: KfdFactAvailabilityV1,
}

impl KfdSemanticCapabilitiesV1 {
    /// Returns availability for a stable capability identifier.
    pub const fn status(self, capability: KfdSemanticCapabilityV1) -> KfdFactAvailabilityV1 {
        match capability {
            KfdSemanticCapabilityV1::DeviceBinding => self.device_binding,
            KfdSemanticCapabilityV1::QueueLifecycle
            | KfdSemanticCapabilityV1::QueueResourceShape => KfdFactAvailabilityV1::Observed,
            KfdSemanticCapabilityV1::QueueException => KfdFactAvailabilityV1::Unavailable(
                KfdUnavailableReasonV1::NotExposedByDirectKfdBoundary,
            ),
            KfdSemanticCapabilityV1::DispatchSubmission
            | KfdSemanticCapabilityV1::DispatchCompletion
            | KfdSemanticCapabilityV1::DispatchTiming => {
                KfdFactAvailabilityV1::Unavailable(KfdUnavailableReasonV1::NoAuthenticatedDispatch)
            }
            KfdSemanticCapabilityV1::KernelIr
            | KfdSemanticCapabilityV1::Workgroups
            | KfdSemanticCapabilityV1::Waves
            | KfdSemanticCapabilityV1::Lanes
            | KfdSemanticCapabilityV1::MemoryAccesses
            | KfdSemanticCapabilityV1::Registers => KfdFactAvailabilityV1::Unavailable(
                KfdUnavailableReasonV1::NotCapturedByQueueLifecycle,
            ),
        }
    }
}

/// Observed direct-KFD queue lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdQueueLifecycleV1 {
    /// The detached observation was issued for a confirmed live queue.
    Live,
    /// KFD queue teardown and resource return were confirmed.
    ///
    /// This does not mean that any dispatch completed or succeeded.
    Destroyed {
        /// Number of distinct queue resources explicitly returned.
        released_resources: u8,
    },
}

/// Fixed provenance for an authority-free direct-KFD report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdSemanticProvenanceV1 {
    source_fact_count: u8,
}

impl KfdSemanticProvenanceV1 {
    /// Claim-boundary manifest for this report schema.
    pub const fn observation_manifest_sha256(self) -> &'static str {
        KFD_SEMANTIC_OBSERVATION_MANIFEST_SHA256_V1
    }

    /// Claim-boundary manifest for the queue facts consumed by this adapter.
    pub const fn queue_session_manifest_sha256(self) -> &'static str {
        GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1
    }

    /// Claim-boundary manifest for an optional device-binding source fact.
    pub const fn device_admission_manifest_sha256(self) -> &'static str {
        DEVICE_ADMISSION_PROFILE_SHA256_V1
    }

    /// Number of detached source facts committed by the evidence identity.
    pub const fn source_fact_count(self) -> u8 {
        self.source_fact_count
    }
}

/// Fixed-size, read-only debugger/profiler observation from direct KFD.
///
/// Exact raw source facts are committed by `evidence_identity`; only safe
/// geometry and scoped opaque identities are retained. This value is inert and
/// grants no descriptor, memory, queue, event, dispatch, or completion access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KfdSemanticObservationReportV1 {
    evidence_identity: KfdOpaqueIdentityV1,
    device_identity: Option<KfdOpaqueIdentityV1>,
    queue_identity: KfdOpaqueIdentityV1,
    lifecycle: KfdQueueLifecycleV1,
    ring_bytes: u32,
    doorbell_slice_bytes: u64,
    cwsr_shadow_pages: u8,
    provenance: KfdSemanticProvenanceV1,
    capabilities: KfdSemanticCapabilitiesV1,
}

impl KfdSemanticObservationReportV1 {
    /// Commits the exact detached raw observations without exporting them.
    pub const fn evidence_identity(&self) -> KfdOpaqueIdentityV1 {
        self.evidence_identity
    }

    /// Identifies the optional device observation within the supplied scope.
    pub const fn device_identity(&self) -> Option<KfdOpaqueIdentityV1> {
        self.device_identity
    }

    /// Identifies the process-local queue within the supplied scope.
    pub const fn queue_identity(&self) -> KfdOpaqueIdentityV1 {
        self.queue_identity
    }

    /// Returns the observed queue lifecycle.
    pub const fn lifecycle(&self) -> KfdQueueLifecycleV1 {
        self.lifecycle
    }

    /// Returns the admitted logical AQL ring byte count.
    pub const fn ring_bytes(&self) -> u32 {
        self.ring_bytes
    }

    /// Returns the complete owned process doorbell-slice byte count.
    pub const fn doorbell_slice_bytes(&self) -> u64 {
        self.doorbell_slice_bytes
    }

    /// Returns the number of owned CPU-visible CWSR shadow pages.
    pub const fn cwsr_shadow_pages(&self) -> u8 {
        self.cwsr_shadow_pages
    }

    /// Returns the fixed source provenance.
    pub const fn provenance(&self) -> KfdSemanticProvenanceV1 {
        self.provenance
    }

    /// Returns the explicit observed/unavailable capability view.
    pub const fn capabilities(&self) -> KfdSemanticCapabilitiesV1 {
        self.capabilities
    }
}

/// Fail-closed rejection at the direct-KFD semantic boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdSemanticObservationErrorV1 {
    /// All-zero scopes are rejected to prevent an accidental global default.
    ZeroObservationScope,
    /// A supposedly detached source observation violated its producer profile.
    InvalidDetachedObservation(&'static str),
    /// A destroyed observation did not identify the report's live queue.
    QueueIdentityMismatch,
    /// A lifecycle transition was attempted from a non-live report.
    UnexpectedLifecycle,
    /// A platform-sized safe field could not fit the stable report schema.
    SizeOverflow,
}

impl fmt::Display for KfdSemanticObservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for KfdSemanticObservationErrorV1 {}

/// Creates a redacted report from real detached live-queue facts.
///
/// `device` is optional because queue observations can be retained separately
/// from device admission evidence. The function performs no I/O or discovery.
pub fn observe_kfd_live_queue_v1(
    scope: &KfdObservationScopeV1,
    device: Option<&DeviceBindingObservation>,
    queue: ComputeAqlQueueObservationV1,
) -> Result<KfdSemanticObservationReportV1, KfdSemanticObservationErrorV1> {
    validate_live_queue(queue)?;
    let doorbell_slice_bytes = u64::try_from(queue.doorbell_slice_bytes())
        .map_err(|_| KfdSemanticObservationErrorV1::SizeOverflow)?;
    let device_identity = device.map(|value| fingerprint_device(scope, value.into()));
    let queue_identity = fingerprint_queue(scope, device_identity, queue.queue_id());
    let evidence_identity = fingerprint_live_evidence(scope, device_identity, queue);
    Ok(KfdSemanticObservationReportV1 {
        evidence_identity,
        device_identity,
        queue_identity,
        lifecycle: KfdQueueLifecycleV1::Live,
        ring_bytes: queue.ring_bytes(),
        doorbell_slice_bytes,
        cwsr_shadow_pages: queue.cwsr_shadow_pages(),
        provenance: KfdSemanticProvenanceV1 {
            source_fact_count: if device.is_some() { 2 } else { 1 },
        },
        capabilities: KfdSemanticCapabilitiesV1 {
            device_binding: if device.is_some() {
                KfdFactAvailabilityV1::Observed
            } else {
                KfdFactAvailabilityV1::Unavailable(KfdUnavailableReasonV1::NoDeviceBindingSupplied)
            },
        },
    })
}

/// Advances a live report with real detached KFD teardown evidence.
///
/// Queue destruction proves the documented teardown and resource return only.
/// Dispatch completion, timing, and kernel success remain unavailable.
pub fn observe_kfd_destroyed_queue_v1(
    scope: &KfdObservationScopeV1,
    live: KfdSemanticObservationReportV1,
    destroyed: ComputeAqlQueueDestroyedV1,
) -> Result<KfdSemanticObservationReportV1, KfdSemanticObservationErrorV1> {
    if live.lifecycle != KfdQueueLifecycleV1::Live {
        return Err(KfdSemanticObservationErrorV1::UnexpectedLifecycle);
    }
    if destroyed.released_resources() != EXPECTED_RELEASED_QUEUE_RESOURCES {
        return Err(KfdSemanticObservationErrorV1::InvalidDetachedObservation(
            "destroyed queue resource count",
        ));
    }
    let queue_identity = fingerprint_queue(scope, live.device_identity, destroyed.queue_id());
    if queue_identity != live.queue_identity {
        return Err(KfdSemanticObservationErrorV1::QueueIdentityMismatch);
    }
    let evidence_identity = fingerprint_destroyed_evidence(scope, &live, destroyed);
    Ok(KfdSemanticObservationReportV1 {
        evidence_identity,
        lifecycle: KfdQueueLifecycleV1::Destroyed {
            released_resources: destroyed.released_resources(),
        },
        provenance: KfdSemanticProvenanceV1 {
            source_fact_count: live.provenance.source_fact_count + 1,
        },
        ..live
    })
}

fn validate_live_queue(
    queue: ComputeAqlQueueObservationV1,
) -> Result<(), KfdSemanticObservationErrorV1> {
    admit_kfd_aql_queue_ring_size(queue.ring_bytes()).map_err(|_| {
        KfdSemanticObservationErrorV1::InvalidDetachedObservation("AQL ring byte count")
    })?;
    if !(GFX942_MIN_ROCR_RING_BYTES_V1..=GFX942_MAX_ADMITTED_RING_BYTES_V1)
        .contains(&queue.ring_bytes())
    {
        return Err(KfdSemanticObservationErrorV1::InvalidDetachedObservation(
            "AQL ring producer profile",
        ));
    }
    if u64::try_from(queue.doorbell_slice_bytes()) != Ok(KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES) {
        return Err(KfdSemanticObservationErrorV1::InvalidDetachedObservation(
            "doorbell slice byte count",
        ));
    }
    if !queue
        .doorbell_byte_offset()
        .is_multiple_of(KFD_GFX942_DOORBELL_BYTES)
        || queue
            .doorbell_byte_offset()
            .checked_add(KFD_GFX942_DOORBELL_BYTES)
            .is_none_or(|end| end > KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES)
    {
        return Err(KfdSemanticObservationErrorV1::InvalidDetachedObservation(
            "doorbell byte offset",
        ));
    }
    if queue.event_id() == 0 || queue.event_id() >= KFD_INTERNAL_SIGNAL_PAGE_SLOT_COUNT {
        return Err(KfdSemanticObservationErrorV1::InvalidDetachedObservation(
            "queue exception event id",
        ));
    }
    if queue.cwsr_shadow_pages() != EXPECTED_CWSR_SHADOW_PAGES {
        return Err(KfdSemanticObservationErrorV1::InvalidDetachedObservation(
            "CWSR shadow page count",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct DeviceFingerprintInputV1 {
    topology_node_id: u32,
    kfd_gpu_id: u32,
    unique_id: u64,
    pci_domain: u16,
    pci_bus: u8,
    pci_device: u8,
    pci_function: u8,
    render_minor: u16,
    render_file_system_device: u64,
    render_inode: u64,
    render_character_device: u64,
    render_major: u32,
    render_descriptor_minor: u32,
    drm_major: i32,
    drm_minor: i32,
    drm_patch: i32,
    acceleration_working: u32,
    drm_device_id: u32,
    drm_chip_rev: u32,
    drm_external_rev: u32,
    drm_pci_rev: u32,
    drm_family: u32,
    vram_lost_counter: u32,
    aperture_gpu_id: u32,
    lds_base: u64,
    lds_limit: u64,
    scratch_base: u64,
    scratch_limit: u64,
    gpuvm_base: u64,
    gpuvm_limit: u64,
}

impl From<&DeviceBindingObservation> for DeviceFingerprintInputV1 {
    fn from(value: &DeviceBindingObservation) -> Self {
        let pci = value.pci();
        let render = value.render_descriptor();
        let drm = value.drm();
        let drm_version = drm.driver_version();
        let drm_device = drm.device();
        let aperture = value.aperture();
        Self {
            topology_node_id: value.topology_node_id(),
            kfd_gpu_id: value.kfd_gpu_id(),
            unique_id: value.unique_id(),
            pci_domain: pci.domain(),
            pci_bus: pci.bus(),
            pci_device: pci.device(),
            pci_function: pci.function(),
            render_minor: value.render_minor(),
            render_file_system_device: render.file_system_device(),
            render_inode: render.inode(),
            render_character_device: render.character_device(),
            render_major: render.major(),
            render_descriptor_minor: render.minor(),
            drm_major: drm_version.major,
            drm_minor: drm_version.minor,
            drm_patch: drm_version.patch,
            acceleration_working: drm.acceleration_working(),
            drm_device_id: drm_device.device_id,
            drm_chip_rev: drm_device.chip_rev,
            drm_external_rev: drm_device.external_rev,
            drm_pci_rev: drm_device.pci_rev,
            drm_family: drm_device.family,
            vram_lost_counter: drm.vram_lost_counter(),
            aperture_gpu_id: aperture.gpu_id(),
            lds_base: aperture.lds().base(),
            lds_limit: aperture.lds().limit(),
            scratch_base: aperture.scratch().base(),
            scratch_limit: aperture.scratch().limit(),
            gpuvm_base: aperture.gpuvm().base(),
            gpuvm_limit: aperture.gpuvm().limit(),
        }
    }
}

fn new_scoped_hasher(domain: &[u8], scope: &KfdObservationScopeV1) -> Sha256 {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, domain);
    hash_bytes(&mut hasher, &scope.0);
    hasher
}

fn fingerprint_device(
    scope: &KfdObservationScopeV1,
    value: DeviceFingerprintInputV1,
) -> KfdOpaqueIdentityV1 {
    let mut hasher = new_scoped_hasher(b"fe2o3.kfd.semantic.device.v1", scope);
    hash_u32(&mut hasher, value.topology_node_id);
    hash_u32(&mut hasher, value.kfd_gpu_id);
    hash_u64(&mut hasher, value.unique_id);
    hash_u16(&mut hasher, value.pci_domain);
    hasher.update([value.pci_bus, value.pci_device, value.pci_function]);
    hash_u16(&mut hasher, value.render_minor);
    hash_u64(&mut hasher, value.render_file_system_device);
    hash_u64(&mut hasher, value.render_inode);
    hash_u64(&mut hasher, value.render_character_device);
    hash_u32(&mut hasher, value.render_major);
    hash_u32(&mut hasher, value.render_descriptor_minor);
    hasher.update(value.drm_major.to_le_bytes());
    hasher.update(value.drm_minor.to_le_bytes());
    hasher.update(value.drm_patch.to_le_bytes());
    hash_u32(&mut hasher, value.acceleration_working);
    hash_u32(&mut hasher, value.drm_device_id);
    hash_u32(&mut hasher, value.drm_chip_rev);
    hash_u32(&mut hasher, value.drm_external_rev);
    hash_u32(&mut hasher, value.drm_pci_rev);
    hash_u32(&mut hasher, value.drm_family);
    hash_u32(&mut hasher, value.vram_lost_counter);
    hash_u32(&mut hasher, value.aperture_gpu_id);
    hash_u64(&mut hasher, value.lds_base);
    hash_u64(&mut hasher, value.lds_limit);
    hash_u64(&mut hasher, value.scratch_base);
    hash_u64(&mut hasher, value.scratch_limit);
    hash_u64(&mut hasher, value.gpuvm_base);
    hash_u64(&mut hasher, value.gpuvm_limit);
    finish_identity(hasher)
}

fn fingerprint_queue(
    scope: &KfdObservationScopeV1,
    device_identity: Option<KfdOpaqueIdentityV1>,
    queue_id: u32,
) -> KfdOpaqueIdentityV1 {
    let mut hasher = new_scoped_hasher(b"fe2o3.kfd.semantic.queue.v1", scope);
    hash_optional_identity(&mut hasher, device_identity);
    hash_u32(&mut hasher, queue_id);
    finish_identity(hasher)
}

fn fingerprint_live_evidence(
    scope: &KfdObservationScopeV1,
    device_identity: Option<KfdOpaqueIdentityV1>,
    queue: ComputeAqlQueueObservationV1,
) -> KfdOpaqueIdentityV1 {
    let mut hasher = new_scoped_hasher(b"fe2o3.kfd.semantic.live-evidence.v1", scope);
    hash_bytes(&mut hasher, KFD_SEMANTIC_OBSERVATION_MANIFEST_V1.as_bytes());
    hash_bytes(
        &mut hasher,
        GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1.as_bytes(),
    );
    hash_bytes(&mut hasher, DEVICE_ADMISSION_PROFILE_SHA256_V1.as_bytes());
    hash_optional_identity(&mut hasher, device_identity);
    hash_u32(&mut hasher, queue.queue_id());
    hash_u32(&mut hasher, queue.ring_bytes());
    hash_u64(&mut hasher, queue.doorbell_slice_bytes() as u64);
    hash_u64(&mut hasher, queue.doorbell_byte_offset());
    hash_u32(&mut hasher, queue.event_id());
    hasher.update([queue.cwsr_shadow_pages()]);
    finish_identity(hasher)
}

fn fingerprint_destroyed_evidence(
    scope: &KfdObservationScopeV1,
    live: &KfdSemanticObservationReportV1,
    destroyed: ComputeAqlQueueDestroyedV1,
) -> KfdOpaqueIdentityV1 {
    let mut hasher = new_scoped_hasher(b"fe2o3.kfd.semantic.destroyed-evidence.v1", scope);
    hash_bytes(&mut hasher, &live.evidence_identity.0);
    hash_u32(&mut hasher, destroyed.queue_id());
    hasher.update([destroyed.released_resources()]);
    finish_identity(hasher)
}

fn hash_optional_identity(hasher: &mut Sha256, value: Option<KfdOpaqueIdentityV1>) {
    match value {
        Some(identity) => {
            hasher.update([1]);
            hash_bytes(hasher, &identity.0);
        }
        None => hasher.update([0]),
    }
}

fn hash_bytes(hasher: &mut Sha256, value: &[u8]) {
    hash_u64(hasher, value.len() as u64);
    hasher.update(value);
}

fn hash_u16(hasher: &mut Sha256, value: u16) {
    hasher.update(value.to_le_bytes());
}

fn hash_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_le_bytes());
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_le_bytes());
}

fn finish_identity(hasher: Sha256) -> KfdOpaqueIdentityV1 {
    KfdOpaqueIdentityV1(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use super::*;

    const RAW_QUEUE_ID: u32 = 0x4d2;
    const RAW_EVENT_ID: u32 = 0xad;
    const RAW_DOORBELL_OFFSET: u64 = 0x1ab8;

    fn scope(byte: u8) -> KfdObservationScopeV1 {
        KfdObservationScopeV1::new([byte; 32]).expect("nonzero scope")
    }

    fn queue_with(
        queue_id: u32,
        ring_bytes: u32,
        slice_bytes: usize,
        offset: u64,
        event_id: u32,
        shadow_pages: u8,
    ) -> ComputeAqlQueueObservationV1 {
        ComputeAqlQueueObservationV1::from_parts_for_semantic_observation_tests(
            queue_id,
            ring_bytes,
            slice_bytes,
            offset,
            event_id,
            shadow_pages,
        )
    }

    fn queue() -> ComputeAqlQueueObservationV1 {
        queue_with(
            RAW_QUEUE_ID,
            4096,
            KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES as usize,
            RAW_DOORBELL_OFFSET,
            RAW_EVENT_ID,
            EXPECTED_CWSR_SHADOW_PAGES,
        )
    }

    fn report(scope: &KfdObservationScopeV1) -> KfdSemanticObservationReportV1 {
        observe_kfd_live_queue_v1(scope, None, queue()).expect("valid detached queue")
    }

    #[test]
    fn manifest_digest_matches() {
        assert!(KFD_SEMANTIC_OBSERVATION_MANIFEST_V1.contains(&format!(
            "source.queue_session_sha256={GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1}\n"
        )));
        let digest = Sha256::digest(KFD_SEMANTIC_OBSERVATION_MANIFEST_V1.as_bytes());
        assert_eq!(
            format!("{:?}", KfdOpaqueIdentityV1(digest.into())),
            format!(
                "KfdOpaqueIdentityV1({})",
                KFD_SEMANTIC_OBSERVATION_MANIFEST_SHA256_V1
            )
        );
    }

    #[test]
    fn rejects_zero_scope_and_redacts_nonzero_scope_debug() {
        assert_eq!(
            KfdObservationScopeV1::new([0; 32]),
            Err(KfdSemanticObservationErrorV1::ZeroObservationScope)
        );
        assert_eq!(
            format!("{:?}", scope(0xa7)),
            "KfdObservationScopeV1(\"<redacted>\")"
        );
    }

    #[test]
    fn report_is_deterministic_scoped_fixed_size_and_authority_free() {
        let first_scope = scope(0x11);
        let first = report(&first_scope);
        let repeat = report(&first_scope);
        let other_scope = report(&scope(0x12));
        assert_eq!(first, repeat);
        assert_ne!(first.evidence_identity(), other_scope.evidence_identity());
        assert_ne!(first.queue_identity(), other_scope.queue_identity());
        assert!(size_of::<KfdSemanticObservationReportV1>() <= 128);
        assert_eq!(first.ring_bytes(), 4096);
        assert_eq!(first.doorbell_slice_bytes(), 8192);
        assert_eq!(first.cwsr_shadow_pages(), 8);
        assert_eq!(first.provenance().source_fact_count(), 1);
    }

    #[test]
    fn every_capability_has_an_explicit_stable_status() {
        let report = report(&scope(0x21));
        let capabilities = report.capabilities();
        assert_eq!(KfdSemanticCapabilityV1::ALL.len(), 13);
        assert_eq!(
            capabilities.status(KfdSemanticCapabilityV1::DeviceBinding),
            KfdFactAvailabilityV1::Unavailable(KfdUnavailableReasonV1::NoDeviceBindingSupplied)
        );
        assert_eq!(
            capabilities.status(KfdSemanticCapabilityV1::QueueLifecycle),
            KfdFactAvailabilityV1::Observed
        );
        assert_eq!(
            capabilities.status(KfdSemanticCapabilityV1::DispatchCompletion),
            KfdFactAvailabilityV1::Unavailable(KfdUnavailableReasonV1::NoAuthenticatedDispatch)
        );
        assert_eq!(
            capabilities.status(KfdSemanticCapabilityV1::Lanes),
            KfdFactAvailabilityV1::Unavailable(KfdUnavailableReasonV1::NotCapturedByQueueLifecycle)
        );
    }

    #[test]
    fn evidence_commits_redacted_raw_fields() {
        let scope = scope(0x31);
        let baseline = report(&scope);
        let changed_event = observe_kfd_live_queue_v1(
            &scope,
            None,
            queue_with(
                RAW_QUEUE_ID,
                4096,
                8192,
                RAW_DOORBELL_OFFSET,
                RAW_EVENT_ID + 1,
                8,
            ),
        )
        .expect("changed event remains valid");
        let changed_offset = observe_kfd_live_queue_v1(
            &scope,
            None,
            queue_with(RAW_QUEUE_ID, 4096, 8192, 0x1ac0, RAW_EVENT_ID, 8),
        )
        .expect("changed offset remains valid");
        assert_eq!(baseline.queue_identity(), changed_event.queue_identity());
        assert_ne!(
            baseline.evidence_identity(),
            changed_event.evidence_identity()
        );
        assert_ne!(
            baseline.evidence_identity(),
            changed_offset.evidence_identity()
        );

        let rendered = format!("{baseline:?}");
        for raw_field in [
            "queue_id: ",
            "event_id: ",
            "doorbell_byte_offset: ",
            "device_address: ",
            "file_descriptor: ",
            "handle: ",
        ] {
            assert!(
                !rendered.contains(raw_field),
                "debug output exported raw field {raw_field}"
            );
        }
        assert!(!rendered.contains("0x4d2"));
        assert!(!rendered.contains("0x1ab8"));
    }

    #[test]
    fn hostile_live_queue_fields_fail_closed() {
        let scope = scope(0x41);
        let invalid = [
            queue_with(RAW_QUEUE_ID, 2048, 8192, 0, 1, 8),
            queue_with(RAW_QUEUE_ID, 4097, 8192, 0, 1, 8),
            queue_with(RAW_QUEUE_ID, 4096, 4096, 0, 1, 8),
            queue_with(RAW_QUEUE_ID, 4096, 8192, 1, 1, 8),
            queue_with(RAW_QUEUE_ID, 4096, 8192, 8192, 1, 8),
            queue_with(RAW_QUEUE_ID, 4096, 8192, u64::MAX, 1, 8),
            queue_with(RAW_QUEUE_ID, 4096, 8192, 0, 0, 8),
            queue_with(RAW_QUEUE_ID, 4096, 8192, 0, 256, 8),
            queue_with(RAW_QUEUE_ID, 4096, 8192, 0, 1, 7),
        ];
        for observation in invalid {
            assert!(matches!(
                observe_kfd_live_queue_v1(&scope, None, observation),
                Err(KfdSemanticObservationErrorV1::InvalidDetachedObservation(_))
            ));
        }
        assert!(
            observe_kfd_live_queue_v1(&scope, None, queue_with(0, 4096, 8192, 0, 1, 8),).is_ok()
        );
    }

    #[test]
    fn destroy_links_queue_but_never_claims_dispatch_completion() {
        let scope = scope(0x51);
        let live = report(&scope);
        let live_evidence = live.evidence_identity();
        let destroyed = observe_kfd_destroyed_queue_v1(
            &scope,
            live,
            ComputeAqlQueueDestroyedV1::from_parts_for_semantic_observation_tests(
                RAW_QUEUE_ID,
                EXPECTED_RELEASED_QUEUE_RESOURCES,
            ),
        )
        .expect("matching destroy evidence");
        assert_eq!(
            destroyed.lifecycle(),
            KfdQueueLifecycleV1::Destroyed {
                released_resources: 4
            }
        );
        assert_ne!(live_evidence, destroyed.evidence_identity());
        assert_eq!(destroyed.provenance().source_fact_count(), 2);
        assert_eq!(
            destroyed
                .capabilities()
                .status(KfdSemanticCapabilityV1::DispatchCompletion),
            KfdFactAvailabilityV1::Unavailable(KfdUnavailableReasonV1::NoAuthenticatedDispatch)
        );
    }

    #[test]
    fn hostile_destroy_evidence_and_replay_fail_closed() {
        let observation_scope = scope(0x61);
        let mismatch = ComputeAqlQueueDestroyedV1::from_parts_for_semantic_observation_tests(
            RAW_QUEUE_ID + 1,
            4,
        );
        assert_eq!(
            observe_kfd_destroyed_queue_v1(
                &observation_scope,
                report(&observation_scope),
                mismatch,
            ),
            Err(KfdSemanticObservationErrorV1::QueueIdentityMismatch)
        );
        let malformed =
            ComputeAqlQueueDestroyedV1::from_parts_for_semantic_observation_tests(RAW_QUEUE_ID, 3);
        assert!(matches!(
            observe_kfd_destroyed_queue_v1(
                &observation_scope,
                report(&observation_scope),
                malformed,
            ),
            Err(KfdSemanticObservationErrorV1::InvalidDetachedObservation(_))
        ));
        let destroyed = observe_kfd_destroyed_queue_v1(
            &observation_scope,
            report(&observation_scope),
            ComputeAqlQueueDestroyedV1::from_parts_for_semantic_observation_tests(RAW_QUEUE_ID, 4),
        )
        .expect("first transition");
        assert_eq!(
            observe_kfd_destroyed_queue_v1(
                &observation_scope,
                destroyed,
                ComputeAqlQueueDestroyedV1::from_parts_for_semantic_observation_tests(
                    RAW_QUEUE_ID,
                    4,
                ),
            ),
            Err(KfdSemanticObservationErrorV1::UnexpectedLifecycle)
        );
        assert_eq!(
            observe_kfd_destroyed_queue_v1(
                &scope(0x62),
                report(&observation_scope),
                ComputeAqlQueueDestroyedV1::from_parts_for_semantic_observation_tests(
                    RAW_QUEUE_ID,
                    4,
                ),
            ),
            Err(KfdSemanticObservationErrorV1::QueueIdentityMismatch)
        );
    }

    fn device_input() -> DeviceFingerprintInputV1 {
        DeviceFingerprintInputV1 {
            topology_node_id: 7,
            kfd_gpu_id: 0xfeed_beef,
            unique_id: 0x0123_4567_89ab_cdef,
            pci_domain: 0x4321,
            pci_bus: 0x87,
            pci_device: 0x1a,
            pci_function: 3,
            render_minor: 0x1234,
            render_file_system_device: 0x1111_2222_3333_4444,
            render_inode: 0x5555_6666_7777_8888,
            render_character_device: 0x9999_aaaa_bbbb_cccc,
            render_major: 226,
            render_descriptor_minor: 0x1234,
            drm_major: 3,
            drm_minor: 64,
            drm_patch: 0,
            acceleration_working: 1,
            drm_device_id: 0x74a1,
            drm_chip_rev: 1,
            drm_external_rev: 71,
            drm_pci_rev: 0,
            drm_family: 141,
            vram_lost_counter: 0x7654_3210,
            aperture_gpu_id: 0xfeed_beef,
            lds_base: 0x1111_0000_0000,
            lds_limit: 0x1111_ffff_ffff,
            scratch_base: 0x2222_0000_0000,
            scratch_limit: 0x2222_ffff_ffff,
            gpuvm_base: 0x3333_0000_0000,
            gpuvm_limit: 0x3333_ffff_ffff,
        }
    }

    #[test]
    fn device_fingerprint_is_deterministic_scoped_and_commits_raw_facts() {
        let first_scope = scope(0x71);
        let input = device_input();
        let first = fingerprint_device(&first_scope, input);
        assert_eq!(first, fingerprint_device(&first_scope, input));
        assert_ne!(first, fingerprint_device(&scope(0x72), input));
        let mut changed_address = input;
        changed_address.gpuvm_base += 4096;
        assert_ne!(first, fingerprint_device(&first_scope, changed_address));
        let mut changed_gpu = input;
        changed_gpu.kfd_gpu_id += 1;
        assert_ne!(first, fingerprint_device(&first_scope, changed_gpu));
        let rendered = format!("{first:?}");
        for raw in [
            input.kfd_gpu_id.to_string(),
            input.unique_id.to_string(),
            input.gpuvm_base.to_string(),
            format!("{:x}", input.kfd_gpu_id),
            format!("{:x}", input.unique_id),
            format!("{:x}", input.gpuvm_base),
        ] {
            assert!(!rendered.contains(&raw), "identity rendering leaked {raw}");
        }
    }
}
