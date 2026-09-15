//! Mount under `collector::production_importer_v1`, like the primitive-cast fixture.
//! These host source-collection regressions do not qualify AMDGPU compilation or launch.

use crate::collector::{CollectError, CollectionResult, DeviceCollector, KernelRoot};
use crate::trusted_device_items::authenticate_reviewed_safe_core_option_compare_helper_v1;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::{
    mir::{Operand, TerminatorKind},
    ty::{EarlyBinder, Instance, TyCtxt, TyKind, TypingEnv},
};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
pub struct Payload(pub u32);
pub fn payload_eq(left: u32, right: u32) -> bool { left == right }
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool { payload_eq(self.0, other.0) }
    fn ne(&self, _: &Self) -> bool { panic!("payload ne must not be called") }
}
impl Drop for Payload { fn drop(&mut self) { panic!("borrowed payload must not be dropped") } }
pub struct UnsafeEq(pub u32);
unsafe fn unsafe_eq(left: u32, right: u32) -> bool { left == right }
impl PartialEq for UnsafeEq {
    fn eq(&self, other: &Self) -> bool { unsafe { unsafe_eq(self.0, other.0) } }
}
pub struct PanickingEq;
impl PartialEq for PanickingEq {
    fn eq(&self, _: &Self) -> bool { panic!("reachable payload eq") }
}
macro_rules! routes {
    ($eq:ident, $ne:ident, $payload:ty) => {
        pub fn $eq(left: &Option<Option<$payload>>, right: &Option<Option<$payload>>) -> bool {
            Option::<Option<$payload>>::eq(left, right)
        }
        pub fn $ne(left: &Option<Option<$payload>>, right: &Option<Option<$payload>>) -> bool {
            Option::<Option<$payload>>::ne(left, right)
        }
    };
}
routes!(nested_eq, nested_ne, Payload);
routes!(unsafe_nested_eq, unsafe_nested_ne, UnsafeEq);
routes!(panic_nested_eq, panic_nested_ne, PanickingEq);
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

fn comparison_chain<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Vec<Instance<'tcx>> {
    let mut chain = vec![root(tcx, name)];
    let helper_count = if name.ends_with("_ne") { 3 } else { 2 };
    for _ in 0..helper_count {
        let helper = only_callee(tcx, *chain.last().unwrap());
        assert!(authenticate_reviewed_safe_core_option_compare_helper_v1(
            tcx, helper
        ));
        chain.push(helper);
    }
    let payload = only_callee(tcx, *chain.last().unwrap());
    assert!(payload.def_id().is_local());
    assert!(!authenticate_reviewed_safe_core_option_compare_helper_v1(
        tcx, payload
    ));
    chain.push(payload);
    chain
}

fn collect<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Result<CollectionResult<'tcx>, CollectError> {
    let mut collector = DeviceCollector::new(tcx, false, Vec::new(), "gfx942".into());
    collector
        .add_root(KernelRoot {
            target: root(tcx, name),
            logical_name: name.into(),
            export_name: name.into(),
            generated_host_contract_identity: None,
            kernel_binding: None,
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
        })
        .unwrap();
    collector.collect()
}

#[derive(Clone, Copy)]
enum Check {
    Retained,
    Unsafe,
    Panic,
}

struct Probe {
    check: Check,
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("option_compare_collection.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let prefix = match self.check {
            Check::Retained => "nested",
            Check::Unsafe => "unsafe_nested",
            Check::Panic => "panic_nested",
        };
        for suffix in ["eq", "ne"] {
            let name = format!("{prefix}_{suffix}");
            let mut chain = comparison_chain(tcx, &name);
            match self.check {
                Check::Retained => {
                    let leaf = only_callee(tcx, *chain.last().unwrap());
                    assert_eq!(leaf, root(tcx, "payload_eq"));
                    chain.push(leaf);
                    let collection = collect(tcx, &name).expect("collect nested Option equality");
                    assert_eq!(
                        collection.functions.len(),
                        chain.len(),
                        "{name}: retain only the reachable equality graph"
                    );
                    for instance in chain {
                        assert_eq!(
                            collection
                                .functions
                                .iter()
                                .filter(|f| f.instance == instance)
                                .count(),
                            1,
                            "{name}: retain exact monomorphized callee {instance:?}"
                        );
                    }
                }
                Check::Unsafe | Check::Panic => {
                    let error = collect(tcx, &name)
                        .expect_err("Option authentication must not admit the payload Eq")
                        .to_string();
                    let diagnostic = match self.check {
                        Check::Unsafe => "[FE2O3-CAP-SOURCE002]",
                        Check::Panic => "panic path",
                        Check::Retained => unreachable!(),
                    };
                    assert!(error.contains(diagnostic), "{name}: {error}");
                    for instance in chain {
                        let symbol = tcx.symbol_name(instance).name;
                        assert!(
                            error.contains(symbol),
                            "{name}: missing {instance:?} in {error}"
                        );
                    }
                    if matches!(self.check, Check::Unsafe) {
                        assert!(error.contains("unsafe_eq"), "{name}: {error}");
                    }
                }
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(check: Check) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=option_compare_collection".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-".into(),
    ];
    let mut probe = Probe { check, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn nested_option_eq_and_ne_collection_retains_recursive_payload_callees() {
    run(Check::Retained);
}

#[test]
fn nested_option_eq_and_ne_collection_rejects_unsafe_payload_eq() {
    run(Check::Unsafe);
}

#[test]
fn nested_option_eq_and_ne_collection_rejects_panicking_payload_eq() {
    run(Check::Panic);
}
