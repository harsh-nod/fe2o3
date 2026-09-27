use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use crate::{ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1};

const ISSUED_ROLE_FLOOR: usize = 37;
const ISSUED_ROLE_LIMIT: usize = 1_000_000_000;

fn run_issued_role_source_v18(
    used: bool,
    work_limit: usize,
    storage_limit: usize,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    run_issued_role_source_custody_v18(used, work_limit, storage_limit,
        &std::cell::Cell::new(None), consume)
}

fn run_issued_role_source_custody_v18(
    used: bool,
    work_limit: usize,
    storage_limit: usize,
    retained_floor: &std::cell::Cell<Option<usize>>,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    run_issued_role_source_shape_v18(1, usize::from(used), work_limit, storage_limit,
        retained_floor, consume)
}

fn run_issued_role_source_shape_v18(
    issuer_count: usize,
    access_count: usize,
    work_limit: usize,
    storage_limit: usize,
    retained_floor: &std::cell::Cell<Option<usize>>,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    use fe2o3_kernel_descriptor::{
        DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName,
    };
    let owner = source_issued_pointer_source_tests_v29::owner_with_shape(issuer_count, access_count);
    let occurrence_storage = owner.occurrence_storage().unwrap().retained_storage();
    let semantic = owner.source_semantic();
    let hash = *owner.source_semantic_sha256();
    let binding = *semantic.functions()[0].kernel_entry().unwrap().kernel_binding_identity().as_bytes();
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &[
        ProductionSourceLaunchRootInputV1::new("issued_pointer_source", binding,
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1])),
    ]).unwrap();
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let arguments: Vec<_> = ["first", "second"].into_iter().enumerate().map(|(ordinal, name)| {
        ProductionKernelArgumentAbiArgumentV18 {
            semantic_type_identity: semantic.types()[4].identity(),
            kind: ProductionKernelArgumentAbiKindV18::Descriptor {
                source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
                argument: LogicalArgumentV1::disjoint_slice(ordinal as u16, ValidName::new(name).unwrap(),
                    &source, &layout, fe2o3_kernel_descriptor::AccessMode::ReadWrite, (ordinal * 16) as u32).unwrap(),
            },
        }
    }).collect();
    let roots = [ProductionKernelArgumentAbiRootV18 {
        kernel_binding: &binding, export: "issued_pointer_source", arguments: &arguments,
        explicit_argument_bytes: 32, kernarg_alignment_bytes: 8,
    }];
    let classes = [ProductionScopeCallableCandidateV29::Ordinary; 3];
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &hash, roots: &[], classes: &classes, events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(ISSUED_ROLE_FLOOR + occurrence_storage).unwrap();
    let result = (|| -> SourceOwnedResultV18<()> {
        let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
            owner, launch, input, ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(), &mut budget,
        )?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                    // Same source-owned candidate reaches the complete physical
                    // consumer before any copied-component hostile control.
                    with_checked_source_memory_v29(original, 0, None, budget, |_, budget| {
                        consume(original, budget)
                    })
                })
            }))
        })
    })();
    assert_eq!(budget.storage(), retained_floor.get().unwrap_or(ISSUED_ROLE_FLOOR), "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

fn issued_rows_v18<'a>(original: &'a ProductionSourceCorrespondenceV18<'_>) -> &'a PendingSourceIssuedRolesV29 {
    &original.source.root_row(0).unwrap().source_slots.pending_memory.as_ref().unwrap().issued
}

fn check_issued_role_positive_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    used: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let rows = issued_rows_v18(original);
    assert_eq!((rows.sources.len(), rows.issuers.len(), rows.accesses.len()), (1, 1, usize::from(used)));
    assert_eq!(rows.sources[0].instance, rows.issuers[0].instance);
    assert_eq!(rows.sources[0].block, rows.issuers[0].block);
    assert_eq!(rows.issuers[0].element, ScalarType::U32);
    assert_eq!(rows.issuers[0].access, AccessMode::ReadWrite);
    assert_eq!(rows.retained_storage()?,
        rows.sources.capacity() * std::mem::size_of::<PendingSourceIssuedSiteV29>()
        + rows.issuers.capacity() * std::mem::size_of::<PendingSourceIssuedIssuerV29>()
        + rows.accesses.capacity() * std::mem::size_of::<PendingSourceIssuedAccessV29>());
    if used {
        assert_eq!(rows.accesses[0].instance, rows.issuers[0].instance);
        assert_eq!(rows.accesses[0].issuer, rows.issuers[0].definition);
    }
    let floor = budget.storage();
    check_immutable_issued_roles_v18(original, 0, rows, budget)?;
    assert_eq!(budget.storage(), floor);
    let peak = budget.peak_storage();
    for _ in 0..3 {
        check_immutable_issued_roles_v18(original, 0, rows, budget)?;
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), peak);
    }
    Ok(())
}

#[test]
fn issued_pointer_retained_original_roster_includes_used_and_unused_issuers() {
    for used in [false, true] {
        let completed = std::cell::Cell::new(false);
        let (result, _, _) = run_issued_role_source_v18(used, ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT, |original, budget| {
            check_issued_role_positive_v18(original, used, budget)?;
            completed.set(true);
            Ok(())
        });
        result.unwrap();
        assert!(completed.get());
    }
}

#[test]
fn issued_pointer_original_issuers_and_accesses_grow_independently() {
    // These are independent source CFGs, not synthesized receipt vectors.
    // Existing SSA/source-span scans are metered but not claimed linear.
    for grow_issuers in [false, true] {
        let mut previous = None;
        for count in [1, 2, 8, 16] {
            let (issuers, accesses) = if grow_issuers { (count, 1) } else { (1, count) };
            let completed = std::cell::Cell::new(false);
            let (result, work, peak) = run_issued_role_source_shape_v18(issuers, accesses,
                ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT, &std::cell::Cell::new(None), |original, budget| {
                    let rows = issued_rows_v18(original);
                    assert_eq!((rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
                        (issuers, issuers, issuers * accesses));
                    for (ordinal, issuer) in rows.issuers.iter().enumerate() {
                        assert_eq!(issuer.instance, ProductionCallInstanceIdV1(0));
                        assert_eq!(issuer.block.index() as usize, ordinal * 4 + 1);
                        assert_eq!(rows.sources[ordinal].block, issuer.block);
                        assert_eq!(rows.accesses.iter().filter(|row|
                            row.instance == issuer.instance && row.issuer == issuer.definition).count(), accesses);
                    }
                    assert_eq!(rows.retained_storage()?,
                        rows.sources.capacity() * std::mem::size_of::<PendingSourceIssuedSiteV29>()
                        + rows.issuers.capacity() * std::mem::size_of::<PendingSourceIssuedIssuerV29>()
                        + rows.accesses.capacity() * std::mem::size_of::<PendingSourceIssuedAccessV29>());
                    let floor = budget.storage();
                    check_immutable_issued_roles_v18(original, 0, rows, budget)?;
                    let replay_peak = budget.peak_storage();
                    for _ in 0..3 {
                        check_immutable_issued_roles_v18(original, 0, rows, budget)?;
                        assert_eq!(budget.storage(), floor);
                        assert_eq!(budget.peak_storage(), replay_peak);
                    }
                    completed.set(true);
                    Ok(())
                });
            result.unwrap();
            assert!(completed.get());
            if let Some((old_work, old_peak)) = previous {
                assert!(work > old_work && peak > old_peak, "independent roster growth {grow_issuers}/{count}");
            }
            previous = Some((work, peak));
        }
    }
}

fn copied_issued_rows_v18(rows: &PendingSourceIssuedRolesV29, budget: &mut ArgumentBudgetV1<'_>)
    -> SourceOwnedResultV18<PendingSourceIssuedRolesV29>
{
    budget.reserve_storage(std::mem::size_of::<PendingSourceIssuedRolesV29>()
        + 2 * std::mem::size_of::<SourceOwnedResultV18<PendingSourceIssuedRolesV29>>())?;
    let mut copy = PendingSourceIssuedRolesV29 {
        sources: emission_vec_v1(rows.sources.len() + 1, budget).map_err(immutable_memory_error_v29)?,
        issuers: emission_vec_v1(rows.issuers.len() + 1, budget).map_err(immutable_memory_error_v29)?,
        accesses: emission_vec_v1(rows.accesses.len() + 1, budget).map_err(immutable_memory_error_v29)?,
    };
    budget.charge_work(rows.sources.len() + rows.issuers.len() + rows.accesses.len())?;
    copy.sources.extend_from_slice(&rows.sources);
    copy.issuers.extend_from_slice(&rows.issuers);
    copy.accesses.extend_from_slice(&rows.accesses);
    Ok(copy)
}

#[test]
fn issued_pointer_retained_replay_rejects_copied_source_identity_and_guard_mutations() {
    // Copied receipts must match the authentic owner's complete roster before
    // replay. These earlier receipt refusals are not downstream guard proof
    // coverage; actual input/tail/edge mutants remain in the source observer.
    for fault in 0..=13 {
        let used = !matches!(fault, 0 | 9 | 12);
        let completed = std::cell::Cell::new(false);
        let (result, _, _) = run_issued_role_source_v18(used, ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT, |original, budget| {
            check_issued_role_positive_v18(original, used, budget)?;
            let floor = budget.storage();
            let mut copy = copied_issued_rows_v18(issued_rows_v18(original), budget)?;
            let owned = budget.storage() - floor;
            check_immutable_issued_roles_v18(original, 0, &copy, budget)?;
            match fault {
                0 => copy.sources.clear(),
                1 => {
                    let root = original.source.root(0, budget)?.1;
                    let body = original.inventory.functions()[root].function.body.as_ref().unwrap();
                    copy.issuers[0].root_input = body.parameters[1];
                }
                2 => copy.issuers[0].index = copy.issuers[0].length,
                3 => copy.issuers.push(copy.issuers[0]),
                4 => copy.accesses.push(copy.accesses[0]),
                5 => {
                    copy.issuers[0].instance = ProductionCallInstanceIdV1(usize::MAX);
                }
                6 => copy.accesses[0].guard_edge ^= 1,
                7 => copy.issuers[0].length = copy.issuers[0].data,
                8 => copy.sources.push(copy.sources[0]),
                9 => {
                    copy.issuers[0].definition = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(u32::MAX));
                }
                10 => copy.accesses[0].access.alignment += 1,
                11 => copy.accesses[0].writing = !copy.accesses[0].writing,
                12 => copy.issuers.clear(),
                13 => copy.accesses.clear(),
                _ => unreachable!(),
            }
            let error = check_immutable_issued_roles_v18(original, 0, &copy, budget).unwrap_err();
            assert!(matches!(&error, ProductionSourceOwnedViewErrorV18::Binding(
                "issued immutable receipt differs from its original owner")), "fault {fault}: {error:?}");
            drop(copy);
            budget.release_storage(owned)?;
            assert_eq!(budget.storage(), floor);
            let work = budget.work();
            assert!(original.query(budget).is_err());
            assert_eq!(budget.work(), work, "the first source refusal remains selected without another debit");
            completed.set(true);
            // The outer source boundary must still return the retained error.
            Ok(())
        });
        assert!(result.is_err() && completed.get(), "fault {fault}: {result:?}");
    }
}

#[test]
fn issued_pointer_retained_complete_source_transaction_has_exact_resource_boundaries() {
    for used in [false, true] {
        let run = |work, storage| run_issued_role_source_v18(used, work, storage,
            |original, budget| check_issued_role_positive_v18(original, used, budget));
        let (positive, work, storage) = run(ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT);
        positive.unwrap();
        let exact = run(work, storage);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (work, storage));
        for (work_limit, storage_limit, work_cut) in [(work - 1, storage, true), (work, storage - 1, false)] {
            let (result, _, _) = run(work_limit, storage_limit);
            let error = result.unwrap_err();
            let resource = issued_role_resource_v18(error);
            match (work_cut, resource) {
                (true, ArgumentResourceV1::Work(error)) => {
                    assert_eq!(error.limit(), work_limit);
                    assert!(error.actual() > work_limit);
                }
                (false, ArgumentResourceV1::Storage(error)) => {
                    assert_eq!(error.limit(), storage_limit);
                    assert!(error.actual() > storage_limit);
                }
                other => panic!("issued source exact resource boundary: {other:?}"),
            }
        }
    }
}

#[test]
fn issued_pointer_retained_replay_authenticates_foreign_custody_before_debit() {
    let completed = std::cell::Cell::new(false);
    let retained = std::cell::Cell::new(None);
    let (result, _, _) = run_issued_role_source_custody_v18(true, ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT,
        &retained, |original, budget| {
            check_issued_role_positive_v18(original, true, budget)?;
            let rows = issued_rows_v18(original);
            let floor = budget.storage();
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(ISSUED_ROLE_LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, ISSUED_ROLE_LIMIT);
            foreign.reserve_storage(floor)?;
            let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
            let error = check_immutable_issued_roles_v18(original, 0, rows, &mut foreign).unwrap_err();
            assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)));
            assert_eq!((foreign.work(), foreign.storage(), foreign.peak_storage()), before);
            assert_eq!(budget.storage(), floor);
            let work = budget.work();
            assert!(matches!(original.query(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
            assert_eq!(budget.work(), work);
            retained.set(Some(floor));
            completed.set(true);
            Ok(())
        });
    assert!(completed.get());
    assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
    assert!(retained.get().unwrap() > ISSUED_ROLE_FLOOR,
        "denied source custody cannot refund the already-owned backing");
}

#[test]
fn issued_pointer_retained_replay_header_cut_preserves_first_failure_and_cleanup() {
    let completed = std::cell::Cell::new(false);
    let (result, _, _) = run_issued_role_source_v18(true, ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT, |original, budget| {
        check_issued_role_positive_v18(original, true, budget)?;
        let rows = issued_rows_v18(original);
        let floor = budget.storage();
        let header = source_issued_replay_headers_v18()?;
        assert!(header > 0);
        let padding = ISSUED_ROLE_LIMIT - floor - (header - 1);
        budget.reserve_storage(padding)?;
        let padded = budget.storage();
        let error = check_immutable_issued_roles_v18(original, 0, rows, budget).unwrap_err();
        let ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)) = error
            else { panic!("the first immutable replay header debit must refuse: {error:?}"); };
        assert_eq!((error.actual(), error.limit()), (ISSUED_ROLE_LIMIT + 1, ISSUED_ROLE_LIMIT));
        assert_eq!(budget.storage(), padded);
        budget.release_storage(padding)?;
        assert_eq!(budget.storage(), floor);
        budget.charge_work(ISSUED_ROLE_LIMIT - budget.work())?;
        let work = budget.work();
        let second = check_immutable_issued_roles_v18(original, 0, rows, budget).unwrap_err();
        assert!(matches!(second, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(found))
            if found.actual() == error.actual() && found.limit() == error.limit()));
        assert_eq!(budget.work(), work);
        assert_eq!(budget.storage(), floor);
        completed.set(true);
        Ok(())
    });
    assert!(completed.get());
    assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
        if error.actual() == ISSUED_ROLE_LIMIT + 1 && error.limit() == ISSUED_ROLE_LIMIT));
}

fn issued_role_resource_v18(error: ProductionSourceOwnedViewErrorV18) -> ArgumentResourceV1 {
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as V, CanonicalKernelIrReplayAdmissionErrorV18 as C,
        KernelIrDecodeError as D, KernelIrEncodeError as E, StorageLayoutErrorV1 as L,
    };
    use ProductionPendingScopedSourceErrorV29 as Pending;
    use ProductionSourceOwnedViewErrorV18 as View;
    match error {
        View::Resource(error)
        | View::Source(Pending::Source(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)))
        | View::Source(Pending::Source(ProductionSemanticKirErrorV1::AssertOrigin(SemanticKirAssertOriginErrorV1::Resource(error))))
        | View::Source(Pending::Canonical(C::Resource(error)))
        | View::Source(Pending::Canonical(C::Decode(D::Resource(error))))
        | View::Source(Pending::Canonical(C::Layout(L::Resource(error))))
        | View::Source(Pending::Canonical(C::Verification(V::Resource(error))))
        | View::Source(Pending::Occurrences(fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error))) => error,
        View::Source(Pending::Canonical(C::Encode(E::WorkLimit(limit))))
        | View::Source(Pending::Canonical(C::Decode(D::WorkLimit(limit))))
        | View::Source(Pending::Canonical(C::Decode(D::Encode(E::WorkLimit(limit))))) => ArgumentResourceV1::Work(limit),
        other => panic!("exact issued source resource refusal required: {other:?}"),
    }
}
