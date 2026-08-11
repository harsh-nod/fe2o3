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
        semantic_type: semantic_type(id as u8 + 1),
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
        semantic_type: semantic_type(id as u8 + 9),
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

fn graph(allocations: Vec<Allocation>) -> DeviceConstantGraphV2 {
    DeviceConstantGraphV2 { allocations }
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
