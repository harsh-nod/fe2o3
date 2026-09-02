#![no_std]
#![forbid(unsafe_code)]

//! Reviewed raw definitions for the first Linux DRM/AMDGPU identity and
//! destructive-reset observation slice.
//!
//! This crate performs no file access, allocation, pointer dereference, FFI, or
//! `ioctl`. It describes only the x86_64 LP64 records and request encodings a
//! later adapter needs to identify an AMDGPU render node and query immutable
//! model information.
//!
//! Source provenance is frozen in the schema manifest and crate README.
//! Third-party notices cover the transcribed DRM/AMDGPU records and constants.
//! The fe2o3-authored encoder implements reviewed Linux `_IOC` ABI facts;
//! its provenance is recorded separately in `THIRD_PARTY_LICENSES.md`.

use core::mem::{align_of, offset_of, size_of};

/// Stable name of the exact reviewed userspace schema.
pub const DRM_UAPI_SCHEMA_ID: &str =
    "linux-x86_64-drm-amdgpu-3.64.0-dkms-6.16.13-identity-currentness-v1";

/// Architecture and data model admitted by this schema.
pub const DRM_UAPI_ADMITTED_TARGET: &str = "linux-x86_64-lp64-generic-ioc";

/// Whether this compilation target matches the admitted architecture profile.
pub const DRM_UAPI_TARGET_MATCHES_BUILD: bool = cfg!(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
));

/// SHA-256 of the running kernel build's reviewed core DRM UAPI header.
pub const DRM_UAPI_KERNEL_CORE_HEADER_SHA256: &str =
    "3ab6ac01bf91067aed96b70d7fa7847a86e7f726d74278151f085143688659cc";

/// SHA-256 of the exported core DRM header used by the active-header oracle.
pub const DRM_UAPI_EXPORTED_CORE_HEADER_SHA256: &str =
    "6b80aff056e2ac2e126e5144a3ce2c750292edb4d080d4689ac487dc17e4dae8";

/// SHA-256 of the active AMDGPU DKMS driver's reviewed UAPI header.
pub const DRM_UAPI_AMDGPU_DKMS_HEADER_SHA256: &str =
    "9d7ff60a211d2aa73a6c15b2da49e050cebe518fc059ee93e31d61288f7b60dc";

/// SHA-256 of the active driver's implementation of AMDGPU INFO queries.
pub const DRM_UAPI_AMDGPU_KMS_SOURCE_SHA256: &str =
    "ef2375c3f35ad4a24b560326b55676a907d6d2ba248e469a62e84e877435101c";

/// SHA-256 of the active driver's VRAM-loss detection and increment sites.
pub const DRM_UAPI_AMDGPU_DEVICE_SOURCE_SHA256: &str =
    "4d0edc4b714c005e911596e0e2e616be7fdbbb3526069938e4cc078eaba83673";

/// SHA-256 of the independently checked libdrm core header.
pub const DRM_UAPI_LIBDRM_CORE_HEADER_SHA256: &str =
    "e97d535df3d33844a7c66578cb5adb501c57d17fb5ba55395309d1f275432060";

/// SHA-256 of the independently checked libdrm AMDGPU header.
pub const DRM_UAPI_LIBDRM_AMDGPU_HEADER_SHA256: &str =
    "2881120496c69fc2154e590d0bc6e615a48adc43df1a658dd8cd8f78ec648557";

/// Canonical content manifest for the admitted schema.
///
/// This identifies reviewed definitions. It does not authenticate a running
/// kernel, driver, render node, or GPU.
pub const DRM_UAPI_SCHEMA_MANIFEST: &str = concat!(
    "schema_id=linux-x86_64-drm-amdgpu-3.64.0-dkms-6.16.13-identity-currentness-v1\n",
    "target=linux-x86_64-lp64-generic-ioc\n",
    "kernel_core_header=include/uapi/drm/drm.h\n",
    "kernel_core_header_sha256=3ab6ac01bf91067aed96b70d7fa7847a86e7f726d74278151f085143688659cc\n",
    "kernel_core_package=linux-headers-6.8.0-124@6.8.0-124.124\n",
    "exported_core_header_sha256=6b80aff056e2ac2e126e5144a3ce2c750292edb4d080d4689ac487dc17e4dae8\n",
    "exported_core_package=linux-libc-dev@6.8.0-137.137\n",
    "amdgpu_header=include/uapi/drm/amdgpu_drm.h\n",
    "amdgpu_header_sha256=9d7ff60a211d2aa73a6c15b2da49e050cebe518fc059ee93e31d61288f7b60dc\n",
    "amdgpu_kms_source_sha256=ef2375c3f35ad4a24b560326b55676a907d6d2ba248e469a62e84e877435101c\n",
    "amdgpu_device_source_sha256=4d0edc4b714c005e911596e0e2e616be7fdbbb3526069938e4cc078eaba83673\n",
    "amdgpu_package=amdgpu-dkms@1:6.16.13.30300400-2341068.24.04\n",
    "libdrm_core_header_sha256=e97d535df3d33844a7c66578cb5adb501c57d17fb5ba55395309d1f275432060\n",
    "libdrm_amdgpu_header_sha256=2881120496c69fc2154e590d0bc6e615a48adc43df1a658dd8cd8f78ec648557\n",
    "libdrm_package=libdrm-dev@2.4.125-1ubuntu0.1~24.04.2\n",
    "amdgpu_driver=amdgpu@3.64.0\n",
    "drm_version=size:64,align:8,major:0,minor:4,patch:8,pad:12,name_len:16,name:24,date_len:32,date:40,desc_len:48,desc:56,request:c0406400\n",
    "amdgpu_info=size:32,align:8,return_pointer:0,return_size:8,query:12,query_data:16,request:40206445\n",
    "device_identity_v1=size:20,align:4,device_id:0,chip_rev:4,external_rev:8,pci_rev:12,family:16\n",
    "queries=accel_working:00,dev_info:16,vram_lost_counter:1f\n",
    "family_constants=ai:141\n",
);

/// SHA-256 of [`DRM_UAPI_SCHEMA_MANIFEST`].
pub const DRM_UAPI_SCHEMA_MANIFEST_SHA256: &str =
    "800569fe9b467b389bcfc6e5d65b23d66a0386a90fc2a669fac8c83800e76d8b";

/// Typed digest bytes of [`DRM_UAPI_SCHEMA_MANIFEST`].
pub const DRM_UAPI_SCHEMA_MANIFEST_SHA256_BYTES: [u8; 32] = [
    0x80, 0x05, 0x69, 0xfe, 0x9b, 0x46, 0x7b, 0x38, 0x9b, 0xcf, 0xc6, 0xe5, 0xd6, 0x5b, 0x23, 0xd6,
    0x6a, 0x03, 0x86, 0xa9, 0x0f, 0xc2, 0xa6, 0x69, 0xfa, 0xc8, 0xc8, 0x38, 0x00, 0xe7, 0x6d, 0x8b,
];

/// Driver name required from `DRM_IOCTL_VERSION` before AMDGPU queries.
pub const AMDGPU_DRM_DRIVER_NAME: &[u8] = b"amdgpu";

/// Exact active driver interface version reviewed by this schema.
pub const AMDGPU_DRM_DRIVER_VERSION: DrmDriverVersion = DrmDriverVersion::new(3, 64, 0);

/// DRM ioctl type byte (`'d'`).
pub const DRM_IOCTL_BASE: u8 = b'd';

/// Base request number for driver-specific DRM ioctls.
pub const DRM_COMMAND_BASE: u8 = 0x40;

/// AMDGPU driver command offset for the INFO request.
pub const DRM_AMDGPU_INFO: u8 = 0x05;

/// Query whether acceleration is operational; output is one `u32`.
pub const AMDGPU_INFO_ACCEL_WORKING: u32 = 0x00;

/// Query the append-only `drm_amdgpu_info_device` record.
pub const AMDGPU_INFO_DEV_INFO: u32 = 0x16;

/// Query the driver's 32-bit counter of resets determined to have lost VRAM.
///
/// This does not count resets that preserve VRAM and is not a general GPU reset
/// generation. The counter may wrap after `u32::MAX`.
pub const AMDGPU_INFO_VRAM_LOST_COUNTER: u32 = 0x1f;

/// `AMDGPU_FAMILY_AI` from the reviewed header.
///
/// This family includes multiple products and is not a unique gfx942 identity.
pub const AMDGPU_FAMILY_AI: u32 = 141;

/// Number of output bytes admitted for `AMDGPU_INFO_ACCEL_WORKING`.
pub const AMDGPU_ACCEL_WORKING_RESULT_BYTES: u32 = 4;

/// Number of immutable prefix bytes admitted from `AMDGPU_INFO_DEV_INFO`.
pub const AMDGPU_DEVICE_IDENTITY_V1_BYTES: u32 = 20;

/// Number of output bytes admitted for `AMDGPU_INFO_VRAM_LOST_COUNTER`.
pub const AMDGPU_VRAM_LOST_COUNTER_RESULT_BYTES: u32 = 4;

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum IoctlDirection {
    None = 0,
    Write = 1,
    Read = 2,
    ReadWrite = 3,
}

/// Encodes a Linux generic ioctl request without libc or generated bindings.
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
        None => panic!("admitted DRM ioctl payload exceeds Linux _IOC size field"),
    }
}

/// Driver interface version returned in `drm_version`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DrmDriverVersion {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
}

impl DrmDriverVersion {
    pub const fn new(major: i32, minor: i32, patch: i32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Tests exact compatibility with this frozen schema.
    pub const fn is_admitted_amdgpu(self) -> bool {
        self.major == AMDGPU_DRM_DRIVER_VERSION.major
            && self.minor == AMDGPU_DRM_DRIVER_VERSION.minor
            && self.patch == AMDGPU_DRM_DRIVER_VERSION.patch
    }
}

/// x86_64 LP64 layout of `struct drm_version`.
///
/// Pointer fields are opaque userspace addresses. This crate never constructs
/// pointers from them or dereferences them. The explicit alignment word models
/// C padding so callers can initialize every transferred byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DrmVersion {
    pub version_major: i32,
    pub version_minor: i32,
    pub version_patchlevel: i32,
    pub alignment_padding: u32,
    pub name_len: u64,
    pub name: u64,
    pub date_len: u64,
    pub date: u64,
    pub desc_len: u64,
    pub desc: u64,
}

impl DrmVersion {
    pub const fn zeroed() -> Self {
        Self {
            version_major: 0,
            version_minor: 0,
            version_patchlevel: 0,
            alignment_padding: 0,
            name_len: 0,
            name: 0,
            date_len: 0,
            date: 0,
            desc_len: 0,
            desc: 0,
        }
    }

    pub const fn reported_driver_version(self) -> DrmDriverVersion {
        DrmDriverVersion::new(
            self.version_major,
            self.version_minor,
            self.version_patchlevel,
        )
    }
}

/// Layout of `struct drm_amdgpu_info` for the admitted no-subquery requests.
///
/// `query_data` is the 16-byte anonymous C union. It must remain zero for the
/// admitted queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DrmAmdgpuInfo {
    pub return_pointer: u64,
    pub return_size: u32,
    pub query: u32,
    pub query_data: [u32; 4],
}

impl DrmAmdgpuInfo {
    pub const fn acceleration_status(return_pointer: u64) -> Self {
        Self {
            return_pointer,
            return_size: AMDGPU_ACCEL_WORKING_RESULT_BYTES,
            query: AMDGPU_INFO_ACCEL_WORKING,
            query_data: [0; 4],
        }
    }

    pub const fn device_identity_v1(return_pointer: u64) -> Self {
        Self {
            return_pointer,
            return_size: AMDGPU_DEVICE_IDENTITY_V1_BYTES,
            query: AMDGPU_INFO_DEV_INFO,
            query_data: [0; 4],
        }
    }

    /// Constructs the query for the driver's destructive-reset observation.
    pub const fn vram_lost_counter(return_pointer: u64) -> Self {
        Self {
            return_pointer,
            return_size: AMDGPU_VRAM_LOST_COUNTER_RESULT_BYTES,
            query: AMDGPU_INFO_VRAM_LOST_COUNTER,
            query_data: [0; 4],
        }
    }
}

/// Immutable 20-byte prefix of `struct drm_amdgpu_info_device` admitted by V1.
///
/// These fields describe a model and revision, not a unique physical GPU. A
/// later adapter must bind the render descriptor to PCI/KFD topology identity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct DrmAmdgpuDeviceIdentityV1 {
    pub device_id: u32,
    pub chip_rev: u32,
    pub external_rev: u32,
    pub pci_rev: u32,
    pub family: u32,
}

/// Request for `_IOWR('d', 0x00, struct drm_version)`.
pub const DRM_IOCTL_VERSION: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::ReadWrite,
    DRM_IOCTL_BASE,
    0x00,
    size_of::<DrmVersion>(),
);

/// Request for `_IOW('d', 0x45, struct drm_amdgpu_info)`.
///
/// The request is write-direction because the fixed struct is input; result
/// bytes are written through `return_pointer`.
pub const DRM_IOCTL_AMDGPU_INFO: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::Write,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_AMDGPU_INFO,
    size_of::<DrmAmdgpuInfo>(),
);

// Compile-time ABI assertions for the admitted schema.
const _: () = {
    assert!(size_of::<DrmVersion>() == 64);
    assert!(align_of::<DrmVersion>() == 8);
    assert!(offset_of!(DrmVersion, version_major) == 0);
    assert!(offset_of!(DrmVersion, version_minor) == 4);
    assert!(offset_of!(DrmVersion, version_patchlevel) == 8);
    assert!(offset_of!(DrmVersion, alignment_padding) == 12);
    assert!(offset_of!(DrmVersion, name_len) == 16);
    assert!(offset_of!(DrmVersion, name) == 24);
    assert!(offset_of!(DrmVersion, date_len) == 32);
    assert!(offset_of!(DrmVersion, date) == 40);
    assert!(offset_of!(DrmVersion, desc_len) == 48);
    assert!(offset_of!(DrmVersion, desc) == 56);

    assert!(size_of::<DrmAmdgpuInfo>() == 32);
    assert!(align_of::<DrmAmdgpuInfo>() == 8);
    assert!(offset_of!(DrmAmdgpuInfo, return_pointer) == 0);
    assert!(offset_of!(DrmAmdgpuInfo, return_size) == 8);
    assert!(offset_of!(DrmAmdgpuInfo, query) == 12);
    assert!(offset_of!(DrmAmdgpuInfo, query_data) == 16);

    assert!(size_of::<DrmAmdgpuDeviceIdentityV1>() == 20);
    assert!(align_of::<DrmAmdgpuDeviceIdentityV1>() == 4);
    assert!(offset_of!(DrmAmdgpuDeviceIdentityV1, device_id) == 0);
    assert!(offset_of!(DrmAmdgpuDeviceIdentityV1, chip_rev) == 4);
    assert!(offset_of!(DrmAmdgpuDeviceIdentityV1, external_rev) == 8);
    assert!(offset_of!(DrmAmdgpuDeviceIdentityV1, pci_rev) == 12);
    assert!(offset_of!(DrmAmdgpuDeviceIdentityV1, family) == 16);

    assert!(DRM_IOCTL_VERSION == 0xc040_6400);
    assert!(DRM_IOCTL_AMDGPU_INFO == 0x4020_6445);
    assert!(AMDGPU_INFO_ACCEL_WORKING == 0x00);
    assert!(AMDGPU_INFO_DEV_INFO == 0x16);
    assert!(AMDGPU_INFO_VRAM_LOST_COUNTER == 0x1f);
};
