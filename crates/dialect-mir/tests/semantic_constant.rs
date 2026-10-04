use std::panic::{AssertUnwindSafe, catch_unwind};

use dialect_mir::{
    MAX_CONSTANT_GRAPH_DEPTH, MAX_CONSTANT_IDENTITY_BYTES, MirAddressSpace, MirAlignment,
    MirAllocationId, MirAllocationOrigin, MirByteOffset, MirConstantAllocation,
    MirConstantDecodeError, MirConstantIdentity, MirConstantRepresentation, MirInitializedMask,
    MirMemoryIdentity, MirMutability, MirPointerProvenance, MirPointerRelocation, MirPointerWidth,
    MirPromotedIdentity, MirSemanticConstantPool, MirStaticIdentity, MirSymbolIdentity,
};

fn allocation(
    id: u32,
    origin: MirAllocationOrigin,
    representation: MirConstantRepresentation,
    bytes: Vec<u8>,
) -> MirConstantAllocation {
    MirConstantAllocation {
        id: MirAllocationId(id),
        origin,
        representation,
        initialized: MirInitializedMask::all(bytes.len()),
        bytes,
        alignment: MirAlignment(1),
        address_space: MirAddressSpace::DEFAULT,
        mutability: MirMutability::Immutable,
        relocations: Vec::new(),
    }
}

fn memory(id: u32, name: impl Into<String>, size: usize) -> MirConstantAllocation {
    allocation(
        id,
        MirAllocationOrigin::Memory(MirMemoryIdentity(name.into())),
        MirConstantRepresentation::Aggregate,
        vec![0; size],
    )
}

fn relocation(target: u32) -> MirPointerRelocation {
    MirPointerRelocation {
        offset: MirByteOffset(0),
        width: MirPointerWidth(8),
        provenance: MirPointerProvenance::Allocation(MirAllocationId(target)),
        target_offset: MirByteOffset(0),
        address_space: MirAddressSpace::DEFAULT,
        mutability: MirMutability::Immutable,
    }
}

fn minimal_pool() -> MirSemanticConstantPool {
    let mut answer = allocation(
        0,
        MirAllocationOrigin::Constant(MirConstantIdentity("crate::ANSWER".into())),
        MirConstantRepresentation::Scalar,
        42_u32.to_le_bytes().to_vec(),
    );
    answer.alignment = MirAlignment(4);
    MirSemanticConstantPool {
        allocations: vec![answer],
    }
}

fn nested_pool() -> MirSemanticConstantPool {
    let mut root = allocation(
        0,
        MirAllocationOrigin::Constant(MirConstantIdentity("crate::TABLE".into())),
        MirConstantRepresentation::Aggregate,
        vec![0; 16],
    );
    root.alignment = MirAlignment(8);
    root.relocations = vec![
        MirPointerRelocation {
            offset: MirByteOffset(0),
            width: MirPointerWidth(8),
            provenance: MirPointerProvenance::Allocation(MirAllocationId(1)),
            target_offset: MirByteOffset(2),
            address_space: MirAddressSpace::DEFAULT,
            mutability: MirMutability::Immutable,
        },
        MirPointerRelocation {
            offset: MirByteOffset(8),
            width: MirPointerWidth(8),
            provenance: MirPointerProvenance::Static(MirStaticIdentity("crate::COUNT".into())),
            target_offset: MirByteOffset(4),
            address_space: MirAddressSpace(1),
            mutability: MirMutability::Immutable,
        },
    ];

    let promoted = allocation(
        1,
        MirAllocationOrigin::Promoted(MirPromotedIdentity {
            owner: MirConstantIdentity("crate::TABLE".into()),
            index: 0,
        }),
        MirConstantRepresentation::Aggregate,
        b"rust".to_vec(),
    );
    let mut static_value = allocation(
        2,
        MirAllocationOrigin::Static(MirStaticIdentity("crate::COUNT".into())),
        MirConstantRepresentation::Aggregate,
        7_u64.to_le_bytes().to_vec(),
    );
    static_value.alignment = MirAlignment(8);
    static_value.address_space = MirAddressSpace(1);

    MirSemanticConstantPool {
        allocations: vec![root, promoted, static_value],
    }
}

#[test]
fn exact_golden_text_and_wire_roundtrip() {
    let pool = minimal_pool();
    assert_eq!(
        pool.canonical_text().unwrap(),
        "mir.constants.v1{allocations=[allocation(id=0;origin=const(13:crate::ANSWER);repr=scalar;align=4;addrspace=0;mut=const;bytes=2a000000;init=0f;relocs=[])]}"
    );

    let expected = from_hex(
        "46324d434f4e5354\
         0100000001000000\
         00000000010d00000063726174653a3a414e53574552\
         0104000000000000000000000001\
         040000002a000000\
         0400000000000000010000000f\
         00000000",
    );
    let encoded = pool.to_bytes().unwrap();
    assert_eq!(encoded, expected);
    let decoded = MirSemanticConstantPool::from_bytes(&encoded).unwrap();
    assert_eq!(decoded, pool);
    assert_eq!(decoded.to_bytes().unwrap(), encoded);
}

#[test]
fn aggregate_preserves_uninitialized_padding_mask() {
    let mut aggregate = allocation(
        0,
        MirAllocationOrigin::Constant(MirConstantIdentity("crate::PADDED".into())),
        MirConstantRepresentation::Aggregate,
        vec![1, 2, 0, 0, 3, 4, 5, 6],
    );
    aggregate.alignment = MirAlignment(4);
    aggregate.initialized = MirInitializedMask {
        byte_len: 8,
        bits: vec![0b1111_0011],
    };
    let pool = MirSemanticConstantPool {
        allocations: vec![aggregate],
    };

    pool.validate().unwrap();
    assert_eq!(
        MirSemanticConstantPool::from_bytes(&pool.to_bytes().unwrap()).unwrap(),
        pool
    );
    assert_eq!(
        pool.allocations[0]
            .initialized
            .is_initialized(MirByteOffset(2)),
        Some(false)
    );
}

#[test]
fn nested_promoted_and_static_allocations_roundtrip() {
    let pool = nested_pool();
    pool.validate().unwrap();
    let encoded = pool.to_bytes().unwrap();
    assert_eq!(MirSemanticConstantPool::from_bytes(&encoded).unwrap(), pool);
    let text = pool.canonical_text().unwrap();
    assert!(text.contains("promoted(owner=12:crate::TABLE;index=0)"));
    assert!(text.contains("provenance=static(12:crate::COUNT)"));
    assert!(text.contains("target_offset=2"));
}

#[test]
fn supported_relocations_require_initialized_zero_storage() {
    let mut pool = nested_pool();
    pool.allocations[0].initialized.bits[0] &= !1;
    let error = pool.validate().unwrap_err();
    assert!(error.reason().contains("must be initialized"));

    let mut pool = nested_pool();
    pool.allocations[0].bytes[4] = 1;
    let error = pool.validate().unwrap_err();
    assert!(error.reason().contains("must be zero"));
}

#[test]
fn function_vtable_tls_and_unknown_relocations_are_rejected() {
    let cases = [
        MirPointerProvenance::Function(MirSymbolIdentity("device_fn".into())),
        MirPointerProvenance::VTable(MirSymbolIdentity("vtable::Trait".into())),
        MirPointerProvenance::ThreadLocal(MirStaticIdentity("crate::TLS".into())),
        MirPointerProvenance::Unknown(99),
    ];
    for provenance in cases {
        let mut source = memory(0, "source", 8);
        source.relocations.push(MirPointerRelocation {
            provenance,
            ..relocation(1)
        });
        let pool = MirSemanticConstantPool {
            allocations: vec![source, memory(1, "target", 1)],
        };
        let error = pool.validate().unwrap_err();
        assert!(
            error.reason().contains("not supported"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn malformed_pointer_target_fails_closed() {
    let mut source = memory(0, "source", 16);
    source.relocations.push(relocation(9));
    let pool = MirSemanticConstantPool {
        allocations: vec![source],
    };
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("out of range")
    );

    let mut pool = nested_pool();
    pool.allocations[0].relocations[0].target_offset = MirByteOffset(5);
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("target offset exceeds")
    );

    let mut pool = nested_pool();
    pool.allocations[0].relocations[0].address_space = MirAddressSpace(9);
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("address spaces differ")
    );
}

#[test]
fn mutable_pointer_requires_mutable_target() {
    let mut source = memory(0, "source", 8);
    source.relocations.push(MirPointerRelocation {
        mutability: MirMutability::Mutable,
        ..relocation(1)
    });
    let mut pool = MirSemanticConstantPool {
        allocations: vec![source, memory(1, "target", 1)],
    };
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("requires mutable")
    );
    pool.allocations[1].mutability = MirMutability::Mutable;
    pool.validate().unwrap();
}

#[test]
fn one_past_target_pointer_is_described_but_not_authorized() {
    let mut source = memory(0, "source", 8);
    source.relocations.push(MirPointerRelocation {
        target_offset: MirByteOffset(4),
        ..relocation(1)
    });
    let pool = MirSemanticConstantPool {
        allocations: vec![source, memory(1, "target", 4)],
    };
    pool.validate().unwrap();
}

#[test]
fn relocation_order_overlap_and_scalar_coverage_are_canonical() {
    let mut source = memory(0, "source", 24);
    source.relocations = vec![
        MirPointerRelocation {
            offset: MirByteOffset(8),
            ..relocation(1)
        },
        relocation(1),
    ];
    let pool = MirSemanticConstantPool {
        allocations: vec![source, memory(1, "target", 1)],
    };
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("ordered by offset")
    );

    let mut scalar = allocation(
        0,
        MirAllocationOrigin::Constant(MirConstantIdentity("crate::PTR".into())),
        MirConstantRepresentation::Scalar,
        vec![0; 8],
    );
    scalar.relocations.push(MirPointerRelocation {
        width: MirPointerWidth(4),
        ..relocation(1)
    });
    let pool = MirSemanticConstantPool {
        allocations: vec![scalar, memory(1, "target", 1)],
    };
    assert!(pool.validate().unwrap_err().reason().contains("full-width"));
}

#[test]
fn initialized_mask_shape_and_unused_bits_are_canonical() {
    let mut pool = minimal_pool();
    pool.allocations[0].initialized.byte_len = 5;
    assert!(pool.validate().unwrap_err().reason().contains("must equal"));

    let mut pool = minimal_pool();
    pool.allocations[0].initialized.bits.push(0);
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("not canonical")
    );

    let mut aggregate = memory(0, "nine", 9);
    aggregate.initialized.bits[1] = 0b1000_0001;
    let pool = MirSemanticConstantPool {
        allocations: vec![aggregate],
    };
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("unused initialized-mask bits")
    );
}

#[test]
fn scalar_bytes_cannot_be_uninitialized() {
    let mut pool = minimal_pool();
    pool.allocations[0].initialized.bits[0] &= !0b0100;
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("must all be initialized")
    );
}

#[test]
fn allocation_cycles_are_rejected() {
    let mut left = memory(0, "left", 8);
    left.relocations.push(relocation(1));
    let mut right = memory(1, "right", 8);
    right.relocations.push(relocation(0));
    let pool = MirSemanticConstantPool {
        allocations: vec![left, right],
    };
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("contains a cycle")
    );
}

#[test]
fn allocation_graph_depth_is_bounded() {
    let count = MAX_CONSTANT_GRAPH_DEPTH + 1;
    let mut allocations = Vec::with_capacity(count);
    for index in 0..count {
        let mut value = memory(index as u32, format!("node-{index}"), 8);
        if index + 1 < count {
            value.relocations.push(relocation((index + 1) as u32));
        }
        allocations.push(value);
    }
    let pool = MirSemanticConstantPool { allocations };
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("maximum traversal depth")
    );
}

#[test]
fn checked_relocation_arithmetic_rejects_overflow() {
    let mut source = memory(0, "source", 8);
    source.relocations.push(MirPointerRelocation {
        offset: MirByteOffset(u64::MAX - 3),
        ..relocation(1)
    });
    let pool = MirSemanticConstantPool {
        allocations: vec![source, memory(1, "target", 1)],
    };
    assert!(pool.validate().unwrap_err().reason().contains("overflows"));
}

#[test]
fn allocation_and_origin_order_is_canonical() {
    let pool = MirSemanticConstantPool {
        allocations: vec![memory(1, "one", 0), memory(0, "zero", 0)],
    };
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("contiguous and ascending")
    );

    let pool = MirSemanticConstantPool {
        allocations: vec![memory(0, "same", 0), memory(1, "same", 0)],
    };
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("origins must be unique")
    );
}

#[test]
fn identity_and_alignment_resources_are_bounded() {
    let mut pool = minimal_pool();
    pool.allocations[0].origin = MirAllocationOrigin::Constant(MirConstantIdentity(String::new()));
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("must not be empty")
    );

    let mut pool = minimal_pool();
    pool.allocations[0].origin = MirAllocationOrigin::Constant(MirConstantIdentity(
        "x".repeat(MAX_CONSTANT_IDENTITY_BYTES + 1),
    ));
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("byte-length bound")
    );

    let mut pool = minimal_pool();
    pool.allocations[0].alignment = MirAlignment(3);
    assert!(
        pool.validate()
            .unwrap_err()
            .reason()
            .contains("power of two")
    );
}

#[test]
fn decoder_rejects_header_and_tag_mutations() {
    let encoded = minimal_pool().to_bytes().unwrap();

    let mut bad_magic = encoded.clone();
    bad_magic[0] ^= 1;
    assert_eq!(
        MirSemanticConstantPool::from_bytes(&bad_magic),
        Err(MirConstantDecodeError::InvalidMagic)
    );

    let mut bad_version = encoded.clone();
    bad_version[8] = 2;
    assert_eq!(
        MirSemanticConstantPool::from_bytes(&bad_version),
        Err(MirConstantDecodeError::UnknownVersion(2))
    );

    let mut bad_flags = encoded.clone();
    bad_flags[10] = 1;
    assert_eq!(
        MirSemanticConstantPool::from_bytes(&bad_flags),
        Err(MirConstantDecodeError::UnsupportedFlags(1))
    );

    let mut bad_origin = encoded;
    bad_origin[20] = 99;
    assert_eq!(
        MirSemanticConstantPool::from_bytes(&bad_origin),
        Err(MirConstantDecodeError::UnknownTag {
            field: "allocation origin",
            tag: 99,
        })
    );
}

#[test]
fn decoder_rejects_hostile_counts_before_allocation() {
    let mut encoded = minimal_pool().to_bytes().unwrap();
    encoded[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        MirSemanticConstantPool::from_bytes(&encoded),
        Err(MirConstantDecodeError::LimitExceeded("allocation count"))
    );
}

#[test]
fn decoder_is_total_for_truncations_and_single_byte_mutations() {
    let encoded = nested_pool().to_bytes().unwrap();
    for end in 0..encoded.len() {
        let result = catch_unwind(|| MirSemanticConstantPool::from_bytes(&encoded[..end]));
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
            MirSemanticConstantPool::from_bytes(&mutated)
        }));
        assert!(result.is_ok(), "decoder panicked at mutation {index}");
        if let Ok(decoded) = result.unwrap() {
            assert_eq!(decoded.to_bytes().unwrap(), mutated);
        }
    }
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
