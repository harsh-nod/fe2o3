use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{Local, StatementKind, TerminatorKind};
use rustc_session::config::Input;
use rustc_span::FileName;

#[path = "cleanup_tests.rs"]
mod cleanup_tests;
#[path = "header_tests.rs"]
mod header_tests;
#[path = "mutations.rs"]
mod mutations;

const SOURCE: &str = r#"
#![no_std]
#![allow(dead_code, unused_unsafe)]
pub fn scalar(x: Result<u32, u8>) -> Result<u32, u64> { x.map_err(|e| e as u64) }
pub fn unit(x: Result<(), ()>) -> Result<(), ()> { x.map_err(|e| e) }
pub struct Owned(pub u32);
impl Drop for Owned { fn drop(&mut self) {} }
pub struct Moved(pub u32);
pub fn owned_error(x: Result<u32, u8>) -> Result<u32, Owned> {
    x.map_err(|e| Owned(e as u32))
}
pub fn noncopy(x: Result<Moved, Moved>) -> Result<Moved, Moved> { x.map_err(|e| e) }
pub fn owned(x: Result<Owned, Owned>, c: Owned) -> Result<Owned, Owned> {
    x.map_err(move |e| { core::mem::drop(c); e })
}
pub fn mutable(x: Result<u32, u32>, c: &mut u32) -> Result<u32, u32> {
    x.map_err(move |e| { *c = e; e })
}
pub fn borrowed<'a>(x: Result<&'a mut u32, &'a mut u32>) -> Result<&'a mut u32, &'a mut u32> {
    x.map_err(|e| e)
}
pub fn aggregate(x: Result<(Owned, Owned), [Owned; 2]>) -> Result<(Owned, Owned), [Owned; 2]> {
    x.map_err(|e| e)
}
pub fn convert(e: u8) -> u64 { e as u64 }
pub fn function_item(x: Result<u32, u8>) -> Result<u32, u64> { x.map_err(convert) }
pub fn function_pointer(x: Result<u32, u8>, f: fn(u8) -> u64) -> Result<u32, u64> { x.map_err(f) }
pub fn panic_callback(x: Result<u32, u8>) -> Result<u32, u64> {
    x.map_err(|_| panic!("unreviewed callback"))
}
pub fn unsafe_callback(x: Result<u32, u8>, p: *const u64) -> Result<u32, u64> {
    x.map_err(move |_| unsafe { *p })
}
pub struct UnsafeDrop(pub *mut u32);
impl Drop for UnsafeDrop { fn drop(&mut self) { unsafe { *self.0 = 1; } } }
pub fn unsafe_capture_drop(x: Result<u32, u8>, c: UnsafeDrop) -> Result<u32, u64> {
    x.map_err(move |e| { core::mem::drop(c); e as u64 })
}
pub fn wrong_method(x: Result<u32, u8>) -> Result<u64, u8> { x.map(|e| e as u64) }
pub struct Impostor;
impl Impostor { pub fn map_err(self, _: fn(u8) -> u64) -> Result<u32, u64> { Ok(0) } }
pub fn impostor() -> Result<u32, u64> { Impostor.map_err(convert) }
pub fn panic_route() { panic!("unreviewed call"); }
pub async fn coroutine_header() {}
// Same algorithm, but never nominal core authority. Low-opt MIR also exercises
// storage markers and compiler drop flags in the body-mutation tests.
pub fn model<T, E, F, O: FnOnce(E) -> F>(x: Result<T, E>, op: O) -> Result<T, F> {
    match x { Ok(t) => Ok(t), Err(e) => Err(op(e)) }
}
pub fn foreign(x: Result<u32, u8>) -> Result<u32, u64> { model(x, convert) }
pub fn cleanup_model<T, E, F, O: FnOnce(E) -> F>(x: Result<T, E>, op: O) -> Result<T, F> {
    let output;
    {
        let op = op;
        output = match x { Ok(t) => Ok(t), Err(e) => Err(op(e)) };
    }
    output
}
"#;

fn local_body<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> &'tcx Body<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .unwrap();
    tcx.optimized_mir(definition)
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
            let TyKind::FnDef(definition, arguments) = *callee.const_.ty().kind() else {
                return None;
            };
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), definition, arguments)
                .ok()?
        })
        .unwrap_or_else(|| panic!("retained helper call in {name}"))
}

fn instances(tcx: TyCtxt<'_>) {
    for name in [
        "scalar",
        "unit",
        "noncopy",
        "owned_error",
        "owned",
        "mutable",
        "borrowed",
        "aggregate",
        "function_item",
        "function_pointer",
        "panic_callback",
        "unsafe_callback",
        "unsafe_capture_drop",
    ] {
        let instance = helper(tcx, name);
        let contract = contract(tcx, instance).unwrap_or_else(|| {
            diagnose(tcx, instance, name);
            panic!("exact contract: {name}")
        });
        let source = tcx.instance_mir(instance.def);
        let before = format!("{source:?}");
        // Installed host core has no output-drop cleanup for a panicking O::drop.
        // Do not invent it or waive the leak for dropping T on an unwind target.
        let expected =
            !tcx.sess.panic_strategy().unwinds() || !["owned", "aggregate"].contains(&name);
        for _ in 0..2 {
            let actual = authenticate_reviewed_safe_core_result_map_helper_v1(tcx, instance);
            if actual != expected {
                diagnose(tcx, instance, name);
            }
            assert_eq!(actual, expected, "{name}");
        }
        assert_eq!(before, format!("{source:?}"));
        assert!(std::ptr::eq(source, tcx.instance_mir(instance.def)));
        assert!(
            !authenticate_reviewed_safe_core_result_map_helper_v1(tcx, contract.callee),
            "wrapper does not authenticate the callback or FnOnce shim: {name}"
        );
        assert!(
            !authenticate_reviewed_safe_core_result_map_helper_v1(
                tcx,
                Instance::resolve_drop_in_place(tcx, contract.concrete[3]),
            ),
            "capture drop glue requires its own recursive admission: {name}"
        );
        let calls = source
            .basic_blocks
            .iter()
            .filter_map(|block| match &block.terminator().kind {
                TerminatorKind::Call { func, args, .. } => Some((func, args)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert!(exact_callback(tcx, instance, calls[0].0, &contract));
        assert_eq!(
            calls[0].1.len(),
            2,
            "callback plus original source tuple, never flattened ABI fields"
        );
        assert!(
            source
                .basic_blocks
                .iter()
                .any(|block| matches!(block.terminator().kind, TerminatorKind::Drop { .. }))
        );
    }
    for name in ["wrong_method", "impostor", "foreign", "panic_route"] {
        assert!(
            !authenticate_reviewed_safe_core_result_map_helper_v1(tcx, helper(tcx, name)),
            "{name}"
        );
    }
    let instance = helper(tcx, "scalar");
    for args in [
        vec![],
        vec![tcx.types.u32.into(); 3],
        vec![tcx.types.u32.into(); 5],
        vec![tcx.lifetimes.re_erased.into(); 4],
        vec![tcx.types.u32.into(); 4],
    ] {
        assert!(!authenticate_reviewed_safe_core_result_map_helper_v1(
            tcx,
            Instance {
                args: tcx.mk_args(&args),
                ..instance
            }
        ));
    }
    assert!(!authenticate_reviewed_safe_core_result_map_helper_v1(
        tcx,
        Instance {
            def: InstanceKind::Intrinsic(instance.def_id()),
            ..instance
        }
    ));
    let c = contract(tcx, instance).unwrap();
    let mut changed = instance.args.iter().collect::<Vec<_>>();
    changed[0] = tcx.types.u64.into();
    let changed = Instance {
        args: tcx.mk_args(&changed),
        ..instance
    };
    assert!(authenticate_reviewed_safe_core_result_map_helper_v1(
        tcx, changed
    ));
    assert!(
        !reviewed_body(tcx, changed, tcx.instance_mir(instance.def), &c),
        "changing T cannot reuse a contract even when the FnOnce instance is unchanged"
    );
    let sig = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id()).instantiate_identity(),
    );
    for changed in [
        FnSig {
            safety: Safety::Unsafe,
            ..sig
        },
        FnSig {
            abi: ExternAbi::RustCall,
            ..sig
        },
        FnSig {
            c_variadic: true,
            ..sig
        },
        FnSig {
            inputs_and_output: tcx.mk_type_list(&[c.input, c.parameters[3], c.input]),
            ..sig
        },
    ] {
        assert!(!signature_matches(
            changed,
            c.input,
            c.parameters[3],
            c.output,
            ExternAbi::Rust
        ));
    }
    for index in [0, 1, 2, 3] {
        let mut args = instance.args.iter().collect::<Vec<_>>();
        args[index] = c.parameters[index].into();
        assert!(!authenticate_reviewed_safe_core_result_map_helper_v1(
            tcx,
            Instance {
                args: tcx.mk_args(&args),
                ..instance
            }
        ));
    }
}

fn diagnose<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, name: &str) {
    let body = tcx.instance_mir(instance.def);
    eprintln!(
        "map_err {name}: {instance:?} target={} panic={:?} mir_opt={:?} source={:?} phase={:?} args={} locals={} blocks={} scopes={} debug={}",
        tcx.sess.target.llvm_target,
        tcx.sess.panic_strategy(),
        tcx.sess.opts.unstable_opts.mir_opt_level,
        body.source,
        body.phase,
        body.arg_count,
        body.local_decls.len(),
        body.basic_blocks.len(),
        body.source_scopes.len(),
        body.var_debug_info.len(),
    );
    for (local, declaration) in body.local_decls.iter_enumerated().take(MAX_LOCALS) {
        eprintln!("  {local:?}: {:?}", declaration.ty);
    }
    let mut remaining = MAX_STATEMENTS;
    for (block, data) in body.basic_blocks.iter_enumerated().take(MAX_BLOCKS) {
        eprintln!("  {block:?}: cleanup={}", data.is_cleanup);
        for statement in data.statements.iter().take(remaining) {
            eprintln!("    {statement:?}");
        }
        remaining = remaining.saturating_sub(data.statements.len());
        eprintln!("    {:?}", data.terminator);
    }
}

#[derive(Clone, Copy)]
enum Suite {
    Instances,
    Mutations,
    Headers,
    Cleanup,
    All,
}

struct Probe {
    suite: Suite,
    amdgpu: bool,
    low_opt: bool,
    ran: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("core_result_map.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            tcx.sess.target.llvm_target.as_ref() == "amdgcn-amd-amdhsa",
            self.amdgpu
        );
        match self.suite {
            Suite::Instances => instances(tcx),
            Suite::Mutations => mutations::check(tcx, self.low_opt),
            Suite::Headers => header_tests::check(tcx),
            Suite::Cleanup => cleanup_tests::check(tcx),
            Suite::All => {
                instances(tcx);
                mutations::check(tcx, self.low_opt);
                header_tests::check(tcx);
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(suite: Suite, low_opt: bool, amdgpu: bool, abort: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=core_result_map".into(),
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
            let path = std::path::PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name} to existing pinned AMDGPU metadata; no host substitution or sysroot build")));
            assert!(path.is_file(), "{name}: {}", path.display());
            path
        };
        let core = metadata("FE2O3_WRAPPING_AMDGPU_CORE");
        let builtins = metadata("FE2O3_WRAPPING_AMDGPU_BUILTINS");
        let cpu = std::env::var("FE2O3_WRAPPING_TARGET_CPU").unwrap_or_else(|_| "gfx942".into());
        assert!(
            matches!(cpu.as_str(), "gfx942" | "gfx950"),
            "unsupported AMDGPU test profile: {cpu}"
        );
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
        suite,
        amdgpu,
        low_opt,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn map_err_actual_host_identity_and_recursive_callee() {
    run(Suite::Instances, true, false, true);
}
#[test]
fn map_err_actual_host_default_mir() {
    run(Suite::All, false, false, true);
}
#[test]
fn map_err_host_unwind_does_not_waive_missing_cleanup() {
    run(Suite::Instances, true, false, false);
}
#[test]
fn map_err_unwind_preserves_output_drop_and_double_panic_edges() {
    run(Suite::Cleanup, true, false, false);
}
#[test]
fn map_err_body_ownership_callee_and_cfg_mutations() {
    run(Suite::Mutations, true, false, true);
}
#[test]
fn map_err_source_headers_and_budget_boundaries() {
    run(Suite::Headers, true, false, true);
}

#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn map_err_actual_amdgpu_metadata_default_mir() {
    run(Suite::All, false, true, true);
}

#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn map_err_actual_amdgpu_metadata_mir_opt_zero() {
    run(Suite::All, true, true, true);
}
