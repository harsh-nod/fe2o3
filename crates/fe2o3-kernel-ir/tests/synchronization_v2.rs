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
        event(
            1,
            EventKind::Fence(Fence {
                scope: MemoryScope::System,
                ordering: MemoryOrdering::Release,
                domains: MemoryDomains::GLOBAL,
            }),
        ),
        event(
            2,
            EventKind::Fence(Fence {
                scope: MemoryScope::System,
                ordering: MemoryOrdering::Acquire,
                domains: MemoryDomains::GLOBAL,
            }),
        ),
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
            },
            SynchronizationEdge {
                before: EventId(1),
                after: EventId(2),
                kind: SynchronizationEdgeKind::SynchronizesWith,
                scope: MemoryScope::System,
                domains: MemoryDomains::GLOBAL,
            },
            SynchronizationEdge {
                before: EventId(2),
                after: EventId(3),
                kind: SynchronizationEdgeKind::ProgramOrder,
                scope: MemoryScope::Wavefront,
                domains: MemoryDomains::GLOBAL,
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
            b'F', b'2', b'S', b'Y', b'N', b'C', b'V', b'2', 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
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
            required_scope: MemoryScope::Wavefront,
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
            let expected = address_space == AddressSpace::Global
                || (address_space == AddressSpace::Lds
                    && matches!(scope, MemoryScope::Wavefront | MemoryScope::Workgroup));
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
            MemoryScope::Agent,
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
                        EventKind::Fence(Fence {
                            scope: MemoryScope::System,
                            ordering: release,
                            domains: MemoryDomains::GLOBAL,
                        }),
                    ),
                    event(
                        1,
                        EventKind::Fence(Fence {
                            scope: MemoryScope::System,
                            ordering: acquire,
                            domains: MemoryDomains::GLOBAL,
                        }),
                    ),
                ];
                let mut candidate = module(events);
                candidate.edges.push(SynchronizationEdge {
                    before: EventId(0),
                    after: EventId(1),
                    kind: SynchronizationEdgeKind::SynchronizesWith,
                    scope,
                    domains: MemoryDomains::GLOBAL,
                });
                let expected = release != MemoryOrdering::Relaxed
                    && acquire != MemoryOrdering::Relaxed
                    && release != MemoryOrdering::Acquire
                    && acquire != MemoryOrdering::Release;
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
    bad_version[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        decode_synchronization_v2(&bad_version, &limits),
        Err(DecodeError::UnsupportedVersion(3))
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
fn one_hundred_sixty_thousand_hostile_decodes_are_bounded_and_panic_free() {
    let limits = SynchronizationLimits::default();
    let seed = encode_synchronization_v2(&full_module(), &limits).unwrap();
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for case in 0..160_000_u32 {
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
