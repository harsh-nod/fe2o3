use fe2o3_kernel_ir::*;

fn canonical_v5(module: Module) -> Vec<u8> {
    VerifiedCanonicalKernelIrV5::from_module(module)
        .expect("valid canonical V5 module")
        .into_canonical_bytes()
}

fn scalar_v5() -> Vec<u8> {
    canonical_v5(scalar_gemm_v1_module())
}

fn scalar_input(
    a: Vec<i128>,
    b: Vec<i128>,
    c: Vec<i128>,
    m: u32,
    n: u32,
    k: u32,
    global_invocations: u64,
) -> ScalarGemmIntegerOracleInputV1 {
    ScalarGemmIntegerOracleInputV1 {
        a,
        b,
        c,
        m,
        n,
        k,
        global_invocations,
    }
}

fn generic_scalar_request(
    a: Vec<i128>,
    b: Vec<i128>,
    c: Vec<i128>,
    m: u32,
    n: u32,
    k: u32,
    global_invocations: u64,
) -> IntegerSemanticOracleRequestV1 {
    IntegerSemanticOracleRequestV1::new(
        SCALAR_GEMM_V1_KERNEL_ID,
        vec![
            IntegerSemanticOracleArgumentV1::Buffer(a),
            IntegerSemanticOracleArgumentV1::Buffer(b),
            IntegerSemanticOracleArgumentV1::Buffer(c),
            IntegerSemanticOracleArgumentV1::Integer(i128::from(m)),
            IntegerSemanticOracleArgumentV1::Integer(i128::from(n)),
            IntegerSemanticOracleArgumentV1::Integer(i128::from(k)),
        ],
        [global_invocations, 1, 1],
    )
}

fn reference_integer_gemm(a: &[i128], b: &[i128], m: u32, n: u32, k: u32) -> Vec<i128> {
    let mut c = vec![0; m as usize * n as usize];
    for row in 0..m as usize {
        for column in 0..n as usize {
            let mut accumulator = 0i128;
            for reduction in 0..k as usize {
                accumulator = accumulator
                    .checked_add(
                        a[row * k as usize + reduction]
                            .checked_mul(b[reduction * n as usize + column])
                            .expect("bounded reference multiplication"),
                    )
                    .expect("bounded reference addition");
            }
            c[row * n as usize + column] = accumulator;
        }
    }
    c
}

#[test]
fn exact_scalar_graph_executes_cfg_ssa_loop_and_memory_semantics() {
    let execution = execute_scalar_gemm_v1_integer_semantic_oracle_v1(
        &scalar_v5(),
        scalar_input(
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8, 9, 10],
            vec![-1; 6],
            2,
            3,
            2,
            11,
        ),
    )
    .expect("integer scalar GEMM");

    assert_eq!(execution.c, [21, 24, 27, 47, 54, 61]);
    assert_eq!(execution.invocations_executed, 11);
    assert!(execution.steps_executed > execution.invocations_executed);
    assert!(!execution.is_verus_proof());
    assert!(!execution.models_ieee_f32());
    assert!(!execution.proves_race_freedom());
    assert!(!execution.grants_compiler_artifact_or_runtime_authority());
}

#[test]
fn generic_entry_preserves_owned_arguments_and_reports_deterministic_counters() {
    let bytes = scalar_v5();
    let request = generic_scalar_request(vec![2], vec![3], vec![99], 1, 1, 1, 3);
    let first = execute_kernel_ir_v5_integer_semantic_oracle_v1(&bytes, request.clone())
        .expect("first execution");
    let second =
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&bytes, request).expect("second execution");

    assert_eq!(first, second);
    assert_eq!(first.buffer(0), Some([2].as_slice()));
    assert_eq!(first.buffer(1), Some([3].as_slice()));
    assert_eq!(first.buffer(2), Some([6].as_slice()));
    assert_eq!(first.invocations_executed(), 3);
    assert!(first.steps_executed() > 0);
    assert!(!first.is_verus_proof());
    assert!(!first.models_ieee_f32());
    assert!(!first.proves_race_freedom());
    assert!(!first.grants_compiler_artifact_or_runtime_authority());
}

#[test]
fn differential_cases_match_an_independent_integer_reference() {
    let bytes = scalar_v5();
    let mut state = 0x4d59_5df4_d0f3_3173u64;
    for m in 1..=4u32 {
        for n in 1..=4u32 {
            for k in 0..=5u32 {
                let mut next = || {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    (state % 19) as i128 - 9
                };
                let a = (0..m as usize * k as usize)
                    .map(|_| next())
                    .collect::<Vec<_>>();
                let b = (0..k as usize * n as usize)
                    .map(|_| next())
                    .collect::<Vec<_>>();
                let expected = reference_integer_gemm(&a, &b, m, n, k);
                let execution = execute_scalar_gemm_v1_integer_semantic_oracle_v1(
                    &bytes,
                    scalar_input(
                        a,
                        b,
                        vec![i128::MIN; m as usize * n as usize],
                        m,
                        n,
                        k,
                        u64::from(m) * u64::from(n) + 3,
                    ),
                )
                .unwrap_or_else(|error| panic!("{m}x{n}x{k} failed: {error}"));
                assert_eq!(execution.c, expected, "differential case {m}x{n}x{k}");
            }
        }
    }
}

#[test]
fn zero_sized_output_takes_only_the_inactive_path() {
    let execution = execute_scalar_gemm_v1_integer_semantic_oracle_v1(
        &scalar_v5(),
        scalar_input(Vec::new(), Vec::new(), Vec::new(), 7, 0, 5, 9),
    )
    .expect("inactive zero-column launch");
    assert!(execution.c.is_empty());
    assert_eq!(execution.invocations_executed, 9);
}

#[test]
fn malformed_non_v5_and_noncanonical_inputs_fail_before_execution() {
    let bytes = scalar_v5();
    let request = generic_scalar_request(vec![1], vec![1], vec![0], 1, 1, 1, 1);
    for malformed in [&bytes[..bytes.len() - 1], b"not kernel IR".as_slice()] {
        assert!(matches!(
            execute_kernel_ir_v5_integer_semantic_oracle_v1(malformed, request.clone()),
            Err(IntegerSemanticOracleErrorV1::CanonicalKernelIr(_))
        ));
    }

    let v4 = encode_module_v4(&scalar_gemm_v1_module()).expect("V4 encoding");
    assert!(matches!(
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&v4, request.clone()),
        Err(IntegerSemanticOracleErrorV1::CanonicalKernelIr(
            VerifiedCanonicalKernelIrErrorV5::NotExactV5 { version: 4 }
        ))
    ));

    let mut structurally_invalid = scalar_gemm_v1_module();
    structurally_invalid.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[0]
        .terminator = None;
    let invalid_bytes = encode_module_v5(&structurally_invalid).expect("bounded invalid encoding");
    assert!(matches!(
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&invalid_bytes, request),
        Err(IntegerSemanticOracleErrorV1::CanonicalKernelIr(
            VerifiedCanonicalKernelIrErrorV5::Verification(_)
        ))
    ));
}

#[test]
fn one_byte_mutations_never_enter_the_exact_scalar_profile() {
    let bytes = scalar_v5();
    for index in [0, bytes.len() / 3, bytes.len() / 2, bytes.len() - 1] {
        let mut mutated = bytes.clone();
        mutated[index] ^= 1;
        let result = execute_scalar_gemm_v1_integer_semantic_oracle_v1(
            &mutated,
            ScalarGemmIntegerOracleInputV1::new(vec![1], vec![1], vec![0], 1, 1, 1),
        );
        assert!(result.is_err(), "byte mutation {index} was accepted");
    }
}

#[test]
fn unsupported_operation_type_and_capability_fail_closed() {
    let request = generic_scalar_request(vec![1], vec![1], vec![0], 1, 1, 1, 1);

    let mut unsupported_operation = scalar_gemm_v1_module();
    unsupported_operation.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .operations[9]
        .kind = OperationKind::Unary {
        op: UnaryOp::Negate,
        operand: ValueId(28),
    };
    let unsupported_operation = canonical_v5(unsupported_operation);
    assert_eq!(
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&unsupported_operation, request.clone()),
        Err(IntegerSemanticOracleErrorV1::UnsupportedOperation {
            block: BlockId(3),
            operation: 9,
            kind: "unary",
        })
    );

    let mut unsupported_capability = scalar_gemm_v1_module();
    unsupported_capability
        .required_capabilities
        .insert(TargetCapability::Float64);
    assert_eq!(
        execute_kernel_ir_v5_integer_semantic_oracle_v1(
            &canonical_v5(unsupported_capability),
            request.clone(),
        ),
        Err(IntegerSemanticOracleErrorV1::UnsupportedCapability(
            TargetCapability::Float64
        ))
    );

    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), Type::F64),
        OperationKind::Constant(Constant::F64Bits(0)),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::kernel_entry(
        "unsupported_type_entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let mut module = Module::new("unsupported_type_module");
    module.functions.push(function);
    module.kernels.push(Kernel::new(
        "unsupported_type",
        "unsupported_type_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    assert_eq!(
        execute_kernel_ir_v5_integer_semantic_oracle_v1(
            &canonical_v5(module),
            IntegerSemanticOracleRequestV1::new("unsupported_type", vec![], [1, 1, 1]),
        ),
        Err(IntegerSemanticOracleErrorV1::UnsupportedType(Type::F64))
    );
}

#[test]
fn exact_profile_rejects_supported_semantic_graph_mutations() {
    let mut mutations = Vec::new();

    let mut accumulator = scalar_gemm_v1_module();
    let OperationKind::Binary { lhs, rhs, .. } =
        &mut accumulator.functions[0].body.as_mut().expect("body").blocks[3].operations[10].kind
    else {
        panic!("accumulator add")
    };
    *lhs = ValueId(31);
    *rhs = ValueId(31);
    mutations.push(accumulator);

    let mut loop_carried = scalar_gemm_v1_module();
    let Some(Terminator::Branch { arguments, .. }) = &mut loop_carried.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .terminator
    else {
        panic!("loop branch")
    };
    arguments[1] = ValueId(31);
    mutations.push(loop_carried);

    let mut output_address = scalar_gemm_v1_module();
    let OperationKind::GetElementPointer { offset, .. } = &mut output_address.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[4]
        .operations[0]
        .kind
    else {
        panic!("output GEP")
    };
    *offset = ValueId(15);
    mutations.push(output_address);

    let mut cfg = scalar_gemm_v1_module();
    let Some(Terminator::ConditionalBranch {
        then_target,
        else_target,
        ..
    }) = &mut cfg.functions[0].body.as_mut().expect("body").blocks[0].terminator
    else {
        panic!("entry branch")
    };
    std::mem::swap(then_target, else_target);
    mutations.push(cfg);

    for mutation in mutations {
        let result = execute_scalar_gemm_v1_integer_semantic_oracle_v1(
            &canonical_v5(mutation),
            scalar_input(vec![2, 3], vec![5, 7], vec![99], 1, 1, 2, 1),
        );
        assert!(matches!(
            result,
            Err(IntegerSemanticOracleErrorV1::ScalarGemmProfile(
                ScalarGemmV1Error::NonCanonicalKernelIr
            ))
        ));
    }
}

#[test]
fn general_executor_observes_supported_graph_mutations_instead_of_normalizing_them() {
    let request = || generic_scalar_request(vec![2, 3], vec![5, 7], vec![99], 1, 1, 2, 1);
    let canonical = execute_kernel_ir_v5_integer_semantic_oracle_v1(&scalar_v5(), request())
        .expect("canonical execution");
    assert_eq!(canonical.buffer(2), Some([31].as_slice()));

    let mut doubled_product = scalar_gemm_v1_module();
    let OperationKind::Binary { lhs, rhs, .. } = &mut doubled_product.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .operations[10]
        .kind
    else {
        panic!("accumulator add")
    };
    *lhs = ValueId(31);
    *rhs = ValueId(31);
    let doubled =
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&canonical_v5(doubled_product), request())
            .expect("supported mutation executes");
    assert_eq!(doubled.buffer(2), Some([42].as_slice()));

    let mut last_product = scalar_gemm_v1_module();
    let Some(Terminator::Branch { arguments, .. }) = &mut last_product.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .terminator
    else {
        panic!("loop branch")
    };
    arguments[1] = ValueId(31);
    let last =
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&canonical_v5(last_product), request())
            .expect("loop-carried mutation executes");
    assert_eq!(last.buffer(2), Some([21].as_slice()));
}

#[test]
fn memory_bounds_and_integer_overflow_fail_closed_without_partial_result() {
    assert!(matches!(
        execute_scalar_gemm_v1_integer_semantic_oracle_v1(
            &scalar_v5(),
            scalar_input(Vec::new(), vec![2], vec![0], 1, 1, 1, 1),
        ),
        Err(IntegerSemanticOracleErrorV1::MemoryOutOfBounds {
            argument: 0,
            index: 0,
            length: 0,
        })
    ));

    assert!(matches!(
        execute_scalar_gemm_v1_integer_semantic_oracle_v1(
            &scalar_v5(),
            scalar_input(vec![i128::MAX], vec![2], vec![0], 1, 1, 1, 1),
        ),
        Err(IntegerSemanticOracleErrorV1::ArithmeticOverflow { .. })
    ));
}

#[test]
fn every_configured_resource_bound_is_enforced() {
    let bytes = scalar_v5();
    let input = || scalar_input(vec![1, 2, 3, 4], vec![1, 2, 3, 4], vec![0; 4], 2, 2, 2, 4);

    let mut limits = IntegerSemanticOracleLimitsV1 {
        max_canonical_bytes: bytes.len() - 1,
        ..IntegerSemanticOracleLimitsV1::default()
    };
    assert!(matches!(
        execute_scalar_gemm_v1_integer_semantic_oracle_with_limits_v1(&bytes, input(), &limits),
        Err(IntegerSemanticOracleErrorV1::ResourceLimitExceeded {
            resource: "canonical bytes",
            ..
        })
    ));

    limits = IntegerSemanticOracleLimitsV1 {
        max_invocations: 3,
        ..IntegerSemanticOracleLimitsV1::default()
    };
    assert_eq!(
        execute_scalar_gemm_v1_integer_semantic_oracle_with_limits_v1(&bytes, input(), &limits),
        Err(IntegerSemanticOracleErrorV1::InvocationLimitExceeded {
            actual: 4,
            limit: 3,
        })
    );

    limits = IntegerSemanticOracleLimitsV1 {
        max_buffer_elements: 3,
        ..IntegerSemanticOracleLimitsV1::default()
    };
    assert!(matches!(
        execute_scalar_gemm_v1_integer_semantic_oracle_with_limits_v1(&bytes, input(), &limits),
        Err(IntegerSemanticOracleErrorV1::BufferElementLimitExceeded {
            argument: 0,
            actual: 4,
            limit: 3,
        })
    ));

    limits = IntegerSemanticOracleLimitsV1 {
        max_total_buffer_elements: 11,
        ..IntegerSemanticOracleLimitsV1::default()
    };
    assert_eq!(
        execute_scalar_gemm_v1_integer_semantic_oracle_with_limits_v1(&bytes, input(), &limits),
        Err(
            IntegerSemanticOracleErrorV1::TotalBufferElementLimitExceeded {
                actual: 12,
                limit: 11,
            }
        )
    );

    limits = IntegerSemanticOracleLimitsV1 {
        max_ssa_values: 36,
        ..IntegerSemanticOracleLimitsV1::default()
    };
    assert!(matches!(
        execute_scalar_gemm_v1_integer_semantic_oracle_with_limits_v1(&bytes, input(), &limits),
        Err(IntegerSemanticOracleErrorV1::SsaValueLimitExceeded { limit: 36, .. })
    ));
}

#[test]
fn shared_fuel_bounds_finite_and_mutated_infinite_loops() {
    let bytes = scalar_v5();
    let mut limits = IntegerSemanticOracleLimitsV1 {
        max_steps: 10,
        ..IntegerSemanticOracleLimitsV1::default()
    };
    assert_eq!(
        execute_scalar_gemm_v1_integer_semantic_oracle_with_limits_v1(
            &bytes,
            scalar_input(vec![1], vec![1], vec![0], 1, 1, 1, 1),
            &limits,
        ),
        Err(IntegerSemanticOracleErrorV1::FuelExhausted {
            consumed: 10,
            limit: 10,
        })
    );

    let mut infinite = scalar_gemm_v1_module();
    let Some(Terminator::Branch { arguments, .. }) =
        &mut infinite.functions[0].body.as_mut().expect("body").blocks[3].terminator
    else {
        panic!("loop branch")
    };
    arguments[0] = ValueId(19);
    limits.max_steps = 200;
    assert_eq!(
        execute_kernel_ir_v5_integer_semantic_oracle_with_limits_v1(
            &canonical_v5(infinite),
            generic_scalar_request(vec![1], vec![1], vec![0], 1, 1, 1, 1),
            &limits,
        ),
        Err(IntegerSemanticOracleErrorV1::FuelExhausted {
            consumed: 200,
            limit: 200,
        })
    );
}

#[test]
fn argument_and_launch_shape_errors_are_rejected_before_interpretation() {
    let bytes = scalar_v5();
    let wrong_count =
        IntegerSemanticOracleRequestV1::new(SCALAR_GEMM_V1_KERNEL_ID, vec![], [1, 1, 1]);
    assert_eq!(
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&bytes, wrong_count),
        Err(IntegerSemanticOracleErrorV1::ArgumentCount {
            expected: 6,
            actual: 0,
        })
    );

    let mut wrong_type = generic_scalar_request(vec![1], vec![1], vec![0], 1, 1, 1, 1);
    wrong_type.arguments[3] = IntegerSemanticOracleArgumentV1::Buffer(vec![1]);
    assert_eq!(
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&bytes, wrong_type),
        Err(IntegerSemanticOracleErrorV1::ArgumentType { argument: 3 })
    );

    let mut wrong_launch = generic_scalar_request(vec![1], vec![1], vec![0], 1, 1, 1, 1);
    wrong_launch.global_size = [1, 2, 1];
    assert_eq!(
        execute_kernel_ir_v5_integer_semantic_oracle_v1(&bytes, wrong_launch),
        Err(IntegerSemanticOracleErrorV1::InvalidLaunchSize {
            global_size: [1, 2, 1],
        })
    );
}
