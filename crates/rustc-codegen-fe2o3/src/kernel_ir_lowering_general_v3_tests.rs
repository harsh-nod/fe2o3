use super::*;
use crate::mir_import::{
    MirImportedType, MirLocal, MirLocalRole, MirPlaceRef, MirProjectionElem,
    MirSemanticAdmissionInputsV2, MirSourceLocation, MirStatement, MirSwitchTarget,
};
use crate::trusted_device_items::{TrustedAmdGpuDiagnosticOperation, TrustedAmdGpuInlineOperation};
use dialect_mir::MirType;
use fe2o3_artifacts::{
    AbiLayout, BlockSize, Capability, Dimensions, Endianness, IdentityText, LaunchContract,
    PointerWidth, TargetIdentity,
};
use fe2o3_kernel_ir::{Axis, IndexKind, IntrinsicKind, IntrinsicOperation, MatrixOperation};
use fe2o3_rustc_front::{
    FrontendLaunchBoundsV1, FrontendWorkgroupDimensionsV1, KernelFrontendContractV1,
};
use reserved_fe2o3_symbols::{
    KernelBindingIdV1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3, derive_crate_binding_id_v1,
    derive_kernel_binding_id_v1, host_kernel_symbol_v1,
};

const S09_CRATE_NAME: &str = "fe2o3_typed_alias_spoof";
const S09_MODULE_PATH: &str = "general_genuine";
const S09_LOGICAL_NAME: &str = "alpha";
const S09_EXPORT_NAME: &str = "alpha";

fn matrix_frontend_binding(
    function: &Function,
) -> Option<&fe2o3_kernel_ir::MatrixFrontendBindingV2> {
    operations(function)
        .into_iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::Matrix(matrix) => matrix.frontend_binding.as_ref(),
            _ => None,
        })
}

#[test]
fn exact_genuine_matrix_call_lowers_to_the_existing_gfx942_mfma_contract() {
    let module = translate_and_verify_for_target(
        &MirModule {
            functions: vec![matrix_frontend_function()],
        },
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .expect("exact matrix frontend slice");

    assert_eq!(
        module.kernels[0].workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );
    let function = function(&module, "tests::tiled_gemm_frontend_v1");
    assert_eq!(
        function.signature.parameters,
        [vec![Type::Scalar(ScalarType::Bf16); 8], vec![Type::F32; 4]].concat()
    );
    let matrix = operations(function)
        .into_iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::Matrix(matrix) => Some(matrix),
            _ => None,
        })
        .expect("one matrix operation");
    assert_eq!(
        matrix.kind,
        MatrixOperation::multiply_accumulate(
            [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
            [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
            [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
        )
        .kind
    );
    let frontend = matrix.frontend_binding.as_ref().expect("rustc ABI binding");

    let generic = dialect_amdgcn::lower_kernel_to_gfx942_llvm_ir(&module, &module.kernels[0].id)
        .expect_err("generic gfx942 lowering must not erase exact xnack-minus identity");
    assert!(generic.to_string().contains("gfx942:xnack-"), "{generic}");
    let llvm =
        dialect_amdgcn::lower_kernel_to_gfx942_xnack_minus_llvm_ir(&module, &module.kernels[0].id)
            .expect("exact dialect matrix validation and lowering");
    assert!(llvm.contains("llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
    assert!(llvm.contains("-wavefrontsize32,+wavefrontsize64,-xnack"));
    assert!(llvm.contains("\"fp-contract\"=\"off\""));
    assert!(llvm.contains(dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT));
    assert!(llvm.contains("fe2o3.projected-kernarg-policy.v1 sha256="));
    assert!(llvm.contains(
        "fe2o3.projected-kernarg explicit-size=32 implicit-bytes=256 segment-size=288 segment-align=8 source=compiler-policy-not-rustc-observation"
    ));
    assert!(!llvm.contains("rustc-observed evidence"));
    let target = fe2o3_kernel_ir::gfx942_xnack_minus_target_capability();
    let abi = frontend.capabilities();
    for capabilities in [
        &module.required_capabilities,
        &module.kernels[0].required_capabilities,
        &function.required_capabilities,
    ] {
        assert!(capabilities.contains(&target));
        assert!(abi.iter().all(|binding| capabilities.contains(binding)));
    }
}

#[test]
fn duplicate_matrix_current_acquisition_fails_closed() {
    let mut function = matrix_frontend_function();
    function.local_count = 8;
    function.locals.push(local(
        7,
        MirLocalRole::Temp,
        matrix_shape(TrustedDeviceItem::DeviceMatrix),
    ));
    function.blocks = vec![
        block(
            0,
            Vec::new(),
            call(TrustedDeviceItem::DeviceMatrixCurrent, Vec::new(), 4, 1),
        ),
        block(
            1,
            Vec::new(),
            call(TrustedDeviceItem::DeviceMatrixCurrent, Vec::new(), 7, 2),
        ),
        block(2, Vec::new(), MirTerminatorKind::Return),
    ];
    let error = translate_and_verify_for_target(
        &MirModule {
            functions: vec![function],
        },
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .expect_err("duplicate matrix authority");
    assert!(
        error
            .to_string()
            .contains("DeviceMatrix::current may be acquired only once per kernel invocation"),
        "{error}"
    );
}

#[test]
fn matrix_frontend_rejects_arity_argument_result_receiver_and_identity_substitutions() {
    let exact = matrix_frontend_function();

    let mut wrong_arity = exact.clone();
    matrix_call_mut(&mut wrong_arity).operands.pop();
    assert_matrix_error(wrong_arity, "expects 4 operand(s), found 3");

    let mut wrong_lhs = exact.clone();
    wrong_lhs.locals[1].ty = imported(matrix_shape(TrustedDeviceItem::F32AccumulatorFragment));
    assert_matrix_error(
        wrong_lhs,
        "matrix frontend ABI requires a kernel entry with exact",
    );

    let mut wrong_result = exact.clone();
    wrong_result.locals[6].ty = imported(matrix_shape(TrustedDeviceItem::Bf16MfmaFragment));
    assert_matrix_error(
        wrong_result,
        "matrix multiply-accumulate destination must have exact type",
    );

    let mut forged_receiver = exact.clone();
    matrix_call_mut(&mut forged_receiver).operands[0] = operand(1);
    assert_matrix_error(
        forged_receiver,
        "DeviceMatrix receiver must be an exact unprojected &DeviceMatrix",
    );

    let mut local_marker_spoof = exact;
    *matrix_call_mut(&mut local_marker_spoof).callee = Some(MirCallee::untrusted_for_test(
        TrustedDeviceItem::DeviceMatrixMultiplyAccumulate.canonical_path(),
    ));
    assert_matrix_error(
        local_marker_spoof,
        "local5 is a Rust aggregate, not one kernel IR value",
    );
}

#[test]
fn matrix_frontend_rejects_malformed_reference_propagation_and_receiver_places() {
    let mut mutable_destination = matrix_frontend_function();
    mutable_destination.locals[5].ty = imported(MirTypeShape::Reference {
        pointee: Box::new(matrix_shape(TrustedDeviceItem::DeviceMatrix)),
        mutable: true,
    });
    assert_matrix_error(
        mutable_destination,
        "reference borrow kind does not match the destination reference mutability preserved by Kernel IR",
    );

    let mut projected_destination = matrix_frontend_function();
    projected_destination.blocks[1].statements[0]
        .destination
        .as_mut()
        .unwrap()
        .projection
        .push(MirProjectionElem::Field(0));
    assert_matrix_error(
        projected_destination,
        "DeviceMatrix autoref requires an unprojected DeviceMatrix source and exact unprojected &DeviceMatrix destination",
    );

    let mut projected_source = matrix_frontend_function();
    let MirOperandRef::Place(source) = &mut projected_source.blocks[1].statements[0].operands[0]
    else {
        unreachable!()
    };
    source.projection.push(MirProjectionElem::Field(0));
    assert_matrix_error(
        projected_source,
        "DeviceMatrix autoref requires an unprojected DeviceMatrix source and exact unprojected &DeviceMatrix destination",
    );

    let mut projected_receiver = matrix_frontend_function();
    let MirOperandRef::Place(receiver) = &mut matrix_call_mut(&mut projected_receiver).operands[0]
    else {
        unreachable!()
    };
    receiver.projection.push(MirProjectionElem::Deref);
    assert_matrix_error(
        projected_receiver,
        "DeviceMatrix receiver must be an exact unprojected &DeviceMatrix",
    );
}

#[test]
fn matrix_frontend_rejects_missing_or_mutated_source_abi_and_projection() {
    let mut missing = matrix_frontend_function();
    missing.matrix_frontend_abi = None;
    assert_matrix_error(
        missing,
        "matrix fragment flattening requires a rustc-bound source ABI observation",
    );

    let mut layout = matrix_frontend_function();
    layout
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .observed_source
        .lhs_layout
        .size_bytes = 9;
    assert_matrix_error(layout, "source ABI observation digest mismatch");

    let mut provider_content = matrix_frontend_function();
    provider_content
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .observed_source
        .provider
        .crate_hash[0] ^= 1;
    assert_matrix_error(provider_content, "source ABI observation digest mismatch");

    let mut provider_source = matrix_frontend_function();
    provider_source
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .observed_source
        .provider
        .source_identity[0] ^= 1;
    assert_matrix_error(provider_source, "source ABI observation digest mismatch");

    let mut method_fn_abi = matrix_frontend_function();
    method_fn_abi
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .observed_source
        .method_abi
        .can_unwind = true;
    assert_matrix_error(method_fn_abi, "source ABI observation digest mismatch");

    let mut source_structure = matrix_frontend_function();
    source_structure
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .observed_source
        .method_source_structure[0] = crate::mir_import::MatrixSourceTypeRoleV2::Bf16Fragment;
    assert_matrix_error(source_structure, "source ABI observation digest mismatch");

    let mut kernarg = matrix_frontend_function();
    kernarg
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .projected_kernarg
        .parameters[8]
        .offset = 18;
    assert_matrix_error(kernarg, "projected kernarg policy differs");

    let mut kernarg_segment = matrix_frontend_function();
    kernarg_segment
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .projected_kernarg
        .kernarg_segment_size = 32;
    assert_matrix_error(kernarg_segment, "projected kernarg policy differs");

    let mut explicit_size = matrix_frontend_function();
    explicit_size
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .projected_kernarg
        .explicit_argument_size = 34;
    assert_matrix_error(explicit_size, "projected kernarg policy differs");

    let mut kernarg_alignment = matrix_frontend_function();
    kernarg_alignment
        .matrix_frontend_abi
        .as_mut()
        .unwrap()
        .projected_kernarg
        .kernarg_segment_alignment = 4;
    assert_matrix_error(kernarg_alignment, "projected kernarg policy differs");

    let mut digest = matrix_frontend_function();
    digest.matrix_frontend_abi.as_mut().unwrap().digest[0] ^= 1;
    assert_matrix_error(digest, "source ABI observation digest mismatch");

    let errors = translate_and_verify_for_target_with_policy(
        &MirModule {
            functions: vec![matrix_frontend_function()],
        },
        &AmdGpuTarget::new("gfx942:xnack-"),
        StrictFloatPolicy::CustomLlvmPipeline,
    )
    .expect_err("custom LLVM pipeline must not enter matrix lowering");
    assert!(
        errors
            .to_string()
            .contains("rejects custom -Cllvm-args and -Cpasses"),
        "{errors}"
    );
}

#[test]
fn exact_dialect_rejects_target_and_abi_binding_mutations() {
    let module = translate_and_verify_for_target(
        &MirModule {
            functions: vec![matrix_frontend_function()],
        },
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .expect("exact matrix frontend IR");
    let target = fe2o3_kernel_ir::gfx942_xnack_minus_target_capability();
    let abi = matrix_frontend_binding(&module.functions[0])
        .expect("matrix frontend binding")
        .capabilities();

    for owner in 0..3 {
        let mut mutated = module.clone();
        match owner {
            0 => {
                mutated.required_capabilities.remove(&target);
            }
            1 => {
                mutated.kernels[0].required_capabilities.remove(&target);
            }
            2 => {
                mutated.functions[0].required_capabilities.remove(&target);
            }
            _ => unreachable!(),
        }
        let errors = dialect_amdgcn::lower_kernel_to_gfx942_xnack_minus_llvm_ir(
            &mutated,
            &mutated.kernels[0].id,
        )
        .expect_err("missing exact target binding");
        assert!(errors.to_string().contains("requires"), "{errors}");
    }

    let mut wrong_abi = module;
    wrong_abi.kernels[0].required_capabilities.remove(&abi[0]);
    let errors = dialect_amdgcn::lower_kernel_to_gfx942_xnack_minus_llvm_ir(
        &wrong_abi,
        &wrong_abi.kernels[0].id,
    )
    .expect_err("missing source ABI binding");
    assert!(
        errors.to_string().contains("digests must be bound"),
        "{errors}"
    );
}

#[test]
fn matrix_frontend_rejects_every_non_exact_target_and_non_kernel_placement() {
    for target in [
        "gfx942",
        "gfx942:xnack+",
        "gfx942:sramecc+:xnack-",
        "gfx942:xnack-:sramecc+",
        "gfx941:xnack-",
        "gfx950:xnack-",
        "gfx1100",
    ] {
        let errors = translate_and_verify_for_target(
            &MirModule {
                functions: vec![matrix_frontend_function()],
            },
            &AmdGpuTarget::new(target),
        )
        .expect_err("non-exact matrix target");
        assert!(
            errors
                .to_string()
                .contains("requires the exact gfx942:xnack- one-wave 64x1x1 kernel context"),
            "target {target}: {errors}"
        );
    }

    let mut helper = matrix_frontend_function();
    helper.kind = MirFunctionKind::InternalHelper;
    let errors = translate_and_verify_for_target(
        &MirModule {
            functions: vec![helper],
        },
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .expect_err("matrix operation in a non-kernel helper");
    assert!(
        errors
            .to_string()
            .contains("matrix frontend ABI requires a kernel entry")
    );

    let errors = translate_and_verify_for_target(
        &MirModule {
            functions: vec![matrix_frontend_function_with_workgroup(128)],
        },
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .expect_err("two-wave matrix workgroup");
    assert!(
        errors
            .to_string()
            .contains("one-wave 64x1x1 kernel context")
    );
}

#[test]
fn divergent_matrix_placement_is_rejected_by_the_existing_amdgcn_validator() {
    let mut module = translate_and_verify_for_target(
        &MirModule {
            functions: vec![matrix_frontend_function()],
        },
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .expect("exact matrix frontend IR");
    let abi = matrix_frontend_binding(&module.functions[0])
        .expect("matrix frontend binding")
        .capabilities();
    for capability in &abi {
        module.required_capabilities.remove(capability);
        module.kernels[0].required_capabilities.remove(capability);
    }
    let function = &mut module.functions[0];
    for capability in &abi {
        function.required_capabilities.remove(capability);
    }
    let body = function.body.as_mut().unwrap();
    for operation in body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::Matrix(matrix) = &mut operation.kind {
            matrix.frontend_binding = None;
        }
    }
    let matrix_operations = std::mem::take(&mut body.blocks[1].operations);
    body.blocks[1].operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(16), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(17), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(18), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::NotEqual,
                lhs: ValueId(16),
                rhs: ValueId(17),
            },
        ),
    ]);
    body.blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(18),
        then_target: BlockId(3),
        then_arguments: Vec::new(),
        else_target: BlockId(2),
        else_arguments: Vec::new(),
    });
    let mut divergent = BasicBlock::new(BlockId(3));
    divergent.operations = matrix_operations;
    divergent.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: Vec::new(),
    });
    body.blocks.push(divergent);

    let errors =
        dialect_amdgcn::lower_kernel_to_gfx942_xnack_minus_llvm_ir(&module, &module.kernels[0].id)
            .expect_err("divergent matrix placement");
    assert!(
        errors.to_string().contains("convergent operation requires"),
        "{errors}"
    );
}

fn matrix_frontend_function() -> MirFunction {
    matrix_frontend_function_with_workgroup(64)
}

fn matrix_frontend_function_with_workgroup(workgroup_x: u32) -> MirFunction {
    let dimensions = FrontendWorkgroupDimensionsV1::new([workgroup_x, 1, 1]).unwrap();
    let launch = FrontendLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let frontend_contract = Some(
        crate::collector::AuthenticatedKernelFrontendContractV1::for_test(
            KernelFrontendContractV1::new(Some(launch), None).unwrap(),
        ),
    );
    let matrix = matrix_shape(TrustedDeviceItem::DeviceMatrix);
    MirFunction {
        semantic_instance: None,
        export_name: "tiled_gemm_frontend_v1".to_string(),
        rust_path: "tests::tiled_gemm_frontend_v1".to_string(),
        kind: MirFunctionKind::KernelEntry,
        typed_profile: None,
        arg_count: 3,
        local_count: 7,
        locals: vec![
            local(0, MirLocalRole::Return, MirTypeShape::Unit),
            local(
                1,
                MirLocalRole::Arg,
                matrix_shape(TrustedDeviceItem::Bf16MfmaFragment),
            ),
            local(
                2,
                MirLocalRole::Arg,
                matrix_shape(TrustedDeviceItem::Bf16MfmaFragment),
            ),
            local(
                3,
                MirLocalRole::Arg,
                matrix_shape(TrustedDeviceItem::F32AccumulatorFragment),
            ),
            local(4, MirLocalRole::Temp, matrix.clone()),
            local(
                5,
                MirLocalRole::Temp,
                MirTypeShape::Reference {
                    pointee: Box::new(matrix),
                    mutable: false,
                },
            ),
            local(
                6,
                MirLocalRole::Temp,
                matrix_shape(TrustedDeviceItem::F32AccumulatorFragment),
            ),
        ],
        blocks: vec![
            block(
                0,
                Vec::new(),
                call(TrustedDeviceItem::DeviceMatrixCurrent, Vec::new(), 4, 1),
            ),
            block(
                1,
                vec![assign(
                    0,
                    place(5),
                    vec![operand(4)],
                    MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::Shared),
                )],
                call(
                    TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
                    vec![operand(5), operand(1), operand(2), operand(3)],
                    6,
                    2,
                ),
            ),
            block(2, Vec::new(), MirTerminatorKind::Return),
        ],
        frontend_contract,
        matrix_frontend_abi: Some(crate::mir_import::MatrixFrontendAbiV2::canonical_for_test()),
    }
}

fn matrix_shape(item: TrustedDeviceItem) -> MirTypeShape {
    MirTypeShape::Adt {
        identity: item.canonical_path().to_string(),
    }
}

struct MatrixCallMut<'a> {
    callee: &'a mut Option<MirCallee>,
    operands: &'a mut Vec<MirOperandRef>,
}

fn matrix_call_mut(function: &mut MirFunction) -> MatrixCallMut<'_> {
    let MirTerminatorKind::Call {
        callee, operands, ..
    } = &mut function.blocks[1]
        .terminator
        .as_mut()
        .expect("matrix call")
        .kind
    else {
        unreachable!("matrix fixture block must end in a call")
    };
    MatrixCallMut { callee, operands }
}

fn assert_matrix_error(function: MirFunction, expected: &str) {
    let errors = translate_and_verify_for_target(
        &MirModule {
            functions: vec![function],
        },
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .expect_err("matrix substitution must fail closed");
    assert!(errors.to_string().contains(expected), "{errors}");
}

#[derive(Debug)]
struct S09SealedOwnerPathFixture {
    kernel_binding: KernelBindingIdV1,
    observed_symbol: String,
    rust_path: String,
}

#[test]
fn exact_alpha_and_zeta_bodies_lower_together() {
    let module = lower_general_v3(&alpha_zeta_module()).expect("alpha/zeta module");

    assert_eq!(
        module
            .kernels
            .iter()
            .map(|kernel| kernel.id.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert_eq!(
        module.functions.len(),
        2,
        "trusted helpers must be semantic operations"
    );
    assert!(
        module
            .functions
            .iter()
            .all(|function| function.body.is_some())
    );
    assert!(module.functions.iter().all(|function| {
        !function.id.as_str().contains("DisjointSlice::<T>")
            && !function.id.as_str().contains("ThreadIndex::get")
    }));

    let alpha = function(&module, "tests::alpha");
    assert_eq!(
        alpha.signature.parameters,
        vec![
            Type::F32,
            slice(AccessMode::ReadOnly),
            slice(AccessMode::ReadWrite),
        ]
    );
    let zeta = function(&module, "tests::zeta");
    assert_eq!(
        zeta.signature.parameters,
        vec![
            slice(AccessMode::ReadOnly),
            slice(AccessMode::ReadOnly),
            Type::F32,
            slice(AccessMode::ReadWrite),
        ]
    );
}

#[test]
fn s09_alpha_requires_exact_guarded_cfg_and_dataflow() {
    let sealed_owner = s09_sealed_owner_path_fixture("current-build-observation");
    let exact = s09_alpha(&sealed_owner.rust_path);
    crate::source_debug::validate_alpha_mir_body(&exact, &sealed_owner.rust_path)
        .expect("exact S09 alpha MIR");

    let stale_binding = KernelBindingIdV1::from_bytes([0x5a; 32]);
    let mut stale_build_observation = exact.clone();
    stale_build_observation.rust_path = format!(
        "{S09_CRATE_NAME}::{S09_MODULE_PATH}::{}",
        host_kernel_symbol_v1(stale_binding)
    );
    assert!(
        crate::source_debug::validate_alpha_mir_body(
            &stale_build_observation,
            &sealed_owner.rust_path,
        )
        .is_err(),
        "stale synthetic S09 build binding was admitted"
    );

    let mut wrong_owner = exact.clone();
    wrong_owner.rust_path = format!(
        "synthetic_wrong_owner::{S09_MODULE_PATH}::{}",
        sealed_owner.observed_symbol
    );
    assert!(
        crate::source_debug::validate_alpha_mir_body(&wrong_owner, &sealed_owner.rust_path)
            .is_err(),
        "synthetic wrong S09 owner was admitted"
    );

    let mut disconnected_guard = exact.clone();
    let MirTerminatorKind::Assert { condition, .. } = &mut disconnected_guard.blocks[4]
        .terminator
        .as_mut()
        .expect("guard terminator")
        .kind
    else {
        panic!("S09 guard assert")
    };
    *condition = operand(5);
    assert!(
        crate::source_debug::validate_alpha_mir_body(&disconnected_guard, &sealed_owner.rust_path)
            .is_err()
    );

    let mut wrong_store = exact.clone();
    wrong_store.blocks[5].statements[1].destination = Some(MirPlaceRef {
        local: 8,
        projection: vec![MirProjectionElem::Deref],
        semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
    });
    assert!(
        crate::source_debug::validate_alpha_mir_body(&wrong_store, &sealed_owner.rust_path)
            .is_err()
    );

    let mut alternate_output = exact.clone();
    let MirTerminatorKind::Call { operands, .. } = &mut alternate_output.blocks[2]
        .terminator
        .as_mut()
        .expect("get-mut terminator")
        .kind
    else {
        panic!("S09 get-mut call")
    };
    operands[0] = operand(3);
    assert!(
        crate::source_debug::validate_alpha_mir_body(&alternate_output, &sealed_owner.rust_path)
            .is_err()
    );
}

#[test]
fn s09_dynamic_build_observations_do_not_change_stable_admission() {
    let first_owner = s09_sealed_owner_path_fixture("build-observation-a");
    let second_owner = s09_sealed_owner_path_fixture("build-observation-b");
    assert_ne!(first_owner.kernel_binding, second_owner.kernel_binding);
    assert_ne!(first_owner.observed_symbol, second_owner.observed_symbol);
    assert_ne!(first_owner.rust_path, second_owner.rust_path);

    let first = s09_alpha(&first_owner.rust_path);
    let second = s09_alpha(&second_owner.rust_path);
    crate::source_debug::validate_alpha_mir_body(&first, &first_owner.rust_path)
        .expect("first sealed owner path");
    crate::source_debug::validate_alpha_mir_body(&second, &second_owner.rust_path)
        .expect("second sealed owner path");

    assert_eq!(
        s09_stable_admission_sha256(first),
        s09_stable_admission_sha256(second),
        "build-specific kernel bindings and observed symbols must remain outside stable admission"
    );
}

#[test]
fn alpha_zeta_share_semantic_helper_lowering_and_emit_fmul_fadd() {
    let module = lower_general_v3(&alpha_zeta_module()).expect("alpha/zeta module");
    for rust_path in ["tests::alpha", "tests::zeta"] {
        let operations = operations(function(&module, rust_path));
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::Intrinsic(_)))
                .count(),
            1,
            "{rust_path}"
        );
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
                .count(),
            0,
            "{rust_path}"
        );
        assert!(operations.iter().any(|operation| matches!(
            operation.kind,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                ..
            }
        )));
    }

    let llvm = dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(&module)
        .expect("gfx942 compiler-module LLVM");
    assert!(llvm.contains("fmul float"), "{llvm}");
    assert_eq!(llvm.matches("fadd float").count(), 2, "{llvm}");
    assert!(!llvm.contains("DisjointSlice::<T>::get_mut"), "{llvm}");
}

#[test]
fn alpha_multiply_may_write_the_guarded_payload_directly() {
    let mut alpha = alpha();
    let arithmetic = &mut alpha.blocks[4].statements;
    arithmetic[1].destination = Some(MirPlaceRef {
        local: 8,
        projection: vec![MirProjectionElem::Deref],
        semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
    });
    arithmetic.remove(2);

    let module = lower_general_v3(&MirModule {
        functions: vec![alpha],
    })
    .expect("direct guarded f32 store");
    let operations = operations(function(&module, "tests::alpha"));
    assert!(operations.iter().any(|operation| matches!(
        operation.kind,
        OperationKind::Binary {
            op: BinaryOp::Multiply,
            ..
        }
    )));
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Store { .. }))
    );
}

#[test]
fn general_v3_multiply_claim_requires_imported_f32_operands() {
    let mut module = alpha_zeta_module();
    let loaded = module.functions[0]
        .locals
        .iter_mut()
        .find(|local| local.index == 10)
        .expect("alpha loaded value");
    loaded.ty = imported(MirTypeShape::U32);

    let errors = lower_general_v3(&module).expect_err("non-f32 multiply must remain unowned");

    assert!(errors.contains(TranslationDiagnosticCode::UnsupportedRvalue));
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("f32 multiply requires an exact General V3 alpha/zeta kernel context")
    }));
}

#[test]
fn gfx942_target_id_feature_states_preserve_the_float_profile() {
    translate_and_verify_for_target(&alpha_zeta_module(), &AmdGpuTarget::new("gfx942:xnack-"))
        .expect("canonical gfx942 target IDs must retain the gfx942 floating-point profile");
}

#[test]
fn general_v3_rejects_wrong_index_untrusted_callee_and_wrong_profile() {
    let mut wrong_index = alpha_zeta_module();
    let get_mut = &mut wrong_index.functions[0].blocks[1]
        .terminator
        .as_mut()
        .expect("get_mut terminator")
        .kind;
    let MirTerminatorKind::Call { operands, .. } = get_mut else {
        panic!("get_mut call")
    };
    operands[1] = usize_constant(0);
    let errors = lower_general_v3(&wrong_index).expect_err("untrusted index provenance");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("did not originate from trusted thread::index_1d")
    }));

    let mut lookalike = alpha_zeta_module();
    let get_mut = &mut lookalike.functions[0].blocks[1]
        .terminator
        .as_mut()
        .expect("get_mut terminator")
        .kind;
    let MirTerminatorKind::Call { callee, .. } = get_mut else {
        panic!("get_mut call")
    };
    *callee = Some(MirCallee::untrusted_for_test(
        TrustedDeviceItem::DisjointSliceGetMut.canonical_path(),
    ));
    let errors = lower_general_v3(&lookalike).expect_err("callee spelling is not authority");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no classified trusted device identity")
    }));

    let mut renamed = alpha_zeta_module();
    renamed.functions[0].export_name = "alpha_lookalike".to_string();
    let errors = lower_general_v3(&renamed).expect_err("kernel name is part of the exact slice");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an exact General V3 alpha/zeta kernel context")
    }));

    let mut untyped = alpha_zeta_module();
    for function in &mut untyped.functions {
        function.typed_profile = None;
    }
    let errors = translate_and_verify_for_target(&untyped, &AmdGpuTarget::new("gfx942"))
        .expect_err("f32 multiply requires exact General V3 context");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an exact General V3 alpha/zeta kernel context")
    }));

    let errors =
        translate_and_verify_for_target(&alpha_zeta_module(), &AmdGpuTarget::new("gfx1100"))
            .expect_err("f32 arithmetic profile must be exact");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires the exact gfx942 floating-point profile")
    }));

    let zeta_only = MirModule {
        functions: vec![zeta()],
    };
    let errors = translate_and_verify_for_target(&zeta_only, &AmdGpuTarget::new("gfx1100"))
        .expect_err("zeta addition must independently require gfx942");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("f32 addition requires the exact gfx942 floating-point profile")
    }));
    let errors = translate_and_verify_with_float_target(
        &zeta_only,
        Some(Gfx942FloatTarget),
        StrictFloatPolicy::CustomLlvmPipeline,
    )
    .expect_err("zeta addition must independently reject custom LLVM pipelines");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rejects custom -Cllvm-args and -Cpasses")
    }));
}

#[test]
fn general_v3_thread_index_spelling_does_not_cross_the_extension_boundary() {
    let mut lookalike = MirModule {
        functions: vec![alpha()],
    };
    let index_call = &mut lookalike.functions[0].blocks[0]
        .terminator
        .as_mut()
        .expect("thread-index terminator")
        .kind;
    let MirTerminatorKind::Call { callee, .. } = index_call else {
        panic!("thread-index call")
    };
    *callee = Some(MirCallee::untrusted_for_test(
        TrustedDeviceItem::ThreadIndex1d.canonical_path(),
    ));

    let errors = lower_general_v3(&lookalike).expect_err("callee spelling is not authority");

    assert!(errors.contains(TranslationDiagnosticCode::UnsupportedCall));
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no classified trusted device identity")
    }));
}

#[test]
fn option_payload_cannot_escape_the_bounds_checked_some_region() {
    let mut false_to_store = MirModule {
        functions: vec![alpha()],
    };
    let switch = &mut false_to_store.functions[0].blocks[2]
        .terminator
        .as_mut()
        .expect("Option switch")
        .kind;
    let MirTerminatorKind::SwitchInt { targets, .. } = switch else {
        panic!("Option switch")
    };
    targets
        .iter_mut()
        .find(|target| target.value == 0)
        .expect("None edge")
        .target = 4;
    let errors = lower_general_v3(&false_to_store).expect_err("false edge reaches store");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Option payload alias escapes the bounds-checked Some region")
    }));

    let mut merged_edges = MirModule {
        functions: vec![alpha()],
    };
    let switch = &mut merged_edges.functions[0].blocks[2]
        .terminator
        .as_mut()
        .expect("Option switch")
        .kind;
    let MirTerminatorKind::SwitchInt { targets, .. } = switch else {
        panic!("Option switch")
    };
    let some_target = targets
        .iter()
        .find(|target| target.value == 1)
        .expect("Some edge")
        .target;
    targets
        .iter_mut()
        .find(|target| target.value == 0)
        .expect("None edge")
        .target = some_target;
    let errors = lower_general_v3(&merged_edges).expect_err("Some and None edges must differ");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("boolean switch must have exact 0/1 cases")
    }));
}

#[test]
fn gfx942_diagnostic_items_lower_to_closed_ir_contracts() {
    let module = lower_general_v3(&MirModule {
        functions: vec![diagnostics_alpha()],
    })
    .expect("bounded diagnostics kernel");
    let kernel = function(&module, "tests::alpha");
    let operations = operations(kernel);
    let assemblies = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::InlineAssembly(assembly) => Some(assembly),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(assemblies.len(), 6);
    assert!(assemblies.iter().all(|assembly| {
        assembly.target == fe2o3_kernel_ir::InlineAssemblyTarget::AmdGpuGfx942
            && assembly.source.is_complete()
            && assembly.declared_effects.is_empty()
            && assembly
                .options
                .contains(&fe2o3_kernel_ir::AssemblyOption::NoMemory)
            && assembly
                .options
                .contains(&fe2o3_kernel_ir::AssemblyOption::Pure)
    }));
    assert_eq!(
        assemblies
            .iter()
            .map(|assembly| assembly.mnemonic.as_str())
            .collect::<Vec<_>>(),
        [
            "v_mov_b32",
            "v_add_u32",
            "v_sub_u32",
            "v_and_b32",
            "v_or_b32",
            "v_xor_b32",
        ]
    );

    let diagnostics = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call { callee, arguments } => {
                AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 6);
    assert!(matches!(
        diagnostics.as_slice(),
        [
            AmdGpuDiagnosticOperation::Clock32,
            AmdGpuDiagnosticOperation::Print { .. },
            AmdGpuDiagnosticOperation::ProfilingMarker { .. },
            AmdGpuDiagnosticOperation::DebugTrap,
            AmdGpuDiagnosticOperation::AssertFail { .. },
            AmdGpuDiagnosticOperation::Trap,
        ]
    ));
    let assert_fail_block = kernel
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| {
            block.operations.last().is_some_and(|operation| {
                matches!(
                    &operation.kind,
                    OperationKind::Call { callee, arguments }
                        if matches!(
                            AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                            Some(AmdGpuDiagnosticOperation::AssertFail { .. })
                        )
                )
            })
        })
        .unwrap();
    assert!(matches!(
        assert_fail_block.terminator,
        Some(fe2o3_kernel_ir::Terminator::Unreachable)
    ));
    let trap_block = kernel
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| {
            block.operations.last().is_some_and(|operation| {
                matches!(
                    &operation.kind,
                    OperationKind::Call { callee, arguments }
                        if matches!(
                            AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                            Some(AmdGpuDiagnosticOperation::Trap)
                        )
                )
            })
        })
        .unwrap();
    assert_eq!(trap_block.operations.len(), 1);
    assert!(matches!(
        trap_block.terminator,
        Some(fe2o3_kernel_ir::Terminator::Unreachable)
    ));
    assert_eq!(
        module
            .functions
            .iter()
            .filter(|function| {
                AmdGpuDiagnosticOperation::from_intrinsic_id(&function.id).is_some()
                    && function.body.is_none()
            })
            .count(),
        6
    );
    assert!(
        module
            .required_capabilities
            .contains(&TargetCapability::Extension {
                namespace: fe2o3_kernel_ir::AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE
                    .to_owned(),
                name: fe2o3_kernel_ir::AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME.to_owned(),
            })
    );
    assert!(
        module
            .required_capabilities
            .contains(&TargetCapability::Extension {
                namespace: fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                    .to_owned(),
                name: fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
            })
    );
}

#[test]
fn gfx942_diagnostic_calls_forward_promoted_ssa_values() {
    let mut fixture = diagnostics_alpha();
    fixture
        .locals
        .push(local(19, MirLocalRole::Temp, MirTypeShape::U32));
    fixture.locals.sort_by_key(|local| local.index);
    fixture.local_count = fixture.locals.len();

    fixture
        .blocks
        .iter_mut()
        .find(|block| block.index == 7)
        .unwrap()
        .statements
        .push(assign(
            0,
            place(19),
            vec![u32_constant(9)],
            MirRvalueKind::Use,
        ));
    let MirTerminatorKind::Call { operands, .. } = &mut fixture
        .blocks
        .iter_mut()
        .find(|block| block.index == 8)
        .unwrap()
        .terminator
        .as_mut()
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    operands[1] = operand(19);
    fixture
        .blocks
        .iter_mut()
        .find(|block| block.index == 9)
        .unwrap()
        .statements
        .push(assign(
            0,
            place(19),
            vec![u32_constant(11)],
            MirRvalueKind::Use,
        ));

    let module = lower_general_v3(&MirModule {
        functions: vec![fixture],
    })
    .expect("diagnostic call must preserve live control-flow values");
    let body = function(&module, "tests::alpha")
        .body
        .as_ref()
        .expect("kernel body");
    let source = body.blocks.iter().find(|block| block.id.0 == 7).unwrap();
    let target = body.blocks.iter().find(|block| block.id.0 == 8).unwrap();
    let fe2o3_kernel_ir::Terminator::Branch { arguments, .. } = source.terminator.as_ref().unwrap()
    else {
        unreachable!()
    };
    assert_eq!(arguments.len(), 1);
    assert_eq!(target.parameters.len(), 1);
}

#[test]
fn gfx942_diagnostic_admission_fails_closed() {
    let mut missing_contract = diagnostics_alpha();
    missing_contract.frontend_contract = None;
    let errors = lower_general_v3(&MirModule {
        functions: vec![missing_contract],
    })
    .expect_err("inline operation without authenticated frontend contract");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an authenticated frontend contract")
    }));

    let mut missing_source = diagnostics_alpha();
    missing_source
        .blocks
        .iter_mut()
        .find(|block| block.index == 7)
        .unwrap()
        .terminator
        .as_mut()
        .unwrap()
        .source = None;
    let errors = lower_general_v3(&MirModule {
        functions: vec![missing_source],
    })
    .expect_err("inline operation without source location");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires a concrete source location")
    }));

    let mut wrong_target = diagnostics_alpha();
    wrong_target
        .blocks
        .iter_mut()
        .find(|block| block.index == 4)
        .unwrap()
        .statements
        .clear();
    let errors = translate_and_verify_for_target(
        &MirModule {
            functions: vec![wrong_target],
        },
        &AmdGpuTarget::new("gfx90a"),
    )
    .expect_err("diagnostics on a non-gfx942 target");
    assert!(
        errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("requires the exact gfx942 target")
        }),
        "{errors}"
    );

    let mut wrong_type = diagnostics_alpha();
    let MirTerminatorKind::Call { operands, .. } = &mut wrong_type
        .blocks
        .iter_mut()
        .find(|block| block.index == 7)
        .unwrap()
        .terminator
        .as_mut()
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    operands[0] = operand(1);
    let errors = lower_general_v3(&MirModule {
        functions: vec![wrong_type],
    })
    .expect_err("f32 passed to a typed u32 inline operation");
    assert!(
        errors
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("operand 0 must lower to u32") })
    );

    let mut lookalike = diagnostics_alpha();
    let MirTerminatorKind::Call { callee, .. } = &mut lookalike
        .blocks
        .iter_mut()
        .find(|block| block.index == 7)
        .unwrap()
        .terminator
        .as_mut()
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    *callee = Some(MirCallee::untrusted_for_test(
        TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VMovB32).canonical_path(),
    ));
    let errors = lower_general_v3(&MirModule {
        functions: vec![lookalike],
    })
    .expect_err("path lookalike without diagnostic-item identity");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no classified trusted device identity")
    }));
}

fn diagnostics_alpha() -> MirFunction {
    let mut function = alpha();
    let dimensions = FrontendWorkgroupDimensionsV1::new([256, 1, 1]).unwrap();
    let launch = FrontendLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    function.frontend_contract = Some(
        crate::collector::AuthenticatedKernelFrontendContractV1::for_test(
            KernelFrontendContractV1::new(Some(launch), None).unwrap(),
        ),
    );
    for index in 12..=18 {
        function
            .locals
            .push(local(index, MirLocalRole::Temp, MirTypeShape::U32));
    }
    function.locals.sort_by_key(|local| local.index);
    function.local_count = function.locals.len();
    function.blocks[4].terminator = Some(MirTerminator {
        kind: call(
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Clock32),
            Vec::new(),
            12,
            7,
        ),
        source: None,
    });
    function.blocks.extend([
        diagnostic_call_block(
            7,
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VMovB32),
            vec![operand(12)],
            13,
            8,
            true,
        ),
        diagnostic_call_block(
            8,
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VAddU32),
            vec![operand(13), operand(12)],
            14,
            9,
            true,
        ),
        diagnostic_call_block(
            9,
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VSubU32),
            vec![operand(14), operand(12)],
            15,
            10,
            true,
        ),
        diagnostic_call_block(
            10,
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VAndB32),
            vec![operand(15), operand(12)],
            16,
            11,
            true,
        ),
        diagnostic_call_block(
            11,
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VOrB32),
            vec![operand(16), operand(12)],
            17,
            12,
            true,
        ),
        diagnostic_call_block(
            12,
            TrustedDeviceItem::AmdGpuInline(TrustedAmdGpuInlineOperation::VXorB32),
            vec![operand(17), operand(12)],
            18,
            13,
            true,
        ),
        diagnostic_call_block(
            13,
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Print2),
            vec![u32_constant(0x1234_5678), operand(12), operand(18)],
            0,
            14,
            false,
        ),
        diagnostic_call_block(
            14,
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::ProfilingMarker),
            vec![u32_constant(73)],
            0,
            15,
            false,
        ),
        diagnostic_call_block(
            15,
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::DebugTrap),
            Vec::new(),
            0,
            16,
            false,
        ),
        diagnostic_call_block(
            16,
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::AssertFail),
            vec![u32_constant(0x7654_3210), u32_constant(41)],
            0,
            17,
            false,
        ),
        diagnostic_call_block(
            17,
            TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
            Vec::new(),
            0,
            5,
            false,
        ),
    ]);
    function
}

fn diagnostic_call_block(
    index: usize,
    item: TrustedDeviceItem,
    operands: Vec<MirOperandRef>,
    destination: usize,
    target: usize,
    with_source: bool,
) -> MirBlock {
    MirBlock {
        index,
        statements: Vec::new(),
        terminator: Some(MirTerminator {
            kind: call(item, operands, destination, target),
            source: with_source.then(|| MirSourceLocation {
                file: "tests/diagnostics.rs".to_owned(),
                line: index + 10,
                column: 5,
            }),
        }),
    }
}

fn u32_constant(value: u32) -> MirOperandRef {
    MirOperandRef::Constant {
        ty: imported(MirTypeShape::U32),
        literal: MirConstant::U32(value),
        value: value.to_string(),
    }
}

fn lower_general_v3(module: &MirModule) -> Result<Module, TranslationErrors> {
    translate_and_verify_for_target(module, &AmdGpuTarget::new("gfx942"))
}

fn alpha_zeta_module() -> MirModule {
    MirModule {
        functions: vec![alpha(), zeta()],
    }
}

fn alpha() -> MirFunction {
    kernel(
        "alpha",
        vec![
            local(1, MirLocalRole::Arg, MirTypeShape::F32),
            local(2, MirLocalRole::Arg, slice_shape(false)),
            local(3, MirLocalRole::Arg, disjoint_shape()),
        ],
        4,
        vec![
            local(10, MirLocalRole::Temp, MirTypeShape::F32),
            local(11, MirLocalRole::Temp, MirTypeShape::F32),
        ],
        vec![
            assign(0, place(10), vec![indexed(2, 9)], MirRvalueKind::Use),
            assign(
                1,
                place(11),
                vec![operand(10), operand(1)],
                MirRvalueKind::Binary(MirBinaryOp::Mul),
            ),
            store(2, 8, 11),
        ],
    )
}

fn s09_alpha(rust_path: &str) -> MirFunction {
    let locals = vec![
        local(0, MirLocalRole::Return, MirTypeShape::Unit),
        local(1, MirLocalRole::Arg, MirTypeShape::F32),
        local(2, MirLocalRole::Arg, slice_shape(false)),
        local(3, MirLocalRole::Arg, disjoint_shape()),
        local(4, MirLocalRole::Temp, thread_index_shape()),
        local(5, MirLocalRole::Temp, MirTypeShape::USize),
        local(
            6,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(thread_index_shape()),
                mutable: false,
            },
        ),
        local(
            7,
            MirLocalRole::Temp,
            MirTypeShape::Adt {
                identity: "std::option::Option".to_owned(),
            },
        ),
        local(
            8,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(disjoint_shape()),
                mutable: true,
            },
        ),
        local(9, MirLocalRole::Temp, MirTypeShape::ISize),
        local(
            10,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(MirTypeShape::F32),
                mutable: true,
            },
        ),
        local(11, MirLocalRole::Temp, MirTypeShape::F32),
        local(12, MirLocalRole::Temp, MirTypeShape::USize),
        local(13, MirLocalRole::Temp, MirTypeShape::Bool),
    ];
    MirFunction {
        semantic_instance: None,
        export_name: "alpha".to_owned(),
        rust_path: rust_path.to_owned(),
        kind: MirFunctionKind::KernelEntry,
        typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
        arg_count: 3,
        local_count: 14,
        locals,
        blocks: vec![
            block(
                0,
                vec![],
                call(TrustedDeviceItem::ThreadIndex1d, vec![], 4, 1),
            ),
            block(
                1,
                vec![assign(
                    0,
                    place(6),
                    vec![operand(4)],
                    MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::Shared),
                )],
                call(TrustedDeviceItem::ThreadIndexGet, vec![operand(6)], 5, 2),
            ),
            block(
                2,
                vec![assign(
                    0,
                    place(8),
                    vec![operand(3)],
                    MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::MutableDefault),
                )],
                call(
                    TrustedDeviceItem::DisjointSliceGetMut,
                    vec![operand(8), operand(4)],
                    7,
                    3,
                ),
            ),
            block(
                3,
                vec![assign(
                    0,
                    place(9),
                    vec![operand(7)],
                    MirRvalueKind::Discriminant,
                )],
                MirTerminatorKind::SwitchInt {
                    discriminant: operand(9),
                    targets: vec![
                        MirSwitchTarget {
                            value: 1,
                            target: 4,
                        },
                        MirSwitchTarget {
                            value: 0,
                            target: 6,
                        },
                    ],
                    otherwise: 7,
                },
            ),
            block(
                4,
                vec![
                    assign(
                        0,
                        place(10),
                        vec![MirOperandRef::Place(MirPlaceRef {
                            local: 7,
                            projection: vec![
                                MirProjectionElem::Downcast { variant: 1 },
                                MirProjectionElem::Field(0),
                            ],
                            semantic_identity:
                                crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
                        })],
                        MirRvalueKind::Use,
                    ),
                    assign(
                        1,
                        place(12),
                        vec![operand(2)],
                        MirRvalueKind::Unary(MirUnaryOp::PtrMetadata),
                    ),
                    assign(
                        2,
                        place(13),
                        vec![operand(5), operand(12)],
                        MirRvalueKind::Binary(MirBinaryOp::Lt),
                    ),
                ],
                MirTerminatorKind::Assert {
                    condition: operand(13),
                    expected: true,
                    target: 5,
                },
            ),
            block(
                5,
                vec![
                    assign(0, place(11), vec![indexed(2, 5)], MirRvalueKind::Use),
                    assign(
                        1,
                        MirPlaceRef {
                            local: 10,
                            projection: vec![MirProjectionElem::Deref],
                            semantic_identity:
                                crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
                        },
                        vec![operand(11), operand(1)],
                        MirRvalueKind::Binary(MirBinaryOp::Mul),
                    ),
                ],
                MirTerminatorKind::Goto { target: 6 },
            ),
            block(6, vec![], MirTerminatorKind::Return),
            block(7, vec![], MirTerminatorKind::Unreachable),
        ],
        frontend_contract: None,
        matrix_frontend_abi: None,
    }
}

fn s09_sealed_owner_path_fixture(metadata: &str) -> S09SealedOwnerPathFixture {
    let crate_binding = derive_crate_binding_id_v1(S09_CRATE_NAME, [metadata]);
    let kernel_binding = derive_kernel_binding_id_v1(
        crate_binding,
        TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
        S09_LOGICAL_NAME,
        S09_EXPORT_NAME,
    );
    let observed_symbol = host_kernel_symbol_v1(kernel_binding);
    let rust_path = format!("{S09_CRATE_NAME}::{S09_MODULE_PATH}::{observed_symbol}");
    S09SealedOwnerPathFixture {
        kernel_binding,
        observed_symbol,
        rust_path,
    }
}

fn s09_stable_admission_sha256(function: MirFunction) -> [u8; 32] {
    let target = TargetIdentity::new(
        IdentityText::new("amdgcn-amd-amdhsa").unwrap(),
        IdentityText::new("gfx942:xnack-").unwrap(),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::Atomics, Capability::AmdWave],
    )
    .unwrap();
    let abi = AbiLayout::new(0, 1, PointerWidth::Bits64, Vec::new()).unwrap();
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(u32::MAX, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    let module = MirModule {
        functions: vec![function],
    };
    *module
        .portable_semantic_digest_v2(MirSemanticAdmissionInputsV2::new(
            S09_EXPORT_NAME,
            &target,
            &abi,
            &launch,
        ))
        .expect("S09 portable semantic admission")
        .as_bytes()
}

fn zeta() -> MirFunction {
    kernel(
        "zeta",
        vec![
            local(1, MirLocalRole::Arg, slice_shape(false)),
            local(2, MirLocalRole::Arg, slice_shape(false)),
            local(3, MirLocalRole::Arg, MirTypeShape::F32),
            local(4, MirLocalRole::Arg, disjoint_shape()),
        ],
        5,
        vec![
            local(11, MirLocalRole::Temp, MirTypeShape::F32),
            local(12, MirLocalRole::Temp, MirTypeShape::F32),
            local(13, MirLocalRole::Temp, MirTypeShape::F32),
            local(14, MirLocalRole::Temp, MirTypeShape::F32),
        ],
        vec![
            assign(0, place(11), vec![indexed(1, 10)], MirRvalueKind::Use),
            assign(1, place(12), vec![indexed(2, 10)], MirRvalueKind::Use),
            assign(
                2,
                place(13),
                vec![operand(11), operand(12)],
                MirRvalueKind::Binary(MirBinaryOp::Add),
            ),
            assign(
                3,
                place(14),
                vec![operand(13), operand(3)],
                MirRvalueKind::Binary(MirBinaryOp::Add),
            ),
            store(4, 9, 14),
        ],
    )
}

fn kernel(
    name: &str,
    arguments: Vec<MirLocal>,
    index_local: usize,
    arithmetic_locals: Vec<MirLocal>,
    arithmetic: Vec<MirStatement>,
) -> MirFunction {
    let output_local = arguments.last().expect("output argument").index;
    let output_ref = index_local + 1;
    let option = index_local + 2;
    let discriminant = index_local + 3;
    let payload = index_local + 4;
    let linear_index = index_local + 5;
    let mut locals = vec![local(0, MirLocalRole::Return, MirTypeShape::Unit)];
    locals.extend(arguments);
    locals.extend([
        local(index_local, MirLocalRole::Temp, thread_index_shape()),
        local(
            output_ref,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(disjoint_shape()),
                mutable: true,
            },
        ),
        local(
            option,
            MirLocalRole::Temp,
            MirTypeShape::Adt {
                identity: "core::option::Option".to_string(),
            },
        ),
        local(discriminant, MirLocalRole::Temp, MirTypeShape::ISize),
        local(
            payload,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(MirTypeShape::F32),
                mutable: true,
            },
        ),
        local(linear_index, MirLocalRole::Temp, MirTypeShape::USize),
    ]);
    locals.extend(arithmetic_locals);
    locals.sort_by_key(|local| local.index);

    MirFunction {
        semantic_instance: None,
        export_name: name.to_string(),
        rust_path: format!("tests::{name}"),
        kind: MirFunctionKind::KernelEntry,
        typed_profile: Some(crate::mir_import::MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
        arg_count: output_local,
        local_count: locals.len(),
        locals,
        blocks: vec![
            block(
                0,
                Vec::new(),
                call(TrustedDeviceItem::ThreadIndex1d, Vec::new(), index_local, 1),
            ),
            block(
                1,
                vec![assign(
                    0,
                    place(output_ref),
                    vec![operand(output_local)],
                    MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::MutableDefault),
                )],
                call(
                    TrustedDeviceItem::DisjointSliceGetMut,
                    vec![operand(output_ref), operand(index_local)],
                    option,
                    2,
                ),
            ),
            block(
                2,
                vec![assign(
                    0,
                    place(discriminant),
                    vec![operand(option)],
                    MirRvalueKind::Discriminant,
                )],
                MirTerminatorKind::SwitchInt {
                    discriminant: operand(discriminant),
                    targets: vec![
                        MirSwitchTarget {
                            value: 1,
                            target: 3,
                        },
                        MirSwitchTarget {
                            value: 0,
                            target: 5,
                        },
                    ],
                    otherwise: 6,
                },
            ),
            block(
                3,
                vec![assign(
                    0,
                    place(payload),
                    vec![MirOperandRef::Place(MirPlaceRef {
                        local: option,
                        projection: vec![
                            MirProjectionElem::Downcast { variant: 1 },
                            MirProjectionElem::Field(0),
                        ],
                        semantic_identity:
                            crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
                    })],
                    MirRvalueKind::Use,
                )],
                call(
                    TrustedDeviceItem::ThreadIndexGet,
                    vec![operand(index_local)],
                    linear_index,
                    4,
                ),
            ),
            block(4, arithmetic, MirTerminatorKind::Goto { target: 5 }),
            block(5, Vec::new(), MirTerminatorKind::Return),
            block(6, Vec::new(), MirTerminatorKind::Unreachable),
        ],
        frontend_contract: None,
        matrix_frontend_abi: None,
    }
}

fn function<'a>(module: &'a Module, id: &str) -> &'a Function {
    module
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .expect("kernel definition")
}

fn operations(function: &Function) -> Vec<&Operation> {
    function
        .body
        .as_ref()
        .expect("kernel body")
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect()
}

fn block(index: usize, statements: Vec<MirStatement>, kind: MirTerminatorKind) -> MirBlock {
    MirBlock {
        index,
        statements,
        terminator: Some(MirTerminator { kind, source: None }),
    }
}

fn call(
    item: TrustedDeviceItem,
    operands: Vec<MirOperandRef>,
    destination: usize,
    target: usize,
) -> MirTerminatorKind {
    MirTerminatorKind::Call {
        callee: Some(MirCallee::trusted_for_test(item)),
        target: Some(target),
        destination: Some(place(destination)),
        operands,
    }
}

fn assign(
    index: usize,
    destination: MirPlaceRef,
    operands: Vec<MirOperandRef>,
    rvalue: MirRvalueKind,
) -> MirStatement {
    MirStatement {
        index,
        kind: MirStatementKind::Assign,
        destination: Some(destination),
        operands,
        rvalue: Some(rvalue),
        semantic_rvalue_type: None,
        operation: None,
        source: None,
    }
}

fn store(index: usize, pointer_local: usize, value_local: usize) -> MirStatement {
    assign(
        index,
        MirPlaceRef {
            local: pointer_local,
            projection: vec![MirProjectionElem::Deref],
            semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
        },
        vec![operand(value_local)],
        MirRvalueKind::Use,
    )
}

fn indexed(slice_local: usize, index_local: usize) -> MirOperandRef {
    MirOperandRef::Place(MirPlaceRef {
        local: slice_local,
        projection: vec![
            MirProjectionElem::Deref,
            MirProjectionElem::Index { local: index_local },
        ],
        semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
    })
}

fn operand(local: usize) -> MirOperandRef {
    MirOperandRef::Place(place(local))
}

fn place(local: usize) -> MirPlaceRef {
    MirPlaceRef {
        local,
        projection: Vec::new(),
        semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
    }
}

fn usize_constant(value: u64) -> MirOperandRef {
    MirOperandRef::Constant {
        ty: imported(MirTypeShape::USize),
        literal: MirConstant::USize(value),
        value: value.to_string(),
    }
}

fn local(index: usize, role: MirLocalRole, shape: MirTypeShape) -> MirLocal {
    MirLocal {
        index,
        role,
        ty: imported(shape),
    }
}

fn imported(shape: MirTypeShape) -> MirImportedType {
    let (kind, rust) = match &shape {
        MirTypeShape::Unit => (MirType::Unit, "()"),
        MirTypeShape::U32 => (MirType::I32, "u32"),
        MirTypeShape::F32 => (MirType::F32, "f32"),
        MirTypeShape::USize => (MirType::USize, "usize"),
        MirTypeShape::ISize => (MirType::I64, "isize"),
        MirTypeShape::Slice { .. } => (MirType::Slice, "&[f32]"),
        MirTypeShape::DisjointSlice { .. } => (MirType::DisjointSlice, "DisjointSlice<f32>"),
        MirTypeShape::Reference { .. } => (MirType::Ptr, "&mut T"),
        MirTypeShape::Adt { .. } => (MirType::Unknown, "adt"),
        _ => (MirType::Unknown, "unknown"),
    };
    MirImportedType {
        kind,
        rust: rust.to_string(),
        shape,
        semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
    }
}

fn thread_index_shape() -> MirTypeShape {
    MirTypeShape::Adt {
        identity: TrustedDeviceItem::ThreadIndex.canonical_path().to_string(),
    }
}

fn slice_shape(mutable: bool) -> MirTypeShape {
    MirTypeShape::Slice {
        element: Box::new(MirTypeShape::F32),
        mutable,
    }
}

fn disjoint_shape() -> MirTypeShape {
    MirTypeShape::DisjointSlice {
        element: Box::new(MirTypeShape::F32),
    }
}

fn slice(access: AccessMode) -> Type {
    Type::slice(Type::F32, AddressSpace::Global, access)
}
