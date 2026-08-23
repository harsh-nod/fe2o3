use fe2o3_kernel_ir::*;

fn canonical_v5_bytes(module: Module) -> Vec<u8> {
    VerifiedCanonicalKernelIrV5::from_module(module)
        .expect("valid exact V5")
        .into_canonical_bytes()
}

fn assert_rejected(label: &str, module: Module) {
    assert!(
        CheckedScalarGemmSemanticProjectionV1::from_module(module).is_err(),
        "mutation must be rejected: {label}"
    );
}

#[test]
fn projects_the_complete_canonical_scalar_gemm_graph_without_authority() {
    let projection = CheckedScalarGemmSemanticProjectionV1::from_module(scalar_gemm_v1_module())
        .expect("canonical scalar GEMM projection");

    assert!(!projection.canonical_kir_v5().is_empty());
    assert!(!projection.canonical_token_preimage().is_empty());
    assert!(
        projection.canonical_token_preimage().len() <= MAX_SCALAR_GEMM_SEMANTIC_PROJECTION_BYTES_V1
    );
    assert_ne!(projection.identity().digest(), &[0; 32]);
    assert_ne!(projection.source_kir_identity().digest(), &[0; 32]);
    assert!(!projection.is_verus_semantic_refinement_proof());
    assert!(!projection.grants_compiler_authority());
    assert!(!projection.grants_artifact_authority());
    assert!(!projection.grants_runtime_authority());
    projection.revalidate().expect("custody revalidation");
}

#[test]
fn projection_is_deterministic_across_module_and_exact_v5_inputs() {
    let first = CheckedScalarGemmSemanticProjectionV1::from_module(scalar_gemm_v1_module())
        .expect("first projection");
    let second = CheckedScalarGemmSemanticProjectionV1::from_module(scalar_gemm_v1_module())
        .expect("second projection");
    let from_bytes = CheckedScalarGemmSemanticProjectionV1::from_canonical_kir_v5_bytes(
        canonical_v5_bytes(scalar_gemm_v1_module()),
    )
    .expect("projection from exact bytes");

    assert_eq!(first.canonical_kir_v5(), second.canonical_kir_v5());
    assert_eq!(
        first.canonical_token_preimage(),
        second.canonical_token_preimage()
    );
    assert_eq!(first.identity(), second.identity());
    assert_eq!(
        first.canonical_token_preimage(),
        from_bytes.canonical_token_preimage()
    );
    assert_eq!(first.identity(), from_bytes.identity());
    assert_eq!(
        first.source_kir_identity(),
        from_bytes.source_kir_identity()
    );
}

#[test]
fn exact_v5_round_trip_preserves_the_projection_preimage_and_identity() {
    let original = CheckedScalarGemmSemanticProjectionV1::from_module(scalar_gemm_v1_module())
        .expect("canonical projection");
    let bytes = original.canonical_kir_v5().to_vec();
    let preimage = original.canonical_token_preimage().to_vec();
    let identity = *original.identity();

    let recovered =
        CheckedScalarGemmSemanticProjectionV1::from_canonical_kir_v5_bytes(bytes.clone())
            .expect("round-trip projection");
    assert_eq!(recovered.canonical_kir_v5(), bytes);
    assert_eq!(recovered.canonical_token_preimage(), preimage);
    assert_eq!(*recovered.identity(), identity);
    assert_eq!(recovered.into_canonical_token_preimage(), preimage);
}

#[test]
fn rejects_one_axis_module_function_block_operation_and_cfg_mutations() {
    let mut mutations = Vec::new();

    let mut module_id = scalar_gemm_v1_module();
    module_id.id = ModuleId::new("fe2o3::scalar_gemm_v1_mutated");
    mutations.push(("module id", module_id));

    let mut module_capability = scalar_gemm_v1_module();
    module_capability
        .required_capabilities
        .insert(TargetCapability::Float64);
    mutations.push(("module capability", module_capability));

    let mut extra_function = scalar_gemm_v1_module();
    extra_function.functions.push(Function::external_import(
        "extra",
        Signature::new(vec![], vec![]),
    ));
    mutations.push(("extra function", extra_function));

    let mut function_id = scalar_gemm_v1_module();
    function_id.functions[0].id = FunctionId::new("mutated");
    mutations.push(("function id", function_id));

    let mut function_role = scalar_gemm_v1_module();
    function_role.functions[0].role = FunctionRole::InternalHelper;
    mutations.push(("function role", function_role));

    let mut signature = scalar_gemm_v1_module();
    signature.functions[0].signature.parameters[3] = Type::Scalar(ScalarType::U64);
    mutations.push(("signature type", signature));

    let mut function_parameter = scalar_gemm_v1_module();
    function_parameter.functions[0]
        .body
        .as_mut()
        .expect("body")
        .parameters[3] = ValueId(37);
    mutations.push(("function parameter", function_parameter));

    let mut block_id = scalar_gemm_v1_module();
    block_id.functions[0].body.as_mut().expect("body").blocks[5].id = BlockId(6);
    mutations.push(("block id", block_id));

    let mut block_parameter = scalar_gemm_v1_module();
    block_parameter.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[2]
        .parameters[0]
        .ty = Type::Scalar(ScalarType::U64);
    mutations.push(("block parameter type", block_parameter));

    let mut extra_operation = scalar_gemm_v1_module();
    extra_operation.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[1]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(37), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(2)),
        ));
    mutations.push(("extra operation", extra_operation));

    let mut missing_operation = scalar_gemm_v1_module();
    missing_operation.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .operations
        .pop();
    mutations.push(("missing operation", missing_operation));

    let mut operation_result = scalar_gemm_v1_module();
    operation_result.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .operations[10]
        .results[0]
        .id = ValueId(37);
    mutations.push(("operation result", operation_result));

    let mut operation_kind = scalar_gemm_v1_module();
    let OperationKind::Binary { op, .. } = &mut operation_kind.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .operations[10]
        .kind
    else {
        panic!("binary accumulation")
    };
    *op = BinaryOp::Subtract;
    mutations.push(("operation kind", operation_kind));

    let mut operation_operand = scalar_gemm_v1_module();
    let OperationKind::Binary { rhs, .. } = &mut operation_operand.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .operations[10]
        .kind
    else {
        panic!("binary accumulation")
    };
    *rhs = ValueId(28);
    mutations.push(("operation operand", operation_operand));

    let mut branch_target = scalar_gemm_v1_module();
    let Some(Terminator::ConditionalBranch { then_target, .. }) = &mut branch_target.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[0]
        .terminator
    else {
        panic!("entry conditional")
    };
    *then_target = BlockId(5);
    mutations.push(("terminator edge", branch_target));

    let mut branch_argument = scalar_gemm_v1_module();
    let Some(Terminator::Branch { arguments, .. }) = &mut branch_argument.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[3]
        .terminator
    else {
        panic!("loop branch")
    };
    arguments[1] = ValueId(31);
    mutations.push(("terminator argument", branch_argument));

    for (label, mutation) in mutations {
        assert_rejected(label, mutation);
    }
}

#[test]
fn rejects_one_axis_kernel_launch_capability_and_memory_access_mutations() {
    let mut mutations = Vec::new();

    let mut missing_kernel = scalar_gemm_v1_module();
    missing_kernel.kernels.clear();
    mutations.push(("missing kernel", missing_kernel));

    let mut extra_kernel = scalar_gemm_v1_module();
    extra_kernel.kernels.push(Kernel::new(
        "scalar_gemm_v1_extra",
        SCALAR_GEMM_V1_FUNCTION_ID,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    mutations.push(("extra kernel", extra_kernel));

    let mut kernel_id = scalar_gemm_v1_module();
    kernel_id.kernels[0].id = KernelId::new("scalar_gemm_v1_alias");
    mutations.push(("kernel id", kernel_id));

    let mut kernel_entry = scalar_gemm_v1_module();
    kernel_entry.kernels[0].entry = FunctionId::new("foreign_entry");
    mutations.push(("kernel entry", kernel_entry));

    let mut launch_domain = scalar_gemm_v1_module();
    launch_domain.kernels[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Dynamic,
        y: LaunchExtent::Dynamic,
    };
    mutations.push(("launch domain", launch_domain));

    let mut launch_extent = scalar_gemm_v1_module();
    launch_extent.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(256),
    };
    mutations.push(("launch extent", launch_extent));

    let mut workgroup_x = scalar_gemm_v1_module();
    workgroup_x.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    mutations.push(("workgroup x", workgroup_x));

    let mut kernel_capability = scalar_gemm_v1_module();
    kernel_capability.kernels[0]
        .required_capabilities
        .insert(TargetCapability::Float64);
    mutations.push(("kernel capability", kernel_capability));

    let mut alignment = scalar_gemm_v1_module();
    let OperationKind::Load { access, .. } =
        &mut alignment.functions[0].body.as_mut().expect("body").blocks[3].operations[6].kind
    else {
        panic!("A load")
    };
    access.alignment = 8;
    mutations.push(("memory alignment", alignment));

    let mut volatile = scalar_gemm_v1_module();
    let OperationKind::Load { access, .. } =
        &mut volatile.functions[0].body.as_mut().expect("body").blocks[3].operations[6].kind
    else {
        panic!("A load")
    };
    access.volatile = true;
    mutations.push(("memory volatility", volatile));

    let mut address_space = scalar_gemm_v1_module();
    let OperationKind::Store { access, .. } = &mut address_space.functions[0]
        .body
        .as_mut()
        .expect("body")
        .blocks[4]
        .operations[1]
        .kind
    else {
        panic!("C store")
    };
    access.address_space = AddressSpace::Generic;
    mutations.push(("memory address space", address_space));

    for (label, mutation) in mutations {
        assert_rejected(label, mutation);
    }
}

#[test]
fn rejects_noncanonical_and_malformed_caller_provided_v5_bytes() {
    let mut noncanonical = scalar_gemm_v1_module();
    noncanonical.id = ModuleId::new("valid-but-foreign");
    let noncanonical = encode_module_v5(&noncanonical).expect("canonical foreign V5");
    assert!(matches!(
        CheckedScalarGemmSemanticProjectionV1::from_canonical_kir_v5_bytes(noncanonical),
        Err(ScalarGemmSemanticProjectionErrorV1::NonCanonicalProjection)
    ));

    let mut malformed = canonical_v5_bytes(scalar_gemm_v1_module());
    malformed.push(0);
    assert!(CheckedScalarGemmSemanticProjectionV1::from_canonical_kir_v5_bytes(malformed).is_err());
}

#[test]
fn rejects_valid_but_unsupported_operation_and_terminator_families() {
    let mut unary = scalar_gemm_v1_module();
    unary.functions[0].body.as_mut().expect("body").blocks[3].operations[10].kind =
        OperationKind::Unary {
            op: UnaryOp::Negate,
            operand: ValueId(28),
        };
    assert!(matches!(
        CheckedScalarGemmSemanticProjectionV1::from_module(unary),
        Err(ScalarGemmSemanticProjectionErrorV1::UnsupportedField(
            "operation kind"
        ))
    ));

    let mut unreachable = scalar_gemm_v1_module();
    unreachable.functions[0].body.as_mut().expect("body").blocks[5].terminator =
        Some(Terminator::Unreachable);
    assert!(matches!(
        CheckedScalarGemmSemanticProjectionV1::from_module(unreachable),
        Err(ScalarGemmSemanticProjectionErrorV1::UnsupportedField(
            "terminator kind"
        ))
    ));
}
