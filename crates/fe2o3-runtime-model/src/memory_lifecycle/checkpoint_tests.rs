use super::*;
use alloc::vec;

// The pre-fast-path implementation is intentionally independent of the production branch.
fn legacy_checkpoint(
    source: &MemoryLifecycleStateV1,
) -> Result<MemoryLifecycleStateV1, MemoryTransitionErrorV1> {
    source
        .validate_global_invariants()
        .map_err(MemoryTransitionErrorV1::SourceInvariant)?;
    if source.identity_discipline != MemoryIdentityDisciplineV1::MonotonicNonReusable {
        return Err(MemoryTransitionErrorV1::CheckpointRequiresMonotonicIdentities);
    }
    let allocations: Vec<_> = source
        .allocations
        .iter()
        .filter(|record| record.state == MemoryAllocationStateV1::Released)
        .map(|record| record.key)
        .collect();
    let mappings: Vec<_> = source
        .mappings
        .iter()
        .filter(|record| record.state == MemoryMappingStateV1::Released)
        .map(|record| record.key)
        .collect();
    let publications: Vec<_> = source
        .publications
        .iter()
        .filter(|record| record.state == MemoryPublicationStateV1::Released)
        .map(|record| record.key)
        .collect();
    let reservations: Vec<_> = source
        .reservations
        .iter()
        .filter(|record| {
            record.state == VaReservationStateV1::Released
                && source.allocations.iter().all(|allocation| {
                    allocation.reservation != record.key || allocations.contains(&allocation.key)
                })
        })
        .map(|record| record.key)
        .collect();
    let mut result = source.clone();
    result
        .publications
        .retain(|record| !publications.contains(&record.key));
    result
        .mappings
        .retain(|record| !mappings.contains(&record.key));
    result
        .allocations
        .retain(|record| !allocations.contains(&record.key));
    result
        .reservations
        .retain(|record| !reservations.contains(&record.key));
    let retained_allocations: Vec<_> = result.allocations.iter().map(|record| record.key).collect();
    let retained_mappings: Vec<_> = result.mappings.iter().map(|record| record.key).collect();
    result
        .issued_id_high_watermarks
        .retain(|watermark| match watermark.scope {
            MemoryIssuedIdScopeV1::Mapping(allocation) => {
                retained_allocations.contains(&allocation)
            }
            MemoryIssuedIdScopeV1::Publication(mapping) => retained_mappings.contains(&mapping),
            MemoryIssuedIdScopeV1::VaReservation(_) | MemoryIssuedIdScopeV1::Allocation(_) => true,
        });
    result
        .validate_global_invariants()
        .map_err(MemoryTransitionErrorV1::NextInvariant)?;
    Ok(result)
}

fn advance(
    state: &MemoryLifecycleStateV1,
    transition: MemoryTransitionV1,
) -> MemoryLifecycleStateV1 {
    let next = state.next(transition).unwrap();
    next.validate_global_invariants().unwrap();
    next
}

struct Fixture {
    states: Vec<(&'static str, MemoryLifecycleStateV1)>,
    vm: VmKeyV1,
    reservation: VaReservationKeyV1,
    allocation: MemoryAllocationKeyV1,
    mapping: MemoryMappingKeyV1,
    publication: MemoryPublicationKeyV1,
}

impl Fixture {
    fn new() -> Self {
        let vm_only = crate::memory_lifecycle_tests::checkpoint_vm_fixture_for_test();
        let vm = vm_only.vms()[0].admission.model_key();
        let reservation = VaReservationKeyV1 {
            vm,
            id: VaReservationIdV1(20),
        };
        let allocation = MemoryAllocationKeyV1 {
            vm,
            id: AllocationIdV1(30),
            generation: AllocationGenerationV1(1),
        };
        let mapping = MemoryMappingKeyV1 {
            allocation,
            id: MappingIdV1(40),
        };
        let publication = MemoryPublicationKeyV1 {
            mapping,
            id: MemoryPublicationIdV1(50),
        };
        let reserved = advance(&vm_only, reserve(vm, 20, 0x2_0000));
        let allocated = advance(
            &reserved,
            MemoryTransitionV1::Allocate {
                key: allocation,
                reservation,
                handle: UntrustedAllocationHandleObservationV1(200),
                spec: MemoryAllocationSpecV1 {
                    byte_len: MEMORY_PAGE_BYTES_V1,
                    alignment: MEMORY_PAGE_BYTES_V1,
                    kind: MemoryKindV1::HostVisibleCoherent,
                    coherence: MemoryCoherenceV1::HostCoherent,
                },
            },
        );
        let map_pending = advance(
            &allocated,
            MemoryTransitionV1::BeginMap {
                key: mapping,
                target_devices: vm_only.vms()[0].mapping_device_keys().collect(),
                access: MemoryAccessV1::ReadWrite,
            },
        );
        let map_failed = advance(
            &map_pending,
            MemoryTransitionV1::ObserveMap {
                key: mapping,
                progress: PartialProgressObservationV1 {
                    n_success: 1,
                    status: PartialOperationStatusV1::Failed,
                },
            },
        );
        let ambiguous = advance(
            &map_pending,
            MemoryTransitionV1::ObserveMap {
                key: mapping,
                progress: PartialProgressObservationV1 {
                    n_success: 1,
                    status: PartialOperationStatusV1::Indeterminate,
                },
            },
        );
        let mapped = advance(
            &map_pending,
            MemoryTransitionV1::ObserveMap {
                key: mapping,
                progress: PartialProgressObservationV1 {
                    n_success: 2,
                    status: PartialOperationStatusV1::Succeeded,
                },
            },
        );
        let published = advance(
            &mapped,
            MemoryTransitionV1::PublishMapping { key: publication },
        );
        let publication_released = advance(
            &published,
            MemoryTransitionV1::ReleasePublication { key: publication },
        );
        let unmap_pending = advance(
            &publication_released,
            MemoryTransitionV1::BeginUnmap { key: mapping },
        );
        let unmap_failed = advance(
            &unmap_pending,
            MemoryTransitionV1::ObserveUnmap {
                key: mapping,
                progress: PartialProgressObservationV1 {
                    n_success: 1,
                    status: PartialOperationStatusV1::Failed,
                },
            },
        );
        let unmapped = advance(
            &unmap_pending,
            MemoryTransitionV1::ObserveUnmap {
                key: mapping,
                progress: PartialProgressObservationV1 {
                    n_success: 2,
                    status: PartialOperationStatusV1::Succeeded,
                },
            },
        );
        let mapping_released = advance(
            &unmapped,
            MemoryTransitionV1::ReleaseMapping { key: mapping },
        );
        let allocation_released = advance(
            &mapping_released,
            MemoryTransitionV1::ReleaseAllocation { key: allocation },
        );
        let reservation_released = advance(
            &allocation_released,
            MemoryTransitionV1::ReleaseVaReservation { key: reservation },
        );
        let retired = advance(
            &legacy_checkpoint(&reservation_released).unwrap(),
            MemoryTransitionV1::RetireVm { key: vm },
        );
        Self {
            states: vec![
                (
                    "empty",
                    MemoryLifecycleStateV1::new_monotonic_non_reusable(vm_only.domain_id()),
                ),
                ("vm", vm_only),
                ("reserved", reserved),
                ("allocated", allocated),
                ("map-pending", map_pending),
                ("map-failed", map_failed),
                ("ambiguous", ambiguous),
                ("mapped", mapped),
                ("published", published),
                ("publication-released", publication_released),
                ("unmap-pending", unmap_pending),
                ("unmap-failed", unmap_failed),
                ("unmapped", unmapped),
                ("mapping-released", mapping_released),
                ("allocation-released", allocation_released),
                ("reservation-released", reservation_released),
                ("retired", retired),
            ],
            vm,
            reservation,
            allocation,
            mapping,
            publication,
        }
    }

    fn state(&self, name: &str) -> &MemoryLifecycleStateV1 {
        &self
            .states
            .iter()
            .find(|(label, _)| *label == name)
            .unwrap()
            .1
    }
}

fn reserve(vm: VmKeyV1, id: u64, base: u64) -> MemoryTransitionV1 {
    MemoryTransitionV1::ReserveVa {
        key: VaReservationKeyV1 {
            vm,
            id: VaReservationIdV1(id),
        },
        range: GpuVaRangeV1 {
            base,
            byte_len: MEMORY_PAGE_BYTES_V1,
        },
        alignment: MEMORY_PAGE_BYTES_V1,
    }
}

fn released_kinds(state: &MemoryLifecycleStateV1) -> [bool; 4] {
    [
        state
            .reservations()
            .iter()
            .any(|r| r.state == VaReservationStateV1::Released),
        state
            .allocations()
            .iter()
            .any(|r| r.state == MemoryAllocationStateV1::Released),
        state
            .mappings()
            .iter()
            .any(|r| r.state == MemoryMappingStateV1::Released),
        state
            .publications()
            .iter()
            .any(|r| r.state == MemoryPublicationStateV1::Released),
    ]
}

fn assert_exact_checkpoint(source: &MemoryLifecycleStateV1) -> MemoryLifecycleStateV1 {
    let actual = source.checkpoint_released();
    let expected = legacy_checkpoint(source);
    assert_eq!(actual, expected);
    let actual = actual.unwrap();
    assert_eq!(actual.authority_domain(), source.authority_domain());
    assert_eq!(actual.domain_id(), source.domain_id());
    assert_eq!(actual.identity_discipline(), source.identity_discipline());
    actual.validate_global_invariants().unwrap();
    actual
}

#[test]
fn checkpoint_fast_path_matches_legacy_across_live_and_retired_states() {
    let fixture = Fixture::new();
    let mut fast_cases = 0;
    for (name, source) in &fixture.states {
        let actual = assert_exact_checkpoint(source);
        if released_kinds(source) == [false; 4] {
            fast_cases += 1;
            assert_eq!(&actual, source, "{name}");
            assert_eq!(
                source.shared_journals_for_test(&actual),
                [true; 6],
                "{name}"
            );
        }
        assert_eq!(
            actual.checkpoint_released().unwrap(),
            actual,
            "idempotence: {name}"
        );
    }
    assert_eq!(fast_cases, 10);
    for name in ["unmap-pending", "unmap-failed", "unmapped"] {
        let clean = legacy_checkpoint(fixture.state(name)).unwrap();
        assert_eq!(released_kinds(&clean), [false; 4]);
        assert_eq!(assert_exact_checkpoint(&clean), clean);
    }
}

#[test]
fn checkpoint_each_released_journal_prevents_identity_shortcut() {
    let fixture = Fixture::new();
    let cases = [
        (
            "published",
            MemoryTransitionV1::ReleasePublication {
                key: fixture.publication,
            },
            3,
        ),
        (
            "unmapped",
            MemoryTransitionV1::ReleaseMapping {
                key: fixture.mapping,
            },
            2,
        ),
        (
            "mapping-released",
            MemoryTransitionV1::ReleaseAllocation {
                key: fixture.allocation,
            },
            1,
        ),
        (
            "allocation-released",
            MemoryTransitionV1::ReleaseVaReservation {
                key: fixture.reservation,
            },
            0,
        ),
    ];
    for (name, release, index) in cases {
        let before = legacy_checkpoint(fixture.state(name)).unwrap();
        let source = advance(&before, release);
        let mut expected_released = [false; 4];
        expected_released[index] = true;
        assert_eq!(released_kinds(&source), expected_released, "{name}");
        let actual = assert_exact_checkpoint(&source);
        assert_ne!(actual, source, "{name}");
        assert_eq!(released_kinds(&actual), [false; 4]);
    }
}

#[test]
fn checkpoint_mixed_live_and_released_records_match_legacy_without_identity_reuse() {
    let fixture = Fixture::new();
    let live = fixture.state("published");
    let reservation = VaReservationKeyV1 {
        vm: fixture.vm,
        id: VaReservationIdV1(21),
    };
    let allocation = MemoryAllocationKeyV1 {
        id: AllocationIdV1(31),
        ..fixture.allocation
    };
    let source = advance(live, reserve(fixture.vm, 21, 0x3_0000));
    let source = advance(
        &source,
        MemoryTransitionV1::Allocate {
            key: allocation,
            reservation,
            handle: UntrustedAllocationHandleObservationV1(201),
            spec: live.allocations()[0].spec,
        },
    );
    let source = advance(
        &source,
        MemoryTransitionV1::ReleaseAllocation { key: allocation },
    );
    let source = advance(
        &source,
        MemoryTransitionV1::ReleaseVaReservation { key: reservation },
    );
    let result = assert_exact_checkpoint(&source);
    assert_eq!(result.reservations(), live.reservations());
    assert_eq!(result.allocations(), live.allocations());
    assert_eq!(result.mappings(), live.mappings());
    assert_eq!(result.publications(), live.publications());
    assert_eq!(
        result.next(reserve(fixture.vm, 21, 0x3_0000)),
        Err(MemoryTransitionErrorV1::NonMonotonicIdentity(
            MemoryRecordRefV1::VaReservation(reservation)
        ))
    );
    assert_eq!(
        result
            .issued_id_high_watermarks()
            .iter()
            .find(|record| record.scope == MemoryIssuedIdScopeV1::Allocation(fixture.vm))
            .unwrap()
            .last_id,
        31
    );
}

#[test]
fn checkpoint_preserves_high_watermarks_and_future_admission() {
    let fixture = Fixture::new();
    for last_id in [80, u64::MAX] {
        let mut source = fixture.state("published").clone();
        assert_eq!(source.issued_id_high_watermarks.len(), 4);
        for index in 0..source.issued_id_high_watermarks.len() {
            source
                .issued_id_high_watermarks
                .get_mut(index)
                .unwrap()
                .last_id = last_id;
        }
        source.validate_global_invariants().unwrap();
        let actual = assert_exact_checkpoint(&source);
        assert_eq!(
            actual.issued_id_high_watermarks(),
            source.issued_id_high_watermarks()
        );
        let stale = VaReservationKeyV1 {
            vm: fixture.vm,
            id: VaReservationIdV1(last_id),
        };
        assert_eq!(
            actual.next(reserve(fixture.vm, last_id, 0x3_0000)),
            Err(MemoryTransitionErrorV1::NonMonotonicIdentity(
                MemoryRecordRefV1::VaReservation(stale)
            ))
        );
        if let Some(fresh) = last_id.checked_add(1) {
            let expected = source.next(reserve(fixture.vm, fresh, 0x3_0000));
            assert!(expected.is_ok());
            assert_eq!(actual.next(reserve(fixture.vm, fresh, 0x3_0000)), expected);
        }
    }
}

#[test]
fn checkpoint_fast_path_shares_warmed_storage_without_record_copies() {
    let fixture = Fixture::new();
    let source = fixture.state("published");
    // Initial validation may materialize canonical views; only the warmed path promises no copies.
    let _ = (
        source.vms(),
        source.reservations(),
        source.allocations(),
        source.mappings(),
        source.publications(),
        source.issued_id_high_watermarks(),
    );
    source.validate_global_invariants().unwrap();
    reset_journal_copied_records_for_test();
    let actual = source.checkpoint_released().unwrap();
    assert_eq!(journal_copied_records_for_test(), 0);
    assert_eq!(source.shared_journals_for_test(&actual), [true; 6]);
    assert_eq!(&actual, source);
    reset_journal_copied_records_for_test();
    let legacy = legacy_checkpoint(source).unwrap();
    assert!(journal_copied_records_for_test() > 0);
    assert_eq!(actual, legacy);
}

#[test]
fn checkpoint_shared_result_preserves_copy_on_write_isolation() {
    let fixture = Fixture::new();
    let source = fixture.state("published");
    let before = source.clone();
    let checkpoint = source.checkpoint_released().unwrap();
    let result_branch = advance(
        &checkpoint,
        MemoryTransitionV1::ReleasePublication {
            key: fixture.publication,
        },
    );
    let source_branch = advance(source, reserve(fixture.vm, 21, 0x3_0000));
    assert_eq!(source, &before);
    assert_eq!(checkpoint, before);
    assert_eq!(
        result_branch.publications()[0].state,
        MemoryPublicationStateV1::Released
    );
    assert_eq!(result_branch.reservations().len(), 1);
    assert_eq!(
        source_branch.publications()[0].state,
        MemoryPublicationStateV1::Live
    );
    assert_eq!(source_branch.reservations().len(), 2);
    assert!(!source.shared_journals_for_test(&result_branch)[4]);
    assert!(!source.shared_journals_for_test(&source_branch)[1]);
}

fn assert_source_refusal(source: &MemoryLifecycleStateV1, error: MemoryInvariantViolationV1) {
    assert_eq!(released_kinds(source), [false; 4]);
    assert_eq!(source.validate_global_invariants(), Err(error));
    let expected = Err(MemoryTransitionErrorV1::SourceInvariant(error));
    assert_eq!(legacy_checkpoint(source), expected);
    assert_eq!(source.checkpoint_released(), expected);
}

#[test]
fn checkpoint_fast_path_preserves_index_and_discipline_error_order() {
    let fixture = Fixture::new();
    for discipline in [
        MemoryIdentityDisciplineV1::MonotonicNonReusable,
        MemoryIdentityDisciplineV1::ReusableGenerations,
    ] {
        for (ordinal, kind) in [
            MemoryRecordKindV1::Vm,
            MemoryRecordKindV1::VaReservation,
            MemoryRecordKindV1::Allocation,
            MemoryRecordKindV1::Mapping,
            MemoryRecordKindV1::Publication,
        ]
        .into_iter()
        .enumerate()
        {
            let mut source = fixture.state("published").clone();
            source.identity_discipline = discipline;
            match ordinal {
                0 => source.vms.index = PersistentJournalIndexV1::new(),
                1 => source.reservations.index = PersistentJournalIndexV1::new(),
                2 => source.allocations.index = PersistentJournalIndexV1::new(),
                3 => source.mappings.index = PersistentJournalIndexV1::new(),
                4 => source.publications.index = PersistentJournalIndexV1::new(),
                _ => unreachable!(),
            }
            assert_source_refusal(
                &source,
                MemoryInvariantViolationV1::JournalIndexMismatch(kind),
            );
        }
        let mut source = fixture.state("empty").clone();
        source.identity_discipline = discipline;
        assert_eq!(source.checkpoint_released(), legacy_checkpoint(&source));
        if discipline == MemoryIdentityDisciplineV1::ReusableGenerations {
            assert_eq!(
                source.checkpoint_released(),
                Err(MemoryTransitionErrorV1::CheckpointRequiresMonotonicIdentities)
            );
        }
    }
}

#[test]
fn checkpoint_fast_path_does_not_hide_invalid_watermarks_or_capacity() {
    let fixture = Fixture::new();
    for kind in 0..4 {
        let mut source = fixture.state("published").clone();
        let invalid = match kind {
            0 => {
                source.issued_id_high_watermarks.get_mut(0).unwrap().last_id = 0;
                *source.issued_id_high_watermarks.get(0).unwrap()
            }
            1 => {
                let first = *source.issued_id_high_watermarks.get(0).unwrap();
                *source.issued_id_high_watermarks.get_mut(1).unwrap() = first;
                first
            }
            2 | 3 => {
                let (index, scope) = if kind == 2 {
                    (
                        2,
                        MemoryIssuedIdScopeV1::Mapping(MemoryAllocationKeyV1 {
                            id: AllocationIdV1(999),
                            ..fixture.allocation
                        }),
                    )
                } else {
                    (
                        3,
                        MemoryIssuedIdScopeV1::Publication(MemoryMappingKeyV1 {
                            id: MappingIdV1(999),
                            ..fixture.mapping
                        }),
                    )
                };
                source
                    .issued_id_high_watermarks
                    .get_mut(index)
                    .unwrap()
                    .scope = scope;
                *source.issued_id_high_watermarks.get(index).unwrap()
            }
            _ => unreachable!(),
        };
        assert_source_refusal(
            &source,
            MemoryInvariantViolationV1::InvalidIssuedIdHighWatermark(invalid),
        );
    }
    let mut source = fixture.state("vm").clone();
    for id in 1..=MAX_VA_RESERVATIONS_V1 + 1 {
        source.reservations.push(VaReservationRecordV1 {
            key: VaReservationKeyV1 {
                vm: fixture.vm,
                id: VaReservationIdV1(id as u64),
            },
            range: GpuVaRangeV1 {
                base: 0x2_0000 + id as u64 * MEMORY_PAGE_BYTES_V1,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
            state: VaReservationStateV1::Reserved,
        });
    }
    assert_source_refusal(
        &source,
        MemoryInvariantViolationV1::CapacityExceeded(MemoryRecordKindV1::VaReservation),
    );
}
