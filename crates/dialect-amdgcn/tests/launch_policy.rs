use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use dialect_amdgcn::{
    Gfx942KernelLaunchPolicyV1, LoweringDiagnosticCode,
    lower_compiler_module_to_gfx942_llvm_ir_with_launch_policies,
};
use fe2o3_kernel_descriptor::Gfx942LaunchBoundsV1;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Function, Kernel, KernelId, LaunchDomain, LaunchExtent, Module, Signature,
    Terminator, WorkgroupSize,
};

fn returning_entry(id: &str) -> Function {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::kernel_entry(id, Signature::new(vec![], vec![]), vec![], vec![block])
}

fn kernel(id: &str, workgroup_x: u32) -> Kernel {
    let mut kernel = Kernel::new(
        id,
        format!("{id}_entry"),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(workgroup_x, 1, 1));
    kernel
}

fn family_module() -> Module {
    let mut module = Module::new("tests::launch_policy_family");
    module.functions = vec![
        returning_entry("saxpy_wg64_entry"),
        returning_entry("saxpy_wg256_entry"),
    ];
    module.kernels = vec![kernel("saxpy_wg64", 64), kernel("saxpy_wg256", 256)];
    module
}

fn policy(
    kernel: &str,
    minimum_flat: u32,
    maximum_flat: u32,
    minimum_waves: u8,
    maximum_waves: u8,
) -> Gfx942KernelLaunchPolicyV1 {
    Gfx942KernelLaunchPolicyV1::new(
        KernelId::new(kernel),
        Gfx942LaunchBoundsV1::new(minimum_flat, maximum_flat, minimum_waves, maximum_waves)
            .unwrap(),
    )
}

fn exact_policies() -> [Gfx942KernelLaunchPolicyV1; 2] {
    [
        policy("saxpy_wg64", 64, 64, 4, 8),
        policy("saxpy_wg256", 256, 256, 2, 4),
    ]
}

#[test]
fn gfx942_family_emits_exact_flat_workgroup_and_occupancy_metadata() {
    let module = family_module();
    let policies = exact_policies();
    let llvm =
        lower_compiler_module_to_gfx942_llvm_ir_with_launch_policies(&module, &policies).unwrap();
    assert!(
        llvm.contains("\"amdgpu-flat-work-group-size\"=\"64,64\" \"amdgpu-waves-per-eu\"=\"4,8\"")
    );
    assert!(
        llvm.contains(
            "\"amdgpu-flat-work-group-size\"=\"256,256\" \"amdgpu-waves-per-eu\"=\"2,4\""
        )
    );
    assert_eq!(
        llvm,
        lower_compiler_module_to_gfx942_llvm_ir_with_launch_policies(
            &module,
            &[policies[1].clone(), policies[0].clone()],
        )
        .unwrap()
    );
}

#[test]
fn missing_duplicate_unknown_and_incompatible_policies_fail_closed() {
    let module = family_module();
    let cases = [
        vec![policy("saxpy_wg64", 64, 64, 4, 8)],
        vec![
            policy("saxpy_wg64", 64, 64, 4, 8),
            policy("saxpy_wg64", 64, 64, 4, 8),
        ],
        vec![
            policy("saxpy_wg64", 64, 64, 4, 8),
            policy("substituted", 256, 256, 2, 4),
        ],
        vec![
            policy("saxpy_wg64", 32, 32, 4, 8),
            policy("saxpy_wg256", 256, 256, 2, 4),
        ],
    ];
    for policies in cases {
        let error =
            lower_compiler_module_to_gfx942_llvm_ir_with_launch_policies(&module, &policies)
                .unwrap_err();
        assert!(error.contains(LoweringDiagnosticCode::InvalidLaunchPolicy));
    }
}

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gfx942-launch-policy-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires the ROCm LLVM toolchain with gfx942 support"]
fn rocm_compiles_the_bounded_gfx942_family() {
    let directory = TemporaryDirectory::new();
    let source = directory.0.join("family.ll");
    let code_object = directory.0.join("family.hsaco");
    let llvm = lower_compiler_module_to_gfx942_llvm_ir_with_launch_policies(
        &family_module(),
        &exact_policies(),
    )
    .unwrap();
    fs::write(&source, llvm).unwrap();
    let output = Command::new("/opt/rocm/llvm/bin/clang")
        .args(["--target=amdgcn-amd-amdhsa", "-mcpu=gfx942", "-nogpulib"])
        .arg(&source)
        .arg("-o")
        .arg(&code_object)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "clang rejected launch policies: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fs::metadata(code_object).unwrap().len() > 64);
}
