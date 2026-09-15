use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use std::{fs, path::PathBuf, process::Command};

const SOURCE: &str = r#"
#![no_std]
#![feature(try_trait_v2)]
use core::{convert::Infallible, ops::{ControlFlow, FromResidual, Try}};
use fe2o3_device::{
    Bf16MatrixViewError, CurrentTarget, GlobalBf16MfmaMatrix, Grid, InitialEpoch,
    KernelCapabilityBrand, KernelError, MfmaOperandA, MfmaOperandB, RegisteredLaunch,
    ThreadIndex, WorkgroupIndex1D, WorkgroupMemoryBrand,
};
pub struct KernelMarker;
pub trait UnreviewedTrait { fn unreviewed(self) -> u8; }
impl UnreviewedTrait for KernelError {
    fn unreviewed(self) -> u8 { 0 }
}
impl core::convert::From<KernelMarker> for KernelError {
    fn from(_: KernelMarker) -> Self { Self::InvalidArgument }
}
impl core::cmp::PartialEq<KernelMarker> for KernelError {
    fn eq(&self, _: &KernelMarker) -> bool { false }
}
type Brand = KernelCapabilityBrand<'static, KernelMarker, CurrentTarget, RegisteredLaunch>;
type MatrixA = GlobalBf16MfmaMatrix<'static, 'static, MfmaOperandA, Brand, Brand>;
type MatrixB = GlobalBf16MfmaMatrix<'static, 'static, MfmaOperandB, Brand, Brand>;
type BrandedGrid = Grid<'static, Brand>;
type MemoryIndex = ThreadIndex<WorkgroupIndex1D, WorkgroupMemoryBrand<'static, Brand, InitialEpoch>>;
pub fn tiled_residual(x: Result<Infallible, Bf16MatrixViewError>) -> Result<(), KernelError> {
    FromResidual::from_residual(x)
}
pub fn kernel_identity(x: Result<Infallible, KernelError>) -> Result<(), KernelError> {
    FromResidual::from_residual(x)
}
pub fn matrix_identity(x: Result<Infallible, Bf16MatrixViewError>) -> Result<(), Bf16MatrixViewError> {
    FromResidual::from_residual(x)
}
pub fn kernel_branch(x: Option<KernelError>) -> ControlFlow<Option<Infallible>, KernelError> {
    Try::branch(x)
}
pub fn matrix_branch(x: Option<Bf16MatrixViewError>) -> ControlFlow<Option<Infallible>, Bf16MatrixViewError> {
    Try::branch(x)
}
pub fn matrix_a_result_branch(x: Result<MatrixA, Bf16MatrixViewError>) -> ControlFlow<Result<Infallible, Bf16MatrixViewError>, MatrixA> {
    Try::branch(x)
}
pub fn matrix_b_result_branch(x: Result<MatrixB, Bf16MatrixViewError>) -> ControlFlow<Result<Infallible, Bf16MatrixViewError>, MatrixB> {
    Try::branch(x)
}
pub fn kernel_result_branch(x: Result<(), KernelError>) -> ControlFlow<Result<Infallible, KernelError>, ()> {
    Try::branch(x)
}
pub fn grid_option_residual(x: Option<Infallible>) -> Option<BrandedGrid> {
    FromResidual::from_residual(x)
}
pub fn workgroup_index_ok_or(x: Option<MemoryIndex>, error: KernelError) -> Result<MemoryIndex, KernelError> {
    x.ok_or(error)
}
pub fn matrix_a_ok_or(x: Option<MatrixA>, error: KernelError) -> Result<MatrixA, KernelError> {
    x.ok_or(error)
}
pub fn scalar_ok_or(x: Option<f32>, error: KernelError) -> Result<f32, KernelError> {
    x.ok_or(error)
}
"#;

struct DeviceCallbacks {
    completed: bool,
}
impl Callbacks for DeviceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let mut checked = 0;
        let mut checked_device_structure = 0;
        let definitions = tcx.iter_local_def_id().take(513).collect::<Vec<_>>();
        assert!(definitions.len() <= 512, "device fixture definition budget");
        for id in definitions
            .into_iter()
            .filter(|id| tcx.def_kind(*id) == DefKind::Fn)
        {
            let body = tcx.instance_mir(Instance::mono(tcx, id.to_def_id()).def);
            assert!(
                body.basic_blocks.len() <= MAX_BLOCKS,
                "device fixture block budget"
            );
            for block in body.basic_blocks.iter() {
                let TerminatorKind::Call {
                    func: Operand::Constant(callee),
                    ..
                } = &block.terminator().kind
                else {
                    continue;
                };
                let TyKind::FnDef(definition, args) = callee.const_.ty().kind() else {
                    continue;
                };
                let instance =
                    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
                        .unwrap()
                        .unwrap();
                let contract = contract(tcx, instance).unwrap_or_else(|| {
                    panic!(
                        "device contract rejected: {instance:?}; signature={:?}",
                        tcx.fn_sig(instance.def_id())
                            .instantiate(tcx, instance.args)
                    )
                });
                let helper = tcx.instance_mir(instance.def);
                eprintln!(
                    "device fixture {}: {instance:?}; target={}; panic={:?}",
                    tcx.item_name(id.to_def_id()),
                    tcx.sess.target.llvm_target,
                    tcx.sess.panic_strategy()
                );
                let mut remaining = MAX_STATEMENTS;
                for (index, block) in helper.basic_blocks.iter_enumerated().take(MAX_BLOCKS) {
                    let count = remaining.min(block.statements.len());
                    eprintln!(
                        "{index:?}: cleanup={} {:?}; {:?}",
                        block.is_cleanup,
                        &block.statements[..count],
                        block.terminator.as_ref().map(|t| &t.kind)
                    );
                    remaining -= count;
                }
                assert!(
                    reviewed_body(tcx, instance, helper, &contract),
                    "device helper rejected"
                );
                if let Some((_, conversion)) = contract.conversion {
                    assert!(
                        !authenticate_reviewed_safe_core_result_try_helper_v1(tcx, conversion),
                        "no source trust transferred to From"
                    );
                }
                if tcx.item_name(id.to_def_id()).as_str() == "tiled_residual" {
                    let (_, conversion) = contract
                        .conversion
                        .expect("tiled residual must retain the concrete device conversion");
                    crate::trusted_device_items::provider_trait_identity_v1::tests::assert_actual_device_from_structure(
                        tcx, conversion,
                    );
                    checked_device_structure += 1;
                }
                checked += 1;
                assert!(checked <= 12, "device fixture concrete-case budget");
            }
        }
        assert_eq!(checked, 12);
        assert_eq!(checked_device_structure, 1);
        self.completed = true;
        Compilation::Stop
    }
}

fn configured_path(name: &str) -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os(name)
            .unwrap_or_else(|| panic!("set {name} to an existing cached dependency path")),
    );
    assert!(path.exists(), "{name}: {}", path.display());
    path
}

fn run_device_fixture(amdgpu: bool) {
    let directory = TestTempDir::create("fe2o3-core-result-try-device");
    let source = directory.path().join("fixture.rs");
    fs::write(&source, SOURCE).unwrap();
    let device = configured_path("FE2O3_CORE_TRY_DEVICE_RMETA");
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_core_result_try_device".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Cpanic=abort".into(),
        "-Copt-level=0".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    if std::env::var_os("FE2O3_CORE_TRY_HOST_DEPS").is_some() || amdgpu {
        let host_deps = configured_path("FE2O3_CORE_TRY_HOST_DEPS");
        args.extend(["-L".into(), format!("dependency={}", host_deps.display())]);
    }
    if amdgpu {
        let core = configured_path("FE2O3_CORE_TRY_AMDGPU_CORE");
        let builtins = configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS");
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Zunstable-options".into(),
            "--extern".into(),
            format!("noprelude,nounused:core={}", core.display()),
            "--extern".into(),
            format!(
                "noprelude,nounused:compiler_builtins={}",
                builtins.display()
            ),
        ]);
    }
    let mut callbacks = DeviceCallbacks { completed: false };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}

#[test]
#[ignore = "requires FE2O3_CORE_TRY_DEVICE_RMETA from a cached device build"]
fn core_result_try_actual_device_errors_host_abort() {
    run_device_fixture(false);
}

#[test]
#[ignore = "requires cached AMD core/builtins/device metadata and host dependency paths"]
fn core_result_try_actual_device_errors_amdgpu_abort() {
    run_device_fixture(true);
}
