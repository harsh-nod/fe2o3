use super::*;
use crate::{
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    ProductionSourceLaunchRosterV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;

include!("production_source_issued_root_binding_v29_tests.rs");

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
    run_issued_role_source_custody_v18(
        used,
        work_limit,
        storage_limit,
        &std::cell::Cell::new(None),
        consume,
    )
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
    run_issued_role_source_shape_v18(
        1,
        usize::from(used),
        work_limit,
        storage_limit,
        retained_floor,
        consume,
    )
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
    run_issued_role_owner_v18(
        source_issued_pointer_source_tests_v29::owner_with_shape(issuer_count, access_count),
        work_limit,
        storage_limit,
        retained_floor,
        consume,
    )
}

pub(in super::super) fn run_issued_role_owner_v18(
    owner: ProductionSemanticSsaOwnerV1,
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
    let occurrence_storage = owner.occurrence_storage().unwrap().retained_storage();
    let semantic = owner.source_semantic();
    let hash = *owner.source_semantic_sha256();
    let binding = *semantic.functions()[0]
        .kernel_entry()
        .unwrap()
        .kernel_binding_identity()
        .as_bytes();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        semantic,
        &[ProductionSourceLaunchRootInputV1::new(
            "issued_pointer_source",
            binding,
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let arguments: Vec<_> = ["first", "second"]
        .into_iter()
        .enumerate()
        .map(|(ordinal, name)| {
            let ty = semantic.functions()[0].abi().source_input_types()[ordinal];
            let kind = if shared_slice_leaf_v1(semantic.types(), ty) {
                let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(
                    ScalarTypeV1::U32,
                ));
                let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(
                    ScalarTypeV1::U32,
                ));
                ProductionKernelArgumentAbiKindV18::Descriptor {
                    source: SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::U32),
                    argument: LogicalArgumentV1::shared_slice(
                        ordinal as u16,
                        ValidName::new(name).unwrap(),
                        &source,
                        &layout,
                        (ordinal * 16) as u32,
                    )
                    .unwrap(),
                }
            } else {
                assert_eq!(ty.index(), 4, "the exact original disjoint carrier");
                ProductionKernelArgumentAbiKindV18::Descriptor {
                    source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
                    argument: LogicalArgumentV1::disjoint_slice(
                        ordinal as u16,
                        ValidName::new(name).unwrap(),
                        &source,
                        &layout,
                        fe2o3_kernel_descriptor::AccessMode::ReadWrite,
                        (ordinal * 16) as u32,
                    )
                    .unwrap(),
                }
            };
            ProductionKernelArgumentAbiArgumentV18 {
                semantic_type_identity: semantic.types()[ty.index() as usize].identity(),
                kind,
            }
        })
        .collect();
    let roots = [ProductionKernelArgumentAbiRootV18 {
        kernel_binding: &binding,
        export: "issued_pointer_source",
        arguments: &arguments,
        explicit_argument_bytes: 32,
        kernarg_alignment_bytes: 8,
    }];
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &hash,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget
        .reserve_storage(ISSUED_ROLE_FLOOR + occurrence_storage)
        .unwrap();
    let result = (|| -> SourceOwnedResultV18<()> {
        let prepared =
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                owner,
                launch,
                input,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                        // Same source-owned candidate reaches the complete physical
                        // consumer before any copied-component hostile control.
                        with_checked_source_memory_v29(original, 0, None, budget, |_, budget| {
                            consume(original, budget)
                        })
                    })
                })
            })
        })
    })();
    assert_eq!(
        budget.storage(),
        retained_floor.get().unwrap_or(ISSUED_ROLE_FLOOR),
        "{result:?}"
    );
    (result, budget.work(), budget.peak_storage())
}

pub(in super::super) fn issued_rows_v18<'a>(
    original: &'a ProductionSourceCorrespondenceV18<'_>,
) -> &'a PendingSourceIssuedRolesV29 {
    &original
        .source
        .root_row(0)
        .unwrap()
        .source_slots
        .pending_memory
        .as_ref()
        .unwrap()
        .issued
}

fn check_issued_role_positive_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    used: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let rows = issued_rows_v18(original);
    assert_eq!(
        (rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
        (1, 1, usize::from(used))
    );
    assert_eq!(rows.sources[0].instance, rows.issuers[0].instance);
    assert_eq!(rows.sources[0].block, rows.issuers[0].block);
    assert_eq!(rows.issuers[0].element, ScalarType::U32);
    assert_eq!(rows.issuers[0].access, AccessMode::ReadWrite);
    assert_eq!(
        rows.retained_storage(budget)?,
        rows.sources.capacity() * std::mem::size_of::<PendingSourceIssuedSiteV29>()
            + rows.issuers.capacity() * std::mem::size_of::<PendingSourceIssuedIssuerV29>()
            + rows.accesses.capacity() * std::mem::size_of::<PendingSourceIssuedAccessV29>()
    );
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
        let (result, _, _) = run_issued_role_source_v18(
            used,
            ISSUED_ROLE_LIMIT,
            ISSUED_ROLE_LIMIT,
            |original, budget| {
                check_issued_role_positive_v18(original, used, budget)?;
                completed.set(true);
                Ok(())
            },
        );
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
            let (result, work, peak) = run_issued_role_source_shape_v18(
                issuers,
                accesses,
                ISSUED_ROLE_LIMIT,
                ISSUED_ROLE_LIMIT,
                &std::cell::Cell::new(None),
                |original, budget| {
                    let rows = issued_rows_v18(original);
                    assert_eq!(
                        (rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
                        (issuers, issuers, issuers * accesses)
                    );
                    for (ordinal, issuer) in rows.issuers.iter().enumerate() {
                        assert_eq!(issuer.instance, ProductionCallInstanceIdV1(0));
                        assert_eq!(issuer.block.index() as usize, ordinal * 4 + 1);
                        assert_eq!(rows.sources[ordinal].block, issuer.block);
                        assert_eq!(
                            rows.accesses
                                .iter()
                                .filter(|row| row.instance == issuer.instance
                                    && row.issuer == issuer.definition)
                                .count(),
                            accesses
                        );
                    }
                    assert_eq!(
                        rows.retained_storage(budget)?,
                        rows.sources.capacity() * std::mem::size_of::<PendingSourceIssuedSiteV29>()
                            + rows.issuers.capacity()
                                * std::mem::size_of::<PendingSourceIssuedIssuerV29>()
                            + rows.accesses.capacity()
                                * std::mem::size_of::<PendingSourceIssuedAccessV29>()
                    );
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
                },
            );
            result.unwrap();
            assert!(completed.get());
            if let Some((old_work, old_peak)) = previous {
                assert!(
                    work > old_work && peak > old_peak,
                    "independent roster growth {grow_issuers}/{count}"
                );
            }
            previous = Some((work, peak));
        }
    }
}

#[test]
fn issued_pointer_reused_original_option_local_remains_an_explicit_admission_gap() {
    use source_issued_pointer_source_tests_v29::{
        owner_with_reused_option_issuers, owner_with_shape,
    };
    let single = owner_with_shape(1, 1);
    let old_single = owner_with_reused_option_issuers(1, 1);
    assert_eq!(
        single.source_semantic_sha256(),
        old_single.source_semantic_sha256(),
        "the one-issuer original source identity is unchanged"
    );
    let owner = owner_with_shape(2, 1);
    let old = owner_with_reused_option_issuers(2, 1);
    assert_ne!(
        owner.source_semantic_sha256(),
        old.source_semantic_sha256(),
        "independent local producers are a fixture correction, not a same-source recovery"
    );
    assert_eq!(old.source_semantic().functions()[0].locals().len(), 9);
    assert_eq!(owner.source_semantic().functions()[0].locals().len(), 10);
    let complete = std::cell::Cell::new(false);
    let positive = run_issued_role_owner_v18(
        owner,
        ISSUED_ROLE_LIMIT,
        ISSUED_ROLE_LIMIT,
        &std::cell::Cell::new(None),
        |original, budget| {
            let rows = issued_rows_v18(original);
            assert_eq!(
                (rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
                (2, 2, 2)
            );
            check_immutable_issued_roles_v18(original, 0, rows, budget)?;
            complete.set(true);
            Ok(())
        },
    )
    .0;
    positive.unwrap();
    assert!(complete.get());
    let entered = std::cell::Cell::new(false);
    let refused = run_issued_role_owner_v18(
        old,
        ISSUED_ROLE_LIMIT,
        ISSUED_ROLE_LIMIT,
        &std::cell::Cell::new(None),
        |_, _| {
            entered.set(true);
            Ok(())
        },
    )
    .0;
    assert!(!entered.get());
    assert!(
        matches!(
            refused,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "an Option capability local does not have one exact producer",
                    }
                )
            ))
        ),
        "{refused:?}"
    );
}

#[test]
fn issued_pointer_reused_original_discriminator_remains_an_explicit_admission_gap() {
    use source_issued_pointer_source_tests_v29::{
        owner_with_reused_discriminator_issuers, owner_with_shape,
    };
    assert_eq!(
        owner_with_shape(1, 1).source_semantic_sha256(),
        owner_with_reused_discriminator_issuers(1, 1).source_semantic_sha256()
    );
    let owner = owner_with_shape(2, 1);
    let reused = owner_with_reused_discriminator_issuers(2, 1);
    assert_ne!(
        owner.source_semantic_sha256(),
        reused.source_semantic_sha256(),
        "distinct discriminators change the original source, not production admission"
    );
    assert_eq!(reused.source_semantic().functions()[0].locals().len(), 9);
    let completed = std::cell::Cell::new(false);
    run_issued_role_owner_v18(
        owner,
        ISSUED_ROLE_LIMIT,
        ISSUED_ROLE_LIMIT,
        &std::cell::Cell::new(None),
        |original, budget| {
            let rows = issued_rows_v18(original);
            assert_eq!(
                (rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
                (2, 2, 2)
            );
            check_immutable_issued_roles_v18(original, 0, rows, budget)?;
            completed.set(true);
            Ok(())
        },
    )
    .0
    .unwrap();
    assert!(completed.get());
    let entered = std::cell::Cell::new(false);
    let refused = run_issued_role_owner_v18(
        reused,
        ISSUED_ROLE_LIMIT,
        ISSUED_ROLE_LIMIT,
        &std::cell::Cell::new(None),
        |_, _| {
            entered.set(true);
            Ok(())
        },
    )
    .0;
    assert!(!entered.get());
    assert!(
        matches!(
            refused,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "an Option capability discriminator does not have one exact definition",
                    }
                )
            ))
        ),
        "{refused:?}"
    );
}

fn copied_issued_rows_v18(
    rows: &PendingSourceIssuedRolesV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<PendingSourceIssuedRolesV29> {
    budget.reserve_storage(
        std::mem::size_of::<PendingSourceIssuedRolesV29>()
            + 2 * std::mem::size_of::<SourceOwnedResultV18<PendingSourceIssuedRolesV29>>(),
    )?;
    let mut copy = PendingSourceIssuedRolesV29 {
        sources: emission_vec_v1(rows.sources.len() + 1, budget)
            .map_err(immutable_memory_error_v29)?,
        issuers: emission_vec_v1(rows.issuers.len() + 1, budget)
            .map_err(immutable_memory_error_v29)?,
        accesses: emission_vec_v1(rows.accesses.len() + 1, budget)
            .map_err(immutable_memory_error_v29)?,
        selected: copied_selected_rows_v30(&rows.selected, budget)
            .map_err(immutable_memory_error_v29)?,
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
    for fault in 0..=14 {
        let used = !matches!(fault, 0 | 9 | 12);
        let completed = std::cell::Cell::new(false);
        let (result, _, _) = run_issued_role_source_v18(
            used,
            ISSUED_ROLE_LIMIT,
            ISSUED_ROLE_LIMIT,
            |original, budget| {
                check_issued_role_positive_v18(original, used, budget)?;
                let floor = budget.storage();
                let mut copy = copied_issued_rows_v18(issued_rows_v18(original), budget)?;
                let owned = budget.storage() - floor;
                check_immutable_issued_roles_v18(original, 0, &copy, budget)?;
                match fault {
                    0 => copy.sources.clear(),
                    1 => {
                        let root = original.source.root(0, budget)?.1;
                        let body = original.inventory.functions()[root]
                            .function
                            .body
                            .as_ref()
                            .unwrap();
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
                        copy.issuers[0].definition = SsaValueV1::Definition(
                            fe2o3_mir_model::SsaDefinitionIdV1::new(u32::MAX),
                        );
                    }
                    10 => copy.accesses[0].access.alignment += 1,
                    11 => copy.accesses[0].writing = !copy.accesses[0].writing,
                    12 => copy.issuers.clear(),
                    13 => copy.accesses.clear(),
                    14 => copy.accesses[0].access.volatile = !copy.accesses[0].access.volatile,
                    _ => unreachable!(),
                }
                let error =
                    check_immutable_issued_roles_v18(original, 0, &copy, budget).unwrap_err();
                assert!(
                    matches!(
                        &error,
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "issued immutable receipt differs from its original owner"
                        )
                    ),
                    "fault {fault}: {error:?}"
                );
                drop(copy);
                budget.release_storage(owned)?;
                assert_eq!(budget.storage(), floor);
                let work = budget.work();
                assert!(original.query(budget).is_err());
                assert_eq!(
                    budget.work(),
                    work,
                    "the first source refusal remains selected without another debit"
                );
                completed.set(true);
                // The outer source boundary must still return the retained error.
                Ok(())
            },
        );
        assert!(
            result.is_err() && completed.get(),
            "fault {fault}: {result:?}"
        );
    }
}

#[test]
fn issued_pointer_retained_complete_source_transaction_has_exact_resource_boundaries() {
    for used in [false, true] {
        let run = |work, storage| {
            run_issued_role_source_v18(used, work, storage, |original, budget| {
                check_issued_role_positive_v18(original, used, budget)
            })
        };
        let (positive, work, storage) = run(ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT);
        positive.unwrap();
        let exact = run(work, storage);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (work, storage));
        for (work_limit, storage_limit, work_cut) in
            [(work - 1, storage, true), (work, storage - 1, false)]
        {
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
    let (result, _, _) = run_issued_role_source_custody_v18(
        true,
        ISSUED_ROLE_LIMIT,
        ISSUED_ROLE_LIMIT,
        &retained,
        |original, budget| {
            check_issued_role_positive_v18(original, true, budget)?;
            let rows = issued_rows_v18(original);
            let floor = budget.storage();
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(ISSUED_ROLE_LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, ISSUED_ROLE_LIMIT);
            foreign.reserve_storage(floor)?;
            let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
            let error =
                check_immutable_issued_roles_v18(original, 0, rows, &mut foreign).unwrap_err();
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            assert_eq!(
                (foreign.work(), foreign.storage(), foreign.peak_storage()),
                before
            );
            assert_eq!(budget.storage(), floor);
            let work = budget.work();
            assert!(matches!(
                original.query(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(budget.work(), work);
            retained.set(Some(floor));
            completed.set(true);
            Ok(())
        },
    );
    assert!(completed.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(
        retained.get().unwrap() > ISSUED_ROLE_FLOOR,
        "denied source custody cannot refund the already-owned backing"
    );
}

#[test]
fn issued_pointer_retained_replay_header_cut_preserves_first_failure_and_cleanup() {
    for cut in 0..3 {
        let completed = std::cell::Cell::new(false);
        let (result, _, _) = run_issued_role_source_v18(
            true,
            ISSUED_ROLE_LIMIT,
            ISSUED_ROLE_LIMIT,
            |original, budget| {
                check_issued_role_positive_v18(original, true, budget)?;
                let rows = issued_rows_v18(original);
                let floor = budget.storage();
                type Attempt<'a, 's> = (
                    &'a ProductionSourceCorrespondenceV18<'s>,
                    &'a usize,
                    &'a PendingSourceIssuedRolesV29,
                );
                let explicit = source_issued_replay_headers_v18()?;
                let attempt = scoped_source_attempt_header_oracle_v29::<
                    usize,
                    ProductionSourceOwnedViewErrorV18,
                    Attempt<'_, '_>,
                >();
                let remaining = match cut {
                    0 => attempt - 1,
                    1 => attempt,
                    _ => attempt + explicit - 1,
                };
                let excess = if cut == 1 { explicit } else { 1 };
                let padding = ISSUED_ROLE_LIMIT - floor - remaining;
                budget.reserve_storage(padding)?;
                let padded = budget.storage();
                let error =
                    check_immutable_issued_roles_v18(original, 0, rows, budget).unwrap_err();
                let ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)) =
                    error
                else {
                    panic!("the first immutable replay header debit must refuse: {error:?}");
                };
                assert_eq!(
                    (error.actual(), error.limit()),
                    (ISSUED_ROLE_LIMIT + excess, ISSUED_ROLE_LIMIT)
                );
                assert_eq!(budget.storage(), padded);
                budget.release_storage(padding)?;
                assert_eq!(budget.storage(), floor);
                budget.charge_work(ISSUED_ROLE_LIMIT - budget.work())?;
                let work = budget.work();
                let second =
                    check_immutable_issued_roles_v18(original, 0, rows, budget).unwrap_err();
                assert!(
                    matches!(second, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(found))
            if found.actual() == error.actual() && found.limit() == error.limit())
                );
                assert_eq!(budget.work(), work);
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok(())
            },
        );
        assert!(completed.get());
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
        if error.actual() > ISSUED_ROLE_LIMIT && error.limit() == ISSUED_ROLE_LIMIT)
        );
    }
}

pub(in super::super) fn issued_role_resource_v18(
    error: ProductionSourceOwnedViewErrorV18,
) -> ArgumentResourceV1 {
    use ProductionPendingScopedSourceErrorV29 as Pending;
    use ProductionSourceOwnedViewErrorV18 as View;
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as V, CanonicalKernelIrReplayAdmissionErrorV18 as C,
        KernelIrDecodeError as D, KernelIrEncodeError as E, StorageLayoutErrorV1 as L,
    };
    match error {
        View::Resource(error)
        | View::Source(Pending::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
        ))
        | View::Source(Pending::Source(ProductionSemanticKirErrorV1::AssertOrigin(
            SemanticKirAssertOriginErrorV1::Resource(error),
        )))
        | View::Source(Pending::Canonical(C::Resource(error)))
        | View::Source(Pending::Canonical(C::Decode(D::Resource(error))))
        | View::Source(Pending::Canonical(C::Layout(L::Resource(error))))
        | View::Source(Pending::Canonical(C::Verification(V::Resource(error))))
        | View::Source(Pending::Occurrences(
            fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error),
        )) => error,
        View::Source(Pending::Canonical(C::Encode(E::WorkLimit(limit))))
        | View::Source(Pending::Canonical(C::Decode(D::WorkLimit(limit))))
        | View::Source(Pending::Canonical(C::Decode(D::Encode(E::WorkLimit(limit))))) => {
            ArgumentResourceV1::Work(limit)
        }
        other => panic!("exact issued source resource refusal required: {other:?}"),
    }
}

fn run_issued_ordered_roster_v18(
    volatile: bool,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let owner = source_issued_pointer_source_tests_v29::owner_with_shape_and_effects(
        1,
        1,
        false,
        Some(if volatile {
            SemanticVolatilityV1::Volatile
        } else {
            SemanticVolatilityV1::NonVolatile
        }),
    );
    let completed = std::cell::Cell::new(false);
    let result = run_issued_role_owner_v18(
        owner,
        work,
        storage,
        &std::cell::Cell::new(None),
        |original, budget| {
            check_issued_role_positive_v18(original, true, budget)?;
            let rows = issued_rows_v18(original);
            assert_eq!(
                rows.accesses.len(),
                1,
                "ordered accesses cannot disappear from the source census"
            );
            assert_eq!(rows.accesses[0].access.volatile, volatile);
            assert!(rows.accesses[0].writing);
            completed.set(true);
            Ok(())
        },
    );
    if result.0.is_ok() {
        assert!(completed.get());
    }
    result
}

#[test]
fn issued_pointer_ordered_store_retains_exact_issuer_access_and_resource_census() {
    for volatile in [false, true] {
        let (result, work, storage) =
            run_issued_ordered_roster_v18(volatile, ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT);
        result.unwrap();
        let exact = run_issued_ordered_roster_v18(volatile, work, storage);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (work, storage));
        assert!(
            matches!(issued_role_resource_v18(run_issued_ordered_roster_v18(volatile, work - 1, storage).0.unwrap_err()),
            ArgumentResourceV1::Work(error) if error.limit() == work - 1 && error.actual() > work - 1)
        );
        assert!(
            matches!(issued_role_resource_v18(run_issued_ordered_roster_v18(volatile, work, storage - 1).0.unwrap_err()),
            ArgumentResourceV1::Storage(error) if error.limit() == storage - 1 && error.actual() > storage - 1)
        );
    }
}

#[test]
fn issued_pointer_original_effect_replay_rejects_added_dropped_store_bit_independently() {
    // This is a copied-operation helper control. Actual candidate mutations are
    // independently exercised by the final-source observer tests.
    for volatile in [false, true] {
        let owner = source_issued_pointer_source_tests_v29::owner_with_shape_and_effects(
            1,
            1,
            false,
            Some(if volatile {
                SemanticVolatilityV1::Volatile
            } else {
                SemanticVolatilityV1::NonVolatile
            }),
        );
        let completed = std::cell::Cell::new(false);
        let result = run_issued_role_owner_v18(
            owner,
            ISSUED_ROLE_LIMIT,
            ISSUED_ROLE_LIMIT,
            &std::cell::Cell::new(None),
            |original, budget| {
                check_issued_role_positive_v18(original, true, budget)?;
                let row = issued_rows_v18(original).accesses[0];
                let [position] = original.attachment_range(
                    TileAttachmentKeyV29 {
                        root: 0,
                        family: TileAttachmentFamilyV29::MemoryAnchor,
                        instance: row.instance.index(),
                        row: row.anchor,
                        field: TileAttachmentFieldV29::MemoryPosition,
                        component: 0,
                        part: 0,
                    },
                    budget,
                )?
                else {
                    panic!("one exact original position");
                };
                let ProductionSourceOperationV18::Operation(coordinate) =
                    original.mapped_source_operation(position.location, budget)?
                else {
                    panic!("actual original Store");
                };
                let operation =
                    source_operation_row_v18(original.inventory, coordinate, budget)?.operation;
                check_issued_original_effect_v18(
                    original,
                    0,
                    row.instance.index(),
                    row.anchor,
                    operation,
                    budget,
                )?;
                let mut changed = operation.clone();
                let OperationKind::Store { access, .. } = &mut changed.kind else {
                    panic!("issued Store");
                };
                assert_eq!(access.volatile, volatile);
                access.volatile = !volatile;
                let error = check_issued_original_effect_v18(
                    original,
                    0,
                    row.instance.index(),
                    row.anchor,
                    &changed,
                    budget,
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Source(
                        ProductionPendingScopedSourceErrorV29::Source(
                            ProductionSemanticKirErrorV1::Unsupported {
                                detail: "scoped memory anchors differ from their source instance",
                                ..
                            }
                        )
                    )
                ));
                // The detailed Source error is propagated by this closed
                // caller. Only Resource/Binding/Analysis are sticky query
                // failures; this copied-operation control invents no new rule.
                completed.set(true);
                Err(error)
            },
        )
        .0;
        assert!(completed.get() && result.is_err(), "{result:?}");
    }
}
