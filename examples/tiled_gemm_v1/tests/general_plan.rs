use fe2o3_amd_target::AmdTargetId;
use fe2o3_tiled_gemm_v1::contract::TARGET_V1;
use fe2o3_tiled_gemm_v1::numerical_contract::{GemmInputs, NumericalOperand, evaluate_source};
use fe2o3_tiled_gemm_v1::{
    AdmittedTargetV1, GeneralGemmRequestV1, GeneralLaunchLimitErrorV1, GeneralLaunchLimitsV1,
    GeneralPlanErrorV1, GeneralPlanLimitV1, admit_target_v1, execute_general_reference_v1,
    plan_general_gemm_v1,
};
use sha2::{Digest, Sha256};

fn target() -> AdmittedTargetV1 {
    admit_target_v1(AmdTargetId::parse(TARGET_V1).unwrap()).unwrap()
}

fn limits() -> GeneralLaunchLimitsV1 {
    GeneralLaunchLimitsV1::representable()
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

#[test]
fn checked_plan_covers_grid_phases_tails_strides_coefficients_and_resources() {
    let plan = plan_general_gemm_v1(
        target(),
        request([17, 19, 18], [23, 29, 31], [2.0, -1.0]),
        limits(),
    )
    .unwrap();

    assert_eq!(plan.request().dimensions(), [17, 19, 18]);
    assert_eq!(plan.request().strides(), [23, 29, 31]);
    assert_eq!(plan.request().alpha_bits(), 2.0_f32.to_bits());
    assert_eq!(plan.request().beta_bits(), (-1.0_f32).to_bits());
    assert_eq!(plan.block_counts(), [2, 2, 1]);
    assert_eq!(plan.aql_grid_work_items(), [128, 2, 1]);
    assert_eq!(plan.workgroup_dimensions(), [64, 1, 1]);
    assert_eq!(plan.reduction_phases(), 2);
    assert_eq!(plan.total_workgroups(), 4);
    assert_eq!(plan.lds_bytes(), 1_024);
    assert!(plan.requires_dispatch());
    assert_eq!(plan.storage().elements(), [386, 512, 515]);
    assert_eq!(plan.storage().bytes(), [772, 1_024, 2_060]);
    assert_eq!(plan.numerical_spec().unwrap().dimensions(), [17, 19, 18]);
}

#[test]
fn empty_output_is_no_dispatch_and_ignores_unused_storage() {
    let no_dispatch_limits = GeneralLaunchLimitsV1::checked([1, 1], 1, 1, 1, 1).unwrap();
    for dimensions in [[0, u32::MAX, u32::MAX], [u32::MAX, 0, u32::MAX]] {
        let plan = plan_general_gemm_v1(
            target(),
            request(dimensions, [0, 0, 0], [f32::NAN, f32::INFINITY]),
            no_dispatch_limits,
        )
        .unwrap();
        assert!(!plan.requires_dispatch());
        assert_eq!(plan.block_counts(), [0, 0, 0]);
        assert_eq!(plan.aql_grid_work_items(), [0, 0, 0]);
        assert_eq!(plan.reduction_phases(), 0);
        assert_eq!(plan.total_workgroups(), 0);
        assert_eq!(plan.storage().elements(), [0, 0, 0]);
        assert_eq!(plan.storage().bytes(), [0, 0, 0]);
        assert_eq!(plan.numerical_spec(), None);
    }
}

#[test]
fn zero_k_dispatches_the_beta_epilogue_without_operand_storage() {
    let plan = plan_general_gemm_v1(
        target(),
        request([17, 19, 0], [0, 0, 23], [7.0, 0.5]),
        limits(),
    )
    .unwrap();
    assert!(plan.requires_dispatch());
    assert_eq!(plan.block_counts(), [2, 2, 1]);
    assert_eq!(plan.reduction_phases(), 0);
    assert_eq!(plan.storage().elements(), [0, 0, 387]);

    let c: Vec<_> = (0..387).map(|index| index as f32).collect();
    let result = execute_general_reference_v1(&plan, &[], &[], &c).unwrap();
    assert_eq!(result.output().len(), 17 * 19);
    for row in 0..17 {
        for column in 0..19 {
            assert_eq!(
                result.output()[row * 19 + column],
                0.5 * c[row * 23 + column]
            );
        }
    }
    assert_eq!(result.trace().reduction_phases(), 0);
    assert_eq!(result.trace().publish_barriers(), 0);
    assert_eq!(result.trace().reuse_barriers(), 0);
}

#[test]
fn stride_and_byte_overflow_fail_before_plan_publication() {
    for (strides, expected) in [
        (
            [17, 19, 19],
            GeneralPlanErrorV1::StrideTooSmall {
                operand: NumericalOperand::A,
                minimum: 18,
                actual: 17,
            },
        ),
        (
            [18, 18, 19],
            GeneralPlanErrorV1::StrideTooSmall {
                operand: NumericalOperand::B,
                minimum: 19,
                actual: 18,
            },
        ),
        (
            [18, 19, 18],
            GeneralPlanErrorV1::StrideTooSmall {
                operand: NumericalOperand::C,
                minimum: 19,
                actual: 18,
            },
        ),
    ] {
        assert_eq!(
            plan_general_gemm_v1(
                target(),
                request([17, 19, 18], strides, [1.0, 0.0]),
                limits(),
            ),
            Err(expected)
        );
    }

    assert_eq!(
        plan_general_gemm_v1(
            target(),
            request([u32::MAX, u32::MAX, 0], [0, 0, u32::MAX], [1.0, 0.0],),
            limits(),
        ),
        Err(GeneralPlanErrorV1::StorageByteCountOverflow(
            NumericalOperand::C
        ))
    );
}

#[test]
fn grid_overflow_and_explicit_device_limits_fail_deterministically() {
    let first_grid_x_overflow_n = (u32::MAX / 64 + 1) * 16;
    assert_eq!(
        plan_general_gemm_v1(
            target(),
            request(
                [1, first_grid_x_overflow_n, 0],
                [0, first_grid_x_overflow_n, first_grid_x_overflow_n],
                [1.0, 0.0],
            ),
            limits(),
        ),
        Err(GeneralPlanErrorV1::GridXOverflow)
    );

    let tight = GeneralLaunchLimitsV1::checked([63, 1], 1, 1_024, 64, 1_024).unwrap();
    assert_eq!(
        plan_general_gemm_v1(
            target(),
            request([16, 16, 16], [16, 16, 16], [1.0, 0.0]),
            tight,
        ),
        Err(GeneralPlanErrorV1::LimitExceeded {
            limit: GeneralPlanLimitV1::GridX,
            actual: 64,
            maximum: 63,
        })
    );

    assert_eq!(
        GeneralLaunchLimitsV1::checked([0, 1], 1, 1, 1, 1),
        Err(GeneralLaunchLimitErrorV1::Zero("max_grid_x"))
    );
}

#[test]
fn every_launch_and_allocation_ceiling_is_enforced_independently() {
    let cases = [
        (
            GeneralLaunchLimitsV1::checked([u32::MAX, 1], u64::MAX, u64::MAX, 64, 1_024).unwrap(),
            request([17, 16, 16], [16, 16, 16], [1.0, 0.0]),
            GeneralPlanErrorV1::LimitExceeded {
                limit: GeneralPlanLimitV1::GridY,
                actual: 2,
                maximum: 1,
            },
        ),
        (
            GeneralLaunchLimitsV1::checked([u32::MAX, u32::MAX], 3, u64::MAX, 64, 1_024).unwrap(),
            request([17, 17, 16], [16, 17, 17], [1.0, 0.0]),
            GeneralPlanErrorV1::LimitExceeded {
                limit: GeneralPlanLimitV1::Workgroups,
                actual: 4,
                maximum: 3,
            },
        ),
        (
            GeneralLaunchLimitsV1::checked([u32::MAX, u32::MAX], u64::MAX, u64::MAX, 63, 1_024)
                .unwrap(),
            request([16, 16, 16], [16, 16, 16], [1.0, 0.0]),
            GeneralPlanErrorV1::LimitExceeded {
                limit: GeneralPlanLimitV1::WorkgroupSize,
                actual: 64,
                maximum: 63,
            },
        ),
        (
            GeneralLaunchLimitsV1::checked([u32::MAX, u32::MAX], u64::MAX, u64::MAX, 64, 1_023)
                .unwrap(),
            request([16, 16, 16], [16, 16, 16], [1.0, 0.0]),
            GeneralPlanErrorV1::LimitExceeded {
                limit: GeneralPlanLimitV1::LdsBytes,
                actual: 1_024,
                maximum: 1_023,
            },
        ),
        (
            GeneralLaunchLimitsV1::checked([u32::MAX, u32::MAX], u64::MAX, 511, 64, 1_024).unwrap(),
            request([16, 16, 16], [16, 16, 16], [1.0, 0.0]),
            GeneralPlanErrorV1::LimitExceeded {
                limit: GeneralPlanLimitV1::BufferBytes(NumericalOperand::A),
                actual: 512,
                maximum: 511,
            },
        ),
    ];

    for (limits, request, expected) in cases {
        assert_eq!(
            plan_general_gemm_v1(target(), request, limits),
            Err(expected)
        );
    }
}

#[test]
fn canonical_identity_is_deterministic_and_binds_every_request_field() {
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    let base = request([17, 19, 18], [23, 29, 31], [2.0, -1.0]);
    let first = plan_general_gemm_v1(target(), base, limits()).unwrap();
    let second = plan_general_gemm_v1(target(), base, limits()).unwrap();
    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.encode_canonical(), second.encode_canonical());
    assert_eq!(
        first.identity().as_bytes(),
        &<[u8; 32]>::from(Sha256::digest(first.encode_canonical()))
    );
    assert_eq!(
        hex(first.identity().as_bytes()),
        "450d75ee70e4a010ea9d2610840fc58a795bb666b72f40451e07d2bd378c3f4e"
    );

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
        let mutated = plan_general_gemm_v1(target(), mutation, limits()).unwrap();
        assert_ne!(mutated.identity(), first.identity(), "{mutation:?}");
        assert_ne!(mutated.encode_canonical(), first.encode_canonical());
    }
}

fn bf16_value(index: usize) -> u16 {
    [0x3f00, 0x3f80, 0xbf80, 0x4000][index % 4]
}

#[test]
fn tiled_reference_matches_shared_scalar_contract_across_tails_and_padding() {
    for m in 0..=5_u32 {
        for n in 0..=5_u32 {
            for k in [0_u32, 1, 15, 16, 17, 18] {
                let lda = k + u32::from(k != 0);
                let ldb = n + u32::from(k != 0 && n != 0);
                let ldc = n + u32::from(m != 0 && n != 0);
                let plan = plan_general_gemm_v1(
                    target(),
                    request([m, n, k], [lda, ldb, ldc], [0.5, -1.0]),
                    limits(),
                )
                .unwrap();
                let lengths = plan.storage().elements();
                let a: Vec<_> = (0..lengths[0]).map(bf16_value).collect();
                let b: Vec<_> = (0..lengths[1]).map(|index| bf16_value(index + 1)).collect();
                let c: Vec<_> = (0..lengths[2])
                    .map(|index| (index as i32 % 7 - 3) as f32)
                    .collect();
                let tiled = execute_general_reference_v1(&plan, &a, &b, &c).unwrap();
                if let Some(spec) = plan.numerical_spec() {
                    let scalar = evaluate_source(
                        spec,
                        GemmInputs {
                            a_bits: &a,
                            b_bits: &b,
                            c: &c,
                            alpha: 0.5,
                            beta: -1.0,
                        },
                    )
                    .unwrap();
                    assert_eq!(
                        tiled
                            .output()
                            .iter()
                            .map(|value| value.to_bits())
                            .collect::<Vec<_>>(),
                        scalar
                            .iter()
                            .map(|value| value.to_bits())
                            .collect::<Vec<_>>(),
                        "M={m} N={n} K={k}"
                    );
                    let trace = tiled.trace();
                    assert_eq!(trace.workgroups(), plan.total_workgroups());
                    assert_eq!(trace.output_stores(), u64::from(m) * u64::from(n));
                    assert_eq!(
                        trace.reduction_phases(),
                        plan.total_workgroups() * u64::from(plan.reduction_phases())
                    );
                    assert_eq!(trace.publish_barriers(), trace.reduction_phases());
                    assert_eq!(trace.reuse_barriers(), trace.reduction_phases());
                } else {
                    assert!(tiled.output().is_empty());
                    assert_eq!(tiled.trace().workgroups(), 0);
                }
            }
        }
    }
}

#[test]
fn trace_exposes_all_tail_classes_and_rejects_wrong_storage_lengths() {
    let plan = plan_general_gemm_v1(
        target(),
        request([17, 19, 18], [23, 29, 31], [1.0, 0.0]),
        limits(),
    )
    .unwrap();
    let lengths = plan.storage().elements();
    let a = vec![0x3f80; lengths[0]];
    let b = vec![0x3f80; lengths[1]];
    let c = vec![0.0; lengths[2]];
    let result = execute_general_reference_v1(&plan, &a, &b, &c).unwrap();
    assert!(result.trace().a_zero_fills() > 0);
    assert!(result.trace().b_zero_fills() > 0);
    assert!(result.trace().c_predicated_stores() > 0);

    assert!(matches!(
        execute_general_reference_v1(&plan, &a[..a.len() - 1], &b, &c),
        Err(fe2o3_tiled_gemm_v1::GeneralReferenceErrorV1::WrongLength {
            operand: NumericalOperand::A,
            ..
        })
    ));
}
