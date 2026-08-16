use fe2o3_kernel_ir::{
    TargetCapability, Wave64CollectiveKindV1, Wave64CollectivesProfileV1, Wave64CollectivesV1Error,
    Wave64F32PolicyV1, Wave64OutputOwnershipV1, Wave64ParticipationV1, WaveWidth,
    wave64_collectives_v1_kernel_ir,
};
use fe2o3_wave64_collectives_v1::{
    WAVE64_COLLECTIVES_V1_KIR_SCHEMA_SHA256, WAVE64_LANES_V1, WAVE64_REFINEMENT_BOUNDARY_V1,
    Wave64RefinementErrorV1, Wave64SemanticOutputV1, exact_wave64_refinement_identities_v1,
    source_contributor_mask_v1, verify_wave64_source_model_to_kir_v1,
};

fn exact_profile() -> Wave64CollectivesProfileV1 {
    Wave64CollectivesProfileV1::exact_gfx942_xnack_minus_cov6()
}

fn corpus() -> [f32; WAVE64_LANES_V1] {
    core::array::from_fn(|lane| ((lane * 37 + 11) % 127) as f32 - 63.0)
}

fn prefix_mask(end: usize) -> u64 {
    match end {
        0 => 0,
        WAVE64_LANES_V1.. => u64::MAX,
        _ => (1_u64 << end) - 1,
    }
}

#[test]
fn exact_checked_in_identities_profile_and_semantics_are_admitted() {
    let identities = exact_wave64_refinement_identities_v1();
    assert_eq!(
        identities.attributed_source_sha256,
        fe2o3_kernel_ir::WAVE64_COLLECTIVES_V1_SOURCE_SHA256
    );
    assert_eq!(
        identities.kernel_ir_schema_sha256,
        WAVE64_COLLECTIVES_V1_KIR_SCHEMA_SHA256
    );
    let receipt = verify_wave64_source_model_to_kir_v1(
        &corpus(),
        0x8000_0042_8000_0021,
        &wave64_collectives_v1_kernel_ir(),
        &exact_profile(),
        identities,
    )
    .unwrap();
    assert_eq!(receipt.identities(), identities);
    assert_eq!(receipt.active_mask(), 0x8000_0042_8000_0021);
    assert_eq!(receipt.active_lanes(), 6);
    assert_eq!(receipt.checked_symbolic_relations(), 3 * 64);
}

#[test]
fn symbolic_contributor_sets_cover_every_lane_and_every_u64_mask() {
    for lane in 0..WAVE64_LANES_V1 {
        assert_eq!(
            source_contributor_mask_v1(Wave64SemanticOutputV1::Reduction, lane),
            u64::MAX
        );
        assert_eq!(
            source_contributor_mask_v1(Wave64SemanticOutputV1::Inclusive, lane),
            prefix_mask(lane + 1)
        );
        assert_eq!(
            source_contributor_mask_v1(Wave64SemanticOutputV1::Exclusive, lane),
            prefix_mask(lane)
        );
    }
    for output in [
        Wave64SemanticOutputV1::Reduction,
        Wave64SemanticOutputV1::Inclusive,
        Wave64SemanticOutputV1::Exclusive,
    ] {
        assert_eq!(source_contributor_mask_v1(output, 64), 0);
        assert_eq!(source_contributor_mask_v1(output, usize::MAX), 0);
    }
}

#[test]
fn deterministic_mask_corpus_checks_empty_singletons_prefixes_and_hostile_patterns() {
    let mut masks = vec![
        0,
        u64::MAX,
        0xaaaa_aaaa_aaaa_aaaa,
        0x5555_5555_5555_5555,
        0x8000_0000_0000_0001,
    ];
    for lane in 0..WAVE64_LANES_V1 {
        masks.push(1_u64 << lane);
        masks.push(!(1_u64 << lane));
    }
    for end in 0..=WAVE64_LANES_V1 {
        masks.push(prefix_mask(end));
        masks.push(!prefix_mask(end));
    }
    let mut state = 0xd1b5_4a32_d192_ed03_u64;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        masks.push(state);
    }

    let input = corpus();
    let ir = wave64_collectives_v1_kernel_ir();
    let profile = exact_profile();
    let identities = exact_wave64_refinement_identities_v1();
    for mask in masks {
        let receipt = verify_wave64_source_model_to_kir_v1(&input, mask, &ir, &profile, identities)
            .unwrap_or_else(|error| panic!("mask {mask:#018x} rejected: {error}"));
        assert_eq!(receipt.active_lanes(), mask.count_ones());
    }
}

#[test]
fn finite_f32_endpoint_and_signed_zero_vectors_refine_by_exact_integer_value() {
    let vectors = [
        [0.0; WAVE64_LANES_V1],
        [-0.0; WAVE64_LANES_V1],
        core::array::from_fn(|lane| if lane % 2 == 0 { -1024.0 } else { 1024.0 }),
        core::array::from_fn(|lane| lane as f32 - 32.0),
    ];
    let masks = [0, 1, 1_u64 << 63, 0x8421_8421_8421_8421, u64::MAX];
    for input in vectors {
        for mask in masks {
            verify_wave64_source_model_to_kir_v1(
                &input,
                mask,
                &wave64_collectives_v1_kernel_ir(),
                &exact_profile(),
                exact_wave64_refinement_identities_v1(),
            )
            .unwrap();
        }
    }
}

#[test]
fn source_and_kernel_ir_schema_identity_mutations_fail_closed() {
    let mut wrong_source = exact_wave64_refinement_identities_v1();
    wrong_source.attributed_source_sha256[0] ^= 1;
    assert_eq!(
        verify_wave64_source_model_to_kir_v1(
            &corpus(),
            u64::MAX,
            &wave64_collectives_v1_kernel_ir(),
            &exact_profile(),
            wrong_source,
        ),
        Err(Wave64RefinementErrorV1::SelectedSourceIdentity)
    );

    let mut wrong_schema = exact_wave64_refinement_identities_v1();
    wrong_schema.kernel_ir_schema_sha256[31] ^= 1;
    assert_eq!(
        verify_wave64_source_model_to_kir_v1(
            &corpus(),
            u64::MAX,
            &wave64_collectives_v1_kernel_ir(),
            &exact_profile(),
            wrong_schema,
        ),
        Err(Wave64RefinementErrorV1::SelectedKernelIrSchemaIdentity)
    );
}

#[test]
fn profile_target_wave_mask_policy_and_source_mutations_fail_closed() {
    type Mutation = fn(&mut Wave64CollectivesProfileV1);
    let mutations: [Mutation; 6] = [
        |profile| profile.source_sha256[0] ^= 1,
        |profile| {
            profile.target = TargetCapability::Extension {
                namespace: "amdgpu".into(),
                name: "gfx942:xnack+".into(),
            }
        },
        |profile| profile.wave_width = WaveWidth::Wave32,
        |profile| profile.grid = [2, 1, 1],
        |profile| profile.f32_policy = Wave64F32PolicyV1::FiniteOnly,
        |profile| profile.descriptor.complete_kernarg_bytes -= 1,
    ];
    for mutate in mutations {
        let mut profile = exact_profile();
        mutate(&mut profile);
        assert!(matches!(
            verify_wave64_source_model_to_kir_v1(
                &corpus(),
                u64::MAX,
                &wave64_collectives_v1_kernel_ir(),
                &profile,
                exact_wave64_refinement_identities_v1(),
            ),
            Err(Wave64RefinementErrorV1::NonCanonicalKernelIr(
                Wave64CollectivesV1Error::UnsupportedProfile
            ))
        ));
    }
}

#[test]
fn collective_order_kind_participation_and_ownership_mutations_fail_closed() {
    let mut mutations = Vec::new();

    let mut wrong_kind = wave64_collectives_v1_kernel_ir();
    wrong_kind.collectives[0].kind = Wave64CollectiveKindV1::InclusiveScanSum;
    mutations.push(wrong_kind);

    let mut wrong_order = wave64_collectives_v1_kernel_ir();
    wrong_order.collectives.swap(0, 1);
    mutations.push(wrong_order);

    let mut wrong_participation = wave64_collectives_v1_kernel_ir();
    wrong_participation.collectives[2].participation =
        Wave64ParticipationV1::DivergentLogicalParticipants;
    mutations.push(wrong_participation);

    let mut wrong_owner = wave64_collectives_v1_kernel_ir();
    wrong_owner.outputs[1].ownership = Wave64OutputOwnershipV1::LaneZeroOwnsEveryIndex;
    mutations.push(wrong_owner);

    let mut wrong_output_source = wave64_collectives_v1_kernel_ir();
    wrong_output_source.outputs[2].source = Wave64CollectiveKindV1::InclusiveScanSum;
    mutations.push(wrong_output_source);

    let mut missing_output = wave64_collectives_v1_kernel_ir();
    missing_output.outputs.pop();
    mutations.push(missing_output);

    for ir in mutations {
        assert!(matches!(
            verify_wave64_source_model_to_kir_v1(
                &corpus(),
                u64::MAX,
                &ir,
                &exact_profile(),
                exact_wave64_refinement_identities_v1(),
            ),
            Err(Wave64RefinementErrorV1::NonCanonicalKernelIr(
                Wave64CollectivesV1Error::NonCanonicalKernelIr
            ))
        ));
    }
}

#[test]
fn nonfinite_fractional_and_out_of_range_inputs_are_rejected() {
    for (lane, value) in [(0, f32::NAN), (17, f32::INFINITY), (31, 0.5), (63, 1025.0)] {
        let mut input = corpus();
        input[lane] = value;
        assert!(matches!(
            verify_wave64_source_model_to_kir_v1(
                &input,
                u64::MAX,
                &wave64_collectives_v1_kernel_ir(),
                &exact_profile(),
                exact_wave64_refinement_identities_v1(),
            ),
            Err(Wave64RefinementErrorV1::SourceModel(_))
        ));
    }
}

#[test]
fn receipt_and_documented_boundary_grant_no_adjacent_authority() {
    let receipt = verify_wave64_source_model_to_kir_v1(
        &corpus(),
        u64::MAX,
        &wave64_collectives_v1_kernel_ir(),
        &exact_profile(),
        exact_wave64_refinement_identities_v1(),
    )
    .unwrap();
    assert!(!receipt.proves_source_to_model_refinement());
    assert!(!receipt.proves_compiler_causality());
    assert!(!receipt.proves_llvm_or_isa_refinement());
    assert!(!receipt.proves_active_zero_sign_refinement());
    assert!(!receipt.grants_protected_execution());
    assert!(!receipt.proves_generalized_safety());
    assert!(!receipt.grants_parity_promotion());
    for boundary in [
        "active zero sign is abstracted",
        "no source-to-model proof",
        "no compiler causality",
        "no LLVM/ISA refinement",
        "no artifact",
        "protected-execution",
        "generalized-safety",
        "parity authority",
    ] {
        assert!(WAVE64_REFINEMENT_BOUNDARY_V1.contains(boundary));
    }
}
