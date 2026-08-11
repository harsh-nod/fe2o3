#[path = "../src/synchronization_v2.rs"]
mod synchronization_v2;

use std::panic::{AssertUnwindSafe, catch_unwind};
use synchronization_v2::*;

const ORDERINGS: [MemoryOrdering; 5] = [
    MemoryOrdering::Relaxed,
    MemoryOrdering::Acquire,
    MemoryOrdering::Release,
    MemoryOrdering::AcquireRelease,
    MemoryOrdering::SequentiallyConsistent,
];
const SCOPES: [MemoryScope; 4] = [
    MemoryScope::Wavefront,
    MemoryScope::Workgroup,
    MemoryScope::Agent,
    MemoryScope::System,
];
const OPERATIONS: [AtomicOperation; 16] = [
    AtomicOperation::Load,
    AtomicOperation::Store,
    AtomicOperation::Exchange,
    AtomicOperation::CompareExchangeStrong,
    AtomicOperation::CompareExchangeWeak,
    AtomicOperation::FetchAdd,
    AtomicOperation::FetchSub,
    AtomicOperation::FetchAnd,
    AtomicOperation::FetchOr,
    AtomicOperation::FetchXor,
    AtomicOperation::FetchNand,
    AtomicOperation::FetchMin,
    AtomicOperation::FetchMax,
    AtomicOperation::AmdInc,
    AtomicOperation::AmdDec,
    AtomicOperation::FloatAdd,
];

fn u32_ty() -> ScalarType {
    ScalarType::Integer {
        width: IntegerWidth::W32,
        signed: false,
    }
}

fn i64_ty() -> ScalarType {
    ScalarType::Integer {
        width: IntegerWidth::W64,
        signed: true,
    }
}

fn lds_allocation() -> LdsAllocation {
    LdsAllocation {
        id: LdsAllocationId(0),
        kind: LdsAllocationKind::Static,
        bytes: 4_096,
        alignment: 16,
        bank_count: 32,
        bank_width: 4,
        element_stride: 4,
        elements: 1_024,
        swizzle: LdsSwizzle::Linear,
    }
}

fn atomic(
    operation: AtomicOperation,
    value_type: ScalarType,
    address_space: AddressSpace,
    scope: MemoryScope,
    success_ordering: MemoryOrdering,
    failure_ordering: Option<MemoryOrdering>,
    dialect: AtomicDialect,
) -> AtomicAccess {
    let coherent_allocation = (address_space == AddressSpace::Global
        && scope == MemoryScope::System)
        .then_some(CoherentAllocationClaim {
            allocation: 0,
            authority: 1,
        });
    AtomicAccess {
        region: MemoryRegion {
            allocation: 0,
            offset: 0,
            bytes: value_type.storage_bytes(),
        },
        dialect,
        operation,
        value_type,
        address_space,
        alignment: value_type.storage_bytes(),
        scope,
        success_ordering,
        failure_ordering,
        coherent_allocation,
    }
}

fn event(id: u32, kind: EventKind) -> Event {
    Event {
        id: EventId(id),
        participation: ParticipationContract::invocation(),
        kind,
    }
}

fn module(events: Vec<Event>) -> SynchronizationModuleV2 {
    let needs_lds = events.iter().any(|event| match &event.kind {
        EventKind::Atomic(access) => access.address_space == AddressSpace::Lds,
        EventKind::NonAtomic(access) => access.address_space == AddressSpace::Lds,
        _ => false,
    });
    SynchronizationModuleV2 {
        target: TargetProfile::Gfx942Wave64,
        lds_allocations: needs_lds.then(lds_allocation).into_iter().collect(),
        events,
        edges: Vec::new(),
    }
}

fn atomic_module(access: AtomicAccess) -> SynchronizationModuleV2 {
    module(vec![event(0, EventKind::Atomic(access))])
}

fn valid_ordering_for(operation: AtomicOperation, success: MemoryOrdering) -> bool {
    match operation {
        AtomicOperation::Load => matches!(
            success,
            MemoryOrdering::Relaxed
                | MemoryOrdering::Acquire
                | MemoryOrdering::SequentiallyConsistent
        ),
        AtomicOperation::Store => matches!(
            success,
            MemoryOrdering::Relaxed
                | MemoryOrdering::Release
                | MemoryOrdering::SequentiallyConsistent
        ),
        _ => true,
    }
}

fn valid_failure(success: MemoryOrdering, failure: MemoryOrdering) -> bool {
    match success {
        MemoryOrdering::Relaxed => failure == MemoryOrdering::Relaxed,
        MemoryOrdering::Acquire => {
            matches!(failure, MemoryOrdering::Relaxed | MemoryOrdering::Acquire)
        }
        MemoryOrdering::Release => failure == MemoryOrdering::Relaxed,
        MemoryOrdering::AcquireRelease => {
            matches!(failure, MemoryOrdering::Relaxed | MemoryOrdering::Acquire)
        }
        MemoryOrdering::SequentiallyConsistent => matches!(
            failure,
            MemoryOrdering::Relaxed
                | MemoryOrdering::Acquire
                | MemoryOrdering::SequentiallyConsistent
        ),
    }
}

fn valid_operation_type(
    dialect: AtomicDialect,
    operation: AtomicOperation,
    value_type: ScalarType,
) -> bool {
    if !matches!(value_type.bit_width(), 32 | 64) {
        return false;
    }
    match value_type {
        ScalarType::Bool | ScalarType::Float64 => false,
        ScalarType::Float32 => {
            dialect == AtomicDialect::AmdGpu && operation == AtomicOperation::FloatAdd
        }
        ScalarType::Pointer64 => matches!(
            operation,
            AtomicOperation::Load
                | AtomicOperation::Store
                | AtomicOperation::Exchange
                | AtomicOperation::CompareExchangeStrong
                | AtomicOperation::CompareExchangeWeak
        ),
        ScalarType::Integer { width, signed } => match operation {
            AtomicOperation::FloatAdd => false,
            AtomicOperation::AmdInc | AtomicOperation::AmdDec => {
                dialect == AtomicDialect::AmdGpu && width == IntegerWidth::W32 && !signed
            }
            AtomicOperation::FetchNand => dialect == AtomicDialect::Rust,
            _ => true,
        },
    }
}

fn ordering_for(operation: AtomicOperation) -> (MemoryOrdering, Option<MemoryOrdering>) {
    match operation {
        AtomicOperation::Load => (MemoryOrdering::Acquire, None),
        AtomicOperation::Store => (MemoryOrdering::Release, None),
        AtomicOperation::CompareExchangeStrong | AtomicOperation::CompareExchangeWeak => (
            MemoryOrdering::AcquireRelease,
            Some(MemoryOrdering::Acquire),
        ),
        _ => (MemoryOrdering::AcquireRelease, None),
    }
}

fn full_module() -> SynchronizationModuleV2 {
    let global = MemoryRegion {
        allocation: 7,
        offset: 0,
        bytes: 4,
    };
    let mut release_atomic = atomic(
        AtomicOperation::Store,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::Release,
        None,
        AtomicDialect::Rust,
    );
    release_atomic.region = global;
    release_atomic
        .coherent_allocation
        .as_mut()
        .unwrap()
        .allocation = global.allocation;
    let mut acquire_atomic = atomic(
        AtomicOperation::Load,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::Acquire,
        None,
        AtomicDialect::Rust,
    );
    acquire_atomic.region = global;
    acquire_atomic
        .coherent_allocation
        .as_mut()
        .unwrap()
        .allocation = global.allocation;
    let mut events = vec![
        event(
            0,
            EventKind::NonAtomic(NonAtomicAccess {
                region: global,
                kind: AccessKind::Write,
                value_type: u32_ty(),
                address_space: AddressSpace::Global,
                alignment: 4,
            }),
        ),
        event(1, EventKind::Atomic(release_atomic)),
        event(2, EventKind::Atomic(acquire_atomic)),
        event(
            3,
            EventKind::NonAtomic(NonAtomicAccess {
                region: global,
                kind: AccessKind::Read,
                value_type: u32_ty(),
                address_space: AddressSpace::Global,
                alignment: 4,
            }),
        ),
        event(
            4,
            EventKind::Atomic(atomic(
                AtomicOperation::FetchAdd,
                u32_ty(),
                AddressSpace::Lds,
                MemoryScope::Workgroup,
                MemoryOrdering::AcquireRelease,
                None,
                AtomicDialect::Rust,
            )),
        ),
        event(
            5,
            EventKind::Atomic(atomic(
                AtomicOperation::FetchAdd,
                u32_ty(),
                AddressSpace::Lds,
                MemoryScope::Workgroup,
                MemoryOrdering::SequentiallyConsistent,
                None,
                AtomicDialect::AmdGpu,
            )),
        ),
    ];
    events.push(Event {
        id: EventId(6),
        participation: ParticipationContract {
            group: GroupKind::Workgroup,
            convergence: ConvergenceContract::UniformRequired,
            expected_participants: 256,
            active_mask: None,
        },
        kind: EventKind::Barrier(Barrier {
            kind: BarrierKind::Workgroup,
            scope: MemoryScope::Workgroup,
            ordering: MemoryOrdering::AcquireRelease,
            domains: MemoryDomains::ALL,
        }),
    });
    events.push(Event {
        id: EventId(7),
        participation: ParticipationContract::full_subgroup(64),
        kind: EventKind::Collective(Collective {
            kind: CollectiveKind::ReduceAdd,
            value_type: i64_ty(),
        }),
    });
    events.push(Event {
        id: EventId(8),
        participation: ParticipationContract::full_subgroup(64),
        kind: EventKind::Shuffle(Shuffle {
            kind: ShuffleKind::Xor,
            value_type: ScalarType::Float64,
            tile_width: 32,
        }),
    });
    events.push(Event {
        id: EventId(9),
        participation: ParticipationContract::full_subgroup(64),
        kind: EventKind::Ballot(Ballot {
            wave_size: 64,
            result_width: IntegerWidth::W64,
        }),
    });
    SynchronizationModuleV2 {
        target: TargetProfile::Gfx942Wave64,
        lds_allocations: vec![lds_allocation()],
        events,
        edges: vec![
            SynchronizationEdge {
                before: EventId(0),
                after: EventId(1),
                kind: SynchronizationEdgeKind::ProgramOrder,
                scope: MemoryScope::Wavefront,
                domains: MemoryDomains::GLOBAL,
                before_outcome: EventOutcome::Unconditional,
                after_outcome: EventOutcome::Unconditional,
                read_from: ReadFromCondition::NotApplicable,
            },
            SynchronizationEdge {
                before: EventId(1),
                after: EventId(2),
                kind: SynchronizationEdgeKind::SynchronizesWith,
                scope: MemoryScope::System,
                domains: MemoryDomains::GLOBAL,
                before_outcome: EventOutcome::Unconditional,
                after_outcome: EventOutcome::Unconditional,
                read_from: ReadFromCondition::VerifierMustProve,
            },
            SynchronizationEdge {
                before: EventId(2),
                after: EventId(3),
                kind: SynchronizationEdgeKind::ProgramOrder,
                scope: MemoryScope::Wavefront,
                domains: MemoryDomains::GLOBAL,
                before_outcome: EventOutcome::Unconditional,
                after_outcome: EventOutcome::Unconditional,
                read_from: ReadFromCondition::NotApplicable,
            },
        ],
    }
}

#[test]
fn empty_codec_golden_is_versioned_and_canonical() {
    assert_eq!(IntegerWidth::W32.bytes(), 4);
    let module = SynchronizationModuleV2 {
        target: TargetProfile::Gfx942Wave64,
        lds_allocations: vec![],
        events: vec![],
        edges: vec![],
    };
    let limits = SynchronizationLimits::default();
    let bytes = encode_synchronization_v2(&module, &limits).unwrap();
    assert_eq!(
        bytes,
        vec![
            b'F', b'2', b'S', b'Y', b'N', b'C', b'V', b'2', 4, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    assert_eq!(decode_synchronization_v2(&bytes, &limits), Ok(module));
}

#[test]
fn full_schema_round_trips_and_reports_only_dynamic_obligations() {
    let module = full_module();
    let limits = SynchronizationLimits::default();
    let report = module.validate(&limits).unwrap();
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::UniformParticipation {
            event: EventId(6),
            ..
        }
    )));
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::HappensBefore {
            kind: SynchronizationEdgeKind::SynchronizesWith,
            before: EventId(1),
            after: EventId(2),
            ..
        }
    )));
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::NonAtomicConflict {
            first: EventId(0),
            second: EventId(3),
            structurally_ordered: true,
            ..
        }
    )));
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::ScopeCompatibility {
            first: EventId(4),
            second: EventId(5),
            required_scope: MemoryScope::Workgroup,
        }
    )));
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::LdsBankMapping {
            allocation: LdsAllocationId(0),
            ..
        }
    )));
    let bytes = encode_synchronization_v2(&module, &limits).unwrap();
    assert_eq!(
        decode_synchronization_v2(&bytes, &limits),
        Ok(module.clone())
    );
    assert_eq!(encode_synchronization_v2(&module, &limits).unwrap(), bytes);
    let mut sorted = report.obligations.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(report.obligations, sorted);
}

#[test]
fn report_identity_binds_module_target_limits_and_obligation_set() {
    assert_eq!(
        sha256_test_vector(b""),
        [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ]
    );
    assert_eq!(
        sha256_test_vector(b"abc"),
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );

    let limits = SynchronizationLimits::default();
    let module = full_module();
    let report = module.validate(&limits).unwrap();
    assert_eq!(report.target, TargetProfile::Gfx942Wave64);
    assert_eq!(report.target_limits, module.target.hard_limits());
    assert_eq!(report.policy_limits, limits);
    assert!(report.verifies_module(&module, &limits).unwrap());

    let mut different_module = module.clone();
    let EventKind::NonAtomic(access) = &mut different_module.events[0].kind else {
        unreachable!()
    };
    access.kind = AccessKind::ReadWrite;
    let different_report = different_module.validate(&limits).unwrap();
    assert_ne!(report.module_digest, different_report.module_digest);
    assert_ne!(report.report_digest, different_report.report_digest);
    assert!(!report.verifies_module(&different_module, &limits).unwrap());

    let wider_encoding_limit = SynchronizationLimits {
        max_encoded_bytes: limits.max_encoded_bytes + 1,
        ..limits
    };
    let policy_report = module.validate(&wider_encoding_limit).unwrap();
    assert_eq!(report.module_digest, policy_report.module_digest);
    assert_eq!(report.obligations_digest, policy_report.obligations_digest);
    assert_ne!(report.report_digest, policy_report.report_digest);
    assert!(
        !report
            .verifies_module(&module, &wider_encoding_limit)
            .unwrap()
    );

    let mut tampered = report.clone();
    tampered.obligations.pop();
    assert!(!tampered.verifies_module(&module, &limits).unwrap());
    tampered = report.clone();
    tampered.target_limits.max_workgroup_participants += 1;
    assert!(!tampered.verifies_module(&module, &limits).unwrap());
}

#[test]
fn system_atomics_require_an_authenticated_coherence_obligation() {
    let limits = SynchronizationLimits::default();
    let mut access = atomic(
        AtomicOperation::Load,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::Acquire,
        None,
        AtomicDialect::Rust,
    );
    access.coherent_allocation = None;
    assert_eq!(
        atomic_module(access.clone()).validate(&limits),
        Err(ValidationError::InvalidCoherentAllocationClaim(EventId(0)))
    );
    access.coherent_allocation = Some(CoherentAllocationClaim {
        allocation: access.region.allocation,
        authority: 7,
    });
    let report = atomic_module(access.clone()).validate(&limits).unwrap();
    assert!(
        report
            .obligations
            .contains(&VerifierObligation::AuthenticateCoherentAllocation {
                event: EventId(0),
                allocation: access.region.allocation,
                authority: 7,
            })
    );
    access.coherent_allocation.as_mut().unwrap().allocation += 1;
    assert_eq!(
        atomic_module(access).validate(&limits),
        Err(ValidationError::InvalidCoherentAllocationClaim(EventId(0)))
    );
}

#[test]
fn overlapping_atomics_must_name_the_same_exact_object() {
    let limits = SynchronizationLimits::default();
    let first = atomic(
        AtomicOperation::FetchAdd,
        u32_ty(),
        AddressSpace::Lds,
        MemoryScope::Workgroup,
        MemoryOrdering::Relaxed,
        None,
        AtomicDialect::Rust,
    );
    let second = atomic(
        AtomicOperation::FetchAdd,
        i64_ty(),
        AddressSpace::Lds,
        MemoryScope::Workgroup,
        MemoryOrdering::Relaxed,
        None,
        AtomicDialect::Rust,
    );
    assert_eq!(
        module(vec![
            event(0, EventKind::Atomic(first.clone())),
            event(1, EventKind::Atomic(second)),
        ])
        .validate(&limits),
        Err(ValidationError::IncompatibleAtomicObject {
            first: EventId(0),
            second: EventId(1),
        })
    );

    let mut incompatible_alignment = first.clone();
    incompatible_alignment.alignment = 8;
    assert_eq!(
        module(vec![
            event(0, EventKind::Atomic(first.clone())),
            event(1, EventKind::Atomic(incompatible_alignment)),
        ])
        .validate(&limits),
        Err(ValidationError::IncompatibleAtomicObject {
            first: EventId(0),
            second: EventId(1),
        })
    );

    let mut incompatible_representation = first.clone();
    incompatible_representation.value_type = ScalarType::Integer {
        width: IntegerWidth::W32,
        signed: true,
    };
    assert_eq!(
        module(vec![
            event(0, EventKind::Atomic(first)),
            event(1, EventKind::Atomic(incompatible_representation)),
        ])
        .validate(&limits),
        Err(ValidationError::IncompatibleAtomicObject {
            first: EventId(0),
            second: EventId(1),
        })
    );
}

#[test]
fn unauthenticated_global_allocation_ids_never_establish_disjointness() {
    let limits = SynchronizationLimits::default();
    let global_access = |id, allocation, kind| {
        event(
            id,
            EventKind::NonAtomic(NonAtomicAccess {
                region: MemoryRegion {
                    allocation,
                    offset: 0,
                    bytes: 4,
                },
                kind,
                value_type: u32_ty(),
                address_space: AddressSpace::Global,
                alignment: 4,
            }),
        )
    };
    let report = module(vec![
        global_access(0, 9, AccessKind::Write),
        global_access(1, 10, AccessKind::Write),
    ])
    .validate(&limits)
    .unwrap();
    assert!(
        report
            .obligations
            .contains(&VerifierObligation::DischargeAllocationAlias {
                first: EventId(0),
                second: EventId(1),
                address_space: AddressSpace::Global,
                first_region: MemoryRegion {
                    allocation: 9,
                    offset: 0,
                    bytes: 4,
                },
                second_region: MemoryRegion {
                    allocation: 10,
                    offset: 0,
                    bytes: 4,
                },
                consequence: AllocationAliasConsequence::NonAtomicConflict,
            })
    );
    assert!(
        report
            .obligations
            .contains(&VerifierObligation::NonAtomicConflict {
                first: EventId(0),
                second: EventId(1),
                address_space: AddressSpace::Global,
                structurally_ordered: false,
                aliasing: AliasingCondition::VerifierMustProveDisjoint,
            })
    );

    let report = module(vec![
        global_access(0, 9, AccessKind::Read),
        global_access(1, 10, AccessKind::Read),
    ])
    .validate(&limits)
    .unwrap();
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::DischargeAllocationAlias {
            consequence: AllocationAliasConsequence::ReadOnlyOverlap,
            ..
        }
    )));
    assert!(
        !report
            .obligations
            .iter()
            .any(|obligation| matches!(obligation, VerifierObligation::NonAtomicConflict { .. }))
    );

    let mut first = atomic(
        AtomicOperation::FetchAdd,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::Relaxed,
        None,
        AtomicDialect::Rust,
    );
    first.region.allocation = 9;
    first.coherent_allocation.as_mut().unwrap().allocation = 9;
    let mut second = first.clone();
    second.region.allocation = 10;
    second.coherent_allocation.as_mut().unwrap().allocation = 10;
    let report = module(vec![
        event(0, EventKind::Atomic(first)),
        event(1, EventKind::Atomic(second)),
    ])
    .validate(&limits)
    .unwrap();
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::DischargeAllocationAlias {
            consequence: AllocationAliasConsequence::AtomicObjectCompatibility,
            ..
        }
    )));
}

#[test]
fn unknown_invocation_pairs_use_conservative_address_space_scope() {
    let limits = SynchronizationLimits::default();
    for (address_space, narrow_scope, required) in [
        (
            AddressSpace::Global,
            MemoryScope::Wavefront,
            MemoryScope::System,
        ),
        (
            AddressSpace::Lds,
            MemoryScope::Wavefront,
            MemoryScope::Workgroup,
        ),
    ] {
        let access = atomic(
            AtomicOperation::FetchAdd,
            u32_ty(),
            address_space,
            narrow_scope,
            MemoryOrdering::Relaxed,
            None,
            AtomicDialect::Rust,
        );
        assert_eq!(
            module(vec![
                event(0, EventKind::Atomic(access.clone())),
                event(1, EventKind::Atomic(access)),
            ])
            .validate(&limits),
            Err(ValidationError::InvalidScope(EventId(0))),
            "required {required:?}"
        );
    }
}

#[test]
fn compare_exchange_edges_require_outcome_and_read_from_preconditions() {
    let limits = SynchronizationLimits::default();
    let release = atomic(
        AtomicOperation::Store,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::Release,
        None,
        AtomicDialect::Rust,
    );
    let acquire_cas = atomic(
        AtomicOperation::CompareExchangeStrong,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::AcquireRelease,
        Some(MemoryOrdering::Relaxed),
        AtomicDialect::Rust,
    );
    let mut candidate = module(vec![
        event(0, EventKind::Atomic(release)),
        event(1, EventKind::Atomic(acquire_cas)),
    ]);
    candidate.edges.push(SynchronizationEdge {
        before: EventId(0),
        after: EventId(1),
        kind: SynchronizationEdgeKind::SynchronizesWith,
        scope: MemoryScope::System,
        domains: MemoryDomains::GLOBAL,
        before_outcome: EventOutcome::Unconditional,
        after_outcome: EventOutcome::Unconditional,
        read_from: ReadFromCondition::VerifierMustProve,
    });
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::InvalidEdgeEndpointKind(0))
    );
    candidate.edges[0].after_outcome = EventOutcome::CompareExchangeFailure;
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::InvalidEdgeEndpointKind(0))
    );
    candidate.edges[0].after_outcome = EventOutcome::CompareExchangeSuccess;
    candidate.edges[0].read_from = ReadFromCondition::NotApplicable;
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::InvalidEdgeEndpointKind(0))
    );
    candidate.edges[0].read_from = ReadFromCondition::VerifierMustProve;
    let report = candidate.validate(&limits).unwrap();
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::HappensBefore {
            after_outcome: EventOutcome::CompareExchangeSuccess,
            read_from: ReadFromCondition::VerifierMustProve,
            ..
        }
    )));

    let EventKind::Atomic(after) = &mut candidate.events[1].kind else {
        unreachable!()
    };
    after.failure_ordering = Some(MemoryOrdering::Acquire);
    candidate.edges[0].after_outcome = EventOutcome::CompareExchangeFailure;
    assert!(candidate.validate(&limits).is_ok());

    let EventKind::Atomic(before) = &mut candidate.events[0].kind else {
        unreachable!()
    };
    before.operation = AtomicOperation::CompareExchangeStrong;
    before.success_ordering = MemoryOrdering::AcquireRelease;
    before.failure_ordering = Some(MemoryOrdering::Acquire);
    let EventKind::Atomic(after) = &mut candidate.events[1].kind else {
        unreachable!()
    };
    after.operation = AtomicOperation::Load;
    after.success_ordering = MemoryOrdering::Acquire;
    after.failure_ordering = None;
    candidate.edges[0].before_outcome = EventOutcome::CompareExchangeFailure;
    candidate.edges[0].after_outcome = EventOutcome::Unconditional;
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::InvalidEdgeEndpointKind(0))
    );
    candidate.edges[0].before_outcome = EventOutcome::CompareExchangeSuccess;
    assert!(candidate.validate(&limits).is_ok());
}

#[test]
fn exhaustive_load_store_and_rmw_ordering_matrix_matches_reference() {
    let limits = SynchronizationLimits::default();
    for operation in [
        AtomicOperation::Load,
        AtomicOperation::Store,
        AtomicOperation::Exchange,
        AtomicOperation::FetchAdd,
    ] {
        for ordering in ORDERINGS {
            let access = atomic(
                operation,
                u32_ty(),
                AddressSpace::Global,
                MemoryScope::System,
                ordering,
                None,
                AtomicDialect::Rust,
            );
            assert_eq!(
                atomic_module(access).validate(&limits).is_ok(),
                valid_ordering_for(operation, ordering),
                "{operation:?} {ordering:?}"
            );
        }
    }
}

#[test]
fn exhaustive_compare_exchange_success_failure_matrix_matches_rust() {
    let limits = SynchronizationLimits::default();
    for operation in [
        AtomicOperation::CompareExchangeStrong,
        AtomicOperation::CompareExchangeWeak,
    ] {
        for success in ORDERINGS {
            let missing = atomic(
                operation,
                u32_ty(),
                AddressSpace::Global,
                MemoryScope::System,
                success,
                None,
                AtomicDialect::Rust,
            );
            assert!(atomic_module(missing).validate(&limits).is_err());
            for failure in ORDERINGS {
                let access = atomic(
                    operation,
                    u32_ty(),
                    AddressSpace::Global,
                    MemoryScope::System,
                    success,
                    Some(failure),
                    AtomicDialect::Rust,
                );
                assert_eq!(
                    atomic_module(access).validate(&limits).is_ok(),
                    valid_failure(success, failure),
                    "{operation:?} {success:?}/{failure:?}"
                );
            }
        }
    }
}

#[test]
fn every_operation_dialect_and_type_matches_independent_platform_matrix() {
    let limits = SynchronizationLimits::default();
    let types = [
        ScalarType::Bool,
        ScalarType::Integer {
            width: IntegerWidth::W8,
            signed: false,
        },
        ScalarType::Integer {
            width: IntegerWidth::W16,
            signed: true,
        },
        u32_ty(),
        ScalarType::Integer {
            width: IntegerWidth::W32,
            signed: true,
        },
        ScalarType::Integer {
            width: IntegerWidth::W64,
            signed: false,
        },
        i64_ty(),
        ScalarType::Integer {
            width: IntegerWidth::W128,
            signed: false,
        },
        ScalarType::Float32,
        ScalarType::Float64,
        ScalarType::Pointer64,
    ];
    for dialect in [AtomicDialect::Rust, AtomicDialect::AmdGpu] {
        for operation in OPERATIONS {
            for value_type in types {
                let (success, failure) = ordering_for(operation);
                let access = atomic(
                    operation,
                    value_type,
                    AddressSpace::Global,
                    MemoryScope::System,
                    success,
                    failure,
                    dialect,
                );
                assert_eq!(
                    atomic_module(access).validate(&limits).is_ok(),
                    valid_operation_type(dialect, operation, value_type),
                    "{dialect:?} {operation:?} {value_type:?}"
                );
            }
        }
    }
}

#[test]
fn exhaustive_scope_address_space_cross_product_is_fail_closed() {
    let limits = SynchronizationLimits::default();
    for address_space in [
        AddressSpace::Private,
        AddressSpace::Global,
        AddressSpace::Constant,
        AddressSpace::Lds,
        AddressSpace::Generic,
    ] {
        for scope in SCOPES {
            let access = atomic(
                AtomicOperation::FetchAdd,
                u32_ty(),
                address_space,
                scope,
                MemoryOrdering::Relaxed,
                None,
                AtomicDialect::Rust,
            );
            let expected = (address_space == AddressSpace::Global && scope == MemoryScope::System)
                || (address_space == AddressSpace::Lds && scope == MemoryScope::Workgroup);
            assert_eq!(
                atomic_module(access).validate(&limits).is_ok(),
                expected,
                "{address_space:?} {scope:?}"
            );
        }
    }
}

#[test]
fn alignment_region_and_lds_bounds_are_exact() {
    let limits = SynchronizationLimits::default();
    for alignment in [0, 1, 2, 3, 4, 8, 16, 32] {
        let mut access = atomic(
            AtomicOperation::FetchAdd,
            u32_ty(),
            AddressSpace::Global,
            MemoryScope::System,
            MemoryOrdering::Relaxed,
            None,
            AtomicDialect::Rust,
        );
        access.alignment = alignment;
        assert_eq!(
            atomic_module(access).validate(&limits).is_ok(),
            matches!(alignment, 4 | 8 | 16),
            "alignment {alignment}"
        );
    }

    let mut out_of_bounds = atomic(
        AtomicOperation::Load,
        u32_ty(),
        AddressSpace::Lds,
        MemoryScope::Workgroup,
        MemoryOrdering::Relaxed,
        None,
        AtomicDialect::Rust,
    );
    out_of_bounds.region.offset = 4_096;
    assert_eq!(
        atomic_module(out_of_bounds).validate(&limits),
        Err(ValidationError::InvalidMemoryRegion(EventId(0)))
    );
    let mut overflow = atomic(
        AtomicOperation::Load,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::Relaxed,
        None,
        AtomicDialect::Rust,
    );
    overflow.region.offset = u32::MAX - 1;
    assert_eq!(
        atomic_module(overflow).validate(&limits),
        Err(ValidationError::ArithmeticOverflow)
    );

    for (bank_count, bank_width, shift, expected) in [
        (32, 4, None, true),
        (64, 4, None, false),
        (32, 8, None, false),
        (32, 4, Some(0), false),
        (32, 4, Some(1), true),
        (32, 4, Some(5), true),
        (32, 4, Some(6), false),
    ] {
        let mut allocation = lds_allocation();
        allocation.bank_count = bank_count;
        allocation.bank_width = bank_width;
        allocation.swizzle = shift.map_or(LdsSwizzle::Linear, |shift| LdsSwizzle::Xor { shift });
        let candidate = SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: vec![allocation],
            events: vec![],
            edges: vec![],
        };
        assert_eq!(candidate.validate(&limits).is_ok(), expected);
    }
}

#[test]
fn lds_layout_accounts_for_padding_and_binds_effective_alignment() {
    let allocation = |id, bytes, alignment, elements| LdsAllocation {
        id: LdsAllocationId(id),
        kind: LdsAllocationKind::Static,
        bytes,
        alignment,
        bank_count: 32,
        bank_width: 4,
        element_stride: 4,
        elements,
        swizzle: LdsSwizzle::Linear,
    };
    let allocations = vec![
        allocation(0, 5, 1, 1),
        allocation(1, 9, 16, 2),
        allocation(2, 4, 8, 1),
    ];
    let report = SynchronizationModuleV2 {
        target: TargetProfile::Gfx942Wave64,
        lds_allocations: allocations.clone(),
        events: vec![],
        edges: vec![],
    }
    .validate(&SynchronizationLimits::default())
    .unwrap();
    let bases: Vec<_> = report
        .obligations
        .iter()
        .filter_map(|obligation| match obligation {
            VerifierObligation::LdsBankMapping {
                allocation,
                base_offset,
                ..
            } => Some((*allocation, *base_offset)),
            _ => None,
        })
        .collect();
    assert_eq!(
        bases,
        vec![
            (LdsAllocationId(0), 0),
            (LdsAllocationId(1), 16),
            (LdsAllocationId(2), 32),
        ]
    );

    let tight = SynchronizationLimits {
        max_total_lds_bytes: 35,
        ..SynchronizationLimits::default()
    };
    assert_eq!(
        SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: allocations.clone(),
            events: vec![],
            edges: vec![],
        }
        .validate(&tight),
        Err(ValidationError::ResourceLimit {
            resource: Resource::TotalLdsBytes,
            observed: 36,
            limit: 35,
        })
    );

    let mut access = atomic(
        AtomicOperation::Load,
        u32_ty(),
        AddressSpace::Lds,
        MemoryScope::Workgroup,
        MemoryOrdering::Relaxed,
        None,
        AtomicDialect::Rust,
    );
    access.region.allocation = 1;
    access.alignment = 16;
    assert!(
        SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: allocations.clone(),
            events: vec![event(0, EventKind::Atomic(access.clone()))],
            edges: vec![],
        }
        .validate(&SynchronizationLimits::default())
        .is_ok()
    );
    access.region.allocation = 0;
    access.alignment = 4;
    assert_eq!(
        SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: allocations,
            events: vec![event(0, EventKind::Atomic(access))],
            edges: vec![],
        }
        .validate(&SynchronizationLimits::default()),
        Err(ValidationError::InvalidAlignment(EventId(0)))
    );
}

#[test]
fn barriers_fences_and_participation_contracts_reject_divergence() {
    let limits = SynchronizationLimits::default();
    for ordering in ORDERINGS {
        let candidate = module(vec![event(
            0,
            EventKind::Fence(Fence {
                scope: MemoryScope::System,
                ordering,
                domains: MemoryDomains::GLOBAL,
            }),
        )]);
        assert_eq!(
            candidate.validate(&limits).is_ok(),
            ordering != MemoryOrdering::Relaxed
        );
    }

    let valid = Event {
        id: EventId(0),
        participation: ParticipationContract {
            group: GroupKind::Workgroup,
            convergence: ConvergenceContract::UniformRequired,
            expected_participants: 1_024,
            active_mask: None,
        },
        kind: EventKind::Barrier(Barrier {
            kind: BarrierKind::Workgroup,
            scope: MemoryScope::Workgroup,
            ordering: MemoryOrdering::AcquireRelease,
            domains: MemoryDomains::LDS,
        }),
    };
    assert!(module(vec![valid.clone()]).validate(&limits).is_ok());
    let mut divergent = valid.clone();
    divergent.participation.convergence = ConvergenceContract::NotRequired;
    assert!(module(vec![divergent]).validate(&limits).is_err());
    let mut too_large = valid;
    too_large.participation.expected_participants = 1_025;
    assert!(matches!(
        module(vec![too_large]).validate(&limits),
        Err(ValidationError::ResourceLimit {
            resource: Resource::WorkgroupParticipants,
            ..
        })
    ));

    for mask in [0_u64, 1, 0b1011, u64::MAX] {
        let expected = mask.count_ones();
        let event = Event {
            id: EventId(0),
            participation: ParticipationContract {
                group: GroupKind::Subgroup,
                convergence: ConvergenceContract::ExplicitMask,
                expected_participants: expected,
                active_mask: Some(mask),
            },
            kind: EventKind::Shuffle(Shuffle {
                kind: ShuffleKind::Index,
                value_type: u32_ty(),
                tile_width: 1,
            }),
        };
        assert_eq!(module(vec![event]).validate(&limits).is_ok(), expected != 0);
    }
}

#[test]
fn gfx942_hard_limits_cannot_be_widened_by_policy() {
    let hard = TargetProfile::Gfx942Wave64.hard_limits();
    assert_eq!(hard.wave_size, 64);
    assert_eq!(hard.max_lds_bytes, 65_536);
    assert_eq!(hard.max_workgroup_participants, 1_024);
    assert_eq!(hard.max_cooperative_participants, 1_048_576);

    let barrier = |participants| Event {
        id: EventId(0),
        participation: ParticipationContract {
            group: GroupKind::Workgroup,
            convergence: ConvergenceContract::UniformRequired,
            expected_participants: participants,
            active_mask: None,
        },
        kind: EventKind::Barrier(Barrier {
            kind: BarrierKind::Workgroup,
            scope: MemoryScope::Workgroup,
            ordering: MemoryOrdering::AcquireRelease,
            domains: MemoryDomains::LDS,
        }),
    };
    let widened = SynchronizationLimits {
        max_workgroup_participants: u32::MAX,
        ..SynchronizationLimits::default()
    };
    assert!(module(vec![barrier(1_024)]).validate(&widened).is_ok());
    assert_eq!(
        module(vec![barrier(1_025)]).validate(&widened),
        Err(ValidationError::ResourceLimit {
            resource: Resource::WorkgroupParticipants,
            observed: 1_025,
            limit: 1_024,
        })
    );

    let narrowed = SynchronizationLimits {
        max_workgroup_participants: 256,
        ..SynchronizationLimits::default()
    };
    assert!(module(vec![barrier(256)]).validate(&narrowed).is_ok());
    assert_eq!(
        module(vec![barrier(257)]).validate(&narrowed),
        Err(ValidationError::ResourceLimit {
            resource: Resource::WorkgroupParticipants,
            observed: 257,
            limit: 256,
        })
    );
}

#[test]
fn collective_shuffle_ballot_and_cooperative_group_matrix_is_explicit() {
    let limits = SynchronizationLimits::default();
    for (kind, value_type, expected) in [
        (CollectiveKind::Any, ScalarType::Bool, true),
        (CollectiveKind::All, u32_ty(), false),
        (CollectiveKind::ReduceAdd, u32_ty(), true),
        (CollectiveKind::ReduceMin, ScalarType::Float32, true),
        (CollectiveKind::ReduceMax, ScalarType::Float64, false),
        (CollectiveKind::Broadcast, ScalarType::Pointer64, true),
    ] {
        let event = Event {
            id: EventId(0),
            participation: ParticipationContract::full_subgroup(64),
            kind: EventKind::Collective(Collective { kind, value_type }),
        };
        assert_eq!(module(vec![event]).validate(&limits).is_ok(), expected);
    }
    for tile_width in [0, 1, 2, 3, 32, 64, 128] {
        let event = Event {
            id: EventId(0),
            participation: ParticipationContract::full_subgroup(64),
            kind: EventKind::Shuffle(Shuffle {
                kind: ShuffleKind::Down,
                value_type: i64_ty(),
                tile_width,
            }),
        };
        assert_eq!(
            module(vec![event]).validate(&limits).is_ok(),
            matches!(tile_width, 1 | 2 | 32 | 64)
        );
    }
    for (wave_size, result_width, expected) in [
        (32, IntegerWidth::W32, false),
        (64, IntegerWidth::W32, false),
        (64, IntegerWidth::W64, true),
    ] {
        let event = Event {
            id: EventId(0),
            participation: ParticipationContract::full_subgroup(64),
            kind: EventKind::Ballot(Ballot {
                wave_size,
                result_width,
            }),
        };
        assert_eq!(module(vec![event]).validate(&limits).is_ok(), expected);
    }
    let cooperative = Event {
        id: EventId(0),
        participation: ParticipationContract {
            group: GroupKind::CooperativeGrid,
            convergence: ConvergenceContract::UniformRequired,
            expected_participants: 4_096,
            active_mask: None,
        },
        kind: EventKind::Barrier(Barrier {
            kind: BarrierKind::CooperativeGroup,
            scope: MemoryScope::Agent,
            ordering: MemoryOrdering::AcquireRelease,
            domains: MemoryDomains::GLOBAL,
        }),
    };
    assert_eq!(
        module(vec![cooperative]).validate(&limits),
        Err(ValidationError::UnsupportedCooperativeGroup(EventId(0)))
    );
}

#[test]
fn edge_kind_scope_domain_and_ordering_checks_are_exhaustive() {
    let limits = SynchronizationLimits::default();
    for release in ORDERINGS {
        for acquire in ORDERINGS {
            for scope in SCOPES {
                let events = vec![
                    event(
                        0,
                        EventKind::Atomic(atomic(
                            AtomicOperation::Exchange,
                            u32_ty(),
                            AddressSpace::Global,
                            MemoryScope::System,
                            release,
                            None,
                            AtomicDialect::Rust,
                        )),
                    ),
                    event(
                        1,
                        EventKind::Atomic(atomic(
                            AtomicOperation::Exchange,
                            u32_ty(),
                            AddressSpace::Global,
                            MemoryScope::System,
                            acquire,
                            None,
                            AtomicDialect::Rust,
                        )),
                    ),
                ];
                let mut candidate = module(events);
                candidate.edges.push(SynchronizationEdge {
                    before: EventId(0),
                    after: EventId(1),
                    kind: SynchronizationEdgeKind::SynchronizesWith,
                    scope,
                    domains: MemoryDomains::GLOBAL,
                    before_outcome: EventOutcome::Unconditional,
                    after_outcome: EventOutcome::Unconditional,
                    read_from: ReadFromCondition::VerifierMustProve,
                });
                let expected = release != MemoryOrdering::Relaxed
                    && acquire != MemoryOrdering::Relaxed
                    && release != MemoryOrdering::Acquire
                    && acquire != MemoryOrdering::Release
                    && scope == MemoryScope::System;
                assert_eq!(
                    candidate.validate(&limits).is_ok(),
                    expected,
                    "{release:?} {acquire:?} {scope:?}"
                );
            }
        }
    }
    let mut candidate = full_module();
    candidate.edges[0].domains = MemoryDomains::LDS;
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::IncompatibleEdgeDomains(0))
    );
    let mut candidate = full_module();
    candidate.edges[0].domains = MemoryDomains::NONE;
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::IncompatibleEdgeDomains(0))
    );
    let mut candidate = full_module();
    candidate.edges[0].after = EventId(0);
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::BackwardOrSelfEdge(0))
    );
    let mut candidate = full_module();
    candidate.edges.swap(0, 1);
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::NonCanonicalEdgeOrder)
    );
    let mut candidate = full_module();
    candidate.edges.insert(1, candidate.edges[0].clone());
    assert_eq!(
        candidate.validate(&limits),
        Err(ValidationError::DuplicateEdge)
    );
}

#[test]
fn synchronization_witnesses_bind_endpoints_participants_and_operations() {
    let limits = SynchronizationLimits::default();
    let atomic_report = full_module().validate(&limits).unwrap();
    assert!(atomic_report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::HappensBefore {
            before: EventId(1),
            after: EventId(2),
            before_kind: EventKind::Atomic(AtomicAccess {
                operation: AtomicOperation::Store,
                ..
            }),
            after_kind: EventKind::Atomic(AtomicAccess {
                operation: AtomicOperation::Load,
                ..
            }),
            participant_witness: ParticipantWitness::SynchronizingParticipants,
            operation_witness: SynchronizationOperationWitness::AtomicReadFrom {
                region: MemoryRegion {
                    allocation: 7,
                    offset: 0,
                    bytes: 4,
                },
                before_operation: AtomicOperation::Store,
                after_operation: AtomicOperation::Load,
            },
            ..
        }
    )));

    let fences = vec![
        event(
            0,
            EventKind::Fence(Fence {
                scope: MemoryScope::System,
                ordering: MemoryOrdering::Release,
                domains: MemoryDomains::GLOBAL,
            }),
        ),
        event(
            1,
            EventKind::Fence(Fence {
                scope: MemoryScope::System,
                ordering: MemoryOrdering::Acquire,
                domains: MemoryDomains::GLOBAL,
            }),
        ),
    ];
    let mut program_order = module(fences.clone());
    program_order.edges.push(SynchronizationEdge {
        before: EventId(0),
        after: EventId(1),
        kind: SynchronizationEdgeKind::ProgramOrder,
        scope: MemoryScope::Wavefront,
        domains: MemoryDomains::GLOBAL,
        before_outcome: EventOutcome::Unconditional,
        after_outcome: EventOutcome::Unconditional,
        read_from: ReadFromCondition::NotApplicable,
    });
    let report = program_order.validate(&limits).unwrap();
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::HappensBefore {
            before_kind: EventKind::Fence(Fence {
                ordering: MemoryOrdering::Release,
                ..
            }),
            after_kind: EventKind::Fence(Fence {
                ordering: MemoryOrdering::Acquire,
                ..
            }),
            participant_witness: ParticipantWitness::SameParticipant,
            operation_witness: SynchronizationOperationWitness::ProgramOrder,
            ..
        }
    )));
    assert_eq!(
        report
            .obligations
            .iter()
            .filter(|obligation| matches!(obligation, VerifierObligation::FenceSemantics { .. }))
            .count(),
        2
    );

    let mut unsupported_direct_fence = module(fences);
    unsupported_direct_fence.edges.push(SynchronizationEdge {
        before: EventId(0),
        after: EventId(1),
        kind: SynchronizationEdgeKind::SynchronizesWith,
        scope: MemoryScope::System,
        domains: MemoryDomains::GLOBAL,
        before_outcome: EventOutcome::Unconditional,
        after_outcome: EventOutcome::Unconditional,
        read_from: ReadFromCondition::VerifierMustProve,
    });
    assert_eq!(
        unsupported_direct_fence.validate(&limits),
        Err(ValidationError::InvalidEdgeEndpointKind(0))
    );

    let barrier_event = |id, ordering, participants| Event {
        id: EventId(id),
        participation: ParticipationContract {
            group: GroupKind::Workgroup,
            convergence: ConvergenceContract::UniformRequired,
            expected_participants: participants,
            active_mask: None,
        },
        kind: EventKind::Barrier(Barrier {
            kind: BarrierKind::Workgroup,
            scope: MemoryScope::Workgroup,
            ordering,
            domains: MemoryDomains::LDS,
        }),
    };
    let mut barrier_pair = module(vec![
        barrier_event(0, MemoryOrdering::Release, 256),
        barrier_event(1, MemoryOrdering::Acquire, 256),
    ]);
    barrier_pair.edges.push(SynchronizationEdge {
        before: EventId(0),
        after: EventId(1),
        kind: SynchronizationEdgeKind::SynchronizesWith,
        scope: MemoryScope::Workgroup,
        domains: MemoryDomains::LDS,
        before_outcome: EventOutcome::Unconditional,
        after_outcome: EventOutcome::Unconditional,
        read_from: ReadFromCondition::NotApplicable,
    });
    let report = barrier_pair.validate(&limits).unwrap();
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::HappensBefore {
            participant_witness: ParticipantWitness::SameBarrierCohort,
            operation_witness: SynchronizationOperationWitness::BarrierPhase {
                kind: BarrierKind::Workgroup,
                expected_participants: 256,
            },
            ..
        }
    )));
    assert_eq!(
        report
            .obligations
            .iter()
            .filter(|obligation| matches!(obligation, VerifierObligation::BarrierSemantics { .. }))
            .count(),
        2
    );

    barrier_pair.events[1].participation.expected_participants = 64;
    assert_eq!(
        barrier_pair.validate(&limits),
        Err(ValidationError::InvalidEdgeEndpointKind(0))
    );
    barrier_pair.events[1] = event(
        1,
        EventKind::Fence(Fence {
            scope: MemoryScope::Workgroup,
            ordering: MemoryOrdering::Acquire,
            domains: MemoryDomains::LDS,
        }),
    );
    assert_eq!(
        barrier_pair.validate(&limits),
        Err(ValidationError::InvalidEdgeEndpointKind(0))
    );
}

#[test]
fn canonical_ids_and_exact_lds_capacity_are_enforced() {
    let limits = SynchronizationLimits::default();
    let mut bad_event = full_module();
    bad_event.events[0].id = EventId(9);
    assert!(matches!(
        bad_event.validate(&limits),
        Err(ValidationError::NonCanonicalEventId {
            position: 0,
            actual: EventId(9),
        })
    ));

    let mut exact = lds_allocation();
    exact.bytes = 64 * 1024;
    exact.elements = 16 * 1024;
    assert!(
        SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: vec![exact.clone()],
            events: vec![],
            edges: vec![],
        }
        .validate(&limits)
        .is_ok()
    );
    let mut tail = lds_allocation();
    tail.id = LdsAllocationId(1);
    tail.bytes = 4;
    tail.elements = 1;
    assert!(matches!(
        SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: vec![exact, tail],
            events: vec![],
            edges: vec![],
        }
        .validate(&limits),
        Err(ValidationError::ResourceLimit {
            resource: Resource::TotalLdsBytes,
            observed: 65_540,
            limit: 65_536,
        })
    ));

    let mut bad_lds = SynchronizationModuleV2 {
        target: TargetProfile::Gfx942Wave64,
        lds_allocations: vec![lds_allocation()],
        events: vec![],
        edges: vec![],
    };
    bad_lds.lds_allocations[0].id = LdsAllocationId(1);
    assert!(matches!(
        bad_lds.validate(&limits),
        Err(ValidationError::NonCanonicalLdsId {
            position: 0,
            actual: LdsAllocationId(1),
        })
    ));
}

#[test]
fn resource_limits_preflight_before_quadratic_or_allocating_work() {
    let limits = SynchronizationLimits {
        max_events: 3,
        ..SynchronizationLimits::default()
    };
    let events = (0..4)
        .map(|id| {
            event(
                id,
                EventKind::NonAtomic(NonAtomicAccess {
                    region: MemoryRegion {
                        allocation: id,
                        offset: 0,
                        bytes: 4,
                    },
                    kind: AccessKind::Read,
                    value_type: u32_ty(),
                    address_space: AddressSpace::Global,
                    alignment: 4,
                }),
            )
        })
        .collect();
    assert!(matches!(
        module(events).validate(&limits),
        Err(ValidationError::ResourceLimit {
            resource: Resource::Events,
            observed: 4,
            limit: 3,
        })
    ));

    let limits = SynchronizationLimits {
        max_pair_checks: 5,
        ..SynchronizationLimits::default()
    };
    let events = (0..4)
        .map(|id| {
            event(
                id,
                EventKind::Fence(Fence {
                    scope: MemoryScope::System,
                    ordering: MemoryOrdering::AcquireRelease,
                    domains: MemoryDomains::GLOBAL,
                }),
            )
        })
        .collect();
    assert!(matches!(
        module(events).validate(&limits),
        Err(ValidationError::ResourceLimit {
            resource: Resource::PairChecks,
            observed: 6,
            limit: 5,
        })
    ));

    let limits = SynchronizationLimits {
        max_obligations: 1,
        ..SynchronizationLimits::default()
    };
    assert!(matches!(
        full_module().validate(&limits),
        Err(ValidationError::ResourceLimit {
            resource: Resource::Obligations,
            ..
        })
    ));
}

#[test]
fn codec_rejects_every_truncation_count_bombs_and_noncanonical_fields() {
    let limits = SynchronizationLimits::default();
    let bytes = encode_synchronization_v2(&full_module(), &limits).unwrap();
    for length in 0..bytes.len() {
        assert!(
            decode_synchronization_v2(&bytes[..length], &limits).is_err(),
            "truncation {length}"
        );
    }

    let empty = encode_synchronization_v2(
        &SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: vec![],
            events: vec![],
            edges: vec![],
        },
        &limits,
    )
    .unwrap();
    for (offset, count, resource) in [
        (20, limits.max_lds_allocations + 1, Resource::LdsAllocations),
        (24, limits.max_events + 1, Resource::Events),
        (28, limits.max_edges + 1, Resource::Edges),
    ] {
        let mut bomb = empty.clone();
        bomb[offset..offset + 4].copy_from_slice(&count.to_le_bytes());
        assert!(matches!(
            decode_synchronization_v2(&bomb, &limits),
            Err(DecodeError::ResourceLimit { resource: actual, .. }) if actual == resource
        ));
    }

    let mut bad_version = empty.clone();
    bad_version[8..10].copy_from_slice(&5_u16.to_le_bytes());
    assert_eq!(
        decode_synchronization_v2(&bad_version, &limits),
        Err(DecodeError::UnsupportedVersion(5))
    );
    let mut bad_target = empty.clone();
    bad_target[16] = 0xff;
    assert_eq!(
        decode_synchronization_v2(&bad_target, &limits),
        Err(DecodeError::UnknownTag)
    );
    let mut reserved = empty;
    reserved[17] = 1;
    assert_eq!(
        decode_synchronization_v2(&reserved, &limits),
        Err(DecodeError::NonZeroReserved)
    );

    let tiny = SynchronizationLimits {
        max_encoded_bytes: 31,
        ..SynchronizationLimits::default()
    };
    let empty_module = SynchronizationModuleV2 {
        target: TargetProfile::Gfx942Wave64,
        lds_allocations: vec![],
        events: vec![],
        edges: vec![],
    };
    assert!(matches!(
        encode_synchronization_v2(&empty_module, &tiny),
        Err(ValidationError::ResourceLimit {
            resource: Resource::EncodedBytes,
            observed: 32,
            limit: 31,
        })
    ));
    let encoded = encode_synchronization_v2(&empty_module, &limits).unwrap();
    assert!(matches!(
        decode_synchronization_v2(&encoded, &tiny),
        Err(DecodeError::ResourceLimit {
            resource: Resource::EncodedBytes,
            observed: 32,
            limit: 31,
        })
    ));
}

#[test]
fn one_hundred_thousand_semantic_cases_match_independent_oracle() {
    let limits = SynchronizationLimits::default();
    let types = [
        ScalarType::Bool,
        ScalarType::Integer {
            width: IntegerWidth::W8,
            signed: false,
        },
        ScalarType::Integer {
            width: IntegerWidth::W16,
            signed: true,
        },
        u32_ty(),
        i64_ty(),
        ScalarType::Integer {
            width: IntegerWidth::W128,
            signed: false,
        },
        ScalarType::Float32,
        ScalarType::Float64,
        ScalarType::Pointer64,
    ];
    let mut state = 0x8c26_7d31_42a9_55e1_u64;
    for _ in 0..100_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let dialect = if state & 1 == 0 {
            AtomicDialect::Rust
        } else {
            AtomicDialect::AmdGpu
        };
        let operation = OPERATIONS[((state >> 8) as usize) % OPERATIONS.len()];
        let value_type = types[((state >> 16) as usize) % types.len()];
        let success = ORDERINGS[((state >> 24) as usize) % ORDERINGS.len()];
        let failure = operation
            .compare_exchange_for_test()
            .then(|| ORDERINGS[((state >> 32) as usize) % ORDERINGS.len()]);
        let access = atomic(
            operation,
            value_type,
            AddressSpace::Global,
            MemoryScope::System,
            success,
            failure,
            dialect,
        );
        let expected = valid_operation_type(dialect, operation, value_type)
            && valid_ordering_for(operation, success)
            && (!operation.compare_exchange_for_test() || valid_failure(success, failure.unwrap()));
        assert_eq!(atomic_module(access).validate(&limits).is_ok(), expected);
    }
}

#[test]
fn one_hundred_twenty_thousand_repair_cases_match_independent_oracle() {
    let limits = SynchronizationLimits::default();
    let mut state = 0xd1b5_4a32_d192_ed03_u64;
    for case in 0..120_000 {
        state = state
            .wrapping_mul(2_862_933_555_777_941_757)
            .wrapping_add(3_037_000_493);
        let (candidate, expected) = match case % 4 {
            0 => {
                let scope = SCOPES[((state >> 8) as usize) % SCOPES.len()];
                let mut access = atomic(
                    AtomicOperation::FetchAdd,
                    u32_ty(),
                    AddressSpace::Global,
                    scope,
                    MemoryOrdering::Relaxed,
                    None,
                    AtomicDialect::Rust,
                );
                let mode = ((state >> 16) & 3) as u8;
                access.coherent_allocation = match mode {
                    0 => None,
                    1 => Some(CoherentAllocationClaim {
                        allocation: access.region.allocation,
                        authority: 9,
                    }),
                    2 => Some(CoherentAllocationClaim {
                        allocation: access.region.allocation + 1,
                        authority: 9,
                    }),
                    _ => Some(CoherentAllocationClaim {
                        allocation: access.region.allocation,
                        authority: 0,
                    }),
                };
                let expected = scope == MemoryScope::System && mode == 1;
                (atomic_module(access), expected)
            }
            1 => {
                let left = atomic(
                    AtomicOperation::FetchAdd,
                    u32_ty(),
                    AddressSpace::Lds,
                    MemoryScope::Workgroup,
                    MemoryOrdering::Relaxed,
                    None,
                    AtomicDialect::Rust,
                );
                let mut right = left.clone();
                let mode = ((state >> 24) & 3) as u8;
                match mode {
                    0 => {}
                    1 => {
                        right.value_type = i64_ty();
                        right.region.bytes = 8;
                        right.alignment = 8;
                    }
                    2 => right.alignment = 8,
                    _ => {
                        right.value_type = ScalarType::Integer {
                            width: IntegerWidth::W32,
                            signed: true,
                        };
                    }
                }
                (
                    module(vec![
                        event(0, EventKind::Atomic(left)),
                        event(1, EventKind::Atomic(right)),
                    ]),
                    mode == 0,
                )
            }
            2 => {
                let address_space = if state & 1 == 0 {
                    AddressSpace::Global
                } else {
                    AddressSpace::Lds
                };
                let scope = if address_space == AddressSpace::Global {
                    SCOPES[((state >> 32) as usize) % SCOPES.len()]
                } else if state & 2 == 0 {
                    MemoryScope::Wavefront
                } else {
                    MemoryScope::Workgroup
                };
                let access = atomic(
                    AtomicOperation::FetchAdd,
                    u32_ty(),
                    address_space,
                    scope,
                    MemoryOrdering::Relaxed,
                    None,
                    AtomicDialect::Rust,
                );
                let expected = match address_space {
                    AddressSpace::Global => scope == MemoryScope::System,
                    AddressSpace::Lds => scope == MemoryScope::Workgroup,
                    _ => unreachable!(),
                };
                (
                    module(vec![
                        event(0, EventKind::Atomic(access.clone())),
                        event(1, EventKind::Atomic(access)),
                    ]),
                    expected,
                )
            }
            _ => {
                let failure = if state & 1 == 0 {
                    MemoryOrdering::Relaxed
                } else {
                    MemoryOrdering::Acquire
                };
                let outcome = match (state >> 40) % 3 {
                    0 => EventOutcome::Unconditional,
                    1 => EventOutcome::CompareExchangeSuccess,
                    _ => EventOutcome::CompareExchangeFailure,
                };
                let read_from = if state & 4 == 0 {
                    ReadFromCondition::NotApplicable
                } else {
                    ReadFromCondition::VerifierMustProve
                };
                let mut candidate = module(vec![
                    event(
                        0,
                        EventKind::Atomic(atomic(
                            AtomicOperation::Store,
                            u32_ty(),
                            AddressSpace::Global,
                            MemoryScope::System,
                            MemoryOrdering::Release,
                            None,
                            AtomicDialect::Rust,
                        )),
                    ),
                    event(
                        1,
                        EventKind::Atomic(atomic(
                            AtomicOperation::CompareExchangeStrong,
                            u32_ty(),
                            AddressSpace::Global,
                            MemoryScope::System,
                            MemoryOrdering::AcquireRelease,
                            Some(failure),
                            AtomicDialect::Rust,
                        )),
                    ),
                ]);
                candidate.edges.push(SynchronizationEdge {
                    before: EventId(0),
                    after: EventId(1),
                    kind: SynchronizationEdgeKind::SynchronizesWith,
                    scope: MemoryScope::System,
                    domains: MemoryDomains::GLOBAL,
                    before_outcome: EventOutcome::Unconditional,
                    after_outcome: outcome,
                    read_from,
                });
                let expected = read_from == ReadFromCondition::VerifierMustProve
                    && match outcome {
                        EventOutcome::CompareExchangeSuccess => true,
                        EventOutcome::CompareExchangeFailure => failure == MemoryOrdering::Acquire,
                        EventOutcome::Unconditional => false,
                    };
                (candidate, expected)
            }
        };
        assert_eq!(
            candidate.validate(&limits).is_ok(),
            expected,
            "case={case} state={state:#x}"
        );
    }
}

#[test]
fn two_hundred_thousand_review_repairs_match_independent_oracles() {
    let mut state = 0x6a09_e667_f3bc_c908_u64;
    for case in 0..200_000_u32 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        match case % 4 {
            0 => {
                let alignments = [1, 2, 4, 8, 16];
                let first_bytes = 4 * (1 + ((state >> 8) as u32 % 32));
                let second_bytes = 4 * (1 + ((state >> 16) as u32 % 32));
                let second_alignment = alignments[((state >> 24) as usize) % alignments.len()];
                let limit = 1 + ((state >> 32) as u32 % 320);
                let allocation = |id, bytes, alignment| LdsAllocation {
                    id: LdsAllocationId(id),
                    kind: LdsAllocationKind::Static,
                    bytes,
                    alignment,
                    bank_count: 32,
                    bank_width: 4,
                    element_stride: 4,
                    elements: bytes / 4,
                    swizzle: LdsSwizzle::Linear,
                };
                let candidate = SynchronizationModuleV2 {
                    target: TargetProfile::Gfx942Wave64,
                    lds_allocations: vec![
                        allocation(0, first_bytes, 1),
                        allocation(1, second_bytes, second_alignment),
                    ],
                    events: vec![],
                    edges: vec![],
                };
                let padding =
                    (second_alignment - first_bytes % second_alignment) % second_alignment;
                let extent = first_bytes + padding + second_bytes;
                let limits = SynchronizationLimits {
                    max_total_lds_bytes: limit,
                    ..SynchronizationLimits::default()
                };
                assert_eq!(
                    candidate.validate(&limits).is_ok(),
                    extent <= limit,
                    "LDS case={case} extent={extent} limit={limit}"
                );
            }
            1 => {
                let alignments = [1, 2, 4, 8, 16];
                let allocation_alignment = alignments[((state >> 8) as usize) % alignments.len()];
                let access_alignment = alignments[((state >> 16) as usize) % alignments.len()];
                let offset = ((state >> 24) as u32) % 12;
                let mut access = atomic(
                    AtomicOperation::Load,
                    u32_ty(),
                    AddressSpace::Lds,
                    MemoryScope::Workgroup,
                    MemoryOrdering::Relaxed,
                    None,
                    AtomicDialect::Rust,
                );
                access.region.allocation = 1;
                access.region.offset = offset;
                access.alignment = access_alignment;
                let allocation = |id, bytes, alignment, elements| LdsAllocation {
                    id: LdsAllocationId(id),
                    kind: LdsAllocationKind::Static,
                    bytes,
                    alignment,
                    bank_count: 32,
                    bank_width: 4,
                    element_stride: 4,
                    elements,
                    swizzle: LdsSwizzle::Linear,
                };
                let candidate = SynchronizationModuleV2 {
                    target: TargetProfile::Gfx942Wave64,
                    lds_allocations: vec![
                        allocation(0, 5, 1, 1),
                        allocation(1, 64, allocation_alignment, 16),
                    ],
                    events: vec![event(0, EventKind::Atomic(access))],
                    edges: vec![],
                };
                let second_base =
                    5 + (allocation_alignment - 5 % allocation_alignment) % allocation_alignment;
                let expected = access_alignment >= 4
                    && allocation_alignment >= access_alignment
                    && (second_base + offset).is_multiple_of(access_alignment);
                assert_eq!(
                    candidate
                        .validate(&SynchronizationLimits::default())
                        .is_ok(),
                    expected,
                    "alignment case={case} base={second_base} offset={offset} allocation_alignment={allocation_alignment} access_alignment={access_alignment}"
                );
            }
            2 => {
                let same_allocation = state & 1 == 0;
                let same_offset = state & 2 == 0;
                let first_writes = state & 4 != 0;
                let second_writes = state & 8 != 0;
                let access = |id, allocation, offset, writes| {
                    event(
                        id,
                        EventKind::NonAtomic(NonAtomicAccess {
                            region: MemoryRegion {
                                allocation,
                                offset,
                                bytes: 4,
                            },
                            kind: if writes {
                                AccessKind::Write
                            } else {
                                AccessKind::Read
                            },
                            value_type: u32_ty(),
                            address_space: AddressSpace::Global,
                            alignment: 4,
                        }),
                    )
                };
                let report = module(vec![
                    access(0, 9, 0, first_writes),
                    access(
                        1,
                        if same_allocation { 9 } else { 10 },
                        if same_offset { 0 } else { 8 },
                        second_writes,
                    ),
                ])
                .validate(&SynchronizationLimits::default())
                .unwrap();
                let has_alias = report.obligations.iter().any(|obligation| {
                    matches!(
                        obligation,
                        VerifierObligation::DischargeAllocationAlias { .. }
                    )
                });
                let has_conflict = report.obligations.iter().any(|obligation| {
                    matches!(obligation, VerifierObligation::NonAtomicConflict { .. })
                });
                let writes = first_writes || second_writes;
                assert_eq!(has_alias, !same_allocation, "alias case={case}");
                assert_eq!(
                    has_conflict,
                    writes && (!same_allocation || same_offset),
                    "conflict case={case}"
                );
            }
            _ => {
                let participants = 1 + ((state >> 8) as u32 % 1_500);
                let policy_limit = 1 + ((state >> 24) as u32 % 2_048);
                let candidate = module(vec![Event {
                    id: EventId(0),
                    participation: ParticipationContract {
                        group: GroupKind::Workgroup,
                        convergence: ConvergenceContract::UniformRequired,
                        expected_participants: participants,
                        active_mask: None,
                    },
                    kind: EventKind::Barrier(Barrier {
                        kind: BarrierKind::Workgroup,
                        scope: MemoryScope::Workgroup,
                        ordering: MemoryOrdering::AcquireRelease,
                        domains: MemoryDomains::LDS,
                    }),
                }]);
                let limits = SynchronizationLimits {
                    max_workgroup_participants: policy_limit,
                    ..SynchronizationLimits::default()
                };
                assert_eq!(
                    candidate.validate(&limits).is_ok(),
                    participants <= policy_limit.min(1_024),
                    "target-limit case={case} participants={participants} policy={policy_limit}"
                );
            }
        }
    }
}

#[test]
fn synchronization_endpoint_witness_matrix_is_exhaustive() {
    #[derive(Clone, Copy, Debug)]
    enum Endpoint {
        Atomic,
        Fence,
        Barrier,
    }
    let endpoints = [Endpoint::Atomic, Endpoint::Fence, Endpoint::Barrier];
    let make_event = |id, endpoint, release, participants| {
        let ordering = if release {
            MemoryOrdering::Release
        } else {
            MemoryOrdering::Acquire
        };
        match endpoint {
            Endpoint::Atomic => event(
                id,
                EventKind::Atomic(atomic(
                    AtomicOperation::Exchange,
                    u32_ty(),
                    AddressSpace::Global,
                    MemoryScope::System,
                    ordering,
                    None,
                    AtomicDialect::Rust,
                )),
            ),
            Endpoint::Fence => event(
                id,
                EventKind::Fence(Fence {
                    scope: MemoryScope::System,
                    ordering,
                    domains: MemoryDomains::GLOBAL,
                }),
            ),
            Endpoint::Barrier => Event {
                id: EventId(id),
                participation: ParticipationContract {
                    group: GroupKind::Workgroup,
                    convergence: ConvergenceContract::UniformRequired,
                    expected_participants: participants,
                    active_mask: None,
                },
                kind: EventKind::Barrier(Barrier {
                    kind: BarrierKind::Workgroup,
                    scope: MemoryScope::Workgroup,
                    ordering,
                    domains: MemoryDomains::LDS,
                }),
            },
        }
    };
    let mut cases = 0;
    for before_endpoint in endpoints {
        for after_endpoint in endpoints {
            for read_from in [
                ReadFromCondition::NotApplicable,
                ReadFromCondition::VerifierMustProve,
            ] {
                for same_cohort in [false, true] {
                    let participants = if same_cohort { 64 } else { 32 };
                    let before = make_event(0, before_endpoint, true, 64);
                    let after = make_event(1, after_endpoint, false, participants);
                    let barrier_pair = matches!(before_endpoint, Endpoint::Barrier)
                        && matches!(after_endpoint, Endpoint::Barrier);
                    let atomic_pair = matches!(before_endpoint, Endpoint::Atomic)
                        && matches!(after_endpoint, Endpoint::Atomic);
                    let domains = if barrier_pair {
                        MemoryDomains::LDS
                    } else {
                        MemoryDomains::GLOBAL
                    };
                    let scope = if barrier_pair {
                        MemoryScope::Workgroup
                    } else {
                        MemoryScope::System
                    };
                    let mut candidate = module(vec![before, after]);
                    candidate.edges.push(SynchronizationEdge {
                        before: EventId(0),
                        after: EventId(1),
                        kind: SynchronizationEdgeKind::SynchronizesWith,
                        scope,
                        domains,
                        before_outcome: EventOutcome::Unconditional,
                        after_outcome: EventOutcome::Unconditional,
                        read_from,
                    });
                    let expected = (atomic_pair
                        && read_from == ReadFromCondition::VerifierMustProve)
                        || (barrier_pair
                            && same_cohort
                            && read_from == ReadFromCondition::NotApplicable);
                    assert_eq!(
                        candidate
                            .validate(&SynchronizationLimits::default())
                            .is_ok(),
                        expected,
                        "before={before_endpoint:?} after={after_endpoint:?} read_from={read_from:?} same_cohort={same_cohort}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 36);
}

trait AtomicOperationTestExt {
    fn compare_exchange_for_test(self) -> bool;
}

impl AtomicOperationTestExt for AtomicOperation {
    fn compare_exchange_for_test(self) -> bool {
        matches!(
            self,
            Self::CompareExchangeStrong | Self::CompareExchangeWeak
        )
    }
}

#[test]
fn two_hundred_fifty_thousand_hostile_decodes_are_bounded_and_panic_free() {
    let limits = SynchronizationLimits::default();
    let seed = encode_synchronization_v2(&full_module(), &limits).unwrap();
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for case in 0..250_000_u32 {
        state = state
            .wrapping_mul(2_862_933_555_777_941_757)
            .wrapping_add(3_037_000_493);
        let mut candidate = if case & 1 == 0 {
            seed.clone()
        } else {
            vec![0; ((state >> 24) as usize) % 384]
        };
        if !candidate.is_empty() {
            let edits = 1 + ((state >> 48) as usize & 3);
            for edit in 0..edits {
                state = state
                    .rotate_left(17)
                    .wrapping_add(u64::from(case) + edit as u64);
                let index = (state as usize) % candidate.len();
                candidate[index] ^= (state >> 56) as u8 | 1;
            }
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            decode_synchronization_v2(&candidate, &limits)
        }));
        assert!(result.is_ok(), "decoder panic in case {case}");
        if let Ok(Ok(decoded)) = result {
            assert_eq!(
                encode_synchronization_v2(&decoded, &limits).unwrap(),
                candidate
            );
        }
    }
}

#[test]
fn schema_is_inert_and_makes_no_race_or_lowering_claim() {
    let public_root = include_str!("../src/lib.rs");
    assert!(!public_root.contains("synchronization_v2"));
    assert!(SYNCHRONIZATION_V2_LIMITATIONS.contains("no LLVM emission"));
    assert!(SYNCHRONIZATION_V2_LIMITATIONS.contains("race-freedom proof"));
    assert!(SYNCHRONIZATION_V2_LIMITATIONS.contains("uniformity proof"));
    assert!(SYNCHRONIZATION_V2_LIMITATIONS.contains("happens-before proof"));
}

#[test]
fn review_counterexamples_are_repaired_incrementally() {
    let limits = SynchronizationLimits::default();

    let allocations = (0..508)
        .map(|id| LdsAllocation {
            id: LdsAllocationId(id),
            kind: LdsAllocationKind::Static,
            bytes: 129,
            alignment: 16,
            bank_count: 32,
            bank_width: 4,
            element_stride: 4,
            elements: 32,
            swizzle: LdsSwizzle::Linear,
        })
        .collect();
    assert_eq!(
        SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: allocations,
            events: vec![],
            edges: vec![],
        }
        .validate(&limits),
        Err(ValidationError::ResourceLimit {
            resource: Resource::TotalLdsBytes,
            observed: 73_137,
            limit: 65_536,
        })
    );

    let mut under_aligned = lds_allocation();
    under_aligned.alignment = 1;
    let mut lds_access = atomic(
        AtomicOperation::Load,
        i64_ty(),
        AddressSpace::Lds,
        MemoryScope::Workgroup,
        MemoryOrdering::Relaxed,
        None,
        AtomicDialect::Rust,
    );
    lds_access.alignment = 8;
    assert_eq!(
        SynchronizationModuleV2 {
            target: TargetProfile::Gfx942Wave64,
            lds_allocations: vec![under_aligned],
            events: vec![event(0, EventKind::Atomic(lds_access))],
            edges: vec![],
        }
        .validate(&limits),
        Err(ValidationError::InvalidAlignment(EventId(0)))
    );

    let writes = [9, 10]
        .into_iter()
        .enumerate()
        .map(|(id, allocation)| {
            event(
                id as u32,
                EventKind::NonAtomic(NonAtomicAccess {
                    region: MemoryRegion {
                        allocation,
                        offset: 0,
                        bytes: 4,
                    },
                    kind: AccessKind::Write,
                    value_type: u32_ty(),
                    address_space: AddressSpace::Global,
                    alignment: 4,
                }),
            )
        })
        .collect();
    let report = module(writes).validate(&limits).unwrap();
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::DischargeAllocationAlias {
            first: EventId(0),
            second: EventId(1),
            consequence: AllocationAliasConsequence::NonAtomicConflict,
            ..
        }
    )));
    assert!(report.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::NonAtomicConflict {
            first: EventId(0),
            second: EventId(1),
            aliasing: AliasingCondition::VerifierMustProveDisjoint,
            ..
        }
    )));

    let widened = SynchronizationLimits {
        max_workgroup_participants: 2_048,
        ..limits
    };
    let oversized = Event {
        id: EventId(0),
        participation: ParticipationContract {
            group: GroupKind::Workgroup,
            convergence: ConvergenceContract::UniformRequired,
            expected_participants: 1_025,
            active_mask: None,
        },
        kind: EventKind::Barrier(Barrier {
            kind: BarrierKind::Workgroup,
            scope: MemoryScope::Workgroup,
            ordering: MemoryOrdering::AcquireRelease,
            domains: MemoryDomains::LDS,
        }),
    };
    assert_eq!(
        module(vec![oversized]).validate(&widened),
        Err(ValidationError::ResourceLimit {
            resource: Resource::WorkgroupParticipants,
            observed: 1_025,
            limit: 1_024,
        })
    );

    let load_report = atomic_module(atomic(
        AtomicOperation::Load,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::Acquire,
        None,
        AtomicDialect::Rust,
    ))
    .validate(&limits)
    .unwrap();
    let store_report = atomic_module(atomic(
        AtomicOperation::Store,
        u32_ty(),
        AddressSpace::Global,
        MemoryScope::System,
        MemoryOrdering::Release,
        None,
        AtomicDialect::Rust,
    ))
    .validate(&limits)
    .unwrap();
    assert_eq!(load_report.obligations, store_report.obligations);
    assert_eq!(
        load_report.obligations_digest,
        store_report.obligations_digest
    );
    assert_ne!(load_report.module_digest, store_report.module_digest);
    assert_ne!(load_report.report_digest, store_report.report_digest);

    let collective_add = module(vec![Event {
        id: EventId(0),
        participation: ParticipationContract::full_subgroup(64),
        kind: EventKind::Collective(Collective {
            kind: CollectiveKind::ReduceAdd,
            value_type: u32_ty(),
        }),
    }])
    .validate(&limits)
    .unwrap();
    let collective_min = module(vec![Event {
        id: EventId(0),
        participation: ParticipationContract::full_subgroup(64),
        kind: EventKind::Collective(Collective {
            kind: CollectiveKind::ReduceMin,
            value_type: u32_ty(),
        }),
    }])
    .validate(&limits)
    .unwrap();
    assert_ne!(collective_add.obligations, collective_min.obligations);
    assert_ne!(
        collective_add.obligations_digest,
        collective_min.obligations_digest
    );
    assert!(collective_add.obligations.iter().any(|obligation| matches!(
        obligation,
        VerifierObligation::CollectiveSemantics {
            collective: Collective {
                kind: CollectiveKind::ReduceAdd,
                ..
            },
            ..
        }
    )));
    assert_ne!(collective_add.module_digest, collective_min.module_digest);
    assert_ne!(collective_add.report_digest, collective_min.report_digest);

    let fences = vec![
        event(
            0,
            EventKind::Fence(Fence {
                scope: MemoryScope::System,
                ordering: MemoryOrdering::Release,
                domains: MemoryDomains::GLOBAL,
            }),
        ),
        event(
            1,
            EventKind::Fence(Fence {
                scope: MemoryScope::System,
                ordering: MemoryOrdering::Acquire,
                domains: MemoryDomains::GLOBAL,
            }),
        ),
    ];
    let mut fence_pair = module(fences);
    fence_pair.edges.push(SynchronizationEdge {
        before: EventId(0),
        after: EventId(1),
        kind: SynchronizationEdgeKind::SynchronizesWith,
        scope: MemoryScope::System,
        domains: MemoryDomains::GLOBAL,
        before_outcome: EventOutcome::Unconditional,
        after_outcome: EventOutcome::Unconditional,
        read_from: ReadFromCondition::NotApplicable,
    });
    assert_eq!(
        fence_pair.validate(&limits),
        Err(ValidationError::InvalidEdgeEndpointKind(0))
    );
}
