// Test-only inputs and an independent CPU oracle. Nothing in this fixture is
// executable, compiler, artifact, proof, or launch authority.
use fe2o3_verifier::{
    SCALAR_GEMM_COVERAGE_PROFILE_V1, SCALAR_GEMM_F32_NUMERICAL_CONTRACT_V1,
    SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1, SCALAR_GEMM_MAX_GRID_THREADS_V1,
    SCALAR_GEMM_ROOT_SYMBOL_V1, SCALAR_GEMM_TARGET_V1, ScalarGemmBufferRegionV1,
    ScalarGemmHostAdmissionErrorV1, ScalarGemmHostRequestV1, ScalarGemmModelErrorV1,
    ScalarGemmShapeV1, ScalarGemmToolchainV1, admit_scalar_gemm_host_v1,
    evaluate_scalar_gemm_abstract_invocation_v1, scalar_gemm_accesses_are_in_bounds_v1,
    scalar_gemm_complete_launch_initializes_output_v1, scalar_gemm_f32_oracle_v1,
    scalar_gemm_flattened_index_is_correct_v1, scalar_gemm_output_initialized_by_invocation_v1,
    scalar_gemm_writers_are_injective_v1,
};

const ROOT: [&str; 1] = [SCALAR_GEMM_ROOT_SYMBOL_V1];

fn region(allocation_id: u64, byte_len: usize) -> ScalarGemmBufferRegionV1 {
    ScalarGemmBufferRegionV1::new(
        allocation_id,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        allocation_id as usize * 0x1_0000,
        byte_len,
        0,
        byte_len,
    )
}

#[inline(never)]
fn fixture_separate_f32_mul(left: f32, right: f32) -> f32 {
    std::hint::black_box(std::hint::black_box(left) * std::hint::black_box(right))
}

#[inline(never)]
fn fixture_separate_f32_add(left: f32, right: f32) -> f32 {
    std::hint::black_box(std::hint::black_box(left) + std::hint::black_box(right))
}

fn valid_request<'a>(
    shape: ScalarGemmShapeV1,
    roots: &'a [&'a str],
    calls: &'a [&'a str],
) -> ScalarGemmHostRequestV1<'a> {
    ScalarGemmHostRequestV1 {
        m: shape.m(),
        n: shape.n(),
        k: shape.k(),
        a_len: shape.a_len(),
        b_len: shape.b_len(),
        c_len: shape.c_len(),
        a_region: region(1, shape.a_bytes()),
        b_region: region(2, shape.b_bytes()),
        c_region: region(3, shape.c_bytes()),
        declared_target: SCALAR_GEMM_TARGET_V1,
        declared_coverage_profile: SCALAR_GEMM_COVERAGE_PROFILE_V1,
        declared_root_symbols: roots,
        declared_called_symbols: calls,
        declared_toolchain: ScalarGemmToolchainV1::UpstreamLlvmLld,
    }
}

#[test]
fn admits_exact_declared_preconditions_without_attesting_artifact() {
    let shape = ScalarGemmShapeV1::checked(2, 3, 4).unwrap();
    let admission = admit_scalar_gemm_host_v1(valid_request(shape, &ROOT, &[])).unwrap();
    assert_eq!(admission.shape(), shape);
    assert!(!admission.grants_compiler_authority());
    assert!(!admission.proves_gpu_execution());
    assert!(!admission.proves_source_to_machine_refinement());
    assert!(!admission.attests_emitted_hsaco_target());
    assert!(!admission.attests_actual_toolchain_execution());
    assert!(!admission.attests_launch_domain_authentication());
    assert!(!admission.attests_allocation_provenance());
    assert!(!admission.proves_physical_non_aliasing());
}

#[test]
fn checked_dimensions_reject_byte_overflow() {
    assert_eq!(
        ScalarGemmShapeV1::checked(u32::MAX, u32::MAX, 1),
        Err(ScalarGemmHostAdmissionErrorV1::ByteCountOverflow { field: "C" })
    );
    assert_eq!(
        ScalarGemmShapeV1::checked(u32::MAX, 1, u32::MAX),
        Err(ScalarGemmHostAdmissionErrorV1::ByteCountOverflow { field: "A" })
    );
    assert_eq!(
        ScalarGemmShapeV1::checked(1, u32::MAX, u32::MAX),
        Err(ScalarGemmHostAdmissionErrorV1::ByteCountOverflow { field: "B" })
    );
}

#[test]
fn rejects_every_length_mismatch() {
    let shape = ScalarGemmShapeV1::checked(2, 3, 4).unwrap();
    for field in ["A", "B", "C"] {
        let mut request = valid_request(shape, &ROOT, &[]);
        match field {
            "A" => request.a_len -= 1,
            "B" => request.b_len -= 1,
            "C" => request.c_len -= 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            admit_scalar_gemm_host_v1(request),
            Err(ScalarGemmHostAdmissionErrorV1::LengthMismatch { field: observed, .. })
                if observed == field
        ));
    }
}

#[test]
fn rejects_region_mismatch_and_endpoint_overflow() {
    let shape = ScalarGemmShapeV1::checked(2, 3, 4).unwrap();
    let mut short = valid_request(shape, &ROOT, &[]);
    short.c_region = region(3, shape.c_bytes() - 1);
    assert!(matches!(
        admit_scalar_gemm_host_v1(short),
        Err(ScalarGemmHostAdmissionErrorV1::RegionLengthMismatch { field: "C", .. })
    ));

    let mut wrapping = valid_request(shape, &ROOT, &[]);
    wrapping.a_region = ScalarGemmBufferRegionV1::new(
        1,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        0,
        usize::MAX,
        usize::MAX - 1,
        shape.a_bytes(),
    );
    assert_eq!(
        admit_scalar_gemm_host_v1(wrapping),
        Err(ScalarGemmHostAdmissionErrorV1::RegionEndOverflow { field: "A" })
    );
}

#[test]
fn rejects_launch_domain_allocation_address_space_and_pointer_substitution() {
    let too_wide = ScalarGemmShapeV1::checked(u32::MAX, 1, 0).unwrap();
    assert!(too_wide.rounded_grid_threads() > SCALAR_GEMM_MAX_GRID_THREADS_V1);
    assert!(matches!(
        admit_scalar_gemm_host_v1(valid_request(too_wide, &ROOT, &[])),
        Err(ScalarGemmHostAdmissionErrorV1::LaunchDomainOverflow { .. })
    ));

    let shape = ScalarGemmShapeV1::checked(2, 2, 2).unwrap();
    let mut wrong_address_space = valid_request(shape, &ROOT, &[]);
    wrong_address_space.a_region =
        ScalarGemmBufferRegionV1::new(1, 7, 0x1_0000, shape.a_bytes(), 0, shape.a_bytes());
    assert_eq!(
        admit_scalar_gemm_host_v1(wrong_address_space),
        Err(ScalarGemmHostAdmissionErrorV1::WrongAddressSpace { field: "A" })
    );

    let mut out_of_bounds = valid_request(shape, &ROOT, &[]);
    out_of_bounds.b_region = ScalarGemmBufferRegionV1::new(
        2,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        0x2_0000,
        shape.b_bytes() - 1,
        0,
        shape.b_bytes(),
    );
    assert_eq!(
        admit_scalar_gemm_host_v1(out_of_bounds),
        Err(ScalarGemmHostAdmissionErrorV1::RegionOutOfBounds { field: "B" })
    );

    let mut pointer_wrap = valid_request(shape, &ROOT, &[]);
    pointer_wrap.c_region = ScalarGemmBufferRegionV1::new(
        3,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        usize::MAX - shape.c_bytes() + 1,
        shape.c_bytes(),
        0,
        shape.c_bytes(),
    );
    assert_eq!(
        admit_scalar_gemm_host_v1(pointer_wrap),
        Err(ScalarGemmHostAdmissionErrorV1::AllocationEndOverflow { field: "C" })
    );

    let mut substituted_identity = valid_request(shape, &ROOT, &[]);
    substituted_identity.a_region = ScalarGemmBufferRegionV1::new(
        9,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        0x9_0000,
        shape.a_bytes(),
        0,
        shape.a_bytes(),
    );
    substituted_identity.b_region = ScalarGemmBufferRegionV1::new(
        9,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        0xa_0000,
        shape.b_bytes(),
        0,
        shape.b_bytes(),
    );
    assert_eq!(
        admit_scalar_gemm_host_v1(substituted_identity),
        Err(ScalarGemmHostAdmissionErrorV1::AllocationIdentityMismatch {
            left: "A",
            right: "B"
        })
    );
}

#[test]
fn rejects_partial_and_exact_output_aliases_but_allows_shared_inputs() {
    let shape = ScalarGemmShapeV1::checked(2, 2, 2).unwrap();
    let mut c_aliases_a = valid_request(shape, &ROOT, &[]);
    c_aliases_a.a_region = ScalarGemmBufferRegionV1::new(
        7,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        0x7_0000,
        shape.a_bytes() + 4,
        0,
        shape.a_bytes(),
    );
    c_aliases_a.c_region = ScalarGemmBufferRegionV1::new(
        7,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        0x7_0000,
        shape.a_bytes() + 4,
        4,
        shape.c_bytes(),
    );
    assert_eq!(
        admit_scalar_gemm_host_v1(c_aliases_a),
        Err(ScalarGemmHostAdmissionErrorV1::OutputAliasesInput { input: "A" })
    );

    let mut c_aliases_b = valid_request(shape, &ROOT, &[]);
    c_aliases_b.b_region = ScalarGemmBufferRegionV1::new(
        8,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        0x8_0000,
        16 + shape.b_bytes(),
        16,
        shape.b_bytes(),
    );
    c_aliases_b.c_region = ScalarGemmBufferRegionV1::new(
        8,
        SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1,
        0x8_0000,
        16 + shape.b_bytes(),
        16,
        shape.c_bytes(),
    );
    assert_eq!(
        admit_scalar_gemm_host_v1(c_aliases_b),
        Err(ScalarGemmHostAdmissionErrorV1::OutputAliasesInput { input: "B" })
    );

    let mut shared_inputs = valid_request(shape, &ROOT, &[]);
    shared_inputs.a_region = region(9, shape.a_bytes());
    shared_inputs.b_region = region(9, shape.b_bytes());
    assert!(admit_scalar_gemm_host_v1(shared_inputs).is_ok());
}

#[test]
fn rejects_wrong_target_profile_toolchain_roots_and_calls() {
    let shape = ScalarGemmShapeV1::checked(1, 1, 1).unwrap();

    for target in ["gfx942", "gfx942:xnack+", "gfx950:xnack-"] {
        let mut request = valid_request(shape, &ROOT, &[]);
        request.declared_target = target;
        assert_eq!(
            admit_scalar_gemm_host_v1(request),
            Err(ScalarGemmHostAdmissionErrorV1::WrongTarget)
        );
    }

    let mut profile = valid_request(shape, &ROOT, &[]);
    profile.declared_coverage_profile = "COV5";
    assert_eq!(
        admit_scalar_gemm_host_v1(profile),
        Err(ScalarGemmHostAdmissionErrorV1::WrongCoverageProfile)
    );

    let mut comgr = valid_request(shape, &ROOT, &[]);
    comgr.declared_toolchain = ScalarGemmToolchainV1::Comgr;
    assert_eq!(
        admit_scalar_gemm_host_v1(comgr),
        Err(ScalarGemmHostAdmissionErrorV1::ComgrForbidden)
    );

    let mut other = valid_request(shape, &ROOT, &[]);
    other.declared_toolchain = ScalarGemmToolchainV1::Other;
    assert_eq!(
        admit_scalar_gemm_host_v1(other),
        Err(ScalarGemmHostAdmissionErrorV1::WrongToolchain)
    );

    for roots in [
        &[][..],
        &[SCALAR_GEMM_ROOT_SYMBOL_V1, "extra"][..],
        &["lookalike"][..],
    ] {
        assert_eq!(
            admit_scalar_gemm_host_v1(valid_request(shape, roots, &[])),
            Err(ScalarGemmHostAdmissionErrorV1::WrongRootSet)
        );
    }
    assert_eq!(
        admit_scalar_gemm_host_v1(valid_request(shape, &ROOT, &["helper"])),
        Err(ScalarGemmHostAdmissionErrorV1::UnexpectedCalls)
    );
}

#[test]
fn zero_dimensions_have_no_outputs_or_stores() {
    for shape in [
        ScalarGemmShapeV1::checked(0, 7, 5).unwrap(),
        ScalarGemmShapeV1::checked(7, 0, 5).unwrap(),
        ScalarGemmShapeV1::checked(0, 0, 0).unwrap(),
    ] {
        let admission = admit_scalar_gemm_host_v1(valid_request(shape, &ROOT, &[])).unwrap();
        assert_eq!(admission.shape().c_len(), 0);
        assert!(shape.invocation(0).is_none());
        let a = vec![1.0; shape.a_len()];
        let b = vec![2.0; shape.b_len()];
        let mut c = [];
        scalar_gemm_f32_oracle_v1(shape, &a, &b, &mut c).unwrap();
    }
}

#[test]
fn flattened_indices_bounds_initialization_and_writers_hold_exhaustively() {
    for m in 0..=5 {
        for n in 0..=5 {
            for k in 0..=5 {
                let shape = ScalarGemmShapeV1::checked(m, n, k).unwrap();
                for p in 0..=shape.output_elements_u64() + 2 {
                    assert!(scalar_gemm_flattened_index_is_correct_v1(shape, p));
                    for t in 0..=k.saturating_add(1) {
                        assert!(scalar_gemm_accesses_are_in_bounds_v1(shape, p, t));
                    }
                    if p < shape.output_elements_u64() {
                        assert!(scalar_gemm_output_initialized_by_invocation_v1(shape, p, p));
                        assert!(scalar_gemm_complete_launch_initializes_output_v1(shape, p));
                    }
                }
                for left in 0..=shape.output_elements_u64() + 1 {
                    for right in 0..=shape.output_elements_u64() + 1 {
                        assert!(scalar_gemm_writers_are_injective_v1(shape, left, right));
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExactExpression {
    Zero,
    A(usize),
    B(usize),
    Mul(Box<Self>, Box<Self>),
    Add(Box<Self>, Box<Self>),
}

#[test]
fn abstract_model_builds_the_exact_left_associated_recurrence() {
    use ExactExpression::{A, Add, B, Mul, Zero};

    let shape = ScalarGemmShapeV1::checked(1, 1, 3).unwrap();
    let a = vec![A(0), A(1), A(2)];
    let b = vec![B(0), B(1), B(2)];
    let expression = evaluate_scalar_gemm_abstract_invocation_v1(
        shape,
        0,
        &a,
        &b,
        Zero,
        |left, right| Mul(Box::new(left.clone()), Box::new(right.clone())),
        |acc, product| Add(Box::new(acc), Box::new(product)),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        expression,
        Add(
            Box::new(Add(
                Box::new(Add(
                    Box::new(Zero),
                    Box::new(Mul(Box::new(A(0)), Box::new(B(0))))
                )),
                Box::new(Mul(Box::new(A(1)), Box::new(B(1))))
            )),
            Box::new(Mul(Box::new(A(2)), Box::new(B(2))))
        )
    );
    assert_eq!(
        evaluate_scalar_gemm_abstract_invocation_v1(
            shape,
            1,
            &a,
            &b,
            Zero,
            |left, right| Mul(Box::new(left.clone()), Box::new(right.clone())),
            |acc, product| Add(Box::new(acc), Box::new(product)),
        ),
        Ok(None)
    );
}

#[test]
fn abstract_and_f32_models_reject_malformed_lengths() {
    let shape = ScalarGemmShapeV1::checked(1, 2, 2).unwrap();
    let mut c = vec![0.0; shape.c_len()];
    assert!(matches!(
        scalar_gemm_f32_oracle_v1(shape, &[1.0], &[1.0; 4], &mut c),
        Err(ScalarGemmModelErrorV1::LengthMismatch { field: "A", .. })
    ));
    assert!(matches!(
        evaluate_scalar_gemm_abstract_invocation_v1(
            shape,
            0,
            &[1_i64],
            &[1_i64; 4],
            0,
            |left, right| left * right,
            |acc, product| acc + product,
        ),
        Err(ScalarGemmModelErrorV1::LengthMismatch { field: "A", .. })
    ));
}

#[test]
fn deterministic_matrix_oracle_preserves_canaries() {
    for m in 0..=4_u32 {
        for n in 0..=4_u32 {
            for k in 0..=4_u32 {
                let shape = ScalarGemmShapeV1::checked(m, n, k).unwrap();
                let a = (0..shape.a_len())
                    .map(|index| (index as f32 - 3.0) * 0.25)
                    .collect::<Vec<_>>();
                let b = (0..shape.b_len())
                    .map(|index| (5.0 - index as f32) * 0.125)
                    .collect::<Vec<_>>();
                let left_canary = f32::from_bits(0x7fc0_1234);
                let right_canary = f32::from_bits(0x7fc0_5678);
                let mut guarded = vec![left_canary; shape.c_len() + 6];
                guarded[shape.c_len() + 3..].fill(right_canary);
                scalar_gemm_f32_oracle_v1(shape, &a, &b, &mut guarded[3..shape.c_len() + 3])
                    .unwrap();
                assert!(
                    guarded[..3]
                        .iter()
                        .all(|value| value.to_bits() == left_canary.to_bits())
                );
                assert!(
                    guarded[shape.c_len() + 3..]
                        .iter()
                        .all(|value| value.to_bits() == right_canary.to_bits())
                );

                for p in 0..shape.c_len() {
                    let invocation = shape.invocation(p as u64).unwrap();
                    let mut expected = 0.0_f32;
                    for t in 0..k as usize {
                        let product = fixture_separate_f32_mul(
                            a[invocation.row() as usize * k as usize + t],
                            b[t * n as usize + invocation.col() as usize],
                        );
                        expected = fixture_separate_f32_add(expected, product);
                    }
                    assert_eq!(guarded[p + 3].to_bits(), expected.to_bits());
                }
            }
        }
    }
}

#[test]
fn oracle_uses_positive_zero_and_separate_multiply_then_add() {
    let k_zero = ScalarGemmShapeV1::checked(1, 1, 0).unwrap();
    let mut zero = [f32::NAN];
    scalar_gemm_f32_oracle_v1(k_zero, &[], &[], &mut zero).unwrap();
    assert_eq!(zero[0].to_bits(), 0.0_f32.to_bits());

    let epsilon = f32::EPSILON;
    let shape = ScalarGemmShapeV1::checked(1, 1, 2).unwrap();
    let a = [-1.0, 1.0 + epsilon];
    let b = [1.0, 1.0 - epsilon];
    let mut sequential = [f32::NAN];
    scalar_gemm_f32_oracle_v1(shape, &a, &b, &mut sequential).unwrap();
    let contracted_second_step = (1.0 + epsilon).mul_add(1.0 - epsilon, -1.0);
    assert_eq!(sequential[0].to_bits(), 0.0_f32.to_bits());
    assert_ne!(sequential[0].to_bits(), contracted_second_step.to_bits());

    let contract = SCALAR_GEMM_F32_NUMERICAL_CONTRACT_V1;
    assert_eq!(contract.initial_accumulator_bits, 0.0_f32.to_bits());
    assert!(contract.sequential_t_order);
    assert!(!contract.reassociation_permitted);
    assert!(!contract.contraction_permitted);
    assert!(!contract.ieee_754_refinement_proved);
    assert!(!contract.source_to_machine_refinement_proved);
}
