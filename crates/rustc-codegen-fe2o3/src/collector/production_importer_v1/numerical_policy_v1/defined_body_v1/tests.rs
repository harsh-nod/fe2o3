//! Real cached provider source checks. These tests do not construct root authority.

use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{Operand, Rvalue, StatementKind, TerminatorKind};
use rustc_session::config::Input;
use rustc_span::FileName;
use std::path::PathBuf;

const SOURCE: &str = r#"
#![no_std]
#![allow(deprecated)]
use fe2o3_device::{CurrentTarget, DeviceMath, KernelCapabilityBrand, KernelContext,
    NumericalPolicyCapability, PolicyDeviceMath, RegisteredLaunch, StrictIeee};
type Brand<'a> = KernelCapabilityBrand<'a, u8, CurrentTarget, RegisteredLaunch>;
pub fn getter<'a>(context: &KernelContext<'a, u8>) -> DeviceMath<Brand<'a>> { context.math() }
pub fn bind<'a>(math: &'a DeviceMath<Brand<'a>>, policy: &'a NumericalPolicyCapability<Brand<'a>, StrictIeee>)
    -> PolicyDeviceMath<'a, Brand<'a>, StrictIeee> { math.with_numerical_policy(policy) }
pub fn current() -> DeviceMath { DeviceMath::current() }
pub fn consumer<'a>(math: &PolicyDeviceMath<'a, Brand<'a>, StrictIeee>, value: f32) -> f32 { math.sqrt_f32(value) }
pub fn math(value: &u32) -> u32 { *value }
pub fn with_numerical_policy(left: &u32, right: &u32) -> u32 { *left + *right }
"#;

struct Probe {
    completed: bool,
    amdgpu: bool,
}

fn root<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .unwrap();
    Instance::mono(tcx, definition.to_def_id())
}

fn callee<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let root = root(tcx, name);
    source::forwarding_callee(tcx, root, tcx.instance_mir(root.def)).unwrap()
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("defined_math_source.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            tcx.sess.target.llvm_target.starts_with("amdgcn"),
            self.amdgpu
        );
        let getter = callee(tcx, "getter");
        let bind = callee(tcx, "bind");
        assert_eq!(source::kind(tcx, getter), Some(source::Kind::Getter));
        assert_eq!(source::kind(tcx, bind), Some(source::Kind::Bind));
        let getter_facts = source::getter(tcx, getter).unwrap();
        let bind_facts = source::bind(tcx, bind).unwrap();
        assert_eq!(getter_facts.brand.ty, bind_facts.brand.ty);
        assert_eq!(getter_facts.types[2], bind_facts.types[1]);
        assert_eq!(
            rust_shared_reference_v1(getter_facts.types[0]),
            Some(getter_facts.types[1])
        );
        assert_eq!(
            rust_shared_reference_v1(bind_facts.types[0]),
            Some(bind_facts.types[1])
        );
        assert_eq!(
            rust_shared_reference_v1(bind_facts.types[2]),
            Some(bind_facts.types[3])
        );
        assert_ne!(getter_facts.types[2], getter_facts.types[3]);
        assert_eq!(
            source::kind(tcx, getter_facts.bridge),
            None,
            "bridge alone is not an issuer"
        );
        assert_eq!(
            source::kind(tcx, getter_facts.current),
            None,
            "unbranded leaf is not a defined issuer"
        );
        for instance in [
            root(tcx, "math"),
            root(tcx, "with_numerical_policy"),
            callee(tcx, "current"),
            callee(tcx, "consumer"),
        ] {
            assert_eq!(source::kind(tcx, instance), None);
            assert!(source::getter(tcx, instance).is_err());
            assert!(source::bind(tcx, instance).is_err());
        }
        for instance in [getter, bind] {
            let selected = crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, instance);
            assert!(!selected.is_source_expansion());
            assert!(std::ptr::eq(
                selected.body(),
                tcx.instance_mir(instance.def)
            ));
            let mut arguments = instance.args.to_vec();
            arguments.push(tcx.types.u16.into());
            let changed = Instance {
                args: tcx.mk_args(&arguments),
                ..instance
            };
            assert!(match source::kind(tcx, instance).unwrap() {
                source::Kind::Getter => source::getter(tcx, changed).is_err(),
                source::Kind::Bind => source::bind(tcx, changed).is_err(),
            });
        }
        // Changing a kernel type is legitimate source monomorphization, but its
        // identity must change so the later authenticated-root check rejects a mix.
        let getter_arguments = getter
            .args
            .iter()
            .enumerate()
            .filter_map(|(i, arg)| arg.as_type().map(|_| i))
            .collect::<Vec<_>>();
        for (axis, index) in getter_arguments.into_iter().enumerate() {
            let mut arguments = getter.args.to_vec();
            arguments[index] = tcx.types.u16.into();
            let changed = Instance {
                args: tcx.mk_args(&arguments),
                ..getter
            };
            if axis == 0 {
                let changed = source::getter(tcx, changed).unwrap();
                assert_ne!(changed.brand.ty, getter_facts.brand.ty);
                assert_ne!(
                    rustc_type_identity_v1(tcx, changed.brand.kernel),
                    rustc_type_identity_v1(tcx, getter_facts.brand.kernel)
                );
            } else {
                assert!(source::getter(tcx, changed).is_err());
            }
        }
        for (index, argument) in bind.args.iter().enumerate() {
            if argument.as_type().is_none() {
                continue;
            }
            let mut arguments = bind.args.to_vec();
            arguments[index] = tcx.types.u16.into();
            assert!(
                source::bind(
                    tcx,
                    Instance {
                        args: tcx.mk_args(&arguments),
                        ..bind
                    }
                )
                .is_err()
            );
        }
        let original = tcx.instance_mir(getter.def);
        let mut erased = original.clone();
        erased.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK]
            .terminator_mut()
            .kind = TerminatorKind::Return;
        assert!(!body::kernel_math_getter(
            tcx,
            getter,
            &erased,
            getter_facts.bridge,
            getter_facts.types[0],
            getter_facts.types[2]
        ));
        assert!(!body::branded_math_bridge(
            tcx,
            getter_facts.bridge,
            tcx.instance_mir(getter_facts.bridge.def),
            getter_facts.bridge,
            getter_facts.types[2],
            getter_facts.types[3]
        ));
        for move_reference in [false, true] {
            let mut changed = tcx.instance_mir(bind.def).clone();
            let StatementKind::Assign(assignment) =
                &mut changed.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK].statements[0]
                    .kind
            else {
                panic!()
            };
            let Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
                panic!()
            };
            if move_reference {
                let Operand::Copy(place) = operands.raw[0] else {
                    panic!()
                };
                operands.raw[0] = Operand::Move(place);
            } else {
                operands.raw.swap(0, 1);
            }
            assert!(!body::policy_math_bind(
                tcx,
                bind,
                &changed,
                bind_facts.types[0],
                bind_facts.types[2],
                bind_facts.types[4]
            ));
        }
        self.completed = true;
        Compilation::Stop
    }
}

const METADATA: &str = "fe2o3-defined-math-source-v1";
const CHILD: &str = "FE2O3_DEFINED_MATH_SOURCE_CHILD";

fn configured_path(name: &str) -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os(name).unwrap_or_else(|| panic!("set {name} to an existing cached path")),
    );
    assert!(path.exists(), "{name}: {}", path.display());
    path
}

fn run(amdgpu: bool) {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    if std::env::var(CHILD).ok().as_deref() != Some(observation.as_str()) {
        let test = if amdgpu {
            "defined_math_original_source_amdgpu"
        } else {
            "defined_math_original_source_host"
        };
        let test = format!(
            "collector::production_importer_v1::numerical_policy_v1::defined_body_v1::tests::{test}"
        );
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, &observation)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
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
    let device = configured_path(if amdgpu {
        "FE2O3_CORE_TRY_DEVICE_RMETA"
    } else {
        "FE2O3_POLICY_DEVICE_RLIB"
    });
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-defined-math-source");
    let mut args = vec![
        "rustc".into(),
        "--crate-name=defined_math_source".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        format!("--out-dir={}", scratch.path().display()),
        "-Cpanic=abort".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        format!("-Cmetadata={METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-".into(),
    ];
    if amdgpu {
        let host = configured_path("FE2O3_CORE_TRY_HOST_DEPS");
        let core = configured_path("FE2O3_CORE_TRY_AMDGPU_CORE");
        let builtins = configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS");
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Zunstable-options".into(),
            "-L".into(),
            format!("dependency={}", host.display()),
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
#[ignore = "requires cached FE2O3_POLICY_DEVICE_RLIB; builds no dependencies"]
fn defined_math_original_source_host() {
    run(false);
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, FE2O3_CORE_TRY_HOST_DEPS, FE2O3_CORE_TRY_AMDGPU_CORE and FE2O3_CORE_TRY_AMDGPU_BUILTINS"]
fn defined_math_original_source_amdgpu() {
    run(true);
}
