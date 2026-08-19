use fe2o3_amd_target::AmdTargetId;
use fe2o3_compiler_api::{
    CompileLimitsV1, CompileRequestV1, CompilerProfileIdentityV1, KernelInstanceIdentityV1,
    ObligationSetIdentityV1, PipelineConfigurationIdentityV1, PipelineSelectorV1,
    RequestIdentityV1, SnapshotFormatIdentityV1, SnapshotIdentityV1, StageSnapshotV1,
    TargetProfileIdentityV1,
};
use fe2o3_compiler_driver::{
    GEMM_REQUIRED_SAFETY_PROPERTIES_V1, GemmProofDiagnosticV1, GemmSafetyPropertyV1,
};
use fe2o3_tiled_gemm_v1::contract::TARGET_V1;
use fe2o3_tiled_gemm_v1::{
    GEMM_REQUIRED_PROPERTIES_V1, GemmRequiredPropertyV1, GeneralGemmCompilerBindingErrorV1,
    GeneralGemmCompilerProfilesV1, GeneralGemmRequestV1, GeneralLaunchLimitsV1, admit_target_v1,
    bind_general_gemm_compiler_request_v1, plan_general_gemm_v1, validate_gemm_property_schema_v1,
    validate_general_gemm_compiler_request_v1,
};

fn plan(request: GeneralGemmRequestV1) -> fe2o3_tiled_gemm_v1::GeneralGemmPlanV1 {
    plan_general_gemm_v1(
        admit_target_v1(AmdTargetId::parse(TARGET_V1).unwrap()).unwrap(),
        request,
        GeneralLaunchLimitsV1::representable(),
    )
    .unwrap()
}

fn request(
    dimensions: [u32; 3],
    strides: [u32; 3],
    coefficients: [f32; 2],
) -> GeneralGemmRequestV1 {
    GeneralGemmRequestV1::new(
        dimensions[0],
        dimensions[1],
        dimensions[2],
        strides[0],
        strides[1],
        strides[2],
        coefficients[0],
        coefficients[1],
    )
}

fn profiles() -> GeneralGemmCompilerProfilesV1 {
    GeneralGemmCompilerProfilesV1::new(
        CompilerProfileIdentityV1::from_untrusted_bytes([0x11; 32]),
        TargetProfileIdentityV1::from_untrusted_bytes([0x22; 32]),
        PipelineConfigurationIdentityV1::from_untrusted_bytes([0x33; 32]),
        PipelineSelectorV1::PlironShadow,
    )
}

#[test]
fn mirror_matches_every_actual_compiler_property_code_and_stage() {
    validate_gemm_property_schema_v1().unwrap();
    assert_eq!(
        GEMM_REQUIRED_PROPERTIES_V1.len(),
        GEMM_REQUIRED_SAFETY_PROPERTIES_V1.len()
    );
    for (mirrored, compiler) in GEMM_REQUIRED_PROPERTIES_V1
        .into_iter()
        .zip(GEMM_REQUIRED_SAFETY_PROPERTIES_V1)
    {
        assert_eq!(mirrored.as_str(), compiler.as_str());
        assert_eq!(mirrored.diagnostic_code(), diagnostic(compiler).code());
        assert_eq!(stage(mirrored), compiler.verification_stage() as u8);
    }
}

fn diagnostic(property: GemmSafetyPropertyV1) -> GemmProofDiagnosticV1 {
    match property {
        GemmSafetyPropertyV1::MemorySafe => GemmProofDiagnosticV1::MemorySafe,
        GemmSafetyPropertyV1::BoundsSafe => GemmProofDiagnosticV1::BoundsSafe,
        GemmSafetyPropertyV1::Initialized => GemmProofDiagnosticV1::Initialized,
        GemmSafetyPropertyV1::RaceFree => GemmProofDiagnosticV1::RaceFree,
        GemmSafetyPropertyV1::BarrierConvergent => GemmProofDiagnosticV1::BarrierConvergent,
        GemmSafetyPropertyV1::OutputRegionInjective => GemmProofDiagnosticV1::OutputRegionInjective,
        GemmSafetyPropertyV1::LdsEpochCorrect => GemmProofDiagnosticV1::LdsEpochCorrect,
        GemmSafetyPropertyV1::AccumulatorPhaseRefinement => {
            GemmProofDiagnosticV1::AccumulatorPhaseRefinement
        }
        GemmSafetyPropertyV1::TailRefinement => GemmProofDiagnosticV1::TailRefinement,
        GemmSafetyPropertyV1::EpilogueRefinement => GemmProofDiagnosticV1::EpilogueRefinement,
        GemmSafetyPropertyV1::NumericalContract => GemmProofDiagnosticV1::NumericalContract,
        GemmSafetyPropertyV1::MachineRefinementBoundary => {
            GemmProofDiagnosticV1::MachineRefinementBoundary
        }
    }
}

fn stage(property: GemmRequiredPropertyV1) -> u8 {
    match property {
        GemmRequiredPropertyV1::MemorySafe
        | GemmRequiredPropertyV1::Initialized
        | GemmRequiredPropertyV1::RaceFree
        | GemmRequiredPropertyV1::BarrierConvergent
        | GemmRequiredPropertyV1::LdsEpochCorrect => 6,
        GemmRequiredPropertyV1::BoundsSafe | GemmRequiredPropertyV1::OutputRegionInjective => 5,
        GemmRequiredPropertyV1::AccumulatorPhaseRefinement
        | GemmRequiredPropertyV1::TailRefinement
        | GemmRequiredPropertyV1::EpilogueRefinement
        | GemmRequiredPropertyV1::NumericalContract => 3,
        GemmRequiredPropertyV1::MachineRefinementBoundary => 7,
    }
}

#[test]
fn exact_plan_and_caller_profiles_are_bound_into_an_inert_request() {
    let plan = plan(request([17, 19, 18], [23, 29, 31], [2.0, -1.0]));
    let profiles = profiles();
    let binding =
        bind_general_gemm_compiler_request_v1(&plan, profiles, CompileLimitsV1::default()).unwrap();
    binding.validate(&plan).unwrap();

    let compiler_request = binding.request();
    assert_eq!(
        compiler_request.compiler_profile_identity(),
        profiles.compiler()
    );
    assert_eq!(
        compiler_request.target_profile_identity(),
        profiles.target()
    );
    assert_eq!(
        compiler_request.pipeline_configuration_identity(),
        profiles.pipeline()
    );
    assert_eq!(compiler_request.selector(), profiles.selector());
    assert!(
        compiler_request
            .input()
            .canonical_bytes()
            .ends_with(&plan.encode_canonical())
    );

    let identities = [
        *compiler_request.identity().as_bytes(),
        *compiler_request.kernel_instance_identity().as_bytes(),
        *compiler_request.input_obligations_identity().as_bytes(),
        *compiler_request.input().identity().as_bytes(),
        *compiler_request.input().format_identity().as_bytes(),
    ];
    for left in 0..identities.len() {
        for right in left + 1..identities.len() {
            assert_ne!(identities[left], identities[right]);
        }
    }
}

#[test]
fn every_caller_profile_identity_and_route_changes_the_request_binding() {
    let plan = plan(request([17, 19, 18], [23, 29, 31], [2.0, -1.0]));
    let base = bind_general_gemm_compiler_request_v1(&plan, profiles(), CompileLimitsV1::default())
        .unwrap();
    let variants = [
        GeneralGemmCompilerProfilesV1::new(
            CompilerProfileIdentityV1::from_untrusted_bytes([0x12; 32]),
            profiles().target(),
            profiles().pipeline(),
            profiles().selector(),
        ),
        GeneralGemmCompilerProfilesV1::new(
            profiles().compiler(),
            TargetProfileIdentityV1::from_untrusted_bytes([0x23; 32]),
            profiles().pipeline(),
            profiles().selector(),
        ),
        GeneralGemmCompilerProfilesV1::new(
            profiles().compiler(),
            profiles().target(),
            PipelineConfigurationIdentityV1::from_untrusted_bytes([0x34; 32]),
            profiles().selector(),
        ),
        GeneralGemmCompilerProfilesV1::new(
            profiles().compiler(),
            profiles().target(),
            profiles().pipeline(),
            PipelineSelectorV1::PlironV1,
        ),
    ];
    for variant in variants {
        let changed =
            bind_general_gemm_compiler_request_v1(&plan, variant, CompileLimitsV1::default())
                .unwrap();
        assert_ne!(changed.request().identity(), base.request().identity());
    }
}

#[test]
fn every_problem_field_and_derived_launch_boundary_changes_the_binding() {
    let base = request([17, 19, 18], [23, 29, 31], [2.0, -1.0]);
    let base_plan = plan(base);
    let base_binding =
        bind_general_gemm_compiler_request_v1(&base_plan, profiles(), CompileLimitsV1::default())
            .unwrap();
    let base_identity = base_binding.request().identity();

    let mutations = [
        request([18, 19, 18], [23, 29, 31], [2.0, -1.0]),
        request([17, 20, 18], [23, 29, 31], [2.0, -1.0]),
        request([17, 19, 19], [23, 29, 31], [2.0, -1.0]),
        request([17, 19, 18], [24, 29, 31], [2.0, -1.0]),
        request([17, 19, 18], [23, 30, 31], [2.0, -1.0]),
        request([17, 19, 18], [23, 29, 32], [2.0, -1.0]),
        request([17, 19, 18], [23, 29, 31], [3.0, -1.0]),
        request([17, 19, 18], [23, 29, 31], [2.0, 1.0]),
    ];
    for mutation in mutations {
        let mutated_plan = plan(mutation);
        let binding = bind_general_gemm_compiler_request_v1(
            &mutated_plan,
            profiles(),
            CompileLimitsV1::default(),
        )
        .unwrap();
        assert_ne!(binding.request().identity(), base_identity);
        assert_ne!(
            binding.request().input().identity(),
            base_binding.request().input().identity()
        );
    }

    let one_tile = plan(request([16, 16, 16], [16, 16, 16], [1.0, 0.0]));
    let two_by_two = plan(request([17, 17, 16], [16, 17, 17], [1.0, 0.0]));
    assert_ne!(one_tile.block_counts(), two_by_two.block_counts());
    assert_ne!(
        one_tile.aql_grid_work_items(),
        two_by_two.aql_grid_work_items()
    );
    let one_binding =
        bind_general_gemm_compiler_request_v1(&one_tile, profiles(), CompileLimitsV1::default())
            .unwrap();
    let two_binding =
        bind_general_gemm_compiler_request_v1(&two_by_two, profiles(), CompileLimitsV1::default())
            .unwrap();
    assert_ne!(
        one_binding.request().identity(),
        two_binding.request().identity()
    );
}

fn rebuild(
    original: &CompileRequestV1,
    identity: RequestIdentityV1,
    kernel: KernelInstanceIdentityV1,
    compiler: CompilerProfileIdentityV1,
    obligations: ObligationSetIdentityV1,
    input: StageSnapshotV1,
) -> CompileRequestV1 {
    CompileRequestV1::new(
        identity,
        kernel,
        compiler,
        original.target_profile_identity(),
        original.pipeline_configuration_identity(),
        obligations,
        original.selector(),
        input,
        original.limits(),
    )
    .unwrap()
}

#[test]
fn malformed_and_substituted_plan_snapshot_and_request_identities_fail_closed() {
    let base_plan = plan(request([17, 19, 18], [23, 29, 31], [2.0, -1.0]));
    let other_plan = plan(request([18, 19, 18], [23, 29, 31], [2.0, -1.0]));
    let binding =
        bind_general_gemm_compiler_request_v1(&base_plan, profiles(), CompileLimitsV1::default())
            .unwrap();
    let other_binding =
        bind_general_gemm_compiler_request_v1(&other_plan, profiles(), CompileLimitsV1::default())
            .unwrap();
    let original = binding.request();

    assert_eq!(
        validate_general_gemm_compiler_request_v1(&base_plan, other_binding.request()),
        Err(GeneralGemmCompilerBindingErrorV1::FrontendPayloadMismatch)
    );

    let bad_request_identity = rebuild(
        original,
        RequestIdentityV1::from_untrusted_bytes([0x80; 32]),
        original.kernel_instance_identity(),
        original.compiler_profile_identity(),
        original.input_obligations_identity(),
        original.input().clone(),
    );
    assert_eq!(
        validate_general_gemm_compiler_request_v1(&base_plan, &bad_request_identity),
        Err(GeneralGemmCompilerBindingErrorV1::RequestIdentityMismatch)
    );

    let bad_snapshot = StageSnapshotV1::new(
        original.input().stage(),
        SnapshotIdentityV1::from_untrusted_bytes([0x81; 32]),
        original.input().format_identity(),
        original.input().canonical_bytes().to_vec(),
    )
    .unwrap();
    let bad_snapshot_request = rebuild(
        original,
        original.identity(),
        original.kernel_instance_identity(),
        original.compiler_profile_identity(),
        original.input_obligations_identity(),
        bad_snapshot,
    );
    assert_eq!(
        validate_general_gemm_compiler_request_v1(&base_plan, &bad_snapshot_request),
        Err(GeneralGemmCompilerBindingErrorV1::SnapshotIdentityMismatch)
    );

    let bad_kernel = rebuild(
        original,
        original.identity(),
        KernelInstanceIdentityV1::from_untrusted_bytes([0x82; 32]),
        original.compiler_profile_identity(),
        original.input_obligations_identity(),
        original.input().clone(),
    );
    assert_eq!(
        validate_general_gemm_compiler_request_v1(&base_plan, &bad_kernel),
        Err(GeneralGemmCompilerBindingErrorV1::KernelInstanceIdentityMismatch)
    );

    let bad_obligations = rebuild(
        original,
        original.identity(),
        original.kernel_instance_identity(),
        original.compiler_profile_identity(),
        ObligationSetIdentityV1::from_untrusted_bytes([0x83; 32]),
        original.input().clone(),
    );
    assert_eq!(
        validate_general_gemm_compiler_request_v1(&base_plan, &bad_obligations),
        Err(GeneralGemmCompilerBindingErrorV1::ObligationSetIdentityMismatch)
    );

    let substituted_compiler = rebuild(
        original,
        original.identity(),
        original.kernel_instance_identity(),
        CompilerProfileIdentityV1::from_untrusted_bytes([0x84; 32]),
        original.input_obligations_identity(),
        original.input().clone(),
    );
    assert_eq!(
        validate_general_gemm_compiler_request_v1(&base_plan, &substituted_compiler),
        Err(GeneralGemmCompilerBindingErrorV1::RequestIdentityMismatch)
    );

    let mut malformed_payload = original.input().canonical_bytes().to_vec();
    let last = malformed_payload.len() - 1;
    malformed_payload[last] ^= 1;
    let malformed_snapshot = StageSnapshotV1::new(
        original.input().stage(),
        original.input().identity(),
        original.input().format_identity(),
        malformed_payload,
    )
    .unwrap();
    let malformed_plan_request = rebuild(
        original,
        original.identity(),
        original.kernel_instance_identity(),
        original.compiler_profile_identity(),
        original.input_obligations_identity(),
        malformed_snapshot,
    );
    assert_eq!(
        validate_general_gemm_compiler_request_v1(&base_plan, &malformed_plan_request),
        Err(GeneralGemmCompilerBindingErrorV1::FrontendPayloadMismatch)
    );

    let wrong_format = StageSnapshotV1::new(
        original.input().stage(),
        original.input().identity(),
        SnapshotFormatIdentityV1::from_untrusted_bytes([0x85; 32]),
        original.input().canonical_bytes().to_vec(),
    )
    .unwrap();
    let wrong_format_request = rebuild(
        original,
        original.identity(),
        original.kernel_instance_identity(),
        original.compiler_profile_identity(),
        original.input_obligations_identity(),
        wrong_format,
    );
    assert_eq!(
        validate_general_gemm_compiler_request_v1(&base_plan, &wrong_format_request),
        Err(GeneralGemmCompilerBindingErrorV1::SnapshotFormatIdentityMismatch)
    );
}

#[test]
fn caller_snapshot_limit_rejects_the_binding_before_publication() {
    let plan = plan(request([17, 19, 18], [23, 29, 31], [2.0, -1.0]));
    let limits = CompileLimitsV1::new(1, 1, 1, 1, 1, 1).unwrap();
    assert!(matches!(
        bind_general_gemm_compiler_request_v1(&plan, profiles(), limits),
        Err(GeneralGemmCompilerBindingErrorV1::Request(_))
    ));
}
