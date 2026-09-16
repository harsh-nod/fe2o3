use super::*;
use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;

fn optimize_policy3(
    input: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CheckedNeutralKernelIrOwnerPolicy3V1 {
    let observed =
        fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(input, budget).unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    let checked = observed.try_check_and_finish_v1(budget).unwrap();
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    checked
}

// These lowerer fixtures keep N=B, as the existing ordinary endpoint tests do.
// Source construction has its own fixture ledger; its returned receipt is paid
// before actual optimization and every endpoint operation on this live ledger.
fn with_policy3_output(
    body: impl FnOnce(
        &CheckedNeutralKernelIrOwnerPolicy3V1,
        &ProductionSourceOutputOccurrencesV1<'_, '_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let source = array_owner(ArrayCase::Write { sparse: true });
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let source_storage = retained(&source);
    budget.reserve_storage(source_storage).unwrap();
    let checked = optimize_policy3(source.executable(), &mut budget);
    let (coordinates, coordinate_storage) = check_canonical_kir_coordinate_preservation_v1(
        source.executable(),
        source.executable(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(coordinate_storage.retained_storage())
        .unwrap();
    let before = budget.storage();
    let (view, storage) =
        derive_source_output_occurrences_policy3_v1(&source, &coordinates, &checked, &mut budget)
            .unwrap();
    assert_eq!(budget.storage(), before);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(view.storage, storage);
    let live = budget.storage();
    let history = budget.work();
    body(&checked, &view, &mut budget);
    assert_eq!(budget.storage(), live);
    assert!(budget.work() >= history);
    drop(view);
    budget.release_storage(storage.retained_storage()).unwrap();
    // NLL ends the coordinate borrow before its reservation is released.
    budget
        .release_storage(coordinate_storage.retained_storage())
        .unwrap();
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(source);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn actual_policy3_endpoint_keeps_changed_output_and_original_execution_witness() {
    with_policy3_output(|checked, view, budget| {
        assert!(std::ptr::eq(view.output(), checked.owner()));
        assert_eq!(
            view.bound().canonical().canonical_bytes(),
            checked.native_input_audit_bytes()
        );
        assert_ne!(
            view.bound().canonical().identity(),
            view.output().canonical().identity()
        );
        assert_eq!(
            (operations(view.bound()), operations(view.output())),
            (7, 6)
        );
        assert_eq!(checked.report().passes().len(), 8);
        let before = budget.work();
        let execution = view.policy3_execution_v1(budget).unwrap().unwrap();
        assert_eq!(budget.work(), before + 1);
        assert!(std::ptr::eq(execution, checked.execution()));
        assert_eq!(
            execution.canonical_bytes(),
            checked.execution().canonical_bytes()
        );
        assert_eq!(execution.policy_version(), 3);
        assert!(!execution.grants_authority());
        assert!(!view.grants_authority());
        assert!(matches!(
            view.private_array_write(
                ARRAY_ROOT,
                ARRAY_ROOT,
                site(2, 1),
                Role::Destination,
                budget
            )
            .unwrap(),
            ProductionSourceOutputPrivateArrayAccessV1::Retained {
                executable: true,
                ..
            }
        ));
    });
}

#[test]
fn paid_execution_getter_has_literal_one_zero_boundaries() {
    with_policy3_output(|checked, view, outer| {
        // Query-only boundaries; no optimizer or constructor accounting is reset.
        // The outer live ledger retains the actual input/output owners throughout.
        for allowance in [1, 0] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + allowance);
            let mut budget = AssertOriginBudgetV1::new(&mut work, outer.storage());
            budget.charge_work(11).unwrap();
            budget.reserve_storage(outer.storage()).unwrap();
            let floor = budget.storage();
            let result = view.policy3_execution_v1(&mut budget);
            if allowance == 1 {
                assert!(std::ptr::eq(result.unwrap().unwrap(), checked.execution()));
                assert_eq!(budget.work(), 12);
            } else {
                assert!(matches!(result,
                    Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(ref error)))
                        if error.actual() == 12));
                assert_eq!(budget.work(), 11);
            }
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor);
            assert_eq!(work.failed_work(), (allowance == 0).then_some(12));
        }
    });
}

#[test]
fn historical_endpoint_never_supplies_policy3_execution() {
    with_output(
        array_owner(ArrayCase::Write { sparse: true }),
        |view, budget| {
            let before = budget.work();
            assert!(view.policy3_execution_v1(budget).unwrap().is_none());
            assert_eq!(budget.work(), before + 1);
        },
    );
}

#[test]
fn semantic_receipt_endpoint_has_no_execution_provenance_even_for_policy3_rows() {
    with_policy3_output(|checked, view, budget| {
        let before = budget.storage();
        let (receipt, storage) =
            fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
                view.bound().canonical().identity(),
                view.output().canonical().identity(),
                checked.occurrences().candidate(),
                budget,
            )
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        // This component uses the actual O and real encoded candidate. A semantic
        // endpoint cannot manufacture producer execution by inspecting its rows.
        let endpoint = SourceOutputCheckedEndpointV1::Receipt {
            output: checked.owner(),
            receipt: &receipt,
            retained: checked.storage().retained_storage() + storage.retained_storage(),
        };
        assert!(std::ptr::eq(endpoint.owner(), checked.owner()));
        assert!(endpoint.policy3_execution().is_none());
        assert!(endpoint.native_input_audit_bytes().is_none());
        drop(receipt);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), before);
    });
}

#[test]
fn wrong_bound_audit_preserves_historical_and_policy3_error_work_prefixes() {
    let source = array_owner(ArrayCase::Write { sparse: false });
    let history_source = array_owner(ArrayCase::Write { sparse: true });
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let source_storage = retained(&source);
    let history_storage = retained(&history_source);
    budget
        .reserve_storage(FLOOR + source_storage + history_storage)
        .unwrap();
    let historical = optimize(history_source.executable(), &mut budget);
    let policy3 = optimize_policy3(history_source.executable(), &mut budget);
    let (coordinates, coordinate_storage) = check_canonical_kir_coordinate_preservation_v1(
        source.executable(),
        source.executable(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(coordinate_storage.retained_storage())
        .unwrap();
    for endpoint in [
        SourceOutputCheckedEndpointV1::Optimizer(&historical),
        SourceOutputCheckedEndpointV1::OptimizerPolicy3(&policy3),
    ] {
        let audit = endpoint.native_input_audit_bytes().unwrap();
        assert_ne!(source.executable().canonical().canonical_bytes(), audit);
        // Entry5 + endpoint dispatch1 + two lengths2 + both complete byte spans.
        let required = 8 + source.executable().canonical().canonical_bytes().len() + audit.len();
        for allowance in [required, required - 1] {
            let mut query_work = CanonicalKernelIrWorkBudgetV1::new(13 + allowance);
            let mut query = AssertOriginBudgetV1::new(&mut query_work, budget.storage());
            query.charge_work(13).unwrap();
            query.reserve_storage(budget.storage()).unwrap();
            let floor = query.storage();
            let result = derive_source_output_occurrences_with_endpoint_v1(
                &source,
                &coordinates,
                endpoint,
                &mut query,
            );
            if allowance == required {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::InputCustody)
                ));
                assert_eq!(query.work(), 13 + required);
            } else {
                assert!(matches!(result,
                    Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(ref error)))
                        if error.actual() == 13 + required));
                assert_eq!(query.work(), 13 + 8);
            }
            assert_eq!(query.storage(), floor);
            assert_eq!(query.peak_storage(), floor);
            assert_eq!(
                query_work.failed_work(),
                (allowance != required).then_some(13 + required)
            );
        }
    }
    budget
        .release_storage(coordinate_storage.retained_storage())
        .unwrap();
    let checked_storage =
        historical.storage().retained_storage() + policy3.storage().retained_storage();
    drop((historical, policy3));
    budget.release_storage(checked_storage).unwrap();
    drop((source, history_source));
    budget
        .release_storage(source_storage + history_storage)
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn equal_bytes_foreign_source_is_rejected_before_input_or_output_retention() {
    with_policy3_output(|checked, view, outer| {
        let foreign = array_owner(ArrayCase::Write { sparse: true });
        assert_eq!(
            foreign.executable().canonical().canonical_bytes(),
            view.source().executable().canonical().canonical_bytes()
        );
        let (coordinates, storage) = check_canonical_kir_coordinate_preservation_v1(
            view.source().executable(),
            view.bound(),
            outer,
        )
        .unwrap();
        outer.reserve_storage(storage.retained_storage()).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(17 + 5);
        let mut budget = AssertOriginBudgetV1::new(&mut work, outer.storage());
        budget.charge_work(17).unwrap();
        budget.reserve_storage(outer.storage()).unwrap();
        let floor = budget.storage();
        assert!(matches!(
            derive_source_output_occurrences_policy3_v1(
                &foreign,
                &coordinates,
                checked,
                &mut budget,
            ),
            Err(ProductionSourceOutputErrorV1::InputCustody)
        ));
        assert_eq!(budget.work(), 22);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(work.failed_work(), None);
        outer.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn policy3_constructor_rejects_unfunded_owner_floor_without_releasing_prefix() {
    with_policy3_output(|checked, view, outer| {
        let (coordinates, storage) = check_canonical_kir_coordinate_preservation_v1(
            view.source().executable(),
            view.bound(),
            outer,
        )
        .unwrap();
        outer.reserve_storage(storage.retained_storage()).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(19 + 5);
        let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR);
        budget.charge_work(19).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        assert!(matches!(
            derive_source_output_occurrences_policy3_v1(
                view.source(),
                &coordinates,
                checked,
                &mut budget,
            ),
            Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting
            ))
        ));
        assert_eq!(budget.work(), 24);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR);
        assert_eq!(work.failed_work(), None);
        outer.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn policy3_endpoint_receipt_accounts_for_actual_current_view_header() {
    with_policy3_output(|_, view, _| {
        // This source has one private write, no assertions or eligible Global
        // effects, and two empty 56-byte catalog encodings. Headers are already
        // embedded in View; count only the actual separately retained payloads.
        assert_eq!(view.private_arrays.len(), 1);
        assert_eq!(
            (view.global_accesses.len(), view.global_accesses.capacity()),
            (0, 0)
        );
        assert_eq!(view.catalogs.source.canonical_bytes().len(), 56);
        assert_eq!(
            view.catalogs.transported.catalog().canonical_bytes().len(),
            56
        );
        // The sparse source jumps from block0 to block2; block1 is unreachable.
        // Retain that original edge occurrence even after CFG folding.
        assert_eq!(view.checked_control_rows.edges.len(), 1);
        assert!(view.checked_control_rows.edges.capacity() >= 4);
        assert_eq!(view.checked_control_rows.uses.capacity(), 0);
        assert_eq!(view.checked_control_rows.arguments.capacity(), 0);
        assert_eq!(view.checked_control_rows.compare_uses.capacity(), 0);
        let payload = view.blocks.capacity() * std::mem::size_of::<SourceOutputBlockRowV1>()
            + view.private_arrays.capacity() * std::mem::size_of::<SourceOutputArrayRowV1>()
            + view.global_spans.capacity() * std::mem::size_of::<SourceOutputGlobalSpanV1>()
            + single_function_zero_return_payload(view)
            + single_private_store_value_payload(view)
            + view.checked_control_rows.edges.capacity()
                * std::mem::size_of::<SourceOutputControlEdgeRowV1>()
            + 2 * 56;
        assert_eq!(
            view.storage.retained_storage().checked_sub(payload),
            Some(std::mem::size_of::<
                ProductionSourceOutputOccurrencesV1<'_, '_>,
            >())
        );
    });
}
