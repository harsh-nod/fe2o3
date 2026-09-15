use fe2o3_qwen3_gqa_prefill_v1::{
    B3PrefillBucketV1, Bf16V1, GqaArithmeticStageV1, GqaInputV1, GqaPrefillProfileDescriptorV1,
    GqaReferenceErrorV1, GqaTensorV1, GqaVectorCoordinateV1, Qwen3AttentionRoleV1,
    ValidatedGqaPrefillProfileV1, gqa_kv_index_v1, gqa_prefill_f64_vector_oracle_v1,
    gqa_prefill_reference_v1, gqa_prefill_reference_vector_v1, validate_gqa_prefill_profile_v1,
};

fn bf16(value: f32) -> Bf16V1 {
    Bf16V1::from_f32_rne(value).unwrap()
}

fn profile(role: Qwen3AttentionRoleV1) -> ValidatedGqaPrefillProfileV1 {
    validate_gqa_prefill_profile_v1(GqaPrefillProfileDescriptorV1::canonical(
        role,
        B3PrefillBucketV1::S1T128,
    ))
    .unwrap()
}

fn zeros(profile: ValidatedGqaPrefillProfileV1) -> (Vec<Bf16V1>, Vec<Bf16V1>, Vec<Bf16V1>) {
    (
        vec![Bf16V1::default(); profile.resources().query_elements as usize],
        vec![Bf16V1::default(); profile.resources().kv_elements_each as usize],
        vec![Bf16V1::default(); profile.resources().kv_elements_each as usize],
    )
}

fn deterministic_values(length: usize, seed: u32) -> Vec<Bf16V1> {
    let mut state = seed;
    (0..length)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let signed = ((state >> 8) % 1_025) as i32 - 512;
            bf16(signed as f32 / 1_024.0)
        })
        .collect()
}

#[test]
fn quotient_gqa_heads_share_the_exact_kv_values() {
    for role in [
        Qwen3AttentionRoleV1::Target8B,
        Qwen3AttentionRoleV1::Draft06B,
    ] {
        let profile = profile(role);
        let (query, key, mut value) = zeros(profile);
        let descriptor = profile.descriptor();
        for sequence in 0..descriptor.sequences {
            for token in 0..descriptor.active_tokens {
                for kv_head in 0..descriptor.geometry.kv_heads {
                    for feature in 0..descriptor.geometry.head_dimension {
                        let index =
                            gqa_kv_index_v1(profile, sequence, token, kv_head, feature).unwrap();
                        value[index] = bf16((kv_head + 1) as f32 / 8.0);
                    }
                }
            }
        }
        for query_head in 0..descriptor.geometry.query_heads {
            let mut output = vec![bf16(-9.0); descriptor.geometry.head_dimension];
            gqa_prefill_reference_vector_v1(
                profile,
                GqaInputV1 {
                    query: &query,
                    key: &key,
                    value: &value,
                },
                GqaVectorCoordinateV1 {
                    sequence: 0,
                    query_token: 127,
                    query_head,
                },
                &mut output,
            )
            .unwrap();
            let kv_head = query_head / descriptor.geometry.query_heads_per_kv_head;
            let expected = bf16((kv_head + 1) as f32 / 8.0);
            assert!(output.into_iter().all(|actual| actual == expected));
        }
    }
}

#[test]
fn vector_reference_agrees_with_independent_f64_oracle() {
    for (case, role) in [
        Qwen3AttentionRoleV1::Target8B,
        Qwen3AttentionRoleV1::Draft06B,
    ]
    .into_iter()
    .enumerate()
    {
        let profile = profile(role);
        let query = deterministic_values(
            profile.resources().query_elements as usize,
            17 + case as u32,
        );
        let key = deterministic_values(
            profile.resources().kv_elements_each as usize,
            71 + case as u32,
        );
        let value = deterministic_values(
            profile.resources().kv_elements_each as usize,
            131 + case as u32,
        );
        let heads = profile.descriptor().geometry.query_heads;
        for (query_token, query_head) in [(0, 0), (31, heads / 2), (127, heads - 1)] {
            let oracle = gqa_prefill_f64_vector_oracle_v1(
                profile,
                GqaInputV1 {
                    query: &query,
                    key: &key,
                    value: &value,
                },
                GqaVectorCoordinateV1 {
                    sequence: 0,
                    query_token,
                    query_head,
                },
            )
            .unwrap();
            let mut output = vec![bf16(9.0); profile.descriptor().geometry.head_dimension];
            let denominator = gqa_prefill_reference_vector_v1(
                profile,
                GqaInputV1 {
                    query: &query,
                    key: &key,
                    value: &value,
                },
                GqaVectorCoordinateV1 {
                    sequence: 0,
                    query_token,
                    query_head,
                },
                &mut output,
            )
            .unwrap();
            assert!(denominator.is_finite() && denominator >= 1.0);
            for (feature, actual) in output.into_iter().enumerate() {
                let actual = f64::from(actual.to_f32());
                let expected = oracle.output[feature];
                let allowance = 0.012_f64.max(expected.abs() * 0.02);
                assert!(
                    (actual - expected).abs() <= allowance,
                    "{role:?} token {query_token} head {query_head} feature {feature}: expected {expected}, actual {actual}"
                );
            }
        }
    }
}

#[test]
fn future_key_and_value_mutations_do_not_affect_a_causal_vector() {
    let profile = profile(Qwen3AttentionRoleV1::Draft06B);
    let query = deterministic_values(profile.resources().query_elements as usize, 29);
    let key = deterministic_values(profile.resources().kv_elements_each as usize, 37);
    let value = deterministic_values(profile.resources().kv_elements_each as usize, 43);
    let mut baseline = vec![Bf16V1::default(); 128];
    gqa_prefill_reference_vector_v1(
        profile,
        GqaInputV1 {
            query: &query,
            key: &key,
            value: &value,
        },
        GqaVectorCoordinateV1 {
            sequence: 0,
            query_token: 5,
            query_head: 7,
        },
        &mut baseline,
    )
    .unwrap();

    let mut mutated_key = key.clone();
    let mut mutated_value = value.clone();
    let geometry = profile.descriptor().geometry;
    for token in 6..profile.descriptor().active_tokens {
        for kv_head in 0..geometry.kv_heads {
            for feature in 0..geometry.head_dimension {
                let index = gqa_kv_index_v1(profile, 0, token, kv_head, feature).unwrap();
                mutated_key[index] = bf16(4.0);
                mutated_value[index] = bf16(-4.0);
            }
        }
    }
    let mut observed = vec![Bf16V1::default(); 128];
    gqa_prefill_reference_vector_v1(
        profile,
        GqaInputV1 {
            query: &query,
            key: &mutated_key,
            value: &mutated_value,
        },
        GqaVectorCoordinateV1 {
            sequence: 0,
            query_token: 5,
            query_head: 7,
        },
        &mut observed,
    )
    .unwrap();
    assert_eq!(observed, baseline);
}

#[test]
fn complete_draft_reference_stages_and_commits_exact_constant_output() {
    let profile = profile(Qwen3AttentionRoleV1::Draft06B);
    let (query, key, mut value) = zeros(profile);
    value.fill(bf16(0.5));
    let mut output = vec![bf16(-9.0); profile.resources().output_elements as usize];
    let state = gqa_prefill_reference_v1(
        profile,
        GqaInputV1 {
            query: &query,
            key: &key,
            value: &value,
        },
        &mut output,
    )
    .unwrap();
    assert_eq!(state.output_vectors, 128 * 16);
    assert!(state.minimum_denominator >= 1.0);
    assert!(state.maximum_denominator <= 128.0);
    assert!(output.into_iter().all(|value| value == bf16(0.5)));
}

#[test]
fn preflight_and_arithmetic_failures_leave_output_unchanged() {
    let profile = profile(Qwen3AttentionRoleV1::Draft06B);
    let (query, key, value) = zeros(profile);
    let sentinel = vec![Bf16V1::from_bits(0x4242); 128];
    let mut output = sentinel.clone();
    assert_eq!(
        gqa_prefill_reference_vector_v1(
            profile,
            GqaInputV1 {
                query: &query[..query.len() - 1],
                key: &key,
                value: &value,
            },
            GqaVectorCoordinateV1 {
                sequence: 0,
                query_token: 0,
                query_head: 0,
            },
            &mut output,
        ),
        Err(GqaReferenceErrorV1::WrongLength {
            tensor: GqaTensorV1::Query,
            expected: query.len(),
            actual: query.len() - 1,
        })
    );
    assert_eq!(output, sentinel);

    let mut hostile_key = key.clone();
    hostile_key[17] = Bf16V1::from_bits(0x7f80);
    let mut output = sentinel.clone();
    assert_eq!(
        gqa_prefill_reference_vector_v1(
            profile,
            GqaInputV1 {
                query: &query,
                key: &hostile_key,
                value: &value,
            },
            GqaVectorCoordinateV1 {
                sequence: 0,
                query_token: 0,
                query_head: 0,
            },
            &mut output,
        ),
        Err(GqaReferenceErrorV1::NonFiniteInput {
            tensor: GqaTensorV1::Key,
            index: 17,
        })
    );
    assert_eq!(output, sentinel);

    let huge_query = vec![Bf16V1::from_bits(0x7f7f); query.len()];
    let huge_key = vec![Bf16V1::from_bits(0x7f7f); key.len()];
    let mut output = sentinel.clone();
    assert_eq!(
        gqa_prefill_reference_vector_v1(
            profile,
            GqaInputV1 {
                query: &huge_query,
                key: &huge_key,
                value: &value,
            },
            GqaVectorCoordinateV1 {
                sequence: 0,
                query_token: 0,
                query_head: 0,
            },
            &mut output,
        ),
        Err(GqaReferenceErrorV1::NonFiniteIntermediate {
            sequence: 0,
            query_token: 0,
            query_head: 0,
            key_token: Some(0),
            feature: Some(0),
            stage: GqaArithmeticStageV1::QkProduct,
        })
    );
    assert_eq!(output, sentinel);
}

#[test]
fn bf16_storage_cast_is_rne_and_rejects_nonfinite_sources() {
    assert_eq!(Bf16V1::from_f32_rne(1.0).unwrap().to_bits(), 0x3f80);
    assert_eq!(Bf16V1::from_f32_rne(-0.0).unwrap().to_bits(), 0x8000);
    assert_eq!(
        Bf16V1::from_f32_rne(f32::from_bits(0x3f80_8000))
            .unwrap()
            .to_bits(),
        0x3f80
    );
    assert_eq!(
        Bf16V1::from_f32_rne(f32::from_bits(0x3f81_8000))
            .unwrap()
            .to_bits(),
        0x3f82
    );
    assert!(Bf16V1::from_f32_rne(f32::INFINITY).is_err());
    assert!(Bf16V1::from_f32_rne(f32::NAN).is_err());
}
