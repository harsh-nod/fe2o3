fn integer_continuation_resources_v1(
    canonical_bytes: usize,
    registered_node_bound: usize,
) -> Result<
    (
        PlironOptimizationResourcesV12,
        crate::kir_optimization_map_v12::CaptureLimitsV12,
    ),
    PlironOptimizationErrorV12,
> {
    // Reuse the admitted opaque scalar envelope, including its SCCP allowance
    // for one synthesized constant per original value. The integer pass creates
    // at most one false constant per original checked binary (two result values).
    // Retained arena/interner payload stays live even if DCE erases the constant.
    // This is versioned logical accounting, not allocator/RSS measurement.
    let (mut profile, capture) =
        policy3_execution_resources_v1(canonical_bytes, registered_node_bound)?;
    profile.report = size_of::<PlironOptimizationReportV1>()
        .checked_add(2 * size_of::<PlironOptimizationPassReportV1>())
        .ok_or(PlironOptimizationErrorV12::Accounting)?;
    Ok((profile, capture))
}

impl KirPlironGraphV12<'_> {
    pub(crate) fn execute_native_integer_continuation_v1(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        occurrences: &crate::kir_occurrence_capture_v1::Capture,
        limits: crate::kir_occurrence_capture_v1::Limits,
    ) -> Result<
        (
            PlironOptimizationReportV1,
            PlironOptimizationResourcesV12,
            usize,
        ),
        PlironOptimizationErrorV12,
    > {
        self.execute_admitted_production_optimization_v1(
            budget,
            ExecutionAdmissionV1::Integer6 {
                occurrences,
                limits,
            },
        )
    }
}
