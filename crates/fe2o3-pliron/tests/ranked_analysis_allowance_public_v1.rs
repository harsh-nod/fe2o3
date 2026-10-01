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
