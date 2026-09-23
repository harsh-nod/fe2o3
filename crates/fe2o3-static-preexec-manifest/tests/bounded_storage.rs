use fe2o3_static_preexec_manifest::{
    DESCRIPTOR_COUNT_OFFSET_V1, DESCRIPTORS_OFFSET_V1, EXECUTABLE_OFFSET_V1, MAGIC_OFFSET_V1,
    MANIFEST_RESERVED_OFFSET_V1, PARENT_PID_OFFSET_V1, PARENT_START_TIME_OFFSET_V1,
    PREEXEC_DESCRIPTOR_BYTES_V1, PREEXEC_MANIFEST_BYTES_V1, PREEXEC_MAX_DESCRIPTORS,
    PREEXEC_SOURCE_FD_BASE, StaticPreexecDescriptorV1 as Descriptor,
    StaticPreexecManifestErrorV1 as Error, StaticPreexecManifestV1 as Manifest,
    StaticPreexecObjectIdentityV1 as Object, VERSION_OFFSET_V1,
};

const OBJECT_CLASS_OFFSET: usize = 28;
const DESCRIPTOR_CLASS_OFFSET: usize = 36;
const DESCRIPTOR_OBJECT_OFFSET: usize = 8;

fn object(index: usize) -> Object {
    Object::new(1, index as u64 + 1, 16, 0o100600)
}

fn descriptors() -> [Descriptor; PREEXEC_MAX_DESCRIPTORS] {
    std::array::from_fn(|index| {
        Descriptor::for_index(index, index as i32, object(index + 1)).unwrap()
    })
}

fn offset(index: usize) -> usize {
    DESCRIPTORS_OFFSET_V1 + index * PREEXEC_DESCRIPTOR_BYTES_V1
}

#[test]
fn slice_and_vector_constructors_preserve_values_wire_clone_and_equality() {
    let descriptors = descriptors();
    let mut previous = None;
    for count in 3..=PREEXEC_MAX_DESCRIPTORS {
        let active = &descriptors[..count];
        let borrowed = Manifest::from_descriptors(2, 1, object(0), active).unwrap();
        let owned = Manifest::new(2, 1, object(0), active.to_vec()).unwrap();
        assert_eq!(borrowed, owned);
        assert_eq!(borrowed.descriptors(), active);
        assert_eq!(borrowed.encode(), owned.encode());
        assert_eq!(borrowed.clone(), borrowed);
        assert_eq!(Manifest::decode(&borrowed.encode()), Ok(borrowed.clone()));
        assert_ne!(previous.as_ref(), Some(&borrowed));
        previous = Some(borrowed);
    }
}

#[test]
fn constructors_preserve_header_error_order_for_empty_and_oversized_slices() {
    let mut backing = [descriptors()[0]; PREEXEC_MAX_DESCRIPTORS + 1];
    backing[..PREEXEC_MAX_DESCRIPTORS].copy_from_slice(&descriptors());
    for count in 0..=backing.len() {
        for parent_pid in [1, 2] {
            for start_time in [0, 1] {
                for executable in [object(0), Object::new_process_pidfd(1, 1, 16, 0o100600)] {
                    let expected = if executable != object(0) {
                        Some(Error::InvalidExecutableObjectClass(1))
                    } else if parent_pid == 1 {
                        Some(Error::InvalidParentPid(1))
                    } else if start_time == 0 {
                        Some(Error::ZeroParentStartTime)
                    } else if !(3..=PREEXEC_MAX_DESCRIPTORS).contains(&count) {
                        Some(Error::InvalidDescriptorCount(count))
                    } else {
                        None
                    };
                    let active = &backing[..count];
                    let borrowed =
                        Manifest::from_descriptors(parent_pid, start_time, executable, active);
                    let owned = Manifest::new(parent_pid, start_time, executable, active.to_vec());
                    assert_eq!(borrowed, owned);
                    assert_eq!(borrowed.err(), expected, "count={count}");
                }
            }
        }
    }
}

#[test]
fn slice_constructor_preserves_descriptor_error_order() {
    let mut entries = descriptors();
    // Source order wins over duplicate destinations, aliases, and missing stderr.
    entries[1] = Descriptor::for_index(2, 0, object(0)).unwrap();
    entries[2] = Descriptor::for_index(2, 9, object(1)).unwrap();
    let cases = [
        (
            entries[1],
            Error::SourceFdOutOfOrder {
                index: 1,
                expected: 201,
                actual: 202,
            },
        ),
        (
            Descriptor::for_index(1, 0, object(0)).unwrap(),
            Error::DuplicateDestinationFd {
                first: 0,
                second: 1,
                destination_fd: 0,
            },
        ),
        (
            Descriptor::for_index(1, 1, object(0)).unwrap(),
            Error::ExecutableDescriptorAlias { descriptor: 1 },
        ),
        (
            Descriptor::for_index(1, 1, object(1)).unwrap(),
            Error::DescriptorObjectAlias {
                first: 0,
                second: 1,
            },
        ),
    ];
    for (replacement, expected) in cases {
        entries[1] = replacement;
        let borrowed = Manifest::from_descriptors(2, 1, object(0), &entries[..3]);
        let owned = Manifest::new(2, 1, object(0), entries[..3].to_vec());
        assert_eq!(borrowed, Err(expected));
        assert_eq!(borrowed, owned);
    }
}

#[test]
fn every_wire_byte_mutation_preserves_exact_diagnostics_or_canonical_value() {
    let entries = descriptors();
    for count in 3..=PREEXEC_MAX_DESCRIPTORS {
        let canonical = Manifest::from_descriptors(2, 1, object(0), &entries[..count])
            .unwrap()
            .encode();
        for changed in 0..PREEXEC_MANIFEST_BYTES_V1 {
            let mut bytes = canonical;
            bytes[changed] ^= 0xa5;
            let u32_at =
                |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let i32_at =
                |offset: usize| i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let expected = match changed {
                0..=7 => Some(Error::InvalidMagic),
                8..=11 => Some(Error::UnsupportedVersion(u32_at(VERSION_OFFSET_V1))),
                12..=15 => Some(Error::InvalidDescriptorCount(
                    u32_at(DESCRIPTOR_COUNT_OFFSET_V1) as usize,
                )),
                16..=19 if i32_at(PARENT_PID_OFFSET_V1) < 2 => {
                    Some(Error::InvalidParentPid(i32_at(PARENT_PID_OFFSET_V1)))
                }
                20..=23 => Some(Error::NonzeroManifestReserved),
                60..=63 => Some(Error::InvalidExecutableObjectClass(u32_at(
                    EXECUTABLE_OFFSET_V1 + OBJECT_CLASS_OFFSET,
                ))),
                DESCRIPTORS_OFFSET_V1.. => {
                    let index = (changed - DESCRIPTORS_OFFSET_V1) / PREEXEC_DESCRIPTOR_BYTES_V1;
                    if index >= count {
                        Some(Error::NonzeroInactiveDescriptor { index })
                    } else {
                        match (changed - DESCRIPTORS_OFFSET_V1) % PREEXEC_DESCRIPTOR_BYTES_V1 {
                            0..=3 => Some(Error::SourceFdOutOfOrder {
                                index,
                                expected: PREEXEC_SOURCE_FD_BASE + index as i32,
                                actual: i32_at(offset(index)),
                            }),
                            4..=7 => Some(Error::InvalidDestinationFd {
                                index,
                                destination_fd: i32_at(offset(index) + 4),
                            }),
                            36..=39 => Some(Error::InvalidDescriptorObjectClass {
                                index,
                                class: u32_at(offset(index) + DESCRIPTOR_CLASS_OFFSET),
                            }),
                            _ => None,
                        }
                    }
                }
                _ => None,
            };
            let decoded = Manifest::decode(&bytes);
            match expected {
                Some(error) => assert_eq!(decoded, Err(error), "count={count} byte={changed}"),
                None => {
                    let value = decoded
                        .unwrap_or_else(|error| panic!("count={count} byte={changed}: {error}"));
                    assert_eq!(value.encode(), bytes);
                    assert_eq!(
                        Manifest::from_descriptors(
                            value.parent_pid(),
                            value.parent_start_time(),
                            *value.executable(),
                            value.descriptors()
                        ),
                        Ok(value)
                    );
                }
            }
        }
    }
}

#[test]
fn simultaneous_wire_faults_keep_structural_then_semantic_error_order() {
    let canonical = Manifest::from_descriptors(2, 1, object(0), &descriptors()[..3])
        .unwrap()
        .encode();
    let faults = [
        (MAGIC_OFFSET_V1, vec![0], Error::InvalidMagic),
        (
            VERSION_OFFSET_V1,
            2_u32.to_le_bytes().to_vec(),
            Error::UnsupportedVersion(2),
        ),
        (
            MANIFEST_RESERVED_OFFSET_V1,
            vec![1],
            Error::NonzeroManifestReserved,
        ),
        (
            DESCRIPTOR_COUNT_OFFSET_V1,
            2_u32.to_le_bytes().to_vec(),
            Error::InvalidDescriptorCount(2),
        ),
        (
            EXECUTABLE_OFFSET_V1 + OBJECT_CLASS_OFFSET,
            vec![1],
            Error::InvalidExecutableObjectClass(1),
        ),
        (
            offset(0) + DESCRIPTOR_CLASS_OFFSET,
            vec![2],
            Error::InvalidDescriptorObjectClass { index: 0, class: 2 },
        ),
        (
            offset(1) + DESCRIPTOR_CLASS_OFFSET,
            vec![3],
            Error::InvalidDescriptorObjectClass { index: 1, class: 3 },
        ),
        (
            offset(3),
            vec![1],
            Error::NonzeroInactiveDescriptor { index: 3 },
        ),
        (
            offset(5) - 1,
            vec![1],
            Error::NonzeroInactiveDescriptor { index: 4 },
        ),
        (
            PARENT_PID_OFFSET_V1,
            1_i32.to_le_bytes().to_vec(),
            Error::InvalidParentPid(1),
        ),
        (
            PARENT_START_TIME_OFFSET_V1,
            0_u64.to_le_bytes().to_vec(),
            Error::ZeroParentStartTime,
        ),
        (
            offset(0),
            201_i32.to_le_bytes().to_vec(),
            Error::SourceFdOutOfOrder {
                index: 0,
                expected: 200,
                actual: 201,
            },
        ),
        (
            offset(0) + 4,
            128_i32.to_le_bytes().to_vec(),
            Error::InvalidDestinationFd {
                index: 0,
                destination_fd: 128,
            },
        ),
        (
            offset(0) + DESCRIPTOR_OBJECT_OFFSET,
            canonical[EXECUTABLE_OFFSET_V1..EXECUTABLE_OFFSET_V1 + 16].to_vec(),
            Error::ExecutableDescriptorAlias { descriptor: 0 },
        ),
        (
            offset(1) + 4,
            0_i32.to_le_bytes().to_vec(),
            Error::DuplicateDestinationFd {
                first: 0,
                second: 1,
                destination_fd: 0,
            },
        ),
        (
            offset(1) + DESCRIPTOR_OBJECT_OFFSET,
            canonical
                [offset(0) + DESCRIPTOR_OBJECT_OFFSET..offset(0) + DESCRIPTOR_OBJECT_OFFSET + 16]
                .to_vec(),
            Error::DescriptorObjectAlias {
                first: 0,
                second: 1,
            },
        ),
        (
            offset(2),
            201_i32.to_le_bytes().to_vec(),
            Error::SourceFdOutOfOrder {
                index: 2,
                expected: 202,
                actual: 201,
            },
        ),
        (
            offset(2) + 4,
            9_i32.to_le_bytes().to_vec(),
            Error::MissingStandardDescriptor(2),
        ),
    ];
    let mut bytes = canonical;
    for (offset, replacement, _) in &faults {
        bytes[*offset..*offset + replacement.len()].copy_from_slice(replacement);
    }
    assert_eq!(
        Manifest::decode(&bytes[..bytes.len() - 1]),
        Err(Error::WrongLength {
            expected: PREEXEC_MANIFEST_BYTES_V1,
            actual: PREEXEC_MANIFEST_BYTES_V1 - 1
        })
    );
    for (offset, replacement, error) in faults {
        assert_eq!(Manifest::decode(&bytes), Err(error));
        bytes[offset..offset + replacement.len()]
            .copy_from_slice(&canonical[offset..offset + replacement.len()]);
    }
    assert_eq!(bytes, canonical);
    assert!(Manifest::decode(&bytes).is_ok());
}

#[test]
fn each_inactive_slot_byte_rejects_for_every_partial_table() {
    let entries = descriptors();
    for count in 3..PREEXEC_MAX_DESCRIPTORS {
        let canonical = Manifest::from_descriptors(2, 1, object(0), &entries[..count])
            .unwrap()
            .encode();
        for changed in offset(count)..PREEXEC_MANIFEST_BYTES_V1 {
            let mut bytes = canonical;
            bytes[changed] = 1;
            assert_eq!(
                Manifest::decode(&bytes),
                Err(Error::NonzeroInactiveDescriptor {
                    index: (changed - DESCRIPTORS_OFFSET_V1) / PREEXEC_DESCRIPTOR_BYTES_V1
                })
            );
        }
    }
}
