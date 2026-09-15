//! Mount under `collector::production_importer_v1`, alongside the bool/Option tests.
//! Host source-collection regressions, not AMDGPU producer or launch qualification.

use crate::collector::{
    CollectError, CollectedFunctionRole, CollectionResult, DeviceCollector, KernelRoot,
};
use crate::production_rustc_drop_v1::{ProductionRustcDropClassV1, classify_rustc_drop_v1};
use crate::trusted_device_items::authenticate_reviewed_safe_core_result_map_helper_v1;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::{
    mir::{Operand, TerminatorKind},
    ty::{ClosureKind, EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypingEnv},
};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
#![allow(unused_unsafe)]
pub struct NonCopy(pub u32);
pub fn callback_leaf(capture: NonCopy, error: u8) -> u64 {
    (capture.0 as u64) ^ (error as u64)
}
pub fn once(x: Result<NonCopy, u8>, capture: NonCopy) -> Result<NonCopy, u64> {
    x.map_err(move |e| callback_leaf(capture, e))
}
// The Fn bound forces a reusable closure before map_err requests FnOnce.
pub fn map_reusable<F: Fn(u8) -> u64>(x: Result<NonCopy, u8>, op: F) -> Result<NonCopy, u64> {
    x.map_err(op)
}
pub fn adapter(x: Result<NonCopy, u8>, seed: u32) -> Result<NonCopy, u64> {
    map_reusable(x, move |e| callback_leaf(NonCopy(seed), e))
}
unsafe fn unsafe_leaf(value: u64) -> u64 { value }
pub fn unsafe_call(x: Result<u32, u8>, seed: u32) -> Result<u32, u64> {
    x.map_err(move |e| unsafe { unsafe_leaf((e as u64) ^ (seed as u64)) })
}
pub fn unsafe_block(x: Result<u32, u8>, seed: u32) -> Result<u32, u64> {
    x.map_err(move |e| unsafe { (e as u64) ^ (seed as u64) })
}
pub fn panic_callback(x: Result<u32, u8>, seed: u32) -> Result<u32, u64> {
    x.map_err(move |e| {
        if seed == 0 { panic!("reachable callback"); }
        e as u64
    })
}
pub struct Dropped(pub u32);
impl Drop for Dropped { fn drop(&mut self) {} }
pub struct Panicking(pub u32);
impl Drop for Panicking { fn drop(&mut self) { panic!("reachable destructor"); } }
pub fn payload_empty(x: Result<u32, Dropped>, seed: u32) -> Result<u32, u64> {
    x.map_err(move |_e| seed as u64)
}
pub fn payload_panic(x: Result<u32, Panicking>, seed: u32) -> Result<u32, u64> {
    x.map_err(move |_e| seed as u64)
}
pub fn capture_empty(x: Result<u32, u8>, capture: Dropped) -> Result<u32, u64> {
    x.map_err(move |e| { let _owned = capture; e as u64 })
}
pub fn capture_panic(x: Result<u32, u8>, capture: Panicking) -> Result<u32, u64> {
    x.map_err(move |e| { let _owned = capture; e as u64 })
}
"#;

fn root<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .expect("fixture function");
    Instance::mono(tcx, definition.to_def_id())
}

fn only_callee<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) -> Instance<'tcx> {
    let mut calls = tcx
        .instance_mir(caller.def)
        .basic_blocks
        .iter()
        .filter_map(|block| {
            let TerminatorKind::Call {
                func: Operand::Constant(callee),
                ..
            } = &block.terminator().kind
            else {
                return None;
            };
            let TyKind::FnDef(definition, args) = callee.const_.ty().kind() else {
                panic!("direct fixture callee");
            };
            let args = tcx.instantiate_and_normalize_erasing_regions(
                caller.args,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(*args),
            );
            Some(
                Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
                    .unwrap()
                    .expect("resolved fixture callee"),
            )
        });
    let callee = calls.next().expect("retained direct call");
    assert!(calls.next().is_none(), "one direct callee in {caller:?}");
    callee
}

fn callback_chain<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Vec<Instance<'tcx>> {
    let root = root(tcx, name);
    assert!(
        !tcx.instance_mir(root.def)
            .basic_blocks
            .iter()
            .any(|block| matches!(block.terminator().kind, TerminatorKind::Drop { .. })),
        "{name}: caller moves the Result and capture, without an incidental caller drop"
    );
    let mut chain = vec![root];
    let mut helper = only_callee(tcx, root);
    if name == "adapter" {
        assert!(helper.def_id().is_local());
        assert_eq!(tcx.item_name(helper.def_id()).as_str(), "map_reusable");
        assert!(!authenticate_reviewed_safe_core_result_map_helper_v1(
            tcx, helper
        ));
        chain.push(helper);
        helper = only_callee(tcx, helper);
    }
    let helper_index = chain.len();
    assert!(authenticate_reviewed_safe_core_result_map_helper_v1(
        tcx, helper
    ));
    let source = tcx.instance_mir(helper.def);
    let selected = crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, helper);
    assert!(!selected.is_source_expansion());
    assert!(std::ptr::eq(source, selected.body()));
    for block in source.basic_blocks.iter() {
        if let TerminatorKind::Call { args, .. } = &block.terminator().kind {
            assert_eq!(args.len(), 2, "original FnOnce receiver and E tuple");
            assert!(args.iter().all(|arg| matches!(arg.node, Operand::Move(_))));
        }
    }
    let target = only_callee(tcx, helper);
    chain.extend([helper, target]);
    if matches!(target.def, InstanceKind::ClosureOnceShim { .. }) {
        chain.push(only_callee(tcx, target));
    }
    let callback = *chain.last().unwrap();
    assert!(callback.def_id().is_local());
    assert_eq!(tcx.def_kind(callback.def_id()), DefKind::Closure);
    for &instance in &chain[helper_index + 1..] {
        assert!(
            !authenticate_reviewed_safe_core_result_map_helper_v1(tcx, instance),
            "wrapper proof cannot authenticate its callback or adapter: {instance:?}"
        );
    }
    chain
}

fn drops<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Vec<ProductionRustcDropClassV1> {
    let body = tcx.instance_mir(instance.def);
    body.basic_blocks
        .iter()
        .filter_map(|block| match block.terminator().kind {
            TerminatorKind::Drop { place, .. } => {
                Some(classify_rustc_drop_v1(tcx, instance, body, place).unwrap())
            }
            _ => None,
        })
        .collect()
}

fn collect<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Result<CollectionResult<'tcx>, CollectError> {
    let mut collector = DeviceCollector::new(tcx, false, Vec::new(), "gfx942".into());
    collector.add_root(KernelRoot {
        target: root(tcx, name),
        logical_name: name.into(),
        export_name: name.into(),
        generated_host_contract_identity: None,
        kernel_binding: None,
        frontend_contract: None,
        kernel_context_contract: None,
        reference_effect_binding: None,
    })?;
    collector.collect()
}

fn assert_chain_in_error<'tcx>(tcx: TyCtxt<'tcx>, chain: &[Instance<'tcx>], error: &str) {
    for &instance in chain {
        assert!(
            error.contains(tcx.symbol_name(instance).name),
            "missing {instance:?} in {error}"
        );
    }
}

fn retained(tcx: TyCtxt<'_>, name: &str, adapter: bool) {
    let mut chain = callback_chain(tcx, name);
    let helper_index = if adapter { 2 } else { 1 };
    assert_eq!(
        matches!(
            chain[helper_index + 1].def,
            InstanceKind::ClosureOnceShim { .. }
        ),
        adapter,
        "exercise both direct FnOnce and the generated adapter"
    );
    assert_eq!(chain.len(), if adapter { 5 } else { 3 });
    let callback = *chain.last().unwrap();
    assert_eq!(
        callback.args.as_closure().kind_ty().to_opt_closure_kind(),
        Some(if adapter {
            ClosureKind::Fn
        } else {
            ClosureKind::FnOnce
        })
    );
    assert_eq!(
        drops(tcx, chain[helper_index]),
        [ProductionRustcDropClassV1::Trivial]
    );
    let leaf = only_callee(tcx, callback);
    assert_eq!(leaf, root(tcx, "callback_leaf"));
    chain.push(leaf);
    let collection = collect(tcx, name).expect("collect callback and recursive leaf");
    assert_eq!(collection.functions.len(), chain.len());
    for (index, instance) in chain.into_iter().enumerate() {
        let mut retained = collection
            .functions
            .iter()
            .filter(|f| f.instance == instance);
        let function = retained
            .next()
            .expect("exact monomorphized callee retained");
        assert!(
            retained.next().is_none(),
            "callee is collected exactly once"
        );
        assert_eq!(
            function.role,
            if index == 0 {
                CollectedFunctionRole::KernelEntry
            } else {
                CollectedFunctionRole::InternalHelper
            }
        );
        assert!(function.dead_branches.is_some());
        if index == helper_index {
            assert!(
                function.closure_plan.is_some(),
                "map_err retains closure custody"
            );
        }
    }
}

fn rejected_callback(tcx: TyCtxt<'_>, name: &str, diagnostic: &str) {
    let chain = callback_chain(tcx, name);
    assert_eq!(drops(tcx, chain[1]), [ProductionRustcDropClassV1::Trivial]);
    let error = collect(tcx, name)
        .expect_err("map_err source authentication must not admit the callback")
        .to_string();
    assert!(error.contains(diagnostic), "{name}: {error}");
    assert_chain_in_error(tcx, &chain, &error);
    if name == "unsafe_call" {
        assert!(error.contains("unsafe_leaf"), "{error}");
    }
}

struct Probe {
    check: for<'tcx> fn(TyCtxt<'tcx>),
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("result_map_collection.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        (self.check)(tcx);
        self.ran = true;
        Compilation::Stop
    }
}

fn run(check: for<'tcx> fn(TyCtxt<'tcx>)) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=result_map_collection".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cdebuginfo=2".into(),
        "-Cpanic=abort".into(),
        "-".into(),
    ];
    let mut probe = Probe { check, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn map_err_collection_retains_once_callback_capture_and_leaf() {
    run(|tcx| retained(tcx, "once", false));
}

#[test]
fn map_err_collection_retains_fn_once_adapter_and_callback() {
    run(|tcx| retained(tcx, "adapter", true));
}

#[test]
fn map_err_collection_rejects_unsafe_callbacks() {
    run(|tcx| {
        rejected_callback(tcx, "unsafe_call", "[FE2O3-CAP-SOURCE002]");
        rejected_callback(tcx, "unsafe_block", "[FE2O3-CAP-SOURCE006]");
    });
}

#[test]
fn map_err_collection_rejects_panicking_callback() {
    run(|tcx| rejected_callback(tcx, "panic_callback", "panic path"));
}

#[test]
fn map_err_collection_rejects_empty_and_panicking_payload_destructors() {
    run(|tcx| {
        for name in ["payload_empty", "payload_panic"] {
            let chain = callback_chain(tcx, name);
            assert_eq!(drops(tcx, chain[1]), [ProductionRustcDropClassV1::Trivial]);
            assert!(
                drops(tcx, *chain.last().unwrap())
                    .contains(&ProductionRustcDropClassV1::RequiresDropGlue)
            );
            let error = collect(tcx, name)
                .expect_err("callback payload destructor must remain a rejected Drop edge")
                .to_string();
            assert!(error.contains("[FE2O3-FFI-EDGE001]"), "{name}: {error}");
            assert!(
                error.contains("Drop requiring drop glue"),
                "{name}: {error}"
            );
            assert_chain_in_error(tcx, &chain, &error);
        }
    });
}

#[test]
fn map_err_collection_rejects_empty_and_panicking_capture_destructors() {
    run(|tcx| {
        for name in ["capture_empty", "capture_panic"] {
            let chain = callback_chain(tcx, name);
            assert_eq!(
                drops(tcx, chain[1]),
                [ProductionRustcDropClassV1::RequiresDropGlue]
            );
            // Capture admission precedes traversal. The helper's original Ok
            // drop is nontrivial even though this earlier guard rejects first.
            let error = collect(tcx, name)
                .expect_err("map_err authentication cannot waive a dropping capture")
                .to_string();
            assert!(error.contains("capture 0 requires drop"), "{name}: {error}");
            assert!(
                error.contains("bounded production closure admission failed"),
                "{name}: {error}"
            );
            assert_chain_in_error(tcx, &chain[..1], &error);
        }
    });
}
