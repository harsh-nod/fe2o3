#![no_std]
#![forbid(unsafe_code)]

//! Reviewed raw definitions for a deliberately small Linux KFD UAPI slice.
//!
//! This crate contains no file access, FFI, or `ioctl` execution. It provides
//! only C-layout data, request encodings, and fail-closed version admission for
//! a later syscall adapter. The admitted schema is pinned to KFD UAPI 1.18 as
//! shipped by the active AMDGPU 6.16.13 DKMS driver on the MI300X test host.

use core::mem::{align_of, offset_of, size_of};

/// Stable name of the reviewed UAPI schema in this crate.
pub const KFD_UAPI_SCHEMA_ID: &str = "linux-kfd-uapi-1.18-generic-ioc-v1";

/// Path of the Linux UAPI header from which this schema was reviewed.
pub const KFD_UAPI_SOURCE_HEADER: &str = "include/uapi/linux/kfd_ioctl.h";

/// Major version declared by the reviewed AMDGPU 6.16.13 KFD UAPI header.
pub const KFD_IOCTL_MAJOR_VERSION: u32 = 1;

/// Minor version declared by the reviewed AMDGPU 6.16.13 KFD UAPI header.
pub const KFD_IOCTL_MINOR_VERSION: u32 = 18;

/// The only minor version admitted by this initial schema.
pub const KFD_IOCTL_MIN_ADMITTED_MINOR_VERSION: u32 = KFD_IOCTL_MINOR_VERSION;

/// The newest minor version reviewed by this initial schema.
pub const KFD_IOCTL_MAX_ADMITTED_MINOR_VERSION: u32 = KFD_IOCTL_MINOR_VERSION;

/// The KFD ioctl type byte (`'K'`).
pub const AMDKFD_IOCTL_BASE: u8 = b'K';

/// Linux generic ioctl request number type.
pub type IoctlRequest = u32;

const IOC_NR_BITS: u32 = 8;
const IOC_TYPE_BITS: u32 = 8;
const IOC_SIZE_BITS: u32 = 14;
const IOC_NR_SHIFT: u32 = 0;
const IOC_TYPE_SHIFT: u32 = IOC_NR_SHIFT + IOC_NR_BITS;
const IOC_SIZE_SHIFT: u32 = IOC_TYPE_SHIFT + IOC_TYPE_BITS;
const IOC_DIR_SHIFT: u32 = IOC_SIZE_SHIFT + IOC_SIZE_BITS;
const IOC_SIZE_MASK: usize = (1usize << IOC_SIZE_BITS) - 1;

/// Transfer direction encoded by Linux's generic `_IOC` convention.
///
/// `Write` means userspace writes data that the kernel reads. `Read` means the
/// kernel writes data that userspace reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum IoctlDirection {
    None = 0,
    Write = 1,
    Read = 2,
    ReadWrite = 3,
}

/// Encodes a Linux generic ioctl request without libc or generated bindings.
///
/// `None` is returned if the payload does not fit the generic 14-bit size
/// field. This helper models the generic Linux encoding used by the admitted
/// x86_64 runtime target; an adapter for an architecture that overrides `_IOC`
/// must define and review a separate schema.
pub const fn encode_ioctl(
    direction: IoctlDirection,
    ioctl_type: u8,
    number: u8,
    payload_size: usize,
) -> Option<IoctlRequest> {
    if payload_size > IOC_SIZE_MASK {
        return None;
    }

    Some(
        ((direction as u32) << IOC_DIR_SHIFT)
            | ((ioctl_type as u32) << IOC_TYPE_SHIFT)
            | ((number as u32) << IOC_NR_SHIFT)
            | ((payload_size as u32) << IOC_SIZE_SHIFT),
    )
}

const fn encode_admitted_ioctl(
    direction: IoctlDirection,
    ioctl_type: u8,
    number: u8,
    payload_size: usize,
) -> IoctlRequest {
    match encode_ioctl(direction, ioctl_type, number, payload_size) {
        Some(request) => request,
        None => panic!("admitted KFD ioctl payload exceeds Linux _IOC size field"),
    }
}

/// C layout of `struct kfd_ioctl_get_version_args`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlGetVersionArgs {
    /// KFD UAPI major version returned by the kernel.
    pub major_version: u32,
    /// KFD UAPI minor version returned by the kernel.
    pub minor_version: u32,
}

impl KfdIoctlGetVersionArgs {
    /// Creates a zero-initialized output buffer for `AMDKFD_IOC_GET_VERSION`.
    pub const fn zeroed() -> Self {
        Self {
            major_version: 0,
            minor_version: 0,
        }
    }

    /// Converts the raw output into the value consumed by version admission.
    pub const fn reported_version(self) -> KfdUapiVersion {
        KfdUapiVersion {
            major: self.major_version,
            minor: self.minor_version,
        }
    }
}

/// C layout of `struct kfd_ioctl_acquire_vm_args`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlAcquireVmArgs {
    /// Nonnegative DRM render-node file descriptor, represented by the UAPI as `__u32`.
    pub drm_fd: u32,
    /// KFD topology GPU identifier whose VM is being acquired.
    pub gpu_id: u32,
}

impl KfdIoctlAcquireVmArgs {
    /// Constructs the raw request after a higher layer validates descriptor and device identity.
    pub const fn new(drm_fd: u32, gpu_id: u32) -> Self {
        Self { drm_fd, gpu_id }
    }
}

/// Request number for `_IOR('K', 0x01, struct kfd_ioctl_get_version_args)`.
pub const AMDKFD_IOC_GET_VERSION: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::Read,
    AMDKFD_IOCTL_BASE,
    0x01,
    size_of::<KfdIoctlGetVersionArgs>(),
);

/// Request number for `_IOW('K', 0x15, struct kfd_ioctl_acquire_vm_args)`.
pub const AMDKFD_IOC_ACQUIRE_VM: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::Write,
    AMDKFD_IOCTL_BASE,
    0x15,
    size_of::<KfdIoctlAcquireVmArgs>(),
);

/// KFD UAPI version reported by `AMDKFD_IOC_GET_VERSION`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KfdUapiVersion {
    pub major: u32,
    pub minor: u32,
}

impl KfdUapiVersion {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

/// Evidence that a reported KFD version is covered by this reviewed schema.
///
/// The private field prevents callers from constructing admission evidence
/// without passing [`negotiate_kfd_uapi_version`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedKfdUapi {
    reported: KfdUapiVersion,
}

impl AdmittedKfdUapi {
    pub const fn reported_version(self) -> KfdUapiVersion {
        self.reported
    }

    pub const fn schema_id(self) -> &'static str {
        KFD_UAPI_SCHEMA_ID
    }

    /// Returns the admitted ACQUIRE_VM request number.
    ///
    /// Keeping this method on the admission token lets higher-level adapters
    /// require reviewed version evidence before exposing the operation.
    pub const fn acquire_vm_request(self) -> IoctlRequest {
        AMDKFD_IOC_ACQUIRE_VM
    }
}

/// Why a kernel-reported KFD UAPI version was not admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdUapiVersionError {
    UnsupportedMajor { reported: u32, admitted: u32 },
    MinorTooOld { reported: u32, minimum: u32 },
    MinorNewerThanReviewed { reported: u32, maximum: u32 },
}

/// Admits only versions whose semantics and layout were explicitly reviewed.
///
/// This initial foundation intentionally accepts exactly KFD UAPI 1.18. A
/// newer minor version may be backwards compatible in Linux, but fe2o3 must
/// review it and extend this schema before making a formal compatibility claim.
pub const fn negotiate_kfd_uapi_version(
    reported: KfdUapiVersion,
) -> Result<AdmittedKfdUapi, KfdUapiVersionError> {
    if reported.major != KFD_IOCTL_MAJOR_VERSION {
        return Err(KfdUapiVersionError::UnsupportedMajor {
            reported: reported.major,
            admitted: KFD_IOCTL_MAJOR_VERSION,
        });
    }
    if reported.minor < KFD_IOCTL_MIN_ADMITTED_MINOR_VERSION {
        return Err(KfdUapiVersionError::MinorTooOld {
            reported: reported.minor,
            minimum: KFD_IOCTL_MIN_ADMITTED_MINOR_VERSION,
        });
    }
    if reported.minor > KFD_IOCTL_MAX_ADMITTED_MINOR_VERSION {
        return Err(KfdUapiVersionError::MinorNewerThanReviewed {
            reported: reported.minor,
            maximum: KFD_IOCTL_MAX_ADMITTED_MINOR_VERSION,
        });
    }

    Ok(AdmittedKfdUapi { reported })
}

// Compile-time ABI assertions for the admitted Linux KFD 1.18 schema.
const _: () = {
    assert!(size_of::<KfdIoctlGetVersionArgs>() == 8);
    assert!(align_of::<KfdIoctlGetVersionArgs>() == 4);
    assert!(offset_of!(KfdIoctlGetVersionArgs, major_version) == 0);
    assert!(offset_of!(KfdIoctlGetVersionArgs, minor_version) == 4);

    assert!(size_of::<KfdIoctlAcquireVmArgs>() == 8);
    assert!(align_of::<KfdIoctlAcquireVmArgs>() == 4);
    assert!(offset_of!(KfdIoctlAcquireVmArgs, drm_fd) == 0);
    assert!(offset_of!(KfdIoctlAcquireVmArgs, gpu_id) == 4);

    assert!(AMDKFD_IOC_GET_VERSION == 0x8008_4b01);
    assert!(AMDKFD_IOC_ACQUIRE_VM == 0x4008_4b15);
};
