use fe2o3_kernel_ir::{GeneralGemmKirV1, GeneralGemmPlanFieldsV1, GeneralGemmPlanSnapshotV1};
use fe2o3_verifier::{
    GENERAL_GEMM_NUMERICAL_CORRESPONDENCE_SCHEMA_V1,
    GENERAL_GEMM_NUMERICAL_DIFFERENTIAL_FIXTURE_COUNT_V1, GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1,
    GeneralGemmEvidenceIdentityV1, GeneralGemmFutureMachineRefinementInputV1,
    GeneralGemmKirModelCorrespondenceV1, GeneralGemmNumericalCorrespondenceBasisV1,
    GeneralGemmNumericalCorrespondenceClaimV1, GeneralGemmNumericalCorrespondenceErrorV1,
    GeneralGemmNumericalCorrespondenceFieldV1, GeneralGemmNumericalCorrespondenceStatusV1,
    GeneralGemmNumericalPropertyV1, GeneralGemmProofRequestV1, GeneralGemmProofScheduleV1,
    GeneralGemmVerusRuntimeClosureLeaseV2, check_general_gemm_kir_model_correspondence_v1,
    check_general_gemm_numerical_correspondence_v1,
    derive_general_gemm_kir_model_correspondence_claim_v1,
    derive_general_gemm_numerical_correspondence_claim_v1,
    execute_general_gemm_numerical_correspondence_with_runtime_closure_v1,
    reviewed_general_gemm_numerical_property_theorem_manifest_v1,
};

const POSITIVE_SOURCE: &str = include_str!("../verus/general_gemm_numerical_contract_v1.rs");
const SCHEDULE_MODEL_SOURCE: &str = include_str!("../verus/general_gemm_schedule_model_v1.rs");
const WIDENING_NEGATIVE: &str =
    include_str!("../verus/negative/general_gemm_numerical_widening_wrong.rs");
const KIR_REFINEMENT_NEGATIVE: &str =
    include_str!("../verus/negative/general_gemm_numerical_kir_refinement_claim_wrong.rs");
const MFMA_NEGATIVE: &str =
    include_str!("../verus/negative/general_gemm_numerical_mfma_claim_wrong.rs");
const MFMA_DESCRIPTOR_NEGATIVE: &str =
    include_str!("../verus/negative/general_gemm_numerical_mfma_descriptor_claim_wrong.rs");
const ORDER_NEGATIVE: &str =
    include_str!("../verus/negative/general_gemm_numerical_order_claim_wrong.rs");
const MANIFEST: &str = include_str!("../verus/pins/GENERAL_GEMM_RUNTIME_CLOSURE_V2.manifest");
const PROPERTY_MANIFEST: &str =
    include_str!("../verus/pins/GENERAL_GEMM_NUMERICAL_PROPERTIES_V1.manifest");

fn identity(seed: u8) -> GeneralGemmEvidenceIdentityV1 {
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([seed; 32])
}

fn flip(identity: GeneralGemmEvidenceIdentityV1) -> GeneralGemmEvidenceIdentityV1 {
    let mut bytes = *identity.as_bytes();
    bytes[0] ^= 0x80;
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(bytes)
}

fn plan() -> GeneralGemmPlanFieldsV1 {
    GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [33, 35, 19],
        strides: [23, 41, 43],
        storage_elements: [755, 773, 1411],
        block_counts: [3, 3, 1],
        aql_grid_work_items: [192, 3, 1],
        reduction_phases: 2,
        alpha_bits: 1.25_f32.to_bits(),
        beta_bits: (-0.5_f32).to_bits(),
    })
    .unwrap()
}

fn request(schedule: GeneralGemmProofScheduleV1) -> GeneralGemmProofRequestV1 {
    GeneralGemmProofRequestV1::checked(
        schedule,
        identity(1),
        identity(2),
        identity(3),
        identity(4),
        identity(5),
        identity(6),
        identity(7),
        identity(8),
        identity(9),
        identity(10),
        identity(11),
    )
    .unwrap()
}

fn correspondence(schedule: GeneralGemmProofScheduleV1) -> GeneralGemmKirModelCorrespondenceV1 {
    let kir = GeneralGemmKirV1::canonical(plan());
    let proof_request = request(schedule);
    let claim = derive_general_gemm_kir_model_correspondence_claim_v1(&kir, proof_request).unwrap();
    check_general_gemm_kir_model_correspondence_v1(&kir, proof_request, claim).unwrap()
}

fn mismatch_field(
    result: Result<
        fe2o3_verifier::GeneralGemmNumericalCorrespondenceV1,
        GeneralGemmNumericalCorrespondenceErrorV1,
    >,
) -> GeneralGemmNumericalCorrespondenceFieldV1 {
    match result.unwrap_err() {
        GeneralGemmNumericalCorrespondenceErrorV1::FieldMismatch(field) => field,
        other => panic!("expected field mismatch, got {other:?}"),
    }
}

type ClaimMutation = fn(&mut GeneralGemmNumericalCorrespondenceClaimV1);

#[test]
fn both_schedules_bind_kir_model_target_tool_and_honest_property_status() {
    for schedule in [
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ] {
        let correspondence = correspondence(schedule);
        let claim = derive_general_gemm_numerical_correspondence_claim_v1(&correspondence).unwrap();
        assert_eq!(claim.schedule, schedule);
        assert_eq!(claim.kir_identity, correspondence.claim().kir_identity);
        assert_eq!(
            claim.numerical_policy_identity,
            correspondence.proof_request().numerical_policy_identity()
        );
        assert_eq!(
            claim.target_identity,
            correspondence.proof_request().target_identity()
        );
        assert_eq!(
            claim.compiler_toolchain_identity,
            correspondence.proof_request().toolchain_identity()
        );
        assert_eq!(claim.mfma_contract.matrix_shape, [16, 16, 16]);
        assert_eq!(claim.mfma_contract.wave_lanes, 64);
        assert_eq!(claim.mfma_contract.control_immediates, [0; 3]);
        assert_eq!(
            claim.mfma_contract.numerical_status,
            GeneralGemmNumericalCorrespondenceStatusV1::Contracted
        );
        assert_ne!(claim.exhaustive_bf16_identity.as_bytes(), &[0; 32]);
        assert_ne!(claim.differential_fixture_identity.as_bytes(), &[0; 32]);
        assert_eq!(GENERAL_GEMM_NUMERICAL_DIFFERENTIAL_FIXTURE_COUNT_V1, 11);
        assert_eq!(
            claim.machine_refinement_join.required_inputs,
            [
                GeneralGemmFutureMachineRefinementInputV1::OwnerBoundPlironGraph,
                GeneralGemmFutureMachineRefinementInputV1::DirectLlvmWorkerRequestResponse,
                GeneralGemmFutureMachineRefinementInputV1::FinalizerPostLinkIsaResult,
            ]
        );
        assert_eq!(
            claim.machine_refinement_join.status,
            GeneralGemmNumericalCorrespondenceStatusV1::Unsupported
        );
        assert!(
            !claim
                .machine_refinement_join
                .has_all_required_input_identities()
        );

        let checked =
            check_general_gemm_numerical_correspondence_v1(correspondence, claim).unwrap();
        assert_eq!(checked.claim(), claim);
        assert!(!checked.can_enter_compiler_proof_gate());
        assert!(!checked.grants_artifact_or_runtime_authority());
    }

    let claim = derive_general_gemm_numerical_correspondence_claim_v1(&correspondence(
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
    ))
    .unwrap();
    let statuses: Vec<_> = claim
        .properties
        .iter()
        .map(|fact| (fact.property, fact.status, fact.basis))
        .collect();
    assert_eq!(statuses.len(), GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1);
    assert_eq!(
        statuses,
        vec![
            (
                GeneralGemmNumericalPropertyV1::ExactBf16ToF32EncodingWidening,
                GeneralGemmNumericalCorrespondenceStatusV1::Proved,
                GeneralGemmNumericalCorrespondenceBasisV1::VerusBf16EncodingTheorem
            ),
            (
                GeneralGemmNumericalPropertyV1::Bf16RustKirRefinement,
                GeneralGemmNumericalCorrespondenceStatusV1::Unsupported,
                GeneralGemmNumericalCorrespondenceBasisV1::OpenRustKirRefinement
            ),
            (
                GeneralGemmNumericalPropertyV1::Bf16IeeeValueInterpretation,
                GeneralGemmNumericalCorrespondenceStatusV1::Contracted,
                GeneralGemmNumericalCorrespondenceBasisV1::Ieee754Binary32Contract
            ),
            (
                GeneralGemmNumericalPropertyV1::Fp32MultiplyRoundToNearestTiesEven,
                GeneralGemmNumericalCorrespondenceStatusV1::Contracted,
                GeneralGemmNumericalCorrespondenceBasisV1::Ieee754Binary32Contract
            ),
            (
                GeneralGemmNumericalPropertyV1::Fp32AddRoundToNearestTiesEven,
                GeneralGemmNumericalCorrespondenceStatusV1::Contracted,
                GeneralGemmNumericalCorrespondenceBasisV1::Ieee754Binary32Contract
            ),
            (
                GeneralGemmNumericalPropertyV1::IncreasingKSeparateMulAddOrder,
                GeneralGemmNumericalCorrespondenceStatusV1::ModelOnly,
                GeneralGemmNumericalCorrespondenceBasisV1::ExactRealScheduleModel
            ),
            (
                GeneralGemmNumericalPropertyV1::SeparateAlphaBetaEpilogueOrder,
                GeneralGemmNumericalCorrespondenceStatusV1::ModelOnly,
                GeneralGemmNumericalCorrespondenceBasisV1::ExactRealScheduleModel
            ),
            (
                GeneralGemmNumericalPropertyV1::Gfx942MfmaShapeAndControls,
                GeneralGemmNumericalCorrespondenceStatusV1::Contracted,
                GeneralGemmNumericalCorrespondenceBasisV1::Gfx942MfmaInstructionContract
            ),
            (
                GeneralGemmNumericalPropertyV1::Gfx942MfmaFp32Accumulation,
                GeneralGemmNumericalCorrespondenceStatusV1::Contracted,
                GeneralGemmNumericalCorrespondenceBasisV1::Gfx942MfmaInstructionContract
            ),
            (
                GeneralGemmNumericalPropertyV1::ExceptionalAndSubnormalValues,
                GeneralGemmNumericalCorrespondenceStatusV1::Unsupported,
                GeneralGemmNumericalCorrespondenceBasisV1::FiniteNormalOrZeroPolicy
            ),
            (
                GeneralGemmNumericalPropertyV1::EmittedMachineNumericalRefinement,
                GeneralGemmNumericalCorrespondenceStatusV1::Unsupported,
                GeneralGemmNumericalCorrespondenceBasisV1::FutureGraphWorkerFinalizerJoin
            ),
        ]
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|(_, status, _)| {
                *status == GeneralGemmNumericalCorrespondenceStatusV1::Proved
            })
            .count(),
        1
    );

    let theorem_manifest = reviewed_general_gemm_numerical_property_theorem_manifest_v1().unwrap();
    assert_eq!(
        claim.property_theorem_manifest_identity,
        theorem_manifest.identity()
    );
    assert_eq!(
        claim.numerical_theorem_set_identity,
        theorem_manifest.theorem_set_identity()
    );
    for (index, binding) in theorem_manifest.bindings().iter().copied().enumerate() {
        assert_eq!(binding.fact(), claim.properties[index]);
        assert_eq!(
            binding.statement_source_identity(),
            claim.property_theorem_binding_identities[index]
        );
        assert!(!binding.theorem_name().is_empty());
        assert!(!binding.statement().is_empty());
        assert_ne!(binding.source_identity().as_bytes(), &[0; 32]);
        assert_ne!(binding.statement_identity().as_bytes(), &[0; 32]);
        assert_ne!(binding.record_identity().as_bytes(), &[0; 32]);
    }
}

#[test]
fn every_top_level_claim_field_substitution_fails_closed() {
    use GeneralGemmNumericalCorrespondenceFieldV1 as Field;
    let exact = derive_general_gemm_numerical_correspondence_claim_v1(&correspondence(
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
    ))
    .unwrap();
    let cases: [(Field, ClaimMutation); 25] = [
        (Field::SchemaIdentity, |claim| {
            claim.schema_identity = flip(claim.schema_identity)
        }),
        (Field::KirCorrespondenceIdentity, |claim| {
            claim.kir_correspondence_identity = flip(claim.kir_correspondence_identity)
        }),
        (Field::KirIdentity, |claim| {
            claim.kir_identity = flip(claim.kir_identity)
        }),
        (Field::ProofRequestIdentity, |claim| {
            claim.proof_request_identity = flip(claim.proof_request_identity)
        }),
        (Field::Schedule, |claim| {
            claim.schedule = GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
        }),
        (Field::ScheduleIdentity, |claim| {
            claim.schedule_identity = flip(claim.schedule_identity)
        }),
        (Field::NumericalPolicyIdentity, |claim| {
            claim.numerical_policy_identity = flip(claim.numerical_policy_identity)
        }),
        (Field::TargetIdentity, |claim| {
            claim.target_identity = flip(claim.target_identity)
        }),
        (Field::CompilerToolchainIdentity, |claim| {
            claim.compiler_toolchain_identity = flip(claim.compiler_toolchain_identity)
        }),
        (Field::ScheduleModelIdentity, |claim| {
            claim.schedule_model_identity = flip(claim.schedule_model_identity)
        }),
        (Field::ScheduleTheoremSetIdentity, |claim| {
            claim.schedule_theorem_set_identity = flip(claim.schedule_theorem_set_identity)
        }),
        (Field::ScheduleSourceClosureIdentity, |claim| {
            claim.schedule_source_closure_identity = flip(claim.schedule_source_closure_identity)
        }),
        (Field::NumericalSourceIdentity, |claim| {
            claim.numerical_source_identity = flip(claim.numerical_source_identity)
        }),
        (Field::PropertyTheoremManifestIdentity, |claim| {
            claim.property_theorem_manifest_identity =
                flip(claim.property_theorem_manifest_identity)
        }),
        (Field::PropertyTheoremBindingIdentities, |claim| {
            claim.property_theorem_binding_identities[0] =
                flip(claim.property_theorem_binding_identities[0])
        }),
        (Field::NumericalTheoremSetIdentity, |claim| {
            claim.numerical_theorem_set_identity = flip(claim.numerical_theorem_set_identity)
        }),
        (Field::NumericalSourceClosureIdentity, |claim| {
            claim.numerical_source_closure_identity = flip(claim.numerical_source_closure_identity)
        }),
        (Field::ReviewedVerusToolIdentity, |claim| {
            claim.reviewed_verus_tool_identity = flip(claim.reviewed_verus_tool_identity)
        }),
        (Field::ExhaustiveBf16Identity, |claim| {
            claim.exhaustive_bf16_identity = flip(claim.exhaustive_bf16_identity)
        }),
        (Field::DifferentialFixtureIdentity, |claim| {
            claim.differential_fixture_identity = flip(claim.differential_fixture_identity)
        }),
        (Field::MfmaContractIdentity, |claim| {
            claim.mfma_contract_identity = flip(claim.mfma_contract_identity)
        }),
        (Field::MfmaContract, |claim| {
            claim.mfma_contract.matrix_shape[0] ^= 1
        }),
        (Field::MachineRefinementJoinIdentity, |claim| {
            claim.machine_refinement_join_identity = flip(claim.machine_refinement_join_identity)
        }),
        (Field::MachineRefinementJoin, |claim| {
            claim
                .machine_refinement_join
                .owner_bound_pliron_graph_identity = Some(identity(72))
        }),
        (Field::Properties, |claim| {
            claim.properties[0].status = GeneralGemmNumericalCorrespondenceStatusV1::Contracted
        }),
    ];

    for (field, mutate) in cases {
        let mut hostile = exact;
        mutate(&mut hostile);
        assert_eq!(
            mismatch_field(check_general_gemm_numerical_correspondence_v1(
                correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1),
                hostile,
            )),
            field
        );
    }
}

#[test]
fn every_mfma_contract_and_property_subfield_substitution_fails_closed() {
    let exact = derive_general_gemm_numerical_correspondence_claim_v1(&correspondence(
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
    ))
    .unwrap();
    let mfma_mutations: [ClaimMutation; 10] = [
        |claim| claim.mfma_contract.target = "gfx942:xnack+:wave64",
        |claim| claim.mfma_contract.llvm_intrinsic = "llvm.amdgcn.mfma.wrong",
        |claim| claim.mfma_contract.isa_mnemonic = "v_mfma_wrong",
        |claim| claim.mfma_contract.matrix_shape[2] ^= 1,
        |claim| claim.mfma_contract.wave_lanes ^= 1,
        |claim| claim.mfma_contract.accumulators_per_lane ^= 1,
        |claim| claim.mfma_contract.control_immediates[0] ^= 1,
        |claim| claim.mfma_contract.input_element_bits ^= 1,
        |claim| claim.mfma_contract.accumulator_element_bits ^= 1,
        |claim| {
            claim.mfma_contract.numerical_status =
                GeneralGemmNumericalCorrespondenceStatusV1::Proved
        },
    ];
    for mutate in mfma_mutations {
        let mut hostile = exact;
        mutate(&mut hostile);
        assert_eq!(
            mismatch_field(check_general_gemm_numerical_correspondence_v1(
                correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1),
                hostile,
            )),
            GeneralGemmNumericalCorrespondenceFieldV1::MfmaContract
        );
    }

    for index in 0..GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1 {
        let mut hostile = exact;
        hostile.property_theorem_binding_identities[index] =
            flip(hostile.property_theorem_binding_identities[index]);
        assert_eq!(
            mismatch_field(check_general_gemm_numerical_correspondence_v1(
                correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1),
                hostile,
            )),
            GeneralGemmNumericalCorrespondenceFieldV1::PropertyTheoremBindingIdentities
        );
    }

    let machine_mutations: [ClaimMutation; 7] = [
        |claim| {
            claim.machine_refinement_join.required_inputs[0] =
                GeneralGemmFutureMachineRefinementInputV1::FinalizerPostLinkIsaResult
        },
        |claim| {
            claim.machine_refinement_join.required_inputs[1] =
                GeneralGemmFutureMachineRefinementInputV1::OwnerBoundPlironGraph
        },
        |claim| {
            claim.machine_refinement_join.required_inputs[2] =
                GeneralGemmFutureMachineRefinementInputV1::DirectLlvmWorkerRequestResponse
        },
        |claim| {
            claim
                .machine_refinement_join
                .owner_bound_pliron_graph_identity = Some(identity(81))
        },
        |claim| {
            claim
                .machine_refinement_join
                .direct_llvm_worker_request_response_identity = Some(identity(82))
        },
        |claim| {
            claim
                .machine_refinement_join
                .finalizer_post_link_isa_result_identity = Some(identity(83))
        },
        |claim| {
            claim.machine_refinement_join.status =
                GeneralGemmNumericalCorrespondenceStatusV1::Contracted
        },
    ];
    for mutate in machine_mutations {
        let mut hostile = exact;
        mutate(&mut hostile);
        assert_eq!(
            mismatch_field(check_general_gemm_numerical_correspondence_v1(
                correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1),
                hostile,
            )),
            GeneralGemmNumericalCorrespondenceFieldV1::MachineRefinementJoin
        );
    }

    for index in 0..GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1 {
        for mutate in 0..3 {
            let mut hostile = exact;
            match mutate {
                0 => {
                    hostile.properties[index].property =
                        GeneralGemmNumericalPropertyV1::EmittedMachineNumericalRefinement
                }
                1 => {
                    hostile.properties[index].status =
                        GeneralGemmNumericalCorrespondenceStatusV1::Contracted
                }
                _ => {
                    hostile.properties[index].basis =
                        GeneralGemmNumericalCorrespondenceBasisV1::Ieee754Binary32Contract
                }
            }
            if hostile.properties[index] == exact.properties[index] {
                hostile.properties[index] = exact.properties[(index + 1) % exact.properties.len()];
            }
            assert_eq!(
                mismatch_field(check_general_gemm_numerical_correspondence_v1(
                    correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1),
                    hostile,
                )),
                GeneralGemmNumericalCorrespondenceFieldV1::Properties
            );
        }
    }
}

#[test]
fn retained_sources_are_exact_pinned_and_have_no_trusted_escape() {
    let tokens = [
        POSITIVE_SOURCE,
        SCHEDULE_MODEL_SOURCE,
        WIDENING_NEGATIVE,
        KIR_REFINEMENT_NEGATIVE,
        ORDER_NEGATIVE,
        MFMA_DESCRIPTOR_NEGATIVE,
        MFMA_NEGATIVE,
    ]
    .into_iter()
    .flat_map(|source| {
        source.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
    })
    .collect::<Vec<_>>();
    for forbidden in ["unsafe", "assume", "admit", "axiom", "external_body"] {
        assert!(
            !tokens.contains(&forbidden),
            "trusted escape `{forbidden}` entered source closure"
        );
    }
    for required in [
        "every_bf16_encoding_widens_without_losing_bits_v1",
        "non_bf16_bit_placement_claims_remain_open_v1",
    ] {
        assert!(POSITIVE_SOURCE.contains(required));
    }
    for rejected_tautology in [
        "accumulation_step_preserves_separate_mul_add_order_v1",
        "epilogue_uses_two_separate_multiplications_then_addition_v1",
        "gfx942_mfma_descriptor_has_reviewed_shape_v1",
    ] {
        assert!(!POSITIVE_SOURCE.contains(rejected_tautology));
    }
    for path in [
        "proof/general_gemm_numerical_contract_v1.rs",
        "proof/negative/general_gemm_numerical_kir_refinement_claim_wrong.rs",
        "proof/negative/general_gemm_numerical_mfma_claim_wrong.rs",
        "proof/negative/general_gemm_numerical_mfma_descriptor_claim_wrong.rs",
        "proof/negative/general_gemm_numerical_order_claim_wrong.rs",
        "proof/negative/general_gemm_numerical_widening_wrong.rs",
        "proof/pins/GENERAL_GEMM_NUMERICAL_PROPERTIES_V1.manifest",
    ] {
        assert_eq!(MANIFEST.matches(path).count(), 1, "manifest pin for {path}");
    }
    assert_eq!(PROPERTY_MANIFEST.matches("|proved|").count(), 1);
    assert_eq!(PROPERTY_MANIFEST.matches("|model-only|").count(), 2);
    assert!(PROPERTY_MANIFEST.contains("open-rust-kir-refinement"));
    assert!(PROPERTY_MANIFEST.contains("future-graph-worker-finalizer-join"));
    assert!(GENERAL_GEMM_NUMERICAL_CORRESPONDENCE_SCHEMA_V1.ends_with(".v1"));
}

#[test]
#[ignore = "requires an independently provisioned root-owned runtime closure beneath /opt"]
fn root_owned_retained_runtime_executes_exact_numerical_suite() {
    let root = std::env::var_os("FE2O3_GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT")
        .expect("set the audited root-owned runtime closure root");
    let runtime = GeneralGemmVerusRuntimeClosureLeaseV2::open(root).unwrap();
    let kir_model = correspondence(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
    let claim = derive_general_gemm_numerical_correspondence_claim_v1(&kir_model).unwrap();
    let numerical = check_general_gemm_numerical_correspondence_v1(kir_model, claim).unwrap();
    let evidence = execute_general_gemm_numerical_correspondence_with_runtime_closure_v1(
        numerical, &runtime, 120,
    )
    .unwrap();
    assert!(!evidence.can_enter_compiler_proof_gate());
    assert_eq!(evidence.positive_output().stdout_bytes(), 44);
    assert_eq!(evidence.positive_output().stderr_bytes(), 0);
    assert_eq!(evidence.negative_outputs().len(), 5);
    assert_ne!(evidence.runtime_closure_identity().as_bytes(), &[0; 32]);
}
