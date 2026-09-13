use super::*;
use crate::production_analysis::pliron_hierarchical_ownership::preflight_hierarchical_ownership_resource_upper_bound_v1;
use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitsV1;

fn census(bytes: usize) -> ProductionAnalysisInputCensusV1 {
    ProductionAnalysisInputCensusV1 {
        operations: 25,
        canonical_bytes: bytes,
        ownership_contracts: 1,
        ..Default::default()
    }
}

#[test]
fn conditional_prefix_reserves_fixed_capacity_and_one_identity_copy() {
    let base = resource_upper_bound_v1(census(0)).unwrap();
    assert_eq!(base.work_upper_bound(), ADAPTER_WORK + RECORD_BYTES);
    assert_eq!(base.retained_storage_upper_bound(), RECORD_BYTES);
    assert_eq!(
        base.peak_storage_upper_bound(),
        RECORD_BYTES + TEMPORARY_BYTES
    );
    for bytes in [1, 4096, crate::MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1] {
        let bound = resource_upper_bound_v1(census(bytes)).unwrap();
        assert_eq!(bound.work_upper_bound(), base.work_upper_bound() + bytes);
        assert_eq!(bound.retained_storage_upper_bound(), RECORD_BYTES + bytes);
        assert_eq!(
            bound.peak_storage_upper_bound(),
            base.peak_storage_upper_bound() + bytes
        );
        let mut larger = census(bytes);
        larger.operations = MAX_CONDITIONAL_PREFIX_OPERATIONS_V1 + 1;
        assert_eq!(resource_upper_bound_v1(larger).unwrap(), bound);
    }
    assert_eq!(
        resource_upper_bound_v1(census(usize::MAX)),
        Err(ProductionAnalysisResourceLimitV1 {
            phase: Phase::HierarchicalOwnership,
            resource: "conditional ownership resource upper bound",
        })
    );
}

#[test]
fn conditional_prefix_no_trace_still_reserves_the_entire_adapter() {
    let input = census(4096);
    let adapter = resource_upper_bound_v1(input).unwrap();
    let bound = preflight_hierarchical_ownership_resource_upper_bound_v1(
        input,
        None,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    assert!(bound.work_upper_bound() > adapter.work_upper_bound());
    assert!(bound.retained_storage_upper_bound() > adapter.retained_storage_upper_bound());
    assert!(bound.peak_storage_upper_bound() > adapter.peak_storage_upper_bound());
    for (work, peak, expected) in [
        (
            bound.work_upper_bound(),
            bound.peak_storage_upper_bound(),
            None,
        ),
        (
            bound.work_upper_bound() - 1,
            bound.peak_storage_upper_bound(),
            Some("work upper bound"),
        ),
        (
            bound.work_upper_bound(),
            bound.peak_storage_upper_bound() - 1,
            Some("peak storage upper bound"),
        ),
    ] {
        let result = preflight_hierarchical_ownership_resource_upper_bound_v1(
            input,
            None,
            ProductionAnalysisResourceLimitsV1::new(work, peak),
        );
        match expected {
            None => assert_eq!(result, Ok(bound)),
            Some(resource) => assert_eq!(
                result,
                Err(ProductionAnalysisResourceLimitV1 {
                    phase: Phase::HierarchicalOwnership,
                    resource,
                })
            ),
        }
    }
}

#[test]
fn conditional_prefix_report_clone_composes_with_the_live_producing_report() {
    let bound = resource_upper_bound_v1(census(4096)).unwrap();
    let clone = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        Phase::ReportValidation,
        bound.retained_storage_upper_bound(),
        bound.retained_storage_upper_bound(),
        0,
    )
    .unwrap();
    let combined = bound
        .checked_then_retain(clone, Phase::ReportValidation)
        .unwrap();
    assert_eq!(
        combined.work_upper_bound(),
        bound.work_upper_bound() + clone.work_upper_bound()
    );
    assert_eq!(
        combined.retained_storage_upper_bound(),
        2 * bound.retained_storage_upper_bound()
    );
    assert_eq!(
        combined.peak_storage_upper_bound(),
        bound
            .peak_storage_upper_bound()
            .max(2 * bound.retained_storage_upper_bound())
    );
    for (work, peak) in [
        (
            combined.work_upper_bound() - 1,
            combined.peak_storage_upper_bound(),
        ),
        (
            combined.work_upper_bound(),
            combined.peak_storage_upper_bound() - 1,
        ),
    ] {
        assert!(
            ProductionAnalysisResourceLimitsV1::new(work, peak)
                .require(Phase::ReportValidation, combined)
                .is_err()
        );
    }
}
