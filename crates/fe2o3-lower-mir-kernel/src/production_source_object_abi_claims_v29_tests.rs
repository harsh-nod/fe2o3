const OBJECT_ABI_CLAIMS_STOP_V29: &str = "typed object ABI claim component completed";
include!("production_source_reference_pending_reads_v29_tests.rs");
thread_local! {
    static OBJECT_ABI_CLAIMS_CASE_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static OBJECT_ABI_CLAIMS_COMPLETED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn observe_object_abi_claims_v29(
    _: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let references = references.unwrap();
    let mode = OBJECT_ABI_CLAIMS_CASE_V29.get();
    let floor = budget.storage();
    // Measured constructor boundaries, independent of semantic authority tests.
    let mut required = (0, 0);
    with_canonical_call_scratch_v1(budget, |budget| {
        let before = (budget.work(), budget.storage());
        let proof = source_reference_backing_pointer_claims_v29(
            references, emitted, slots, SourceReferencePointerClaimsV29::SelectedBacking, budget,
        )?;
        required = (budget.work() - before.0, budget.storage() - before.1);
        assert!(proof.cell_slots.iter().flatten().any(|&slot|
            matches!(slots.slots[slot].representation, ScopedSlotRepresentationV29::Object { .. })));
        drop(proof);
        Ok(())
    })?;
    assert_eq!(budget.storage(), floor);
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        if mode == 4 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let before = (foreign.work(), foreign.storage());
            let result = source_reference_backing_pointer_claims_v29(
                references, emitted, slots, SourceReferencePointerClaimsV29::SelectedBacking, &mut foreign,
            );
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting))));
            assert_eq!((foreign.work(), foreign.storage()), before);
        } else if matches!(mode, 5 | 6) {
            if mode == 5 {
                budget.charge_work(usize::MAX - budget.work() - required.0 + 1)?;
            } else {
                budget.reserve_storage(usize::MAX - budget.storage() - required.1 + 1)?;
            }
            let result = source_reference_backing_pointer_claims_v29(
                references, emitted, slots, SourceReferencePointerClaimsV29::SelectedBacking, budget,
            );
            assert!(matches!((mode, &result),
                (5, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))))
                | (6, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_))))));
            let failure = (budget.work(), budget.storage());
            assert!(references.check(budget).is_err());
            assert_eq!((budget.work(), budget.storage()), failure);
        } else {
            let mut proof = source_reference_backing_pointer_claims_v29(
                references, emitted, slots,
                if mode == 1 { SourceReferencePointerClaimsV29::ScalarOnly }
                else { SourceReferencePointerClaimsV29::SelectedBacking }, budget,
            )?;
            if mode == 1 {
                // Graph 3 has no helper ABI payloads, so an empty census can
                // succeed. It still cannot supply any typed object's origin.
                proof.check_payloads(emitted, budget)?;
                let (cell, row) = references.plan.cells.rows.iter().enumerate()
                    .find(|(_, row)| matches!(row.kind, SourceBackingKindV29::Object(_))).unwrap();
                assert!(proof.cell_slots[cell].is_none());
                let slot = source_address_object_slot_v29(references.plan.instances,
                    references.plan, slots, row.instance, row.local, row.generation,
                    row.ty, budget)?;
                let key = (row.instance.index(), slots.slots[slot].origin.pointer);
                assert!(proof.require_payload_backing(cell, key, budget).is_err(),
                    "scalar-only proof must not authorize a typed object ABI");
            } else {
                proof.check_payloads(emitted, budget)?;
                let (cell, row) = references.plan.cells.rows.iter().enumerate()
                    .find(|(_, row)| matches!(row.kind, SourceBackingKindV29::Object(_))).unwrap();
                let slot = proof.cell_slots[cell].unwrap();
                let key = (row.instance.index(), slots.slots[slot].origin.pointer);
                proof.require_payload_backing(cell, key, budget)?;
                match mode {
                    0 => {}
                    2 => {
                        let ordinal = source_reference_pointer_ordinal_v29(&proof.ordinals, key, budget)?;
                        proof.origins[ordinal] = origin_worklist_v1::OriginStateV1::Exact(None);
                        assert!(proof.require_payload_backing(cell, key, budget).is_err(),
                            "schema alone is not the original allocation");
                    }
                    3 => assert!(proof.require_cell(cell, key, budget).is_err(),
                        "pending object origin must not acquire scalar memory authority"),
                    _ => unreachable!(),
                }
            }
        }
        OBJECT_ABI_CLAIMS_COMPLETED_V29.set(true);
        Err(source_reference_error_v29(OBJECT_ABI_CLAIMS_STOP_V29))
    });
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn original_object_abi_claims_preserve_typed_origins_without_scalar_authority() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) { SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0); }
    }
    let _restore = Restore(SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe_object_abi_claims_v29)));
    for immutable in [false, true] {
        for mode in 0..7 {
            SELECTED_POINTER_FIXTURE.set((immutable, 3));
            OBJECT_ABI_CLAIMS_CASE_V29.set(mode);
            OBJECT_ABI_CLAIMS_COMPLETED_V29.set(false);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = scalar_payload_prepared_from_v18(selected_pointer_emission_owner_v29, &mut budget);
            let result = prepared.with_source_consumer_v18(&mut budget,
                |_, _| -> SourceOwnedResultV18<()> { panic!("component test cannot grant final source admission"); });
            assert!(OBJECT_ABI_CLAIMS_COMPLETED_V29.get(),
                "immutable={immutable}, mode={mode}: {result:?}");
            assert!(result.is_err());
            if mode <= 3 {
                assert!(format!("{result:?}").contains(OBJECT_ABI_CLAIMS_STOP_V29));
            }
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}
