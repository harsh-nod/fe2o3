use fe2o3_qwen3_swiglu_v1::{
    B3SwiGluBucketV1, Bf16V1, Qwen3ModelRoleV1, SwiGluArithmeticStageV1, SwiGluBufferBindingV1,
    SwiGluBufferV1, SwiGluCandidateDescriptorV1, SwiGluProfileDescriptorV1, SwiGluReferenceErrorV1,
    SwiGluScheduleDescriptorV1, swiglu_f64_oracle_v1, swiglu_reference_v1,
    validate_swiglu_candidate_v1, validate_swiglu_profile_v1,
};

fn candidate(role: Qwen3ModelRoleV1) -> fe2o3_qwen3_swiglu_v1::ValidatedSwiGluCandidateV1 {
    let profile = SwiGluProfileDescriptorV1::canonical(role, B3SwiGluBucketV1::DecodeS1);
    let bytes = u64::try_from(
        validate_swiglu_profile_v1(profile)
            .unwrap()
            .resources()
            .bytes_per_buffer,
    )
    .unwrap();
    validate_swiglu_candidate_v1(SwiGluCandidateDescriptorV1 {
        profile,
        gate: SwiGluBufferBindingV1 {
            allocation_id: 1,
            generation: 1,
            byte_offset: 0,
            byte_len: bytes,
        },
        up: SwiGluBufferBindingV1 {
            allocation_id: 2,
            generation: 1,
            byte_offset: 0,
            byte_len: bytes,
        },
        activated: SwiGluBufferBindingV1 {
            allocation_id: 3,
            generation: 1,
            byte_offset: 0,
            byte_len: bytes,
        },
        schedule: SwiGluScheduleDescriptorV1::canonical(),
    })
    .unwrap()
}

fn bf16(value: f32) -> Bf16V1 {
    Bf16V1::from_f32_rne(value).unwrap()
}

#[test]
fn target_and_draft_match_the_independent_f64_oracle() {
    for role in [Qwen3ModelRoleV1::Target8B, Qwen3ModelRoleV1::Draft06B] {
        let candidate = candidate(role);
        let elements = candidate.profile().resources().elements;
        let samples = [-8.0_f32, -2.0, -0.5, 0.0, 0.5, 2.0, 8.0];
        let gate: Vec<_> = (0..elements)
            .map(|index| bf16(samples[index % samples.len()]))
            .collect();
        let up: Vec<_> = (0..elements)
            .map(|index| bf16(samples[(index * 3 + 1) % samples.len()]))
            .collect();
        let mut output = vec![Bf16V1::from_bits(0x3f80); elements];
        let state = swiglu_reference_v1(candidate, &gate, &up, &mut output).unwrap();
        let oracle = swiglu_f64_oracle_v1(candidate, &gate, &up).unwrap();
        assert_eq!(state.elements, elements);
        for (actual, expected) in output.iter().zip(oracle.activated.iter()) {
            let actual = f64::from(actual.to_f32());
            let tolerance = 0.008 * expected.abs().max(1.0);
            assert!((actual - expected).abs() <= tolerance);
        }
    }
}

#[test]
fn exact_simple_values_have_stable_bf16_results() {
    let candidate = candidate(Qwen3ModelRoleV1::Draft06B);
    let elements = candidate.profile().resources().elements;
    let gate = vec![bf16(0.0); elements];
    let up = vec![bf16(3.0); elements];
    let mut output = vec![bf16(9.0); elements];
    swiglu_reference_v1(candidate, &gate, &up, &mut output).unwrap();
    assert!(
        output
            .iter()
            .all(|value| value.to_bits() == bf16(0.0).to_bits())
    );

    let gate = vec![bf16(1.0); elements];
    let up = vec![bf16(2.0); elements];
    swiglu_reference_v1(candidate, &gate, &up, &mut output).unwrap();
    let expected = bf16(2.0 / (1.0 + (-1.0_f32).exp()));
    assert!(
        output
            .iter()
            .all(|value| value.to_bits() == expected.to_bits())
    );
}

#[test]
fn extreme_negative_gate_uses_stable_branch() {
    let candidate = candidate(Qwen3ModelRoleV1::Draft06B);
    let elements = candidate.profile().resources().elements;
    let gate = vec![Bf16V1::from_bits(0xff7f); elements];
    let up = vec![bf16(1.0); elements];
    let mut output = vec![bf16(7.0); elements];
    swiglu_reference_v1(candidate, &gate, &up, &mut output).unwrap();
    assert!(output.iter().all(|value| value.to_f32() == 0.0));
}

#[test]
fn all_preflight_errors_leave_output_unchanged() {
    let candidate = candidate(Qwen3ModelRoleV1::Draft06B);
    let elements = candidate.profile().resources().elements;
    let gate = vec![bf16(1.0); elements];
    let up = vec![bf16(2.0); elements];
    let initial = vec![Bf16V1::from_bits(0x4242); elements];

    let mut output = initial.clone();
    assert_eq!(
        swiglu_reference_v1(candidate, &gate[..elements - 1], &up, &mut output),
        Err(SwiGluReferenceErrorV1::WrongLength {
            buffer: SwiGluBufferV1::Gate,
            expected: elements,
            actual: elements - 1,
        })
    );
    assert_eq!(output, initial);

    let mut nonfinite = gate.clone();
    nonfinite[elements / 2] = Bf16V1::from_bits(0x7f80);
    assert_eq!(
        swiglu_reference_v1(candidate, &nonfinite, &up, &mut output),
        Err(SwiGluReferenceErrorV1::NonFiniteInput {
            buffer: SwiGluBufferV1::Gate,
            index: elements / 2,
        })
    );
    assert_eq!(output, initial);
}

#[test]
fn intermediate_overflow_is_fail_closed_and_transactional() {
    let candidate = candidate(Qwen3ModelRoleV1::Draft06B);
    let elements = candidate.profile().resources().elements;
    let gate = vec![Bf16V1::from_bits(0x7f7f); elements];
    let up = vec![Bf16V1::from_bits(0x7f7f); elements];
    let initial = vec![Bf16V1::from_bits(0x3f80); elements];
    let mut output = initial.clone();
    assert_eq!(
        swiglu_reference_v1(candidate, &gate, &up, &mut output),
        Err(SwiGluReferenceErrorV1::NonFiniteIntermediate {
            index: 0,
            stage: SwiGluArithmeticStageV1::Product,
        })
    );
    assert_eq!(output, initial);
}
