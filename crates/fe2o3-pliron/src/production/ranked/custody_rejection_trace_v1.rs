#[derive(Clone, Copy)]
enum RankedCustodyRejectionV1 {
    VerifyRecipeIdentity,
    VerifyTypedRoots,
    PrepareAnalysisBinding,
    PrepareReplayError,
    PrepareResourceReceipt,
    PrepareReport,
    PrepareRecipeIdentity,
    PrepareRecordedReceipt,
}

impl RankedCustodyRejectionV1 {
    fn tag(self) -> &'static str {
        match self {
            Self::VerifyRecipeIdentity => "verify_recipe_identity",
            Self::VerifyTypedRoots => "verify_typed_roots",
            Self::PrepareAnalysisBinding => "prepare_analysis_binding",
            Self::PrepareReplayError => "prepare_replay_error",
            Self::PrepareResourceReceipt => "prepare_resource_receipt",
            Self::PrepareReport => "prepare_report",
            Self::PrepareRecipeIdentity => "prepare_recipe_identity",
            Self::PrepareRecordedReceipt => "prepare_recorded_receipt",
        }
    }
}

fn ranked_custody_error_tag_v1(error: Option<&ProductionSessionErrorV1>) -> &'static str {
    let Some(error) = error else { return "none" };
    match error {
        ProductionSessionErrorV1::SessionPoisoned => "session_poisoned",
        ProductionSessionErrorV1::ConstructionLimitExceeded => "construction_limit",
        ProductionSessionErrorV1::DuplicateConstructionName(_) => "duplicate_construction_name",
        ProductionSessionErrorV1::StageIdentitySpaceExhausted => "stage_identity_exhausted",
        ProductionSessionErrorV1::RootIdentitySpaceExhausted => "root_identity_exhausted",
        ProductionSessionErrorV1::ForeignSession => "foreign_session",
        ProductionSessionErrorV1::StaleStage => "stale_stage",
        ProductionSessionErrorV1::StageRootMismatch => "stage_root_mismatch",
        ProductionSessionErrorV1::WrongConstructionKind => "wrong_construction_kind",
        ProductionSessionErrorV1::RankedGraphChanged => "ranked_graph_changed",
        ProductionSessionErrorV1::AnalysisResourceLimit { .. } => "analysis_resource_limit",
        ProductionSessionErrorV1::RankedRecipe(_) => "ranked_recipe",
        ProductionSessionErrorV1::RankedTensorLayout(_) => "ranked_tensor_layout",
        ProductionSessionErrorV1::RankedBounds(_) => "ranked_bounds",
        ProductionSessionErrorV1::RankedAtomic(_) => "ranked_atomic",
        ProductionSessionErrorV1::RankedRace(_) => "ranked_race",
        ProductionSessionErrorV1::RankedOwnership(_) => "ranked_ownership",
        ProductionSessionErrorV1::RankedBarrier(_) => "ranked_barrier",
        ProductionSessionErrorV1::RankedPipeline(_) => "ranked_pipeline",
        ProductionSessionErrorV1::RankedWorkgroup(_) => "ranked_workgroup",
        ProductionSessionErrorV1::RankedSemantic(_) => "ranked_semantic",
        ProductionSessionErrorV1::RankedPassPreservation(_) => "ranked_pass_preservation",
        ProductionSessionErrorV1::RankedReportValidation(_) => "ranked_report_validation",
        ProductionSessionErrorV1::Operation(_) => "operation",
        ProductionSessionErrorV1::ConditionalOwnership(_) => "conditional-ownership",
    }
}

enum RankedCustodyErrorDetailV1 {
    None,
    PreservationResourceLimit,
    ReportResourceLimit,
    Recipe(std::mem::Discriminant<ProductionRankedKernelErrorV1>),
    Preservation(std::mem::Discriminant<crate::PlironPassPreservationErrorV1>),
    Report(std::mem::Discriminant<crate::ProductionAnalysisReportValidationErrorV1>),
    Operation(std::mem::Discriminant<super::OperationHandleError>),
}

impl std::fmt::Display for RankedCustodyErrorDetailV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::PreservationResourceLimit => formatter.write_str("PreservationResourceLimit"),
            Self::ReportResourceLimit => formatter.write_str("ReportResourceLimit"),
            Self::Recipe(value) => write!(formatter, "Recipe({value:?})"),
            Self::Preservation(value) => write!(formatter, "Preservation({value:?})"),
            Self::Report(value) => write!(formatter, "Report({value:?})"),
            Self::Operation(value) => write!(formatter, "Operation({value:?})"),
        }
    }
}

fn ranked_custody_error_detail_v1(
    error: Option<&ProductionSessionErrorV1>,
) -> RankedCustodyErrorDetailV1 {
    match error {
        Some(ProductionSessionErrorV1::RankedPassPreservation(
            crate::PlironPassPreservationErrorV1::ResourceLimit { .. },
        )) => RankedCustodyErrorDetailV1::PreservationResourceLimit,
        Some(ProductionSessionErrorV1::RankedReportValidation(
            crate::ProductionAnalysisReportValidationErrorV1::ResourceLimit { .. },
        )) => RankedCustodyErrorDetailV1::ReportResourceLimit,
        Some(ProductionSessionErrorV1::RankedRecipe(error)) => {
            RankedCustodyErrorDetailV1::Recipe(std::mem::discriminant(error))
        }
        Some(ProductionSessionErrorV1::RankedPassPreservation(error)) => {
            RankedCustodyErrorDetailV1::Preservation(std::mem::discriminant(error))
        }
        Some(ProductionSessionErrorV1::RankedReportValidation(error)) => {
            RankedCustodyErrorDetailV1::Report(std::mem::discriminant(error))
        }
        Some(ProductionSessionErrorV1::Operation(error)) => {
            RankedCustodyErrorDetailV1::Operation(std::mem::discriminant(error))
        }
        _ => RankedCustodyErrorDetailV1::None,
    }
}

fn write_ranked_custody_rejection_v1(
    enabled: bool,
    writer: &mut impl std::io::Write,
    branch: RankedCustodyRejectionV1,
    error: Option<&ProductionSessionErrorV1>,
) {
    if !enabled {
        return;
    }
    // Only closed tags and same-build variant discriminants are formatted.
    let _ = writeln!(
        writer,
        "RANKED_CUSTODY_REJECT_V1 branch={} inner={} detail={} resource={}",
        branch.tag(),
        ranked_custody_error_tag_v1(error),
        ranked_custody_error_detail_v1(error),
        ranked_custody_resource_tag_v1(error),
    );
}

fn ranked_custody_resource_tag_v1(error: Option<&ProductionSessionErrorV1>) -> &'static str {
    let resource = match error {
        Some(ProductionSessionErrorV1::RankedPassPreservation(
            crate::PlironPassPreservationErrorV1::ResourceLimit { resource },
        ))
        | Some(ProductionSessionErrorV1::RankedReportValidation(
            crate::ProductionAnalysisReportValidationErrorV1::ResourceLimit { resource, .. },
        )) => *resource,
        _ => return "none",
    };
    match resource {
        "work upper bound" => "work",
        "retained storage upper bound" => "retained_storage",
        "peak storage upper bound" => "peak_storage",
        _ => "unknown_resource",
    }
}

fn trace_ranked_custody_rejection_v1(
    branch: RankedCustodyRejectionV1,
    error: Option<&ProductionSessionErrorV1>,
) {
    if std::env::var_os("FE2O3_TRACE_RANKED_CUSTODY_V1").is_none() {
        return;
    }
    write_ranked_custody_rejection_v1(true, &mut std::io::stderr().lock(), branch, error);
}

#[cfg(test)]
mod ranked_custody_rejection_trace_tests_v1 {
    use super::*;
    include!("custody_rejection_trace_v1_tests.rs");
}
