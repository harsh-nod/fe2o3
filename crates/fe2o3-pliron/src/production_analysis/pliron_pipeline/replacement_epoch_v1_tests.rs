#[test]
fn replacement_changed_epoch_keeps_original_input_capture_allowance() {
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisReplacementLimitsV1;
    use std::cell::Cell;
    let context = &mut setup();
    let (function, ret) = valid_tensor_function(context, "replacement_changed_epoch");
    let mut preservation = begin_production_pliron_pass_contract_session_v1(
        LivePlironStructuralIdentityProviderV1::new(context, &function),
    )
    .unwrap();
    let original_identity = preservation.lineage_identity_resource_upper_bound_v1();
    assert!(original_identity.retained_storage_upper_bound() > 0);
    // A real pinned-PLIRON mutable borrow advances the attempt epoch even
    // though no payload is changed. begin_pass must recapture before callback.
    drop(ret.get_operation().deref_mut(context));
    let called = Cell::new(false);
    let result = preservation.run_contiguous_pass_with_resource_limits_v1(
        KernelCheckPassKindV1::TensorLayout,
        ProductionAnalysisReplacementLimitsV1 {
            input: ProductionAnalysisResourceLimitsV1::new(usize::MAX, 0),
            output: ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        },
        || {
            called.set(true);
            Ok::<_, ()>(())
        },
    );
    // Using the output allowance would instead reach StaleMutationEpoch;
    // the original zero input storage must reject the capture first.
    assert_eq!(
        result,
        Err(PlironPassPreservationErrorV1::ResourceLimit {
            resource: "peak storage upper bound",
        })
    );
    assert!(!called.get());
    assert!(
        preservation
            .last_checkpoint_resource_upper_bound_v1()
            .is_none()
    );
    assert_eq!(
        preservation.lineage_identity_resource_upper_bound_v1(),
        original_identity
    );
    // Resource denial did not mutate the graph; a genuinely fresh owner still
    // runs all nine exact-preservation comparisons on this unchanged function.
    let report = require_production_pliron_checks_before_lowering_v2(context, &function).unwrap();
    assert!(report.is_clean());
    assert_eq!(report.preservation().certificates().len(), 9);
}
