use fe2o3_compiler_api::{
    CompileDispositionV1, CompileLimitsV1, CompileOutputV1, CompileRequestV1,
    CompilerProfileIdentityV1, CompilerStageV1, KernelInstanceIdentityV1, ObligationSetIdentityV1,
    PipelineConfigurationIdentityV1, PipelineSelectorV1, RequestIdentityV1,
    SnapshotFormatIdentityV1, SnapshotIdentityV1, StageSnapshotV1, TargetProfileIdentityV1,
};
use fe2o3_compiler_driver::{
    CompilerBackendFailureV1, GemmSemanticAnalysisErrorV1, GemmSemanticCheckingBackendV1,
    GemmSemanticProgramBindingErrorV1, GemmSemanticProgramV1, TransactionalCompilerBackendV1,
    analyze_gemm_semantics_v1, general_gemm_semantic_obligation_set_identity_v1,
};
use fe2o3_kernel_ir::{
    GENERAL_GEMM_MUTATION_EXPECTATIONS_V1, GeneralGemmKirV1, GeneralGemmPlanFieldsV1,
    GeneralGemmPlanSnapshotV1, GeneralGemmVerificationStageV1,
    general_gemm_semantic_mutation_kir_v1,
};

fn request(
    identity: u8,
    snapshot_identity: SnapshotIdentityV1,
    obligation_set_identity: ObligationSetIdentityV1,
) -> CompileRequestV1 {
    CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes([identity; 32]),
        KernelInstanceIdentityV1::from_untrusted_bytes([2; 32]),
        CompilerProfileIdentityV1::from_untrusted_bytes([3; 32]),
        TargetProfileIdentityV1::from_untrusted_bytes([4; 32]),
        PipelineConfigurationIdentityV1::from_untrusted_bytes([5; 32]),
        obligation_set_identity,
        PipelineSelectorV1::PlironV1,
        StageSnapshotV1::new(
            CompilerStageV1::FrontendInput,
            snapshot_identity,
            SnapshotFormatIdentityV1::from_untrusted_bytes([7; 32]),
            vec![8],
        )
        .unwrap(),
        CompileLimitsV1::default(),
    )
    .unwrap()
}

fn bound_request(
    identity: u8,
    snapshot_identity: SnapshotIdentityV1,
    kir: &GeneralGemmKirV1,
) -> CompileRequestV1 {
    request(
        identity,
        snapshot_identity,
        general_gemm_semantic_obligation_set_identity_v1(snapshot_identity, kir),
    )
}

fn plan() -> GeneralGemmPlanFieldsV1 {
    GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [17, 19, 18],
        strides: [23, 29, 31],
        storage_elements: [386, 512, 515],
        block_counts: [2, 2, 1],
        aql_grid_work_items: [128, 2, 1],
        reduction_phases: 2,
        alpha_bits: 2.0_f32.to_bits(),
        beta_bits: (-1.0_f32).to_bits(),
    })
    .unwrap()
}

const fn compiler_stage(stage: GeneralGemmVerificationStageV1) -> CompilerStageV1 {
    match stage {
        GeneralGemmVerificationStageV1::Kernel => CompilerStageV1::Kernel,
        GeneralGemmVerificationStageV1::Tile => CompilerStageV1::Tile,
        GeneralGemmVerificationStageV1::Gpu => CompilerStageV1::Gpu,
        GeneralGemmVerificationStageV1::Amdgcn => CompilerStageV1::Amdgcn,
    }
}

#[derive(Default)]
struct RecordingBackend {
    calls: usize,
}

impl TransactionalCompilerBackendV1 for RecordingBackend {
    fn compile_transaction(
        &mut self,
        _request: &CompileRequestV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        self.calls += 1;
        Err(CompilerBackendFailureV1::UnsupportedRequest)
    }
}

#[test]
fn valid_structured_kir_delegates_without_minting_proof_authority() {
    let kir = GeneralGemmKirV1::canonical(plan());
    let request = bound_request(1, SnapshotIdentityV1::from_untrusted_bytes([6; 32]), &kir);
    let program = GemmSemanticProgramV1::new(&request, kir).unwrap();
    assert_eq!(analyze_gemm_semantics_v1(&program), Ok(()));

    let mut backend = GemmSemanticCheckingBackendV1::new(program, RecordingBackend::default());
    assert_eq!(
        backend.compile_transaction(&request),
        Err(CompilerBackendFailureV1::UnsupportedRequest)
    );
    assert_eq!(backend.parts().1.calls, 1);
}

#[test]
fn all_registered_kir_mutations_reject_transactionally_with_frozen_diagnostics() {
    let snapshot_identity = SnapshotIdentityV1::from_untrusted_bytes([6; 32]);
    let canonical_binding = general_gemm_semantic_obligation_set_identity_v1(
        snapshot_identity,
        &GeneralGemmKirV1::canonical(plan()),
    );
    let mut mutation_bindings = std::collections::BTreeSet::new();
    assert_eq!(GENERAL_GEMM_MUTATION_EXPECTATIONS_V1.len(), 15);
    for expectation in GENERAL_GEMM_MUTATION_EXPECTATIONS_V1 {
        let kir = general_gemm_semantic_mutation_kir_v1(plan(), expectation.mutation);
        let binding = general_gemm_semantic_obligation_set_identity_v1(snapshot_identity, &kir);
        assert_ne!(
            binding,
            canonical_binding,
            "{}",
            expectation.mutation.as_str()
        );
        assert!(
            mutation_bindings.insert(binding),
            "{} did not have a unique semantic binding",
            expectation.mutation.as_str()
        );
        let request = request(1, snapshot_identity, binding);
        let program = GemmSemanticProgramV1::new(&request, kir).unwrap();
        let counterexample = match analyze_gemm_semantics_v1(&program) {
            Err(GemmSemanticAnalysisErrorV1::Counterexample(counterexample)) => counterexample,
            other => panic!("{} produced {other:?}", expectation.mutation.as_str()),
        };
        assert_eq!(
            counterexample.property().as_str(),
            expectation.property.as_str(),
            "{}",
            expectation.mutation.as_str()
        );

        let mut backend = GemmSemanticCheckingBackendV1::new(program, RecordingBackend::default());
        let output = backend.compile_transaction(&request).unwrap();
        assert_eq!(backend.parts().1.calls, 0);
        assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
        assert!(output.snapshots().is_empty());
        assert!(output.receipts().is_empty());
        assert!(output.candidate().is_none());
        assert_eq!(output.diagnostics().len(), 1);
        let diagnostic = &output.diagnostics()[0];
        assert_eq!(
            diagnostic.code().get(),
            expectation.code,
            "{}",
            expectation.mutation.as_str()
        );
        assert_eq!(
            diagnostic.stage(),
            Some(compiler_stage(expectation.stage)),
            "{}",
            expectation.mutation.as_str()
        );
        assert!(
            diagnostic
                .message()
                .as_str()
                .contains(expectation.property.as_str())
        );
    }
}

#[test]
fn request_substitution_fails_before_downstream_or_artifact_construction() {
    let kir = GeneralGemmKirV1::canonical(plan());
    let snapshot_identity = SnapshotIdentityV1::from_untrusted_bytes([6; 32]);
    let first = bound_request(1, snapshot_identity, &kir);
    let second = bound_request(2, snapshot_identity, &kir);
    let program = GemmSemanticProgramV1::new(&first, kir).unwrap();
    let mut backend = GemmSemanticCheckingBackendV1::new(program, RecordingBackend::default());
    let output = backend.compile_transaction(&second).unwrap();

    assert_eq!(backend.parts().1.calls, 0);
    assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
    assert!(output.snapshots().is_empty());
    assert!(output.receipts().is_empty());
    assert!(output.candidate().is_none());
    assert_eq!(output.diagnostics()[0].code().get(), 0x4647_0006);
    assert_eq!(output.diagnostics()[0].stage(), Some(CompilerStageV1::Mir));
}

#[test]
fn valid_kir_cannot_replace_mutated_kir_committed_by_request() {
    let snapshot_identity = SnapshotIdentityV1::from_untrusted_bytes([6; 32]);
    let invalid_kir = general_gemm_semantic_mutation_kir_v1(
        plan(),
        GENERAL_GEMM_MUTATION_EXPECTATIONS_V1[0].mutation,
    );
    let invalid_request = bound_request(1, snapshot_identity, &invalid_kir);

    assert_eq!(
        GemmSemanticProgramV1::new(&invalid_request, GeneralGemmKirV1::canonical(plan())),
        Err(GemmSemanticProgramBindingErrorV1::ObligationSetMismatch)
    );

    let invalid_program = GemmSemanticProgramV1::new(&invalid_request, invalid_kir).unwrap();
    let mut backend =
        GemmSemanticCheckingBackendV1::new(invalid_program, RecordingBackend::default());
    let output = backend.compile_transaction(&invalid_request).unwrap();
    assert_eq!(backend.parts().1.calls, 0);
    assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
    assert!(output.snapshots().is_empty());
    assert!(output.receipts().is_empty());
    assert!(output.candidate().is_none());
}

#[test]
fn changed_source_snapshot_cannot_reuse_semantic_obligation_binding() {
    let kir = GeneralGemmKirV1::canonical(plan());
    let first_snapshot = SnapshotIdentityV1::from_untrusted_bytes([6; 32]);
    let second_snapshot = SnapshotIdentityV1::from_untrusted_bytes([16; 32]);
    let first_binding = general_gemm_semantic_obligation_set_identity_v1(first_snapshot, &kir);
    let substituted_request = request(1, second_snapshot, first_binding);

    assert_eq!(
        GemmSemanticProgramV1::new(&substituted_request, kir),
        Err(GemmSemanticProgramBindingErrorV1::ObligationSetMismatch)
    );
}
