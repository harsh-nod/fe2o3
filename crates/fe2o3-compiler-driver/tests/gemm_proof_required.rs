use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_compiler_api::{
    CandidateFormatIdentityV1, CandidateIdentityV1, CompileDispositionV1, CompileLimitsV1,
    CompileOutputV1, CompileRequestV1, CompilerProfileIdentityV1, CompilerStageV1,
    DiagnosticSubjectIdentityV1, ExecutableCandidateV1, KernelInstanceIdentityV1,
    ObligationSetIdentityV1, PipelineConfigurationIdentityV1, PipelineSelectorV1, ReceiptOutcomeV1,
    RequestIdentityV1, SnapshotFormatIdentityV1, SnapshotIdentityV1, StageReceiptV1,
    StageSnapshotV1, TargetProfileIdentityV1, TransformConfigurationIdentityV1,
    TransformIdentityV1,
};
use fe2o3_compiler_driver::{
    AdmittedGemmCompilerBackendV1, CompilerBackendFailureV1, GEMM_REQUIRED_SAFETY_PROPERTIES_V1,
    GemmExpectedUnsafeObligationV1, GemmObligationFindingV1, GemmObligationOutcomeV1,
    GemmProofDiagnosticV1, GemmProofEvaluationFailureV1, GemmProofRejectionKindV1,
    GemmProofReportProviderV1, GemmProofReportV1, GemmProofRequirementsV1, GemmSafetyPropertyV1,
    MAX_GEMM_OBLIGATION_FINDINGS_V1, MAX_GEMM_UNSAFE_OBLIGATIONS_V1, ProofRequiredGemmAdmissionV1,
    ProofRequiredGemmBackendV1, TransactionalCompilerBackendV1, admit_proof_required_gemm_v1,
};

static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn identity(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn obligations() -> ObligationSetIdentityV1 {
    ObligationSetIdentityV1::from_untrusted_bytes(identity(0x16))
}

fn request() -> CompileRequestV1 {
    request_with(0x11, 0x16)
}

fn request_with(request_identity: u8, obligation_identity: u8) -> CompileRequestV1 {
    CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes(identity(request_identity)),
        KernelInstanceIdentityV1::from_untrusted_bytes(identity(0x12)),
        CompilerProfileIdentityV1::from_untrusted_bytes(identity(0x13)),
        TargetProfileIdentityV1::from_untrusted_bytes(identity(0x14)),
        PipelineConfigurationIdentityV1::from_untrusted_bytes(identity(0x15)),
        ObligationSetIdentityV1::from_untrusted_bytes(identity(obligation_identity)),
        PipelineSelectorV1::PlironV1,
        StageSnapshotV1::new(
            CompilerStageV1::FrontendInput,
            SnapshotIdentityV1::from_untrusted_bytes(identity(0x17)),
            SnapshotFormatIdentityV1::from_untrusted_bytes(identity(0x18)),
            vec![0x19],
        )
        .unwrap(),
        CompileLimitsV1::new(16, 16, 16, 1024, 4096, 1024).unwrap(),
    )
    .unwrap()
}

fn required_findings() -> Vec<GemmObligationFindingV1> {
    GEMM_REQUIRED_SAFETY_PROPERTIES_V1
        .into_iter()
        .map(|property| {
            GemmObligationFindingV1::required(property, GemmObligationOutcomeV1::Discharged, None)
        })
        .collect()
}

fn report(findings: Vec<GemmObligationFindingV1>) -> GemmProofReportV1 {
    GemmProofReportV1::new(obligations(), findings).unwrap()
}

fn report_with_failure(
    property: GemmSafetyPropertyV1,
    outcome: GemmObligationOutcomeV1,
) -> GemmProofReportV1 {
    report(
        GEMM_REQUIRED_SAFETY_PROPERTIES_V1
            .into_iter()
            .map(|candidate| {
                GemmObligationFindingV1::required(
                    candidate,
                    if candidate == property {
                        outcome
                    } else {
                        GemmObligationOutcomeV1::Discharged
                    },
                    None,
                )
            })
            .collect(),
    )
}

#[derive(Clone, Debug)]
enum ProviderAction {
    Report(GemmProofReportV1),
    Fail(GemmProofEvaluationFailureV1),
}

#[derive(Clone, Debug)]
struct FixedProvider(ProviderAction);

impl GemmProofReportProviderV1 for FixedProvider {
    fn evaluate(
        &mut self,
        _request: &CompileRequestV1,
        _requirements: &GemmProofRequirementsV1,
    ) -> Result<GemmProofReportV1, GemmProofEvaluationFailureV1> {
        match &self.0 {
            ProviderAction::Report(report) => Ok(report.clone()),
            ProviderAction::Fail(failure) => Err(*failure),
        }
    }
}

#[derive(Debug)]
struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gemm-proof-required-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn artifact(&self) -> PathBuf {
        self.0.join("device.hsaco")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug)]
struct RecordingEmitter {
    calls: Rc<Cell<usize>>,
    artifact: PathBuf,
}

impl AdmittedGemmCompilerBackendV1 for RecordingEmitter {
    fn compile_admitted(
        &mut self,
        request: &CompileRequestV1,
        admission: ProofRequiredGemmAdmissionV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        assert_eq!(admission.request_identity(), request.identity());
        assert_eq!(
            admission.obligation_set_identity(),
            request.input_obligations_identity()
        );
        self.calls.set(self.calls.get() + 1);
        let bytes = [0x7f, b'E', b'L', b'F'];
        fs::write(&self.artifact, bytes).unwrap();

        let hsaco_identity = SnapshotIdentityV1::from_untrusted_bytes(identity(0x31));
        let hsaco = StageSnapshotV1::new(
            CompilerStageV1::Hsaco,
            hsaco_identity,
            SnapshotFormatIdentityV1::from_untrusted_bytes(identity(0x32)),
            bytes.to_vec(),
        )
        .unwrap();
        let receipt = StageReceiptV1::new(
            0,
            CompilerStageV1::Hsaco,
            TransformIdentityV1::from_untrusted_bytes(identity(0x33)),
            TransformConfigurationIdentityV1::from_untrusted_bytes(identity(0x34)),
            request.input().identity(),
            Some(hsaco_identity),
            request.input_obligations_identity(),
            Some(request.input_obligations_identity()),
            ReceiptOutcomeV1::Produced,
        )
        .unwrap();
        let candidate = ExecutableCandidateV1::new(
            CandidateIdentityV1::from_untrusted_bytes(identity(0x35)),
            CandidateFormatIdentityV1::from_untrusted_bytes(identity(0x36)),
            hsaco_identity,
            bytes.to_vec(),
        )
        .unwrap();

        Ok(CompileOutputV1::new(
            request,
            CompileDispositionV1::CandidateProduced,
            vec![hsaco],
            vec![receipt],
            Vec::new(),
            Some(candidate),
        )
        .unwrap())
    }
}

fn expected_unsafe(
    obligation_id: u32,
    property: GemmSafetyPropertyV1,
    subject: Option<DiagnosticSubjectIdentityV1>,
) -> GemmExpectedUnsafeObligationV1 {
    GemmExpectedUnsafeObligationV1::new(obligation_id, property, subject).unwrap()
}

fn compile_with_expected(
    action: ProviderAction,
    expected: Vec<GemmExpectedUnsafeObligationV1>,
    directory: &TestDirectory,
) -> (CompileOutputV1, usize) {
    let calls = Rc::new(Cell::new(0));
    let emitter = RecordingEmitter {
        calls: Rc::clone(&calls),
        artifact: directory.artifact(),
    };
    let request = request();
    let requirements = GemmProofRequirementsV1::new(&request, expected).unwrap();
    let mut backend = ProofRequiredGemmBackendV1::new(requirements, FixedProvider(action), emitter);
    let output = backend.compile_transaction(&request).unwrap();
    (output, calls.get())
}

fn compile(action: ProviderAction, directory: &TestDirectory) -> (CompileOutputV1, usize) {
    compile_with_expected(action, Vec::new(), directory)
}

fn assert_rejected_without_artifact(
    output: &CompileOutputV1,
    calls: usize,
    directory: &TestDirectory,
) {
    assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
    assert!(output.candidate().is_none());
    assert!(output.snapshots().is_empty());
    assert!(output.receipts().is_empty());
    assert_eq!(calls, 0, "candidate backend must not be invoked");
    assert!(
        !directory.artifact().exists(),
        "failed verification emitted a device artifact"
    );
}

#[test]
fn every_required_property_fails_independently_before_candidate_emission() {
    for property in GEMM_REQUIRED_SAFETY_PROPERTIES_V1 {
        let directory = TestDirectory::new();
        let (output, calls) = compile(
            ProviderAction::Report(report_with_failure(
                property,
                GemmObligationOutcomeV1::Counterexample,
            )),
            &directory,
        );
        assert_rejected_without_artifact(&output, calls, &directory);

        let diagnostic = &output.diagnostics()[0];
        assert_eq!(
            diagnostic.code().get(),
            match property {
                GemmSafetyPropertyV1::MemorySafe => GemmProofDiagnosticV1::MemorySafe,
                GemmSafetyPropertyV1::BoundsSafe => GemmProofDiagnosticV1::BoundsSafe,
                GemmSafetyPropertyV1::Initialized => GemmProofDiagnosticV1::Initialized,
                GemmSafetyPropertyV1::RaceFree => GemmProofDiagnosticV1::RaceFree,
                GemmSafetyPropertyV1::BarrierConvergent => {
                    GemmProofDiagnosticV1::BarrierConvergent
                }
                GemmSafetyPropertyV1::OutputRegionInjective => {
                    GemmProofDiagnosticV1::OutputRegionInjective
                }
                GemmSafetyPropertyV1::LdsEpochCorrect => GemmProofDiagnosticV1::LdsEpochCorrect,
                GemmSafetyPropertyV1::AccumulatorPhaseRefinement => {
                    GemmProofDiagnosticV1::AccumulatorPhaseRefinement
                }
                GemmSafetyPropertyV1::TailRefinement => GemmProofDiagnosticV1::TailRefinement,
                GemmSafetyPropertyV1::EpilogueRefinement => {
                    GemmProofDiagnosticV1::EpilogueRefinement
                }
                GemmSafetyPropertyV1::NumericalContract => {
                    GemmProofDiagnosticV1::NumericalContract
                }
                GemmSafetyPropertyV1::MachineRefinementBoundary => {
                    GemmProofDiagnosticV1::MachineRefinementBoundary
                }
            }
            .code()
        );
        assert_eq!(diagnostic.stage(), Some(property.verification_stage()));
        assert!(diagnostic.message().as_str().contains(property.as_str()));
        assert!(diagnostic.message().as_str().contains("counterexample"));
        assert_eq!(
            diagnostic.subject().unwrap().into_bytes(),
            request().kernel_instance_identity().into_bytes()
        );
    }
}

#[test]
fn unknown_timeout_and_incomplete_are_unproved_not_counterexamples() {
    for outcome in [
        GemmObligationOutcomeV1::Unsupported,
        GemmObligationOutcomeV1::TimedOut,
        GemmObligationOutcomeV1::Incomplete,
    ] {
        let directory = TestDirectory::new();
        let (output, calls) = compile(
            ProviderAction::Report(report_with_failure(GemmSafetyPropertyV1::RaceFree, outcome)),
            &directory,
        );
        assert_rejected_without_artifact(&output, calls, &directory);
        let message = output.diagnostics()[0].message().as_str();
        assert!(message.contains("could not be proved"));
        assert!(!message.contains("counterexample"));
    }
}

#[test]
fn missing_property_cannot_be_replaced_by_discharged_unsafe_finding() {
    let mut findings = required_findings();
    findings.retain(|finding| finding.property() != GemmSafetyPropertyV1::BoundsSafe);
    findings.push(
        GemmObligationFindingV1::unsafe_escape(
            7,
            GemmSafetyPropertyV1::BoundsSafe,
            GemmObligationOutcomeV1::Discharged,
            None,
        )
        .unwrap(),
    );
    let directory = TestDirectory::new();
    let (output, calls) = compile_with_expected(
        ProviderAction::Report(report(findings)),
        vec![expected_unsafe(7, GemmSafetyPropertyV1::BoundsSafe, None)],
        &directory,
    );
    assert_rejected_without_artifact(&output, calls, &directory);
    assert_eq!(
        output.diagnostics()[0].code().get(),
        GemmProofDiagnosticV1::BoundsSafe.code()
    );
    assert!(
        output.diagnostics()[0]
            .message()
            .as_str()
            .contains("result is missing")
    );
}

#[test]
fn unsafe_is_not_proof_authority_and_preserves_its_semantic_subject() {
    let unsafe_subject = DiagnosticSubjectIdentityV1::from_untrusted_bytes(identity(0xa5));
    let mut findings = required_findings();
    findings.push(
        GemmObligationFindingV1::unsafe_escape(
            19,
            GemmSafetyPropertyV1::Initialized,
            GemmObligationOutcomeV1::Incomplete,
            Some(unsafe_subject),
        )
        .unwrap(),
    );
    let directory = TestDirectory::new();
    let (output, calls) = compile_with_expected(
        ProviderAction::Report(report(findings)),
        vec![expected_unsafe(
            19,
            GemmSafetyPropertyV1::Initialized,
            Some(unsafe_subject),
        )],
        &directory,
    );
    assert_rejected_without_artifact(&output, calls, &directory);

    let diagnostic = &output.diagnostics()[0];
    assert_eq!(
        diagnostic.code().get(),
        GemmProofDiagnosticV1::UnsafeObligationUnresolved.code()
    );
    assert_eq!(diagnostic.stage(), Some(CompilerStageV1::Gpu));
    assert_eq!(diagnostic.subject(), Some(unsafe_subject));
    assert!(diagnostic.message().as_str().contains("unsafe"));
    assert!(diagnostic.message().as_str().contains("initialized"));
}

#[test]
fn omitted_compiler_derived_unsafe_obligation_cannot_obtain_admission() {
    let subject = DiagnosticSubjectIdentityV1::from_untrusted_bytes(identity(0xb1));
    let directory = TestDirectory::new();
    let (output, calls) = compile_with_expected(
        ProviderAction::Report(report(required_findings())),
        vec![expected_unsafe(
            23,
            GemmSafetyPropertyV1::RaceFree,
            Some(subject),
        )],
        &directory,
    );
    assert_rejected_without_artifact(&output, calls, &directory);

    let diagnostic = &output.diagnostics()[0];
    assert_eq!(
        diagnostic.code().get(),
        GemmProofDiagnosticV1::UnsafeInventoryMismatch.code()
    );
    assert_eq!(diagnostic.subject(), Some(subject));
    assert!(diagnostic.message().as_str().contains("obligation 23"));
    assert!(diagnostic.message().as_str().contains("no verifier result"));
}

#[test]
fn unexpected_and_substituted_unsafe_findings_fail_closed() {
    let expected_subject = DiagnosticSubjectIdentityV1::from_untrusted_bytes(identity(0xb2));
    let reported_subject = DiagnosticSubjectIdentityV1::from_untrusted_bytes(identity(0xb3));
    let cases = [
        (
            Vec::new(),
            GemmObligationFindingV1::unsafe_escape(
                29,
                GemmSafetyPropertyV1::MemorySafe,
                GemmObligationOutcomeV1::Discharged,
                None,
            )
            .unwrap(),
            "unexpected unsafe obligation 29",
        ),
        (
            vec![expected_unsafe(29, GemmSafetyPropertyV1::BoundsSafe, None)],
            GemmObligationFindingV1::unsafe_escape(
                29,
                GemmSafetyPropertyV1::MemorySafe,
                GemmObligationOutcomeV1::Discharged,
                None,
            )
            .unwrap(),
            "property mismatch",
        ),
        (
            vec![expected_unsafe(
                29,
                GemmSafetyPropertyV1::BoundsSafe,
                Some(expected_subject),
            )],
            GemmObligationFindingV1::unsafe_escape(
                29,
                GemmSafetyPropertyV1::BoundsSafe,
                GemmObligationOutcomeV1::Discharged,
                Some(reported_subject),
            )
            .unwrap(),
            "semantic subject",
        ),
    ];

    for (expected, unsafe_finding, expected_message) in cases {
        let mut findings = required_findings();
        findings.push(unsafe_finding);
        let directory = TestDirectory::new();
        let (output, calls) = compile_with_expected(
            ProviderAction::Report(report(findings)),
            expected,
            &directory,
        );
        assert_rejected_without_artifact(&output, calls, &directory);
        assert_eq!(
            output.diagnostics()[0].code().get(),
            GemmProofDiagnosticV1::UnsafeInventoryMismatch.code()
        );
        assert!(
            output.diagnostics()[0]
                .message()
                .as_str()
                .contains(expected_message)
        );
    }
}

#[test]
fn report_identity_duplicates_and_evaluator_failures_fail_closed() {
    let mut duplicate = required_findings();
    duplicate.push(GemmObligationFindingV1::required(
        GemmSafetyPropertyV1::RaceFree,
        GemmObligationOutcomeV1::Discharged,
        None,
    ));
    let wrong_identity = GemmProofReportV1::new(
        ObligationSetIdentityV1::from_untrusted_bytes(identity(0xee)),
        required_findings(),
    )
    .unwrap();
    let cases = [
        (
            ProviderAction::Report(report(duplicate)),
            GemmProofDiagnosticV1::DuplicateObligation,
        ),
        (
            ProviderAction::Report(wrong_identity),
            GemmProofDiagnosticV1::ObligationSetMismatch,
        ),
        (
            ProviderAction::Fail(GemmProofEvaluationFailureV1::Unavailable),
            GemmProofDiagnosticV1::EvaluationFailed,
        ),
        (
            ProviderAction::Fail(GemmProofEvaluationFailureV1::TimedOut),
            GemmProofDiagnosticV1::EvaluationFailed,
        ),
        (
            ProviderAction::Fail(GemmProofEvaluationFailureV1::ResourceExhausted),
            GemmProofDiagnosticV1::EvaluationFailed,
        ),
        (
            ProviderAction::Fail(GemmProofEvaluationFailureV1::InvalidResult),
            GemmProofDiagnosticV1::EvaluationFailed,
        ),
    ];
    for (action, expected) in cases {
        let directory = TestDirectory::new();
        let (output, calls) = compile(action, &directory);
        assert_rejected_without_artifact(&output, calls, &directory);
        assert_eq!(output.diagnostics()[0].code().get(), expected.code());
    }
}

#[test]
fn duplicate_unsafe_obligation_ids_are_rejected_deterministically() {
    let first_subject = DiagnosticSubjectIdentityV1::from_untrusted_bytes(identity(0x41));
    let second_subject = DiagnosticSubjectIdentityV1::from_untrusted_bytes(identity(0x42));
    let mut findings = required_findings();
    findings.push(
        GemmObligationFindingV1::unsafe_escape(
            9,
            GemmSafetyPropertyV1::BoundsSafe,
            GemmObligationOutcomeV1::Discharged,
            Some(first_subject),
        )
        .unwrap(),
    );
    findings.push(
        GemmObligationFindingV1::unsafe_escape(
            9,
            GemmSafetyPropertyV1::MemorySafe,
            GemmObligationOutcomeV1::Discharged,
            Some(second_subject),
        )
        .unwrap(),
    );
    let directory = TestDirectory::new();
    let (output, calls) = compile_with_expected(
        ProviderAction::Report(report(findings)),
        vec![expected_unsafe(
            9,
            GemmSafetyPropertyV1::BoundsSafe,
            Some(first_subject),
        )],
        &directory,
    );
    assert_rejected_without_artifact(&output, calls, &directory);
    assert_eq!(
        output.diagnostics()[0].code().get(),
        GemmProofDiagnosticV1::DuplicateObligation.code()
    );
    assert!(
        output.diagnostics()[0]
            .message()
            .as_str()
            .contains("unsafe obligation 9")
    );
}

#[test]
fn only_complete_results_allow_candidate_emission() {
    let mut findings = required_findings();
    findings.push(
        GemmObligationFindingV1::unsafe_escape(
            1,
            GemmSafetyPropertyV1::MemorySafe,
            GemmObligationOutcomeV1::Discharged,
            None,
        )
        .unwrap(),
    );
    let directory = TestDirectory::new();
    let (output, calls) = compile_with_expected(
        ProviderAction::Report(report(findings)),
        vec![expected_unsafe(1, GemmSafetyPropertyV1::MemorySafe, None)],
        &directory,
    );

    assert_eq!(
        output.disposition(),
        CompileDispositionV1::CandidateProduced
    );
    assert!(output.candidate().is_some());
    assert_eq!(calls, 1);
    assert_eq!(
        fs::read(directory.artifact()).unwrap(),
        [0x7f, b'E', b'L', b'F']
    );
}

#[test]
fn direct_admission_reports_stable_semantic_failure_without_token() {
    let request = request();
    let requirements = GemmProofRequirementsV1::new(&request, Vec::new()).unwrap();
    let rejection = admit_proof_required_gemm_v1(
        &request,
        &requirements,
        &report_with_failure(
            GemmSafetyPropertyV1::BarrierConvergent,
            GemmObligationOutcomeV1::TimedOut,
        ),
    )
    .unwrap_err();

    assert_eq!(
        rejection,
        GemmProofRejectionKindV1::RequiredPropertyNotDischarged {
            property: GemmSafetyPropertyV1::BarrierConvergent,
            outcome: GemmObligationOutcomeV1::TimedOut,
        }
    );
}

#[test]
fn report_and_unsafe_inventory_are_hard_bounded() {
    let too_many = vec![
        GemmObligationFindingV1::required(
            GemmSafetyPropertyV1::MemorySafe,
            GemmObligationOutcomeV1::Discharged,
            None,
        );
        MAX_GEMM_OBLIGATION_FINDINGS_V1 + 1
    ];
    assert!(GemmProofReportV1::new(obligations(), too_many).is_err());
    assert!(
        GemmObligationFindingV1::unsafe_escape(
            0,
            GemmSafetyPropertyV1::MemorySafe,
            GemmObligationOutcomeV1::Discharged,
            None,
        )
        .is_err()
    );

    let too_many_expected = vec![
        expected_unsafe(1, GemmSafetyPropertyV1::MemorySafe, None);
        MAX_GEMM_UNSAFE_OBLIGATIONS_V1 + 1
    ];
    assert!(GemmProofRequirementsV1::new(&request(), too_many_expected).is_err());
    assert!(
        GemmProofRequirementsV1::new(
            &request(),
            vec![
                expected_unsafe(7, GemmSafetyPropertyV1::BoundsSafe, None),
                expected_unsafe(7, GemmSafetyPropertyV1::MemorySafe, None),
            ],
        )
        .is_err()
    );
}

#[test]
fn compiler_requirements_are_bound_to_the_exact_request_and_obligation_set() {
    let original = request();
    let requirements = GemmProofRequirementsV1::new(&original, Vec::new()).unwrap();
    let report = report(required_findings());

    assert_eq!(
        admit_proof_required_gemm_v1(&request_with(0x99, 0x16), &requirements, &report,)
            .unwrap_err(),
        GemmProofRejectionKindV1::RequirementsRequestMismatch
    );
    assert_eq!(
        admit_proof_required_gemm_v1(&request_with(0x11, 0x99), &requirements, &report,)
            .unwrap_err(),
        GemmProofRejectionKindV1::RequirementsObligationSetMismatch
    );
}

#[test]
fn diagnostic_selection_is_independent_of_report_order() {
    let first_subject = DiagnosticSubjectIdentityV1::from_untrusted_bytes(identity(0x51));
    let second_subject = DiagnosticSubjectIdentityV1::from_untrusted_bytes(identity(0x52));
    let failures = [
        GemmObligationFindingV1::unsafe_escape(
            11,
            GemmSafetyPropertyV1::MemorySafe,
            GemmObligationOutcomeV1::TimedOut,
            Some(second_subject),
        )
        .unwrap(),
        GemmObligationFindingV1::unsafe_escape(
            3,
            GemmSafetyPropertyV1::BoundsSafe,
            GemmObligationOutcomeV1::Unsupported,
            Some(first_subject),
        )
        .unwrap(),
    ];
    let mut forward = required_findings();
    forward.extend(failures);
    let mut reverse = required_findings();
    reverse.extend(failures.into_iter().rev());

    let first_directory = TestDirectory::new();
    let second_directory = TestDirectory::new();
    let expected = vec![
        expected_unsafe(3, GemmSafetyPropertyV1::BoundsSafe, Some(first_subject)),
        expected_unsafe(11, GemmSafetyPropertyV1::MemorySafe, Some(second_subject)),
    ];
    let (first, first_calls) = compile_with_expected(
        ProviderAction::Report(report(forward)),
        expected.clone(),
        &first_directory,
    );
    let (second, second_calls) = compile_with_expected(
        ProviderAction::Report(report(reverse)),
        expected,
        &second_directory,
    );
    assert_rejected_without_artifact(&first, first_calls, &first_directory);
    assert_rejected_without_artifact(&second, second_calls, &second_directory);

    assert_eq!(first.diagnostics(), second.diagnostics());
    assert_eq!(first.diagnostics()[0].subject(), Some(first_subject));
    assert!(
        first.diagnostics()[0]
            .message()
            .as_str()
            .contains("unsafe GEMM obligation 3")
    );
}
