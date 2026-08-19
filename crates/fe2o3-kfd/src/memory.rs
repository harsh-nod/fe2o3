//! Owned, fail-closed host-visible KFD memory transactions.

use core::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kfd_uapi::{
    KFD_ALLOC_MEMORY_FLAGS_HOST_VISIBLE_COHERENT, KFD_MEMORY_LIFECYCLE_SCHEMA_MANIFEST_SHA256,
    KfdAllocMemoryFlags, KfdIoctlAllocMemoryOfGpuArgs,
};

use crate::{CheckedGfx942XnackMinusDevice, DeviceBindingError, InclusiveAperture};
use fe2o3_runtime_model::{
    AllocationGenerationV1, AllocationIdV1, GpuVaRangeV1, MappingIdV1, MemoryAccessV1,
    MemoryAllocationKeyV1, MemoryAllocationSpecV1, MemoryCoherenceV1, MemoryKindV1,
    MemoryLifecycleStateV1, MemoryMappingKeyV1, MemoryTransitionV1, ModelDeviceAdmissionV1,
    PartialOperationStatusV1, PartialProgressObservationV1, UntrustedAllocationHandleObservationV1,
    UntrustedVmHandleObservationV1, VaReservationIdV1, VaReservationKeyV1, VmIdV1,
};

pub const HOST_VISIBLE_MEMORY_PAGE_BYTES_V1: u64 = 4_096;

/// Canonical contract for the first executable memory adapter slice.
///
/// The source hashes are observations of the admitted DKMS source tree, not a
/// proof that the running kernel was built from those files. The adapter is
/// consequently Contracted at the syscall boundary and makes no refinement or
/// kernel-correctness claim.
pub const HOST_VISIBLE_MEMORY_PROFILE_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-single-device-host-visible-memory-r2-v1\n",
    "device_profile_sha256=e12ea33b259666e7928612403109640b03b0d637b893a2c15b87d17a4211c8de\n",
    "kfd_memory_schema_sha256=e2d6987b7c8e61a405b2f775d5d004f458a096241459e4cfdf90bd4497f4d58a\n",
    "platform=linux-x86_64,kernel:6.8.0-124-generic,amdgpu:6.16.13,page:4096\n",
    "module_zst_sha256=e5a327a8f46459e07ee3f59cc991d16feee17103e199d39149823879b7fcff0b\n",
    "module_ko_sha256=61317154cee502ea97a74818879dff4b20abf8f074a2f4d19a94288e25d4ac3a\n",
    "module_srcversion=A6F143BEC60C0AFC3263226\n",
    "dkms_config_sha256=8cf6cf2335f4e3c481eb77e0797aa1fb294b80b6cbc136fa675716c51e014f2c\n",
    "source.kfd_ioctl.h=b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d\n",
    "source.kfd_chardev.c=f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba\n",
    "source.kfd_process.c=d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409\n",
    "source.kfd_priv.h=f991330031c14725b2be0636ec1896ab530dc3d07d530ebd4f47efff97a82a99\n",
    "source.kfd_flat_memory.c=1ba1ff708e4ecc498043f9e3ab7373904985d7f12c78837292ade51f44aabec7\n",
    "source.amdgpu_amdkfd_gpuvm.c=c7cca2ee47a08c99bb73906662d82dd7d0b5738468fbef54848e5e6dd62ba50d\n",
    "source.amdgpu_gem.c=a577f4da607cc580ccefea5d5a25d15a20f5912e9d047c3c3095e41f87bc1edf\n",
    "source.amdgpu_ttm.c=47aadc9e352ec58fbac9965fe756e40c3fed6959565c5e182b7795c580f8ce68\n",
    "source.ttm_bo_vm.c=e837d10429d5f4baa67eb5c7369a5a61486e8d12340478a7af0d8aba0bb8e93a\n",
    "source.kcl_drm_gem.c=db5268c25558857f49f253c2a9671ec5f95149f544515c7d7e38d24001550ddc\n",
    "source.gmc_v9_0.c=2b76f87a7189877e5d03320abe6368c7d0225f98fb56857fa3d13c70e7a1d5cf\n",
    "source.amdgpu_vm.h=64b8c0cdb32c28714996de8a721e4b0cad55f9ddaa83e24ba2083098d7c48453\n",
    "source.amdgpu_amdkfd.c=ce2d3a70928a267431313e5f0ad76ee2ebc5c8a724308e2cd89a7a67a1959c07\n",
    "source_linkage=contracted,source-hashes-do-not-prove-loaded-binary\n",
    "ordinary_backing=checked-page-align-4096,aql-executable-kernarg-vram-peer=unsupported\n",
    "allocation=host-visible-coherent-gtt-writable,checked-fixed-gpu-va,whole-bo-map\n",
    "mapping=one-immutable-gpu-id,cumulative-n-success,no-retry\n",
    "cpu_vma=release-gpu-va-reservation,kernel-selected-map-shared,madv-dontfork-before-borrow\n",
    "cleanup=munmap-before-free,free-exactly-once,no-drop-retry,owned-fds-still-close\n",
    "currentness=contracted-composite,post-mutation-failure-quarantines\n",
    "authority=retained-kfd-and-render-fds,no-raw-handle-va-vma-or-fd-export\n",
    "proof=model-only-success-journal-and-hostile-tests,no-concrete-or-verus-refinement\n",
);

/// SHA-256 of [`HOST_VISIBLE_MEMORY_PROFILE_MANIFEST_V1`].
pub const HOST_VISIBLE_MEMORY_PROFILE_SHA256_V1: &str =
    "02da8014e7f51d9519613736b9e677ec9533b4c098b421452f5300e27ffdb7f4";

/// Typed digest bytes of [`HOST_VISIBLE_MEMORY_PROFILE_MANIFEST_V1`].
pub const HOST_VISIBLE_MEMORY_PROFILE_SHA256_BYTES_V1: [u8; 32] = [
    0x02, 0xda, 0x80, 0x14, 0xe7, 0xf5, 0x1d, 0x95, 0x19, 0x61, 0x37, 0x36, 0xb9, 0xe6, 0x77, 0xec,
    0x95, 0x33, 0xb4, 0xc0, 0x98, 0xb4, 0x21, 0x45, 0x2f, 0x53, 0x00, 0xe2, 0x7f, 0xfd, 0xb7, 0xf4,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub struct HostVisibleAllocationLayout {
    requested_bytes: usize,
    backing_bytes: usize,
}

impl HostVisibleAllocationLayout {
    pub const fn requested_bytes(self) -> usize {
        self.requested_bytes
    }

    pub const fn backing_bytes(self) -> usize {
        self.backing_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub struct MemoryModelJournalSummary {
    vm_records: usize,
    reservation_records: usize,
    allocation_records: usize,
    mapping_records: usize,
}

impl MemoryModelJournalSummary {
    pub const fn vm_records(self) -> usize {
        self.vm_records
    }

    pub const fn reservation_records(self) -> usize {
        self.reservation_records
    }

    pub const fn allocation_records(self) -> usize {
        self.allocation_records
    }

    pub const fn mapping_records(self) -> usize {
        self.mapping_records
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostVisibleMemoryPhase {
    Ready,
    CpuAccessible,
    GpuAccessible,
    CpuAccessibleAfterUnmap,
    Released,
    Quarantined,
}

#[derive(Debug)]
pub enum MemorySessionError {
    ProcessVmAlreadyConsumed,
    ProcessVmStatePoisoned,
    ProcessChanged,
    InvalidRequestedSize,
    SizeOverflow,
    UnsupportedPageSize(usize),
    AddressOutsideAperture,
    AddressNotPageAligned,
    IsolationRequired,
    DontForkMappingInherited,
    ChildProbe(&'static str),
    InvalidPhase {
        operation: &'static str,
        phase: HostVisibleMemoryPhase,
    },
    KernelResultMalformed(&'static str),
    Syscall {
        operation: &'static str,
        source: rustix::io::Errno,
    },
    Device(DeviceBindingError),
    Model(&'static str),
    Injected(&'static str),
}

impl fmt::Display for MemorySessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcessVmAlreadyConsumed => formatter.write_str(
                "this process has already attempted its one admitted KFD VM acquisition",
            ),
            Self::ProcessVmStatePoisoned => {
                formatter.write_str("the process KFD VM state is permanently quarantined")
            }
            Self::ProcessChanged => formatter.write_str("the memory session process changed"),
            Self::InvalidRequestedSize => formatter.write_str("allocation size must be nonzero"),
            Self::SizeOverflow => formatter.write_str("allocation footprint overflowed"),
            Self::UnsupportedPageSize(size) => {
                write!(
                    formatter,
                    "host page size {size} is outside the 4096-byte profile"
                )
            }
            Self::AddressOutsideAperture => {
                formatter.write_str("reserved address is outside the admitted GPUVM aperture")
            }
            Self::AddressNotPageAligned => {
                formatter.write_str("reserved address is not 4096-byte aligned")
            }
            Self::IsolationRequired => formatter
                .write_str("the DONTFORK child probe requires an isolated single-threaded process"),
            Self::DontForkMappingInherited => {
                formatter.write_str("the AMDGPU CPU mapping was inherited across fork")
            }
            Self::ChildProbe(detail) => {
                write!(formatter, "the DONTFORK child probe failed: {detail}")
            }
            Self::InvalidPhase { operation, phase } => {
                write!(formatter, "{operation} is invalid in phase {phase:?}")
            }
            Self::KernelResultMalformed(field) => {
                write!(
                    formatter,
                    "kernel result violated the admitted {field} contract"
                )
            }
            Self::Syscall { operation, source } => {
                write!(formatter, "{operation} failed: {source}")
            }
            Self::Device(source) => write!(formatter, "device currentness failed: {source}"),
            Self::Model(detail) => write!(formatter, "model projection failed: {detail}"),
            Self::Injected(operation) => write!(formatter, "injected {operation} failure"),
        }
    }
}

impl std::error::Error for MemorySessionError {}

impl From<DeviceBindingError> for MemorySessionError {
    fn from(source: DeviceBindingError) -> Self {
        Self::Device(source)
    }
}

#[derive(Debug)]
pub(super) struct KernelOutcome<T> {
    pub(super) value: T,
    pub(super) result: Result<(), MemorySessionError>,
}

pub(super) trait MemoryBackend {
    type Reservation;
    type Mapping;

    fn opener_pid(&self) -> u32;
    fn gpu_id(&self) -> u32;
    fn gpuvm_aperture(&self) -> InclusiveAperture;
    fn page_size(&self) -> usize;
    fn check_currentness(&mut self) -> Result<(), MemorySessionError>;
    fn acquire_vm(&mut self) -> Result<(), MemorySessionError>;
    fn reserve_va(&mut self, bytes: usize) -> Result<Self::Reservation, MemorySessionError>;
    fn reservation_address(reservation: &Self::Reservation) -> u64;
    fn alloc(&mut self, va: u64, bytes: u64) -> KernelOutcome<KfdIoctlAllocMemoryOfGpuArgs>;
    fn map_cpu(
        &mut self,
        reservation: &mut Self::Reservation,
        mmap_offset: u64,
        bytes: usize,
    ) -> Result<Self::Mapping, MemorySessionError>;
    fn advise_dontfork(&mut self, mapping: &mut Self::Mapping) -> Result<(), MemorySessionError>;
    fn map_gpu(&mut self, handle: u64, old_success: u32) -> KernelOutcome<u32>;
    fn unmap_gpu(&mut self, handle: u64, old_success: u32) -> KernelOutcome<u32>;
    fn with_bytes<R>(
        mapping: &Self::Mapping,
        requested_bytes: usize,
        f: impl FnOnce(&[u8]) -> R,
    ) -> R;
    fn with_bytes_mut<R>(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> R;
    fn unmap_cpu(&mut self, mapping: &mut Self::Mapping) -> Result<(), MemorySessionError>;
    fn free(&mut self, handle: u64) -> Result<(), MemorySessionError>;
}

struct MemoryEngine<B: MemoryBackend> {
    backend: B,
    phase: HostVisibleMemoryPhase,
    layout: Option<HostVisibleAllocationLayout>,
    gpu_va: Option<u64>,
    reservation: Option<B::Reservation>,
    mapping: Option<B::Mapping>,
    handle: Option<u64>,
    free_attempted: bool,
}

impl<B: MemoryBackend> MemoryEngine<B> {
    fn acquire(mut backend: B) -> Result<Self, MemorySessionError> {
        if backend.opener_pid() != std::process::id() {
            return Err(MemorySessionError::ProcessChanged);
        }
        if backend.page_size() != HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize {
            return Err(MemorySessionError::UnsupportedPageSize(backend.page_size()));
        }
        backend.check_currentness()?;
        backend.acquire_vm()?;
        backend.check_currentness()?;
        Ok(Self {
            backend,
            phase: HostVisibleMemoryPhase::Ready,
            layout: None,
            gpu_va: None,
            reservation: None,
            mapping: None,
            handle: None,
            free_attempted: false,
        })
    }

    fn phase(&self) -> HostVisibleMemoryPhase {
        self.phase
    }

    fn layout(&self) -> Option<HostVisibleAllocationLayout> {
        self.layout
    }

    fn quarantine<T>(&mut self, error: MemorySessionError) -> Result<T, MemorySessionError> {
        self.phase = HostVisibleMemoryPhase::Quarantined;
        Err(error)
    }

    fn require_phase(
        &self,
        operation: &'static str,
        allowed: &[HostVisibleMemoryPhase],
    ) -> Result<(), MemorySessionError> {
        if allowed.contains(&self.phase) {
            Ok(())
        } else {
            Err(MemorySessionError::InvalidPhase {
                operation,
                phase: self.phase,
            })
        }
    }

    fn allocate(
        &mut self,
        requested_bytes: usize,
    ) -> Result<HostVisibleAllocationLayout, MemorySessionError> {
        self.require_phase("allocate", &[HostVisibleMemoryPhase::Ready])?;
        let backing_bytes = ordinary_footprint(requested_bytes)?;
        if self.backend.opener_pid() != std::process::id() {
            return self.quarantine(MemorySessionError::ProcessChanged);
        }
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        let reservation = match self.backend.reserve_va(backing_bytes) {
            Ok(value) => value,
            Err(error) => return self.quarantine(error),
        };
        let va = B::reservation_address(&reservation);
        if let Err(error) =
            validate_reserved_range(va, backing_bytes as u64, self.backend.gpuvm_aperture())
        {
            self.reservation = Some(reservation);
            return self.quarantine(error);
        }
        self.reservation = Some(reservation);
        let outcome = self.backend.alloc(va, backing_bytes as u64);
        let args = outcome.value;
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if args.va_addr != va
            || args.size != backing_bytes as u64
            || args.gpu_id != self.backend.gpu_id()
            || args.flags != KFD_ALLOC_MEMORY_FLAGS_HOST_VISIBLE_COHERENT
            || args.handle == 0
            || args.mmap_offset == 0
            || !args
                .mmap_offset
                .is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)
        {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "ALLOC_MEMORY_OF_GPU output",
            ));
        }
        self.gpu_va = Some(va);
        self.handle = Some(args.handle);
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        let Some(reservation) = self.reservation.as_mut() else {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "reservation ownership",
            ));
        };
        let mut mapping = match self
            .backend
            .map_cpu(reservation, args.mmap_offset, backing_bytes)
        {
            Ok(value) => value,
            Err(error) => return self.quarantine(error),
        };
        if let Err(error) = self.backend.advise_dontfork(&mut mapping) {
            self.mapping = Some(mapping);
            return self.quarantine(error);
        }
        self.mapping = Some(mapping);
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        let layout = HostVisibleAllocationLayout {
            requested_bytes,
            backing_bytes,
        };
        self.layout = Some(layout);
        self.phase = HostVisibleMemoryPhase::CpuAccessible;
        Ok(layout)
    }

    fn with_bytes<R>(&self, f: impl FnOnce(&[u8]) -> R) -> Result<R, MemorySessionError> {
        if self.backend.opener_pid() != std::process::id() {
            return Err(MemorySessionError::ProcessChanged);
        }
        self.require_phase(
            "borrow CPU bytes",
            &[
                HostVisibleMemoryPhase::CpuAccessible,
                HostVisibleMemoryPhase::CpuAccessibleAfterUnmap,
            ],
        )?;
        let mapping = self
            .mapping
            .as_ref()
            .ok_or(MemorySessionError::KernelResultMalformed(
                "CPU mapping ownership",
            ))?;
        let requested = self
            .layout
            .ok_or(MemorySessionError::KernelResultMalformed(
                "allocation layout",
            ))?
            .requested_bytes;
        Ok(B::with_bytes(mapping, requested, f))
    }

    fn with_bytes_mut<R>(
        &mut self,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        if self.backend.opener_pid() != std::process::id() {
            return self.quarantine(MemorySessionError::ProcessChanged);
        }
        self.require_phase(
            "borrow mutable CPU bytes",
            &[
                HostVisibleMemoryPhase::CpuAccessible,
                HostVisibleMemoryPhase::CpuAccessibleAfterUnmap,
            ],
        )?;
        let mapping = self
            .mapping
            .as_mut()
            .ok_or(MemorySessionError::KernelResultMalformed(
                "CPU mapping ownership",
            ))?;
        let requested = self
            .layout
            .ok_or(MemorySessionError::KernelResultMalformed(
                "allocation layout",
            ))?
            .requested_bytes;
        Ok(B::with_bytes_mut(mapping, requested, f))
    }

    fn map_to_gpu(&mut self) -> Result<(), MemorySessionError> {
        self.require_phase("map to GPU", &[HostVisibleMemoryPhase::CpuAccessible])?;
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        let handle = self
            .handle
            .ok_or(MemorySessionError::KernelResultMalformed(
                "allocation handle ownership",
            ))?;
        let outcome = self.backend.map_gpu(handle, 0);
        if outcome.value > 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "MAP_MEMORY_TO_GPU cumulative n_success",
            ));
        }
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if outcome.value != 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "MAP_MEMORY_TO_GPU full prefix",
            ));
        }
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        self.phase = HostVisibleMemoryPhase::GpuAccessible;
        Ok(())
    }

    fn unmap_from_gpu(&mut self) -> Result<(), MemorySessionError> {
        self.require_phase("unmap from GPU", &[HostVisibleMemoryPhase::GpuAccessible])?;
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        let handle = self
            .handle
            .ok_or(MemorySessionError::KernelResultMalformed(
                "allocation handle ownership",
            ))?;
        let outcome = self.backend.unmap_gpu(handle, 0);
        if outcome.value > 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "UNMAP_MEMORY_FROM_GPU cumulative n_success",
            ));
        }
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if outcome.value != 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "UNMAP_MEMORY_FROM_GPU full prefix",
            ));
        }
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        self.phase = HostVisibleMemoryPhase::CpuAccessibleAfterUnmap;
        Ok(())
    }

    fn release(&mut self) -> Result<(), MemorySessionError> {
        self.require_phase(
            "release",
            &[
                HostVisibleMemoryPhase::CpuAccessible,
                HostVisibleMemoryPhase::CpuAccessibleAfterUnmap,
            ],
        )?;
        if self.free_attempted {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "FREE_MEMORY_OF_GPU exactly-once",
            ));
        }
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        let Some(mapping) = self.mapping.as_mut() else {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "CPU mapping ownership",
            ));
        };
        if let Err(error) = self.backend.unmap_cpu(mapping) {
            return self.quarantine(error);
        }
        self.mapping = None;
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        let handle = self
            .handle
            .ok_or(MemorySessionError::KernelResultMalformed(
                "allocation handle ownership",
            ))?;
        self.free_attempted = true;
        if let Err(error) = self.backend.free(handle) {
            return self.quarantine(error);
        }
        self.handle = None;
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        self.phase = HostVisibleMemoryPhase::Released;
        Ok(())
    }
}

impl<B: MemoryBackend> Drop for MemoryEngine<B> {
    fn drop(&mut self) {
        // Deliberately no ioctl, munmap, or FREE retry. Kernel results can be
        // ambiguous and FREE is destructive before all internal error points.
    }
}

fn ordinary_footprint(requested_bytes: usize) -> Result<usize, MemorySessionError> {
    if requested_bytes == 0 {
        return Err(MemorySessionError::InvalidRequestedSize);
    }
    requested_bytes
        .checked_add(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize - 1)
        .map(|bytes| bytes & !(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize - 1))
        .ok_or(MemorySessionError::SizeOverflow)
}

fn validate_reserved_range(
    base: u64,
    byte_len: u64,
    aperture: InclusiveAperture,
) -> Result<(), MemorySessionError> {
    if !base.is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1) {
        return Err(MemorySessionError::AddressNotPageAligned);
    }
    let end = base
        .checked_add(byte_len)
        .ok_or(MemorySessionError::AddressOutsideAperture)?;
    let aperture_end = aperture
        .limit()
        .checked_add(1)
        .ok_or(MemorySessionError::AddressOutsideAperture)?;
    if byte_len == 0 || base < aperture.base() || end > aperture_end {
        return Err(MemorySessionError::AddressOutsideAperture);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessVmState {
    Fresh,
    Attempting { pid: u32, gpu_id: u32 },
    Acquired { pid: u32, gpu_id: u32 },
    Poisoned,
}

static PROCESS_VM_STATE: Mutex<ProcessVmState> = Mutex::new(ProcessVmState::Fresh);
static NEXT_MODEL_VM_ID: AtomicU64 = AtomicU64::new(1);

fn begin_process_vm_attempt(pid: u32, gpu_id: u32) -> Result<(), MemorySessionError> {
    let mut state = PROCESS_VM_STATE
        .lock()
        .map_err(|_| MemorySessionError::ProcessVmStatePoisoned)?;
    match *state {
        ProcessVmState::Fresh => {
            *state = ProcessVmState::Attempting { pid, gpu_id };
            Ok(())
        }
        ProcessVmState::Attempting { .. } | ProcessVmState::Acquired { .. } => {
            Err(MemorySessionError::ProcessVmAlreadyConsumed)
        }
        ProcessVmState::Poisoned => Err(MemorySessionError::ProcessVmStatePoisoned),
    }
}

fn finish_process_vm_attempt(success: bool, pid: u32, gpu_id: u32) {
    let Ok(mut state) = PROCESS_VM_STATE.lock() else {
        return;
    };
    *state = if success {
        ProcessVmState::Acquired { pid, gpu_id }
    } else {
        ProcessVmState::Poisoned
    };
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[must_use = "dropping the session performs no munmap, FREE, or retry"]
pub struct HostVisibleMemorySession {
    engine: MemoryEngine<crate::memory_linux::LinuxMemoryBackend>,
    model: MemoryLifecycleStateV1,
    model_device: ModelDeviceAdmissionV1,
    reservation_key: VaReservationKeyV1,
    allocation_key: MemoryAllocationKeyV1,
    mapping_key: MemoryMappingKeyV1,
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl CheckedGfx942XnackMinusDevice {
    /// Irreversibly binds this process's admitted KFD VM to the retained render
    /// file. The attempt is permitted exactly once for the selected GPU.
    pub fn acquire_host_visible_memory_session(
        self,
    ) -> Result<HostVisibleMemorySession, MemorySessionError> {
        let pid = std::process::id();
        let gpu_id = self.observation().kfd_gpu_id();
        let vm_id = NEXT_MODEL_VM_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| MemorySessionError::Model("VM identity exhausted"))?;
        begin_process_vm_attempt(pid, gpu_id)?;
        let result = (|| {
            let mut engine =
                MemoryEngine::acquire(crate::memory_linux::LinuxMemoryBackend::new(self))?;
            let model_device = engine.backend.model_device();
            let aperture = engine.backend.model_aperture();
            let model_vm = engine.backend.bind_model_vm(VmIdV1(vm_id))?;
            let byte_len = aperture
                .limit()
                .checked_sub(aperture.base())
                .and_then(|length| length.checked_add(1))
                .ok_or(MemorySessionError::Model("invalid model aperture"))?;
            let model = MemoryLifecycleStateV1::new(model_device.domain_id())
                .next(MemoryTransitionV1::AcquireVm {
                    admission: model_vm,
                    mapping_devices: vec![model_device],
                    handle: UntrustedVmHandleObservationV1(vm_id),
                    aperture: GpuVaRangeV1 {
                        base: aperture.base(),
                        byte_len,
                    },
                })
                .map_err(|_| MemorySessionError::Model("VM acquisition projection"))?;
            let vm = model_vm.model_key();
            let reservation_key = VaReservationKeyV1 {
                vm,
                id: VaReservationIdV1(1),
            };
            let allocation_key = MemoryAllocationKeyV1 {
                vm,
                id: AllocationIdV1(1),
                generation: AllocationGenerationV1(1),
            };
            let mapping_key = MemoryMappingKeyV1 {
                allocation: allocation_key,
                id: MappingIdV1(1),
            };
            Ok(HostVisibleMemorySession {
                engine,
                model,
                model_device,
                reservation_key,
                allocation_key,
                mapping_key,
            })
        })();
        finish_process_vm_attempt(result.is_ok(), pid, gpu_id);
        result
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl HostVisibleMemorySession {
    pub fn phase(&self) -> HostVisibleMemoryPhase {
        self.engine.phase()
    }

    pub fn layout(&self) -> Option<HostVisibleAllocationLayout> {
        self.engine.layout()
    }

    /// Redacted counts from the private model-only success journal.
    ///
    /// Native handles, GPU virtual addresses, mapping addresses, descriptors,
    /// and model keys are intentionally not exposed.
    pub fn model_journal_summary(&self) -> MemoryModelJournalSummary {
        MemoryModelJournalSummary {
            vm_records: self.model.vms().len(),
            reservation_records: self.model.reservations().len(),
            allocation_records: self.model.allocations().len(),
            mapping_records: self.model.mappings().len(),
        }
    }

    pub fn allocate(
        &mut self,
        requested_bytes: usize,
    ) -> Result<HostVisibleAllocationLayout, MemorySessionError> {
        let layout = self.engine.allocate(requested_bytes)?;
        let Some(base) = self.engine.gpu_va else {
            return self
                .engine
                .quarantine(MemorySessionError::Model("missing GPU VA"));
        };
        let Some(handle) = self.engine.handle else {
            return self
                .engine
                .quarantine(MemorySessionError::Model("missing allocation handle"));
        };
        let projected = self
            .model
            .next(MemoryTransitionV1::ReserveVa {
                key: self.reservation_key,
                range: GpuVaRangeV1 {
                    base,
                    byte_len: layout.backing_bytes() as u64,
                },
                alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
            })
            .and_then(|state| {
                state.next(MemoryTransitionV1::Allocate {
                    key: self.allocation_key,
                    reservation: self.reservation_key,
                    handle: UntrustedAllocationHandleObservationV1(handle),
                    spec: MemoryAllocationSpecV1 {
                        byte_len: layout.backing_bytes() as u64,
                        alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
                        kind: MemoryKindV1::HostVisibleCoherent,
                        coherence: MemoryCoherenceV1::HostCoherent,
                    },
                })
            });
        match projected {
            Ok(model) => {
                self.model = model;
                Ok(layout)
            }
            Err(_) => self
                .engine
                .quarantine(MemorySessionError::Model("allocation projection")),
        }
    }

    pub fn with_bytes<R>(&self, f: impl FnOnce(&[u8]) -> R) -> Result<R, MemorySessionError> {
        self.engine.with_bytes(f)
    }

    pub fn with_bytes_mut<R>(
        &mut self,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        self.engine.with_bytes_mut(f)
    }

    /// Forks an isolated child and requires mincore to report that the CPU VMA
    /// is absent. The probe refuses to run unless /proc/self/task contains
    /// exactly one task. Available only to the non-production validation lane.
    #[cfg(feature = "live-validation")]
    pub fn verify_dontfork_child_negative(&self) -> Result<(), MemorySessionError> {
        if self.engine.backend.opener_pid() != std::process::id() {
            return Err(MemorySessionError::ProcessChanged);
        }
        self.engine.require_phase(
            "verify MADV_DONTFORK",
            &[
                HostVisibleMemoryPhase::CpuAccessible,
                HostVisibleMemoryPhase::CpuAccessibleAfterUnmap,
            ],
        )?;
        let mapping =
            self.engine
                .mapping
                .as_ref()
                .ok_or(MemorySessionError::KernelResultMalformed(
                    "CPU mapping ownership",
                ))?;
        self.engine.backend.verify_dontfork_child_negative(mapping)
    }

    pub fn map_to_gpu(&mut self) -> Result<(), MemorySessionError> {
        self.engine.map_to_gpu()?;
        let projected = self
            .model
            .next(MemoryTransitionV1::BeginMap {
                key: self.mapping_key,
                target_devices: vec![self.model_device.model_key()],
                access: MemoryAccessV1::ReadWrite,
            })
            .and_then(|state| {
                state.next(MemoryTransitionV1::ObserveMap {
                    key: self.mapping_key,
                    progress: PartialProgressObservationV1 {
                        n_success: 1,
                        status: PartialOperationStatusV1::Succeeded,
                    },
                })
            });
        match projected {
            Ok(model) => {
                self.model = model;
                Ok(())
            }
            Err(_) => self
                .engine
                .quarantine(MemorySessionError::Model("map projection")),
        }
    }

    pub fn unmap_from_gpu(&mut self) -> Result<(), MemorySessionError> {
        self.engine.unmap_from_gpu()?;
        let projected = self
            .model
            .next(MemoryTransitionV1::BeginUnmap {
                key: self.mapping_key,
            })
            .and_then(|state| {
                state.next(MemoryTransitionV1::ObserveUnmap {
                    key: self.mapping_key,
                    progress: PartialProgressObservationV1 {
                        n_success: 1,
                        status: PartialOperationStatusV1::Succeeded,
                    },
                })
            });
        match projected {
            Ok(model) => {
                self.model = model;
                Ok(())
            }
            Err(_) => self
                .engine
                .quarantine(MemorySessionError::Model("unmap projection")),
        }
    }

    pub fn release(&mut self) -> Result<(), MemorySessionError> {
        let mut projected = self.model.clone();
        if !projected.mappings().is_empty() {
            projected = projected
                .next(MemoryTransitionV1::ReleaseMapping {
                    key: self.mapping_key,
                })
                .map_err(|_| MemorySessionError::Model("mapping release projection"))?;
        }
        projected = projected
            .next(MemoryTransitionV1::ReleaseAllocation {
                key: self.allocation_key,
            })
            .and_then(|state| {
                state.next(MemoryTransitionV1::ReleaseVaReservation {
                    key: self.reservation_key,
                })
            })
            .map_err(|_| MemorySessionError::Model("allocation release projection"))?;
        self.engine.release()?;
        self.model = projected;
        Ok(())
    }
}

const _: () = {
    assert!(KFD_ALLOC_MEMORY_FLAGS_HOST_VISIBLE_COHERENT == 0x8400_0002);
    assert!(KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT.bits() == 0x8400_0002);
    assert!(KFD_MEMORY_LIFECYCLE_SCHEMA_MANIFEST_SHA256.len() == 64);
};

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[derive(Debug)]
    struct ScriptedBackend {
        calls: usize,
        fail_at: Option<usize>,
        map_progress: u32,
        unmap_progress: u32,
        ioctl_errno_at: Option<usize>,
        free_calls: usize,
        unmap_cpu_calls: usize,
        alloc_override: Option<KfdIoctlAllocMemoryOfGpuArgs>,
    }

    impl ScriptedBackend {
        fn good() -> Self {
            Self {
                calls: 0,
                fail_at: None,
                map_progress: 1,
                unmap_progress: 1,
                ioctl_errno_at: None,
                free_calls: 0,
                unmap_cpu_calls: 0,
                alloc_override: None,
            }
        }

        fn tick(&mut self, operation: &'static str) -> Result<(), MemorySessionError> {
            self.calls += 1;
            if self.fail_at == Some(self.calls) {
                Err(MemorySessionError::Injected(operation))
            } else {
                Ok(())
            }
        }

        fn outcome<T>(&mut self, operation: &'static str, value: T) -> KernelOutcome<T> {
            self.calls += 1;
            let result =
                if self.fail_at == Some(self.calls) || self.ioctl_errno_at == Some(self.calls) {
                    Err(MemorySessionError::Injected(operation))
                } else {
                    Ok(())
                };
            KernelOutcome { value, result }
        }
    }

    impl MemoryBackend for ScriptedBackend {
        type Reservation = u64;
        type Mapping = Vec<u8>;

        fn opener_pid(&self) -> u32 {
            std::process::id()
        }
        fn gpu_id(&self) -> u32 {
            7
        }
        fn gpuvm_aperture(&self) -> InclusiveAperture {
            InclusiveAperture::from_checked_parts_for_memory_tests(0x1_0000, 0x1f_ffff)
        }
        fn page_size(&self) -> usize {
            4096
        }
        fn check_currentness(&mut self) -> Result<(), MemorySessionError> {
            self.tick("currentness")
        }
        fn acquire_vm(&mut self) -> Result<(), MemorySessionError> {
            self.tick("acquire_vm")
        }
        fn reserve_va(&mut self, _bytes: usize) -> Result<Self::Reservation, MemorySessionError> {
            self.tick("reserve_va")?;
            Ok(0x2_0000)
        }
        fn reservation_address(reservation: &Self::Reservation) -> u64 {
            *reservation
        }
        fn alloc(&mut self, va: u64, bytes: u64) -> KernelOutcome<KfdIoctlAllocMemoryOfGpuArgs> {
            let mut args = KfdIoctlAllocMemoryOfGpuArgs::new(
                va,
                bytes,
                7,
                KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
            );
            args.handle = 0x55;
            args.mmap_offset = 0x40_000;
            if let Some(override_args) = self.alloc_override {
                args = override_args;
            }
            self.outcome("alloc", args)
        }
        fn map_cpu(
            &mut self,
            _reservation: &mut u64,
            _offset: u64,
            bytes: usize,
        ) -> Result<Vec<u8>, MemorySessionError> {
            self.tick("map_cpu")?;
            Ok(vec![0; bytes])
        }
        fn advise_dontfork(&mut self, _mapping: &mut Vec<u8>) -> Result<(), MemorySessionError> {
            self.tick("madvise_dontfork")
        }
        fn map_gpu(&mut self, _handle: u64, _old: u32) -> KernelOutcome<u32> {
            self.outcome("map_gpu", self.map_progress)
        }
        fn unmap_gpu(&mut self, _handle: u64, _old: u32) -> KernelOutcome<u32> {
            self.outcome("unmap_gpu", self.unmap_progress)
        }
        fn with_bytes<R>(mapping: &Vec<u8>, requested: usize, f: impl FnOnce(&[u8]) -> R) -> R {
            f(&mapping[..requested])
        }
        fn with_bytes_mut<R>(
            mapping: &mut Vec<u8>,
            requested: usize,
            f: impl FnOnce(&mut [u8]) -> R,
        ) -> R {
            f(&mut mapping[..requested])
        }
        fn unmap_cpu(&mut self, _mapping: &mut Vec<u8>) -> Result<(), MemorySessionError> {
            self.unmap_cpu_calls += 1;
            self.tick("munmap")
        }
        fn free(&mut self, _handle: u64) -> Result<(), MemorySessionError> {
            self.free_calls += 1;
            self.tick("free")
        }
    }

    fn acquired() -> MemoryEngine<ScriptedBackend> {
        MemoryEngine::acquire(ScriptedBackend::good()).unwrap()
    }

    #[test]
    fn memory_profile_manifest_is_frozen() {
        let digest = Sha256::digest(HOST_VISIBLE_MEMORY_PROFILE_MANIFEST_V1);
        assert_eq!(
            digest.as_slice(),
            HOST_VISIBLE_MEMORY_PROFILE_SHA256_BYTES_V1
        );
        assert_eq!(HOST_VISIBLE_MEMORY_PROFILE_SHA256_V1.len(), 64);
        assert!(
            HOST_VISIBLE_MEMORY_PROFILE_MANIFEST_V1
                .contains(crate::DEVICE_ADMISSION_PROFILE_SHA256_V1)
        );
        assert!(
            HOST_VISIBLE_MEMORY_PROFILE_MANIFEST_V1
                .contains(fe2o3_kfd_uapi::KFD_MEMORY_LIFECYCLE_SCHEMA_MANIFEST_SHA256)
        );
    }

    #[test]
    fn ordinary_footprint_is_checked_and_page_aligned() {
        assert_eq!(ordinary_footprint(1).unwrap(), 4096);
        assert_eq!(ordinary_footprint(4096).unwrap(), 4096);
        assert_eq!(ordinary_footprint(4097).unwrap(), 8192);
        assert!(matches!(
            ordinary_footprint(0),
            Err(MemorySessionError::InvalidRequestedSize)
        ));
        assert!(matches!(
            ordinary_footprint(usize::MAX),
            Err(MemorySessionError::SizeOverflow)
        ));
    }

    #[test]
    fn successful_lifecycle_orders_munmap_before_one_free() {
        let mut engine = acquired();
        let layout = engine.allocate(4097).unwrap();
        assert_eq!(layout.backing_bytes(), 8192);
        engine.with_bytes_mut(|bytes| bytes[0] = 42).unwrap();
        engine.map_to_gpu().unwrap();
        assert!(engine.with_bytes(|_| ()).is_err());
        engine.unmap_from_gpu().unwrap();
        assert_eq!(engine.with_bytes(|bytes| bytes[0]).unwrap(), 42);
        engine.release().unwrap();
        assert_eq!(engine.phase(), HostVisibleMemoryPhase::Released);
        assert_eq!(engine.backend.unmap_cpu_calls, 1);
        assert_eq!(engine.backend.free_calls, 1);
        assert!(engine.release().is_err());
        assert_eq!(engine.backend.free_calls, 1);
    }

    #[test]
    fn full_prefix_plus_errno_is_ambiguous_and_quarantines() {
        let mut engine = acquired();
        let _ = engine.allocate(4096).unwrap();
        engine.backend.ioctl_errno_at = Some(engine.backend.calls + 2);
        assert!(engine.map_to_gpu().is_err());
        assert_eq!(engine.phase(), HostVisibleMemoryPhase::Quarantined);
    }

    #[test]
    fn malformed_progress_quarantines() {
        let mut map = acquired();
        let _ = map.allocate(4096).unwrap();
        map.backend.map_progress = 2;
        assert!(map.map_to_gpu().is_err());
        assert_eq!(map.phase(), HostVisibleMemoryPhase::Quarantined);

        let mut unmap = acquired();
        let _ = unmap.allocate(4096).unwrap();
        unmap.map_to_gpu().unwrap();
        unmap.backend.unmap_progress = 2;
        assert!(unmap.unmap_from_gpu().is_err());
        assert_eq!(unmap.phase(), HostVisibleMemoryPhase::Quarantined);
    }

    #[test]
    fn free_failure_is_terminal_and_never_retried_by_drop() {
        let mut engine = acquired();
        let _ = engine.allocate(4096).unwrap();
        engine.backend.fail_at = Some(engine.backend.calls + 4);
        assert!(engine.release().is_err());
        assert_eq!(engine.phase(), HostVisibleMemoryPhase::Quarantined);
        assert_eq!(engine.backend.unmap_cpu_calls, 1);
        assert_eq!(engine.backend.free_calls, 1);
    }

    #[test]
    fn every_allocation_stage_failure_quarantines() {
        for relative_failure in 1..=7 {
            let mut engine = acquired();
            engine.backend.fail_at = Some(engine.backend.calls + relative_failure);
            assert!(engine.allocate(4096).is_err(), "failure {relative_failure}");
            assert_eq!(engine.phase(), HostVisibleMemoryPhase::Quarantined);
        }
    }

    #[test]
    fn every_map_and_unmap_stage_failure_quarantines() {
        for relative_failure in 1..=3 {
            let mut engine = acquired();
            let _ = engine.allocate(4096).unwrap();
            engine.backend.fail_at = Some(engine.backend.calls + relative_failure);
            assert!(
                engine.map_to_gpu().is_err(),
                "map failure {relative_failure}"
            );
            assert_eq!(engine.phase(), HostVisibleMemoryPhase::Quarantined);
        }

        for relative_failure in 1..=3 {
            let mut engine = acquired();
            let _ = engine.allocate(4096).unwrap();
            engine.map_to_gpu().unwrap();
            engine.backend.fail_at = Some(engine.backend.calls + relative_failure);
            assert!(
                engine.unmap_from_gpu().is_err(),
                "unmap failure {relative_failure}"
            );
            assert_eq!(engine.phase(), HostVisibleMemoryPhase::Quarantined);
        }
    }

    #[test]
    fn every_release_stage_failure_quarantines_without_free_retry() {
        for relative_failure in 1..=5 {
            let mut engine = acquired();
            let _ = engine.allocate(4096).unwrap();
            engine.backend.fail_at = Some(engine.backend.calls + relative_failure);
            assert!(
                engine.release().is_err(),
                "release failure {relative_failure}"
            );
            assert_eq!(engine.phase(), HostVisibleMemoryPhase::Quarantined);
            assert!(engine.backend.free_calls <= 1);
            assert!(engine.release().is_err());
            assert!(engine.backend.free_calls <= 1);
        }
    }

    #[test]
    fn malformed_alloc_outputs_quarantine() {
        let mut good = KfdIoctlAllocMemoryOfGpuArgs::new(
            0x2_0000,
            4096,
            7,
            KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
        );
        good.handle = 0x55;
        good.mmap_offset = 0x40_000;

        let mut mutations = Vec::new();
        let mut value = good;
        value.va_addr += 4096;
        mutations.push(value);
        value = good;
        value.size += 4096;
        mutations.push(value);
        value = good;
        value.gpu_id += 1;
        mutations.push(value);
        value = good;
        value.flags = 0;
        mutations.push(value);
        value = good;
        value.handle = 0;
        mutations.push(value);
        value = good;
        value.mmap_offset = 1;
        mutations.push(value);

        for mutation in mutations {
            let mut engine = acquired();
            engine.backend.alloc_override = Some(mutation);
            assert!(engine.allocate(4096).is_err());
            assert_eq!(engine.phase(), HostVisibleMemoryPhase::Quarantined);
        }
    }
}
