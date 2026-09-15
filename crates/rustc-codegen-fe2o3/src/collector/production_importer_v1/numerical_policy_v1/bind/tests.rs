use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{Operand, Rvalue, StatementKind, TerminatorKind};
use rustc_session::config::Input;
use rustc_span::FileName;

#[test]
fn policy_pairing_never_classifies_issuance_consumers_or_type_markers_as_helpers() {
    assert_eq!(
        BindKind::from_marker(TrustedDeviceItem::PolicyMathBind),
        Some(BindKind::Math)
    );
    assert_eq!(
        BindKind::from_marker(TrustedDeviceItem::PolicyMatrixBind),
        Some(BindKind::Matrix)
    );
    for item in [
        TrustedDeviceItem::NumericalPolicyIssue,
        TrustedDeviceItem::StrictIeeeNumericalPolicy,
        TrustedDeviceItem::PolicyMathCapability,
        TrustedDeviceItem::PolicyMatrixCapability,
        TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::F32(
            fe2o3_kernel_ir::F32MathFunction::Exp,
        )),
    ] {
        assert_eq!(BindKind::from_marker(item), None);
        assert!(!crate::production_semantic_terminal_v1::is_traversed_reviewed_helper_v1(item));
    }
}

const SOURCE: &str = r#"
#![no_std]
#![allow(deprecated)]
use fe2o3_device::{
    CurrentTarget, DeviceMath, InitialEpoch, KernelCapabilityBrand, MatrixCapability,
    NumericalPolicyCapability, PolicyDeviceMath, PolicyMatrixCapability, RegisteredLaunch,
    ReusableWorkgroupBrand, StrictIeee, SubgroupBrand, SubgroupWidth64,
};
type Root<'a> = KernelCapabilityBrand<'a, u8, CurrentTarget, RegisteredLaunch>;
type Matrix<'a> = SubgroupBrand<'a, SubgroupWidth64, ReusableWorkgroupBrand<'a, Root<'a>>, InitialEpoch>;
pub fn math_pair<'a>(math: &'a DeviceMath<Root<'a>>, policy: &'a NumericalPolicyCapability<Root<'a>, StrictIeee>) -> PolicyDeviceMath<'a, Root<'a>, StrictIeee> {
    math.with_numerical_policy(policy)
}
pub fn matrix_pair<'a>(matrix: &'a MatrixCapability<Root<'a>>, policy: &'a NumericalPolicyCapability<Root<'a>, StrictIeee>) -> PolicyMatrixCapability<'a, Root<'a>, Root<'a>, StrictIeee> {
    matrix.with_numerical_policy(policy)
}
pub fn scoped_matrix_pair<'a>(matrix: &'a MatrixCapability<Matrix<'a>>, policy: &'a NumericalPolicyCapability<Root<'a>, StrictIeee>) -> PolicyMatrixCapability<'a, Matrix<'a>, Root<'a>, StrictIeee> {
    matrix.with_numerical_policy(policy)
}
pub fn with_numerical_policy(left: &u32, right: &u32) -> u32 { *left + *right }
macro_rules! unary {
    ($($method:ident),*) => {$(
        pub fn $method<'a>(math: &PolicyDeviceMath<'a, Root<'a>, StrictIeee>, value: f32) -> f32 {
            math.$method(value)
        }
    )*};
}
unary!(sqrt_f32, floor_f32, ceil_f32, trunc_f32, round_ties_even_f32,
    sin_f32, cos_f32, exp_f32, exp2_f32, ln_f32, log2_f32, log10_f32);
pub fn mul_add_f32<'a>(math: &PolicyDeviceMath<'a, Root<'a>, StrictIeee>, x: f32, y: f32, z: f32) -> f32 {
    math.mul_add_f32(x, y, z)
}
pub fn bare_sqrt<'a>(math: &DeviceMath<Root<'a>>, value: f32) -> f32 {
    unsafe { math.sqrt_f32(value) }
}
pub fn packed_fma<'a>(math: &PolicyDeviceMath<'a, Root<'a>, StrictIeee>, x: fe2o3_device::Bf16x2) -> fe2o3_device::Bf16x2 {
    math.mul_add_bf16x2(x, x, x)
}
"#;

const PROBE_METADATA: &str = "fe2o3-policy-pairing-source-v1";
const PROBE_CHILD_ENV: &str = "FE2O3_POLICY_PAIRING_PROBE_CHILD";

struct Probe {
    completed: bool,
    amdgpu: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("policy_pairing_source.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            tcx.sess.target.llvm_target.starts_with("amdgcn"),
            self.amdgpu,
            "probe must inspect the requested target's metadata"
        );
        let definition = |name: &str| {
            tcx.iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap()
        };
        let lookalike = definition("with_numerical_policy").to_def_id();
        assert_eq!(trusted_device_items::classify(tcx, lookalike), None);
        for (name, marker) in [
            ("math_pair", TrustedDeviceItem::PolicyMathBind),
            ("matrix_pair", TrustedDeviceItem::PolicyMatrixBind),
            ("scoped_matrix_pair", TrustedDeviceItem::PolicyMatrixBind),
        ] {
            let body = tcx.optimized_mir(definition(name));
            let callee = body
                .basic_blocks
                .iter()
                .find_map(|block| {
                    let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                        return None;
                    };
                    let TyKind::FnDef(definition, args) = *func.ty(&body.local_decls, tcx).kind()
                    else {
                        return None;
                    };
                    Instance::try_resolve(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                        definition,
                        tcx.erase_and_anonymize_regions(args),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap();
            assert_eq!(
                trusted_device_items::classify(tcx, callee.def_id()),
                Some(marker),
                "{name}: constructor must authenticate before testing substitutions: {:?}",
                trusted_device_items::rejected_provider(tcx, callee.def_id())
            );
            validate_policy_bind_source_v1(tcx, callee).unwrap();
            assert_eq!(
                crate::production_semantic_terminal_v1::classify(tcx, callee.def_id()),
                None
            );
            let original = tcx.instance_mir(callee.def);
            let selected = crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, callee);
            assert!(
                std::ptr::eq(original, selected.body()),
                "pairing must retain the actual Rust body"
            );
            assert!(
                !selected.is_source_expansion(),
                "no synthetic replacement MIR"
            );
            let operands = original
                .basic_blocks
                .iter()
                .flat_map(|block| &block.statements)
                .find_map(|statement| {
                    let StatementKind::Assign(assignment) = &statement.kind else {
                        return None;
                    };
                    if assignment.0.local.index() != 0 || !assignment.0.projection.is_empty() {
                        return None;
                    }
                    let Rvalue::Aggregate(_, operands) = &assignment.1 else {
                        return None;
                    };
                    Some(operands)
                })
                .expect("original constructor aggregate");
            for (index, operand) in operands.iter().take(2).enumerate() {
                assert!(
                    matches!(operand, Operand::Copy(place) | Operand::Move(place)
                    if place.local.index() == index + 1 && place.projection.is_empty()),
                    "retain both inputs in source field order"
                );
            }
            assert!(operands.len() >= 3);
            let type_arguments = callee
                .args
                .iter()
                .enumerate()
                .filter_map(|(index, arg)| arg.as_type().map(|_| index))
                .collect::<Vec<_>>();
            for index in type_arguments {
                let mut arguments = callee.args.to_vec();
                arguments[index] = tcx.types.u16.into();
                let changed = Instance {
                    args: tcx.mk_args(&arguments),
                    ..callee
                };
                assert!(
                    validate_policy_bind_source_v1(tcx, changed).is_err(),
                    "{name}: substituted brand or policy at {index}"
                );
            }
            let mut arguments = callee.args.to_vec();
            arguments.push(tcx.types.u16.into());
            let changed = Instance {
                args: tcx.mk_args(&arguments),
                ..callee
            };
            assert!(
                validate_policy_bind_source_v1(tcx, changed).is_err(),
                "wrong generic arity"
            );
        }
        use super::super::math::validate_policy_math_source_v1;
        use fe2o3_kernel_ir::F32MathFunction as F;
        for (name, function) in [
            ("sqrt_f32", F::Sqrt),
            ("mul_add_f32", F::FusedMultiplyAdd),
            ("floor_f32", F::Floor),
            ("ceil_f32", F::Ceil),
            ("trunc_f32", F::Truncate),
            ("round_ties_even_f32", F::RoundTiesEven),
            ("sin_f32", F::Sin),
            ("cos_f32", F::Cos),
            ("exp_f32", F::Exp),
            ("exp2_f32", F::Exp2),
            ("ln_f32", F::Ln),
            ("log2_f32", F::Log2),
            ("log10_f32", F::Log10),
            ("bare_sqrt", F::Sqrt),
            ("packed_fma", F::FusedMultiplyAdd),
        ] {
            let body = tcx.optimized_mir(definition(name));
            let callee = body
                .basic_blocks
                .iter()
                .find_map(|block| {
                    let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                        return None;
                    };
                    let TyKind::FnDef(definition, arguments) =
                        *func.ty(&body.local_decls, tcx).kind()
                    else {
                        return None;
                    };
                    Instance::try_resolve(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                        definition,
                        tcx.erase_and_anonymize_regions(arguments),
                    )
                    .ok()
                    .flatten()
                })
                .expect("source consumer call");
            if matches!(name, "bare_sqrt" | "packed_fma") {
                assert!(
                    validate_policy_math_source_v1(tcx, callee, function).is_err(),
                    "{name}"
                );
                continue;
            }
            let source = validate_policy_math_source_v1(tcx, callee, function).unwrap();
            assert_eq!(
                crate::production_semantic_terminal_v1::classify(tcx, callee.def_id()),
                Some(crate::production_semantic_terminal_v1::ProductionSemanticTerminalRuleV1::Expand(
                    crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1::PolicyMathF32(function),
                )),
                "{name}: policy authority must survive terminal classification",
            );
            assert_eq!(source.function, function);
            assert_eq!(source.types[6], tcx.types.f32);
            for (reference, pointee) in [(0, 1), (2, 3), (4, 5)] {
                assert_eq!(
                    rust_shared_reference_v1(source.types[reference]),
                    Some(source.types[pointee])
                );
            }
            assert_eq!(
                rust_trusted_adt_type_arguments_v1(
                    tcx,
                    source.types[5],
                    TrustedDeviceItem::NumericalPolicyCapability
                ),
                Some(vec![source.kernel_brand, source.policy])
            );
            assert!(validate_policy_math_source_v1(tcx, callee, F::Abs).is_err());
            let other = if function == F::Sqrt { F::Exp } else { F::Sqrt };
            assert!(
                validate_policy_math_source_v1(tcx, callee, other).is_err(),
                "function substitution"
            );
            for (index, argument) in callee.args.iter().enumerate() {
                if argument.as_type().is_none() {
                    continue;
                }
                let mut arguments = callee.args.to_vec();
                arguments[index] = tcx.types.u16.into();
                let changed = Instance {
                    args: tcx.mk_args(&arguments),
                    ..callee
                };
                assert!(
                    validate_policy_math_source_v1(tcx, changed, function).is_err(),
                    "{name}: type {index}"
                );
            }
            let TyKind::Adt(brand_definition, brand_arguments) = *source.kernel_brand.kind() else {
                panic!("authenticated root brand");
            };
            let brand_index = callee
                .args
                .iter()
                .position(|argument| argument.as_type() == Some(source.kernel_brand))
                .unwrap();
            for axis in brand_arguments
                .iter()
                .enumerate()
                .filter_map(|(index, argument)| argument.as_type().map(|_| index))
                .skip(1)
            {
                let mut changed_brand = brand_arguments.to_vec();
                changed_brand[axis] = tcx.types.u16.into();
                let mut arguments = callee.args.to_vec();
                arguments[brand_index] =
                    Ty::new_adt(tcx, brand_definition, tcx.mk_args(&changed_brand)).into();
                assert!(
                    validate_policy_math_source_v1(
                        tcx,
                        Instance {
                            args: tcx.mk_args(&arguments),
                            ..callee
                        },
                        function
                    )
                    .is_err(),
                    "{name}: substituted target/launch axis {axis}"
                );
            }
            let mut arguments = callee.args.to_vec();
            arguments.push(tcx.types.u16.into());
            assert!(
                validate_policy_math_source_v1(
                    tcx,
                    Instance {
                        args: tcx.mk_args(&arguments),
                        ..callee
                    },
                    function
                )
                .is_err()
            );
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn configured_path(name: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(
        std::env::var_os(name)
            .unwrap_or_else(|| panic!("set {name} to an existing cached dependency path")),
    );
    assert!(path.exists(), "{name}: {}", path.display());
    path
}

fn run_policy_pairing_fixture(device: std::path::PathBuf, amdgpu: bool) {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };

    let observation = derive_cargo_metadata_build_observation_v2(&[PROBE_METADATA]).to_hex();
    if std::env::var(PROBE_CHILD_ENV).ok().as_deref() != Some(observation.as_str()) {
        // Match the real probe invocation's metadata without mutating the
        // shared test process's managed-build environment or source cache.
        let test = if amdgpu {
            "collector::production_importer_v1::numerical_policy_v1::bind::tests::policy_pairing_real_source_amdgpu_retains_references_and_rejects_substituted_types"
        } else {
            "collector::production_importer_v1::numerical_policy_v1::bind::tests::policy_pairing_real_source_retains_references_and_rejects_substituted_types"
        };
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(PROBE_CHILD_ENV, &observation)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "policy pairing child failed:\n{}\n{}",
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
    assert!(device.is_file());
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=policy_pairing_source".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Cpanic=abort".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        format!("-Cmetadata={PROBE_METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-".into(),
    ];
    if amdgpu {
        let host_deps = configured_path("FE2O3_CORE_TRY_HOST_DEPS");
        let core = configured_path("FE2O3_CORE_TRY_AMDGPU_CORE");
        let builtins = configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS");
        assert!(host_deps.is_dir());
        assert!(core.is_file() && builtins.is_file());
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Zunstable-options".into(),
            "-L".into(),
            format!("dependency={}", host_deps.display()),
            "--extern".into(),
            format!("noprelude,nounused:core={}", core.display()),
            "--extern".into(),
            format!(
                "noprelude,nounused:compiler_builtins={}",
                builtins.display()
            ),
        ]);
    }
    let mut probe = Probe {
        completed: false,
        amdgpu,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.completed);
}

#[test]
#[ignore = "requires existing host fe2o3-device metadata with encoded MIR: FE2O3_POLICY_DEVICE_RLIB; no dependency build is performed"]
fn policy_pairing_real_source_retains_references_and_rejects_substituted_types() {
    run_policy_pairing_fixture(configured_path("FE2O3_POLICY_DEVICE_RLIB"), false);
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, FE2O3_CORE_TRY_HOST_DEPS, FE2O3_CORE_TRY_AMDGPU_CORE and FE2O3_CORE_TRY_AMDGPU_BUILTINS; no dependency build is performed"]
fn policy_pairing_real_source_amdgpu_retains_references_and_rejects_substituted_types() {
    run_policy_pairing_fixture(configured_path("FE2O3_CORE_TRY_DEVICE_RMETA"), true);
}
