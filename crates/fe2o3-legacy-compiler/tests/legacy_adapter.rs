use std::{cell::RefCell, rc::Rc};

use fe2o3_compiler_api::{
    CanonicalDiagnosticV1, CompileDispositionV1, CompileLimitsV1, CompileOutputV1,
    CompileRequestV1, CompilerProfileIdentityV1, CompilerStageV1, DiagnosticCodeV1,
    DiagnosticMessageV1, DiagnosticSeverityV1, KernelInstanceIdentityV1, ObligationSetIdentityV1,
    PipelineConfigurationIdentityV1, PipelineSelectorV1, RequestIdentityV1,
    SnapshotFormatIdentityV1, SnapshotIdentityV1, StageSnapshotV1, TargetProfileIdentityV1,
};
use fe2o3_compiler_driver::{CompilerBackendFailureV1, TransactionalCompilerBackendV1};
use fe2o3_legacy_compiler::{LegacyCompilePathV1, LegacyCompilerAdapterV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PathCall {
    selector: PipelineSelectorV1,
    request_identity: RequestIdentityV1,
}

#[derive(Clone, Debug)]
struct RecordingLegacyPath {
    calls: Rc<RefCell<Vec<PathCall>>>,
}

impl LegacyCompilePathV1 for RecordingLegacyPath {
    fn compile_legacy_transaction(
        &mut self,
        request: &CompileRequestV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        self.calls.borrow_mut().push(PathCall {
            selector: request.selector(),
            request_identity: request.identity(),
        });
        Ok(source_rejection(request))
    }
}

fn request(selector: PipelineSelectorV1, identity: u8) -> CompileRequestV1 {
    CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes([identity; 32]),
        KernelInstanceIdentityV1::from_untrusted_bytes([2; 32]),
        CompilerProfileIdentityV1::from_untrusted_bytes([3; 32]),
        TargetProfileIdentityV1::from_untrusted_bytes([4; 32]),
        PipelineConfigurationIdentityV1::from_untrusted_bytes([5; 32]),
        ObligationSetIdentityV1::from_untrusted_bytes([6; 32]),
        selector,
        StageSnapshotV1::new(
            CompilerStageV1::FrontendInput,
            SnapshotIdentityV1::from_untrusted_bytes([7; 32]),
            SnapshotFormatIdentityV1::from_untrusted_bytes([8; 32]),
            vec![0x99],
        )
        .unwrap(),
        CompileLimitsV1::new(2, 2, 2, 8, 16, 8).unwrap(),
    )
    .unwrap()
}

fn source_rejection(request: &CompileRequestV1) -> CompileOutputV1 {
    let diagnostic = CanonicalDiagnosticV1::new(
        0,
        DiagnosticCodeV1::new(0x4c45_0001).unwrap(),
        DiagnosticSeverityV1::Error,
        None,
        None,
        DiagnosticMessageV1::new("legacy source rejection").unwrap(),
    );
    CompileOutputV1::new(
        request,
        CompileDispositionV1::Rejected,
        Vec::new(),
        Vec::new(),
        vec![diagnostic],
        None,
    )
    .unwrap()
}

#[test]
fn non_legacy_selectors_are_rejected_before_the_path_is_invoked() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut adapter = LegacyCompilerAdapterV1::new(RecordingLegacyPath {
        calls: Rc::clone(&calls),
    });

    for selector in [
        PipelineSelectorV1::PlironShadow,
        PipelineSelectorV1::PlironV1,
    ] {
        let request = request(selector, selector as u8);
        let result = adapter.compile_transaction(&request);

        assert_eq!(result, Err(CompilerBackendFailureV1::UnsupportedRequest));
        assert!(calls.borrow().is_empty());
    }
}

#[test]
fn legacy_request_is_forwarded_once_and_its_output_is_preserved() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut adapter = LegacyCompilerAdapterV1::new(RecordingLegacyPath {
        calls: Rc::clone(&calls),
    });
    let request = request(PipelineSelectorV1::Legacy, 0x41);
    let expected = source_rejection(&request);

    let output = adapter.compile_transaction(&request).unwrap();

    assert_eq!(output, expected);
    assert_eq!(
        *calls.borrow(),
        [PathCall {
            selector: PipelineSelectorV1::Legacy,
            request_identity: request.identity(),
        }]
    );
}
