use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{BasicBlock, Const, Local, TerminatorKind};
use rustc_session::config::Input;
use rustc_span::FileName;

#[path = "mutations.rs"]
mod mutations;

const SOURCE: &str = r#"
#![no_std]
#![allow(dead_code)]
pub fn scalar(a: Option<usize>, b: Option<usize>) -> Option<(usize, usize)> { a.zip(b) }
pub fn different(a: Option<u32>, b: Option<f32>) -> Option<(u32, f32)> { a.zip(b) }
pub fn unit(a: Option<()>, b: Option<()>) -> Option<((), ())> { a.zip(b) }
pub struct NonCopy(pub u32);
pub struct Owned(pub u32);
impl Drop for Owned { fn drop(&mut self) {} }
pub struct Panicking;
impl Drop for Panicking { fn drop(&mut self) { panic!("payload drop"); } }
pub struct UnsafeDrop(pub *mut u32);
impl Drop for UnsafeDrop { fn drop(&mut self) { unsafe { *self.0 = 0; } } }
pub fn noncopy(a: Option<NonCopy>, b: Option<NonCopy>) -> Option<(NonCopy, NonCopy)> { a.zip(b) }
pub fn owned(a: Option<Owned>, b: Option<Owned>) -> Option<(Owned, Owned)> { a.zip(b) }
pub fn left_owned(a: Option<Owned>, b: Option<u32>) -> Option<(Owned, u32)> { a.zip(b) }
pub fn right_owned(a: Option<u32>, b: Option<Owned>) -> Option<(u32, Owned)> { a.zip(b) }
pub fn aggregate(a: Option<[Owned; 2]>, b: Option<(Owned, Owned)>) -> Option<([Owned; 2], (Owned, Owned))> { a.zip(b) }
pub fn borrowed<'a>(a: Option<&'a mut u32>, b: Option<&'a mut u32>) -> Option<(&'a mut u32, &'a mut u32)> { a.zip(b) }
pub fn panic_drop(a: Option<Panicking>, b: Option<Panicking>) -> Option<(Panicking, Panicking)> { a.zip(b) }
pub fn unsafe_drop(a: Option<UnsafeDrop>, b: Option<UnsafeDrop>) -> Option<(UnsafeDrop, UnsafeDrop)> { a.zip(b) }
pub fn wrong_method(a: Option<usize>, b: Option<usize>) -> Option<usize> { a.and(b) }
pub struct Impostor;
impl Impostor { pub fn zip(self, _: Option<usize>) -> Option<(usize, usize)> { None } }
pub fn impostor(b: Option<usize>) -> Option<(usize, usize)> { Impostor.zip(b) }
pub fn model<T, U>(a: Option<T>, b: Option<U>) -> Option<(T, U)> {
    match (a,b) { (Some(a), Some(b)) => Some((a,b)), _ => None }
}
pub fn foreign(a: Option<usize>, b: Option<usize>) -> Option<(usize, usize)> { model(a,b) }
pub fn panic_route() { panic!("not a zip"); }
"#;

fn local_body<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> &'tcx Body<'tcx> {
    let def = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .expect("fixture function");
    tcx.optimized_mir(def)
}

fn helper<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    local_body(tcx, name)
        .basic_blocks
        .iter()
        .find_map(|block| {
            let TerminatorKind::Call {
                func: Operand::Constant(callee),
                ..
            } = &block.terminator().kind
            else {
                return None;
            };
            let TyKind::FnDef(def, args) = *callee.const_.ty().kind() else {
                return None;
            };
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def, args).ok()?
        })
        .unwrap_or_else(|| panic!("retained helper call: {name}"))
}

fn instances(tcx: TyCtxt<'_>) {
    for name in [
        "scalar",
        "different",
        "unit",
        "noncopy",
        "owned",
        "left_owned",
        "right_owned",
        "aggregate",
        "borrowed",
        "panic_drop",
        "unsafe_drop",
    ] {
        let instance = helper(tcx, name);
        let c = contract(tcx, instance)
            .unwrap_or_else(|| panic!("nominal contract: {name}: {instance:?}"));
        let source = tcx.instance_mir(instance.def);
        let before = format!("{source:?}");
        for _ in 0..2 {
            assert!(
                authenticate_reviewed_safe_core_option_zip_helper_v1(tcx, instance),
                "{name}: {:?} {:?}",
                source.local_decls,
                source.basic_blocks
            );
        }
        assert_eq!(before, format!("{source:?}"));
        assert!(std::ptr::eq(source, tcx.instance_mir(instance.def)));
        assert!(
            source
                .basic_blocks
                .iter()
                .all(|b| !matches!(b.terminator().kind, TerminatorKind::Call { .. }))
        );
        let drops = source
            .basic_blocks
            .iter()
            .filter(|b| matches!(b.terminator().kind, TerminatorKind::Drop { .. }))
            .count();
        assert!(drops >= 2, "both original payload drop paths retained");
        for raw in c.parameters {
            let concrete = normalized(tcx, instance, raw).unwrap();
            assert!(
                !authenticate_reviewed_safe_core_option_zip_helper_v1(
                    tcx,
                    Instance::resolve_drop_in_place(tcx, concrete)
                ),
                "no authority for payload drop glue: {name}"
            );
        }
    }
    for name in ["wrong_method", "impostor", "foreign", "panic_route"] {
        assert!(
            !authenticate_reviewed_safe_core_option_zip_helper_v1(tcx, helper(tcx, name)),
            "{name}"
        );
    }
    let instance = helper(tcx, "scalar");
    let c = contract(tcx, instance).unwrap();
    for args in [
        vec![],
        vec![tcx.types.usize.into()],
        vec![tcx.types.usize.into(); 3],
        vec![tcx.lifetimes.re_erased.into(); 2],
        c.parameters.iter().map(|p| (*p).into()).collect(),
    ] {
        assert!(!authenticate_reviewed_safe_core_option_zip_helper_v1(
            tcx,
            Instance {
                args: tcx.mk_args(&args),
                ..instance
            }
        ));
    }
    assert!(!authenticate_reviewed_safe_core_option_zip_helper_v1(
        tcx,
        Instance {
            def: InstanceKind::Intrinsic(instance.def_id()),
            ..instance
        }
    ));
    let other = helper(tcx, "different");
    assert!(
        !body::reviewed(tcx, other, tcx.instance_mir(instance.def), &c),
        "instance-bound proof cannot transfer"
    );
}

struct Probe {
    mutations: bool,
    amdgpu: bool,
    ran: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("option_zip.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            tcx.sess.target.llvm_target.as_ref() == "amdgcn-amd-amdhsa",
            self.amdgpu
        );
        instances(tcx);
        if self.mutations {
            mutations::check(tcx);
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(mutations: bool, amdgpu: bool, low_opt: bool, abort: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=option_zip".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cdebuginfo=2".into(),
        "-Zalways-encode-mir".into(),
        format!("-Cpanic={}", if abort { "abort" } else { "unwind" }),
    ];
    if low_opt {
        args.push("-Zmir-opt-level=0".into());
    }
    if amdgpu {
        let metadata = |name| {
            let path = std::path::PathBuf::from(
                std::env::var_os(name)
                    .unwrap_or_else(|| panic!("set {name} to existing pinned AMDGPU metadata")),
            );
            assert!(path.is_file(), "{name}: {}", path.display());
            path
        };
        let core = metadata("FE2O3_WRAPPING_AMDGPU_CORE");
        let builtins = metadata("FE2O3_WRAPPING_AMDGPU_BUILTINS");
        let cpu = std::env::var("FE2O3_WRAPPING_TARGET_CPU").unwrap_or_else(|_| "gfx942".into());
        assert!(matches!(cpu.as_str(), "gfx942" | "gfx950"));
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            format!("-Ctarget-cpu={cpu}"),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Zunstable-options".into(),
            "--extern".into(),
            format!("noprelude,nounused:core={}", core.display()),
            "--extern".into(),
            format!(
                "noprelude,nounused:compiler_builtins={}",
                builtins.display()
            ),
            "-L".into(),
            format!("dependency={}", core.parent().unwrap().display()),
        ]);
        if core.parent() != builtins.parent() {
            args.extend([
                "-L".into(),
                format!("dependency={}", builtins.parent().unwrap().display()),
            ]);
        }
    }
    args.push("-".into());
    let mut probe = Probe {
        mutations,
        amdgpu,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn option_zip_actual_host_generic_ownership_and_identity() {
    run(false, false, false, true);
}
#[test]
fn option_zip_actual_host_unwind_preserves_both_drop_paths() {
    run(true, false, false, false);
}
#[test]
fn option_zip_actual_host_body_mutations_and_bounds() {
    run(true, false, true, true);
}
#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn option_zip_actual_amdgpu_generic_body_and_mutations() {
    run(true, true, false, true);
}
#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn option_zip_actual_amdgpu_mir_opt_zero() {
    run(true, true, true, true);
}
