use fe2o3_static_preexec_manifest::{
    DESCRIPTOR_COUNT_OFFSET_V1, DESCRIPTORS_OFFSET_V1, EXECUTABLE_OFFSET_V1, MAGIC_OFFSET_V1,
    MANIFEST_RESERVED_OFFSET_V1, PARENT_PID_OFFSET_V1, PARENT_START_TIME_OFFSET_V1,
    PREEXEC_DESCRIPTOR_BYTES_V1, PREEXEC_EXECUTABLE_FD, PREEXEC_MANIFEST_BYTES_V1,
    PREEXEC_MANIFEST_FD, PREEXEC_MANIFEST_MAGIC, PREEXEC_MANIFEST_VERSION, PREEXEC_MAX_DESCRIPTORS,
    PREEXEC_MAX_DESTINATION_FD, PREEXEC_OBJECT_IDENTITY_BYTES_V1, PREEXEC_SOURCE_FD_BASE,
    StaticPreexecDescriptorV1, StaticPreexecManifestErrorV1, StaticPreexecManifestV1,
    StaticPreexecObjectClassV1, StaticPreexecObjectIdentityV1, VERSION_OFFSET_V1,
};

const OBJECT_CLASS_OFFSET: usize = 28;
const DESCRIPTOR_OBJECT_OFFSET: usize = 8;

fn object(seed: u8) -> StaticPreexecObjectIdentityV1 {
    let base = u64::from(seed) << 56;
    StaticPreexecObjectIdentityV1::new(
        base | 0x0001_0203_0405_0607,
        base | 0x0011_1213_1415_1617,
        base | 0x0021_2223_2425_2627,
        u32::from(seed) << 24 | 0x0001_0203,
    )
}

fn descriptor(index: usize, destination_fd: i32, seed: u8) -> StaticPreexecDescriptorV1 {
    StaticPreexecDescriptorV1::for_index(index, destination_fd, object(seed)).unwrap()
}

fn fixture() -> StaticPreexecManifestV1 {
    StaticPreexecManifestV1::new(
        0x1122_3344,
        0x0102_0304_0506_0708,
        StaticPreexecObjectIdentityV1::new(
            0x1112_1314_1516_1718,
            0x2122_2324_2526_2728,
            0x3132_3334_3536_3738,
            0x4142_4344,
        ),
        vec![
            StaticPreexecDescriptorV1::for_index(
                0,
                0,
                StaticPreexecObjectIdentityV1::new(
                    0x5152_5354_5556_5758,
                    0x6162_6364_6566_6768,
                    0x7172_7374_7576_7778,
                    0x8182_8384,
                ),
            )
            .unwrap(),
            StaticPreexecDescriptorV1::for_index(
                1,
                1,
                StaticPreexecObjectIdentityV1::new(
                    0x9192_9394_9596_9798,
                    0xa1a2_a3a4_a5a6_a7a8,
                    0xb1b2_b3b4_b5b6_b7b8,
                    0xc1c2_c3c4,
                ),
            )
            .unwrap(),
            StaticPreexecDescriptorV1::for_index(
                2,
                2,
                StaticPreexecObjectIdentityV1::new(
                    0xd1d2_d3d4_d5d6_d7d8,
                    0xe1e2_e3e4_e5e6_e7e8,
                    0xf1f2_f3f4_f5f6_f7f8,
                    0x0102_0304,
                ),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn constants_and_offsets_match_the_c_v1_abi() {
    assert_eq!(PREEXEC_MANIFEST_FD, 198);
    assert_eq!(PREEXEC_EXECUTABLE_FD, 199);
    assert_eq!(PREEXEC_SOURCE_FD_BASE, 200);
    assert_eq!(PREEXEC_MAX_DESCRIPTORS, 16);
    assert_eq!(PREEXEC_MAX_DESTINATION_FD, 127);
    assert_eq!(PREEXEC_MANIFEST_VERSION, 1);
    assert_eq!(PREEXEC_MANIFEST_MAGIC, *b"FE2PXM1\0");
    assert_eq!(PREEXEC_OBJECT_IDENTITY_BYTES_V1, 32);
    assert_eq!(PREEXEC_DESCRIPTOR_BYTES_V1, 40);
    assert_eq!(PREEXEC_MANIFEST_BYTES_V1, 704);
    assert_eq!(MAGIC_OFFSET_V1, 0);
    assert_eq!(VERSION_OFFSET_V1, 8);
    assert_eq!(DESCRIPTOR_COUNT_OFFSET_V1, 12);
    assert_eq!(PARENT_PID_OFFSET_V1, 16);
    assert_eq!(MANIFEST_RESERVED_OFFSET_V1, 20);
    assert_eq!(PARENT_START_TIME_OFFSET_V1, 24);
    assert_eq!(EXECUTABLE_OFFSET_V1, 32);
    assert_eq!(DESCRIPTORS_OFFSET_V1, 64);
    assert_eq!(
        DESCRIPTORS_OFFSET_V1 + 16 * PREEXEC_DESCRIPTOR_BYTES_V1,
        704
    );
}

#[test]
fn all_sixteen_descriptor_slots_are_usable() {
    let descriptors = (0..PREEXEC_MAX_DESCRIPTORS)
        .map(|index| descriptor(index, index as i32, index as u8 + 2))
        .collect();
    let manifest = StaticPreexecManifestV1::new(2, 1, object(1), descriptors).unwrap();
    assert_eq!(manifest.descriptors().len(), PREEXEC_MAX_DESCRIPTORS);
    assert_eq!(
        StaticPreexecManifestV1::decode(&manifest.encode()).unwrap(),
        manifest
    );
}

#[test]
fn encoder_matches_the_hand_checked_golden_prefix_and_zero_tail() {
    #[rustfmt::skip]
    const GOLDEN_PREFIX: [u8; 184] = [
        0x46,0x45,0x32,0x50,0x58,0x4d,0x31,0x00, 0x01,0x00,0x00,0x00, 0x03,0x00,0x00,0x00,
        0x44,0x33,0x22,0x11, 0x00,0x00,0x00,0x00, 0x08,0x07,0x06,0x05,0x04,0x03,0x02,0x01,
        0x18,0x17,0x16,0x15,0x14,0x13,0x12,0x11, 0x28,0x27,0x26,0x25,0x24,0x23,0x22,0x21,
        0x38,0x37,0x36,0x35,0x34,0x33,0x32,0x31, 0x44,0x43,0x42,0x41, 0x00,0x00,0x00,0x00,
        0xc8,0x00,0x00,0x00, 0x00,0x00,0x00,0x00, 0x58,0x57,0x56,0x55,0x54,0x53,0x52,0x51,
        0x68,0x67,0x66,0x65,0x64,0x63,0x62,0x61, 0x78,0x77,0x76,0x75,0x74,0x73,0x72,0x71,
        0x84,0x83,0x82,0x81, 0x00,0x00,0x00,0x00,
        0xc9,0x00,0x00,0x00, 0x01,0x00,0x00,0x00, 0x98,0x97,0x96,0x95,0x94,0x93,0x92,0x91,
        0xa8,0xa7,0xa6,0xa5,0xa4,0xa3,0xa2,0xa1, 0xb8,0xb7,0xb6,0xb5,0xb4,0xb3,0xb2,0xb1,
        0xc4,0xc3,0xc2,0xc1, 0x00,0x00,0x00,0x00,
        0xca,0x00,0x00,0x00, 0x02,0x00,0x00,0x00, 0xd8,0xd7,0xd6,0xd5,0xd4,0xd3,0xd2,0xd1,
        0xe8,0xe7,0xe6,0xe5,0xe4,0xe3,0xe2,0xe1, 0xf8,0xf7,0xf6,0xf5,0xf4,0xf3,0xf2,0xf1,
        0x04,0x03,0x02,0x01, 0x00,0x00,0x00,0x00,
    ];

    let encoded = fixture().encode();
    assert_eq!(&encoded[..GOLDEN_PREFIX.len()], &GOLDEN_PREFIX);
    assert!(encoded[GOLDEN_PREFIX.len()..].iter().all(|byte| *byte == 0));
    assert_eq!(
        StaticPreexecManifestV1::decode(&encoded).unwrap(),
        fixture()
    );
}

#[test]
fn every_accepted_single_byte_mutation_is_canonical() {
    let canonical = fixture().encode();
    for offset in 0..canonical.len() {
        let mut mutated = canonical;
        mutated[offset] ^= 0xa5;
        if let Ok(decoded) = StaticPreexecManifestV1::decode(&mutated) {
            assert_eq!(
                decoded.encode(),
                mutated,
                "accepted mutation at byte {offset} did not re-encode exactly"
            );
        }
    }
}

#[test]
fn every_magic_reserved_executable_class_and_inactive_byte_mutation_rejects() {
    let canonical = fixture().encode();
    let mut required_rejections = Vec::new();
    required_rejections.extend(MAGIC_OFFSET_V1..VERSION_OFFSET_V1);
    required_rejections.extend(MANIFEST_RESERVED_OFFSET_V1..PARENT_START_TIME_OFFSET_V1);
    required_rejections.extend(
        EXECUTABLE_OFFSET_V1 + OBJECT_CLASS_OFFSET
            ..EXECUTABLE_OFFSET_V1 + PREEXEC_OBJECT_IDENTITY_BYTES_V1,
    );
    required_rejections
        .extend(DESCRIPTORS_OFFSET_V1 + 3 * PREEXEC_DESCRIPTOR_BYTES_V1..PREEXEC_MANIFEST_BYTES_V1);

    for offset in required_rejections {
        let mut mutated = canonical;
        mutated[offset] = 1;
        assert!(
            StaticPreexecManifestV1::decode(&mutated).is_err(),
            "noncanonical byte {offset} was accepted"
        );
    }
}

#[test]
fn descriptor_object_classes_are_strict_and_canonical() {
    let canonical = fixture().encode();
    for index in 0..3 {
        let class_offset = DESCRIPTORS_OFFSET_V1
            + index * PREEXEC_DESCRIPTOR_BYTES_V1
            + DESCRIPTOR_OBJECT_OFFSET
            + OBJECT_CLASS_OFFSET;
        let mut pidfd = canonical;
        pidfd[class_offset..class_offset + 4]
            .copy_from_slice(&(StaticPreexecObjectClassV1::ProcessPidfd as u32).to_le_bytes());
        let decoded = StaticPreexecManifestV1::decode(&pidfd).unwrap();
        assert_eq!(
            decoded.descriptors()[index].object().class(),
            StaticPreexecObjectClassV1::ProcessPidfd
        );
        assert_eq!(decoded.encode(), pidfd);

        let mut unsupported = canonical;
        unsupported[class_offset..class_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            StaticPreexecManifestV1::decode(&unsupported),
            Err(StaticPreexecManifestErrorV1::InvalidDescriptorObjectClass { index, class: 2 })
        );
    }
}

#[test]
fn hostile_header_fields_reject() {
    let canonical = fixture().encode();

    let mut wrong_version = canonical;
    wrong_version[VERSION_OFFSET_V1..VERSION_OFFSET_V1 + 4].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        StaticPreexecManifestV1::decode(&wrong_version),
        Err(StaticPreexecManifestErrorV1::UnsupportedVersion(2))
    );

    for descriptor_count in [0_u32, 1, 2, 17, u32::MAX] {
        let mut invalid_count = canonical;
        invalid_count[DESCRIPTOR_COUNT_OFFSET_V1..DESCRIPTOR_COUNT_OFFSET_V1 + 4]
            .copy_from_slice(&descriptor_count.to_le_bytes());
        assert!(matches!(
            StaticPreexecManifestV1::decode(&invalid_count),
            Err(StaticPreexecManifestErrorV1::InvalidDescriptorCount(actual))
                if actual == descriptor_count as usize
        ));
    }

    for parent_pid in [i32::MIN, -1, 0, 1] {
        let mut invalid_parent = canonical;
        invalid_parent[PARENT_PID_OFFSET_V1..PARENT_PID_OFFSET_V1 + 4]
            .copy_from_slice(&parent_pid.to_le_bytes());
        assert_eq!(
            StaticPreexecManifestV1::decode(&invalid_parent),
            Err(StaticPreexecManifestErrorV1::InvalidParentPid(parent_pid))
        );
    }

    let mut zero_start_time = canonical;
    zero_start_time[PARENT_START_TIME_OFFSET_V1..PARENT_START_TIME_OFFSET_V1 + 8].fill(0);
    assert_eq!(
        StaticPreexecManifestV1::decode(&zero_start_time),
        Err(StaticPreexecManifestErrorV1::ZeroParentStartTime)
    );
}

#[test]
fn every_wrong_length_rejects() {
    let canonical = fixture().encode();
    for length in 0..PREEXEC_MANIFEST_BYTES_V1 {
        assert!(matches!(
            StaticPreexecManifestV1::decode(&canonical[..length]),
            Err(StaticPreexecManifestErrorV1::WrongLength { .. })
        ));
    }
    let mut extended = canonical.to_vec();
    extended.push(0);
    assert!(matches!(
        StaticPreexecManifestV1::decode(&extended),
        Err(StaticPreexecManifestErrorV1::WrongLength { .. })
    ));
}

#[test]
fn parent_and_descriptor_count_bounds_match_the_launcher() {
    for parent_pid in [i32::MIN, -1, 0, 1] {
        let result = StaticPreexecManifestV1::new(
            parent_pid,
            1,
            object(1),
            vec![
                descriptor(0, 0, 2),
                descriptor(1, 1, 3),
                descriptor(2, 2, 4),
            ],
        );
        assert!(matches!(
            result,
            Err(StaticPreexecManifestErrorV1::InvalidParentPid(actual)) if actual == parent_pid
        ));
    }
    assert!(matches!(
        StaticPreexecManifestV1::new(
            2,
            0,
            object(1),
            vec![
                descriptor(0, 0, 2),
                descriptor(1, 1, 3),
                descriptor(2, 2, 4)
            ],
        ),
        Err(StaticPreexecManifestErrorV1::ZeroParentStartTime)
    ));
    for count in 0..3 {
        let descriptors = (0..count)
            .map(|index| descriptor(index, index as i32, index as u8 + 2))
            .collect();
        assert!(matches!(
            StaticPreexecManifestV1::new(2, 1, object(1), descriptors),
            Err(StaticPreexecManifestErrorV1::InvalidDescriptorCount(actual)) if actual == count
        ));
    }
    let too_many = (0..PREEXEC_MAX_DESCRIPTORS)
        .map(|index| descriptor(index, index as i32, index as u8 + 2))
        .chain(std::iter::once(descriptor(0, 17, 99)))
        .collect();
    assert!(matches!(
        StaticPreexecManifestV1::new(2, 1, object(1), too_many),
        Err(StaticPreexecManifestErrorV1::InvalidDescriptorCount(17))
    ));
}

#[test]
fn hostile_descriptor_tables_reject() {
    let canonical = fixture().encode();

    let mut wrong_source = canonical;
    wrong_source[DESCRIPTORS_OFFSET_V1..DESCRIPTORS_OFFSET_V1 + 4]
        .copy_from_slice(&201_i32.to_le_bytes());
    assert!(matches!(
        StaticPreexecManifestV1::decode(&wrong_source),
        Err(StaticPreexecManifestErrorV1::SourceFdOutOfOrder { index: 0, .. })
    ));

    for destination_fd in [-1_i32, 128, i32::MAX] {
        let mut out_of_bounds = canonical;
        out_of_bounds[DESCRIPTORS_OFFSET_V1 + 4..DESCRIPTORS_OFFSET_V1 + 8]
            .copy_from_slice(&destination_fd.to_le_bytes());
        assert!(matches!(
            StaticPreexecManifestV1::decode(&out_of_bounds),
            Err(StaticPreexecManifestErrorV1::InvalidDestinationFd { index: 0, .. })
        ));
    }

    let mut duplicate = canonical;
    let third_destination = DESCRIPTORS_OFFSET_V1 + 2 * PREEXEC_DESCRIPTOR_BYTES_V1 + 4;
    duplicate[third_destination..third_destination + 4].copy_from_slice(&1_i32.to_le_bytes());
    assert!(matches!(
        StaticPreexecManifestV1::decode(&duplicate),
        Err(StaticPreexecManifestErrorV1::DuplicateDestinationFd {
            first: 1,
            second: 2,
            destination_fd: 1,
        })
    ));

    for (descriptor_index, missing_fd) in [(0_usize, 0_i32), (1, 1), (2, 2)] {
        let mut missing_standard = canonical;
        let destination =
            DESCRIPTORS_OFFSET_V1 + descriptor_index * PREEXEC_DESCRIPTOR_BYTES_V1 + 4;
        missing_standard[destination..destination + 4].copy_from_slice(&9_i32.to_le_bytes());
        assert_eq!(
            StaticPreexecManifestV1::decode(&missing_standard),
            Err(StaticPreexecManifestErrorV1::MissingStandardDescriptor(
                missing_fd
            ))
        );
    }
}

#[test]
fn object_aliases_use_only_device_and_inode_like_the_launcher() {
    let executable = object(1);
    let executable_alias = StaticPreexecObjectIdentityV1::new(
        executable.device(),
        executable.inode(),
        executable.size() + 1,
        executable.mode() ^ 0o100,
    );
    assert!(matches!(
        StaticPreexecManifestV1::new(
            2,
            1,
            executable,
            vec![
                StaticPreexecDescriptorV1::for_index(0, 0, executable_alias).unwrap(),
                descriptor(1, 1, 3),
                descriptor(2, 2, 4),
            ],
        ),
        Err(StaticPreexecManifestErrorV1::ExecutableDescriptorAlias { descriptor: 0 })
    ));

    let first = object(2);
    let descriptor_alias = StaticPreexecObjectIdentityV1::new(
        first.device(),
        first.inode(),
        first.size() + 1,
        first.mode() ^ 0o100,
    );
    assert!(matches!(
        StaticPreexecManifestV1::new(
            2,
            1,
            executable,
            vec![
                StaticPreexecDescriptorV1::for_index(0, 0, first).unwrap(),
                StaticPreexecDescriptorV1::for_index(1, 1, descriptor_alias).unwrap(),
                descriptor(2, 2, 4),
            ],
        ),
        Err(StaticPreexecManifestErrorV1::DescriptorObjectAlias {
            first: 0,
            second: 1,
        })
    ));
}

#[test]
fn process_pidfd_snapshots_may_share_the_kernel_anonymous_inode_key() {
    let shared = object(5);
    let first_pidfd = StaticPreexecObjectIdentityV1::new_process_pidfd(
        shared.device(),
        shared.inode(),
        shared.size(),
        shared.mode(),
    );
    let second_pidfd = StaticPreexecObjectIdentityV1::new_process_pidfd(
        shared.device(),
        shared.inode(),
        shared.size(),
        shared.mode(),
    );
    let manifest = StaticPreexecManifestV1::new(
        2,
        1,
        object(1),
        vec![
            descriptor(0, 0, 2),
            descriptor(1, 1, 3),
            descriptor(2, 2, 4),
            StaticPreexecDescriptorV1::for_index(3, 5, first_pidfd).unwrap(),
            StaticPreexecDescriptorV1::for_index(4, 11, second_pidfd).unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(
        StaticPreexecManifestV1::decode(&manifest.encode()).unwrap(),
        manifest
    );
    assert_eq!(
        manifest.descriptors()[3].object().class(),
        StaticPreexecObjectClassV1::ProcessPidfd
    );

    assert_eq!(
        StaticPreexecManifestV1::new(
            2,
            1,
            first_pidfd,
            vec![
                descriptor(0, 0, 2),
                descriptor(1, 1, 3),
                descriptor(2, 2, 4),
            ]
        ),
        Err(StaticPreexecManifestErrorV1::InvalidExecutableObjectClass(
            1
        ))
    );
}

#[test]
fn external_manifest_object_aliases_reject() {
    let manifest = fixture();
    let executable_alias = StaticPreexecObjectIdentityV1::new(
        manifest.executable().device(),
        manifest.executable().inode(),
        0,
        0,
    );
    assert_eq!(
        manifest.validate_manifest_object(&executable_alias),
        Err(StaticPreexecManifestErrorV1::ExecutableManifestAlias)
    );

    let descriptor = manifest.descriptors()[1].object();
    let descriptor_alias =
        StaticPreexecObjectIdentityV1::new(descriptor.device(), descriptor.inode(), 0, 0);
    assert_eq!(
        manifest.validate_manifest_object(&descriptor_alias),
        Err(StaticPreexecManifestErrorV1::DescriptorManifestAlias { descriptor: 1 })
    );
    assert!(manifest.validate_manifest_object(&object(0xff)).is_ok());
}

#[test]
fn descriptor_constructor_enforces_index_and_destination_bounds() {
    assert!(matches!(
        StaticPreexecDescriptorV1::for_index(PREEXEC_MAX_DESCRIPTORS, 0, object(1)),
        Err(StaticPreexecManifestErrorV1::InvalidDescriptorIndex(16))
    ));
    assert!(matches!(
        StaticPreexecDescriptorV1::for_index(0, -1, object(1)),
        Err(StaticPreexecManifestErrorV1::InvalidDestinationFd { .. })
    ));
    let last = StaticPreexecDescriptorV1::for_index(15, 127, object(1)).unwrap();
    assert_eq!(last.source_fd(), 215);
    assert_eq!(last.destination_fd(), 127);
}

#[test]
fn reordered_typed_descriptors_reject() {
    let result = StaticPreexecManifestV1::new(
        2,
        1,
        object(1),
        vec![
            descriptor(1, 0, 2),
            descriptor(0, 1, 3),
            descriptor(2, 2, 4),
        ],
    );
    assert!(matches!(
        result,
        Err(StaticPreexecManifestErrorV1::SourceFdOutOfOrder {
            index: 0,
            expected: 200,
            actual: 201,
        })
    ));
}
