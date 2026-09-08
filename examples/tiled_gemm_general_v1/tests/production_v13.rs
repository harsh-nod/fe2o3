#[cfg(feature = "bundle-v8-simulator")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(feature = "bundle-v8-simulator")]
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{
    ExecutionCapabilityOperationV1, ExecutionCapabilityRequirementV1, NumericalModeV1,
    OperationKind, ResourceCapabilityRequirementV1, ScalarType, TargetCapability,
    TensorLdsSwizzleV1, TensorOperandRoleV1, TensorTailMaskV1, Terminator,
    VerifiedCanonicalKernelIrV13, VerifiedSimulationBundleV8,
};
use fe2o3_tiled_gemm_general_v1::contract::{
    GENERAL_GEMM_NUMERICAL_POLICY_V1, STATIC_WORKGROUP_MEMORY_BYTES_V1,
};
#[cfg(feature = "bundle-v8-simulator")]
use fe2o3_tiled_gemm_general_v1::reference::{ReferenceProblemV1, evaluate_reference_v1};

#[cfg(feature = "bundle-v8-simulator")]
static NONCE: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "bundle-v8-simulator")]
struct Scratch(PathBuf);

#[cfg(feature = "bundle-v8-simulator")]
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-general-gemm-production-v13-{}-{}",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

#[cfg(feature = "bundle-v8-simulator")]
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn load_production_bundle() -> (VerifiedSimulationBundleV8, fe2o3_kernel_ir::Module) {
    let path = std::env::var_os("FE2O3_M4_BUNDLE_V8")
        .map(PathBuf::from)
        .expect("FE2O3_M4_BUNDLE_V8 must name the compiler-produced M4 Bundle V8");
    let bytes = std::fs::read(&path).expect("read compiler-produced M4 Bundle V8");
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(bytes)
        .expect("admit exact compiler-produced Bundle V8");
    bundle.revalidate().expect("revalidate Bundle V8 custody");
    assert!(bundle.final_graph_epoch() > 0);
    assert!(!bundle.grants_compiler_authority());
    assert!(!bundle.grants_proof_authority());
    assert!(!bundle.grants_hardware_authority());
    let (_, module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        bundle.canonical_kir_v13().to_vec(),
    )
    .expect("decode exact canonical KIR V13 from Bundle V8");
    (bundle, module)
}

fn expected_target() -> String {
    std::env::var("FE2O3_M4_EXPECTED_TARGET")
        .expect("FE2O3_M4_EXPECTED_TARGET must name the exact bound target")
}

fn validate_m4_contract(
    module: &fe2o3_kernel_ir::Module,
    actual_target: &str,
    expected_target: &str,
) -> Result<(), Vec<&'static str>> {
    let mut errors = Vec::new();
    if actual_target != expected_target
        || !matches!(expected_target, "gfx942:xnack-" | "gfx950:xnack-")
    {
        errors.push("exact target binding");
    }

    let Some(kernel) = module
        .kernels
        .iter()
        .find(|kernel| kernel.id.as_str() == "tiled_gemm_general_v1")
    else {
        return Err(vec!["kernel root"]);
    };
    if kernel.workgroup_size.map(|size| (size.x, size.y, size.z)) != Some((64, 1, 1)) {
        errors.push("launch shape");
    }

    let capabilities = module.effective_capabilities();
    for (required, label) in [
        (TargetCapability::Subgroups, "subgroup capability"),
        (TargetCapability::SubgroupSize(64), "subgroup width"),
        (
            TargetCapability::WorkgroupMemory,
            "workgroup memory capability",
        ),
        (
            TargetCapability::Execution(ExecutionCapabilityRequirementV1::Matrix {
                m: 16,
                n: 16,
                k: 16,
                input_type: ScalarType::Bf16,
                accumulator_type: ScalarType::F32,
            }),
            "matrix shape and roles",
        ),
        (
            TargetCapability::Execution(ExecutionCapabilityRequirementV1::Numerical {
                value_type: ScalarType::F32,
                mode: NumericalModeV1::StrictIeee,
            }),
            "numerical policy",
        ),
        (
            TargetCapability::Execution(ExecutionCapabilityRequirementV1::Resource(
                ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(64),
            )),
            "workgroup resource limit",
        ),
        (
            TargetCapability::Execution(ExecutionCapabilityRequirementV1::Resource(
                ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(
                    STATIC_WORKGROUP_MEMORY_BYTES_V1 as u64,
                ),
            )),
            "LDS resource limit",
        ),
    ] {
        if !capabilities.contains(&required) {
            errors.push(label);
        }
    }

    let mut workgroup = false;
    let mut subgroup = false;
    let mut matrix_access = false;
    let mut lds_allocate = false;
    let mut lds_publish = false;
    let mut barrier_epoch = false;
    let mut matrix_operation = false;
    let mut dynamic_k_phases = false;
    for function in &module.functions {
        let Some(body) = &function.body else { continue };
        dynamic_k_phases |= has_matrix_cycle(body);
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            match &operation.kind {
                OperationKind::ExecutionCapability(capability) => match &capability.operation {
                    ExecutionCapabilityOperationV1::WorkgroupDerive { .. } => workgroup = true,
                    ExecutionCapabilityOperationV1::SubgroupDerive { width: 64, .. } => {
                        subgroup = true;
                    }
                    ExecutionCapabilityOperationV1::MatrixAccess { width: 64, .. } => {
                        matrix_access = true;
                    }
                    ExecutionCapabilityOperationV1::LdsAllocate { .. } => lds_allocate = true,
                    ExecutionCapabilityOperationV1::LdsPublish { .. } => lds_publish = true,
                    ExecutionCapabilityOperationV1::WorkgroupBarrier {
                        input_workgroup,
                        output_workgroup,
                        semantics,
                    } => {
                        barrier_epoch |= input_workgroup != output_workgroup
                            && semantics.scope
                                == fe2o3_kernel_ir::ExecutionMemoryScopeV1::Workgroup
                            && semantics.ordering
                                == fe2o3_kernel_ir::ExecutionMemoryOrderingV1::AcquireRelease
                            && semantics.spaces
                                == fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup;
                    }
                    _ => {}
                },
                OperationKind::Matrix(matrix) => {
                    matrix_operation = true;
                    let Some(layout) = matrix.tensor_layout else {
                        errors.push("matrix layout");
                        continue;
                    };
                    if matrix.active_lanes != 64
                        || layout.subgroup_width != 64
                        || layout.a.role != TensorOperandRoleV1::A
                        || layout.b.role != TensorOperandRoleV1::B
                        || layout.accumulator.role != TensorOperandRoleV1::Accumulator
                    {
                        errors.push("matrix layout roles");
                    }
                    if layout.a.lds_swizzle != TensorLdsSwizzleV1::None
                        || layout.b.lds_swizzle != TensorLdsSwizzleV1::None
                    {
                        errors.push("matrix LDS layout");
                    }
                    if layout.tail_mask != TensorTailMaskV1::ZeroFilledPredicateInputs {
                        errors.push("matrix edge and K-tail policy");
                    }
                }
                _ => {}
            }
        }
    }
    for (present, label) in [
        (workgroup, "workgroup derivation"),
        (subgroup, "subgroup derivation"),
        (matrix_access, "matrix capability issuance"),
        (lds_allocate, "LDS allocation"),
        (lds_publish, "LDS publication"),
        (barrier_epoch, "barrier epoch transition"),
        (matrix_operation, "matrix operation"),
        (dynamic_k_phases, "dynamic K phases"),
    ] {
        if !present {
            errors.push(label);
        }
    }

    let policy = GENERAL_GEMM_NUMERICAL_POLICY_V1;
    if !policy.exact_bf16_widening
        || !policy.strict_ieee_f32
        || !policy.positive_zero_k_tail
        || !policy.separate_alpha_beta_products
        || policy.contraction_and_reassociation
    {
        errors.push("source numerical policy");
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn has_matrix_cycle(body: &fe2o3_kernel_ir::FunctionBody) -> bool {
    body.blocks
        .iter()
        .filter(|block| {
            block
                .operations
                .iter()
                .any(|operation| matches!(&operation.kind, OperationKind::Matrix(_)))
        })
        .any(|block| {
            successors(block.terminator.as_ref())
                .into_iter()
                .any(|next| {
                    next == block.id
                        || reaches_block(
                            body,
                            next,
                            block.id,
                            &mut Vec::with_capacity(body.blocks.len()),
                        )
                })
        })
}

fn reaches_block(
    body: &fe2o3_kernel_ir::FunctionBody,
    current: fe2o3_kernel_ir::BlockId,
    target: fe2o3_kernel_ir::BlockId,
    visited: &mut Vec<fe2o3_kernel_ir::BlockId>,
) -> bool {
    if current == target {
        return true;
    }
    if visited.contains(&current) {
        return false;
    }
    visited.push(current);
    body.blocks
        .iter()
        .find(|block| block.id == current)
        .is_some_and(|block| {
            successors(block.terminator.as_ref())
                .into_iter()
                .any(|next| reaches_block(body, next, target, visited))
        })
}

fn successors(terminator: Option<&Terminator>) -> Vec<fe2o3_kernel_ir::BlockId> {
    match terminator {
        Some(Terminator::Branch { target, .. }) => vec![*target],
        Some(Terminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        }) => vec![*then_target, *else_target],
        Some(Terminator::Switch {
            cases,
            default_target,
            ..
        }) => cases
            .iter()
            .map(|case| case.target)
            .chain(core::iter::once(*default_target))
            .collect(),
        Some(Terminator::IntegerSwitch {
            cases,
            default_target,
            ..
        }) => cases
            .iter()
            .map(|case| case.target)
            .chain(core::iter::once(*default_target))
            .collect(),
        Some(Terminator::Return { .. } | Terminator::Unreachable) | None => Vec::new(),
    }
}

#[test]
#[ignore = "requires a genuine W6 compiler-produced Bundle V8; never substitute a synthetic receipt"]
fn finalized_graph_carries_every_m4_requirement() {
    let (bundle, module) = load_production_bundle();
    validate_m4_contract(&module, bundle.target(), &expected_target())
        .expect("finalized graph satisfies every M4 requirement");
}

fn assert_contract_rejects(
    module: &fe2o3_kernel_ir::Module,
    actual_target: &str,
    expected_target: &str,
    expected_error: &'static str,
) {
    let errors = validate_m4_contract(module, actual_target, expected_target)
        .expect_err("hostile M4 contract mutation was accepted");
    assert!(
        errors.contains(&expected_error),
        "mutation omitted {expected_error:?}: {errors:?}"
    );
}

#[test]
#[ignore = "requires a genuine W6 compiler-produced Bundle V8 as the mutation baseline"]
fn finalized_graph_mutations_fail_closed_at_the_m4_gate() {
    let (bundle, module) = load_production_bundle();
    let target = expected_target();

    let substituted_target = if target == "gfx942:xnack-" {
        "gfx950:xnack-"
    } else {
        "gfx942:xnack-"
    };
    assert_contract_rejects(
        &module,
        bundle.target(),
        substituted_target,
        "exact target binding",
    );

    let mut launch = module.clone();
    launch
        .kernels
        .iter_mut()
        .find(|kernel| kernel.id.as_str() == "tiled_gemm_general_v1")
        .unwrap()
        .workgroup_size
        .as_mut()
        .unwrap()
        .x = 32;
    assert_contract_rejects(&launch, bundle.target(), &target, "launch shape");

    let mut layout = module.clone();
    mutate_first_matrix(&mut layout, |matrix| {
        matrix.tensor_layout.as_mut().unwrap().a.lds_swizzle = TensorLdsSwizzleV1::Xor4;
    });
    assert_contract_rejects(&layout, bundle.target(), &target, "matrix LDS layout");

    let mut role = module.clone();
    mutate_first_matrix(&mut role, |matrix| {
        matrix.tensor_layout.as_mut().unwrap().a.role = TensorOperandRoleV1::B;
    });
    assert_contract_rejects(&role, bundle.target(), &target, "matrix layout roles");

    let mut tail = module.clone();
    mutate_first_matrix(&mut tail, |matrix| {
        matrix.tensor_layout.as_mut().unwrap().tail_mask = TensorTailMaskV1::Missing;
    });
    assert_contract_rejects(
        &tail,
        bundle.target(),
        &target,
        "matrix edge and K-tail policy",
    );

    let mut phases = module.clone();
    for function in &mut phases.functions {
        if let Some(body) = &mut function.body {
            for block in &mut body.blocks {
                if matches!(
                    block.terminator,
                    Some(
                        Terminator::ConditionalBranch { .. }
                            | Terminator::Switch { .. }
                            | Terminator::IntegerSwitch { .. }
                    )
                ) {
                    block.terminator = None;
                }
            }
        }
    }
    assert_contract_rejects(&phases, bundle.target(), &target, "dynamic K phases");

    let mut barrier = module.clone();
    for function in &mut barrier.functions {
        if let Some(body) = &mut function.body {
            for block in &mut body.blocks {
                block.operations.retain(|operation| {
                    !matches!(
                        &operation.kind,
                        OperationKind::ExecutionCapability(capability)
                            if matches!(
                                capability.operation,
                                ExecutionCapabilityOperationV1::WorkgroupBarrier { .. }
                            )
                    )
                });
            }
        }
    }
    assert_contract_rejects(
        &barrier,
        bundle.target(),
        &target,
        "barrier epoch transition",
    );

    let mut resources = module.clone();
    remove_capability(&mut resources, |capability| {
        matches!(
            capability,
            TargetCapability::Execution(ExecutionCapabilityRequirementV1::Resource(_))
        )
    });
    assert_contract_rejects(
        &resources,
        bundle.target(),
        &target,
        "workgroup resource limit",
    );

    let mut numerical = module.clone();
    remove_capability(&mut numerical, |capability| {
        matches!(
            capability,
            TargetCapability::Execution(ExecutionCapabilityRequirementV1::Numerical { .. })
        )
    });
    assert_contract_rejects(&numerical, bundle.target(), &target, "numerical policy");
}

fn mutate_first_matrix(
    module: &mut fe2o3_kernel_ir::Module,
    mutate: impl FnOnce(&mut fe2o3_kernel_ir::MatrixOperation),
) {
    let matrix = module
        .functions
        .iter_mut()
        .filter_map(|function| function.body.as_mut())
        .flat_map(|body| &mut body.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => Some(matrix),
            _ => None,
        })
        .expect("baseline has a matrix operation");
    mutate(matrix);
}

fn remove_capability(
    module: &mut fe2o3_kernel_ir::Module,
    remove: impl Fn(&TargetCapability) -> bool,
) {
    module.required_capabilities.retain(|item| !remove(item));
    for function in &mut module.functions {
        function.required_capabilities.retain(|item| !remove(item));
    }
    for kernel in &mut module.kernels {
        kernel.required_capabilities.retain(|item| !remove(item));
    }
}

#[cfg(feature = "bundle-v8-simulator")]
fn u16_hex(values: &[u16]) -> String {
    let mut output = String::with_capacity(2 + values.len() * 4);
    output.push_str("0x");
    for byte in values.iter().flat_map(|value| value.to_le_bytes()) {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

#[cfg(feature = "bundle-v8-simulator")]
fn f32_hex(values: &[f32]) -> String {
    let bits = values
        .iter()
        .map(|value| value.to_bits())
        .collect::<Vec<_>>();
    let mut output = String::with_capacity(2 + bits.len() * 8);
    output.push_str("0x");
    for byte in bits.iter().flat_map(|value| value.to_le_bytes()) {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

#[cfg(feature = "bundle-v8-simulator")]
fn scalar(ty: &str, bits: u32) -> serde_json::Value {
    serde_json::json!({
        "kind": "scalar",
        "type": ty,
        "bits": format!("0x{bits:08x}"),
    })
}

#[cfg(feature = "bundle-v8-simulator")]
fn write_small_request(path: &Path) -> (Vec<u16>, Vec<u16>, Vec<f32>, ReferenceProblemV1) {
    let bits = |value| fe2o3_device::Bf16::from_f32(value).to_bits();
    let problem = ReferenceProblemV1 {
        rows: 3,
        columns: 5,
        reduction: 7,
        lhs_stride: 9,
        rhs_stride: 8,
        output_stride: 7,
        product_scale: 0.75,
        output_scale: -0.25,
    };
    let lhs = (0..problem.rows * problem.lhs_stride)
        .map(|index| bits((index % 7) as f32 * 0.25 - 0.5))
        .collect::<Vec<_>>();
    let rhs = (0..problem.reduction * problem.rhs_stride)
        .map(|index| bits((index % 5) as f32 * 0.125 - 0.25))
        .collect::<Vec<_>>();
    let initial = (0..problem.rows * problem.output_stride)
        .map(|index| index as f32 * 0.03125 - 1.0)
        .collect::<Vec<_>>();
    let request = serde_json::json!({
        "schema": "fe2o3-simulation-request-v1",
        "kernel": "tiled_gemm_general_v1",
        "grid": [64, 1, 1],
        "workgroup": [64, 1, 1],
        "arguments": [
            {"kind": "buffer", "element": "bf16", "access": "read_only", "alignment": 2, "bytes": u16_hex(&lhs)},
            {"kind": "buffer", "element": "bf16", "access": "read_only", "alignment": 2, "bytes": u16_hex(&rhs)},
            {"kind": "buffer", "element": "f32", "access": "read_write", "alignment": 4, "bytes": f32_hex(&initial)},
            scalar("u32", problem.rows),
            scalar("u32", problem.columns),
            scalar("u32", problem.reduction),
            scalar("u32", problem.lhs_stride),
            scalar("u32", problem.rhs_stride),
            scalar("u32", problem.output_stride),
            scalar("f32", problem.product_scale.to_bits()),
            scalar("f32", problem.output_scale.to_bits()),
        ],
    });
    std::fs::write(path, serde_json::to_vec(&request).unwrap()).unwrap();
    (lhs, rhs, initial, problem)
}

#[test]
#[cfg(feature = "bundle-v8-simulator")]
#[ignore = "requires W6 Bundle V8 export and complete simulator support for the finalized M4 graph"]
fn bundle_v8_matches_the_deterministic_cpu_oracle() {
    let bundle = std::env::var_os("FE2O3_M4_BUNDLE_V8")
        .map(PathBuf::from)
        .expect("FE2O3_M4_BUNDLE_V8 must name the compiler-produced M4 Bundle V8");
    let scratch = Scratch::new();
    let request = scratch.0.join("request.json");
    let (lhs, rhs, initial, problem) = write_small_request(&request);
    let expected = evaluate_reference_v1(&lhs, &rhs, &initial, problem).unwrap();

    let actual = fe2o3_tiled_gemm_general_v1::simulator::simulate_bundle_v8(bundle, request)
        .expect("simulate exact Bundle V8");

    assert_eq!(
        actual
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
}
