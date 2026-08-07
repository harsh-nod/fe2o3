use std::panic::{AssertUnwindSafe, catch_unwind};

use dialect_mir::{
    MAX_MEMORY_OPERATION_WIRE_BYTES, MirAddressSpace, MirCopyNonOverlappingContract,
    MirElementCount, MirLayout, MirMemoryAccessContract, MirMemoryContractDecodeError,
    MirMemoryPermission, MirOperationProvenance, MirOverlapContract, MirPointerDistanceContract,
    MirPointerDistanceResult, MirPointerDistanceUnit, MirPointerOperandContract, MirPointerWidth,
    MirProvenanceRegion, MirSemanticMemoryOperation, MirVolatileAccessContract,
};

fn allocation_pointer(address_space: u32, region: u32) -> MirPointerOperandContract {
    MirPointerOperandContract {
        address_space: MirAddressSpace(address_space),
        provenance: MirOperationProvenance::Allocation(MirProvenanceRegion(region)),
    }
}

fn external_pointer(address_space: u32) -> MirPointerOperandContract {
    MirPointerOperandContract {
        address_space: MirAddressSpace(address_space),
        provenance: MirOperationProvenance::ExposedAddress,
    }
}

fn access(
    pointer: MirPointerOperandContract,
    permission: MirMemoryPermission,
) -> MirMemoryAccessContract {
    MirMemoryAccessContract {
        pointer,
        permission,
    }
}

fn pointer_distance() -> MirSemanticMemoryOperation {
    MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
        pointee_layout: MirLayout::sized(4, 4),
        pointer_width: MirPointerWidth(8),
        unit: MirPointerDistanceUnit::Elements,
        result: MirPointerDistanceResult::Signed,
        pointer: allocation_pointer(1, 0),
        origin: allocation_pointer(1, 0),
    })
}

fn volatile_load() -> MirSemanticMemoryOperation {
    MirSemanticMemoryOperation::VolatileLoad(MirVolatileAccessContract {
        pointee_layout: MirLayout::sized(4, 4),
        pointer_width: MirPointerWidth(8),
        access: access(external_pointer(2), MirMemoryPermission::Read),
    })
}

fn volatile_store() -> MirSemanticMemoryOperation {
    MirSemanticMemoryOperation::VolatileStore(MirVolatileAccessContract {
        pointee_layout: MirLayout::sized(8, 8),
        pointer_width: MirPointerWidth(8),
        access: access(allocation_pointer(1, 0), MirMemoryPermission::Write),
    })
}

fn copy_nonoverlapping() -> MirSemanticMemoryOperation {
    MirSemanticMemoryOperation::CopyNonOverlapping(MirCopyNonOverlappingContract {
        element_layout: MirLayout::sized(4, 4),
        pointer_width: MirPointerWidth(8),
        element_count: MirElementCount::Constant(3),
        source: access(allocation_pointer(1, 0), MirMemoryPermission::Read),
        destination: access(allocation_pointer(3, 1), MirMemoryPermission::Write),
        overlap: MirOverlapContract::NonOverlapping,
    })
}

#[test]
fn pointer_distance_has_exact_canonical_text_and_wire_encoding() {
    let operation = pointer_distance();
    assert_eq!(
        operation.canonical_text().unwrap(),
        "mir.memory.v1:pointer-distance(layout(size=4;align=4);pointer-width=8;unit=elements;result=signed;pointer=ptr(addrspace=1;provenance=allocation(0));origin=ptr(addrspace=1;provenance=allocation(0)))"
    );
    let expected = from_hex(
        "46324d4d454d4f50\
         0100000001\
         04000000000000000400000000000000080101\
         010000000100000000\
         010000000100000000",
    );
    let encoded = operation.to_bytes().unwrap();
    assert_eq!(encoded, expected);
    assert_eq!(
        MirSemanticMemoryOperation::from_bytes(&encoded).unwrap(),
        operation
    );
}

#[test]
fn every_memory_operation_roundtrips_canonically() {
    let operations = [
        pointer_distance(),
        volatile_load(),
        volatile_store(),
        copy_nonoverlapping(),
        MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
            pointee_layout: MirLayout::sized(0, 8),
            pointer_width: MirPointerWidth(8),
            unit: MirPointerDistanceUnit::Bytes,
            result: MirPointerDistanceResult::Unsigned,
            pointer: allocation_pointer(0, 0),
            origin: allocation_pointer(0, 0),
        }),
        MirSemanticMemoryOperation::CopyNonOverlapping(MirCopyNonOverlappingContract {
            element_layout: MirLayout::sized(1, 1),
            pointer_width: MirPointerWidth(4),
            element_count: MirElementCount::Runtime,
            source: access(allocation_pointer(0, 0), MirMemoryPermission::Read),
            destination: access(allocation_pointer(0, 0), MirMemoryPermission::Write),
            overlap: MirOverlapContract::NonOverlapping,
        }),
    ];

    for operation in operations {
        operation.validate().unwrap();
        let encoded = operation.to_bytes().unwrap();
        let decoded = MirSemanticMemoryOperation::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, operation);
        assert_eq!(decoded.to_bytes().unwrap(), encoded);
        assert!(
            decoded
                .canonical_text()
                .unwrap()
                .starts_with("mir.memory.v1:")
        );
    }
}

#[test]
fn pointer_distance_requires_compatible_allocation_provenance() {
    let MirSemanticMemoryOperation::PointerDistance(base) = pointer_distance() else {
        unreachable!()
    };

    let operation = MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
        origin: allocation_pointer(3, 0),
        ..base
    });
    assert!(
        operation
            .validate()
            .unwrap_err()
            .reason()
            .contains("same address space")
    );

    let operation = MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
        origin: allocation_pointer(1, 1),
        ..base
    });
    assert!(
        operation
            .validate()
            .unwrap_err()
            .reason()
            .contains("region must be 0")
    );

    let operation = MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
        pointer: external_pointer(1),
        ..base
    });
    assert!(
        operation
            .validate()
            .unwrap_err()
            .reason()
            .contains("requires allocation provenance")
    );
}

#[test]
fn pointer_distance_distinguishes_element_and_byte_layout_rules() {
    let MirSemanticMemoryOperation::PointerDistance(base) = pointer_distance() else {
        unreachable!()
    };
    let element_zst = MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
        pointee_layout: MirLayout::sized(0, 8),
        ..base
    });
    assert!(
        element_zst
            .validate()
            .unwrap_err()
            .reason()
            .contains("non-zero-sized")
    );

    let byte_zst = MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
        pointee_layout: MirLayout::sized(0, 8),
        unit: MirPointerDistanceUnit::Bytes,
        ..base
    });
    byte_zst.validate().unwrap();

    let dynamically_sized =
        MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
            pointee_layout: MirLayout::dynamically_sized(4),
            ..base
        });
    assert!(
        dynamically_sized
            .validate()
            .unwrap_err()
            .reason()
            .contains("must be sized")
    );
}

#[test]
fn malformed_layouts_and_pointer_widths_fail_closed() {
    let MirSemanticMemoryOperation::PointerDistance(base) = pointer_distance() else {
        unreachable!()
    };
    for layout in [MirLayout::sized(4, 3), MirLayout::sized(6, 4)] {
        let operation = MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
            pointee_layout: layout,
            ..base
        });
        assert!(operation.validate().is_err());
    }

    let operation = MirSemanticMemoryOperation::PointerDistance(MirPointerDistanceContract {
        pointer_width: MirPointerWidth(7),
        ..base
    });
    assert!(
        operation
            .validate()
            .unwrap_err()
            .reason()
            .contains("4, 8, or 16")
    );

    let operation = MirSemanticMemoryOperation::VolatileLoad(MirVolatileAccessContract {
        pointee_layout: MirLayout::sized(1_u64 << 31, 1),
        pointer_width: MirPointerWidth(4),
        access: access(allocation_pointer(0, 0), MirMemoryPermission::Read),
    });
    assert!(
        operation
            .validate()
            .unwrap_err()
            .reason()
            .contains("signed pointer-offset range")
    );
}

#[test]
fn volatile_access_retains_permission_and_provenance_mode() {
    volatile_load().validate().unwrap();
    volatile_store().validate().unwrap();

    let MirSemanticMemoryOperation::VolatileLoad(load) = volatile_load() else {
        unreachable!()
    };
    let wrong_load_permission =
        MirSemanticMemoryOperation::VolatileLoad(MirVolatileAccessContract {
            access: access(load.access.pointer, MirMemoryPermission::Write),
            ..load
        });
    assert!(
        wrong_load_permission
            .validate()
            .unwrap_err()
            .reason()
            .contains("requires read")
    );

    let MirSemanticMemoryOperation::VolatileStore(store) = volatile_store() else {
        unreachable!()
    };
    let wrong_store_permission =
        MirSemanticMemoryOperation::VolatileStore(MirVolatileAccessContract {
            access: access(store.access.pointer, MirMemoryPermission::Read),
            ..store
        });
    assert!(
        wrong_store_permission
            .validate()
            .unwrap_err()
            .reason()
            .contains("requires write")
    );

    let noncanonical_region =
        MirSemanticMemoryOperation::VolatileStore(MirVolatileAccessContract {
            access: access(allocation_pointer(1, 9), MirMemoryPermission::Write),
            ..store
        });
    assert!(
        noncanonical_region
            .validate()
            .unwrap_err()
            .reason()
            .contains("numbered zero")
    );
}

#[test]
fn copy_scales_element_count_and_preserves_distinct_address_spaces() {
    let operation = copy_nonoverlapping();
    let MirSemanticMemoryOperation::CopyNonOverlapping(contract) = operation else {
        unreachable!()
    };
    assert_eq!(contract.constant_byte_count().unwrap(), Some(12));
    assert_eq!(contract.source.pointer.address_space, MirAddressSpace(1));
    assert_eq!(
        contract.destination.pointer.address_space,
        MirAddressSpace(3)
    );
    assert!(
        operation
            .canonical_text()
            .unwrap()
            .contains("count=constant(3)")
    );

    let runtime = MirCopyNonOverlappingContract {
        element_count: MirElementCount::Runtime,
        ..contract
    };
    assert_eq!(runtime.constant_byte_count().unwrap(), None);
    assert_eq!(
        runtime.maximum_element_count().unwrap(),
        i64::MAX as u64 / 4
    );

    let zst = MirCopyNonOverlappingContract {
        element_layout: MirLayout::sized(0, 16),
        element_count: MirElementCount::Constant(u64::MAX),
        ..contract
    };
    assert_eq!(zst.constant_byte_count().unwrap(), Some(0));
    assert_eq!(zst.maximum_element_count().unwrap(), u64::MAX);
}

#[test]
fn copy_rejects_permission_provenance_and_overlap_confusion() {
    let MirSemanticMemoryOperation::CopyNonOverlapping(base) = copy_nonoverlapping() else {
        unreachable!()
    };

    let cases = [
        MirCopyNonOverlappingContract {
            source: access(base.source.pointer, MirMemoryPermission::Write),
            ..base
        },
        MirCopyNonOverlappingContract {
            destination: access(base.destination.pointer, MirMemoryPermission::Read),
            ..base
        },
        MirCopyNonOverlappingContract {
            source: access(external_pointer(1), MirMemoryPermission::Read),
            ..base
        },
        MirCopyNonOverlappingContract {
            source: access(allocation_pointer(1, 1), MirMemoryPermission::Read),
            ..base
        },
        MirCopyNonOverlappingContract {
            destination: access(allocation_pointer(3, 2), MirMemoryPermission::Write),
            ..base
        },
        MirCopyNonOverlappingContract {
            overlap: MirOverlapContract::MayOverlap,
            ..base
        },
    ];

    for contract in cases {
        assert!(
            MirSemanticMemoryOperation::CopyNonOverlapping(contract)
                .validate()
                .is_err()
        );
    }
}

#[test]
fn constant_copy_extent_must_fit_pointer_offset_range() {
    let MirSemanticMemoryOperation::CopyNonOverlapping(base) = copy_nonoverlapping() else {
        unreachable!()
    };
    let operation = MirSemanticMemoryOperation::CopyNonOverlapping(MirCopyNonOverlappingContract {
        element_layout: MirLayout::sized(8, 8),
        element_count: MirElementCount::Constant((i64::MAX as u64 / 8) + 1),
        ..base
    });
    assert!(
        operation
            .validate()
            .unwrap_err()
            .reason()
            .contains("element count times layout size")
    );
}

#[test]
fn decoder_rejects_header_tag_and_trailing_mutations() {
    let encoded = pointer_distance().to_bytes().unwrap();

    let mut bad_magic = encoded.clone();
    bad_magic[0] ^= 1;
    assert_eq!(
        MirSemanticMemoryOperation::from_bytes(&bad_magic),
        Err(MirMemoryContractDecodeError::InvalidMagic)
    );

    let mut bad_version = encoded.clone();
    bad_version[8] = 2;
    assert_eq!(
        MirSemanticMemoryOperation::from_bytes(&bad_version),
        Err(MirMemoryContractDecodeError::UnknownVersion(2))
    );

    let mut bad_flags = encoded.clone();
    bad_flags[10] = 1;
    assert_eq!(
        MirSemanticMemoryOperation::from_bytes(&bad_flags),
        Err(MirMemoryContractDecodeError::UnsupportedFlags(1))
    );

    let mut bad_operation = encoded.clone();
    bad_operation[12] = 99;
    assert_eq!(
        MirSemanticMemoryOperation::from_bytes(&bad_operation),
        Err(MirMemoryContractDecodeError::UnknownTag {
            field: "memory operation",
            tag: 99,
        })
    );

    let mut bad_unit = encoded.clone();
    bad_unit[30] = 99;
    assert_eq!(
        MirSemanticMemoryOperation::from_bytes(&bad_unit),
        Err(MirMemoryContractDecodeError::UnknownTag {
            field: "pointer distance unit",
            tag: 99,
        })
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        MirSemanticMemoryOperation::from_bytes(&trailing),
        Err(MirMemoryContractDecodeError::TrailingBytes)
    );
}

#[test]
fn decoder_is_total_for_truncations_and_single_byte_mutations() {
    let encoded = copy_nonoverlapping().to_bytes().unwrap();
    for end in 0..encoded.len() {
        let result = catch_unwind(|| MirSemanticMemoryOperation::from_bytes(&encoded[..end]));
        assert!(result.is_ok(), "decoder panicked at truncation {end}");
        assert!(
            result.unwrap().is_err(),
            "decoder accepted truncation {end}"
        );
    }

    for index in 0..encoded.len() {
        let mut mutated = encoded.clone();
        mutated[index] ^= 0x5a;
        let result = catch_unwind(AssertUnwindSafe(|| {
            MirSemanticMemoryOperation::from_bytes(&mutated)
        }));
        assert!(result.is_ok(), "decoder panicked at mutation {index}");
        if let Ok(decoded) = result.unwrap() {
            assert_eq!(decoded.to_bytes().unwrap(), mutated);
        }
    }
}

#[test]
fn decoder_bounds_input_before_parsing() {
    let bytes = vec![0; MAX_MEMORY_OPERATION_WIRE_BYTES + 1];
    assert_eq!(
        MirSemanticMemoryOperation::from_bytes(&bytes),
        Err(MirMemoryContractDecodeError::InputTooLarge)
    );
}

fn from_hex(value: &str) -> Vec<u8> {
    let compact: Vec<u8> = value
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    assert!(compact.len().is_multiple_of(2));
    compact
        .chunks_exact(2)
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
        .collect()
}

fn nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("invalid test hex"),
    }
}
