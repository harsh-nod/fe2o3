//! Real AMD macro/session-bound source -> complete canonical import -> ranked
//! expression proof of the retained core helper. No Cargo or kernel execution.

use super::*;
use crate::production_target_v1::RetainedProductionTargetV1;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, rustc_mir_body_sha256_v1,
};
use crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::ty::TyCtxt;
use rustc_session::config::Input;
use rustc_span::FileName;
use std::path::PathBuf;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{KernelContext, kernel};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn unsigned_subtract_source(context: KernelContext<'_>, lhs: usize, rhs: usize) {
    let _ = context;
    let _difference = lhs.checked_sub(rhs);
}
"#;

struct Probe {
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("unsigned_subtract_source.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(tcx.sess.target.llvm_target.as_ref(), "amdgcn-amd-amdhsa");
        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .expect("safe source collection retains the real checked_sub body");
        let helpers = closure
            .exact_codegen_function_symbols_v1()
            .map(|(instance, _, _)| instance)
            .filter(|instance| {
                matches!(
                    tcx.def_kind(instance.def_id()),
                    rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::AssocFn
                ) && tcx.item_name(instance.def_id()).as_str() == "checked_sub"
            })
            .collect::<Vec<_>>();
        assert_eq!(
            helpers.len(),
            1,
            "fixture must reach exactly one checked_sub helper"
        );
        let instance = helpers[0];
        assert!(
            crate::trusted_device_items::authenticate_reviewed_safe_core_arithmetic_helper_v1(
                tcx, instance
            )
        );
        let identity = canonical_function_identities_v1(tcx, instance).function();
        let rustc_body = tcx.instance_mir(instance.def);
        assert_eq!(
            rustc_body.local_decls[rustc_middle::mir::Local::from_usize(1)].ty,
            tcx.types.usize
        );
        assert_eq!(
            rustc_body.local_decls[rustc_middle::mir::Local::from_usize(2)].ty,
            tcx.types.usize
        );
        assert!(
            matches!(&rustc_body.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(2)].statements[0].kind,
            rustc_middle::mir::StatementKind::Assign(assignment)
                if matches!(&assignment.1, rustc_middle::mir::Rvalue::BinaryOp(rustc_middle::mir::BinOp::SubUnchecked, _)))
        );
        let body_hash = rustc_mir_body_sha256_v1(tcx, instance);
        let imported = crate::collector::construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("the real unsigned Lt predicate must pass canonical arithmetic admission");
        let mir = &imported.semantic_mir;
        mir.require_complete_external_entries().unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .expect("actual source canonical round-trip reruns the typed proof");
        assert_eq!(decoded.functions(), mir.functions());
        let function = mir
            .functions()
            .iter()
            .find(|function| function.identity() == identity)
            .unwrap();
        let before = function.clone();
        assert_eq!(function.blocks().len(), 4);
        assert!(matches!(
            value(function, 0, 0).kind(),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                ..
            }
        ));
        assert!(
            matches!(value(function, 2, 0).kind(), SemanticRvalueKindV1::UncheckedBinary(binary)
            if binary.operation() == SemanticUncheckedBinaryOpV1::Subtract)
        );
        assert!(
            semantic_unchecked_arithmetic_violation_v1(function)
                .unwrap()
                .is_some(),
            "untyped API remains conservative even for this authentic body"
        );
        assert!(
            semantic_unchecked_arithmetic_violation_with_types_v1(mir.types(), function)
                .unwrap()
                .is_none()
        );
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 64,
        };
        let expected = ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Subtract,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(0).unwrap(),
                scalar,
            }),
            rhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(1).unwrap(),
                scalar,
            }),
        };
        let synthetic = value(function, 2, 0).clone();
        let mut resolver = GpuSemanticExpressionResolverV2::new(mir.types(), function);
        assert_eq!(resolver.resolve_rvalue_v2(&synthetic), Err(UNPROVEN));
        assert_eq!(
            resolver.resolve_rvalue_v2(value(function, 2, 0)).unwrap(),
            expected
        );
        assert_eq!(&before, function);
        assert_eq!(rustc_mir_body_sha256_v1(tcx, instance), body_hash);
        self.completed = true;
        Compilation::Stop
    }
}

const CRATE_NAME: &str = "unsigned_subtract_import_source";
const METADATA: &str = "fe2o3-unsigned-subtract-source-v1";
const CHILD_ENV: &str = "FE2O3_UNSIGNED_SUBTRACT_IMPORT_CHILD";

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-unsigned-subtract-import-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        let scratch = Self(path);
        let device = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fe2o3-device");
        std::fs::write(scratch.0.join("Cargo.toml"), format!(
            "[package]\nname = \"unsigned-subtract-import-source\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device:?} }}\n"
        )).unwrap();
        scratch
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn configured_path(name: &str, directory: bool) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
        panic!("set {name} to complete existing metadata; no Cargo or host fallback")
    }));
    assert!(
        if directory {
            path.is_dir()
        } else {
            path.is_file()
        },
        "{name}: {}",
        path.display()
    );
    path.canonicalize().unwrap()
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; no Cargo"]
fn actual_amdgpu_checked_sub_import_and_ranked_consumer() {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };
    let device = configured_path("FE2O3_CORE_TRY_DEVICE_RMETA", false);
    let host = configured_path("FE2O3_CORE_TRY_HOST_DEPS", true);
    let core = configured_path("FE2O3_CORE_TRY_AMDGPU_CORE", false);
    let builtins = configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS", false);
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    let binding =
        reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA]).to_hex();
    if std::env::var(CHILD_ENV).ok().as_deref() != Some(observation.as_str()) {
        let scratch = Scratch::new();
        let test_name = format!(
            "{}::actual_amdgpu_checked_sub_import_and_ranked_consumer",
            module_path!().split_once("::").unwrap().1
        );
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test_name,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .env(CHILD_ENV, &observation)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins)
                .env("CARGO_MANIFEST_DIR", &scratch.0)
                .env("CARGO_PKG_NAME", "unsigned-subtract-import-source")
                .env(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1, &binding)
                .env("FE2O3_SIMULATION_MODE_V1", "1")
                .env(
                    "FE2O3_SIMULATION_ATTEMPT_V1",
                    "94949494949494949494949494949494",
                ),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "actual source child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    assert_eq!(
        std::env::var(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1).unwrap(),
        binding
    );
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        format!("--crate-name={CRATE_NAME}"),
        format!("--out-dir={}", std::env::var("CARGO_MANIFEST_DIR").unwrap()),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        "-Ctarget-cpu=gfx950".into(),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Zunstable-options".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-Cdebuginfo=2".into(),
        format!("-Cmetadata={METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", host.display()),
        "--extern".into(),
        format!("noprelude,nounused:core={}", core.display()),
        "--extern".into(),
        format!(
            "noprelude,nounused:compiler_builtins={}",
            builtins.display()
        ),
        "-L".into(),
        format!("dependency={}", core.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", builtins.parent().unwrap().display()),
        "-".into(),
    ];
    let mut probe = Probe { completed: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(
        probe.completed,
        "must finish canonical import and ranked source-node resolution"
    );
}
