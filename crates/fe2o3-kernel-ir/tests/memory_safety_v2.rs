#[path = "../src/memory_safety_v2.rs"]
mod memory_safety_v2;

use memory_safety_v2::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn ty(value: u32) -> MemoryTypeIdV2 {
    MemoryTypeIdV2::new(value).unwrap()
}
fn alloc(value: u32) -> AllocationIdV2 {
    AllocationIdV2::new(value).unwrap()
}
fn owner(value: u32) -> OwnerIdV2 {
    OwnerIdV2::new(value).unwrap()
}
fn loan(value: u32) -> LoanIdV2 {
    LoanIdV2::new(value).unwrap()
}
fn cap(value: u32) -> CapabilityIdV2 {
    CapabilityIdV2::new(value).unwrap()
}
fn life(start: u64, end: u64) -> LifetimeRegionV2 {
    LifetimeRegionV2 {
        start: EpochV2(start),
        end_inclusive: EpochV2(end),
    }
}
fn provenance() -> ProvenanceV2 {
    ProvenanceV2 {
        allocation: alloc(1),
        generation: 7,
    }
}

fn scalar(id: u32, bits: u16, validity: BitValidityV2) -> MemoryTypeV2 {
    MemoryTypeV2 {
        id: ty(id),
        size: u64::from(bits.div_ceil(8)),
        alignment: u64::from(bits.div_ceil(8)),
        kind: MemoryTypeKindV2::Scalar {
            bit_width: bits,
            validity,
        },
    }
}

fn type_table() -> Vec<MemoryTypeV2> {
    vec![
        scalar(1, 8, BitValidityV2::Any),
        scalar(2, 8, BitValidityV2::Bool),
        scalar(3, 8, BitValidityV2::NonZero),
        scalar(4, 32, BitValidityV2::Any),
        MemoryTypeV2 {
            id: ty(5),
            size: 16,
            alignment: 4,
            kind: MemoryTypeKindV2::Array {
                element: ty(4),
                length: 4,
                stride: 4,
            },
        },
        MemoryTypeV2 {
            id: ty(6),
            size: 20,
            alignment: 4,
            kind: MemoryTypeKindV2::Aggregate {
                fields: vec![
                    MemoryFieldV2 {
                        offset: 0,
                        ty: ty(2),
                    },
                    MemoryFieldV2 {
                        offset: 4,
                        ty: ty(5),
                    },
                ],
            },
        },
        MemoryTypeV2 {
            id: ty(7),
            size: 4,
            alignment: 4,
            kind: MemoryTypeKindV2::OpaqueBytes,
        },
    ]
}

fn allocate(byte_len: u64) -> MemoryActionV2 {
    allocate_at(
        1,
        7,
        AddressSpaceV2::Global,
        0x1000,
        byte_len,
        16,
        life(0, 100),
    )
}

fn allocate_at(
    allocation: u32,
    generation: u64,
    address_space: AddressSpaceV2,
    base_address: u64,
    byte_len: u64,
    alignment: u64,
    lifetime: LifetimeRegionV2,
) -> MemoryActionV2 {
    MemoryActionV2::Allocate {
        allocation: alloc(allocation),
        generation,
        owner: owner(1),
        address_space,
        base_address,
        byte_len,
        alignment,
        lifetime,
    }
}

fn place(root: u32, base_offset: u64, projections: Vec<ProjectionV2>) -> TypedPlaceV2 {
    TypedPlaceV2 {
        provenance: provenance(),
        base_offset,
        root_type: ty(root),
        projections,
    }
}

fn program(actions: Vec<MemoryActionV2>) -> MemoryProgramV2 {
    MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        type_table(),
        actions,
        MemoryBudgetsV2::default(),
    )
    .unwrap()
}

fn execute(actions: Vec<MemoryActionV2>) -> Result<MemoryExecutionV2, MemoryModelErrorV2> {
    execute_memory_program_v2(&program(actions), MemoryBudgetsV2::default())
}

fn reason(actions: Vec<MemoryActionV2>) -> MemoryErrorReasonV2 {
    execute(actions).unwrap_err().reason
}

#[test]
fn typed_projection_write_and_read_emit_local_obligations() {
    let projected = place(6, 0, vec![ProjectionV2::Field(1), ProjectionV2::Index(3)]);
    let actions = vec![
        allocate(64),
        MemoryActionV2::WriteTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: projected.clone(),
            value: TypedWriteValueV2::KnownBits(0xfeed_beef),
        },
        MemoryActionV2::ReadTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: projected,
        },
    ];
    let execution = execute(actions).unwrap();
    assert_eq!(execution.live_allocations(), 1);
    assert_eq!(execution.final_epoch(), EpochV2(0));
    assert_ne!(execution.untrusted_program_identity().digest(), &[0; 32]);
    assert_ne!(execution.report_identity().digest(), &[0; 32]);
    assert!(execution.records().iter().all(|record| {
        &record.program_identity == execution.untrusted_program_identity()
            && record
                .obligations
                .iter()
                .all(|obligation| obligation.program_identity == record.program_identity)
    }));
    assert!(
        execution.records()[2]
            .obligations
            .iter()
            .any(|item| item.kind == MemoryObligationKindV2::Initialized)
    );
    assert!(!execution.grants_runtime_authority());
    assert!(!execution.grants_proof_authority());
    assert!(!execution.proves_compiler_refinement());
    assert!(!execution.proves_gpu_behavior());
    assert!(!execution.proves_race_freedom());
}

#[test]
fn out_of_bounds_and_misaligned_places_fail_closed() {
    assert_eq!(
        reason(vec![
            allocate(8),
            MemoryActionV2::WriteTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(4, 6, vec![]),
                value: TypedWriteValueV2::KnownBits(1),
            }
        ]),
        MemoryErrorReasonV2::OutOfBounds
    );
    assert_eq!(
        reason(vec![
            allocate(16),
            MemoryActionV2::WriteTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(4, 1, vec![]),
                value: TypedWriteValueV2::KnownBits(1),
            }
        ]),
        MemoryErrorReasonV2::Misaligned
    );
    assert_eq!(
        reason(vec![
            allocate(64),
            MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(5, 0, vec![ProjectionV2::Index(4)]),
            }
        ]),
        MemoryErrorReasonV2::InvalidProjection
    );
}

#[test]
fn provenance_generation_and_lifetime_prevent_use_after_free() {
    let stale = TypedPlaceV2 {
        provenance: ProvenanceV2 {
            allocation: alloc(1),
            generation: 6,
        },
        base_offset: 0,
        root_type: ty(1),
        projections: vec![],
    };
    assert_eq!(
        reason(vec![
            allocate(8),
            MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: stale
            }
        ]),
        MemoryErrorReasonV2::ProvenanceMismatch
    );
    assert_eq!(
        reason(vec![
            allocate(8),
            MemoryActionV2::Deallocate {
                allocation: alloc(1),
                owner: owner(1)
            },
            MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(1, 0, vec![]),
            }
        ]),
        MemoryErrorReasonV2::UseAfterFree
    );
    assert_eq!(
        reason(vec![
            allocate(8),
            MemoryActionV2::AdvanceEpoch { to: EpochV2(101) },
            MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(1, 0, vec![]),
            }
        ]),
        MemoryErrorReasonV2::UseAfterFree
    );
}

#[test]
fn initialization_and_bit_validity_are_separate_obligations() {
    let bool_place = place(2, 0, vec![]);
    assert_eq!(
        reason(vec![
            allocate(8),
            MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: bool_place.clone()
            }
        ]),
        MemoryErrorReasonV2::UninitializedRead
    );
    assert_eq!(
        reason(vec![
            allocate(8),
            MemoryActionV2::WriteTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: bool_place.clone(),
                value: TypedWriteValueV2::KnownBits(2),
            }
        ]),
        MemoryErrorReasonV2::InvalidBitPattern
    );
    execute(vec![
        allocate(8),
        MemoryActionV2::WriteTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: bool_place.clone(),
            value: TypedWriteValueV2::KnownBits(1),
        },
        MemoryActionV2::ReadTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: bool_place,
        },
    ])
    .unwrap();
    assert_eq!(
        reason(vec![
            allocate(8),
            MemoryActionV2::WriteTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(3, 0, vec![]),
                value: TypedWriteValueV2::KnownBits(0),
            }
        ]),
        MemoryErrorReasonV2::InvalidBitPattern
    );
}

#[test]
fn aggregate_validity_recurses_into_constrained_fields() {
    assert_eq!(
        reason(vec![
            allocate(32),
            MemoryActionV2::WriteTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(6, 0, vec![]),
                value: TypedWriteValueV2::ValidOpaque,
            },
        ]),
        MemoryErrorReasonV2::InvalidBitPattern
    );
}

#[test]
fn borrow_epochs_and_alias_rules_reject_stale_or_conflicting_access() {
    let left = place(7, 0, vec![]);
    let right = place(7, 4, vec![]);
    execute(vec![
        allocate(16),
        MemoryActionV2::BeginBorrow {
            loan: loan(1),
            owner: owner(1),
            place: left.clone(),
            kind: BorrowKindV2::Exclusive,
            lifetime: life(0, 10),
        },
        MemoryActionV2::BeginBorrow {
            loan: loan(2),
            owner: owner(1),
            place: right,
            kind: BorrowKindV2::Exclusive,
            lifetime: life(0, 10),
        },
        MemoryActionV2::WriteTyped {
            actor: AccessActorV2::Loan {
                loan: loan(1),
                borrow_epoch: 1,
            },
            place: left,
            value: TypedWriteValueV2::ValidOpaque,
        },
    ])
    .unwrap();
    assert_eq!(
        reason(vec![
            allocate(16),
            MemoryActionV2::BeginBorrow {
                loan: loan(1),
                owner: owner(1),
                place: place(7, 0, vec![]),
                kind: BorrowKindV2::Shared,
                lifetime: life(0, 10)
            },
            MemoryActionV2::BeginBorrow {
                loan: loan(2),
                owner: owner(1),
                place: place(7, 0, vec![]),
                kind: BorrowKindV2::Exclusive,
                lifetime: life(0, 10)
            },
        ]),
        MemoryErrorReasonV2::AliasConflict
    );
    assert_eq!(
        reason(vec![
            allocate(16),
            MemoryActionV2::BeginBorrow {
                loan: loan(1),
                owner: owner(1),
                place: place(7, 0, vec![]),
                kind: BorrowKindV2::Exclusive,
                lifetime: life(0, 10)
            },
            MemoryActionV2::EndBorrow {
                loan: loan(1),
                owner: owner(1)
            },
            MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Loan {
                    loan: loan(1),
                    borrow_epoch: 1
                },
                place: place(7, 0, vec![])
            },
        ]),
        MemoryErrorReasonV2::StaleBorrow
    );
}

#[test]
fn owner_cannot_bypass_an_active_exclusive_loan_or_deallocate_it() {
    let borrow = MemoryActionV2::BeginBorrow {
        loan: loan(1),
        owner: owner(1),
        place: place(7, 0, vec![]),
        kind: BorrowKindV2::Exclusive,
        lifetime: life(0, 10),
    };
    assert_eq!(
        reason(vec![
            allocate(16),
            borrow.clone(),
            MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(7, 0, vec![]),
            }
        ]),
        MemoryErrorReasonV2::AliasConflict
    );
    assert_eq!(
        reason(vec![
            allocate(16),
            borrow,
            MemoryActionV2::Deallocate {
                allocation: alloc(1),
                owner: owner(1)
            }
        ]),
        MemoryErrorReasonV2::ActiveBorrowAtDeallocation
    );
}

fn raw_capability(id: u32, access: RawAccessV2, range: ByteRangeV2) -> MemoryActionV2 {
    MemoryActionV2::GrantRawCapability {
        capability: cap(id),
        owner: owner(1),
        provenance: provenance(),
        scope: CapabilityScopeV2::Owner(owner(1)),
        range,
        access,
        lifetime: life(0, 20),
    }
}

fn raw_place(space: AddressSpaceV2, offset: u64, len: u64, alignment: u64) -> RawPlaceV2 {
    RawPlaceV2 {
        provenance: provenance(),
        pointer_address_space: space,
        byte_offset: offset,
        byte_len: len,
        alignment,
    }
}

#[test]
fn raw_access_requires_exact_capability_scope_range_and_access() {
    assert_eq!(
        reason(vec![
            allocate(16),
            MemoryActionV2::WriteRaw {
                actor: AccessActorV2::Owner(owner(1)),
                place: raw_place(AddressSpaceV2::Global, 0, 4, 4),
                raw_capability: cap(1),
                cast_capability: None,
            }
        ]),
        MemoryErrorReasonV2::MissingRawCapability
    );
    assert_eq!(
        reason(vec![
            allocate(16),
            raw_capability(1, RawAccessV2::Read, ByteRangeV2 { start: 0, len: 4 }),
            MemoryActionV2::WriteRaw {
                actor: AccessActorV2::Owner(owner(1)),
                place: raw_place(AddressSpaceV2::Global, 0, 4, 4),
                raw_capability: cap(1),
                cast_capability: None,
            }
        ]),
        MemoryErrorReasonV2::InvalidCapability
    );
    let result = execute(vec![
        allocate(16),
        raw_capability(1, RawAccessV2::ReadWrite, ByteRangeV2 { start: 0, len: 8 }),
        MemoryActionV2::WriteRaw {
            actor: AccessActorV2::Owner(owner(1)),
            place: raw_place(AddressSpaceV2::Global, 0, 4, 4),
            raw_capability: cap(1),
            cast_capability: None,
        },
        MemoryActionV2::ReadRaw {
            actor: AccessActorV2::Owner(owner(1)),
            place: raw_place(AddressSpaceV2::Global, 0, 4, 4),
            raw_capability: cap(1),
            cast_capability: None,
        },
    ])
    .unwrap();
    assert!(result.records()[2].obligations.iter().any(|item| item.kind
        == MemoryObligationKindV2::ExplicitRawCapability
        && item.basis == ObligationBasisV2::ExplicitCapability));
}

#[test]
fn address_space_casts_require_a_second_exact_capability() {
    assert_eq!(
        reason(vec![
            allocate(16),
            raw_capability(1, RawAccessV2::Write, ByteRangeV2 { start: 0, len: 4 }),
            MemoryActionV2::WriteRaw {
                actor: AccessActorV2::Owner(owner(1)),
                place: raw_place(AddressSpaceV2::Flat, 0, 4, 4),
                raw_capability: cap(1),
                cast_capability: None,
            }
        ]),
        MemoryErrorReasonV2::MissingAddressSpaceCastCapability
    );
    let execution = execute(vec![
        allocate(16),
        raw_capability(1, RawAccessV2::Write, ByteRangeV2 { start: 0, len: 4 }),
        MemoryActionV2::GrantAddressSpaceCastCapability {
            capability: cap(2),
            owner: owner(1),
            provenance: provenance(),
            scope: CapabilityScopeV2::Owner(owner(1)),
            range: ByteRangeV2 { start: 0, len: 4 },
            from: AddressSpaceV2::Global,
            to: AddressSpaceV2::Flat,
            lifetime: life(0, 20),
        },
        MemoryActionV2::WriteRaw {
            actor: AccessActorV2::Owner(owner(1)),
            place: raw_place(AddressSpaceV2::Flat, 0, 4, 4),
            raw_capability: cap(1),
            cast_capability: Some(cap(2)),
        },
    ])
    .unwrap();
    assert!(
        execution.records()[3]
            .obligations
            .iter()
            .any(|item| item.kind == MemoryObligationKindV2::ExplicitAddressSpaceCastCapability)
    );
}

#[test]
fn address_space_cast_rechecks_destination_pointer_width() {
    let high_base = u64::from(u32::MAX) + 0x1001;
    let allocation = MemoryActionV2::Allocate {
        allocation: alloc(1),
        generation: 7,
        owner: owner(1),
        address_space: AddressSpaceV2::Global,
        base_address: high_base,
        byte_len: 8,
        alignment: 1,
        lifetime: life(0, 20),
    };
    assert_eq!(
        reason(vec![
            allocation,
            raw_capability(1, RawAccessV2::Write, ByteRangeV2 { start: 0, len: 4 }),
            MemoryActionV2::GrantAddressSpaceCastCapability {
                capability: cap(2),
                owner: owner(1),
                provenance: provenance(),
                scope: CapabilityScopeV2::Owner(owner(1)),
                range: ByteRangeV2 { start: 0, len: 4 },
                from: AddressSpaceV2::Global,
                to: AddressSpaceV2::Workgroup,
                lifetime: life(0, 20),
            },
            MemoryActionV2::WriteRaw {
                actor: AccessActorV2::Owner(owner(1)),
                place: raw_place(AddressSpaceV2::Workgroup, 0, 4, 1),
                raw_capability: cap(1),
                cast_capability: Some(cap(2)),
            },
        ]),
        MemoryErrorReasonV2::AddressNotRepresentable
    );
}

#[test]
fn raw_writes_initialize_bytes_but_invalidate_constrained_typed_facts() {
    assert_eq!(
        reason(vec![
            allocate(8),
            MemoryActionV2::WriteTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(2, 0, vec![]),
                value: TypedWriteValueV2::KnownBits(1)
            },
            raw_capability(1, RawAccessV2::Write, ByteRangeV2 { start: 0, len: 1 }),
            MemoryActionV2::WriteRaw {
                actor: AccessActorV2::Owner(owner(1)),
                place: raw_place(AddressSpaceV2::Global, 0, 1, 1),
                raw_capability: cap(1),
                cast_capability: None
            },
            MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(2, 0, vec![])
            },
        ]),
        MemoryErrorReasonV2::IncompatibleBitValidity
    );
}

#[test]
fn pointer_distance_requires_same_provenance_and_element_divisibility() {
    let left = raw_place(AddressSpaceV2::Global, 0, 0, 4);
    let right = raw_place(AddressSpaceV2::Global, 12, 0, 4);
    let actions = vec![
        allocate(32),
        raw_capability(1, RawAccessV2::Read, ByteRangeV2 { start: 0, len: 16 }),
        MemoryActionV2::PointerDistance {
            actor: AccessActorV2::Owner(owner(1)),
            left,
            right,
            element_size: 4,
            left_capability: cap(1),
            right_capability: cap(1),
            left_cast_capability: None,
            right_cast_capability: None,
        },
    ];
    let program = program(actions);
    let bytes = program.canonical_bytes(MemoryBudgetsV2::default()).unwrap();
    let decoded = MemoryProgramV2::decode_canonical(&bytes, MemoryBudgetsV2::default()).unwrap();
    let execution = execute_memory_program_v2(&decoded, MemoryBudgetsV2::default()).unwrap();
    assert!(
        execution.records()[2]
            .obligations
            .iter()
            .any(|item| item.kind == MemoryObligationKindV2::PointerDistanceSameAllocation)
    );
    assert!(
        execution.records()[2]
            .obligations
            .iter()
            .any(|item| item.kind == MemoryObligationKindV2::PointerDistanceElementDivisibility)
    );

    assert_eq!(
        reason(vec![
            allocate(32),
            raw_capability(1, RawAccessV2::Read, ByteRangeV2 { start: 0, len: 16 }),
            MemoryActionV2::PointerDistance {
                actor: AccessActorV2::Owner(owner(1)),
                left,
                right: raw_place(AddressSpaceV2::Global, 10, 0, 2),
                element_size: 4,
                left_capability: cap(1),
                right_capability: cap(1),
                left_cast_capability: None,
                right_cast_capability: None,
            },
        ]),
        MemoryErrorReasonV2::InvalidPointerDistance
    );
}

#[test]
fn nonoverlapping_copy_requires_initialized_source_and_disjoint_ranges() {
    let read_write = raw_capability(1, RawAccessV2::ReadWrite, ByteRangeV2 { start: 0, len: 16 });
    let source = raw_place(AddressSpaceV2::Global, 0, 4, 4);
    let destination = raw_place(AddressSpaceV2::Global, 8, 4, 4);
    let execution = execute(vec![
        allocate(32),
        read_write.clone(),
        MemoryActionV2::WriteRaw {
            actor: AccessActorV2::Owner(owner(1)),
            place: source,
            raw_capability: cap(1),
            cast_capability: None,
        },
        MemoryActionV2::CopyNonOverlapping {
            actor: AccessActorV2::Owner(owner(1)),
            source,
            destination,
            source_capability: cap(1),
            destination_capability: cap(1),
            source_cast_capability: None,
            destination_cast_capability: None,
        },
        MemoryActionV2::ReadRaw {
            actor: AccessActorV2::Owner(owner(1)),
            place: destination,
            raw_capability: cap(1),
            cast_capability: None,
        },
    ])
    .unwrap();
    assert!(
        execution.records()[3]
            .obligations
            .iter()
            .any(|item| item.kind == MemoryObligationKindV2::NonOverlappingCopy)
    );

    assert_eq!(
        reason(vec![
            allocate(32),
            read_write,
            MemoryActionV2::CopyNonOverlapping {
                actor: AccessActorV2::Owner(owner(1)),
                source,
                destination: raw_place(AddressSpaceV2::Global, 2, 4, 2),
                source_capability: cap(1),
                destination_capability: cap(1),
                source_cast_capability: None,
                destination_cast_capability: None,
            },
        ]),
        MemoryErrorReasonV2::OverlappingCopy
    );
    assert_eq!(
        reason(vec![
            allocate(32),
            raw_capability(1, RawAccessV2::ReadWrite, ByteRangeV2 { start: 0, len: 16 }),
            MemoryActionV2::CopyNonOverlapping {
                actor: AccessActorV2::Owner(owner(1)),
                source,
                destination,
                source_capability: cap(1),
                destination_capability: cap(1),
                source_cast_capability: None,
                destination_cast_capability: None,
            },
        ]),
        MemoryErrorReasonV2::UninitializedRead
    );
}

#[test]
fn gfx942_32_bit_spaces_enforce_address_representability() {
    let action = MemoryActionV2::Allocate {
        allocation: alloc(1),
        generation: 1,
        owner: owner(1),
        address_space: AddressSpaceV2::Workgroup,
        base_address: u64::from(u32::MAX) - 3,
        byte_len: 8,
        alignment: 4,
        lifetime: life(0, 1),
    };
    assert_eq!(
        reason(vec![action]),
        MemoryErrorReasonV2::AddressNotRepresentable
    );

    let unrepresentable_empty = MemoryActionV2::Allocate {
        allocation: alloc(1),
        generation: 1,
        owner: owner(1),
        address_space: AddressSpaceV2::Workgroup,
        base_address: u64::from(u32::MAX) + 1,
        byte_len: 0,
        alignment: 1,
        lifetime: life(0, 1),
    };
    assert_eq!(
        reason(vec![unrepresentable_empty]),
        MemoryErrorReasonV2::AddressNotRepresentable
    );
}

#[test]
fn gfx942_32_bit_one_past_arithmetic_bound_is_not_a_pointer() {
    let allocation = MemoryActionV2::Allocate {
        allocation: alloc(1),
        generation: 7,
        owner: owner(1),
        address_space: AddressSpaceV2::Workgroup,
        base_address: u64::from(u32::MAX) - 3,
        byte_len: 4,
        alignment: 4,
        lifetime: life(0, 20),
    };
    let capability = raw_capability(1, RawAccessV2::ReadWrite, ByteRangeV2 { start: 0, len: 4 });

    execute(vec![
        allocation.clone(),
        capability.clone(),
        MemoryActionV2::WriteRaw {
            actor: AccessActorV2::Owner(owner(1)),
            place: raw_place(AddressSpaceV2::Workgroup, 3, 1, 1),
            raw_capability: cap(1),
            cast_capability: None,
        },
    ])
    .unwrap();

    assert_eq!(
        reason(vec![
            allocation,
            capability,
            MemoryActionV2::PointerDistance {
                actor: AccessActorV2::Owner(owner(1)),
                left: raw_place(AddressSpaceV2::Workgroup, 0, 0, 1),
                right: raw_place(AddressSpaceV2::Workgroup, 4, 0, 1),
                element_size: 1,
                left_capability: cap(1),
                right_capability: cap(1),
                left_cast_capability: None,
                right_cast_capability: None,
            },
        ]),
        MemoryErrorReasonV2::AddressNotRepresentable
    );
}

fn named_allocation(
    id: u32,
    generation: u64,
    address_space: AddressSpaceV2,
    base_address: u64,
    byte_len: u64,
) -> MemoryActionV2 {
    MemoryActionV2::Allocate {
        allocation: alloc(id),
        generation,
        owner: owner(1),
        address_space,
        base_address,
        byte_len,
        alignment: 1,
        lifetime: life(0, 100),
    }
}

#[test]
fn live_allocation_physical_ranges_are_disjoint_per_address_space() {
    for second in [
        named_allocation(2, 8, AddressSpaceV2::Global, 0x1000, 16),
        named_allocation(2, 9, AddressSpaceV2::Global, 0x1008, 16),
    ] {
        assert_eq!(
            reason(vec![
                named_allocation(1, 7, AddressSpaceV2::Global, 0x1000, 16),
                second,
            ]),
            MemoryErrorReasonV2::OverlappingLiveAllocation
        );
    }

    execute(vec![
        named_allocation(1, 7, AddressSpaceV2::Global, 0x1000, 16),
        named_allocation(2, 8, AddressSpaceV2::Global, 0x1010, 16),
    ])
    .unwrap();
    execute(vec![
        named_allocation(1, 7, AddressSpaceV2::Global, 0x1000, 16),
        named_allocation(2, 8, AddressSpaceV2::Workgroup, 0x1000, 16),
    ])
    .unwrap();
}

#[test]
fn zero_size_allocations_do_not_claim_storage_and_dead_storage_can_be_reused() {
    execute(vec![
        named_allocation(1, 7, AddressSpaceV2::Global, 0x1008, 0),
        named_allocation(2, 8, AddressSpaceV2::Global, 0x1008, 0),
        named_allocation(3, 9, AddressSpaceV2::Global, 0x1000, 16),
    ])
    .unwrap();

    execute(vec![
        named_allocation(1, 7, AddressSpaceV2::Global, 0x1000, 16),
        MemoryActionV2::Deallocate {
            allocation: alloc(1),
            owner: owner(1),
        },
        named_allocation(2, 8, AddressSpaceV2::Global, 0x1000, 16),
    ])
    .unwrap();

    assert_eq!(
        reason(vec![
            named_allocation(1, 7, AddressSpaceV2::Global, 0x1000, 16),
            MemoryActionV2::Deallocate {
                allocation: alloc(1),
                owner: owner(1),
            },
            named_allocation(1, 8, AddressSpaceV2::Global, 0x1000, 16),
        ]),
        MemoryErrorReasonV2::DuplicateAllocation(alloc(1))
    );
}

#[test]
fn canonical_codec_is_deterministic_and_strict() {
    let mut reversed = type_table();
    reversed.reverse();
    let actions = vec![
        allocate(64),
        MemoryActionV2::WriteTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: place(4, 0, vec![]),
            value: TypedWriteValueV2::KnownBits(9),
        },
    ];
    let left = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        type_table(),
        actions.clone(),
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let right = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        reversed,
        actions,
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let bytes = left.canonical_bytes(MemoryBudgetsV2::default()).unwrap();
    assert_eq!(left.target().architecture(), "gfx942");
    assert!(left.target().xnack_disabled());
    assert!(left.target().little_endian());
    assert_eq!(left.target().address_spaces().len(), 5);
    assert_eq!(left.types().len(), 7);
    assert_eq!(left.actions().len(), 2);
    assert_eq!(
        bytes,
        right.canonical_bytes(MemoryBudgetsV2::default()).unwrap()
    );
    assert_eq!(&bytes[..12], b"FE2OMEM2\x02\x00\x00\x00");
    assert_eq!(
        MemoryProgramV2::decode_canonical(&bytes, MemoryBudgetsV2::default()).unwrap(),
        left
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        MemoryProgramV2::decode_canonical(&trailing, MemoryBudgetsV2::default())
            .unwrap_err()
            .reason,
        MemoryErrorReasonV2::Decode { .. }
    ));
}

#[test]
fn validity_sets_have_one_canonical_representation() {
    let rejected = [
        scalar(
            1,
            8,
            BitValidityV2::Ranges(vec![BitValidityRangeV2 {
                start: 0,
                end_inclusive: u8::MAX.into(),
            }]),
        ),
        scalar(
            1,
            8,
            BitValidityV2::Ranges(vec![BitValidityRangeV2 {
                start: 0,
                end_inclusive: 1,
            }]),
        ),
        scalar(
            1,
            8,
            BitValidityV2::Ranges(vec![BitValidityRangeV2 {
                start: 1,
                end_inclusive: u8::MAX.into(),
            }]),
        ),
        scalar(
            1,
            8,
            BitValidityV2::Ranges(vec![
                BitValidityRangeV2 {
                    start: 2,
                    end_inclusive: 3,
                },
                BitValidityRangeV2 {
                    start: 4,
                    end_inclusive: 5,
                },
            ]),
        ),
        scalar(
            1,
            8,
            BitValidityV2::Ranges(vec![
                BitValidityRangeV2 {
                    start: 2,
                    end_inclusive: 4,
                },
                BitValidityRangeV2 {
                    start: 4,
                    end_inclusive: 5,
                },
            ]),
        ),
        scalar(
            1,
            32,
            BitValidityV2::Ranges(vec![
                BitValidityRangeV2 {
                    start: 0,
                    end_inclusive: 0xd7ff,
                },
                BitValidityRangeV2 {
                    start: 0xe000,
                    end_inclusive: 0x10ffff,
                },
            ]),
        ),
    ];
    for alternate in rejected {
        let error = MemoryProgramV2::new(
            TargetLayoutV2::gfx942_xnack_minus(),
            vec![alternate],
            vec![],
            MemoryBudgetsV2::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error.reason,
            MemoryErrorReasonV2::InvalidType { .. }
        ));
    }

    let canonical = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        vec![scalar(
            1,
            8,
            BitValidityV2::Ranges(vec![
                BitValidityRangeV2 {
                    start: 2,
                    end_inclusive: 3,
                },
                BitValidityRangeV2 {
                    start: 5,
                    end_inclusive: 6,
                },
            ]),
        )],
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let bytes = canonical
        .canonical_bytes(MemoryBudgetsV2::default())
        .unwrap();
    assert_eq!(
        MemoryProgramV2::decode_canonical(&bytes, MemoryBudgetsV2::default()).unwrap(),
        canonical
    );
}

#[test]
fn resource_limits_reject_before_unbounded_collection_growth() {
    let budgets = MemoryBudgetsV2 {
        max_types: 1,
        ..MemoryBudgetsV2::default()
    };
    assert!(matches!(
        MemoryProgramV2::new(
            TargetLayoutV2::gfx942_xnack_minus(),
            type_table(),
            vec![],
            budgets
        )
        .unwrap_err()
        .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "types",
            ..
        }
    ));
    let budgets = MemoryBudgetsV2 {
        max_actions: 1,
        ..MemoryBudgetsV2::default()
    };
    assert!(matches!(
        MemoryProgramV2::new(
            TargetLayoutV2::gfx942_xnack_minus(),
            type_table(),
            vec![allocate(8), MemoryActionV2::AdvanceEpoch { to: EpochV2(1) }],
            budgets
        )
        .unwrap_err()
        .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "actions",
            ..
        }
    ));
    let budgets = MemoryBudgetsV2 {
        max_projections_per_place: 1,
        ..MemoryBudgetsV2::default()
    };
    assert!(matches!(
        MemoryProgramV2::new(
            TargetLayoutV2::gfx942_xnack_minus(),
            type_table(),
            vec![MemoryActionV2::ReadTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(6, 0, vec![ProjectionV2::Field(1), ProjectionV2::Index(0)]),
            }],
            budgets
        )
        .unwrap_err()
        .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "place projections",
            ..
        }
    ));
}

#[test]
fn validity_traversal_uses_the_global_work_budget_at_exact_boundaries() {
    let ranges = (0_u128..1_000)
        .map(|index| BitValidityRangeV2 {
            start: index * 2,
            end_inclusive: index * 2,
        })
        .collect();
    let one_type = vec![scalar(1, 128, BitValidityV2::Ranges(ranges))];

    let reset_reproducer = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        one_type.clone(),
        vec![],
        MemoryBudgetsV2 {
            max_validation_work: 1_010,
            ..MemoryBudgetsV2::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        reset_reproducer.reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "validation work",
            actual: 1_014,
            max: 1_010,
        }
    ));

    let (accepted, construction_work) = MemoryProgramV2::new_with_work(
        TargetLayoutV2::gfx942_xnack_minus(),
        one_type,
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    assert!(construction_work > 3_000);
    let (_, exact_construction_work) = MemoryProgramV2::new_with_work(
        TargetLayoutV2::gfx942_xnack_minus(),
        accepted.types().to_vec(),
        vec![],
        MemoryBudgetsV2 {
            max_validation_work: construction_work,
            ..MemoryBudgetsV2::default()
        },
    )
    .unwrap();
    assert_eq!(exact_construction_work, construction_work);
    assert_eq!(
        MemoryProgramV2::new(
            TargetLayoutV2::gfx942_xnack_minus(),
            accepted.types().to_vec(),
            vec![],
            MemoryBudgetsV2 {
                max_validation_work: construction_work - 1,
                ..MemoryBudgetsV2::default()
            },
        )
        .unwrap_err()
        .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "validation work",
            actual: construction_work,
            max: construction_work - 1,
        }
    );

    let (bytes, canonical_work) = accepted
        .canonical_bytes_with_work(MemoryBudgetsV2::default())
        .unwrap();
    assert_eq!(canonical_work + 1, construction_work);
    let (_, decode_work) =
        MemoryProgramV2::decode_canonical_with_work(&bytes, MemoryBudgetsV2::default()).unwrap();
    let (_, exact_decode_work) = MemoryProgramV2::decode_canonical_with_work(
        &bytes,
        MemoryBudgetsV2 {
            max_validation_work: decode_work,
            ..MemoryBudgetsV2::default()
        },
    )
    .unwrap();
    assert_eq!(exact_decode_work, decode_work);
    assert_eq!(
        MemoryProgramV2::decode_canonical(
            &bytes,
            MemoryBudgetsV2 {
                max_validation_work: decode_work - 1,
                ..MemoryBudgetsV2::default()
            },
        )
        .unwrap_err()
        .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "validation work",
            actual: decode_work,
            max: decode_work - 1,
        }
    );
}

#[test]
fn decode_through_identity_and_execution_uses_one_validation_budget() {
    let ranges = (0_u128..1_000)
        .map(|index| BitValidityRangeV2 {
            start: index * 2,
            end_inclusive: index * 2,
        })
        .collect();
    let constructed = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        vec![scalar(1, 128, BitValidityV2::Ranges(ranges))],
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let direct_execution =
        execute_memory_program_v2(&constructed, MemoryBudgetsV2::default()).unwrap();
    assert!(direct_execution.validation_work() > constructed.admission_validation_work());

    let bytes = constructed
        .canonical_bytes(MemoryBudgetsV2::default())
        .unwrap();
    let (decoded, decode_work) =
        MemoryProgramV2::decode_canonical_with_work(&bytes, MemoryBudgetsV2::default()).unwrap();
    assert_eq!(decoded.admission_validation_work(), decode_work);
    let baseline = execute_memory_program_v2(&decoded, MemoryBudgetsV2::default()).unwrap();
    let total_work = baseline.validation_work();
    assert!(total_work > decode_work);

    let exact = MemoryBudgetsV2 {
        max_validation_work: total_work,
        ..MemoryBudgetsV2::default()
    };
    let exact_decoded = MemoryProgramV2::decode_canonical(&bytes, exact).unwrap();
    let exact_execution = execute_memory_program_v2(&exact_decoded, exact).unwrap();
    assert_eq!(exact_execution.validation_work(), total_work);
    assert!(
        exact_execution
            .verify_identities(&exact_decoded, exact)
            .unwrap()
    );

    let one_less = MemoryBudgetsV2 {
        max_validation_work: total_work - 1,
        ..MemoryBudgetsV2::default()
    };
    let (rejected_at_execution, repeated_decode_work) =
        MemoryProgramV2::decode_canonical_with_work(&bytes, one_less).unwrap();
    assert_eq!(repeated_decode_work, decode_work);
    assert_eq!(
        execute_memory_program_v2(&rejected_at_execution, one_less)
            .unwrap_err()
            .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "validation work",
            actual: total_work,
            max: total_work - 1,
        }
    );
    assert!(matches!(
        exact_execution
            .verify_identities(&rejected_at_execution, one_less)
            .unwrap_err()
            .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "validation work",
            max,
            ..
        } if max == total_work - 1
    ));
}

#[test]
fn execution_obligation_and_typed_fact_limits_are_enforced() {
    let one_type = vec![MemoryTypeV2 {
        id: ty(1),
        size: 0,
        alignment: 1,
        kind: MemoryTypeKindV2::OpaqueBytes,
    }];
    let actions = vec![
        allocate(1),
        MemoryActionV2::WriteTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: place(1, 0, vec![]),
            value: TypedWriteValueV2::ValidOpaque,
        },
        MemoryActionV2::WriteTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: place(1, 1, vec![]),
            value: TypedWriteValueV2::ValidOpaque,
        },
    ];
    let budgets = MemoryBudgetsV2 {
        max_state_ranges: 1,
        ..MemoryBudgetsV2::default()
    };
    let program = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        one_type,
        actions,
        budgets,
    )
    .unwrap();
    assert!(matches!(
        execute_memory_program_v2(&program, budgets)
            .unwrap_err()
            .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "typed state ranges",
            ..
        }
    ));

    let budgets = MemoryBudgetsV2 {
        max_obligations: 1,
        ..MemoryBudgetsV2::default()
    };
    let program = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        type_table(),
        vec![allocate(8)],
        budgets,
    )
    .unwrap();
    assert!(matches!(
        execute_memory_program_v2(&program, budgets)
            .unwrap_err()
            .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "obligations",
            ..
        }
    ));
}

#[test]
fn deep_type_graph_validation_is_iterative_and_cycles_fail_closed() {
    let mut types = vec![MemoryTypeV2 {
        id: ty(1),
        size: 1,
        alignment: 1,
        kind: MemoryTypeKindV2::OpaqueBytes,
    }];
    for id in 2..=4_096 {
        types.push(MemoryTypeV2 {
            id: ty(id),
            size: 1,
            alignment: 1,
            kind: MemoryTypeKindV2::Array {
                element: ty(id - 1),
                length: 1,
                stride: 1,
            },
        });
    }
    MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        types,
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();

    let cycle = vec![
        MemoryTypeV2 {
            id: ty(1),
            size: 1,
            alignment: 1,
            kind: MemoryTypeKindV2::Array {
                element: ty(2),
                length: 1,
                stride: 1,
            },
        },
        MemoryTypeV2 {
            id: ty(2),
            size: 1,
            alignment: 1,
            kind: MemoryTypeKindV2::Array {
                element: ty(1),
                length: 1,
                stride: 1,
            },
        },
    ];
    assert!(matches!(
        MemoryProgramV2::new(
            TargetLayoutV2::gfx942_xnack_minus(),
            cycle,
            vec![],
            MemoryBudgetsV2::default()
        )
        .unwrap_err()
        .reason,
        MemoryErrorReasonV2::TypeCycle(_)
    ));
}

#[test]
fn hostile_decoder_campaign_is_bounded_deterministic_and_panic_free() {
    let seed = program(vec![
        allocate(64),
        MemoryActionV2::WriteTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: place(6, 0, vec![ProjectionV2::Field(1), ProjectionV2::Index(2)]),
            value: TypedWriteValueV2::KnownBits(5),
        },
    ])
    .canonical_bytes(MemoryBudgetsV2::default())
    .unwrap();
    let mut state = 0x8f13_3a42_d199_6f01_u64;
    for case in 0..250_000_u64 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let mut bytes = seed.clone();
        match case % 4 {
            0 => bytes.truncate((state as usize) % (bytes.len() + 1)),
            1 => {
                let index = (state as usize) % bytes.len();
                bytes[index] ^= 1 << ((state >> 9) & 7);
            }
            2 => {
                let index = (state as usize) % bytes.len();
                bytes[index] = (state >> 32) as u8;
            }
            _ => bytes.extend_from_slice(&state.to_le_bytes()),
        }
        let first = catch_unwind(AssertUnwindSafe(|| {
            MemoryProgramV2::decode_canonical(&bytes, MemoryBudgetsV2::default())
        }))
        .expect("decoder panicked");
        let second = MemoryProgramV2::decode_canonical(&bytes, MemoryBudgetsV2::default());
        assert_eq!(first, second);
        if let Ok(decoded) = first {
            assert_eq!(
                decoded.canonical_bytes(MemoryBudgetsV2::default()).unwrap(),
                bytes
            );
        }
    }
}

#[test]
fn independent_32_bit_pointer_oracle_matches_100k_allocations() {
    let mut state = 0xa110_c942_32b1_7a5e_u64;
    for _ in 0..100_000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let base = (u64::from(u32::MAX) - 31).saturating_add((state >> 8) & 63);
        let len = (state >> 20) & 31;
        let expected = base <= u64::from(u32::MAX)
            && base
                .checked_add(len)
                .is_some_and(|end| end <= u64::from(u32::MAX) + 1);
        let candidate = named_allocation(1, 7, AddressSpaceV2::Workgroup, base, len);
        assert_eq!(
            execute(vec![candidate]).is_ok(),
            expected,
            "base={base} len={len}"
        );
    }
}

#[test]
fn independent_physical_interval_oracle_matches_50k_allocation_pairs() {
    let mut state = 0xd15a_110c_9420_5eed_u64;
    for _ in 0..50_000 {
        state = state
            .wrapping_mul(2862933555777941757)
            .wrapping_add(3037000493);
        let left_start = 0x1000 + ((state >> 8) & 63);
        let right_start = 0x1000 + ((state >> 16) & 63);
        let left_len = (state >> 24) & 31;
        let right_len = (state >> 32) & 31;
        let overlap = left_len != 0
            && right_len != 0
            && left_start < right_start + right_len
            && right_start < left_start + left_len;
        let actions = vec![
            named_allocation(1, 7, AddressSpaceV2::Global, left_start, left_len),
            named_allocation(2, 8, AddressSpaceV2::Global, right_start, right_len),
        ];
        assert_eq!(
            execute(actions).is_ok(),
            !overlap,
            "left={left_start}..{} right={right_start}..{}",
            left_start + left_len,
            right_start + right_len,
        );
    }
}

#[test]
fn independent_validity_gap_oracle_matches_50k_range_pairs() {
    let mut state = 0xca11_0a1c_9420_5eed_u64;
    for _ in 0..50_000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let left_start = ((state >> 8) & 127) as u128;
        let left_end = left_start + ((state >> 16) & 7) as u128;
        let right_start = ((state >> 24) & 255) as u128;
        let right_end = right_start + ((state >> 32) & 7) as u128;
        let in_domain = left_end <= u8::MAX.into() && right_end <= u8::MAX.into();
        let canonical = in_domain && left_end.saturating_add(1) < right_start;
        let candidate = scalar(
            1,
            8,
            BitValidityV2::Ranges(vec![
                BitValidityRangeV2 {
                    start: left_start,
                    end_inclusive: left_end,
                },
                BitValidityRangeV2 {
                    start: right_start,
                    end_inclusive: right_end,
                },
            ]),
        );
        assert_eq!(
            MemoryProgramV2::new(
                TargetLayoutV2::gfx942_xnack_minus(),
                vec![candidate],
                vec![],
                MemoryBudgetsV2::default(),
            )
            .is_ok(),
            canonical,
            "left={left_start}..={left_end} right={right_start}..={right_end}",
        );
    }
}

#[test]
fn independent_bounds_alignment_oracle_matches_50k_programs() {
    let only = vec![MemoryTypeV2 {
        id: ty(1),
        size: 4,
        alignment: 4,
        kind: MemoryTypeKindV2::OpaqueBytes,
    }];
    let mut state = 0xded0_0bad_cafe_f00d_u64;
    for _ in 0..50_000 {
        state = state
            .wrapping_mul(2862933555777941757)
            .wrapping_add(3037000493);
        let len = state & 63;
        let offset = (state >> 8) & 79;
        let base = 0x1000_u64;
        let expected = offset.checked_add(4).is_some_and(|end| end <= len)
            && (base + offset).is_multiple_of(4);
        let actions = vec![
            MemoryActionV2::Allocate {
                allocation: alloc(1),
                generation: 7,
                owner: owner(1),
                address_space: AddressSpaceV2::Global,
                base_address: base,
                byte_len: len,
                alignment: 16,
                lifetime: life(0, 1),
            },
            MemoryActionV2::WriteTyped {
                actor: AccessActorV2::Owner(owner(1)),
                place: place(1, offset, vec![]),
                value: TypedWriteValueV2::ValidOpaque,
            },
        ];
        let program = MemoryProgramV2::new(
            TargetLayoutV2::gfx942_xnack_minus(),
            only.clone(),
            actions,
            MemoryBudgetsV2::default(),
        )
        .unwrap();
        assert_eq!(
            execute_memory_program_v2(&program, MemoryBudgetsV2::default()).is_ok(),
            expected,
            "len={len} offset={offset}"
        );
    }
}

#[test]
fn independent_alias_oracle_matches_50k_borrow_pairs() {
    let only = vec![MemoryTypeV2 {
        id: ty(1),
        size: 4,
        alignment: 4,
        kind: MemoryTypeKindV2::OpaqueBytes,
    }];
    let mut state = 0x5eed_9420_1234_9876_u64;
    for _ in 0..50_000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let left = ((state >> 8) & 7) * 4;
        let right = ((state >> 16) & 7) * 4;
        let left_kind = if state & 1 == 0 {
            BorrowKindV2::Shared
        } else {
            BorrowKindV2::Exclusive
        };
        let right_kind = if state & 2 == 0 {
            BorrowKindV2::Shared
        } else {
            BorrowKindV2::Exclusive
        };
        let overlap = left < right + 4 && right < left + 4;
        let expected =
            !overlap || (left_kind == BorrowKindV2::Shared && right_kind == BorrowKindV2::Shared);
        let actions = vec![
            allocate(64),
            MemoryActionV2::BeginBorrow {
                loan: loan(1),
                owner: owner(1),
                place: place(1, left, vec![]),
                kind: left_kind,
                lifetime: life(0, 1),
            },
            MemoryActionV2::BeginBorrow {
                loan: loan(2),
                owner: owner(1),
                place: place(1, right, vec![]),
                kind: right_kind,
                lifetime: life(0, 1),
            },
        ];
        let program = MemoryProgramV2::new(
            TargetLayoutV2::gfx942_xnack_minus(),
            only.clone(),
            actions,
            MemoryBudgetsV2::default(),
        )
        .unwrap();
        assert_eq!(
            execute_memory_program_v2(&program, MemoryBudgetsV2::default()).is_ok(),
            expected,
            "left={left} right={right} {left_kind:?} {right_kind:?}"
        );
    }
}

#[test]
fn expired_allocations_cannot_be_deallocated_or_counted_live() {
    let expired_deallocation = execute(vec![
        allocate_at(1, 7, AddressSpaceV2::Global, 0x1000, 16, 16, life(0, 10)),
        MemoryActionV2::AdvanceEpoch { to: EpochV2(11) },
        MemoryActionV2::Deallocate {
            allocation: alloc(1),
            owner: owner(1),
        },
    ])
    .unwrap_err();
    assert_eq!(expired_deallocation.action_index, Some(2));
    assert_eq!(
        expired_deallocation.reason,
        MemoryErrorReasonV2::UseAfterFree
    );

    let expired_but_not_deallocated = execute(vec![
        allocate_at(1, 7, AddressSpaceV2::Global, 0x1000, 16, 16, life(0, 10)),
        MemoryActionV2::AdvanceEpoch { to: EpochV2(11) },
    ])
    .unwrap();
    assert_eq!(expired_but_not_deallocated.live_allocations(), 0);
    assert!(
        expired_but_not_deallocated.records()[1]
            .obligations
            .is_empty()
    );
}

#[test]
fn gfx942_alias_domains_and_mutability_are_conservative() {
    let target = TargetLayoutV2::gfx942_xnack_minus();
    assert_eq!(
        target.address_space_semantics(AddressSpaceV2::Global),
        target.address_space_semantics(AddressSpaceV2::Flat)
    );
    assert_eq!(
        target
            .address_space_semantics(AddressSpaceV2::Constant)
            .alias_domain,
        PhysicalAliasDomainV2::GlobalFlat
    );
    assert_eq!(
        target
            .address_space_semantics(AddressSpaceV2::Constant)
            .mutability,
        MemoryMutabilityV2::ReadOnly
    );

    for second_space in [AddressSpaceV2::Flat, AddressSpaceV2::Constant] {
        let error = execute(vec![
            allocate(16),
            MemoryActionV2::Allocate {
                allocation: alloc(2),
                generation: 8,
                owner: owner(1),
                address_space: second_space,
                base_address: 0x1000,
                byte_len: 16,
                alignment: 16,
                lifetime: life(0, 100),
            },
        ])
        .unwrap_err();
        assert_eq!(error.action_index, Some(1));
        assert_eq!(error.reason, MemoryErrorReasonV2::OverlappingLiveAllocation);
    }

    execute(vec![
        allocate(16),
        MemoryActionV2::Allocate {
            allocation: alloc(2),
            generation: 8,
            owner: owner(1),
            address_space: AddressSpaceV2::Workgroup,
            base_address: 0x1000,
            byte_len: 16,
            alignment: 16,
            lifetime: life(0, 100),
        },
    ])
    .unwrap();
}

#[test]
fn constant_memory_rejects_typed_raw_and_copy_destination_writes() {
    let constant_allocation =
        allocate_at(1, 7, AddressSpaceV2::Constant, 0x1000, 16, 16, life(0, 100));
    let typed_error = reason(vec![
        constant_allocation.clone(),
        MemoryActionV2::WriteTyped {
            actor: AccessActorV2::Owner(owner(1)),
            place: place(1, 0, vec![]),
            value: TypedWriteValueV2::KnownBits(1),
        },
    ]);
    assert_eq!(
        typed_error,
        MemoryErrorReasonV2::ReadOnlyAddressSpace(AddressSpaceV2::Constant)
    );

    let write_capability_error = reason(vec![
        constant_allocation,
        MemoryActionV2::GrantRawCapability {
            capability: cap(1),
            owner: owner(1),
            provenance: provenance(),
            scope: CapabilityScopeV2::Owner(owner(1)),
            range: ByteRangeV2 { start: 0, len: 4 },
            access: RawAccessV2::Write,
            lifetime: life(0, 10),
        },
    ]);
    assert_eq!(
        write_capability_error,
        MemoryErrorReasonV2::ReadOnlyAddressSpace(AddressSpaceV2::Constant)
    );
}

#[test]
fn identities_bind_policy_action_generation_and_every_fact() {
    assert_eq!(
        sha256_test_vector_v2(b""),
        [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ]
    );
    assert_eq!(
        sha256_test_vector_v2(b"abc"),
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );

    let with_generation = |generation, base_address, budgets| {
        let program = program(vec![allocate_at(
            1,
            generation,
            AddressSpaceV2::Global,
            base_address,
            16,
            16,
            life(0, 100),
        )]);
        execute_memory_program_v2(&program, budgets).unwrap()
    };
    let baseline = with_generation(7, 0x1000, MemoryBudgetsV2::default());
    let generation = with_generation(8, 0x1000, MemoryBudgetsV2::default());
    let action = with_generation(7, 0x2000, MemoryBudgetsV2::default());
    let policy = with_generation(
        7,
        0x1000,
        MemoryBudgetsV2 {
            max_execution_work: MemoryBudgetsV2::default().max_execution_work - 1,
            ..MemoryBudgetsV2::default()
        },
    );

    for substituted in [&generation, &action, &policy] {
        assert_ne!(
            baseline.untrusted_program_identity(),
            substituted.untrusted_program_identity()
        );
        assert_ne!(baseline.records(), substituted.records());
        assert_ne!(baseline.report_identity(), substituted.report_identity());
    }
    assert_eq!(
        baseline.records()[0].obligations[0].allocation_generation,
        7
    );
    assert_eq!(
        generation.records()[0].obligations[0].allocation_generation,
        8
    );
    for record in baseline.records() {
        assert_eq!(
            record.program_identity,
            *baseline.untrusted_program_identity()
        );
        for obligation in &record.obligations {
            assert_eq!(obligation.program_identity, record.program_identity);
            assert_eq!(obligation.action_identity, record.action_identity);
            assert_eq!(obligation.action_index, record.action_index);
        }
    }
}

#[test]
fn detached_identity_mutation_matrix_fails_closed() {
    let budgets = MemoryBudgetsV2::default();
    let baseline_program = program(vec![allocate(16)]);
    let execution = execute_memory_program_v2(&baseline_program, budgets).unwrap();
    assert!(
        execution
            .verify_identities(&baseline_program, budgets)
            .unwrap()
    );
    let record = execution.records()[0].clone();
    assert_ne!(record.transition_identity().digest(), &[0; 32]);
    assert!(
        record
            .verify_identity_for(
                *execution.untrusted_program_identity(),
                &baseline_program.actions()[0],
                0,
                budgets,
            )
            .unwrap()
    );
    for obligation in &record.obligations {
        assert_ne!(obligation.obligation_identity().digest(), &[0; 32]);
        assert!(obligation.verify_identity_in(&record, budgets).unwrap());
    }

    let detached = record.obligations[0].clone();
    assert_eq!(detached.obligation_index(), 0);
    let mut deleted = record.clone();
    deleted.obligations.remove(0);
    assert!(!detached.verify_identity_in(&deleted, budgets).unwrap());

    let mut substituted = record.clone();
    substituted.obligations[0] = record.obligations[1].clone();
    assert!(!detached.verify_identity_in(&substituted, budgets).unwrap());

    let mut reordered = record.clone();
    reordered.obligations.swap(0, 1);
    assert!(!detached.verify_identity_in(&reordered, budgets).unwrap());

    let mut duplicated = record.clone();
    duplicated.obligations.push(detached.clone());
    assert!(!detached.verify_identity_in(&duplicated, budgets).unwrap());

    let two_transition_program = program(vec![
        allocate(16),
        MemoryActionV2::AdvanceEpoch { to: EpochV2(1) },
    ]);
    let two_transition = execute_memory_program_v2(&two_transition_program, budgets).unwrap();
    assert!(
        !two_transition.records()[0].obligations[0]
            .verify_identity_in(&two_transition.records()[1], budgets)
            .unwrap()
    );

    let alternate_program = program(vec![allocate_at(
        1,
        9,
        AddressSpaceV2::Global,
        0x2000,
        16,
        16,
        life(0, 100),
    )]);
    let alternate = execute_memory_program_v2(&alternate_program, budgets).unwrap();
    assert!(
        !execution
            .verify_identities(&alternate_program, budgets)
            .unwrap()
    );

    let original = record.obligations[0].clone();
    let reject_obligation = |mutated: MemoryObligationV2| {
        assert!(!mutated.verify_identity_in(&record, budgets).unwrap());
        let mut detached_record = record.clone();
        detached_record.obligations[0] = mutated;
        assert!(
            !detached_record
                .verify_identity_for(
                    *execution.untrusted_program_identity(),
                    &baseline_program.actions()[0],
                    0,
                    budgets,
                )
                .unwrap()
        );
    };

    let mut mutated = original.clone();
    mutated.program_identity = *alternate.untrusted_program_identity();
    reject_obligation(mutated);
    let mut mutated = original.clone();
    mutated.action_identity = alternate.records()[0].action_identity;
    reject_obligation(mutated);
    let mut mutated = original.clone();
    mutated.action_index = 1;
    reject_obligation(mutated);
    let mut mutated = original.clone();
    mutated.kind = MemoryObligationKindV2::Aligned;
    reject_obligation(mutated);
    let mut mutated = original.clone();
    mutated.allocation = alloc(2);
    reject_obligation(mutated);
    let mut mutated = original.clone();
    mutated.allocation_generation += 1;
    reject_obligation(mutated);
    let mut mutated = original.clone();
    mutated.range.start += 1;
    reject_obligation(mutated);
    let mut mutated = original.clone();
    mutated.range.len += 1;
    reject_obligation(mutated);
    let mut mutated = original.clone();
    mutated.epoch.0 += 1;
    reject_obligation(mutated);
    let mut mutated = original;
    mutated.basis = ObligationBasisV2::ExplicitCapability;
    reject_obligation(mutated);

    for mutated_record in [
        {
            let mut value = record.clone();
            value.program_identity = *alternate.untrusted_program_identity();
            value
        },
        {
            let mut value = record.clone();
            value.action_identity = alternate.records()[0].action_identity;
            value
        },
        {
            let mut value = record.clone();
            value.action_index = 1;
            value
        },
        {
            let mut value = record.clone();
            value.obligations.swap(0, 1);
            value
        },
    ] {
        assert!(
            !mutated_record
                .verify_identity_for(
                    *execution.untrusted_program_identity(),
                    &baseline_program.actions()[0],
                    0,
                    budgets,
                )
                .unwrap()
        );
    }
}

#[test]
fn immutable_hard_caps_and_execution_work_bound_every_scan() {
    for budgets in [
        MemoryBudgetsV2 {
            max_validation_work: MemoryBudgetsV2::default().max_validation_work + 1,
            ..MemoryBudgetsV2::default()
        },
        MemoryBudgetsV2 {
            max_execution_work: MemoryBudgetsV2::default().max_execution_work + 1,
            ..MemoryBudgetsV2::default()
        },
    ] {
        assert!(matches!(
            MemoryProgramV2::new(TargetLayoutV2::gfx942_xnack_minus(), vec![], vec![], budgets)
                .unwrap_err()
                .reason,
            MemoryErrorReasonV2::ResourceLimit { resource, .. }
                if resource == "configured validation work"
                    || resource == "configured execution work"
        ));
    }

    let mut actions = vec![allocate(64)];
    for index in 1..=32 {
        actions.push(MemoryActionV2::BeginBorrow {
            loan: loan(index),
            owner: owner(1),
            place: place(1, u64::from(index), vec![]),
            kind: BorrowKindV2::Shared,
            lifetime: life(0, 10),
        });
    }
    let program = program(actions);
    let used = execute_memory_program_v2(&program, MemoryBudgetsV2::default())
        .unwrap()
        .execution_work();
    let exact = MemoryBudgetsV2 {
        max_execution_work: used,
        ..MemoryBudgetsV2::default()
    };
    assert_eq!(
        execute_memory_program_v2(&program, exact)
            .unwrap()
            .execution_work(),
        used
    );
    let error = execute_memory_program_v2(
        &program,
        MemoryBudgetsV2 {
            max_execution_work: used - 1,
            ..MemoryBudgetsV2::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error.reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "execution work",
            actual: used,
            max: used - 1,
        }
    );
}

#[test]
fn truncated_collection_counts_fail_before_reservation() {
    let empty = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        vec![],
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let canonical = empty.canonical_bytes(MemoryBudgetsV2::default()).unwrap();
    assert_eq!(canonical.len(), 59);

    let mut types = canonical[..55].to_vec();
    types[51..55].copy_from_slice(&4_096_u32.to_le_bytes());
    let type_error =
        MemoryProgramV2::decode_canonical(&types, MemoryBudgetsV2::default()).unwrap_err();
    assert_eq!(
        type_error.reason,
        MemoryErrorReasonV2::Decode {
            offset: 55,
            detail: "collection count exceeds remaining input",
        }
    );

    let mut oversized = types.clone();
    oversized[51..55].copy_from_slice(&100_000_u32.to_le_bytes());
    assert!(matches!(
        MemoryProgramV2::decode_canonical(&oversized, MemoryBudgetsV2::default())
            .unwrap_err()
            .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "types",
            actual: 100_000,
            max: 4_096,
        }
    ));

    let mut actions = canonical.clone();
    actions.truncate(59);
    actions[55..59].copy_from_slice(&65_536_u32.to_le_bytes());
    let action_error =
        MemoryProgramV2::decode_canonical(&actions, MemoryBudgetsV2::default()).unwrap_err();
    assert_eq!(
        action_error.reason,
        MemoryErrorReasonV2::Decode {
            offset: 59,
            detail: "collection count exceeds remaining input",
        }
    );

    let ranged = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        vec![scalar(
            1,
            128,
            BitValidityV2::Ranges(vec![BitValidityRangeV2 {
                start: 0,
                end_inclusive: 0,
            }]),
        )],
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let ranged_bytes = ranged.canonical_bytes(MemoryBudgetsV2::default()).unwrap();
    let mut ranges = ranged_bytes[..83].to_vec();
    ranges[79..83].copy_from_slice(&16_384_u32.to_le_bytes());
    let range_error =
        MemoryProgramV2::decode_canonical(&ranges, MemoryBudgetsV2::default()).unwrap_err();
    assert_eq!(
        range_error.reason,
        MemoryErrorReasonV2::Decode {
            offset: 83,
            detail: "collection count exceeds remaining input",
        }
    );
}

#[test]
fn aggregate_decode_limits_reject_before_nested_reservation() {
    let aggregate_types = vec![
        scalar(1, 8, BitValidityV2::Any),
        MemoryTypeV2 {
            id: ty(2),
            size: 2,
            alignment: 1,
            kind: MemoryTypeKindV2::Aggregate {
                fields: vec![
                    MemoryFieldV2 {
                        offset: 0,
                        ty: ty(1),
                    },
                    MemoryFieldV2 {
                        offset: 1,
                        ty: ty(1),
                    },
                ],
            },
        },
        MemoryTypeV2 {
            id: ty(3),
            size: 2,
            alignment: 1,
            kind: MemoryTypeKindV2::Aggregate {
                fields: vec![
                    MemoryFieldV2 {
                        offset: 0,
                        ty: ty(1),
                    },
                    MemoryFieldV2 {
                        offset: 1,
                        ty: ty(1),
                    },
                ],
            },
        },
    ];
    let aggregate = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        aggregate_types,
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let aggregate_bytes = aggregate
        .canonical_bytes(MemoryBudgetsV2::default())
        .unwrap();
    let edge_boundary = MemoryBudgetsV2 {
        max_type_edges: 4,
        ..MemoryBudgetsV2::default()
    };
    MemoryProgramV2::decode_canonical(&aggregate_bytes, edge_boundary).unwrap();
    assert_eq!(
        MemoryProgramV2::decode_canonical(
            &aggregate_bytes,
            MemoryBudgetsV2 {
                max_type_edges: 3,
                ..MemoryBudgetsV2::default()
            },
        )
        .unwrap_err()
        .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "type edges",
            actual: 4,
            max: 3,
        }
    );

    let ranged = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        vec![
            scalar(
                1,
                8,
                BitValidityV2::Ranges(vec![
                    BitValidityRangeV2 {
                        start: 0,
                        end_inclusive: 0,
                    },
                    BitValidityRangeV2 {
                        start: 2,
                        end_inclusive: 2,
                    },
                ]),
            ),
            scalar(
                2,
                8,
                BitValidityV2::Ranges(vec![
                    BitValidityRangeV2 {
                        start: 4,
                        end_inclusive: 4,
                    },
                    BitValidityRangeV2 {
                        start: 6,
                        end_inclusive: 6,
                    },
                ]),
            ),
        ],
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let ranged_bytes = ranged.canonical_bytes(MemoryBudgetsV2::default()).unwrap();
    let range_boundary = MemoryBudgetsV2 {
        max_validity_ranges: 4,
        ..MemoryBudgetsV2::default()
    };
    MemoryProgramV2::decode_canonical(&ranged_bytes, range_boundary).unwrap();
    assert_eq!(
        MemoryProgramV2::decode_canonical(
            &ranged_bytes,
            MemoryBudgetsV2 {
                max_validity_ranges: 3,
                ..MemoryBudgetsV2::default()
            },
        )
        .unwrap_err()
        .reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "validity ranges",
            actual: 4,
            max: 3,
        }
    );
}

#[test]
fn allocation_bomb_inputs_fail_before_internal_growth() {
    let empty = MemoryProgramV2::new(
        TargetLayoutV2::gfx942_xnack_minus(),
        vec![],
        vec![],
        MemoryBudgetsV2::default(),
    )
    .unwrap();
    let canonical = empty.canonical_bytes(MemoryBudgetsV2::default()).unwrap();

    let mut count_bomb = canonical.clone();
    count_bomb[51..55].copy_from_slice(&u32::MAX.to_le_bytes());
    for _ in 0..128 {
        let result = catch_unwind(AssertUnwindSafe(|| {
            MemoryProgramV2::decode_canonical(&count_bomb, MemoryBudgetsV2::default())
        }))
        .expect("count bomb panicked")
        .unwrap_err();
        assert_eq!(
            result.reason,
            MemoryErrorReasonV2::ResourceLimit {
                resource: "types",
                actual: u64::from(u32::MAX),
                max: 4_096,
            }
        );
    }

    let canonical_bomb = vec![0_u8; 16 * 1024 * 1024 + 1];
    let result = catch_unwind(AssertUnwindSafe(|| {
        MemoryProgramV2::decode_canonical(&canonical_bomb, MemoryBudgetsV2::default())
    }))
    .expect("canonical-size bomb panicked")
    .unwrap_err();
    assert_eq!(
        result.reason,
        MemoryErrorReasonV2::ResourceLimit {
            resource: "canonical bytes",
            actual: 16 * 1024 * 1024 + 1,
            max: 16 * 1024 * 1024,
        }
    );

    let mut target_name_bomb = canonical[..14].to_vec();
    target_name_bomb[12..14].copy_from_slice(&u16::MAX.to_le_bytes());
    let result = catch_unwind(AssertUnwindSafe(|| {
        MemoryProgramV2::decode_canonical(&target_name_bomb, MemoryBudgetsV2::default())
    }))
    .expect("target-name bomb panicked")
    .unwrap_err();
    assert_eq!(
        result.reason,
        MemoryErrorReasonV2::Decode {
            offset: 14,
            detail: "target name too long",
        }
    );
}

#[test]
fn u64_exclusive_end_matches_executable_range_semantics() {
    execute(vec![allocate_at(
        1,
        7,
        AddressSpaceV2::Global,
        u64::MAX,
        0,
        1,
        life(0, 100),
    )])
    .unwrap();
    assert_eq!(
        reason(vec![allocate_at(
            1,
            7,
            AddressSpaceV2::Global,
            u64::MAX,
            1,
            1,
            life(0, 100),
        )]),
        MemoryErrorReasonV2::AddressNotRepresentable
    );
}
