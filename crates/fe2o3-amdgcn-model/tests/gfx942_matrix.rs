use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_amdgcn_model::{
    GFX942_XNACK_MINUS_DATA_LAYOUT, LoweringDiagnosticCode,
    lower_compiler_module_to_gfx942_llvm_ir, lower_kernel_to_gfx942_llvm_ir,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gfx942-matrix-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn lds(result: u32, element: Type) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(result),
            Type::pointer(
                element.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element,
            extent: WorkgroupMemoryExtent::Static(256),
            alignment: 16,
        }),
    )
}

fn matrix_module() -> Module {
    let bf16 = Type::Scalar(ScalarType::Bf16);
    let mut next = 7;
    let mut matrix_operation = |matrix: MatrixOperation| {
        let results = matrix
            .result_types()
            .into_iter()
            .map(|ty| {
                let result = ValueDef::new(ValueId(next), ty);
                next += 1;
                result
            })
            .collect();
        Operation::new(results, OperationKind::Matrix(matrix))
    };

    let load_a = matrix_operation(MatrixOperation::lds_load(ValueId(4), MatrixElement::Bf16));
    let load_b = matrix_operation(MatrixOperation::lds_load(ValueId(5), MatrixElement::Bf16));
    let mma = matrix_operation(
        MatrixOperation::multiply_accumulate(
            [ValueId(7), ValueId(8), ValueId(9), ValueId(10)],
            [ValueId(11), ValueId(12), ValueId(13), ValueId(14)],
            [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        )
        .with_declared_tensor_layout(
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64_lds_xor4(),
        ),
    );
    let store = matrix_operation(MatrixOperation::lds_store(
        ValueId(6),
        [ValueId(15), ValueId(16), ValueId(17), ValueId(18)],
        MatrixElement::F32,
    ));

    let block = BasicBlock {
        id: BlockId(0),
        parameters: vec![],
        operations: vec![
            lds(4, bf16.clone()),
            lds(5, bf16),
            lds(6, Type::F32),
            load_a,
            load_b,
            mma,
            store,
        ],
        terminator: Some(Terminator::Return { values: vec![] }),
    };
    let mut function = Function::kernel_entry(
        "matrix_impl",
        Signature::new(vec![Type::F32; 4], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();

    let mut kernel = Kernel::new(
        "matrix_kernel",
        "matrix_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::gfx942_matrix");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn hand_built_untrusted_frontend_binding() -> MatrixFrontendBindingV2 {
    fn bytes(record: &mut Vec<u8>, value: &[u8]) {
        record.extend_from_slice(&(value.len() as u32).to_le_bytes());
        record.extend_from_slice(value);
    }

    let provider = MatrixProviderIdentityV2 {
        crate_name: "fe2o3_device".to_owned(),
        stable_crate_id: 1,
        crate_hash: [2; 16],
        cargo_metadata_build_observation: [3; 32],
        source_identity: [4; 32],
        definition_identities: vec![[5; 16]; 6],
    };
    let mut record = MATRIX_SOURCE_ABI_RECORD_DOMAIN_V2.to_vec();
    bytes(&mut record, provider.crate_name.as_bytes());
    record.extend_from_slice(&provider.stable_crate_id.to_le_bytes());
    bytes(&mut record, &provider.crate_hash);
    bytes(&mut record, &provider.cargo_metadata_build_observation);
    bytes(&mut record, &provider.source_identity);
    record.extend_from_slice(&(provider.definition_identities.len() as u32).to_le_bytes());
    for identity in &provider.definition_identities {
        bytes(&mut record, identity);
    }
    record.extend_from_slice(b"hand-built-public-layout-and-fnabi-claim");
    MatrixFrontendBindingV2 {
        observed_source: MatrixSourceAbiObservationV2::new_untrusted_claim(provider, record)
            .unwrap(),
        projected_kernarg: MatrixProjectedKernargPolicyV1::canonical(),
    }
}

fn hand_built_frontend_claim_module() -> Module {
    let binding = hand_built_untrusted_frontend_binding();
    let parameters = [vec![Type::Scalar(ScalarType::Bf16); 8], vec![Type::F32; 4]].concat();
    let matrix = MatrixOperation::multiply_accumulate(
        [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
        [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
    )
    .with_declared_tensor_layout(TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64())
    .with_frontend_binding(binding.clone());
    let operation = Operation::new(
        (12..16)
            .map(|id| ValueDef::new(ValueId(id), Type::F32))
            .collect(),
        OperationKind::Matrix(matrix),
    );
    let mut function = Function::kernel_entry(
        "matrix_frontend_impl",
        Signature::new(parameters, vec![]),
        (0..12).map(ValueId).collect(),
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations: vec![operation],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    );
    let target = gfx942_xnack_minus_target_capability();
    function.required_capabilities = function.derived_capabilities();
    function.required_capabilities.insert(target.clone());

    let mut kernel = Kernel::new(
        "matrix_frontend_kernel",
        "matrix_frontend_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities.insert(target.clone());
    kernel.required_capabilities.extend(binding.capabilities());

    let mut module = Module::new("tests::hand_built_frontend_claim");
    module.required_capabilities.insert(target);
    module.required_capabilities.extend(binding.capabilities());
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn exact_gfx942_matrix_profile_lowers_mfma_and_xor4_lds() {
    let llvm = lower_kernel_to_gfx942_llvm_ir(&matrix_module(), &"matrix_kernel".into()).unwrap();

    assert!(llvm.contains("\"target-cpu\"=\"gfx942\""));
    assert!(llvm.contains("\"target-features\"=\"-wavefrontsize32,+wavefrontsize64\""));
    assert!(llvm.contains("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
    assert_eq!(llvm.matches(" = load i16, ptr addrspace(3)").count(), 8);
    assert_eq!(llvm.matches("store float ").count(), 4);
    assert_eq!(llvm.matches(" = xor i32 ").count(), 12);
    assert!(llvm.contains(" = shl i32 %matrix.0.3.swizzle.row, 2"));
    let compiler_module = lower_compiler_module_to_gfx942_llvm_ir(&matrix_module()).unwrap();
    assert_eq!(
        compiler_module
            .matches("llvm.amdgcn.mfma.f32.16x16x16bf16.1k")
            .count(),
        2
    );
}

#[test]
fn exact_xnack_minus_kernel_api_requires_and_emits_the_retained_target_identity() {
    let mut module = matrix_module();
    let target = gfx942_xnack_minus_target_capability();
    module.required_capabilities.insert(target.clone());
    module.functions[0]
        .required_capabilities
        .insert(target.clone());
    module.kernels[0]
        .required_capabilities
        .insert(target.clone());

    let generic = lower_kernel_to_gfx942_llvm_ir(&module, &module.kernels[0].id)
        .expect_err("generic gfx942 profile cannot consume an exact target binding");
    assert!(generic.to_string().contains("gfx942:xnack-"));
    let llvm = lower_kernel_to_gfx942_xnack_minus_llvm_ir(&module, &module.kernels[0].id).unwrap();
    assert!(llvm.contains(GFX942_XNACK_MINUS_DATA_LAYOUT));
    assert!(llvm.contains("-wavefrontsize32,+wavefrontsize64,-xnack"));
    assert!(llvm.contains("\"fp-contract\"=\"off\""));

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
        assert!(
            lower_kernel_to_gfx942_xnack_minus_llvm_ir(&mutated, &mutated.kernels[0].id).is_err()
        );
    }
}

#[test]
fn public_capability_or_hand_built_record_never_becomes_observed_abi_evidence() {
    let binding = hand_built_untrusted_frontend_binding();
    let target = gfx942_xnack_minus_target_capability();
    let mut capability_only = matrix_module();
    for capabilities in [
        &mut capability_only.required_capabilities,
        &mut capability_only.kernels[0].required_capabilities,
        &mut capability_only.functions[0].required_capabilities,
    ] {
        capabilities.insert(target.clone());
        capabilities.insert(binding.observed_source.capability());
    }
    let errors = lower_kernel_to_gfx942_xnack_minus_llvm_ir(
        &capability_only,
        &capability_only.kernels[0].id,
    )
    .expect_err("a public digest capability cannot stand in for a structured record");
    assert!(
        errors
            .to_string()
            .contains("exactly one matrix operation must carry the structured rustc source ABI")
    );

    let hand_built = hand_built_frontend_claim_module();
    let llvm = lower_kernel_to_gfx942_xnack_minus_llvm_ir(&hand_built, &hand_built.kernels[0].id)
        .expect("integrity-valid public IR remains lowerable without an authentication claim");
    assert!(llvm.contains("llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
    assert!(llvm.contains("fe2o3.projected-kernarg-policy.v1"));
    for forbidden in [
        MATRIX_SOURCE_ABI_OBSERVATION_NAMESPACE_V2,
        "observed-source-abi",
        "authenticated",
        "cargo-metadata-build-observation",
    ] {
        assert!(
            !llvm.contains(forbidden),
            "generic dialect output claimed unestablished source evidence `{forbidden}`:\n{llvm}"
        );
    }
}

#[test]
fn matrix_lowering_accepts_full_wave_workgroups_and_rejects_partial_waves() {
    let module = matrix_module();
    let baseline = lower_kernel_to_llvm_ir(&module, &"matrix_kernel".into()).unwrap_err();
    assert!(baseline.contains(LoweringDiagnosticCode::UnsupportedCapability));

    let mut multi_wave = module.clone();
    multi_wave.kernels[0].workgroup_size = Some(WorkgroupSize::new(128, 1, 1));
    let llvm = lower_kernel_to_gfx942_llvm_ir(&multi_wave, &"matrix_kernel".into()).unwrap();
    assert!(llvm.contains("\"amdgpu-flat-work-group-size\"=\"128,128\""));
    assert!(llvm.contains("!0 = !{i32 128, i32 1, i32 1}"));
    let compiler_llvm = lower_compiler_module_to_gfx942_llvm_ir(&multi_wave).unwrap();
    assert!(compiler_llvm.contains("\"amdgpu-flat-work-group-size\"=\"128,128\""));
    assert!(compiler_llvm.contains("!0 = !{i32 128, i32 1, i32 1}"));

    let mut two_dimensional = module.clone();
    two_dimensional.kernels[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Static(1),
        y: LaunchExtent::Static(1),
    };
    two_dimensional.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 2, 1));
    let llvm = lower_kernel_to_gfx942_llvm_ir(&two_dimensional, &"matrix_kernel".into()).unwrap();
    assert!(llvm.contains("\"amdgpu-flat-work-group-size\"=\"64,64\""));
    assert!(llvm.contains("!0 = !{i32 32, i32 2, i32 1}"));
    let compiler_llvm = lower_compiler_module_to_gfx942_llvm_ir(&two_dimensional).unwrap();
    assert!(compiler_llvm.contains("\"amdgpu-flat-work-group-size\"=\"64,64\""));
    assert!(compiler_llvm.contains("!0 = !{i32 32, i32 2, i32 1}"));

    let mut partial_wave = module;
    partial_wave.kernels[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Static(1),
        y: LaunchExtent::Static(1),
    };
    partial_wave.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 3, 1));
    let errors =
        lower_kernel_to_gfx942_llvm_ir(&partial_wave, &"matrix_kernel".into()).unwrap_err();
    assert!(errors.contains(LoweringDiagnosticCode::UnsupportedMatrixOperation));
    assert!(errors.to_string().contains("multiple of 64, found 96"));
}

#[test]
fn divergent_matrix_placement_is_rejected() {
    let mut module = matrix_module();
    let function = &mut module.functions[0];
    let body = function.body.as_mut().unwrap();
    let matrix_operations = body.blocks[0].operations.split_off(3);
    body.blocks[0].operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(19), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(21), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::NotEqual,
                lhs: ValueId(19),
                rhs: ValueId(20),
            },
        ),
    ]);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(21),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    body.blocks.push(BasicBlock {
        id: BlockId(1),
        parameters: vec![],
        operations: matrix_operations,
        terminator: Some(Terminator::Return { values: vec![] }),
    });
    body.blocks.push(BasicBlock {
        id: BlockId(2),
        parameters: vec![],
        operations: vec![],
        terminator: Some(Terminator::Return { values: vec![] }),
    });

    let errors = lower_kernel_to_gfx942_llvm_ir(&module, &"matrix_kernel".into()).unwrap_err();
    assert!(errors.contains(LoweringDiagnosticCode::UnprovenBarrierConvergence));
    assert!(errors.to_string().contains("convergent operation requires"));
}

#[test]
#[ignore = "requires ROCm LLVM tools with gfx942 support"]
fn rocm_compiles_and_inspects_gfx942_matrix_workgroup_shapes() {
    let directory = TemporaryDirectory::new();
    for (name, domain, workgroup_size) in [
        (
            "single_wave",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
            WorkgroupSize::new(64, 1, 1),
        ),
        (
            "multi_wave",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
            WorkgroupSize::new(128, 1, 1),
        ),
        (
            "two_dimensional",
            LaunchDomain::D2 {
                x: LaunchExtent::Static(1),
                y: LaunchExtent::Static(1),
            },
            WorkgroupSize::new(32, 2, 1),
        ),
    ] {
        let input_name = format!("matrix-{name}.ll");
        let object_name = format!("matrix-{name}.o");
        let input = directory.join(&input_name);
        let object = directory.join(&object_name);
        let mut module = matrix_module();
        module.kernels[0].domain = domain;
        module.kernels[0].workgroup_size = Some(workgroup_size);
        fs::write(
            &input,
            lower_kernel_to_gfx942_llvm_ir(&module, &"matrix_kernel".into()).unwrap(),
        )
        .unwrap();

        let compile = Command::new("/opt/rocm/llvm/bin/clang")
            .args([
                "-x",
                "ir",
                "--target=amdgcn-amd-amdhsa",
                "-mcpu=gfx942",
                "-mcode-object-version=6",
                "-nogpulib",
                "-c",
            ])
            .arg(&input)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "clang failed for {name}:\n{}",
            String::from_utf8_lossy(&compile.stderr)
        );

        let disassembly = Command::new("/opt/rocm/llvm/bin/llvm-objdump")
            .args(["-d", "--mcpu=gfx942"])
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            disassembly.status.success(),
            "llvm-objdump failed for {name}:\n{}",
            String::from_utf8_lossy(&disassembly.stderr)
        );
        let disassembly = String::from_utf8_lossy(&disassembly.stdout);
        assert!(
            disassembly.contains("v_mfma_f32_16x16x16_bf16"),
            "missing gfx942 MFMA instruction for {name}:\n{disassembly}"
        );
    }
}
