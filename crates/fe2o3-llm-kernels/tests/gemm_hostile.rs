use fe2o3_llm_kernels::gemm::*;

fn canonical_selection() -> Qwen3LinearSelectionV1 {
    Qwen3LinearSelectionV1 {
        role: Qwen3LinearRoleV1::Draft06B,
        mode: Qwen3LinearModeV1::Decode,
        bucket: Qwen3LinearBucketV1::DecodeS1C8192,
        operator: Qwen3B3OperatorV1::KeyProjection,
        layer: 0,
    }
}

fn canonical() -> Qwen3LinearCandidateV1 {
    exact_qwen3_linear_candidate_v1(canonical_selection()).unwrap()
}

fn assert_noncanonical(candidate: Qwen3LinearCandidateV1) {
    assert_eq!(
        validate_qwen3_linear_candidate_v1(&candidate, canonical_selection()),
        Err(Qwen3LinearErrorV1::NonCanonical)
    );
}

#[test]
fn adjacent_non_linear_b3_operators_are_explicitly_rejected() {
    for operator in [
        Qwen3B3OperatorV1::TokenEmbedding,
        Qwen3B3OperatorV1::InputRmsNorm,
        Qwen3B3OperatorV1::QueryRmsNorm,
        Qwen3B3OperatorV1::KeyRmsNorm,
        Qwen3B3OperatorV1::Rope,
        Qwen3B3OperatorV1::KvWrite,
        Qwen3B3OperatorV1::Attention,
        Qwen3B3OperatorV1::PostAttentionRmsNorm,
        Qwen3B3OperatorV1::SwiGlu,
        Qwen3B3OperatorV1::FinalRmsNorm,
        Qwen3B3OperatorV1::ArgmaxCompactCompletion,
    ] {
        let mut selection = canonical_selection();
        selection.operator = operator;
        assert_eq!(
            exact_qwen3_linear_candidate_v1(selection),
            Err(Qwen3LinearErrorV1::UnsupportedOperator(operator))
        );
    }
}

#[test]
fn adjacent_bucket_mode_and_layer_descriptors_are_rejected() {
    let mut selection = canonical_selection();
    selection.mode = Qwen3LinearModeV1::Prefill;
    assert_eq!(
        exact_qwen3_linear_candidate_v1(selection),
        Err(Qwen3LinearErrorV1::UnsupportedBucketMode)
    );

    let mut selection = canonical_selection();
    selection.layer = 28;
    assert_eq!(
        exact_qwen3_linear_candidate_v1(selection),
        Err(Qwen3LinearErrorV1::LayerOutOfBounds)
    );

    let mut selection = canonical_selection();
    selection.operator = Qwen3B3OperatorV1::LogitsProjection;
    assert_eq!(
        exact_qwen3_linear_candidate_v1(selection),
        Err(Qwen3LinearErrorV1::LogitsLayerMustBeAbsent)
    );
}

#[test]
fn family_schema_route_and_b3_identity_drift_are_rejected() {
    let mut mutated = canonical();
    mutated.family_id[0] ^= 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.candidate_schema_id[31] ^= 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.general_route_id[7] ^= 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.b3_source.commit = "f078ca3f37aeddab43b04e568831b1c7a1471204";
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.b3_source.tree = "21d048144b76548d5e3c79f15d09934206903fa3";
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.b3_source.graph_blob = "04ed32aca3275d6be4b3c01471b83db3432ab722";
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.b3_source.graph_source_sha256 =
        "076cd0714444ce13152bfa21e82b4b608542dfc4b43d557d790099db335d1b48";
    assert_noncanonical(mutated);
}

#[test]
fn selection_geometry_and_shape_drift_are_rejected() {
    let mut mutated = canonical();
    mutated.selection.role = Qwen3LinearRoleV1::Target8B;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.selection.mode = Qwen3LinearModeV1::Prefill;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.selection.bucket = Qwen3LinearBucketV1::DecodeS8C8192;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.selection.operator = Qwen3B3OperatorV1::ValueProjection;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.selection.layer = 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.geometry.layers += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.geometry.hidden += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.geometry.intermediate += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.geometry.query_heads -= 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.geometry.kv_heads -= 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.geometry.head_dimension -= 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.dimensions.m += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.dimensions.n += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.dimensions.k += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.dimensions.bucket.sequences = 2;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.dimensions.bucket.active_tokens = 2;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.dimensions.bucket.context_tokens = 4_096;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.implementation = Qwen3LinearImplementationV1::Gemm;
    assert_noncanonical(mutated);
}

#[test]
fn numerical_and_layout_field_drift_are_rejected() {
    let mut mutated = canonical();
    mutated.numerical.activation = Qwen3LinearScalarTypeV1::Fp32;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.numerical.weight = Qwen3LinearScalarTypeV1::Fp32;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.numerical.initial_output = Qwen3LinearScalarTypeV1::Bf16;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.numerical.accumulator = Qwen3LinearScalarTypeV1::Bf16;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.numerical.output = Qwen3LinearScalarTypeV1::Bf16;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.numerical.alpha_bits = 0.0_f32.to_bits();
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.numerical.beta_bits = 1.0_f32.to_bits();
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.numerical.residual_epilogue = true;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.numerical.fused_contraction = true;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.layout.activation = Qwen3LinearLayoutV1::PreparedLogicalKxNFromQwenRowMajorNxK;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.layout.weight = Qwen3LinearLayoutV1::RowMajorContiguous;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.layout.initial_output = Qwen3LinearLayoutV1::PreparedLogicalKxNFromQwenRowMajorNxK;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.layout.output = Qwen3LinearLayoutV1::PreparedLogicalKxNFromQwenRowMajorNxK;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.layout.strides[1] += 1;
    assert_noncanonical(mutated);
}

#[test]
fn effect_alias_and_tail_field_drift_are_rejected() {
    let mut mutated = canonical();
    mutated.effects.swap(0, 1);
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.effects[0].access = Qwen3LinearAccessV1::Write;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.effects[0].requires_initialized = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.effects[3].requires_exclusive_owner = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.alias.activation_weight_disjoint = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.alias.activation_output_disjoint = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.alias.weight_output_disjoint = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.alias.initial_output_disjoint = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.alias.output_coordinate_single_writer = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.tail_epilogue.guarded_a_tail = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.tail_epilogue.guarded_b_tail = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.tail_epilogue.zero_filled_k_tail = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.tail_epilogue.predicated_output_tail = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.tail_epilogue.accumulator_carries_all_phases = false;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.tail_epilogue.alpha_beta_epilogue = false;
    assert_noncanonical(mutated);
}

#[test]
fn resource_plan_obligation_and_selection_identity_drift_are_rejected() {
    let mut mutated = canonical();
    mutated.resources.storage_elements[0] += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.resources.storage_bytes[1] += 2;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.resources.block_counts[0] += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.resources.inert_grid_work_items[0] += 64;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.resources.workgroup_dimensions[0] = 32;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.resources.reduction_phases += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.resources.total_workgroups += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.resources.lds_bytes += 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.general_plan_identity[0] ^= 1;
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.obligations.swap(0, 1);
    assert_noncanonical(mutated);

    let mut mutated = canonical();
    mutated.selection_identity[31] ^= 1;
    assert_noncanonical(mutated);
}

#[test]
fn production_authority_promotion_is_rejected_field_by_field() {
    let mut candidates = Vec::new();
    let mut mutated = canonical();
    mutated.authority.attributed_source_authority = true;
    candidates.push(mutated);
    let mut mutated = canonical();
    mutated.authority.kernel_ir_authority = true;
    candidates.push(mutated);
    let mut mutated = canonical();
    mutated.authority.artifact_authority = true;
    candidates.push(mutated);
    let mut mutated = canonical();
    mutated.authority.load_authority = true;
    candidates.push(mutated);
    let mut mutated = canonical();
    mutated.authority.launch_authority = true;
    candidates.push(mutated);
    let mut mutated = canonical();
    mutated.authority.hardware_authority = true;
    candidates.push(mutated);
    let mut mutated = canonical();
    mutated.authority.performance_authority = true;
    candidates.push(mutated);
    let mut mutated = canonical();
    mutated.authority.machine_refinement = true;
    candidates.push(mutated);
    for candidate in candidates {
        assert_noncanonical(candidate);
    }
}

#[test]
fn expected_selection_substitution_and_weight_extent_fail_closed() {
    let candidate = canonical();
    let mut wrong = canonical_selection();
    wrong.operator = Qwen3B3OperatorV1::ValueProjection;
    assert_eq!(
        validate_qwen3_linear_candidate_v1(&candidate, wrong),
        Err(Qwen3LinearErrorV1::NonCanonical)
    );

    let expected = 1_024 * 1_024;
    assert_eq!(
        prepare_qwen_linear_weight_v1(&candidate, canonical_selection(), &vec![0; expected - 1],),
        Err(Qwen3LinearErrorV1::WeightSourceExtent {
            expected,
            actual: expected - 1,
        })
    );
}
