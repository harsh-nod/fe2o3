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

mod rust_call_body;

const FIXTURE_SOURCE: &str = r#"
#![allow(dead_code)]
#![feature(unboxed_closures)]

struct Token(u32);

fn host_unused<F: FnOnce(u32) -> u32>(_: F) {}
fn register_unused(seed: u32) {
    let token = Token(seed);
    host_unused(move |delta| { let moved = token; moved.0 ^ delta });
}
fn receiver_with_inner(seed: u32) -> u32 {
    let token = Token(seed);
    let outer = move || {
        let moved = token;
        let inner = |delta: u32| moved.0 ^ delta;
        inner(7)
    };
    outer()
}

fn device_zero() -> u32 { let closure = || 7; closure() }
fn device_unit() -> u32 { let closure = |(): ()| 7; closure(()) }
fn device_multiple() -> u32 { let closure = |a: u32, b: u32| a.wrapping_sub(b); closure(29, 11) }
fn device_tuple() -> u32 { let closure = |pair: (u32, u32)| pair.0.wrapping_sub(pair.1); closure((29, 11)) }
impl Token {
    extern "rust-call" fn packed(self, args: (u32, u32)) -> u32 {
        self.0 ^ args.0 ^ args.1
    }
}

fn body_fn(seed: u32) -> u32 {
    let closure = move |delta: u32| seed ^ delta;
    closure(7)
}
fn body_fn_mut(mut seed: u32) -> u32 {
    let mut closure = move |delta: u32| { seed ^= delta; seed };
    closure(7)
}
fn body_fn_mut_ref(mut seed: u32) -> u32 {
    let mut closure = |delta: u32| { seed ^= delta; seed };
    closure(7)
}
fn body_fn_once(seed: u32) -> u32 {
    let token = Token(seed);
    let closure = move |delta: u32| { let moved = token; moved.0 ^ delta };
    closure(7)
}
fn body_multiple() -> u32 {
    let closure = |a: u32, b: u32| a ^ b;
    closure(29, 11)
}
fn body_mixed() -> u32 {
    let closure = |value: u32, flag: bool| value ^ flag as u32;
    closure(29, true)
}
fn body_tuple() -> u32 {
    let closure = |pair: (u32, u32)| pair.0 ^ pair.1;
    closure((29, 11))
}

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

struct Wrap<F>(F);
fn consume_wrap<F: Fn() -> u32>(value: Wrap<F>) -> u32 { (value.0)() }
fn nested_struct_capture(seed: u32) -> u32 {
    let wrapped = Wrap(move || seed);
    let outer = move || consume_wrap(wrapped);
    outer()
}

struct Borrowed<'a>(&'a u32);
struct BorrowedMut<'a>(&'a mut u32);
struct Raw(*const u32);
fn host_wrapped_shared(seed: &u32) -> u32 {
    let wrapped = Borrowed(seed);
    host_apply(move |x| { let moved = wrapped; *moved.0 ^ x }, 1)
}
fn host_wrapped_mutable(seed: &mut u32) -> u32 {
    let wrapped = BorrowedMut(seed);
    host_apply(move |x| { let moved = wrapped; *moved.0 ^= x; *moved.0 }, 1)
}
fn host_wrapped_raw(pointer: *const u32) -> u32 {
    let wrapped = Raw(pointer);
    host_apply(move |x| { let moved = wrapped; (unsafe { *moved.0 }) ^ x }, 1)
}
fn device_wrapped_raw(pointer: *const u32) -> u32 {
    let wrapped = Raw(pointer);
    let closure = move |x| { let moved = wrapped; (unsafe { *moved.0 }) ^ x };
    closure(1)
}
"#;

#[derive(Clone, Debug)]
struct DriverResults {
    wrapped_capture_cases: usize,
    receiver_cases: usize,
    rust_call_abi_cases: usize,
    rust_call_body_cases: (usize, usize),
    host: BoundedClosureAdmissionV2,
    device_fn: BoundedClosureAdmissionV2,
    device_fn_mut: BoundedClosureAdmissionV2,
    device_fn_once: BoundedClosureAdmissionV2,
    host_ref_error: String,
    escape_error: String,
    raw_error: String,
    dynamic_error: String,
    return_error: String,
    projected_error: String,
    asm_error: String,
    origin_error: String,
    continuity_errors: Vec<String>,
    nested_capture_error: String,
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

        let host_plan = analyze_bounded_closures_v2(tcx, host, ClosureOriginPolicyV1::HostArgument)
            .expect("host closure registration must be admitted");
        let repeated = analyze_bounded_closures_v2(tcx, host, ClosureOriginPolicyV1::HostArgument)
            .expect("repeat host admission");
        assert_eq!(host_plan, repeated);
        let observed = observe_closures_v2(tcx, host)
            .expect("observe host closure")
            .expect("host closure presence")
            .into_observation();
        assert_eq!(observed, host_plan.observation);
        revalidate_closure_observation_v2(tcx, host, Some(&observed))
            .expect("same compiler observation at import");
        let nonclosure = Instance::mono(tcx, local_function(tcx, "consume"));
        revalidate_closure_observation_v2(tcx, nonclosure, None)
            .expect("no closure at either boundary");
        let mut continuity_errors = vec![
            revalidate_closure_observation_v2(tcx, host, None)
                .expect_err("missing observation")
                .to_string(),
            revalidate_closure_observation_v2(tcx, nonclosure, Some(&observed))
                .expect_err("extra observation")
                .to_string(),
            revalidate_closure_observation_v2(tcx, device_fn_instance, Some(&observed))
                .expect_err("other function cannot reuse an observation")
                .to_string(),
        ];
        for axis in 0..3 {
            let mut substituted = observed.clone();
            match axis {
                0 => substituted.function = SemanticFunctionIdentityV1::from_sha256([0; 32]),
                1 => substituted.mir_body[0] ^= 1,
                2 => substituted.target = SemanticLayoutIdentityV1::from_sha256([0; 32]),
                _ => unreachable!(),
            }
            continuity_errors.push(
                revalidate_closure_observation_v2(tcx, host, Some(&substituted))
                    .expect_err("substituted identity axis")
                    .to_string(),
            );
        }

        self.results = Some(DriverResults {
            wrapped_capture_cases: check_wrapped_captures(tcx),
            receiver_cases: check_own_receivers(tcx),
            rust_call_abi_cases: check_rust_call_signatures(tcx),
            rust_call_body_cases: rust_call_body::check(tcx),
            host: host_plan,
            device_fn: analyze_bounded_closures_v2(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect("device Fn closure"),
            device_fn_mut: analyze_bounded_closures_v2(
                tcx,
                device_fn_mut_instance,
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect("device FnMut closure"),
            device_fn_once: analyze_bounded_closures_v2(
                tcx,
                device_fn_once_instance,
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect("device FnOnce closure"),
            host_ref_error: analyze_bounded_closures_v2(
                tcx,
                host_ref,
                ClosureOriginPolicyV1::HostArgument,
            )
            .expect_err("host reference capture must require allocation authority")
            .to_string(),
            escape_error: analyze_bounded_closures_v2(
                tcx,
                Instance::mono(tcx, local_function(tcx, "escaped")),
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect_err("escaping closure must fail")
            .to_string(),
            raw_error: analyze_bounded_closures_v2(
                tcx,
                Instance::mono(tcx, local_function(tcx, "raw_capture")),
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect_err("raw capture must fail")
            .to_string(),
            dynamic_error: analyze_bounded_closures_v2(
                tcx,
                Instance::mono(tcx, local_function(tcx, "dynamic_dispatch")),
                ClosureOriginPolicyV1::Either,
            )
            .expect_err("dynamic dispatch must fail")
            .to_string(),
            return_error: analyze_bounded_closures_v2(
                tcx,
                Instance::mono(tcx, local_function(tcx, "returned")),
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect_err("returned closure must escape")
            .to_string(),
            projected_error: analyze_bounded_closures_v2(
                tcx,
                Instance::mono(tcx, local_function(tcx, "projected_reference")),
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect_err("projected closure reference destination must fail")
            .to_string(),
            asm_error: analyze_bounded_closures_v2(
                tcx,
                Instance::mono(tcx, local_function(tcx, "inline_asm_escape")),
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect_err("inline assembly closure use must fail")
            .to_string(),
            origin_error: analyze_bounded_closures_v2(
                tcx,
                device_fn_instance,
                ClosureOriginPolicyV1::HostArgument,
            )
            .expect_err("device closure must not satisfy host registration policy")
            .to_string(),
            continuity_errors,
            nested_capture_error: analyze_bounded_closures_v2(
                tcx,
                Instance::mono(tcx, local_function(tcx, "nested_struct_capture")),
                ClosureOriginPolicyV1::DeviceInternal,
            )
            .expect_err("a closure inside an aggregate is still a nested capture")
            .to_string(),
        });
        Compilation::Stop
    }
}

fn check_wrapped_captures(tcx: TyCtxt<'_>) -> usize {
    let cases = [
        ("host_wrapped_shared", true, "allocation/completion token"),
        ("host_wrapped_mutable", true, "allocation/completion token"),
        ("host_wrapped_raw", true, "raw-pointer captures"),
        ("device_wrapped_raw", false, "raw-pointer captures"),
    ];
    for (name, host, diagnostic) in cases {
        let caller = Instance::mono(tcx, local_function(tcx, name));
        let instance = if host {
            resolved_call_named(tcx, caller, "host_apply")
        } else {
            caller
        };
        for error in [
            observe_closures_v2(tcx, instance)
                .expect_err("wrapped capture must reject at collection")
                .to_string(),
            revalidate_closure_observation_v2(tcx, instance, None)
                .expect_err("wrapped capture must reject at import")
                .to_string(),
        ] {
            assert!(error.contains(diagnostic), "{name}: {error}");
        }
    }
    cases.len()
}

#[test]
fn aggregate_wrappers_cannot_hide_borrowed_or_raw_capture_authority() {
    assert_eq!(compiler_results().wrapped_capture_cases, 4);
}

fn local_function(tcx: TyCtxt<'_>, name: &str) -> rustc_hir::def_id::DefId {
    tcx.iter_local_def_id()
        .find(|definition| {
            matches!(
                tcx.def_kind(definition.to_def_id()),
                DefKind::Fn | DefKind::AssocFn
            ) && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing fixture function `{name}`"))
        .to_def_id()
}

fn check_rust_call_signatures(tcx: TyCtxt<'_>) -> usize {
    use crate::rustc_semantic_plan_v1::source_signature_v1;
    use rustc_middle::ty::{EarlyBinder, InstanceKind, List};

    let cases = [
        ("device_fn", 1),
        ("device_fn_mut", 1),
        ("device_fn_once", 1),
        ("device_zero", 0),
        ("device_unit", 1),
        ("device_multiple", 2),
        ("device_tuple", 1),
        ("packed", 2),
    ];
    let env = TypingEnv::fully_monomorphized();
    for (name, arity) in cases {
        let caller = Instance::mono(tcx, local_function(tcx, name));
        let expanded = name != "packed";
        let instance = if expanded {
            tcx.instance_mir(caller.def)
                .local_decls
                .iter()
                .find_map(|local| match local.ty.kind() {
                    TyKind::Closure(def_id, args) => Some(Instance {
                        def: InstanceKind::Item(*def_id),
                        args,
                    }),
                    _ => None,
                })
                .expect("fixture closure environment")
        } else {
            caller
        };
        let signature = source_signature_v1(tcx, instance).expect("actual source signature");
        assert_eq!(signature.abi, rustc_abi::ExternAbi::RustCall);
        let [receiver, tuple] = signature.inputs() else {
            panic!("receiver and outer tuple")
        };
        let TyKind::Tuple(fields) = tuple.kind() else {
            panic!("outer tuple")
        };
        assert_eq!(fields.len(), arity, "{name}");
        if name == "device_tuple" {
            assert!(matches!(fields[0].kind(), TyKind::Tuple(inner) if inner.len() == 2));
        }
        let body = tcx.instance_mir(instance.def);
        assert_eq!(
            body.spread_arg.map(|local| local.index()),
            if expanded { None } else { Some(2) },
            "{name}"
        );
        let expected = if expanded {
            std::iter::once(*receiver)
                .chain(fields.iter())
                .collect::<Vec<_>>()
        } else {
            signature.inputs().to_vec()
        };
        assert_eq!(body.arg_count, expected.len(), "{name}");
        let normalize = |ty| {
            instance
                .try_instantiate_mir_and_normalize_erasing_regions(tcx, env, EarlyBinder::bind(ty))
                .expect("normalize actual body type")
        };
        assert_eq!(normalize(body.return_ty()), signature.output(), "{name}");
        for (local, expected) in body.args_iter().zip(&expected) {
            assert_eq!(normalize(body.local_decls[local].ty), *expected, "{name}");
        }
        let physical = tcx
            .fn_abi_of_instance(env.as_query_input((instance, List::empty())))
            .expect("actual instance FnAbi");
        let adjusted = std::iter::once(*receiver)
            .chain(fields.iter())
            .collect::<Vec<_>>();
        assert_eq!(physical.args.len(), adjusted.len(), "{name}");
        for (argument, expected) in physical.args.iter().zip(adjusted) {
            assert_eq!(argument.layout.ty, expected, "{name}");
        }
        assert_eq!(physical.ret.layout.ty, signature.output(), "{name}");
    }
    cases.len()
}

fn check_own_receivers(tcx: TyCtxt<'_>) -> usize {
    let closure = |name| {
        let caller = Instance::mono(tcx, local_function(tcx, name));
        tcx.instance_mir(caller.def)
            .local_decls
            .iter()
            .find_map(|local| match local.ty.kind() {
                TyKind::Closure(definition, args) => Some(Instance {
                    def: InstanceKind::Item(*definition),
                    args,
                }),
                _ => None,
            })
            .expect("fixture closure body")
    };
    let instance = closure("body_fn_once");
    let body = tcx.instance_mir(instance.def);
    assert!(observe_closures_v2(tcx, instance).unwrap().is_none());
    revalidate_closure_observation_v2(tcx, instance, None).unwrap();
    let caller = Instance::mono(tcx, local_function(tcx, "body_fn_once"));
    let admission = observe_closures_v2(tcx, caller).unwrap().unwrap();
    assert_eq!(admission.environments().len(), 1);
    assert_eq!(admission.calls().len(), 1);
    assert_eq!(
        admission.environments()[0].call_kind,
        ClosureCallKindV1::FnOnce
    );
    let other = closure("device_fn_once");
    let mut substituted = body.clone();
    substituted.local_decls[Local::from_usize(1)].ty =
        tcx.instance_mir(other.def).local_decls[Local::from_usize(1)].ty;
    assert!(
        own_closure_receiver_v1(tcx, instance, &substituted)
            .unwrap_err()
            .to_string()
            .contains("receiver identity changed")
    );
    let inner = observe_closures_v2(tcx, closure("receiver_with_inner"))
        .unwrap()
        .unwrap();
    assert_eq!(inner.environments().len(), 1);
    assert_eq!(inner.calls().len(), 1);
    let unused = resolved_call_named(
        tcx,
        Instance::mono(tcx, local_function(tcx, "register_unused")),
        "host_unused",
    );
    let error = observe_closures_v2(tcx, unused).unwrap_err().to_string();
    assert!(error.contains("0"), "{error}");
    4
}

#[test]
fn closure_body_receiver_is_not_a_new_host_callable() {
    assert_eq!(compiler_results().receiver_cases, 4);
}

#[test]
fn rust_call_source_signatures_match_real_body_locals_and_instance_abis() {
    assert_eq!(compiler_results().rust_call_abi_cases, 8);
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
            let mut command = Command::new("rustc");
            command.args(["--print", "sysroot"]);
            let sysroot = crate::process_execution::capture_output(&mut command)
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
                "-Cpanic=abort".to_owned(),
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
    assert_ne!(plan.observation.function.as_bytes(), &[0; 32]);
    assert_ne!(plan.observation.mir_body, [0; 32]);
    assert_ne!(plan.observation.target.as_bytes(), &[0; 32]);
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
            .nested_capture_error
            .contains("nested closure captures"),
        "{}",
        results.nested_capture_error
    );
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
}

#[test]
fn collection_import_continuity_binds_presence_function_body_and_live_target() {
    let errors = compiler_results().continuity_errors;
    assert_eq!(errors.len(), 6);
    for error in errors {
        assert!(
            error.contains("collection/import closure presence"),
            "{error}"
        );
    }
}
