use std::{cell::RefCell, rc::Rc};

use fe2o3_compiler_api::{
    CandidateFormatIdentityV1, CandidateIdentityV1, CanonicalDiagnosticV1, CompileDispositionV1,
    CompileLimitsV1, CompileOutputV1, CompileRequestV1, CompilerProfileIdentityV1, CompilerStageV1,
    DiagnosticCodeV1, DiagnosticMessageV1, DiagnosticSeverityV1, ExecutableCandidateV1,
    KernelInstanceIdentityV1, ObligationSetIdentityV1, PipelineConfigurationIdentityV1,
    ReceiptOutcomeV1, RequestIdentityV1, SnapshotFormatIdentityV1, SnapshotIdentityV1,
    StageReceiptV1, StageSnapshotV1, TargetProfileIdentityV1, TransformConfigurationIdentityV1,
    TransformIdentityV1,
};
use fe2o3_compiler_driver::{
    CompilerBackendFailureV1, DriverDiagnosticV1, ProductionCompilerDriverV1,
    TransactionalCompilerBackendV1, TransactionalCompilerDriverV1,
};

const BACKEND_DIAGNOSTIC_CODE: u32 = 0x4245_0001;

#[derive(Clone, Debug)]
enum FakeAction {
    ValidSourceRejection,
    Return(Box<CompileOutputV1>),
    Fail(CompilerBackendFailureV1),
}

#[derive(Clone, Debug)]
struct RecordingBackend {
    calls: Rc<RefCell<Vec<RequestIdentityV1>>>,
    action: FakeAction,
}

impl TransactionalCompilerBackendV1 for RecordingBackend {
    fn compile_transaction(
        &mut self,
        request: &CompileRequestV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        self.calls.borrow_mut().push(request.identity());
        match &self.action {
            FakeAction::ValidSourceRejection => Ok(source_rejection(request)),
            FakeAction::Return(output) => Ok((**output).clone()),
            FakeAction::Fail(failure) => Err(*failure),
        }
    }
}

fn limits() -> CompileLimitsV1 {
    CompileLimitsV1::new(4, 4, 4, 16, 32, 16).unwrap()
}

fn request(request_identity: u8, input_identity: u8, limits: CompileLimitsV1) -> CompileRequestV1 {
    CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes([request_identity; 32]),
        KernelInstanceIdentityV1::from_untrusted_bytes([2; 32]),
        CompilerProfileIdentityV1::from_untrusted_bytes([3; 32]),
        TargetProfileIdentityV1::from_untrusted_bytes([4; 32]),
        PipelineConfigurationIdentityV1::from_untrusted_bytes([5; 32]),
        obligations(6),
        snapshot(CompilerStageV1::FrontendInput, input_identity),
        limits,
    )
    .unwrap()
}

fn snapshot(stage: CompilerStageV1, identity: u8) -> StageSnapshotV1 {
    StageSnapshotV1::new(
        stage,
        snapshot_identity(identity),
        SnapshotFormatIdentityV1::from_untrusted_bytes([0xf0; 32]),
        vec![identity],
    )
    .unwrap()
}

fn snapshot_identity(byte: u8) -> SnapshotIdentityV1 {
    SnapshotIdentityV1::from_untrusted_bytes([byte; 32])
}

fn obligations(byte: u8) -> ObligationSetIdentityV1 {
    ObligationSetIdentityV1::from_untrusted_bytes([byte; 32])
}

fn produced_receipt(
    sequence: u16,
    stage: CompilerStageV1,
    input: SnapshotIdentityV1,
    output: SnapshotIdentityV1,
    input_obligations: ObligationSetIdentityV1,
    output_obligations: ObligationSetIdentityV1,
) -> StageReceiptV1 {
    StageReceiptV1::new(
        sequence,
        stage,
        TransformIdentityV1::from_untrusted_bytes([0x71; 32]),
        TransformConfigurationIdentityV1::from_untrusted_bytes([0x72; 32]),
        input,
        Some(output),
        input_obligations,
        Some(output_obligations),
        ReceiptOutcomeV1::Produced,
    )
    .unwrap()
}

fn rejected_receipt(
    sequence: u16,
    input: SnapshotIdentityV1,
    input_obligations: ObligationSetIdentityV1,
) -> StageReceiptV1 {
    StageReceiptV1::new(
        sequence,
        CompilerStageV1::Kernel,
        TransformIdentityV1::from_untrusted_bytes([0x73; 32]),
        TransformConfigurationIdentityV1::from_untrusted_bytes([0x74; 32]),
        input,
        None,
        input_obligations,
        None,
        ReceiptOutcomeV1::Rejected,
    )
    .unwrap()
}

fn diagnostic(sequence: u16, severity: DiagnosticSeverityV1) -> CanonicalDiagnosticV1 {
    CanonicalDiagnosticV1::new(
        sequence,
        DiagnosticCodeV1::new(BACKEND_DIAGNOSTIC_CODE + u32::from(sequence)).unwrap(),
        severity,
        None,
        None,
        DiagnosticMessageV1::new("fake backend diagnostic").unwrap(),
    )
}

fn source_rejection(request: &CompileRequestV1) -> CompileOutputV1 {
    let mir = snapshot(CompilerStageV1::Mir, 0x20);
    let mir_obligations = obligations(0x21);
    let produced = produced_receipt(
        0,
        CompilerStageV1::Mir,
        request.input().identity(),
        mir.identity(),
        request.input_obligations_identity(),
        mir_obligations,
    );
    let rejected = rejected_receipt(1, mir.identity(), mir_obligations);
    CompileOutputV1::new(
        request,
        CompileDispositionV1::Rejected,
        vec![mir],
        vec![produced, rejected],
        vec![diagnostic(0, DiagnosticSeverityV1::Error)],
        None,
    )
    .unwrap()
}

fn candidate_output(request: &CompileRequestV1) -> CompileOutputV1 {
    let hsaco = snapshot(CompilerStageV1::Hsaco, 0x30);
    let receipt = produced_receipt(
        0,
        CompilerStageV1::Hsaco,
        request.input().identity(),
        hsaco.identity(),
        request.input_obligations_identity(),
        obligations(0x31),
    );
    let candidate = ExecutableCandidateV1::new(
        CandidateIdentityV1::from_untrusted_bytes([0x32; 32]),
        CandidateFormatIdentityV1::from_untrusted_bytes([0x33; 32]),
        hsaco.identity(),
        vec![0x7f, b'E', b'L', b'F'],
    )
    .unwrap();
    CompileOutputV1::new(
        request,
        CompileDispositionV1::CandidateProduced,
        vec![hsaco],
        vec![receipt],
        Vec::new(),
        Some(candidate),
    )
    .unwrap()
}

fn two_diagnostic_rejection(request: &CompileRequestV1) -> CompileOutputV1 {
    CompileOutputV1::new(
        request,
        CompileDispositionV1::Rejected,
        Vec::new(),
        Vec::new(),
        vec![
            diagnostic(0, DiagnosticSeverityV1::Error),
            diagnostic(1, DiagnosticSeverityV1::Note),
        ],
        None,
    )
    .unwrap()
}

fn driver(
    calls: &Rc<RefCell<Vec<RequestIdentityV1>>>,
    action: FakeAction,
) -> ProductionCompilerDriverV1<RecordingBackend> {
    ProductionCompilerDriverV1::new(RecordingBackend {
        calls: Rc::clone(calls),
        action,
    })
}

fn assert_driver_rejection(
    output: &CompileOutputV1,
    request: &CompileRequestV1,
    reason: DriverDiagnosticV1,
) {
    assert_eq!(output.request_identity(), request.identity());
    assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
    assert!(output.snapshots().is_empty());
    assert!(output.receipts().is_empty());
    assert!(output.candidate().is_none());
    assert_eq!(output.diagnostics().len(), 1);
    assert_eq!(output.diagnostics()[0].sequence(), 0);
    assert_eq!(
        output.diagnostics()[0].severity(),
        DiagnosticSeverityV1::Error
    );
    assert_eq!(output.diagnostics()[0].code().get(), reason.code());
}

#[test]
fn every_request_invokes_the_sole_backend_once() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut driver = driver(&calls, FakeAction::ValidSourceRejection);
    let request = request(0x11, 0x21, limits());

    let output = driver.compile_transaction(&request);

    assert_eq!(output.request_identity(), request.identity());
    assert_eq!(calls.borrow().as_slice(), &[request.identity()]);
}

#[test]
fn valid_candidate_from_the_sole_backend_is_preserved() {
    let request = request(0x12, 0x22, limits());
    let expected = candidate_output(&request);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut driver = driver(&calls, FakeAction::Return(Box::new(expected.clone())));

    assert_eq!(driver.compile_transaction(&request), expected);
    assert_eq!(calls.borrow().as_slice(), &[request.identity()]);
}

#[test]
fn backend_failures_have_stable_fail_closed_diagnostics() {
    let cases = [
        (
            CompilerBackendFailureV1::UnsupportedRequest,
            DriverDiagnosticV1::BackendUnsupportedRequest,
        ),
        (
            CompilerBackendFailureV1::Unavailable,
            DriverDiagnosticV1::BackendUnavailable,
        ),
        (
            CompilerBackendFailureV1::ResourceExhausted,
            DriverDiagnosticV1::BackendResourceExhausted,
        ),
        (
            CompilerBackendFailureV1::Internal,
            DriverDiagnosticV1::BackendInternalFailure,
        ),
    ];
    for (failure, reason) in cases {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut driver = driver(&calls, FakeAction::Fail(failure));
        let request = request(0x13, 0x23, limits());
        let output = driver.compile_transaction(&request);
        assert_driver_rejection(&output, &request, reason);
        assert_eq!(calls.borrow().as_slice(), &[request.identity()]);
    }
}

#[test]
fn request_identity_mismatch_is_rejected_without_committing_backend_records() {
    let routed = request(0x14, 0x24, limits());
    let foreign = request(0x15, 0x24, limits());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut driver = driver(
        &calls,
        FakeAction::Return(Box::new(source_rejection(&foreign))),
    );

    let output = driver.compile_transaction(&routed);

    assert_driver_rejection(&output, &routed, DriverDiagnosticV1::OutputRequestMismatch);
    assert_eq!(calls.borrow().as_slice(), &[routed.identity()]);
}

#[test]
fn backend_output_malformed_for_routed_input_is_rejected() {
    let routed = request(0x16, 0x26, limits());
    let colliding_identity = request(0x16, 0x27, limits());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut driver = driver(
        &calls,
        FakeAction::Return(Box::new(source_rejection(&colliding_identity))),
    );

    let output = driver.compile_transaction(&routed);

    assert_driver_rejection(&output, &routed, DriverDiagnosticV1::InvalidBackendOutput);
    assert_eq!(calls.borrow().as_slice(), &[routed.identity()]);
}

#[test]
fn backend_output_valid_only_under_looser_limits_is_rejected() {
    let tight_limits = CompileLimitsV1::new(1, 1, 1, 1, 1, 1).unwrap();
    let routed = request(0x17, 0x28, tight_limits);
    let permissive = request(0x17, 0x28, limits());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut driver = driver(
        &calls,
        FakeAction::Return(Box::new(two_diagnostic_rejection(&permissive))),
    );

    let output = driver.compile_transaction(&routed);

    assert_driver_rejection(&output, &routed, DriverDiagnosticV1::InvalidBackendOutput);
    assert_eq!(calls.borrow().as_slice(), &[routed.identity()]);
}

#[test]
fn valid_source_rejection_preserves_the_complete_receipt_chain() {
    let request = request(0x18, 0x29, limits());
    let expected = source_rejection(&request);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut driver = driver(&calls, FakeAction::Return(Box::new(expected.clone())));

    let output = driver.compile_transaction(&request);

    assert_eq!(output, expected);
    assert_eq!(output.receipts().len(), 2);
    assert_eq!(output.receipts()[0].outcome(), ReceiptOutcomeV1::Produced);
    assert_eq!(output.receipts()[1].outcome(), ReceiptOutcomeV1::Rejected);
    assert_eq!(
        output.receipts()[0].output_snapshot_identity(),
        Some(output.receipts()[1].input_snapshot_identity())
    );
    assert_eq!(
        output.receipts()[0].output_obligations_identity(),
        Some(output.receipts()[1].input_obligations_identity())
    );
    assert_eq!(calls.borrow().as_slice(), &[request.identity()]);
}
