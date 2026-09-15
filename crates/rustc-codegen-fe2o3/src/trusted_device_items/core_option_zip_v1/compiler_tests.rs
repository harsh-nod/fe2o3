//! Source-collection tests. Original zip MIR is retained, not a terminal.
//! Collector guarantees use cached panic-abort AMDGPU core. Installed host
//! core retains cleanup MIR even when the caller session uses panic=abort.

use crate::collector::{
    CollectError, CollectedFunctionRole, CollectionResult, DeviceCollector, KernelRoot,
};
use crate::production_rustc_drop_v1::{ProductionRustcDropClassV1, classify_rustc_drop_v1};
use crate::trusted_device_items::authenticate_reviewed_safe_core_option_zip_helper_v1;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::{
    mir::{Operand, TerminatorKind},
    ty::{Instance, TyCtxt, TyKind, TypingEnv},
};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
#![allow(unused_unsafe)]
pub struct NonCopy(pub u32);
pub struct EmptyDrop(pub u32);
impl Drop for EmptyDrop { fn drop(&mut self) {} }
pub struct PanicDrop;
impl Drop for PanicDrop { fn drop(&mut self) { panic!("reachable payload destructor"); } }
pub struct UnsafeDrop(pub *mut u32);
impl Drop for UnsafeDrop { fn drop(&mut self) { unsafe { *self.0 = 1; } } }
pub fn scalar(a: Option<usize>, b: Option<usize>) -> Option<(usize, usize)> { a.zip(b) }
pub fn different(a: Option<u32>, b: Option<f32>) -> Option<(u32, f32)> { a.zip(b) }
pub fn noncopy(a: Option<NonCopy>, b: Option<NonCopy>) -> Option<(NonCopy, NonCopy)> { a.zip(b) }
pub fn borrowed<'a>(a: Option<&'a mut u32>, b: Option<&'a mut u32>) -> Option<(&'a mut u32, &'a mut u32)> { a.zip(b) }
pub fn unsafe_caller(a: Option<usize>, b: Option<usize>) -> Option<(usize, usize)> {
    let result = a.zip(b);
    let _ = unsafe { a };
    result
}
pub fn panic_caller(a: Option<usize>, b: Option<usize>, fail: bool) -> Option<(usize, usize)> {
    let result = a.zip(b);
    if fail { panic!("reachable caller panic"); }
    result
}
macro_rules! drop_routes {
    ($left:ident, $right:ident, $ty:ty) => {
        pub fn $left(a: Option<$ty>, b: Option<usize>) -> Option<($ty, usize)> { a.zip(b) }
        pub fn $right(a: Option<usize>, b: Option<$ty>) -> Option<(usize, $ty)> { a.zip(b) }
    }
}
drop_routes!(left_empty, right_empty, EmptyDrop);
drop_routes!(left_panic, right_panic, PanicDrop);
drop_routes!(left_unsafe, right_unsafe, UnsafeDrop);
"#;

fn root<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .expect("fixture root");
    Instance::mono(tcx, definition.to_def_id())
}

fn zip<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) -> Instance<'tcx> {
    let mut instances = tcx
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
            let TyKind::FnDef(def, args) = *callee.const_.ty().kind() else {
                return None;
            };
            let instance =
                Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def, args).ok()??;
            authenticate_reviewed_safe_core_option_zip_helper_v1(tcx, instance).then_some(instance)
        });
    let helper = instances
        .next()
        .expect("actual authenticated core Option::zip call");
    assert!(instances.next().is_none());
    let source = tcx.instance_mir(helper.def);
    let selected = crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, helper);
    assert!(!selected.is_source_expansion());
    assert!(std::ptr::eq(source, selected.body()));
    assert!(crate::production_semantic_terminal_v1::classify(tcx, helper.def_id()).is_none());
    assert!(
        source
            .basic_blocks
            .iter()
            .all(|b| !matches!(b.terminator().kind, TerminatorKind::Call { .. }))
    );
    assert!(
        source
            .basic_blocks
            .iter()
            .filter(|b| matches!(b.terminator().kind, TerminatorKind::Drop { .. }))
            .count()
            >= 2
    );
    helper
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

fn retained(tcx: TyCtxt<'_>) {
    for name in ["scalar", "different", "noncopy", "borrowed"] {
        let caller = root(tcx, name);
        assert!(!authenticate_reviewed_safe_core_option_zip_helper_v1(
            tcx, caller
        ));
        let helper = zip(tcx, caller);
        let body = tcx.instance_mir(helper.def);
        let source_hash = crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, helper);
        let collection = collect(tcx, name)
            .expect("closed zip body admitted through production source-safety hook");
        assert_eq!(
            collection.functions.len(),
            2,
            "caller and original core helper: {name}"
        );
        for (instance, role) in [
            (caller, CollectedFunctionRole::KernelEntry),
            (helper, CollectedFunctionRole::InternalHelper),
        ] {
            let mut functions = collection
                .functions
                .iter()
                .filter(|f| f.instance == instance);
            let function = functions.next().expect("exact original instance retained");
            assert!(functions.next().is_none());
            assert_eq!(function.role, role);
            assert!(function.dead_branches.is_some());
        }
        for block in body.basic_blocks.iter() {
            if let TerminatorKind::Drop { place, .. } = block.terminator().kind {
                assert_eq!(
                    classify_rustc_drop_v1(tcx, helper, body, place).unwrap(),
                    ProductionRustcDropClassV1::Trivial
                );
            }
        }
        assert_eq!(
            source_hash,
            crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, helper)
        );
        assert!(std::ptr::eq(
            body,
            crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, helper).body()
        ));
    }
}

fn rejected_callers(tcx: TyCtxt<'_>) {
    for (name, diagnostic) in [
        ("unsafe_caller", "[FE2O3-CAP-SOURCE006]"),
        ("panic_caller", "panic path"),
    ] {
        zip(tcx, root(tcx, name));
        let error = collect(tcx, name)
            .expect_err("zip proof must not authenticate surrounding unsafe or panic source")
            .to_string();
        assert!(error.contains(diagnostic), "{name}: {error}");
        assert!(error.contains(name), "{error}");
    }
}

fn rejected_drops(tcx: TyCtxt<'_>) {
    for name in [
        "left_empty",
        "right_empty",
        "left_panic",
        "right_panic",
        "left_unsafe",
        "right_unsafe",
    ] {
        let caller = root(tcx, name);
        assert!(
            tcx.instance_mir(caller.def)
                .basic_blocks
                .iter()
                .all(|b| !matches!(b.terminator().kind, TerminatorKind::Drop { .. })),
            "no incidental caller drop: {name}"
        );
        let helper = zip(tcx, caller);
        let body = tcx.instance_mir(helper.def);
        assert!(body.basic_blocks.iter().any(|b| match b.terminator().kind {
            TerminatorKind::Drop { place, .. } =>
                classify_rustc_drop_v1(tcx, helper, body, place).unwrap()
                    == ProductionRustcDropClassV1::RequiresDropGlue,
            _ => false,
        }));
        let error = collect(tcx, name)
            .expect_err("nontrivial payload destructors remain explicit rejected Drop edges")
            .to_string();
        assert!(error.contains("[FE2O3-FFI-EDGE001]"), "{name}: {error}");
        assert!(
            error.contains("Drop requiring drop glue"),
            "{name}: {error}"
        );
        assert!(
            error.contains(tcx.symbol_name(helper).name),
            "{name}: {error}"
        );
    }
}

fn rejected_host_cleanup(tcx: TyCtxt<'_>) {
    let name = "scalar";
    let helper = zip(tcx, root(tcx, name));
    let body = tcx.instance_mir(helper.def);
    assert!(
        body.basic_blocks.iter().any(|block| {
            block.is_cleanup && matches!(block.terminator().kind, TerminatorKind::UnwindResume)
        }),
        "installed host core retains its original cleanup body"
    );
    let before = crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, helper);
    let error = collect(tcx, name)
        .expect_err("panic-abort caller does not authorize deleting prebuilt core cleanup")
        .to_string();
    assert!(
        error.contains("[FE2O3-FFI-EDGE001] unsupported executable MIR edge `resume`"),
        "{name}: {error}"
    );
    assert!(
        error.contains(tcx.symbol_name(helper).name),
        "{name}: {error}"
    );
    assert_eq!(
        before,
        crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, helper)
    );
}

struct Probe {
    check: for<'tcx> fn(TyCtxt<'tcx>),
    amdgpu: bool,
    ran: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("option_zip_collection.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            tcx.sess.target.llvm_target.as_ref() == "amdgcn-amd-amdhsa",
            self.amdgpu
        );
        assert!(
            !tcx.sess.panic_strategy().unwinds(),
            "production collector fixture must use panic=abort"
        );
        (self.check)(tcx);
        self.ran = true;
        Compilation::Stop
    }
}

fn run(check: for<'tcx> fn(TyCtxt<'tcx>), amdgpu: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=option_zip_collection".into(),
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
    ];
    if amdgpu {
        let metadata = |name| {
            let p = std::path::PathBuf::from(
                std::env::var_os(name)
                    .unwrap_or_else(|| panic!("set {name} to existing pinned AMDGPU metadata")),
            );
            assert!(p.is_file(), "{name}: {}", p.display());
            p
        };
        let core = metadata("FE2O3_WRAPPING_AMDGPU_CORE");
        let builtins = metadata("FE2O3_WRAPPING_AMDGPU_BUILTINS");
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
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
        check,
        amdgpu,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn option_zip_host_collection_rejects_retained_cleanup_resume() {
    run(rejected_host_cleanup, false);
}
#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn option_zip_actual_amdgpu_collection_rejects_unsafe_and_panicking_callers() {
    run(rejected_callers, true);
}
#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn option_zip_actual_amdgpu_collection_rejects_both_payload_destructors() {
    run(rejected_drops, true);
}
#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn option_zip_actual_amdgpu_collection_retains_original_core_body() {
    run(retained, true);
}
