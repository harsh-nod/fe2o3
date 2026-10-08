//! Closed mapped-route accounting only; no native/Request owner is manufactured.
use super::*;
use std::error::Error as _;

fn expected() -> NativeConditionalCpuMappingExpectationV1 {
    NativeConditionalCpuMappingExpectationV1 {
        rustc_invocation_sha256: [3; 32],
        native_policy_sha256: [5; 32],
        policy_generation: 7,
        enrollment_binding_count: 1,
    }
}

#[test]
fn mapped_entry_prepays_decoder_views_and_actual_expectation_backing() {
    let expected = expected();
    let selection = CpuSelection::Mapping(&expected);
    let floor = METADATA + CAPACITY + selection_backing(selection) + 19;
    let peak = floor + HEADER + WORKING + selection_working(selection);
    assert!(selection_working(selection) >= RUSTC_ENROLLMENT_INVENTORY_WORKING_STORAGE_V1);
    for (storage_limit, success) in [(peak, true), (peak - 1, false)] {
        let mut work = Work::new(10);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = begin_selected(CAPACITY, selection, &mut budget);
        if success {
            let entry = result.unwrap();
            finish_with_origin_working(
                &entry,
                selection_working(selection),
                &mut budget,
                Ok(((), HEADER)),
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), peak);
        } else {
            assert!(matches!(
                result,
                Err(Error(Cause::Resource(Resource::Storage(_))))
            ));
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), Some(peak));
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    let short = METADATA + CAPACITY + selection_backing(selection) - 1;
    budget.reserve_storage(short).unwrap();
    assert!(matches!(
        begin_selected(CAPACITY, selection, &mut budget),
        Err(Error(Cause::Resource(Resource::Accounting)))
    ));
    assert_eq!(budget.storage(), short);
}

#[test]
fn mapped_terminal_error_and_unwind_keep_original_charges_after_drop() {
    let expected = expected();
    let selection = CpuSelection::Mapping(&expected);
    let floor = METADATA + CAPACITY + selection_backing(selection) + 19;
    for panic in [false, true] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, MAX_STORAGE);
        budget.reserve_storage(floor).unwrap();
        let drops = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let entry = begin_selected(CAPACITY, selection, &mut budget)?;
            budget.reserve_storage(17)?;
            let _owned = Dropped(&drops);
            if panic {
                panic!("mapped component unwind");
            }
            let result: Result<((), usize), Error> = Err(Error(Cause::Inventory(
                fe2o3_compiler_lineage::RustcEnrollmentInventoryErrorV1::Header,
            )));
            finish_with_origin_working(&entry, selection_working(selection), &mut budget, result)
        }));
        assert_eq!(drops.get(), 1);
        assert_eq!(
            budget.storage(),
            floor + HEADER + WORKING + selection_working(selection) + 17
        );
        if !panic {
            assert!(result.unwrap().unwrap_err().source().is_none());
        } else {
            assert!(result.is_err());
        }
    }
}

#[test]
fn mapped_original_window_retains_overlap_and_unreserves_only_completed_success() {
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    let expected = expected();
    let selection = CpuSelection::Mapping(&expected);
    let inputs = METADATA + CAPACITY + selection_backing(selection);
    let outside = MAX_STORAGE + inputs;
    let peak = outside
        + inputs
        + Budget::STORAGE_WINDOW_SCRATCH_V1
        + HEADER
        + WORKING
        + selection_working(selection);
    let mut owned = Owned::new(Work::new(100), peak);
    owned.with_budget(|budget| {
        budget.reserve_storage(outside).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        original_recovery(inputs, budget, |budget| {
            let entry = begin_selected(CAPACITY, selection, budget)?;
            finish_with_origin_working(
                &entry,
                selection_working(selection),
                budget,
                Ok(((), HEADER)),
            )
        })
        .unwrap();
        assert_eq!(budget.storage(), outside);
        assert_eq!(budget.peak_storage(), peak);
        assert!(budget.work_ledger_identity_v1() == ledger);
    });
}

#[test]
fn legacy_native_selector_representation_and_bills_do_not_expand() {
    let rows = [NativeConditionalCpuExpectationV1 {
        semantic_root: 4,
        origin: crate::NativeConditionalCpuOriginExpectationV1::SourceRegistrationV1,
    }];
    for origins in [None, Some(rows.as_slice())] {
        assert_eq!(
            selection_backing(CpuSelection::Legacy(origins)),
            origin_backing(origins)
        );
        assert_eq!(
            selection_working(CpuSelection::Legacy(origins)),
            origin_working(origins)
        );
    }
}

#[allow(dead_code)]
fn mapped_native_public_call_shapes(
    handoff: Handoff,
    accepted: &[NativeConditionalRootPolicyV2<'_>],
    expected: &NativeConditionalCpuMappingExpectationV1,
    limits: Limits,
    profile: Profile,
    policy: &[u8],
    budget: &mut Budget<'_>,
    route: u8,
) {
    match route {
        0 => {
            let _ = crate::recover_compiler_conditional_native_semantic_handoff_with_cpu_mapping_v5(
                handoff, accepted, expected, limits, profile, budget,
            );
        }
        1 => {
            let _ = crate::recover_compiler_conditional_native_semantic_handoff_with_cpu_mapping_in_original_account_v5(handoff, accepted, expected, limits, profile, budget);
        }
        _ => {
            let _ = crate::recover_native_conditional_handoff_under_policy_file_with_cpu_mapping_v1(
                policy, handoff, expected, budget,
            );
        }
    }
}
