use super::tests::{admit, candidate, mixed, noop, with_fixture};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    panic::{AssertUnwindSafe, catch_unwind, panic_any},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn check_rows(
    inventory: &Inventory<'_>,
    metadata: &Metadata<'_, '_>,
    rows: Vec<Row>,
    base: usize,
) -> Result<()> {
    let candidate = Candidate::from_rows(inventory, metadata, rows);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let bytes = candidate.retained_storage()? + metadata.storage_extent(&mut budget)?;
    budget.reserve_storage(base + bytes)?;
    let before = budget.storage();
    let result = with_checked_canonical_ranked_view_v1(
        inventory,
        metadata,
        &candidate,
        &mut budget,
        |_, _| Ok::<_, Error>(()),
    );
    assert_eq!(budget.storage(), before);
    result
}

#[test]
fn canonical_ranked_every_missing_duplicate_reordered_and_extra_row_is_rejected() {
    with_fixture(&mixed(), |inventory, budget| {
        let facts = [Fact::Definition(0), Fact::Effect(0)];
        let annotations = [CanonicalRankedMetadataRowV1 {
            subject: Subject::Module,
            kind: CanonicalRankedMetadataKindV1::Memory,
            facts: &facts,
        }];
        let metadata = Metadata::new(inventory.owner(), &annotations);
        let base = budget.storage();
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let original = candidate.rows();
        for i in 0..original.len() {
            let mut rows = original.to_vec();
            rows.remove(i);
            assert!(matches!(
                check_rows(inventory, &metadata, rows, base),
                Err(Error::MissingRow { .. } | Error::MismatchedRow { .. })
            ));
            let mut rows = original.to_vec();
            rows.insert(i, rows[i]);
            assert!(matches!(
                check_rows(inventory, &metadata, rows, base),
                Err(Error::ExtraRows { .. } | Error::MismatchedRow { .. })
            ));
            if i + 1 < original.len() {
                let mut rows = original.to_vec();
                rows.swap(i, i + 1);
                assert!(matches!(
                    check_rows(inventory, &metadata, rows, base),
                    Err(Error::MismatchedRow { .. })
                ));
            }
        }
        let mut rows = original.to_vec();
        rows.push(original[0]);
        assert!(matches!(
            check_rows(inventory, &metadata, rows, base),
            Err(Error::ExtraRows { .. })
        ));
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_obligation_and_duplicate_target_edge_substitution_fail() {
    with_fixture(&mixed(), |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let base = budget.storage();
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        for (i, expected) in candidate.rows().iter().enumerate() {
            if expected.obligations != Obligations::NONE {
                let mut rows = candidate.rows().to_vec();
                rows[i].obligations = Obligations::NONE;
                assert!(
                    matches!(check_rows(inventory, &metadata, rows, base), Err(Error::MismatchedRow { ordinal, .. }) if ordinal == i)
                );
            }
        }
        let mut rows = candidate.rows().to_vec();
        let index = rows
            .iter()
            .position(|r| r.subject == Subject::Edge(1))
            .unwrap();
        rows[index].role = Role::Edge(EdgeClass::True);
        assert!(matches!(
            check_rows(inventory, &metadata, rows, base),
            Err(Error::MismatchedRow { .. })
        ));
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_same_bytes_do_not_bind_foreign_inventory_graph_or_metadata() {
    with_fixture(&noop(), |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let other_metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let floor = budget.storage();
        let result = with_checked_canonical_ranked_view_v1(
            inventory,
            &other_metadata,
            &candidate,
            budget,
            |_, _| Ok::<_, Error>(()),
        );
        assert_eq!(result, Err(Error::ForeignMetadata));
        let (other_inventory, other_storage) =
            Inventory::derive(inventory.owner(), budget).unwrap();
        budget
            .reserve_storage(other_storage.retained_storage())
            .unwrap();
        let result = with_checked_canonical_ranked_view_v1(
            &other_inventory,
            &metadata,
            &candidate,
            budget,
            |_, _| Ok::<_, Error>(()),
        );
        assert_eq!(result, Err(Error::ForeignInventory));
        drop(other_inventory);
        budget
            .release_storage(other_storage.retained_storage())
            .unwrap();
        let (owner, owner_bytes) = admit(&noop());
        assert_eq!(
            inventory.owner().canonical().identity(),
            owner.canonical().identity()
        );
        budget.reserve_storage(owner_bytes).unwrap();
        let foreign_metadata = Metadata::new(&owner, &[]);
        assert!(matches!(
            build_canonical_ranked_candidate_v1(inventory, &foreign_metadata, budget),
            Err(Error::ForeignMetadata)
        ));
        let (other_inventory, other_storage) = Inventory::derive(&owner, budget).unwrap();
        budget
            .reserve_storage(other_storage.retained_storage())
            .unwrap();
        assert_eq!(
            with_checked_canonical_ranked_view_v1(
                &other_inventory,
                &foreign_metadata,
                &candidate,
                budget,
                |_, _| Ok::<_, Error>(())
            ),
            Err(Error::ForeignInventory)
        );
        drop(other_inventory);
        budget
            .release_storage(other_storage.retained_storage())
            .unwrap();
        drop(owner);
        budget.release_storage(owner_bytes).unwrap();
        assert_eq!(budget.storage(), floor);
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_unanchored_duplicate_metadata_and_out_of_range_fact_fail() {
    with_fixture(&noop(), |inventory, budget| {
        let base = budget.storage();
        let facts = [Fact::Definition(usize::MAX)];
        for bad in 0..3 {
            let mut annotations = vec![CanonicalRankedMetadataRowV1 {
                subject: if bad == 0 {
                    Subject::Metadata(0)
                } else {
                    Subject::Kernel(0)
                },
                kind: CanonicalRankedMetadataKindV1::Launch,
                facts: if bad == 2 { &facts } else { &[] },
            }];
            if bad == 1 {
                annotations.push(CanonicalRankedMetadataRowV1 {
                    subject: Subject::Kernel(0),
                    kind: CanonicalRankedMetadataKindV1::Launch,
                    facts: &[],
                });
            }
            let metadata = Metadata::new(inventory.owner(), &annotations);
            let (candidate, bytes) = candidate(inventory, &metadata, budget);
            let result = check_rows(inventory, &metadata, candidate.rows().to_vec(), base);
            match bad {
                0 => assert!(matches!(result, Err(Error::MetadataSubject { ordinal: 0 }))),
                1 => assert!(matches!(result, Err(Error::MetadataOrder { ordinal: 1 }))),
                2 => assert!(matches!(
                    result,
                    Err(Error::MetadataFact { row: 0, fact: 0 })
                )),
                _ => unreachable!(),
            }
            drop(candidate);
            budget.release_storage(bytes).unwrap();
        }
    });
}

#[test]
fn canonical_ranked_foreign_query_is_not_charged_and_cannot_be_ignored() {
    with_fixture(&noop(), |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let floor = budget.storage();
        let mut foreign_work = Work::new(100);
        let mut foreign = Budget::new(&mut foreign_work, 100);
        foreign.reserve_storage(23).unwrap();
        let result = with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            budget,
            |view, _| {
                assert!(matches!(
                    view.row_count(&mut foreign),
                    Err(Error::Resource(Resource::Accounting))
                ));
                Ok::<_, Error>(())
            },
        );
        assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
        assert_eq!(foreign.work(), 0);
        assert_eq!(foreign.storage(), 23);
        assert_eq!(budget.storage(), floor);
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_same_ledger_in_a_moved_slot_is_not_a_valid_query() {
    with_fixture(&noop(), |inventory, outer| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, outer);
        let floor = outer.storage();
        let mut work = Work::new(10_000);
        let mut spare_work = Work::new(10_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let mut spare = Budget::new(&mut spare_work, 1_000_000);
        budget.reserve_storage(floor).unwrap();
        let original = budget.work_ledger_identity_v1();
        let result = with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            &mut budget,
            |view, budget| {
                std::mem::swap(budget, &mut spare);
                assert!(spare.work_ledger_identity_v1() == original);
                assert!(matches!(
                    view.row_count(&mut spare),
                    Err(Error::Resource(Resource::Accounting))
                ));
                std::mem::swap(budget, &mut spare);
                Ok::<_, Error>(())
            },
        );
        assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
        assert_eq!(budget.storage(), floor);
        assert_eq!(spare.storage(), 0);
        drop(candidate);
        outer.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_released_backing_query_and_no_query_cannot_refund_or_mint() {
    with_fixture(&noop(), |inventory, outer| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, outer);
        let floor = outer.storage();
        let headers = size_of::<control::Accounting>()
            + size_of::<CheckedCanonicalRankedViewV1<'_, '_, '_, '_>>();
        for query in [false, true] {
            let mut work = Work::new(10_000);
            let mut budget = Budget::new(&mut work, 1_000_000);
            budget.reserve_storage(floor).unwrap();
            let result = with_checked_canonical_ranked_view_v1(
                inventory,
                &metadata,
                &candidate,
                &mut budget,
                |view, budget| {
                    budget.release_storage(1)?;
                    if query {
                        assert!(matches!(
                            view.row_count(budget),
                            Err(Error::Resource(Resource::Accounting))
                        ));
                    }
                    Ok::<_, Error>(())
                },
            );
            assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
            assert_eq!(budget.storage(), floor + headers - 1);
        }
        drop(candidate);
        outer.release_storage(bytes).unwrap();
    });
}

struct PanicPayload(Arc<AtomicUsize>);
impl Drop for PanicPayload {
    fn drop(&mut self) {
        self.0.fetch_or(2, Ordering::SeqCst);
        panic!("panic payload destructor");
    }
}
struct RejectDrop(Arc<AtomicUsize>);
impl Drop for RejectDrop {
    fn drop(&mut self) {
        self.0.fetch_or(1, Ordering::SeqCst);
        panic_any(PanicPayload(self.0.clone()));
    }
}

#[test]
fn canonical_ranked_rejected_result_destructor_and_panic_payload_cleanup_is_paid() {
    with_fixture(&noop(), |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let floor = budget.storage();
        let observed = Arc::new(AtomicUsize::new(0));
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_checked_canonical_ranked_view_v1(
                inventory,
                &metadata,
                &candidate,
                budget,
                |_, budget| {
                    budget.reserve_storage(1)?;
                    Ok::<_, Error>(RejectDrop(observed.clone()))
                },
            );
        }));
        assert!(caught.is_err());
        assert_eq!(observed.load(Ordering::SeqCst), 3);
        // Only the guard is refunded; unrelated caller excess remains paid.
        assert_eq!(budget.storage(), floor + 1);
        budget.release_storage(1).unwrap();
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<()> = with_checked_canonical_ranked_view_v1(
                inventory,
                &metadata,
                &candidate,
                budget,
                |_, _| {
                    panic_any(PanicPayload(observed.clone()));
                },
            );
        }));
        assert!(caught.is_err());
        assert_eq!(budget.storage(), floor);
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_plain_callback_error_and_panic_restore_original_floor() {
    with_fixture(&noop(), |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let floor = budget.storage();
        let failure = Error::InvalidCoordinate(Subject::Module);
        let result: Result<()> = with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            budget,
            |_, _| Err(failure.clone()),
        );
        assert_eq!(result, Err(failure));
        assert_eq!(budget.storage(), floor);
        let result: Result<()> = with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            budget,
            |_, _| panic!("callback"),
        );
        assert_eq!(result, Err(Error::Panicked));
        assert_eq!(budget.storage(), floor);
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_new_wire_families_are_explicit_typed_refusals_not_safe_defaults() {
    use fe2o3_kernel_ir::{
        AssemblySourceIdentity, ExecutionOperationV15, Gfx942OrderedProgramRegistersV1,
        Gfx942OrderedProgramV1, Gfx942OrderedRegionRegistersV1, Gfx942OrderedRegionV1,
        Gfx942ProgramDestinationV1, Gfx942ProgramInstructionV1, Gfx942ProgramRoleV1,
        Gfx942U32ProgramV1, OperationKind, ValueId,
    };
    let source = AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]);
    let region = Gfx942OrderedRegionV1::new(
        source,
        Gfx942OrderedRegionRegistersV1::new(0, 1, [2, 3, 4]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
    )
    .unwrap();
    let program = Gfx942OrderedProgramV1::new(
        source,
        Gfx942OrderedProgramRegistersV1::new(0, 1, [2, 3, 4]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
        Gfx942U32ProgramV1::from_instructions(&[Gfx942ProgramInstructionV1::Move {
            destination: Gfx942ProgramDestinationV1::Output,
            source: Gfx942ProgramRoleV1::Input0,
        }])
        .unwrap(),
    )
    .unwrap();
    for (wire_version, op) in [
        (
            15,
            OperationKind::Execution(ExecutionOperationV15::ContextIssue),
        ),
        (16, OperationKind::Gfx942OrderedRegion(region)),
        (17, OperationKind::Gfx942OrderedProgram(program)),
    ] {
        // A local classification test, not construction of a sealed V12 owner.
        assert_eq!(
            effects::operation(9, &op),
            Err(Error::UnsupportedOperation {
                ordinal: 9,
                wire_version
            })
        );
    }
}

#[test]
fn canonical_ranked_replaced_ledger_is_untouched_and_original_receipt_is_not_refunded_elsewhere() {
    with_fixture(&noop(), |inventory, outer| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, outer);
        let floor = outer.storage();
        let headers = size_of::<control::Accounting>()
            + size_of::<CheckedCanonicalRankedViewV1<'_, '_, '_, '_>>();
        let mut original_work = Work::new(10_000);
        let mut foreign_work = Work::new(10_000);
        let mut budget = Budget::new(&mut original_work, 1_000_000);
        let mut foreign = Budget::new(&mut foreign_work, 1_000_000);
        budget.reserve_storage(floor).unwrap();
        foreign.reserve_storage(19).unwrap();
        let foreign_identity = foreign.work_ledger_identity_v1();
        let result = with_checked_canonical_ranked_view_v1(
            inventory,
            &metadata,
            &candidate,
            &mut budget,
            |_, budget| {
                std::mem::swap(budget, &mut foreign);
                Ok::<_, Error>(())
            },
        );
        assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
        assert!(budget.work_ledger_identity_v1() == foreign_identity);
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.work(), 0);
        assert_eq!(foreign.storage(), floor + headers);
        drop(candidate);
        outer.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_invalid_queries_poison_ignored_and_propagated_callbacks() {
    with_fixture(&noop(), |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(budget);
        let headers = size_of::<control::Accounting>()
            + size_of::<CheckedCanonicalRankedViewV1<'_, '_, '_, '_>>();
        for accessor in 0..5 {
            let expected = match accessor {
                0 => Error::InvalidRow {
                    ordinal: usize::MAX,
                },
                1 => Error::InvalidCoordinate(Subject::Operation(usize::MAX)),
                2 => Error::InvalidCoordinate(Subject::Edge(usize::MAX)),
                3 => Error::InvalidCoordinate(Subject::EdgeArgument(usize::MAX)),
                4 => Error::InvalidCoordinate(Subject::Metadata(usize::MAX)),
                _ => unreachable!(),
            };
            for propagate in [false, true] {
                let before = budget.work();
                let expected_peak = budget.peak_storage().max(floor + headers);
                let result: Result<()> = with_checked_canonical_ranked_view_v1(
                    inventory,
                    &metadata,
                    &candidate,
                    budget,
                    |view, budget| {
                        let before = budget.work();
                        let rejected = match accessor {
                            0 => view.row(usize::MAX, budget).map(|_| ()),
                            1 => view.operation(usize::MAX, budget).map(|_| ()),
                            2 => view.edge(usize::MAX, budget).map(|_| ()),
                            3 => view.edge_argument(usize::MAX, budget).map(|_| ()),
                            4 => view.metadata(usize::MAX, budget).map(|_| ()),
                            _ => unreachable!(),
                        };
                        assert_eq!(rejected, Err(expected.clone()));
                        assert_eq!(budget.work(), before + 1);
                        // A valid query cannot clear the first failed lookup or charge again.
                        assert_eq!(view.row_count(budget), Err(expected.clone()));
                        assert_eq!(budget.work(), before + 1);
                        if propagate { rejected } else { Ok(()) }
                    },
                );
                assert_eq!(result, Err(expected.clone()));
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.work(), before + 13);
                assert_eq!(budget.peak_storage(), expected_peak);
                assert_eq!(budget.failed_storage(), None);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert!(std::ptr::eq(std::ptr::from_ref(budget), slot));
            }
        }
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}

#[test]
fn canonical_ranked_invalid_query_rejected_result_panic_restores_paid_floor() {
    with_fixture(&noop(), |inventory, budget| {
        let metadata = Metadata::new(inventory.owner(), &[]);
        let (candidate, bytes) = candidate(inventory, &metadata, budget);
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(budget);
        let headers = size_of::<control::Accounting>()
            + size_of::<CheckedCanonicalRankedViewV1<'_, '_, '_, '_>>();
        let observed = Arc::new(AtomicUsize::new(0));
        let before = budget.work();
        let expected_peak = budget.peak_storage().max(floor + headers);
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_checked_canonical_ranked_view_v1(
                inventory,
                &metadata,
                &candidate,
                budget,
                |view, budget| {
                    assert_eq!(
                        view.row(usize::MAX, budget).map(|_| ()),
                        Err(Error::InvalidRow {
                            ordinal: usize::MAX,
                        }),
                    );
                    assert_eq!(budget.storage(), floor + headers);
                    Ok::<_, Error>(RejectDrop(observed.clone()))
                },
            );
        }));
        assert!(caught.is_err());
        assert_eq!(observed.load(Ordering::SeqCst), 3);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), before + 13);
        assert_eq!(budget.peak_storage(), expected_peak);
        assert_eq!(budget.failed_storage(), None);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(std::ptr::eq(std::ptr::from_ref(budget), slot));
        drop(candidate);
        budget.release_storage(bytes).unwrap();
    });
}
