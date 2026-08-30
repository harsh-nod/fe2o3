use fe2o3_debug_protocol::{
    BaselineComparatorAvailabilityV1, BudgetPolicyStatusV1, CallerBoundBaselineComparatorRecordV1,
    CallerBoundComponentIdentityV1, CapabilityEvidenceV1, CaptureModeV1,
    ComparisonCapabilityUnavailableReasonV1, ComponentInstallationV1, DurationStatisticV1,
    MAX_QUALIFICATION_MANIFEST_BYTES_V1, MeasuredOverheadMetricV1, MeasuredOverheadV1,
    OpaqueIdentityV1, OverheadAssessmentV1, OverheadComparisonAxesV1, OverheadObservationV1,
    QualificationComponentV1, QualificationDecodeErrorV1, QualificationManifestV1,
    QualificationValidationErrorV1, VersionEvidenceV1, VersionUnavailableReasonV1,
    decode_qualification_manifest_v1,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/mi300x-qualification-v1.json");

fn fixture() -> fe2o3_debug_protocol::QualificationManifestV1 {
    decode_qualification_manifest_v1(FIXTURE).expect("checked-in qualification fixture")
}

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).expect("nonzero test identity")
}

fn install_baseline_comparator(
    manifest: &mut QualificationManifestV1,
) -> (OpaqueIdentityV1, CallerBoundBaselineComparatorRecordV1) {
    let environment_identity = manifest.environment.identity().unwrap();
    let collector_content_sha256 = manifest.components[0]
        .installation
        .identity()
        .unwrap()
        .content_sha256;
    let record = CallerBoundBaselineComparatorRecordV1 {
        workload_identity: identity(5),
        input_identity: identity(6),
        artifact_identity: identity(7),
        environment_identity,
        device_identity: identity(8),
        collector_content_sha256,
        raw_evidence_id: identity(3),
        no_capture_evidence_id: identity(4),
        warmups: 5,
        repetitions: 30,
        statistic: DurationStatisticV1::Median,
        clock_domain: "monotonic_raw".to_owned(),
        raw_duration_nanoseconds: 1_000_000_000,
        no_capture_duration_nanoseconds: 1_005_000_000,
    };
    manifest.baseline_comparator.availability =
        BaselineComparatorAvailabilityV1::CallerBoundAvailable {
            record: Box::new(record.clone()),
        };
    (manifest.baseline_comparator.identity().unwrap(), record)
}

#[test]
fn mi300x_fixture_is_complete_bounded_and_non_authoritative() {
    let manifest = fixture();
    assert_eq!(manifest.components.len(), 7);
    assert_eq!(manifest.overhead_budgets.len(), 6);
    assert!(!manifest.grants_observation_authority());
    assert_eq!(
        manifest
            .component(QualificationComponentV1::RocprofComputeViewerAtt)
            .expect("compute viewer row")
            .installation,
        ComponentInstallationV1::CallerBoundObservedUnusable {
            identity: CallerBoundComponentIdentityV1 {
                kind: fe2o3_debug_protocol::ComponentArtifactKindV1::Executable,
                version: VersionEvidenceV1::Unavailable {
                    reason: VersionUnavailableReasonV1::ProbeFailed,
                },
                content_sha256: serde_json::from_str(
                    "\"c8b6f2bd389b4e031cc8b700c3310e3de2799bd1a07da6cf2d58208cf46a9ba6\"",
                )
                .unwrap(),
                configuration_sha256: serde_json::from_str(
                    "\"65b1ce88c31225c921edf2d983f617ddaeaee7e3bfd60c30dd61a23b4c5e97c8\"",
                )
                .unwrap(),
            },
            reason: fe2o3_debug_protocol::InstallationUnavailableReasonV1::DependencyUnavailable,
            evidence_id: serde_json::from_str(
                "\"33fd0d0c9df203771c2c8a3ec4f0dc685cd088ad37b2862fd0d527a78a929c72\"",
            )
            .unwrap(),
        }
    );
    for mode in [
        CaptureModeV1::NoCapture,
        CaptureModeV1::Counters,
        CaptureModeV1::PcSampling,
        CaptureModeV1::Att,
        CaptureModeV1::DebuggerStop,
        CaptureModeV1::Instrumented,
    ] {
        assert_eq!(
            manifest.evaluate_overhead(mode).unwrap(),
            OverheadAssessmentV1::CandidatePolicy
        );
    }
    assert!(
        !manifest.components[0]
            .capabilities
            .live_gpu_state
            .grants_observation_authority()
    );
    assert!(
        !manifest
            .evaluate_overhead(CaptureModeV1::NoCapture)
            .unwrap()
            .grants_qualification_authority()
    );
    assert_eq!(manifest.identity().unwrap(), manifest.identity().unwrap());

    fn reject_authority_keys(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    assert!(!matches!(
                        key.as_str(),
                        "path"
                            | "argv"
                            | "pid"
                            | "address"
                            | "descriptor"
                            | "queue_token"
                            | "execute"
                            | "attach"
                            | "pause"
                            | "launch"
                    ));
                    reject_authority_keys(value);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    reject_authority_keys(item);
                }
            }
            _ => {}
        }
    }
    reject_authority_keys(&serde_json::from_slice(FIXTURE).unwrap());
}

#[test]
fn strict_decoder_rejects_unknown_duplicate_trailing_and_oversized_input() {
    let text = std::str::from_utf8(FIXTURE).unwrap();
    let unknown = text.replacen(
        "\"qualification_date_utc\"",
        "\"unknown\":0,\"qualification_date_utc\"",
        1,
    );
    assert_eq!(
        decode_qualification_manifest_v1(unknown.as_bytes()),
        Err(QualificationDecodeErrorV1::MalformedJson)
    );

    let duplicate = text.replacen(
        "\"qualification_date_utc\"",
        "\"schema\":\"fe2o3-debug-qualification-manifest-v1\",\"qualification_date_utc\"",
        1,
    );
    assert_eq!(
        decode_qualification_manifest_v1(duplicate.as_bytes()),
        Err(QualificationDecodeErrorV1::MalformedJson)
    );

    let mut trailing = FIXTURE.to_vec();
    trailing.extend_from_slice(b"{}\n");
    assert_eq!(
        decode_qualification_manifest_v1(&trailing),
        Err(QualificationDecodeErrorV1::MalformedJson)
    );
    assert_eq!(
        decode_qualification_manifest_v1(&vec![b' '; MAX_QUALIFICATION_MANIFEST_BYTES_V1 + 1]),
        Err(QualificationDecodeErrorV1::ManifestTooLarge)
    );
}

#[test]
fn matrices_are_exhaustive_and_canonically_ordered() {
    let mut manifest = fixture();
    manifest.components.swap(0, 1);
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::IncompleteComponentMatrix)
    );

    let mut manifest = fixture();
    manifest.overhead_budgets.pop();
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::IncompleteOverheadMatrix)
    );

    let mut manifest = fixture();
    manifest.overhead_budgets[3].collector = QualificationComponentV1::Rocgdb;
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::InvalidBudgetCollector)
    );

    let mut manifest = fixture();
    manifest.overhead_budgets[1].configuration_sha256 =
        manifest.overhead_budgets[0].configuration_sha256;
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::InvalidBudgetConfiguration)
    );
}

#[test]
fn manifest_level_evaluation_revalidates_all_post_decode_mutations() {
    let mut manifest = fixture();
    manifest.overhead_budgets[5].policy.min_repetitions = 0;
    assert_eq!(
        manifest.evaluate_overhead(CaptureModeV1::NoCapture),
        Err(QualificationValidationErrorV1::BudgetOutOfRange(
            "repetitions"
        ))
    );

    let mut manifest = fixture();
    manifest.components.swap(0, 1);
    assert_eq!(
        manifest.evaluate_overhead(CaptureModeV1::NoCapture),
        Err(QualificationValidationErrorV1::IncompleteComponentMatrix)
    );
}

#[test]
fn dates_are_gregorian_and_documentation_urls_have_real_https_authorities() {
    let mut manifest = fixture();
    manifest.qualification_date_utc = "2024-02-29".to_owned();
    manifest.validate().unwrap();
    manifest.qualification_date_utc = "2000-02-29".to_owned();
    manifest.validate().unwrap();
    for invalid in ["0000-01-01", "2023-02-29", "2100-02-29", "2026-04-31"] {
        manifest.qualification_date_utc = invalid.to_owned();
        assert_eq!(
            manifest.validate(),
            Err(QualificationValidationErrorV1::InvalidQualificationDate)
        );
    }

    let mut manifest = fixture();
    let CapabilityEvidenceV1::Documented { reference, .. } =
        &mut manifest.components[2].capabilities.live_gpu_state
    else {
        unreachable!()
    };
    reference.url = "https://example.com:443/reference?version=1#section".to_owned();
    manifest.validate().unwrap();
    for invalid in [
        "https:///reference",
        "https://?query",
        "https://user@example.com/reference",
        "https://bad_host/reference",
        "https://example.com:0/reference",
        "https://example.com:not-a-port/reference",
        "http://example.com/reference",
        "https://example.com/ref erence",
    ] {
        let CapabilityEvidenceV1::Documented { reference, .. } =
            &mut manifest.components[2].capabilities.live_gpu_state
        else {
            unreachable!()
        };
        reference.url = invalid.to_owned();
        assert_eq!(
            manifest.validate(),
            Err(QualificationValidationErrorV1::InvalidDocumentationUrl)
        );
    }

    let mut manifest = fixture();
    let CapabilityEvidenceV1::CallerBoundObserved { limitations, .. } =
        &mut manifest.components[0].capabilities.live_gpu_state
    else {
        unreachable!()
    };
    *limitations = "Caller text may mention /tmp/tool, PID 7, or argv without creating a typed authority field.".to_owned();
    manifest.validate().unwrap();
}

#[test]
fn caller_bound_observation_cannot_be_attached_to_an_unavailable_tool() {
    let mut manifest = fixture();
    let mojo = manifest
        .components
        .iter_mut()
        .find(|record| record.component == QualificationComponentV1::MojoGpuWorkflow)
        .unwrap();
    mojo.capabilities.live_gpu_state = CapabilityEvidenceV1::CallerBoundObserved {
        evidence_id: identity(1),
        limitations: "caller claim".to_owned(),
    };
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::ObservedCapabilityWithoutUsableTool)
    );
}

#[test]
fn usable_component_requires_an_exact_version() {
    let mut manifest = fixture();
    let rocgdb = manifest
        .components
        .iter_mut()
        .find(|record| record.component == QualificationComponentV1::Rocgdb)
        .unwrap();
    let Some(component) = rocgdb.installation.identity().cloned() else {
        panic!("rocgdb identity")
    };
    rocgdb.installation = ComponentInstallationV1::CallerBoundObservedUsable {
        identity: CallerBoundComponentIdentityV1 {
            version: VersionEvidenceV1::Unavailable {
                reason: VersionUnavailableReasonV1::ProbeFailed,
            },
            ..component
        },
        evidence_id: identity(2),
    };
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::UsableVersionUnavailable)
    );
}

#[test]
fn approved_policy_satisfaction_is_rederived_over_exact_comparison_axes() {
    let mut manifest = fixture();
    let (baseline_comparator_identity, _) = install_baseline_comparator(&mut manifest);
    let environment_identity = manifest.environment.identity().unwrap();
    let collector_content_sha256 = manifest.components[0]
        .installation
        .identity()
        .unwrap()
        .content_sha256;
    let raw_configuration_sha256 = manifest.baseline_comparator.raw_configuration_sha256;
    let record = &mut manifest.overhead_budgets[0];
    assert_eq!(record.mode, CaptureModeV1::NoCapture);
    record.policy.status = BudgetPolicyStatusV1::Approved;
    let captured_configuration_sha256 = record.configuration_sha256;
    record.observation = OverheadObservationV1::Measured {
        measurement: Box::new(MeasuredOverheadV1 {
            configuration_sha256: captured_configuration_sha256,
            baseline_comparator_identity,
            baseline_evidence_id: identity(3),
            captured_evidence_id: identity(4),
            comparison: OverheadComparisonAxesV1 {
                workload_identity: identity(5),
                input_identity: identity(6),
                artifact_identity: identity(7),
                environment_identity,
                device_identity: identity(8),
                collector_content_sha256,
                baseline_configuration_sha256: raw_configuration_sha256,
                captured_configuration_sha256,
            },
            warmups: 5,
            repetitions: 30,
            statistic: DurationStatisticV1::Median,
            clock_domain: "monotonic_raw".to_owned(),
            metric: MeasuredOverheadMetricV1::RelativeDuration {
                baseline_nanoseconds: 1_000_000_000,
                captured_nanoseconds: 1_005_000_000,
            },
            storage_bytes: 0,
            collection_milliseconds: 10_000,
            loss_free: true,
            truncated: false,
        }),
    };
    manifest.validate().unwrap();
    assert_eq!(
        manifest
            .evaluate_overhead(CaptureModeV1::NoCapture)
            .unwrap(),
        OverheadAssessmentV1::CallerBoundPolicySatisfied
    );

    let mut failed = manifest.clone();
    if let OverheadObservationV1::Measured { measurement } =
        &mut failed.overhead_budgets[0].observation
    {
        measurement.loss_free = false;
    } else {
        unreachable!()
    }
    assert_eq!(
        failed.evaluate_overhead(CaptureModeV1::NoCapture).unwrap(),
        OverheadAssessmentV1::Failed
    );
    if let OverheadObservationV1::Measured { measurement } =
        &mut failed.overhead_budgets[0].observation
    {
        measurement.loss_free = true;
        measurement.metric = MeasuredOverheadMetricV1::RelativeDuration {
            baseline_nanoseconds: 0,
            captured_nanoseconds: 1,
        };
    } else {
        unreachable!()
    }
    assert_eq!(
        failed.evaluate_overhead(CaptureModeV1::NoCapture),
        Err(QualificationValidationErrorV1::InvalidMeasurementMetric)
    );

    if let OverheadObservationV1::Measured { measurement } =
        &mut manifest.overhead_budgets[0].observation
    {
        measurement.comparison.artifact_identity = identity(10);
    } else {
        unreachable!()
    }
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::MeasurementBaselineMismatch)
    );
    if let OverheadObservationV1::Measured { measurement } =
        &mut manifest.overhead_budgets[0].observation
    {
        measurement.comparison.artifact_identity = identity(7);
        measurement.comparison.baseline_configuration_sha256 = identity(9);
    } else {
        unreachable!()
    }
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::MeasurementBaselineMismatch)
    );
    if let OverheadObservationV1::Measured { measurement } =
        &mut manifest.overhead_budgets[0].observation
    {
        measurement.comparison.baseline_configuration_sha256 =
            manifest.baseline_comparator.raw_configuration_sha256;
        measurement.comparison.environment_identity = identity(11);
    } else {
        unreachable!()
    }
    assert_eq!(
        manifest.validate(),
        Err(QualificationValidationErrorV1::MeasurementBaselineMismatch)
    );
}

#[test]
fn non_no_capture_measurement_must_bind_the_canonical_no_capture_comparator() {
    let mut manifest = fixture();
    let (baseline_comparator_identity, baseline) = install_baseline_comparator(&mut manifest);
    let rocprof_content_sha256 = manifest.components[3]
        .installation
        .identity()
        .unwrap()
        .content_sha256;
    let no_capture_configuration_sha256 =
        manifest.baseline_comparator.no_capture_configuration_sha256;
    let record = &mut manifest.overhead_budgets[1];
    record.policy.status = BudgetPolicyStatusV1::Approved;
    let captured_configuration_sha256 = record.configuration_sha256;
    record.observation = OverheadObservationV1::Measured {
        measurement: Box::new(MeasuredOverheadV1 {
            configuration_sha256: captured_configuration_sha256,
            baseline_comparator_identity,
            baseline_evidence_id: baseline.no_capture_evidence_id,
            captured_evidence_id: identity(12),
            comparison: OverheadComparisonAxesV1 {
                workload_identity: baseline.workload_identity,
                input_identity: baseline.input_identity,
                artifact_identity: baseline.artifact_identity,
                environment_identity: baseline.environment_identity,
                device_identity: baseline.device_identity,
                collector_content_sha256: rocprof_content_sha256,
                baseline_configuration_sha256: no_capture_configuration_sha256,
                captured_configuration_sha256,
            },
            warmups: 5,
            repetitions: 30,
            statistic: DurationStatisticV1::Median,
            clock_domain: "monotonic_raw".to_owned(),
            metric: MeasuredOverheadMetricV1::RelativeDuration {
                baseline_nanoseconds: baseline.no_capture_duration_nanoseconds,
                captured_nanoseconds: 1_050_000_000,
            },
            storage_bytes: 1024,
            collection_milliseconds: 10_000,
            loss_free: true,
            truncated: false,
        }),
    };
    assert_eq!(
        manifest.evaluate_overhead(CaptureModeV1::Counters).unwrap(),
        OverheadAssessmentV1::CallerBoundPolicySatisfied
    );

    if let OverheadObservationV1::Measured { measurement } =
        &mut manifest.overhead_budgets[1].observation
    {
        measurement.baseline_evidence_id = baseline.raw_evidence_id;
    } else {
        unreachable!()
    }
    assert_eq!(
        manifest.evaluate_overhead(CaptureModeV1::Counters),
        Err(QualificationValidationErrorV1::MeasurementBaselineMismatch)
    );
    if let OverheadObservationV1::Measured { measurement } =
        &mut manifest.overhead_budgets[1].observation
    {
        measurement.baseline_evidence_id = baseline.no_capture_evidence_id;
        measurement.baseline_comparator_identity = identity(13);
    } else {
        unreachable!()
    }
    assert_eq!(
        manifest.evaluate_overhead(CaptureModeV1::Counters),
        Err(QualificationValidationErrorV1::MeasurementBaselineMismatch)
    );
}

#[test]
fn policy_failure_and_unavailable_measurement_remain_typed() {
    let mut manifest = fixture();
    manifest.overhead_budgets[1].policy.status = BudgetPolicyStatusV1::Approved;
    assert_eq!(
        manifest.evaluate_overhead(CaptureModeV1::Counters).unwrap(),
        OverheadAssessmentV1::Unavailable
    );

    let capability = &manifest.components[0].capabilities.source_break_and_step;
    assert!(matches!(
        capability,
        CapabilityEvidenceV1::Unavailable {
            reason: ComparisonCapabilityUnavailableReasonV1::NotImplemented,
            ..
        }
    ));
}
