#[path = "../src/device_constants_v2.rs"]
mod device_constants_v2;

use device_constants_v2::*;

fn semantic_type(seed: u8) -> SemanticTypeId {
    SemanticTypeId {
        schema_version: 2,
        domain: [seed.max(1); 16],
        digest: [seed.wrapping_add(1).max(1); 32],
    }
}

fn bytes_region(len: u32) -> Vec<ValidityRegion> {
    if len == 0 {
        Vec::new()
    } else {
        vec![ValidityRegion {
            offset: 0,
            len,
            class: ValidityClass::Bytes,
        }]
    }
}

fn constant(id: u32, bytes: Vec<u8>) -> Allocation {
    Allocation {
        id: AllocationId(id),
        semantic_type: semantic_type((id as u8).wrapping_add(1)),
        kind: AllocationKind::Constant,
        alignment: 4,
        mutability: Mutability::ReadOnly,
        address_space: AddressSpace::Constant,
        validity: bytes_region(u32::try_from(bytes.len()).unwrap()),
        bytes,
        relocations: Vec::new(),
    }
}

fn pointer_source(id: u32, target: u32) -> Allocation {
    Allocation {
        id: AllocationId(id),
        semantic_type: semantic_type((id as u8).wrapping_add(9)),
        kind: AllocationKind::Constant,
        alignment: 8,
        mutability: Mutability::ReadOnly,
        address_space: AddressSpace::Constant,
        bytes: vec![0; 8],
        validity: vec![ValidityRegion {
            offset: 0,
            len: 8,
            class: ValidityClass::Pointer,
        }],
        relocations: vec![Relocation {
            source_offset: 0,
            width: 8,
            target: AllocationId(target),
            addend: 0,
            provenance: ProvenancePolicy::SharedReadOnly,
            capability: CapabilityPolicy::ReadOnly,
        }],
    }
}

fn pointer_source_at(id: u32, target: u32, width: u8, offset: u32, alignment: u32) -> Allocation {
    let byte_len = offset.checked_add(u32::from(width)).unwrap();
    let mut validity = Vec::with_capacity(2);
    if offset != 0 {
        validity.push(ValidityRegion {
            offset: 0,
            len: offset,
            class: ValidityClass::Bytes,
        });
    }
    validity.push(ValidityRegion {
        offset,
        len: u32::from(width),
        class: ValidityClass::Pointer,
    });
    Allocation {
        id: AllocationId(id),
        semantic_type: semantic_type((id as u8).wrapping_add(9)),
        kind: AllocationKind::Constant,
        alignment,
        mutability: Mutability::ReadOnly,
        address_space: AddressSpace::Constant,
        bytes: vec![0; usize::try_from(byte_len).unwrap()],
        validity,
        relocations: vec![Relocation {
            source_offset: offset,
            width,
            target: AllocationId(target),
            addend: 0,
            provenance: ProvenancePolicy::SharedReadOnly,
            capability: CapabilityPolicy::ReadOnly,
        }],
    }
}

fn mixed_width_pointer_source(alignment: u32) -> Allocation {
    Allocation {
        id: AllocationId(0),
        semantic_type: semantic_type(17),
        kind: AllocationKind::Constant,
        alignment,
        mutability: Mutability::ReadOnly,
        address_space: AddressSpace::Constant,
        bytes: vec![0; 16],
        validity: vec![
            ValidityRegion {
                offset: 0,
                len: 4,
                class: ValidityClass::Pointer,
            },
            ValidityRegion {
                offset: 4,
                len: 4,
                class: ValidityClass::PaddingZero,
            },
            ValidityRegion {
                offset: 8,
                len: 8,
                class: ValidityClass::Pointer,
            },
        ],
        relocations: vec![
            Relocation {
                source_offset: 0,
                width: 4,
                target: AllocationId(1),
                addend: 0,
                provenance: ProvenancePolicy::SharedReadOnly,
                capability: CapabilityPolicy::ReadOnly,
            },
            Relocation {
                source_offset: 8,
                width: 8,
                target: AllocationId(2),
                addend: 0,
                provenance: ProvenancePolicy::SharedReadOnly,
                capability: CapabilityPolicy::ReadOnly,
            },
        ],
    }
}

fn two_narrow_pointer_source(alignment: u32) -> Allocation {
    Allocation {
        id: AllocationId(0),
        semantic_type: semantic_type(18),
        kind: AllocationKind::Constant,
        alignment,
        mutability: Mutability::ReadOnly,
        address_space: AddressSpace::Constant,
        bytes: vec![0; 8],
        validity: vec![
            ValidityRegion {
                offset: 0,
                len: 4,
                class: ValidityClass::Pointer,
            },
            ValidityRegion {
                offset: 4,
                len: 4,
                class: ValidityClass::Pointer,
            },
        ],
        relocations: vec![
            Relocation {
                source_offset: 0,
                width: 4,
                target: AllocationId(1),
                addend: 0,
                provenance: ProvenancePolicy::SharedReadOnly,
                capability: CapabilityPolicy::ReadOnly,
            },
            Relocation {
                source_offset: 4,
                width: 4,
                target: AllocationId(2),
                addend: 0,
                provenance: ProvenancePolicy::SharedReadOnly,
                capability: CapabilityPolicy::ReadOnly,
            },
        ],
    }
}

fn graph(allocations: Vec<Allocation>) -> DeviceConstantGraphV2 {
    DeviceConstantGraphV2 { allocations }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

#[test]
fn canonical_graph_validates_and_round_trips() {
    let graph = graph(vec![pointer_source(0, 1), constant(1, vec![1, 2, 3, 4])]);
    let limits = GraphLimits::default();
    graph.validate(&limits).unwrap();
    let first = graph.encode_canonical(&limits).unwrap();
    let second = graph.encode_canonical(&limits).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        DeviceConstantGraphV2::decode_canonical(&first, &limits).unwrap(),
        graph
    );
}

#[test]
fn allocation_identity_and_policy_are_canonical() {
    let limits = GraphLimits::default();

    let mut bad_id = constant(1, vec![1]);
    assert!(matches!(
        graph(vec![bad_id.clone()]).validate(&limits),
        Err(ValidationError::NonCanonicalAllocationId { .. })
    ));

    bad_id.id = AllocationId(0);
    bad_id.semantic_type.domain = [0; 16];
    assert_eq!(
        graph(vec![bad_id.clone()]).validate(&limits),
        Err(ValidationError::InvalidSemanticTypeId(AllocationId(0)))
    );

    bad_id.semantic_type = semantic_type(1);
    bad_id.kind = AllocationKind::Constant;
    bad_id.mutability = Mutability::Mutable;
    bad_id.address_space = AddressSpace::Global;
    assert_eq!(
        graph(vec![bad_id.clone()]).validate(&limits),
        Err(ValidationError::InvalidAllocationPolicy(AllocationId(0)))
    );

    bad_id.kind = AllocationKind::Static;
    bad_id.alignment = 3;
    assert_eq!(
        graph(vec![bad_id]).validate(&limits),
        Err(ValidationError::InvalidAlignment(AllocationId(0)))
    );
}

#[test]
fn validity_regions_reject_gaps_overlap_padding_and_bad_bits() {
    let limits = GraphLimits::default();
    let mut allocation = constant(0, vec![0, 0, 0, 0]);

    allocation.validity = vec![ValidityRegion {
        offset: 1,
        len: 3,
        class: ValidityClass::Bytes,
    }];
    assert_eq!(
        graph(vec![allocation.clone()]).validate(&limits),
        Err(ValidationError::ValidityCoverageGap(AllocationId(0)))
    );

    allocation.validity = vec![
        ValidityRegion {
            offset: 0,
            len: 3,
            class: ValidityClass::Bytes,
        },
        ValidityRegion {
            offset: 2,
            len: 2,
            class: ValidityClass::Bytes,
        },
    ];
    assert_eq!(
        graph(vec![allocation.clone()]).validate(&limits),
        Err(ValidationError::ValidityCoverageOverlap(AllocationId(0)))
    );

    allocation.bytes = vec![1];
    allocation.validity = vec![ValidityRegion {
        offset: 0,
        len: 1,
        class: ValidityClass::PaddingZero,
    }];
    assert_eq!(
        graph(vec![allocation.clone()]).validate(&limits),
        Err(ValidationError::InvalidPadding(AllocationId(0)))
    );

    allocation.bytes = vec![2];
    allocation.validity[0].class = ValidityClass::Bool;
    assert_eq!(
        graph(vec![allocation.clone()]).validate(&limits),
        Err(ValidationError::InvalidBitPattern(AllocationId(0)))
    );

    allocation.bytes = vec![0, 0];
    allocation.validity[0] = ValidityRegion {
        offset: 0,
        len: 2,
        class: ValidityClass::NonZero,
    };
    assert_eq!(
        graph(vec![allocation]).validate(&limits),
        Err(ValidationError::InvalidBitPattern(AllocationId(0)))
    );
}

#[test]
fn pointers_exist_only_as_relocations() {
    let limits = GraphLimits::default();

    let mut forged = pointer_source(0, 1);
    forged.bytes[0] = 1;
    assert_eq!(
        graph(vec![forged, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::IntegerForgedPointer(AllocationId(0)))
    );

    let mut missing = pointer_source(0, 1);
    missing.relocations.clear();
    assert_eq!(
        graph(vec![missing, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::PointerRegionWithoutRelocation(
            AllocationId(0)
        ))
    );

    let mut untyped = pointer_source(0, 1);
    untyped.validity[0].class = ValidityClass::Bytes;
    assert_eq!(
        graph(vec![untyped, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::RelocationWithoutPointerRegion(
            AllocationId(0)
        ))
    );

    let mut out_of_bounds = pointer_source(0, 1);
    out_of_bounds.relocations[0].source_offset = 8;
    assert_eq!(
        graph(vec![out_of_bounds, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::RelocationOutOfBounds(AllocationId(0)))
    );
}

#[test]
fn targets_and_capabilities_are_checked() {
    let limits = GraphLimits::default();

    assert!(matches!(
        graph(vec![pointer_source(0, 9)]).validate(&limits),
        Err(ValidationError::UnknownRelocationTarget { .. })
    ));

    let mut bad_addend = pointer_source(0, 1);
    bad_addend.relocations[0].addend = 4;
    assert!(matches!(
        graph(vec![bad_addend, constant(1, vec![1, 2, 3, 4])]).validate(&limits),
        Err(ValidationError::TargetAddendOutOfBounds { .. })
    ));

    let mut mutable_target = constant(1, vec![1]);
    mutable_target.kind = AllocationKind::Static;
    mutable_target.mutability = Mutability::Mutable;
    mutable_target.address_space = AddressSpace::Global;
    let source = pointer_source(0, 1);
    assert!(matches!(
        graph(vec![source, mutable_target]).validate(&limits),
        Err(ValidationError::CapabilityMismatch { .. })
    ));
}

#[test]
fn mutable_and_global_targets_have_unique_ingress() {
    let limits = GraphLimits::default();
    let mut first = pointer_source(0, 2);
    let mut second = pointer_source(1, 2);
    for source in [&mut first, &mut second] {
        source.relocations[0].provenance = ProvenancePolicy::Unique;
        source.relocations[0].capability = CapabilityPolicy::ReadWrite;
    }
    let mut target = constant(2, vec![1]);
    target.kind = AllocationKind::Static;
    target.mutability = Mutability::Mutable;
    target.address_space = AddressSpace::Global;
    assert_eq!(
        graph(vec![first, second, target]).validate(&limits),
        Err(ValidationError::AmbiguousMutableOrGlobalAlias(
            AllocationId(2)
        ))
    );
}

#[test]
fn cycles_are_rejected_until_an_ownership_model_exists() {
    let limits = GraphLimits::default();
    let first = pointer_source(0, 1);
    let second = pointer_source(1, 0);
    assert_eq!(
        graph(vec![first, second]).validate(&limits),
        Err(ValidationError::UnsupportedRelocationCycle)
    );
}

#[test]
fn limits_are_checked_before_codec_allocation() {
    let graph = graph(vec![constant(0, vec![1, 2, 3, 4])]);
    let limits = GraphLimits {
        max_total_allocation_bytes: 3,
        ..GraphLimits::default()
    };
    assert!(matches!(
        graph.validate(&limits),
        Err(ValidationError::ResourceLimit {
            resource: Resource::AllocationBytes,
            observed: 4,
            limit: 3,
        })
    ));

    let encoded = graph.encode_canonical(&GraphLimits::default()).unwrap();
    let limits = GraphLimits {
        max_encoded_bytes: u64::try_from(encoded.len() - 1).unwrap(),
        ..GraphLimits::default()
    };
    assert!(matches!(
        DeviceConstantGraphV2::decode_canonical(&encoded, &limits),
        Err(DecodeError::ResourceLimit {
            resource: Resource::EncodedBytes,
            ..
        })
    ));
}

#[test]
fn codec_v2_relocation_golden_is_stable() {
    let graph = graph(vec![pointer_source(0, 1), constant(1, vec![1, 2, 3, 4])]);
    let encoded = graph.encode_canonical(&GraphLimits::default()).unwrap();
    assert_eq!(
        hex(&encoded),
        "463243320200000002000000cc000000000000000000000002000000000008000000080000000100000001000000090909090909090909090909090909090a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a00000000000000000000000008000000040000000000000008000000010000000000000000000000010000000200000000000400000004000000010000000000000002020202020202020202020202020202030303030303030303030303030303030303030303030303030303030303030301020304000000000400000000000000"
    );
    assert_eq!(
        DeviceConstantGraphV2::decode_canonical(&encoded, &GraphLimits::default()).unwrap(),
        graph
    );
}

#[test]
fn decoder_preflights_hostile_counts_before_reserving() {
    let limits = GraphLimits {
        max_allocations: u32::MAX,
        max_relocations: u32::MAX,
        max_validity_regions: u32::MAX,
        ..GraphLimits::default()
    };
    let empty = graph(Vec::new())
        .encode_canonical(&GraphLimits::default())
        .unwrap();
    let mut huge_allocation_count = empty;
    huge_allocation_count[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        DeviceConstantGraphV2::decode_canonical(&huge_allocation_count, &limits),
        Err(DecodeError::Truncated)
    );

    let mut huge_region_count = graph(vec![constant(0, vec![1])])
        .encode_canonical(&GraphLimits::default())
        .unwrap();
    huge_region_count[38..42].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        DeviceConstantGraphV2::decode_canonical(&huge_region_count, &limits),
        Err(DecodeError::Truncated)
    );
}

#[test]
fn decoder_rejects_structural_corruption_and_truncation() {
    let limits = GraphLimits::default();
    let encoded = graph(vec![pointer_source(0, 1), constant(1, vec![1, 2, 3, 4])])
        .encode_canonical(&limits)
        .unwrap();

    for cut in 0..encoded.len() {
        assert!(DeviceConstantGraphV2::decode_canonical(&encoded[..cut], &limits).is_err());
    }

    for offset in [0_usize, 4, 6, 8, 12, 26, 27, 28, 29, 102, 103, 104, 105] {
        let mut corrupt = encoded.clone();
        corrupt[offset] ^= 0xff;
        assert!(DeviceConstantGraphV2::decode_canonical(&corrupt, &limits).is_err());
    }

    let mut appended = encoded;
    appended.push(0);
    assert_eq!(
        DeviceConstantGraphV2::decode_canonical(&appended, &limits),
        Err(DecodeError::LengthMismatch)
    );
}

#[test]
fn hostile_decoder_mutation_campaign_remains_canonical_and_bounded() {
    let limits = GraphLimits {
        max_encoded_bytes: 4_096,
        max_total_allocation_bytes: 1_024,
        ..GraphLimits::default()
    };
    let encoded = graph(vec![pointer_source(0, 1), constant(1, vec![1, 2, 3, 4])])
        .encode_canonical(&limits)
        .unwrap();
    let mut state = 0x5a07_c0de_d15c_a11c_u64;

    for _ in 0..100_000 {
        let mut mutated = encoded.clone();
        let mutations = usize::try_from(next_random(&mut state) % 4 + 1).unwrap();
        for _ in 0..mutations {
            let offset = usize::try_from(next_random(&mut state)).unwrap() % mutated.len();
            let bit = u8::try_from(next_random(&mut state) % 8).unwrap();
            mutated[offset] ^= 1_u8 << bit;
        }
        if let Ok(decoded) = DeviceConstantGraphV2::decode_canonical(&mutated, &limits) {
            assert_eq!(decoded.encode_canonical(&limits).unwrap(), mutated);
        }
    }

    for _ in 0..60_000 {
        let len = usize::try_from(next_random(&mut state) % 513).unwrap();
        let mut hostile = vec![0_u8; len];
        for byte in &mut hostile {
            *byte = u8::try_from(next_random(&mut state) >> 56).unwrap();
        }
        if let Ok(decoded) = DeviceConstantGraphV2::decode_canonical(&hostile, &limits) {
            assert_eq!(decoded.encode_canonical(&limits).unwrap(), hostile);
        }
    }
}

fn graph_oracle_case(seed: u64) -> (DeviceConstantGraphV2, GraphLimits, bool) {
    let mut candidate = graph(vec![
        pointer_source(0, 2),
        constant(1, vec![u8::try_from(seed & 0xff).unwrap()]),
        constant(2, vec![1, 2, 3, 4]),
    ]);
    let mut limits = GraphLimits::default();
    let case = seed % 16;
    let expected = match case {
        0 | 15 => true,
        1 => {
            candidate.allocations[0].bytes[0] = 1;
            false
        }
        2 => {
            candidate.allocations[0].relocations[0].target = AllocationId(99);
            false
        }
        3 => {
            candidate.allocations[0].relocations[0].addend = -1;
            false
        }
        4 => {
            candidate.allocations[0].relocations[0].capability = CapabilityPolicy::ReadWrite;
            false
        }
        5 => {
            candidate.allocations[1].id = AllocationId(9);
            false
        }
        6 => {
            candidate.allocations[2].semantic_type.digest = [0; 32];
            false
        }
        7 => {
            candidate.allocations[2].alignment = 3;
            false
        }
        8 => {
            candidate.allocations[2].bytes[0] = 2;
            candidate.allocations[2].validity = vec![
                ValidityRegion {
                    offset: 0,
                    len: 1,
                    class: ValidityClass::Bool,
                },
                ValidityRegion {
                    offset: 1,
                    len: 3,
                    class: ValidityClass::Bytes,
                },
            ];
            false
        }
        9 => {
            candidate.allocations[0].relocations.clear();
            false
        }
        10 => {
            candidate.allocations[0].relocations[0].target = AllocationId(0);
            false
        }
        11 => {
            limits.max_total_allocation_bytes = 12;
            false
        }
        12 => {
            candidate.allocations[2].validity[0].offset = 1;
            false
        }
        13 => {
            candidate.allocations[0].bytes = vec![0; 16];
            candidate.allocations[0].validity = vec![
                ValidityRegion {
                    offset: 0,
                    len: 8,
                    class: ValidityClass::Pointer,
                },
                ValidityRegion {
                    offset: 8,
                    len: 8,
                    class: ValidityClass::Pointer,
                },
            ];
            let first = candidate.allocations[0].relocations[0];
            candidate.allocations[0].relocations = vec![
                Relocation {
                    source_offset: 8,
                    ..first
                },
                first,
            ];
            false
        }
        14 => {
            candidate.allocations[0].bytes = vec![0; 16];
            candidate.allocations[0].validity = vec![
                ValidityRegion {
                    offset: 0,
                    len: 8,
                    class: ValidityClass::Pointer,
                },
                ValidityRegion {
                    offset: 8,
                    len: 8,
                    class: ValidityClass::Pointer,
                },
            ];
            let first = candidate.allocations[0].relocations[0];
            candidate.allocations[0].relocations = vec![
                first,
                Relocation {
                    source_offset: 4,
                    ..first
                },
            ];
            false
        }
        _ => unreachable!(),
    };
    (candidate, limits, expected)
}

#[test]
fn fifty_thousand_case_graph_oracle_matches_validator() {
    for seed in 0..50_000_u64 {
        let (candidate, limits, expected) = graph_oracle_case(seed);
        let actual = candidate.validate(&limits).is_ok();
        assert_eq!(actual, expected, "oracle mismatch for seed {seed}");
        if actual && seed.is_multiple_of(997) {
            let encoded = candidate.encode_canonical(&limits).unwrap();
            assert_eq!(
                DeviceConstantGraphV2::decode_canonical(&encoded, &limits).unwrap(),
                candidate
            );
        }
    }
}

fn relocation_chain(count: u32) -> DeviceConstantGraphV2 {
    assert!(count > 0);
    let mut allocations = Vec::with_capacity(usize::try_from(count).unwrap());
    for id in 0..count - 1 {
        allocations.push(pointer_source(id, id + 1));
    }
    allocations.push(constant(count - 1, vec![1]));
    graph(allocations)
}

#[test]
fn graph_resource_and_depth_boundaries_are_exact() {
    let chain = relocation_chain(512);
    let exact = GraphLimits {
        max_allocations: 512,
        max_relocations: 511,
        max_validity_regions: 512,
        max_total_allocation_bytes: 4_089,
        max_relocation_depth: 511,
        ..GraphLimits::default()
    };
    chain.validate(&exact).unwrap();

    let too_shallow = GraphLimits {
        max_relocation_depth: 510,
        ..exact
    };
    assert!(matches!(
        chain.validate(&too_shallow),
        Err(ValidationError::ResourceLimit {
            resource: Resource::RelocationDepth,
            observed: 511,
            limit: 510,
        })
    ));

    let too_few_allocations = GraphLimits {
        max_allocations: 511,
        ..exact
    };
    assert!(matches!(
        chain.validate(&too_few_allocations),
        Err(ValidationError::ResourceLimit {
            resource: Resource::Allocations,
            ..
        })
    ));
}

#[test]
fn limitations_do_not_claim_unimplemented_trust_boundaries() {
    for required in [
        "untrusted-type-commitments",
        "no relocation cycles",
        "integer-derived pointers",
        "unique ingress",
        "no export",
        "formal-verification claim",
    ] {
        assert!(DEVICE_CONSTANTS_V2_LIMITATIONS.contains(required));
    }
}

#[test]
fn allocation_policy_matrix_is_conservative() {
    let limits = GraphLimits::default();
    let valid_policies = [
        (
            AllocationKind::Constant,
            Mutability::ReadOnly,
            AddressSpace::Constant,
        ),
        (
            AllocationKind::Static,
            Mutability::ReadOnly,
            AddressSpace::Constant,
        ),
        (
            AllocationKind::Static,
            Mutability::ReadOnly,
            AddressSpace::Global,
        ),
        (
            AllocationKind::Static,
            Mutability::Mutable,
            AddressSpace::Global,
        ),
    ];
    for (kind, mutability, address_space) in valid_policies {
        let mut allocation = constant(0, vec![1]);
        allocation.kind = kind;
        allocation.mutability = mutability;
        allocation.address_space = address_space;
        graph(vec![allocation]).validate(&limits).unwrap();
    }

    for (kind, mutability, address_space) in [
        (
            AllocationKind::Constant,
            Mutability::ReadOnly,
            AddressSpace::Global,
        ),
        (
            AllocationKind::Constant,
            Mutability::Mutable,
            AddressSpace::Constant,
        ),
        (
            AllocationKind::Constant,
            Mutability::Mutable,
            AddressSpace::Global,
        ),
        (
            AllocationKind::Static,
            Mutability::Mutable,
            AddressSpace::Constant,
        ),
    ] {
        let mut allocation = constant(0, vec![1]);
        allocation.kind = kind;
        allocation.mutability = mutability;
        allocation.address_space = address_space;
        assert_eq!(
            graph(vec![allocation]).validate(&limits),
            Err(ValidationError::InvalidAllocationPolicy(AllocationId(0)))
        );
    }
}

#[test]
fn relocation_width_alignment_and_policy_fields_are_bound() {
    let limits = GraphLimits::default();

    let mut bad_width = pointer_source(0, 1);
    bad_width.relocations[0].width = 3;
    assert_eq!(
        graph(vec![bad_width, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::InvalidRelocationWidth(AllocationId(0)))
    );

    let mut unaligned = pointer_source(0, 1);
    unaligned.bytes = vec![9, 9, 0, 0, 0, 0, 9, 9];
    unaligned.validity = vec![
        ValidityRegion {
            offset: 0,
            len: 2,
            class: ValidityClass::Bytes,
        },
        ValidityRegion {
            offset: 2,
            len: 4,
            class: ValidityClass::Pointer,
        },
        ValidityRegion {
            offset: 6,
            len: 2,
            class: ValidityClass::Bytes,
        },
    ];
    unaligned.relocations[0].source_offset = 2;
    unaligned.relocations[0].width = 4;
    assert_eq!(
        graph(vec![unaligned, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::UnalignedRelocation(AllocationId(0)))
    );

    let mut readonly_global = constant(1, vec![1]);
    readonly_global.kind = AllocationKind::Static;
    readonly_global.address_space = AddressSpace::Global;
    let shared_source = pointer_source(0, 1);
    assert!(matches!(
        graph(vec![shared_source, readonly_global.clone()]).validate(&limits),
        Err(ValidationError::CapabilityMismatch { .. })
    ));

    let mut unique_source = pointer_source(0, 1);
    unique_source.relocations[0].provenance = ProvenancePolicy::Unique;
    graph(vec![unique_source, readonly_global])
        .validate(&limits)
        .unwrap();
}

#[test]
fn relocation_width_requires_matching_allocation_base_alignment() {
    let limits = GraphLimits::default();
    let under_aligned = pointer_source_at(0, 1, 8, 0, 4);
    assert_eq!(
        graph(vec![under_aligned, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::UnalignedRelocation(AllocationId(0)))
    );

    let exactly_aligned = pointer_source_at(0, 1, 8, 0, 8);
    graph(vec![exactly_aligned, constant(1, vec![1])])
        .validate(&limits)
        .unwrap();
}

#[test]
fn multi_relocation_alignment_uses_the_widest_record() {
    let limits = GraphLimits::default();
    let targets = || vec![constant(1, vec![1]), constant(2, vec![2])];

    let mut mixed = vec![mixed_width_pointer_source(8)];
    mixed.extend(targets());
    graph(mixed).validate(&limits).unwrap();

    let mut under_aligned_mixed = vec![mixed_width_pointer_source(4)];
    under_aligned_mixed.extend(targets());
    assert_eq!(
        graph(under_aligned_mixed).validate(&limits),
        Err(ValidationError::UnalignedRelocation(AllocationId(0)))
    );

    let mut narrow = vec![two_narrow_pointer_source(4)];
    narrow.extend(targets());
    graph(narrow).validate(&limits).unwrap();
}

#[test]
fn exhaustive_alignment_width_and_offset_congruence_matrix() {
    let limits = GraphLimits::default();
    let mut checked = 0_u64;

    // Offsets 0..16 cover two periods of every supported pointer width.
    for alignment in 1..=limits.max_alignment {
        for width in [4_u8, 8] {
            for offset in 0..16_u32 {
                let source = pointer_source_at(0, 1, width, offset, alignment);
                let candidate = graph(vec![source, constant(1, vec![1])]);
                let expected = alignment.is_power_of_two()
                    && alignment >= u32::from(width)
                    && offset.is_multiple_of(u32::from(width));
                assert_eq!(
                    candidate.validate(&limits).is_ok(),
                    expected,
                    "alignment={alignment} width={width} offset={offset}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 2_097_152);
}

#[test]
fn sixty_five_thousand_case_independent_alignment_oracle() {
    let limits = GraphLimits::default();
    let mut state = 0xa11c_07ba_5e00_0001_u64;

    for case in 0..65_536_u64 {
        let alignment =
            u32::try_from(next_random(&mut state) % u64::from(limits.max_alignment) + 1).unwrap();
        let width = if next_random(&mut state) & 1 == 0 {
            4_u8
        } else {
            8_u8
        };
        let offset = u32::try_from(next_random(&mut state) % 64).unwrap();
        let source = pointer_source_at(0, 1, width, offset, alignment);
        let candidate = graph(vec![source, constant(1, vec![1])]);

        let allocation_alignment_is_canonical = alignment.is_power_of_two();
        let base_covers_width = u64::from(alignment) >= u64::from(width);
        let offset_is_aligned = u64::from(offset) % u64::from(width) == 0;
        let oracle_accepts =
            allocation_alignment_is_canonical && base_covers_width && offset_is_aligned;
        assert_eq!(
            candidate.validate(&limits).is_ok(),
            oracle_accepts,
            "independent oracle mismatch case={case} alignment={alignment} width={width} offset={offset}"
        );
    }
}

#[test]
fn canonical_decoder_binds_allocation_alignment_to_relocations() {
    const FIRST_ALLOCATION_ALIGNMENT: std::ops::Range<usize> = 30..34;
    let limits = GraphLimits::default();

    for width in [4_u8, 8] {
        let exact_alignment = u32::from(width);
        let graph = graph(vec![
            pointer_source_at(0, 1, width, 0, exact_alignment),
            constant(1, vec![1]),
        ]);
        let encoded = graph.encode_canonical(&limits).unwrap();
        assert_eq!(
            DeviceConstantGraphV2::decode_canonical(&encoded, &limits).unwrap(),
            graph
        );

        for alignment in 1..exact_alignment {
            if !alignment.is_power_of_two() {
                continue;
            }
            let mut under_aligned = encoded.clone();
            under_aligned[FIRST_ALLOCATION_ALIGNMENT].copy_from_slice(&alignment.to_le_bytes());
            assert_eq!(
                DeviceConstantGraphV2::decode_canonical(&under_aligned, &limits),
                Err(DecodeError::Graph(ValidationError::UnalignedRelocation(
                    AllocationId(0)
                )))
            );
        }

        for alignment in [exact_alignment, exact_alignment * 2] {
            let mut strengthened = encoded.clone();
            strengthened[FIRST_ALLOCATION_ALIGNMENT].copy_from_slice(&alignment.to_le_bytes());
            let decoded = DeviceConstantGraphV2::decode_canonical(&strengthened, &limits).unwrap();
            assert_eq!(decoded.allocations[0].alignment, alignment);
            assert_eq!(decoded.encode_canonical(&limits).unwrap(), strengthened);
        }
    }

    let mixed = graph(vec![
        mixed_width_pointer_source(8),
        constant(1, vec![1]),
        constant(2, vec![2]),
    ]);
    let mut under_aligned = mixed.encode_canonical(&limits).unwrap();
    under_aligned[FIRST_ALLOCATION_ALIGNMENT].copy_from_slice(&4_u32.to_le_bytes());
    assert_eq!(
        DeviceConstantGraphV2::decode_canonical(&under_aligned, &limits),
        Err(DecodeError::Graph(ValidationError::UnalignedRelocation(
            AllocationId(0)
        )))
    );
}

#[test]
fn duplicate_and_noncanonical_records_are_rejected() {
    let limits = GraphLimits::default();

    let mut duplicate_validity = constant(0, vec![1, 2, 3, 4]);
    let region = duplicate_validity.validity[0];
    duplicate_validity.validity.push(region);
    assert_eq!(
        graph(vec![duplicate_validity]).validate(&limits),
        Err(ValidationError::NonCanonicalValidityOrder(AllocationId(0)))
    );

    let mut duplicate_relocation = pointer_source(0, 1);
    let relocation = duplicate_relocation.relocations[0];
    duplicate_relocation.relocations.push(relocation);
    assert_eq!(
        graph(vec![duplicate_relocation, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::NonCanonicalRelocationOrder(AllocationId(
            0
        )))
    );

    let mut overlap = pointer_source(0, 1);
    overlap.bytes = vec![0; 16];
    overlap.validity = vec![
        ValidityRegion {
            offset: 0,
            len: 8,
            class: ValidityClass::Pointer,
        },
        ValidityRegion {
            offset: 8,
            len: 8,
            class: ValidityClass::Pointer,
        },
    ];
    overlap.relocations.push(Relocation {
        source_offset: 4,
        ..relocation
    });
    assert_eq!(
        graph(vec![overlap, constant(1, vec![1])]).validate(&limits),
        Err(ValidationError::RelocationOverlap(AllocationId(0)))
    );
}

#[test]
fn semantic_type_commitment_round_trips_without_raw_type_bytes() {
    let limits = GraphLimits::default();
    let mut allocation = constant(0, vec![7]);
    allocation.semantic_type = SemanticTypeId {
        schema_version: u16::MAX,
        domain: *b"type-domain-v2!!",
        digest: [0xa5; 32],
    };
    let graph = graph(vec![allocation]);
    let encoded = graph.encode_canonical(&limits).unwrap();
    let decoded = DeviceConstantGraphV2::decode_canonical(&encoded, &limits).unwrap();
    assert_eq!(decoded, graph);
    assert_eq!(decoded.allocations[0].semantic_type.digest, [0xa5; 32]);
}

#[test]
fn codec_rejects_unknown_tags_versions_and_reserved_bits() {
    let limits = GraphLimits::default();
    let encoded = graph(vec![pointer_source(0, 1), constant(1, vec![1])])
        .encode_canonical(&limits)
        .unwrap();

    let mut bad_version = encoded.clone();
    bad_version[4..6].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        DeviceConstantGraphV2::decode_canonical(&bad_version, &limits),
        Err(DecodeError::UnsupportedVersion(1))
    );

    for offset in [6_usize, 29, 111, 121] {
        let mut reserved = encoded.clone();
        reserved[offset] = 1;
        assert_eq!(
            DeviceConstantGraphV2::decode_canonical(&reserved, &limits),
            Err(DecodeError::NonZeroReserved)
        );
    }

    for offset in [26_usize, 27, 28, 110, 119, 120] {
        let mut unknown = encoded.clone();
        unknown[offset] = 0xff;
        assert_eq!(
            DeviceConstantGraphV2::decode_canonical(&unknown, &limits),
            Err(DecodeError::UnknownTag)
        );
    }
}

#[test]
fn empty_graph_has_a_versioned_header_golden() {
    let limits = GraphLimits::default();
    let empty = graph(Vec::new());
    let encoded = empty.encode_canonical(&limits).unwrap();
    assert_eq!(hex(&encoded), "4632433202000000000000000000000000000000");
    assert_eq!(
        DeviceConstantGraphV2::decode_canonical(&encoded, &limits).unwrap(),
        empty
    );
}

#[test]
fn alignment_limit_reports_observed_and_limit() {
    let mut allocation = constant(0, vec![1]);
    allocation.alignment = 128;
    let limits = GraphLimits {
        max_alignment: 64,
        ..GraphLimits::default()
    };
    assert_eq!(
        graph(vec![allocation]).validate(&limits),
        Err(ValidationError::ResourceLimit {
            resource: Resource::Alignment,
            observed: 128,
            limit: 64,
        })
    );
}
