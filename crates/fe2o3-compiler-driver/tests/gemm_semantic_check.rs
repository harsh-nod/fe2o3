use std::{cell::Cell, rc::Rc};

use fe2o3_compiler_api::{
    CompileDispositionV1, CompileLimitsV1, CompileOutputV1, CompileRequestV1,
    CompilerProfileIdentityV1, CompilerStageV1, KernelInstanceIdentityV1, ObligationSetIdentityV1,
    PipelineConfigurationIdentityV1, RequestIdentityV1, SnapshotFormatIdentityV1,
    SnapshotIdentityV1, StageSnapshotV1, TargetProfileIdentityV1,
};
use fe2o3_compiler_driver::{
    AdmittedGemmCompilerBackendV1, CompilerBackendFailureV1, GemmProofEvaluationFailureV1,
    GemmProofReportProviderV1, GemmProofReportV1, GemmProofRequirementsV1,
    GemmSemanticAnalysisErrorV1, GemmSemanticCheckingBackendV1, GemmSemanticProgramBindingErrorV1,
    GemmSemanticProgramV1, ProofRequiredGemmAdmissionV1, ProofRequiredGemmBackendV1,
    TransactionalCompilerBackendV1, analyze_gemm_semantics_v1,
    general_gemm_semantic_obligation_set_identity_v1,
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

#[derive(Clone, Copy, Debug)]
enum SameIdentityRequestMutation {
    KernelInstance,
    CompilerProfile,
    TargetProfile,
    PipelineConfiguration,
    Limits,
    SnapshotFormat,
    SnapshotPayload,
}

const SAME_IDENTITY_REQUEST_MUTATIONS: [SameIdentityRequestMutation; 7] = [
    SameIdentityRequestMutation::KernelInstance,
    SameIdentityRequestMutation::CompilerProfile,
    SameIdentityRequestMutation::TargetProfile,
    SameIdentityRequestMutation::PipelineConfiguration,
    SameIdentityRequestMutation::Limits,
    SameIdentityRequestMutation::SnapshotFormat,
    SameIdentityRequestMutation::SnapshotPayload,
];

fn mutate_request_with_same_identity(
    request: &CompileRequestV1,
    mutation: SameIdentityRequestMutation,
) -> CompileRequestV1 {
    let kernel_instance_identity = match mutation {
        SameIdentityRequestMutation::KernelInstance => {
            KernelInstanceIdentityV1::from_untrusted_bytes([12; 32])
        }
        _ => request.kernel_instance_identity(),
    };
    let compiler_profile_identity = match mutation {
        SameIdentityRequestMutation::CompilerProfile => {
            CompilerProfileIdentityV1::from_untrusted_bytes([13; 32])
        }
        _ => request.compiler_profile_identity(),
    };
    let target_profile_identity = match mutation {
        SameIdentityRequestMutation::TargetProfile => {
            TargetProfileIdentityV1::from_untrusted_bytes([14; 32])
        }
        _ => request.target_profile_identity(),
    };
    let pipeline_configuration_identity = match mutation {
        SameIdentityRequestMutation::PipelineConfiguration => {
            PipelineConfigurationIdentityV1::from_untrusted_bytes([15; 32])
        }
        _ => request.pipeline_configuration_identity(),
    };
    let limits = match mutation {
        SameIdentityRequestMutation::Limits => CompileLimitsV1::new(
            request.limits().max_stage_snapshots() - 1,
            request.limits().max_stage_receipts(),
            request.limits().max_diagnostics(),
            request.limits().max_snapshot_bytes(),
            request.limits().max_total_snapshot_bytes(),
            request.limits().max_candidate_bytes(),
        )
        .unwrap(),
        _ => request.limits(),
    };
    let format_identity = match mutation {
        SameIdentityRequestMutation::SnapshotFormat => {
            SnapshotFormatIdentityV1::from_untrusted_bytes([17; 32])
        }
        _ => request.input().format_identity(),
    };
    let payload = match mutation {
        SameIdentityRequestMutation::SnapshotPayload => vec![18],
        _ => request.input().canonical_bytes().to_vec(),
    };
    let input = StageSnapshotV1::new(
        request.input().stage(),
        request.input().identity(),
        format_identity,
        payload,
    )
    .unwrap();

    CompileRequestV1::new(
        request.identity(),
        kernel_instance_identity,
        compiler_profile_identity,
        target_profile_identity,
        pipeline_configuration_identity,
        request.input_obligations_identity(),
        input,
        limits,
    )
    .unwrap()
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

#[derive(Clone)]
struct RejectingProofProvider {
    calls: Rc<Cell<usize>>,
}

impl GemmProofReportProviderV1 for RejectingProofProvider {
    fn evaluate(
        &mut self,
        _request: &CompileRequestV1,
        _requirements: &GemmProofRequirementsV1,
    ) -> Result<GemmProofReportV1, GemmProofEvaluationFailureV1> {
        self.calls.set(self.calls.get() + 1);
        Err(GemmProofEvaluationFailureV1::InvalidResult)
    }
}

#[derive(Clone)]
struct MaliciousCandidateBackend {
    calls: Rc<Cell<usize>>,
}

impl AdmittedGemmCompilerBackendV1 for MaliciousCandidateBackend {
    fn compile_admitted(
        &mut self,
        _request: &CompileRequestV1,
        _admission: ProofRequiredGemmAdmissionV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        self.calls.set(self.calls.get() + 1);
        Err(CompilerBackendFailureV1::Internal)
    }
}

type TestSemanticBackend = GemmSemanticCheckingBackendV1<
    ProofRequiredGemmBackendV1<RejectingProofProvider, MaliciousCandidateBackend>,
>;

fn semantic_backend(
    program: GemmSemanticProgramV1,
    request: &CompileRequestV1,
) -> (TestSemanticBackend, Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let proof_calls = Rc::new(Cell::new(0));
    let candidate_calls = Rc::new(Cell::new(0));
    let proof_gate = ProofRequiredGemmBackendV1::new(
        GemmProofRequirementsV1::new(request, Vec::new()).unwrap(),
        RejectingProofProvider {
            calls: Rc::clone(&proof_calls),
        },
        MaliciousCandidateBackend {
            calls: Rc::clone(&candidate_calls),
        },
    );
    (
        GemmSemanticCheckingBackendV1::new(program, proof_gate),
        proof_calls,
        candidate_calls,
    )
}

#[test]
fn valid_structured_kir_reaches_proof_gate_but_not_malicious_candidate_backend() {
    let kir = GeneralGemmKirV1::canonical(plan());
    let request = bound_request(1, SnapshotIdentityV1::from_untrusted_bytes([6; 32]), &kir);
    let program = GemmSemanticProgramV1::new(&request, kir).unwrap();
    assert_eq!(analyze_gemm_semantics_v1(&program), Ok(()));

    let (mut backend, proof_calls, candidate_calls) = semantic_backend(program, &request);
    let output = backend.compile_transaction(&request).unwrap();
    assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
    assert!(output.candidate().is_none());
    assert_eq!(proof_calls.get(), 1);
    assert_eq!(candidate_calls.get(), 0);
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

        let (mut backend, proof_calls, candidate_calls) = semantic_backend(program, &request);
        let output = backend.compile_transaction(&request).unwrap();
        assert_eq!(proof_calls.get(), 0);
        assert_eq!(candidate_calls.get(), 0);
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
    let (mut backend, proof_calls, candidate_calls) = semantic_backend(program, &first);
    let output = backend.compile_transaction(&second).unwrap();

    assert_eq!(proof_calls.get(), 0);
    assert_eq!(candidate_calls.get(), 0);
    assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
    assert!(output.snapshots().is_empty());
    assert!(output.receipts().is_empty());
    assert!(output.candidate().is_none());
    assert_eq!(output.diagnostics()[0].code().get(), 0x4647_0006);
    assert_eq!(output.diagnostics()[0].stage(), Some(CompilerStageV1::Mir));
}

#[test]
fn same_request_identity_cannot_hide_any_request_field_substitution() {
    let kir = GeneralGemmKirV1::canonical(plan());
    let snapshot_identity = SnapshotIdentityV1::from_untrusted_bytes([6; 32]);
    let original = bound_request(1, snapshot_identity, &kir);
    let program = GemmSemanticProgramV1::new(&original, kir).unwrap();

    for mutation in SAME_IDENTITY_REQUEST_MUTATIONS {
        let substituted = mutate_request_with_same_identity(&original, mutation);
        assert_eq!(substituted.identity(), original.identity());
        assert_ne!(substituted, original, "{mutation:?}");

        let (mut backend, proof_calls, candidate_calls) =
            semantic_backend(program.clone(), &original);
        let output = backend.compile_transaction(&substituted).unwrap();
        assert_eq!(proof_calls.get(), 0, "{mutation:?}");
        assert_eq!(candidate_calls.get(), 0, "{mutation:?}");
        assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
        assert!(output.snapshots().is_empty());
        assert!(output.receipts().is_empty());
        assert!(output.candidate().is_none());
        assert_eq!(output.diagnostics()[0].code().get(), 0x4647_0006);
        assert_eq!(output.diagnostics()[0].stage(), Some(CompilerStageV1::Mir));
    }
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
    let (mut backend, proof_calls, candidate_calls) =
        semantic_backend(invalid_program, &invalid_request);
    let output = backend.compile_transaction(&invalid_request).unwrap();
    assert_eq!(proof_calls.get(), 0);
    assert_eq!(candidate_calls.get(), 0);
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
