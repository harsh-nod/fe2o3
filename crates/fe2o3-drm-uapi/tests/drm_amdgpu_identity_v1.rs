use core::mem::{align_of, offset_of, size_of};

use fe2o3_drm_uapi::*;
use sha2::{Digest, Sha256};

#[test]
fn schema_and_provenance_are_frozen() {
    assert_eq!(
        DRM_UAPI_SCHEMA_ID,
        "linux-x86_64-drm-amdgpu-3.64.0-dkms-6.16.13-identity-currentness-v1"
    );
    assert_eq!(
        DRM_UAPI_KERNEL_CORE_HEADER_SHA256,
        "3ab6ac01bf91067aed96b70d7fa7847a86e7f726d74278151f085143688659cc"
    );
    assert_eq!(
        DRM_UAPI_EXPORTED_CORE_HEADER_SHA256,
        "6b80aff056e2ac2e126e5144a3ce2c750292edb4d080d4689ac487dc17e4dae8"
    );
    assert_eq!(
        DRM_UAPI_AMDGPU_DKMS_HEADER_SHA256,
        "9d7ff60a211d2aa73a6c15b2da49e050cebe518fc059ee93e31d61288f7b60dc"
    );
    assert_eq!(
        DRM_UAPI_AMDGPU_KMS_SOURCE_SHA256,
        "ef2375c3f35ad4a24b560326b55676a907d6d2ba248e469a62e84e877435101c"
    );
    assert_eq!(
        DRM_UAPI_AMDGPU_DEVICE_SOURCE_SHA256,
        "4d0edc4b714c005e911596e0e2e616be7fdbbb3526069938e4cc078eaba83673"
    );
    assert_eq!(
        DRM_UAPI_LIBDRM_CORE_HEADER_SHA256,
        "e97d535df3d33844a7c66578cb5adb501c57d17fb5ba55395309d1f275432060"
    );
    assert_eq!(
        DRM_UAPI_LIBDRM_AMDGPU_HEADER_SHA256,
        "2881120496c69fc2154e590d0bc6e615a48adc43df1a658dd8cd8f78ec648557"
    );
    let manifest_digest = Sha256::digest(DRM_UAPI_SCHEMA_MANIFEST);
    assert_eq!(&manifest_digest[..], &DRM_UAPI_SCHEMA_MANIFEST_SHA256_BYTES);
    assert_eq!(
        DRM_UAPI_SCHEMA_MANIFEST_SHA256_BYTES,
        [
            0x80, 0x05, 0x69, 0xfe, 0x9b, 0x46, 0x7b, 0x38, 0x9b, 0xcf, 0xc6, 0xe5, 0xd6, 0x5b,
            0x23, 0xd6, 0x6a, 0x03, 0x86, 0xa9, 0x0f, 0xc2, 0xa6, 0x69, 0xfa, 0xc8, 0xc8, 0x38,
            0x00, 0xe7, 0x6d, 0x8b,
        ]
    );
    assert_eq!(
        DRM_UAPI_SCHEMA_MANIFEST_SHA256,
        "800569fe9b467b389bcfc6e5d65b23d66a0386a90fc2a669fac8c83800e76d8b"
    );
}

#[test]
fn drm_version_layout_matches_both_c_oracles() {
    assert_eq!(size_of::<DrmVersion>(), 64);
    assert_eq!(align_of::<DrmVersion>(), 8);
    assert_eq!(offset_of!(DrmVersion, version_major), 0);
    assert_eq!(offset_of!(DrmVersion, version_minor), 4);
    assert_eq!(offset_of!(DrmVersion, version_patchlevel), 8);
    assert_eq!(offset_of!(DrmVersion, alignment_padding), 12);
    assert_eq!(offset_of!(DrmVersion, name_len), 16);
    assert_eq!(offset_of!(DrmVersion, name), 24);
    assert_eq!(offset_of!(DrmVersion, date_len), 32);
    assert_eq!(offset_of!(DrmVersion, date), 40);
    assert_eq!(offset_of!(DrmVersion, desc_len), 48);
    assert_eq!(offset_of!(DrmVersion, desc), 56);
}

#[test]
fn amdgpu_info_layout_matches_both_c_oracles() {
    assert_eq!(size_of::<DrmAmdgpuInfo>(), 32);
    assert_eq!(align_of::<DrmAmdgpuInfo>(), 8);
    assert_eq!(offset_of!(DrmAmdgpuInfo, return_pointer), 0);
    assert_eq!(offset_of!(DrmAmdgpuInfo, return_size), 8);
    assert_eq!(offset_of!(DrmAmdgpuInfo, query), 12);
    assert_eq!(offset_of!(DrmAmdgpuInfo, query_data), 16);
}

#[test]
fn immutable_device_prefix_matches_both_c_oracles() {
    assert_eq!(size_of::<DrmAmdgpuDeviceIdentityV1>(), 20);
    assert_eq!(align_of::<DrmAmdgpuDeviceIdentityV1>(), 4);
    assert_eq!(offset_of!(DrmAmdgpuDeviceIdentityV1, device_id), 0);
    assert_eq!(offset_of!(DrmAmdgpuDeviceIdentityV1, chip_rev), 4);
    assert_eq!(offset_of!(DrmAmdgpuDeviceIdentityV1, external_rev), 8);
    assert_eq!(offset_of!(DrmAmdgpuDeviceIdentityV1, pci_rev), 12);
    assert_eq!(offset_of!(DrmAmdgpuDeviceIdentityV1, family), 16);
}

#[test]
fn ioctl_and_query_numbers_match_both_c_oracles() {
    assert_eq!(DRM_IOCTL_VERSION, 0xc040_6400);
    assert_eq!(DRM_IOCTL_AMDGPU_INFO, 0x4020_6445);
    assert_eq!(DRM_AMDGPU_INFO, 0x05);
    assert_eq!(AMDGPU_INFO_ACCEL_WORKING, 0x00);
    assert_eq!(AMDGPU_INFO_DEV_INFO, 0x16);
    assert_eq!(AMDGPU_INFO_VRAM_LOST_COUNTER, 0x1f);
    assert_eq!(AMDGPU_FAMILY_AI, 141);

    assert_eq!(
        encode_ioctl(
            IoctlDirection::ReadWrite,
            DRM_IOCTL_BASE,
            0x00,
            size_of::<DrmVersion>(),
        ),
        Some(DRM_IOCTL_VERSION),
    );
    assert_eq!(
        encode_ioctl(
            IoctlDirection::Write,
            DRM_IOCTL_BASE,
            DRM_COMMAND_BASE + DRM_AMDGPU_INFO,
            size_of::<DrmAmdgpuInfo>(),
        ),
        Some(DRM_IOCTL_AMDGPU_INFO),
    );
}

#[test]
fn admitted_builders_initialize_all_fixed_input_bytes() {
    let acceleration = DrmAmdgpuInfo::acceleration_status(0x1234_5000);
    assert_eq!(acceleration.return_pointer, 0x1234_5000);
    assert_eq!(acceleration.return_size, 4);
    assert_eq!(acceleration.query, AMDGPU_INFO_ACCEL_WORKING);
    assert_eq!(acceleration.query_data, [0; 4]);

    let identity = DrmAmdgpuInfo::device_identity_v1(0x5678_9000);
    assert_eq!(identity.return_pointer, 0x5678_9000);
    assert_eq!(identity.return_size, 20);
    assert_eq!(identity.query, AMDGPU_INFO_DEV_INFO);
    assert_eq!(identity.query_data, [0; 4]);

    let reset = DrmAmdgpuInfo::vram_lost_counter(0x9abc_d000);
    assert_eq!(reset.return_pointer, 0x9abc_d000);
    assert_eq!(reset.return_size, 4);
    assert_eq!(reset.query, AMDGPU_INFO_VRAM_LOST_COUNTER);
    assert_eq!(reset.query_data, [0; 4]);

    let version = DrmVersion::zeroed();
    assert_eq!(version.alignment_padding, 0);
    assert_eq!(version.name, 0);
    assert_eq!(version.date, 0);
    assert_eq!(version.desc, 0);
}

#[test]
fn exact_driver_version_admission_is_fail_closed() {
    assert!(DrmDriverVersion::new(3, 64, 0).is_admitted_amdgpu());
    assert!(!DrmDriverVersion::new(3, 63, 0).is_admitted_amdgpu());
    assert!(!DrmDriverVersion::new(3, 65, 0).is_admitted_amdgpu());
    assert!(!DrmDriverVersion::new(4, 0, 0).is_admitted_amdgpu());
}

#[test]
fn ioctl_encoder_rejects_unrepresentable_payload_size() {
    assert_eq!(
        encode_ioctl(IoctlDirection::ReadWrite, DRM_IOCTL_BASE, 0xff, 1 << 14,),
        None,
    );
}
