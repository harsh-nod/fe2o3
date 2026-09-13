#[test]
fn replacement_allowance_retains_work_history_and_rejects_invalid_owners() {
    let phase = ProductionAnalysisResourcePhaseV1::PassPreservation;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 4, 7, 3).unwrap();
    let contract = ProductionAnalysisResourceContractV1 {
        limits: ProductionAnalysisResourceLimitsV1::new(24, 18),
        cumulative: bound,
    };
    assert_eq!(
        contract.remaining(phase).unwrap(),
        ProductionAnalysisResourceLimitsV1::new(20, 11),
    );
    assert_eq!(
        contract.remaining_for_replacement(phase, 5).unwrap(),
        ProductionAnalysisResourceLimitsV1::new(20, 16),
    );
    assert_eq!(contract.cumulative(), bound);
    assert_eq!(
        contract.remaining_for_replacement(phase, 0),
        contract.remaining(phase)
    );
    assert_eq!(
        contract.remaining_for_replacement(phase, 8),
        Err(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "replaced retained storage upper bound",
        }),
    );
    // Even removal of every retained cell cannot erase a previous peak or work.
    for (work, peak, resource) in [
        (3, 18, "work upper bound"),
        (24, 9, "peak storage upper bound"),
    ] {
        let invalid = ProductionAnalysisResourceContractV1 {
            limits: ProductionAnalysisResourceLimitsV1::new(work, peak),
            cumulative: bound,
        };
        assert_eq!(
            invalid.remaining_for_replacement(phase, 7),
            Err(ProductionAnalysisResourceLimitV1 { phase, resource }),
        );
        assert_eq!(invalid.cumulative(), bound);
    }
}
