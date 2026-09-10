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
fn host_apply_second<F: FnOnce(u32) -> u32>(value: u32, closure: F) -> u32 {
    closure(value)
}

#[inline(never)]
fn host_registered_second(seed: u32) -> u32 {
    let token = Token(seed);
    let closure = move |delta: u32| consume(token).wrapping_add(delta);
    host_apply_second(7, closure)
}

#[inline(never)]
fn borrowed_apply<F: Fn(u32) -> u32>(closure: &F, value: u32) -> u32 {
    closure(value)
}

#[inline(never)]
fn borrowed_transport(seed: u32) -> u32 {
    let closure = move |delta: u32| seed.wrapping_add(delta);
    borrowed_apply(&closure, 1)
}

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
fn forward_ref<F: FnOnce(u32) -> u32>(closure: F, value: u32) -> u32 {
    host_ref_apply(closure, value)
}

#[inline(never)]
fn ref_chain(mut seed: u32) -> u32 {
    let closure = |delta: u32| {
        seed = seed.wrapping_add(delta);
        seed
    };
    forward_ref(closure, 1)
}

#[inline(never)]
fn constant_transport(value: u32) -> u32 {
    host_ref_apply(|delta| delta, value)
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
fn wrapped_passthrough<F>(value: F) -> Option<F> { Some(value) }

#[inline(never)]
fn wrapped_escape(seed: u32) -> u32 {
    let closure = move |delta: u32| seed.wrapping_add(delta);
    let _escaped = wrapped_passthrough(closure);
    0
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
    host: ProductionClosureLoweringV1,
    transported: ProductionClosureLoweringV1,
    transported_second: ProductionClosureLoweringV1,
    device_fn: ProductionClosureLoweringV1,
    device_fn_mut: ProductionClosureLoweringV1,
    device_fn_once: ProductionClosureLoweringV1,
    gfx950: ProductionClosureLoweringV1,
    host_ref_error: String,
    escape_error: String,
    wrapped_escape_error: String,
    borrowed_transport_error: String,
    raw_error: String,
    dynamic_error: String,
    return_error: String,
    projected_error: String,
    asm_error: String,
    origin_error: String,
    target_error: String,
    custody: CustodyResults,
    large_environment: ProductionClosureLoweringV1,
    capture_budget_errors: Vec<String>,
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

        let host_plan = analyze_production_closures_v1(
            tcx,
            host,
            ClosureOriginPolicyV1::HostArgument,
            "gfx942:xnack-",
        )
        .expect("host closure registration must be admitted");
        let repeated = analyze_production_closures_v1(
            tcx,
            host,
            ClosureOriginPolicyV1::HostArgument,
            "gfx942:xnack-",
        )
        .expect("repeat host admission");
        assert_eq!(host_plan.identity(), repeated.identity());

        self.results = Some(DriverResults {
            host: host_plan,
            transported: analyze_production_closures_v1(
                tcx,
                host_registered,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942:xnack-",
            )
            .expect("bounded by-value closure transport"),
            transported_second: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "host_registered_second")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942:xnack-",
            )
            .expect("bounded by-value closure transport in argument one"),
            device_fn: analyze_production_closures_v1(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect("device Fn closure"),
            device_fn_mut: analyze_production_closures_v1(
                tcx,
                device_fn_mut_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect("device FnMut closure"),
            device_fn_once: analyze_production_closures_v1(
                tcx,
                device_fn_once_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect("device FnOnce closure"),
            gfx950: analyze_production_closures_v1(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx950:xnack-",
            )
            .expect("gfx950 uses the same target-bound closure profile"),
            host_ref_error: analyze_production_closures_v1(
                tcx,
                host_ref,
                ClosureOriginPolicyV1::HostArgument,
                "gfx942",
            )
            .expect_err("host reference capture must require allocation authority")
            .to_string(),
            escape_error: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "escaped")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("escaping closure must fail")
            .to_string(),
            wrapped_escape_error: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "wrapped_escape")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942:xnack-",
            )
            .expect_err("transport must reject a closure-bearing return type")
            .to_string(),
            borrowed_transport_error: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "borrowed_transport")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942:xnack-",
            )
            .expect_err("ordinary helper transport must not borrow a closure")
            .to_string(),
            raw_error: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "raw_capture")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("raw capture must fail")
            .to_string(),
            dynamic_error: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "dynamic_dispatch")),
                ClosureOriginPolicyV1::Either,
                "gfx942",
            )
            .expect_err("dynamic dispatch must fail")
            .to_string(),
            return_error: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "returned")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("returned closure must escape")
            .to_string(),
            projected_error: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "projected_reference")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("projected closure reference destination must fail")
            .to_string(),
            asm_error: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "inline_asm_escape")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx942",
            )
            .expect_err("inline assembly closure use must fail")
            .to_string(),
            origin_error: analyze_production_closures_v1(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::HostArgument,
                "gfx942",
            )
            .expect_err("device closure must not satisfy host registration policy")
            .to_string(),
            target_error: analyze_production_closures_v1(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx1100",
            )
            .expect_err("unsupported GPU target must fail closed")
            .to_string(),
            custody: custody_results(tcx, host_ref_registered, host_ref),
            large_environment: analyze_production_closures_v1(
                tcx,
                Instance::mono(tcx, local_function(tcx, "large_environment")),
                ClosureOriginPolicyV1::DeviceInternal,
                "gfx950",
            )
            .expect("37 captures fit the unchanged aggregate budget"),
            capture_budget_errors: [
                "too_many_captures",
                "aggregate_captures",
                "oversized_environment",
            ]
            .into_iter()
            .map(|name| {
                analyze_production_closures_v1(
                    tcx,
                    Instance::mono(tcx, local_function(tcx, name)),
                    ClosureOriginPolicyV1::DeviceInternal,
                    "gfx950",
                )
                .expect_err("the aggregate budget remains bounded")
                .to_string()
            })
            .collect(),
        });
        Compilation::Stop
    }
}

#[derive(Clone, Debug)]
struct CustodyResults {
    shared: ProductionClosureLoweringV1,
    chained: ProductionClosureLoweringV1,
    missing: String,
    entry: String,
    target: String,
    untracked_call: String,
    altered_callee: String,
    altered_argument: String,
    altered_caller: String,
    altered_root: String,
}

fn custody_results<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    callee: Instance<'tcx>,
) -> CustodyResults {
    let mut custody = CollectedClosureCustodyV1::default();
    let missing = custody
        .analyze(tcx, callee, true, "gfx942")
        .expect_err("helper role alone does not confer borrow custody")
        .to_string();
    let caller_plan = custody
        .analyze(tcx, caller, false, "gfx942")
        .expect("device-created shared borrow environment");
    custody
        .observe_calls(tcx, caller, Some(&caller_plan))
        .expect("exact caller transport");
    let shared = custody
        .analyze(tcx, callee, true, "gfx942")
        .expect("shared reference retains caller custody");
    let entry = custody
        .analyze(tcx, callee, false, "gfx942")
        .expect_err("an exported entry cannot reuse internal custody")
        .to_string();
    let target = custody
        .analyze(tcx, callee, true, "gfx950")
        .expect_err("custody is target-bound")
        .to_string();
    let untracked_call = custody
        .observe_calls(tcx, caller, None)
        .expect_err("a previous incoming edge cannot authorize an untracked call")
        .to_string();
    let mut changed = caller_plan.clone();
    let other_caller = Instance::mono(tcx, local_function(tcx, "host_registered"));
    let altered_caller = custody
        .observe_calls(tcx, other_caller, Some(&changed))
        .expect_err("caller substitution must reject")
        .to_string();
    let ClosureCustodyV1::EnvironmentLocal(local) = &mut changed.transport_calls[0].closure_custody
    else {
        panic!("fixture transports a local environment");
    };
    *local += 1;
    let altered_root = custody
        .observe_calls(tcx, caller, Some(&changed))
        .expect_err("environment-root substitution must reject")
        .to_string();
    changed = caller_plan.clone();
    changed.transport_calls[0].target_mir_identity[0] ^= 1;
    let altered_callee = custody
        .observe_calls(tcx, caller, Some(&changed))
        .expect_err("callee MIR substitution must reject")
        .to_string();
    changed = caller_plan;
    changed.transport_calls[0].argument_index += 1;
    let altered_argument = custody
        .observe_calls(tcx, caller, Some(&changed))
        .expect_err("argument-slot substitution must reject")
        .to_string();

    let root = Instance::mono(tcx, local_function(tcx, "ref_chain"));
    let middle = resolved_call_named(tcx, root, "forward_ref");
    let leaf = resolved_call_named(tcx, middle, "host_ref_apply");
    let root_plan = custody
        .analyze(tcx, root, false, "gfx942")
        .expect("mutable source borrow");
    custody
        .observe_calls(tcx, root, Some(&root_plan))
        .expect("root transfer");
    let middle_plan = custody
        .analyze(tcx, middle, true, "gfx942")
        .expect("middle custody");
    custody
        .observe_calls(tcx, middle, Some(&middle_plan))
        .expect("nested transfer");
    let chained = custody
        .analyze(tcx, leaf, true, "gfx942")
        .expect("leaf custody");
    CustodyResults {
        shared,
        chained,
        missing,
        entry,
        target,
        untracked_call,
        altered_callee,
        altered_argument,
        altered_caller,
        altered_root,
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
        let mut text = FIXTURE_SOURCE.to_owned();
        append_capture_fixture(&mut text, "large_environment", 1, 37);
        append_capture_fixture(&mut text, "too_many_captures", 1, MAX_TOTAL_CAPTURES + 1);
        append_capture_fixture(&mut text, "aggregate_captures", 2, 80);
        text.push_str(&format!(
            "fn oversized_environment(seed: u8) -> u8 {{ let values = [seed; {}]; let closure = move || values[0]; closure() }}",
            MAX_TOTAL_ENVIRONMENT_BYTES + 1,
        ));
        fs::write(&source, text).expect("write closure fixture");
        Self {
            root,
            source,
            output,
        }
    }
}

fn append_capture_fixture(source: &mut String, name: &str, closures: usize, captures: usize) {
    use std::fmt::Write;
    write!(source, "\nfn {name}(seed: u32) -> u32 {{").unwrap();
    for capture in 0..captures {
        write!(source, "let c{capture} = seed.wrapping_add({capture});").unwrap();
    }
    for closure in 0..closures {
        write!(source, "let closure{closure} = || c0").unwrap();
        for capture in 1..captures {
            write!(source, ".wrapping_add(c{capture})").unwrap();
        }
        source.push(';');
    }
    source.push_str("closure0()");
    for closure in 1..closures {
        write!(source, ".wrapping_add(closure{closure}())").unwrap();
    }
    source.push_str("}\n");
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
            let mut callbacks = CaptureCallbacks::default();
            run_fixture(&mut callbacks, 0);
            callbacks.results.expect("closure callback did not run")
        })
        .clone()
}

fn run_fixture(callbacks: &mut (dyn Callbacks + Send), mir_optimization: u8) {
    let fixture = CompilerFixture::create();
    let mut command = Command::new("rustc");
    command.args(["--print", "sysroot"]);
    let sysroot =
        crate::process_execution::capture_output(&mut command).expect("query rustc sysroot");
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
        format!("-Zmir-opt-level={mir_optimization}"),
        "-Zinline-mir=no".to_owned(),
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
    rustc_driver::run_compiler(&args, callbacks);
}

#[test]
fn optimized_zero_capture_transport_has_exact_constant_custody() {
    #[derive(Default)]
    struct Check {
        completed: bool,
    }
    impl Callbacks for Check {
        fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let caller = Instance::mono(tcx, local_function(tcx, "constant_transport"));
            assert!(contains_concrete_closure_v1(tcx, caller).unwrap());
            let mut custody = CollectedClosureCustodyV1::default();
            let plan = custody
                .analyze(tcx, caller, false, "gfx942")
                .expect("constant transport");
            assert!(plan.environments().is_empty());
            assert_eq!(plan.transport_calls().len(), 1);
            let ClosureCustodyV1::ZeroSizedConstant {
                closure_type_identity,
                ..
            } = &plan.transport_calls()[0].closure_custody
            else {
                panic!("optimized fixture must use constant custody");
            };
            assert_eq!(
                plan.authenticated_closure_type_identities()
                    .collect::<Vec<_>>(),
                vec![*closure_type_identity]
            );
            custody
                .observe_calls(tcx, caller, Some(&plan))
                .expect("constant incoming edge");
            let callee = resolved_call_named(tcx, caller, "host_ref_apply");
            let receiving = custody
                .analyze(tcx, callee, true, "gfx942")
                .expect("constant receiving argument");
            assert!(receiving.environments()[0].captures.is_empty());
            assert_eq!(receiving.environments()[0].size_bytes, 0);
            let mut changed = plan;
            let ClosureCustodyV1::ZeroSizedConstant {
                operand_source_identity,
                ..
            } = &mut changed.transport_calls[0].closure_custody
            else {
                unreachable!()
            };
            operand_source_identity[0] ^= 1;
            assert!(custody.observe_calls(tcx, caller, Some(&changed)).is_err());
            self.completed = true;
            Compilation::Stop
        }
    }
    let mut check = Check::default();
    run_fixture(&mut check, 3);
    assert!(check.completed);
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
fn device_borrow_custody_follows_exact_shared_and_mutable_transports() {
    let results = compiler_results().custody;
    for (plan, mode) in [
        (results.shared, ClosureCaptureModeV1::SharedReference),
        (results.chained, ClosureCaptureModeV1::MutableReference),
    ] {
        let [environment] = plan.environments() else {
            panic!("one environment expected")
        };
        assert_eq!(environment.origin, ClosureOriginV1::DeviceTransport);
        assert_ne!(
            environment.transport_identity.expect("retained custody"),
            [0; 32]
        );
        assert_eq!(environment.captures[0].mode, mode);
        let mut changed = plan.clone();
        changed.environments[0].transport_identity = Some([0; 32]);
        assert_ne!(
            plan.identity(),
            lowering_identity(
                changed.target(),
                &changed.owner,
                &changed.environments,
                &changed.aliases,
                &changed.calls,
                &changed.transport_calls,
                &changed.higher_order_calls,
            )
        );
    }
}

#[test]
fn device_borrow_custody_rejects_missing_exported_and_mismatched_edges() {
    let results = compiler_results().custody;
    for error in [results.missing, results.entry, results.target] {
        assert!(error.contains("allocation/completion token"), "{error}");
    }
    assert!(
        results
            .untracked_call
            .contains("no admitted caller environment custody")
    );
    assert!(
        results
            .altered_callee
            .contains("exact callee identity, MIR, or ABI")
    );
    assert!(
        results
            .altered_argument
            .contains("no exact admitted transport or invocation")
    );
    assert!(
        results
            .altered_caller
            .contains("different caller, MIR, or ABI")
    );
    assert!(
        results
            .altered_root
            .contains("exact caller environment root")
    );
}

#[test]
fn large_environments_share_a_bounded_per_function_capture_budget() {
    let results = compiler_results();
    let environment = &results.large_environment.environments()[0];
    assert_eq!(environment.captures.len(), 37);
    assert_eq!(environment.size_bytes, 37 * 8);
    let mut indices = environment
        .captures
        .iter()
        .map(|capture| capture.memory_index)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    assert_eq!(indices, (0..37).collect::<Vec<_>>());
    for error in &results.capture_budget_errors[..2] {
        assert!(
            error.contains("per-function budget of 128 captures"),
            "{error}"
        );
    }
    assert!(results.capture_budget_errors[2].contains("environment budget of 2048 bytes"));
}

#[test]
fn concrete_closure_transport_is_target_bound_and_cannot_be_a_passthrough() {
    let results = compiler_results();
    for (argument_index, plan) in [&results.transported, &results.transported_second]
        .into_iter()
        .enumerate()
    {
        assert_eq!(plan.environments().len(), 1);
        assert_eq!(plan.calls().len(), 0);
        assert_eq!(plan.transport_calls().len(), 1);
        let transport = &plan.transport_calls()[0];
        assert_eq!(transport.argument_index, argument_index);
        assert_eq!(
            transport.closure_custody,
            ClosureCustodyV1::EnvironmentLocal(plan.environments()[0].local)
        );
        assert_ne!(transport.target_function_identity, [0; 32]);
        assert_ne!(transport.target_monomorphization_identity, [0; 32]);
        assert_ne!(transport.target_mir_identity, [0; 32]);
        assert_ne!(transport.target_fn_abi_identity, [0; 32]);
        assert_eq!(plan.target(), "gfx942:xnack-");
    }
    assert_eq!(results.gfx950.target(), "gfx950:xnack-");
    assert_ne!(results.device_fn.identity(), results.gfx950.identity());
    assert!(
        results
            .wrapped_escape_error
            .contains("returns a closure-bearing value"),
        "{}",
        results.wrapped_escape_error
    );
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
    // Pinned MIR forwards the borrowed receiver through an assignment rejected
    // by the bounded alias rules before transport-call authentication.
    assert_eq!(
        results.borrowed_transport_error,
        "production closure profile rejected MIR: closure value escapes through an unsupported assignment in bb0"
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
