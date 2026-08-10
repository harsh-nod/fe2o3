use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{Instance, TyCtxt, TyKind, TypingEnv};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const FIXTURE_SOURCE: &str = r#"
#![allow(dead_code)]

struct Token(u32);

#[inline(never)]
fn consume(token: Token) -> u32 { token.0 }

#[inline(never)]
fn host_apply<F: FnOnce(u32) -> u32>(closure: F, value: u32) -> u32 {
    closure(value)
}

#[inline(never)]
fn host_registered(seed: u32) -> u32 {
    let token = Token(seed);
    let closure = move |delta: u32| consume(token).wrapping_add(delta);
    host_apply(closure, 7)
}

#[used]
static HOST_REGISTRATION: fn(u32) -> u32 = host_registered;

#[inline(never)]
fn device_fn(seed: u32) -> u32 {
    let closure = move |delta: u32| seed.wrapping_add(delta);
    closure(1).wrapping_add(closure(2))
}

#[inline(never)]
fn device_fn_mut(mut seed: u32) -> u32 {
    let mut closure = |delta: u32| {
        seed = seed.wrapping_add(delta);
        seed
    };
    closure(1).wrapping_add(closure(2))
}

#[inline(never)]
fn device_fn_once(seed: u32) -> u32 {
    let token = Token(seed);
    let closure = move |delta: u32| consume(token).wrapping_add(delta);
    closure(3)
}

#[inline(never)]
fn host_ref_apply<F: FnOnce(u32) -> u32>(closure: F, value: u32) -> u32 {
    closure(value)
}

#[inline(never)]
fn host_ref_registered(seed: u32) -> u32 {
    let reference = &seed;
    let closure = move |delta: u32| reference.wrapping_add(delta);
    host_ref_apply(closure, 1)
}

#[inline(never)]
fn passthrough<F>(value: F) -> F { value }

#[inline(never)]
fn escaped(seed: u32) -> u32 {
    let closure = move |delta: u32| seed.wrapping_add(delta);
    let closure = passthrough(closure);
    closure(1)
}

#[inline(never)]
fn raw_capture(pointer: *const u32) -> u32 {
    let closure = move || pointer as usize as u32;
    closure()
}

#[inline(never)]
fn dynamic_dispatch(closure: &dyn Fn(u32) -> u32) -> u32 {
    closure(1)
}

#[inline(never)]
fn returned(seed: u32) -> impl Fn(u32) -> u32 {
    move |delta: u32| seed.wrapping_add(delta)
}

#[inline(never)]
fn projected_reference(seed: u32) -> u32 {
    let closure = move |delta: u32| seed.wrapping_add(delta);
    let mut slot = (&closure,);
    slot.0 = &closure;
    let _ = slot;
    closure(1)
}

#[inline(never)]
fn inline_asm_escape(seed: u32) -> u32 {
    let closure = move |delta: u32| seed.wrapping_add(delta);
    let reference = &closure;
    unsafe {
        core::arch::asm!("/* {0} */", in(reg) reference, options(nomem, nostack));
    }
    closure(1)
}
"#;

#[derive(Clone, Debug)]
struct DriverResults {
    host: Gfx942ClosureLoweringV1,
    device_fn: Gfx942ClosureLoweringV1,
    device_fn_mut: Gfx942ClosureLoweringV1,
    device_fn_once: Gfx942ClosureLoweringV1,
    host_ref_error: String,
    escape_error: String,
    raw_error: String,
    dynamic_error: String,
    return_error: String,
    projected_error: String,
    asm_error: String,
    origin_error: String,
    target_error: String,
}

#[derive(Default)]
struct CaptureCallbacks {
    results: Option<DriverResults>,
}

impl Callbacks for CaptureCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let host_registered = Instance::mono(tcx, local_function(tcx, "host_registered"));
        let host = resolved_call_named(tcx, host_registered, "host_apply");
        let host_ref_registered = Instance::mono(tcx, local_function(tcx, "host_ref_registered"));
        let host_ref = resolved_call_named(tcx, host_ref_registered, "host_ref_apply");
        let device_fn_instance = Instance::mono(tcx, local_function(tcx, "device_fn"));
        let device_fn_mut_instance = Instance::mono(tcx, local_function(tcx, "device_fn_mut"));
        let device_fn_once_instance = Instance::mono(tcx, local_function(tcx, "device_fn_once"));

        let host_plan = analyze_gfx942_closures_v1(
            tcx,
            host,
            ClosureOriginPolicyV1::HostArgument,
            "gfx942:xnack-",
        )
        .expect("host closure registration must be admitted");
        let repeated = analyze_gfx942_closures_v1(
            tcx,
            host,
            ClosureOriginPolicyV1::HostArgument,
            "gfx942:xnack-",
        )
        .expect("repeat host admission");
        assert_eq!(host_plan.identity(), repeated.identity());

        self.results = Some(DriverResults {
            host: host_plan,
            device_fn: analyze_gfx942_closures_v1(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect("device Fn closure"),
            device_fn_mut: analyze_gfx942_closures_v1(
                tcx,
                device_fn_mut_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect("device FnMut closure"),
            device_fn_once: analyze_gfx942_closures_v1(
                tcx,
                device_fn_once_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect("device FnOnce closure"),
            host_ref_error: analyze_gfx942_closures_v1(
                tcx,
                host_ref,
                ClosureOriginPolicyV1::HostArgument,
                "gfx942",
            )
            .expect_err("host reference capture must require allocation authority")
            .to_string(),
            escape_error: analyze_gfx942_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "escaped")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("escaping closure must fail")
            .to_string(),
            raw_error: analyze_gfx942_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "raw_capture")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("raw capture must fail")
            .to_string(),
            dynamic_error: analyze_gfx942_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "dynamic_dispatch")),
                ClosureOriginPolicyV1::Either,
                "gfx942",
            )
            .expect_err("dynamic dispatch must fail")
            .to_string(),
            return_error: analyze_gfx942_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "returned")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("returned closure must escape")
            .to_string(),
            projected_error: analyze_gfx942_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "projected_reference")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("projected closure reference destination must fail")
            .to_string(),
            asm_error: analyze_gfx942_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "inline_asm_escape")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("inline assembly closure use must fail")
            .to_string(),
            origin_error: analyze_gfx942_closures_v1(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::HostArgument,
                "gfx942",
            )
            .expect_err("device closure must not satisfy host registration policy")
            .to_string(),
            target_error: analyze_gfx942_closures_v1(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx1100",
            )
            .expect_err("unsupported GPU target must fail closed")
            .to_string(),
        });
        Compilation::Stop
    }
}

fn local_function(tcx: TyCtxt<'_>, name: &str) -> rustc_hir::def_id::DefId {
    tcx.iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing fixture function `{name}`"))
        .to_def_id()
}

fn resolved_call_named<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    expected: &str,
) -> Instance<'tcx> {
    tcx.instance_mir(caller.def)
        .basic_blocks
        .iter()
        .filter_map(|block| {
            let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                return None;
            };
            let Operand::Constant(constant) = func else {
                return None;
            };
            let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
                return None;
            };
            let normalized_args = caller
                .try_instantiate_mir_and_normalize_erasing_regions(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    rustc_middle::ty::EarlyBinder::bind(*args),
                )
                .ok()?;
            let resolved = Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                *def_id,
                normalized_args,
            )
            .ok()
            .flatten()?;
            (tcx.item_name(resolved.def_id()).as_str() == expected).then_some(resolved)
        })
        .next()
        .unwrap_or_else(|| panic!("missing resolved call to `{expected}`"))
}

struct CompilerFixture {
    root: PathBuf,
    source: PathBuf,
    output: PathBuf,
}

impl CompilerFixture {
    fn create() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("fe2o3-closure-v1-{}-{serial}", std::process::id()));
        let source = root.join("fixture.rs");
        let output = root.join("fixture.rmeta");
        fs::create_dir(&root).expect("create closure fixture directory");
        fs::write(&source, FIXTURE_SOURCE).expect("write closure fixture");
        Self {
            root,
            source,
            output,
        }
    }
}

impl Drop for CompilerFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn compiler_results() -> DriverResults {
    static RESULTS: OnceLock<DriverResults> = OnceLock::new();
    RESULTS
        .get_or_init(|| {
            let fixture = CompilerFixture::create();
            let sysroot = Command::new("rustc")
                .args(["--print", "sysroot"])
                .output()
                .expect("query rustc sysroot");
            assert!(sysroot.status.success());
            let sysroot = String::from_utf8(sysroot.stdout)
                .expect("UTF-8 sysroot")
                .trim()
                .to_owned();
            let args = vec![
                "rustc".to_owned(),
                "--crate-name".to_owned(),
                "fe2o3_closure_v1_fixture".to_owned(),
                "--crate-type".to_owned(),
                "lib".to_owned(),
                "--edition".to_owned(),
                "2024".to_owned(),
                "--emit".to_owned(),
                "metadata".to_owned(),
                "-Zmir-opt-level=0".to_owned(),
                "-Coverflow-checks=off".to_owned(),
                "--sysroot".to_owned(),
                sysroot,
                "-o".to_owned(),
                fixture.output.display().to_string(),
                fixture.source.display().to_string(),
            ];
            static DRIVER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let _guard = DRIVER_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut callbacks = CaptureCallbacks::default();
            rustc_driver::run_compiler(&args, &mut callbacks);
            callbacks.results.expect("closure callback did not run")
        })
        .clone()
}

#[test]
fn host_registration_has_exact_by_value_environment_and_static_once_call() {
    let plan = compiler_results().host;
    assert_eq!(plan.environments().len(), 1);
    assert_eq!(plan.calls().len(), 1);
    assert_ne!(plan.identity(), [0; 32]);
    let environment = &plan.environments()[0];
    assert_eq!(environment.origin, ClosureOriginV1::HostArgument);
    assert_eq!(environment.call_kind, ClosureCallKindV1::FnOnce);
    assert_eq!(environment.captures.len(), 1);
    assert_eq!(environment.captures[0].mode, ClosureCaptureModeV1::ByValue);
    assert_eq!(environment.captures[0].offset_bytes, 0);
    assert_eq!(environment.captures[0].layout.size_bytes, 4);
    assert_eq!(plan.calls()[0].call_kind, ClosureCallKindV1::FnOnce);
    assert_eq!(plan.calls()[0].argument_count, 1);
}

#[test]
fn device_internal_fn_fnmut_and_fnonce_lower_to_bounded_static_calls() {
    let results = compiler_results();
    let cases = [
        (results.device_fn, ClosureCallKindV1::Fn, 2),
        (results.device_fn_mut, ClosureCallKindV1::FnMut, 2),
        (results.device_fn_once, ClosureCallKindV1::FnOnce, 1),
    ];
    for (plan, kind, calls) in cases {
        assert_eq!(plan.environments().len(), 1);
        assert_eq!(plan.calls().len(), calls);
        assert_eq!(
            plan.environments()[0].origin,
            ClosureOriginV1::DeviceInternal
        );
        assert_eq!(plan.environments()[0].call_kind, kind);
        assert!(plan.calls().iter().all(|call| call.call_kind == kind));
        assert!(
            plan.calls()
                .iter()
                .all(|call| call.target_definition_hash != [0; 16])
        );
    }
    assert_eq!(
        cases_capture_mode(ClosureCallKindV1::FnMut),
        ClosureCaptureModeV1::MutableReference
    );
}

fn cases_capture_mode(kind: ClosureCallKindV1) -> ClosureCaptureModeV1 {
    let results = compiler_results();
    let plan = match kind {
        ClosureCallKindV1::Fn => results.device_fn,
        ClosureCallKindV1::FnMut => results.device_fn_mut,
        ClosureCallKindV1::FnOnce => results.device_fn_once,
    };
    plan.environments()[0].captures[0].mode
}

#[test]
fn unsupported_authority_escape_and_dispatch_paths_fail_closed() {
    let results = compiler_results();
    assert!(
        results
            .host_ref_error
            .contains("allocation/completion token"),
        "{}",
        results.host_ref_error
    );
    assert!(
        results.escape_error.contains("closure escapes"),
        "{}",
        results.escape_error
    );
    assert!(
        results.raw_error.contains("raw-pointer captures"),
        "{}",
        results.raw_error
    );
    assert!(
        results.dynamic_error.contains("dynamic dispatch"),
        "{}",
        results.dynamic_error
    );
    assert!(
        results.return_error.contains("escapes"),
        "{}",
        results.return_error
    );
    assert!(
        results.projected_error.contains("unsupported assignment"),
        "{}",
        results.projected_error
    );
    assert!(
        results.asm_error.contains("inline assembly")
            || results.asm_error.contains("unsupported assignment"),
        "{}",
        results.asm_error
    );
    assert!(
        results.origin_error.contains("does not satisfy policy"),
        "{}",
        results.origin_error
    );
    assert!(
        results.target_error.contains("supports gfx942"),
        "{}",
        results.target_error
    );
}
