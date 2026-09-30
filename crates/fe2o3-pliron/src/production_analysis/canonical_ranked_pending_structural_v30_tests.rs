struct StructuralMeasuredV30 {
    result: Result<(), Failure>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    entered: bool,
}

fn measure_structural_pending_v30(
    module: &Module,
    work_limit: usize,
    storage_limit: usize,
) -> StructuralMeasuredV30 {
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryV18 as Inventory, CanonicalRankedMetadataV18 as Metadata,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let mut prepare_work = Work::new(AMPLE);
    let mut prepare = Budget::new(&mut prepare_work, AMPLE);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut prepare,
        )
        .unwrap();
    let floor = receipt.retained_storage() + 37;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let mut entered = false;
    let result = (|| -> Result<(), Failure> {
        let (inventory, receipt) =
            Inventory::derive_v18(&owner, &mut budget).map_err(|error| match error {
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => {
                    Failure::Resource(error)
                }
                _ => Failure::ExactGraph,
            })?;
        budget.reserve_storage(receipt.retained_storage())?;
        let metadata = Metadata::new(&owner, &[]);
        let metadata_storage = metadata.storage_extent(&mut budget)?;
        budget.reserve_storage(metadata_storage)?;
        let (candidate, receipt) =
            build_canonical_ranked_candidate_v18(&inventory, &metadata, &mut budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let prepared = budget.storage();
        let result = with_checked_canonical_ranked_view_v18(
            &inventory,
            &metadata,
            &candidate,
            &mut budget,
            |checked, budget| {
                with_pending_canonical_ranked_source_roles_v18(
                    checked,
                    LAYOUTS,
                    budget,
                    |pending, budget| {
                        let phase_floor = budget.storage();
                        let observed = pending.with_native_observations(budget, |view, budget| {
                        assert!(std::ptr::eq(view.owner(budget)?, &owner));
                        assert!(view.obligations(budget)?.is_empty());
                        assert_eq!(view.function_count(budget)?, 1);
                        assert!(view.report(0, budget)?.unwrap().is_clean());
                        assert_eq!(view.report(0, budget)?.unwrap().pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                        assert!(view.history(0, budget)?.is_some());
                        assert!(!view.ranked_verification_is_complete());
                        entered = true;
                        Ok(())
                    }).map_err(|error| error.failure);
                        assert_eq!(budget.storage(), phase_floor);
                        observed
                    },
                )
            },
        );
        assert_eq!(budget.storage(), prepared);
        drop(candidate);
        drop(metadata);
        drop(inventory);
        result
    })();
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
    StructuralMeasuredV30 {
        result,
        work: budget.work(),
        peak: budget.peak_storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
        entered,
    }
}

#[test]
fn pending_structural_native_runs_all_stages_with_exact_and_one_short_budgets() {
    let module = pointer_flow(false);
    let measured = measure_structural_pending_v30(&module, 500_000_000, 20_000_000);
    measured.result.unwrap();
    assert!(measured.entered);
    let exact = measure_structural_pending_v30(&module, measured.work, measured.peak);
    exact.result.unwrap();
    assert!(exact.entered);
    assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
    let short = measure_structural_pending_v30(&module, measured.work - 1, measured.peak);
    assert!(
        matches!(short.result, Err(Failure::Resource(Resource::Work(error))) if error.actual() == measured.work && error.limit() == measured.work - 1)
    );
    assert_eq!(short.failed_work, Some(measured.work));
    let short = measure_structural_pending_v30(&module, measured.work, measured.peak - 1);
    // The exact peak belongs to canonical layout admission inside extraction.
    assert!(
        matches!(&short.result,
            Err(Failure::StorageBridge(crate::KirBridgeErrorV18::Canonical(
                fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Layout(
                    fe2o3_kernel_ir::StorageLayoutErrorV1::Resource(Resource::Storage(error))
                )
            ))) if error.actual() == measured.peak && error.limit() == measured.peak - 1),
        "storage result {:?}, failed {:?}, measured peak {}",
        short.result,
        short.failed_storage,
        measured.peak
    );
    assert!(!short.entered);
    assert_eq!(short.failed_storage, Some(measured.peak));
    assert_eq!(short.failed_work, None);
}

#[test]
fn pending_structural_native_metadata_padding_has_linear_work_without_changing_stages() {
    let mut observations = Vec::new();
    for padding in [256, 512, 768] {
        let mut module = pointer_flow(false);
        module.id = "m".repeat(padding).into();
        let measured = measure_structural_pending_v30(&module, 500_000_000, 20_000_000);
        measured.result.unwrap();
        assert!(measured.entered);
        observations.push(measured.work);
    }
    assert!(observations[1] > observations[0]);
    assert_eq!(
        observations[2] - observations[1],
        observations[1] - observations[0]
    );
}

#[test]
fn pending_structural_native_rejects_foreign_witness_without_fallback_and_restores_credit() {
    with_checked(&pointer_flow(false), |checked, budget| {
        let owner = checked.inventory(budget).unwrap().owner();
        let floor = budget.storage();
        let (mut first, first_witness, first_storage) =
            crate::kir_bridge_v1::import_structural_native_v30(owner, budget).unwrap();
        budget
            .reserve_storage(first_storage.retained_storage())
            .unwrap();
        let (second, second_witness, second_storage) =
            crate::kir_bridge_v1::import_structural_native_v30(owner, budget).unwrap();
        budget
            .reserve_storage(second_storage.retained_storage())
            .unwrap();
        let retained = budget.storage();
        let epoch = first.ranked_policy_epoch_v18().unwrap();
        let refused = exact_snapshot(&mut first, LAYOUTS, epoch, Some(&second_witness), budget);
        assert!(matches!(
            refused,
            Err(Failure::StorageBridge(crate::KirBridgeErrorV18::Bridge(
                crate::KirBridgeErrorV1::GraphIdentityMismatch
            )))
        ));
        assert_eq!(budget.storage(), retained);
        exact_snapshot(&mut first, LAYOUTS, epoch, Some(&first_witness), budget).unwrap();
        assert_eq!(budget.storage(), retained);
        drop(second_witness);
        drop(second);
        budget
            .release_storage(second_storage.retained_storage())
            .unwrap();
        drop(first_witness);
        drop(first);
        budget
            .release_storage(first_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn pending_structural_native_constructor_frames_have_independent_field_oracles() {
    #[allow(dead_code)]
    struct PendingFields<'s, 'g> {
        owner: &'g VerifiedCanonicalKernelIrModuleV18,
        obligations: &'s [CanonicalRankedSourceObligationV18],
        graph: &'s mut crate::KirPlironGraphV18<'g>,
        structural: &'s crate::kir_bridge_v1::StructuralBridgeWitnessV18,
        epoch: u64,
        layouts: StorageLayoutLimitsV1,
        guard: &'s Guard,
        refund_denied: &'s Cell<bool>,
    }
    assert_eq!(
        size_of::<PendingFields<'_, '_>>(),
        size_of::<PendingCanonicalRankedSourceRolesV18<'_, '_>>()
    );
    assert_eq!(
        std::mem::align_of::<PendingFields<'_, '_>>(),
        std::mem::align_of::<PendingCanonicalRankedSourceRolesV18<'_, '_>>()
    );
    type Snapshot<'a> = (Option<&'a ()>, Option<&'a ()>, &'a (), &'a ());
    let snapshot = size_of::<Snapshot<'_>>() + std::mem::align_of::<Snapshot<'_>>();
    assert_eq!(snapshot_structural_headers_v30(), snapshot);
    type Import<'a> = Result<
        (
            crate::KirPlironGraphV18<'a>,
            crate::kir_bridge_v1::StructuralBridgeWitnessV18,
            crate::KirBridgeStorageV18,
        ),
        crate::KirBridgeErrorV18,
    >;
    assert_eq!(
        pending_structural_headers_v30().unwrap(),
        2 * size_of::<Import<'_>>()
            + snapshot
            + size_of::<&VerifiedCanonicalKernelIrModuleV18>()
            + size_of::<&mut Budget<'_>>()
    );
}
