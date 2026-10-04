fn compose_hierarchical_ownership_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    ownership_local: ProductionAnalysisResourceUpperBoundV1,
    bounds: ProductionAnalysisResourceUpperBoundV1,
    race: ProductionAnalysisResourceUpperBoundV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    if census.ownership_contracts == 0 {
        // Contract collection returns clean before either nested analysis for
        // an authenticated empty inventory. Keep the local scan reservation.
        Ok(ownership_local)
    } else {
        ownership_local.checked_with_nested_sequence_discard(
            &[bounds, race],
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        )
    }
}

include!("ownership_resource_tests.rs");
