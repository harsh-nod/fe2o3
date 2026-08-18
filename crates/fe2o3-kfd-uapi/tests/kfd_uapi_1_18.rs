use core::mem::{align_of, offset_of, size_of};

use fe2o3_kfd_uapi::{
    AMDKFD_IOC_ACQUIRE_VM, AMDKFD_IOC_GET_PROCESS_APERTURES_NEW, AMDKFD_IOC_GET_VERSION,
    AMDKFD_IOC_SET_XNACK_MODE, AMDKFD_IOC_SMI_EVENTS, AMDKFD_IOCTL_BASE, IoctlDirection,
    KFD_IOCTL_MAJOR_VERSION, KFD_IOCTL_MAX_ADMITTED_MINOR_VERSION,
    KFD_IOCTL_MIN_ADMITTED_MINOR_VERSION, KFD_IOCTL_MINOR_VERSION, KFD_SMI_EVENT_GPU_POST_RESET,
    KFD_SMI_EVENT_GPU_PRE_RESET, KFD_SMI_EVENT_GPU_RESET_MASK, KFD_SMI_EVENT_MSG_SIZE,
    KFD_UAPI_CHARDEV_SOURCE_SHA256, KFD_UAPI_DEVICE_SOURCE_SHA256, KFD_UAPI_SCHEMA_ID,
    KFD_UAPI_SCHEMA_MANIFEST, KFD_UAPI_SCHEMA_MANIFEST_SHA256,
    KFD_UAPI_SCHEMA_MANIFEST_SHA256_BYTES, KFD_UAPI_SMI_EVENTS_SOURCE_SHA256,
    KFD_UAPI_SOURCE_HEADER_SHA256, KFD_XNACK_MODE_DISABLED, KFD_XNACK_MODE_ENABLED,
    KFD_XNACK_MODE_QUERY, KfdIoctlAcquireVmArgs, KfdIoctlGetProcessAperturesNewArgs,
    KfdIoctlGetVersionArgs, KfdIoctlSetXnackModeArgs, KfdIoctlSmiEventsArgs,
    KfdProcessDeviceApertures, KfdUapiVersion, KfdUapiVersionError, encode_ioctl,
    negotiate_kfd_uapi_version,
};
use sha2::{Digest, Sha256};

#[test]
fn schema_identity_is_linux_kfd_1_18() {
    assert_eq!(KFD_UAPI_SCHEMA_ID, "linux-kfd-uapi-1.18-generic-ioc-v1");
    assert_eq!(KFD_IOCTL_MAJOR_VERSION, 1);
    assert_eq!(KFD_IOCTL_MINOR_VERSION, 18);
    assert_eq!(KFD_IOCTL_MIN_ADMITTED_MINOR_VERSION, 18);
    assert_eq!(KFD_IOCTL_MAX_ADMITTED_MINOR_VERSION, 18);
    assert_eq!(
        KFD_UAPI_SOURCE_HEADER_SHA256,
        "b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d"
    );
    assert_eq!(
        KFD_UAPI_SMI_EVENTS_SOURCE_SHA256,
        "2d786562fe1e97b8257841b755106c8bce47658a2aa3b439ce4e0178323004bd"
    );
    assert_eq!(
        KFD_UAPI_DEVICE_SOURCE_SHA256,
        "ccf20227c5cdd5b258758f50f61bbc1008a09ea776c101f035f83963e7d23037"
    );
    assert_eq!(
        KFD_UAPI_CHARDEV_SOURCE_SHA256,
        "f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba"
    );
    let manifest_digest = Sha256::digest(KFD_UAPI_SCHEMA_MANIFEST);
    assert_eq!(&manifest_digest[..], &KFD_UAPI_SCHEMA_MANIFEST_SHA256_BYTES);
    assert_eq!(
        KFD_UAPI_SCHEMA_MANIFEST_SHA256_BYTES,
        [
            0xe4, 0xaa, 0xd5, 0xd8, 0xe3, 0x17, 0x7e, 0xa6, 0xd7, 0x02, 0x98, 0xad, 0xab, 0x77,
            0x41, 0xc3, 0x77, 0xcb, 0x09, 0x13, 0x73, 0x55, 0x3c, 0xe6, 0x89, 0xf3, 0x52, 0x5e,
            0x75, 0x14, 0xd9, 0xb4,
        ]
    );
    assert_eq!(
        KFD_UAPI_SCHEMA_MANIFEST_SHA256,
        "e4aad5d8e3177ea6d70298adab7741c377cb091373553ce689f3525e7514d9b4"
    );
}

#[test]
fn get_version_layout_matches_kfd_uapi_1_18_golden() {
    assert_eq!(size_of::<KfdIoctlGetVersionArgs>(), 8);
    assert_eq!(align_of::<KfdIoctlGetVersionArgs>(), 4);
    assert_eq!(offset_of!(KfdIoctlGetVersionArgs, major_version), 0);
    assert_eq!(offset_of!(KfdIoctlGetVersionArgs, minor_version), 4);
}

#[test]
fn acquire_vm_layout_matches_kfd_uapi_1_18_golden() {
    assert_eq!(size_of::<KfdIoctlAcquireVmArgs>(), 8);
    assert_eq!(align_of::<KfdIoctlAcquireVmArgs>(), 4);
    assert_eq!(offset_of!(KfdIoctlAcquireVmArgs, drm_fd), 0);
    assert_eq!(offset_of!(KfdIoctlAcquireVmArgs, gpu_id), 4);
}

#[test]
fn process_apertures_layouts_match_kfd_uapi_1_18_golden() {
    assert_eq!(size_of::<KfdProcessDeviceApertures>(), 56);
    assert_eq!(align_of::<KfdProcessDeviceApertures>(), 8);
    assert_eq!(offset_of!(KfdProcessDeviceApertures, lds_base), 0);
    assert_eq!(offset_of!(KfdProcessDeviceApertures, lds_limit), 8);
    assert_eq!(offset_of!(KfdProcessDeviceApertures, scratch_base), 16);
    assert_eq!(offset_of!(KfdProcessDeviceApertures, scratch_limit), 24);
    assert_eq!(offset_of!(KfdProcessDeviceApertures, gpuvm_base), 32);
    assert_eq!(offset_of!(KfdProcessDeviceApertures, gpuvm_limit), 40);
    assert_eq!(offset_of!(KfdProcessDeviceApertures, gpu_id), 48);
    assert_eq!(offset_of!(KfdProcessDeviceApertures, pad), 52);

    assert_eq!(size_of::<KfdIoctlGetProcessAperturesNewArgs>(), 16);
    assert_eq!(align_of::<KfdIoctlGetProcessAperturesNewArgs>(), 8);
    assert_eq!(
        offset_of!(
            KfdIoctlGetProcessAperturesNewArgs,
            kfd_process_device_apertures_ptr
        ),
        0
    );
    assert_eq!(
        offset_of!(KfdIoctlGetProcessAperturesNewArgs, num_of_nodes),
        8
    );
    assert_eq!(offset_of!(KfdIoctlGetProcessAperturesNewArgs, pad), 12);
}

#[test]
fn set_xnack_mode_layout_matches_kfd_uapi_1_18_golden() {
    assert_eq!(size_of::<KfdIoctlSetXnackModeArgs>(), 4);
    assert_eq!(align_of::<KfdIoctlSetXnackModeArgs>(), 4);
    assert_eq!(offset_of!(KfdIoctlSetXnackModeArgs, xnack_enabled), 0);
}

#[test]
fn smi_events_layout_and_reset_mask_match_kfd_uapi_1_18_golden() {
    assert_eq!(size_of::<KfdIoctlSmiEventsArgs>(), 8);
    assert_eq!(align_of::<KfdIoctlSmiEventsArgs>(), 4);
    assert_eq!(offset_of!(KfdIoctlSmiEventsArgs, gpu_id), 0);
    assert_eq!(offset_of!(KfdIoctlSmiEventsArgs, anon_fd), 4);
    assert_eq!(KFD_SMI_EVENT_GPU_PRE_RESET, 3);
    assert_eq!(KFD_SMI_EVENT_GPU_POST_RESET, 4);
    assert_eq!(KFD_SMI_EVENT_GPU_RESET_MASK, 0x0c);
    assert_eq!(KFD_SMI_EVENT_MSG_SIZE, 96);
}

#[test]
fn ioctl_numbers_match_linux_generic_ioc_golden() {
    assert_eq!(AMDKFD_IOC_GET_VERSION, 0x8008_4b01);
    assert_eq!(AMDKFD_IOC_GET_PROCESS_APERTURES_NEW, 0xc010_4b14);
    assert_eq!(AMDKFD_IOC_ACQUIRE_VM, 0x4008_4b15);
    assert_eq!(AMDKFD_IOC_SET_XNACK_MODE, 0xc004_4b21);
    assert_eq!(AMDKFD_IOC_SMI_EVENTS, 0xc008_4b1f);

    assert_eq!(
        encode_ioctl(
            IoctlDirection::ReadWrite,
            AMDKFD_IOCTL_BASE,
            0x14,
            size_of::<KfdIoctlGetProcessAperturesNewArgs>(),
        ),
        Some(AMDKFD_IOC_GET_PROCESS_APERTURES_NEW),
    );
    assert_eq!(
        encode_ioctl(
            IoctlDirection::Read,
            AMDKFD_IOCTL_BASE,
            0x01,
            size_of::<KfdIoctlGetVersionArgs>(),
        ),
        Some(AMDKFD_IOC_GET_VERSION),
    );
    assert_eq!(
        encode_ioctl(
            IoctlDirection::Write,
            AMDKFD_IOCTL_BASE,
            0x15,
            size_of::<KfdIoctlAcquireVmArgs>(),
        ),
        Some(AMDKFD_IOC_ACQUIRE_VM),
    );
    assert_eq!(
        encode_ioctl(
            IoctlDirection::ReadWrite,
            AMDKFD_IOCTL_BASE,
            0x21,
            size_of::<KfdIoctlSetXnackModeArgs>(),
        ),
        Some(AMDKFD_IOC_SET_XNACK_MODE),
    );
    assert_eq!(
        encode_ioctl(
            IoctlDirection::ReadWrite,
            AMDKFD_IOCTL_BASE,
            0x1f,
            size_of::<KfdIoctlSmiEventsArgs>(),
        ),
        Some(AMDKFD_IOC_SMI_EVENTS),
    );
}

#[test]
fn ioctl_encoder_rejects_unrepresentable_payload_size() {
    let first_unrepresentable_size = 1 << 14;
    assert_eq!(
        encode_ioctl(
            IoctlDirection::ReadWrite,
            AMDKFD_IOCTL_BASE,
            0xff,
            first_unrepresentable_size,
        ),
        None,
    );
}

#[test]
fn exact_reviewed_version_produces_admission_evidence() {
    let admitted = negotiate_kfd_uapi_version(KfdUapiVersion::new(1, 18)).unwrap();
    assert_eq!(admitted.reported_version(), KfdUapiVersion::new(1, 18));
    assert_eq!(admitted.schema_id(), KFD_UAPI_SCHEMA_ID);
    assert_eq!(
        admitted.schema_manifest_sha256(),
        KFD_UAPI_SCHEMA_MANIFEST_SHA256
    );
    assert_eq!(admitted.acquire_vm_request(), AMDKFD_IOC_ACQUIRE_VM);
    assert_eq!(
        admitted.get_process_apertures_new_request(),
        AMDKFD_IOC_GET_PROCESS_APERTURES_NEW
    );
    assert_eq!(admitted.set_xnack_mode_request(), AMDKFD_IOC_SET_XNACK_MODE);
    assert_eq!(admitted.smi_events_request(), AMDKFD_IOC_SMI_EVENTS);
}

#[test]
fn version_negotiation_fails_closed() {
    assert_eq!(
        negotiate_kfd_uapi_version(KfdUapiVersion::new(0, 18)),
        Err(KfdUapiVersionError::UnsupportedMajor {
            reported: 0,
            admitted: 1,
        }),
    );
    assert_eq!(
        negotiate_kfd_uapi_version(KfdUapiVersion::new(1, 17)),
        Err(KfdUapiVersionError::MinorTooOld {
            reported: 17,
            minimum: 18,
        }),
    );
    assert_eq!(
        negotiate_kfd_uapi_version(KfdUapiVersion::new(1, 19)),
        Err(KfdUapiVersionError::MinorNewerThanReviewed {
            reported: 19,
            maximum: 18,
        }),
    );
    assert_eq!(
        negotiate_kfd_uapi_version(KfdUapiVersion::new(2, 0)),
        Err(KfdUapiVersionError::UnsupportedMajor {
            reported: 2,
            admitted: 1,
        }),
    );
}

#[test]
fn raw_argument_constructors_preserve_uapi_values() {
    let version = KfdIoctlGetVersionArgs {
        major_version: 1,
        minor_version: 18,
    };
    assert_eq!(version.reported_version(), KfdUapiVersion::new(1, 18));

    let acquire_vm = KfdIoctlAcquireVmArgs::new(27, 9_812);
    assert_eq!(acquire_vm.drm_fd, 27);
    assert_eq!(acquire_vm.gpu_id, 9_812);

    let apertures = KfdIoctlGetProcessAperturesNewArgs::new(0x1234_5000, 16);
    assert_eq!(apertures.kfd_process_device_apertures_ptr, 0x1234_5000);
    assert_eq!(apertures.num_of_nodes, 16);
    assert_eq!(apertures.pad, 0);

    let smi = KfdIoctlSmiEventsArgs::new(9_812);
    assert_eq!(smi.gpu_id, 9_812);
    assert_eq!(smi.anon_fd, u32::MAX);

    assert_eq!(
        KfdIoctlSetXnackModeArgs::query().xnack_enabled,
        KFD_XNACK_MODE_QUERY
    );
    assert_eq!(
        KfdIoctlSetXnackModeArgs::set(false).xnack_enabled,
        KFD_XNACK_MODE_DISABLED
    );
    assert_eq!(
        KfdIoctlSetXnackModeArgs::set(true).xnack_enabled,
        KFD_XNACK_MODE_ENABLED
    );
}
