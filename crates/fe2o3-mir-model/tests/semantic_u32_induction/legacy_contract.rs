//! Standalone baseline/composite transcript: this leaf uses only unchanged APIs.
use super::*;
use fe2o3_mir_model::{
    InertCanonicalSemanticU32InductionEvidenceV1,
    analyze_semantic_u32_induction_no_overflow_reachable_with_limits_v2,
};
use std::fmt::Write;

#[test]
fn exact_legacy_report_evidence_and_charge_transcript() {
    for mutation in [
        Mutation::None,
        Mutation::AliasGuardInduction,
        Mutation::AliasGuardBound,
        Mutation::StaleAliasGuardInduction,
        Mutation::LessOrEqualGuard,
        Mutation::StepTwo,
        Mutation::ResetDefinition,
        Mutation::AlternateBodyEntry,
        Mutation::UnrelatedExitNop,
    ] {
        let source = admitted(1, mutation);
        let first = analyze_semantic_u32_induction_no_overflow_v1(&source, FUNCTION).unwrap();
        let second = analyze_semantic_u32_induction_no_overflow_reachable_with_limits_v2(
            &source,
            FUNCTION,
            SemanticU32InductionAnalysisLimitsV1::default(),
        )
        .unwrap();
        for (version, report) in [(1, &first), (2, &second)] {
            let evidence =
                InertCanonicalSemanticU32InductionEvidenceV1::from_report(report).unwrap();
            assert_eq!(evidence.work_units(), report.work_units() as u64);
            let mut hex = String::new();
            for byte in evidence.canonical_bytes() {
                write!(&mut hex, "{byte:02x}").unwrap();
            }
            println!(
                "LEGACY-U32 {mutation:?} V{version} work={} {hex}",
                report.work_units()
            );
        }
    }
}
