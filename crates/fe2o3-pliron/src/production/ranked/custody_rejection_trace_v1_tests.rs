#[derive(Default)]
struct RejectingWriter {
    writes: usize,
    flushes: usize,
}

impl std::io::Write for RejectingWriter {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        self.writes += 1;
        Err(std::io::Error::other("injected writer failure"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushes += 1;
        Err(std::io::Error::other("injected flush failure"))
    }
}

#[test]
fn ranked_custody_trace_all_rejection_tags_have_fixed_records() {
    for (branch, tag) in [
        (
            RankedCustodyRejectionV1::VerifyRecipeIdentity,
            "verify_recipe_identity",
        ),
        (
            RankedCustodyRejectionV1::VerifyTypedRoots,
            "verify_typed_roots",
        ),
        (
            RankedCustodyRejectionV1::PrepareAnalysisBinding,
            "prepare_analysis_binding",
        ),
        (
            RankedCustodyRejectionV1::PrepareReplayError,
            "prepare_replay_error",
        ),
        (
            RankedCustodyRejectionV1::PrepareResourceReceipt,
            "prepare_resource_receipt",
        ),
        (RankedCustodyRejectionV1::PrepareReport, "prepare_report"),
        (
            RankedCustodyRejectionV1::PrepareRecipeIdentity,
            "prepare_recipe_identity",
        ),
        (
            RankedCustodyRejectionV1::PrepareRecordedReceipt,
            "prepare_recorded_receipt",
        ),
    ] {
        let mut output = Vec::new();
        write_ranked_custody_rejection_v1(true, &mut output, branch, None);
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("RANKED_CUSTODY_REJECT_V1 branch={tag} inner=none detail=None resource=none\n",)
        );
    }
}

#[test]
fn ranked_custody_trace_disabled_never_touches_writer_or_payload() {
    let error = ProductionSessionErrorV1::DuplicateConstructionName("secret".repeat(8192));
    let mut writer = RejectingWriter::default();
    write_ranked_custody_rejection_v1(
        false,
        &mut writer,
        RankedCustodyRejectionV1::PrepareReplayError,
        Some(&error),
    );
    assert_eq!((writer.writes, writer.flushes), (0, 0));
    assert_eq!(
        ranked_custody_error_tag_v1(Some(&error)),
        "duplicate_construction_name"
    );
}

#[test]
fn ranked_custody_trace_inner_resource_and_variant_tags_exclude_payloads() {
    let errors = [
        ProductionSessionErrorV1::DuplicateConstructionName("secret".repeat(8192)),
        ProductionSessionErrorV1::RankedRecipe(ProductionRankedKernelErrorV1::Materialization(
            "secret semantic payload",
        )),
        ProductionSessionErrorV1::RankedPassPreservation(
            crate::PlironPassPreservationErrorV1::ResourceLimit {
                resource: "secret resource",
            },
        ),
        ProductionSessionErrorV1::RankedReportValidation(
            crate::ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                producing_pass: None,
                resource: "secret report resource",
            },
        ),
        ProductionSessionErrorV1::Operation(super::super::OperationHandleError::UpstreamPanicked),
    ];
    for (error, expected) in errors.into_iter().zip([
        "inner=duplicate_construction_name detail=None",
        "inner=ranked_recipe detail=Recipe(Discriminant(",
        "inner=ranked_pass_preservation detail=PreservationResourceLimit",
        "inner=ranked_report_validation detail=ReportResourceLimit",
        "inner=operation detail=Operation(Discriminant(",
    ]) {
        let mut output = Vec::new();
        write_ranked_custody_rejection_v1(
            true,
            &mut output,
            RankedCustodyRejectionV1::PrepareReplayError,
            Some(&error),
        );
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(expected), "{output}");
        assert!(!output.contains("secret"));
        assert_eq!(output.lines().count(), 1);
        assert!(output.len() <= 256);
    }
}

#[test]
fn ranked_custody_trace_writer_failure_preserves_existing_replay_classification() {
    let error = ProductionSessionErrorV1::RankedPassPreservation(
        crate::PlironPassPreservationErrorV1::ResourceLimit {
            resource: "peak storage upper bound",
        },
    );
    let mut writer = RejectingWriter::default();
    write_ranked_custody_rejection_v1(
        true,
        &mut writer,
        RankedCustodyRejectionV1::PrepareReplayError,
        Some(&error),
    );
    assert_eq!((writer.writes, writer.flushes), (1, 0));
    assert_eq!(
        error,
        ProductionSessionErrorV1::RankedPassPreservation(
            crate::PlironPassPreservationErrorV1::ResourceLimit {
                resource: "peak storage upper bound"
            },
        )
    );
    // Writer failure does not alter the separately corrected resource classification.
    assert_eq!(
        classify_replay_failure_v1(error),
        ProductionSessionErrorV1::AnalysisResourceLimit {
            phase: crate::production_analysis::ProductionAnalysisResourcePhaseV1::PassPreservation,
            producing_pass: None,
            resource: "peak storage upper bound",
        }
    );
}

#[test]
fn ranked_custody_trace_resource_tags_are_closed_and_preserve_errors() {
    for (resource, tag) in [
        ("work upper bound", "work"),
        ("retained storage upper bound", "retained_storage"),
        ("peak storage upper bound", "peak_storage"),
        ("secret unrecognized resource", "unknown_resource"),
    ] {
        for error in [
            ProductionSessionErrorV1::RankedPassPreservation(
                crate::PlironPassPreservationErrorV1::ResourceLimit { resource },
            ),
            ProductionSessionErrorV1::RankedReportValidation(
                crate::ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                    producing_pass: None,
                    resource,
                },
            ),
        ] {
            let expected = error.clone();
            let mut output = Vec::new();
            write_ranked_custody_rejection_v1(
                true,
                &mut output,
                RankedCustodyRejectionV1::PrepareReplayError,
                Some(&error),
            );
            let output = String::from_utf8(output).unwrap();
            assert!(output.ends_with(&format!(" resource={tag}\n")), "{output}");
            assert!(!output.contains(resource));
            assert!(output.len() <= 256);
            assert_eq!(error, expected);
        }
    }
}
