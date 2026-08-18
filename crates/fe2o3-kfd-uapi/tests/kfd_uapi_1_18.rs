use core::mem::{align_of, offset_of, size_of};

use fe2o3_kfd_uapi::{
    AMDKFD_IOC_ACQUIRE_VM, AMDKFD_IOC_GET_VERSION, AMDKFD_IOCTL_BASE, IoctlDirection,
    KFD_IOCTL_MAJOR_VERSION, KFD_IOCTL_MAX_ADMITTED_MINOR_VERSION,
    KFD_IOCTL_MIN_ADMITTED_MINOR_VERSION, KFD_IOCTL_MINOR_VERSION, KFD_UAPI_SCHEMA_ID,
    KfdIoctlAcquireVmArgs, KfdIoctlGetVersionArgs, KfdUapiVersion, KfdUapiVersionError,
    encode_ioctl, negotiate_kfd_uapi_version,
};

#[test]
fn schema_identity_is_linux_kfd_1_18() {
    assert_eq!(KFD_UAPI_SCHEMA_ID, "linux-kfd-uapi-1.18-generic-ioc-v1");
    assert_eq!(KFD_IOCTL_MAJOR_VERSION, 1);
    assert_eq!(KFD_IOCTL_MINOR_VERSION, 18);
    assert_eq!(KFD_IOCTL_MIN_ADMITTED_MINOR_VERSION, 18);
    assert_eq!(KFD_IOCTL_MAX_ADMITTED_MINOR_VERSION, 18);
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
fn ioctl_numbers_match_linux_generic_ioc_golden() {
    assert_eq!(AMDKFD_IOC_GET_VERSION, 0x8008_4b01);
    assert_eq!(AMDKFD_IOC_ACQUIRE_VM, 0x4008_4b15);

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
    assert_eq!(admitted.acquire_vm_request(), AMDKFD_IOC_ACQUIRE_VM);
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
}
