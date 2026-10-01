use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedAnalysisAllowanceErrorV1,
    ProductionRankedAnalysisAllowanceV1, ProductionRankedCompileErrorV1,
    ProductionRankedKernelLoweringInputV1, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_gfx942_lowering_with_analysis_allowance_v1,
    compile_ranked_kernel_for_lowering_with_analysis_allowance_v1,
};

#[test]
fn caller_allowance_and_closed_compile_signatures_are_public() {
    let allowance = ProductionRankedAnalysisAllowanceV1::new(0, 0).unwrap();
    assert_eq!((allowance.max_work(), allowance.max_peak_storage()), (0, 0));
    let _: fn(
        ProductionConstructionV1,
        ProductionSessionLimitsV1,
        ProductionRankedAnalysisAllowanceV1,
    )
        -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> =
        compile_ranked_kernel_for_lowering_with_analysis_allowance_v1;
    let _: fn(
        ProductionConstructionV1,
        ProductionSessionLimitsV1,
        [u64; 0],
        ProductionRankedAnalysisAllowanceV1,
    )
        -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> =
        compile_ranked_kernel_for_gfx942_lowering_with_analysis_allowance_v1;
    let error = ProductionRankedAnalysisAllowanceErrorV1::WorkAboveHardCeiling;
    let _: &dyn std::error::Error = &error;
}

#[test]
fn combined_snapshot_allowance_endpoint_is_public_and_checked() {
    use fe2o3_pliron::{
        ProductionRankedSnapshotAllowanceErrorV1, ProductionRankedSnapshotAllowanceV1,
        compile_ranked_kernel_for_lowering_with_analysis_and_snapshot_allowances_v1,
    };
    let _: fn(
        ProductionConstructionV1,
        ProductionSessionLimitsV1,
        ProductionRankedAnalysisAllowanceV1,
        ProductionRankedSnapshotAllowanceV1,
    )
        -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> =
        compile_ranked_kernel_for_lowering_with_analysis_and_snapshot_allowances_v1;
    let zero = ProductionRankedSnapshotAllowanceV1::new(0, 0).unwrap();
    assert_eq!((zero.max_bytes(), zero.max_work()), (0, 0));
    let analysis = ProductionRankedAnalysisAllowanceV1::production_hard_ceiling();
    let snapshot = ProductionRankedSnapshotAllowanceV1::production_hard_ceiling();
    assert_eq!(snapshot.max_work(), analysis.max_work());
    let error = ProductionRankedSnapshotAllowanceErrorV1::BytesAboveHardCeiling;
    let _: &dyn std::error::Error = &error;
}

#[test]
fn gfx942_combined_allowance_endpoint_is_named_and_public() {
    type Compile = fn(
        fe2o3_pliron::ProductionConstructionV1,
        fe2o3_pliron::ProductionSessionLimitsV1,
        Vec<u64>,
        fe2o3_pliron::ProductionRankedAnalysisAllowanceV1,
        fe2o3_pliron::ProductionRankedSnapshotAllowanceV1,
    ) -> Result<
        fe2o3_pliron::ProductionRankedKernelLoweringInputV1,
        fe2o3_pliron::ProductionRankedCompileErrorV1,
    >;
    let _: Compile = fe2o3_pliron::compile_ranked_kernel_for_gfx942_lowering_with_analysis_and_snapshot_allowances_v1;
}
