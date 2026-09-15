//! Mount under `collector::production_importer_v1`, like the primitive-cast fixture.
//! Actual host MIR only: these tests make no AMDGPU source or launch claim.

use crate::collector::{CollectError, CollectionResult, DeviceCollector, KernelRoot};
use crate::production_rustc_drop_v1::{ProductionRustcDropClassV1, classify_rustc_drop_v1};
use crate::trusted_device_items::authenticate_reviewed_safe_core_bool_helper_v1;
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
pub struct NonCopy(pub u32);
pub struct Dropped;
impl Drop for Dropped { fn drop(&mut self) {} }
pub struct Panicking;
impl Drop for Panicking { fn drop(&mut self) { panic!("reachable destructor") } }
pub fn trivial(flag: bool, value: NonCopy) -> Option<NonCopy> { flag.then_some(value) }
pub fn empty_destructor(flag: bool, value: Dropped) -> Option<Dropped> { flag.then_some(value) }
pub fn panicking_destructor(flag: bool, value: Panicking) -> Option<Panicking> { flag.then_some(value) }
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

fn helper<'tcx>(tcx: TyCtxt<'tcx>, root: Instance<'tcx>) -> Instance<'tcx> {
    let body = tcx.instance_mir(root.def);
    // Returning the Option moves it out: rejection must originate in then_some,
    // not an incidental drop of its result or argument in the fixture caller.
    assert!(
        !body
            .basic_blocks
            .iter()
            .any(|block| matches!(block.terminator().kind, TerminatorKind::Drop { .. }))
    );
    let mut calls = body.basic_blocks.iter().filter_map(|block| {
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
        Some(
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
                .unwrap()
                .expect("resolved then_some"),
        )
    });
    let helper = calls.next().expect("then_some call");
    assert!(calls.next().is_none());
    assert!(authenticate_reviewed_safe_core_bool_helper_v1(tcx, helper));
    helper
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

struct Probe {
    nontrivial: bool,
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("bool_then_some_collection.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let names: &[&str] = if self.nontrivial {
            &["empty_destructor", "panicking_destructor"]
        } else {
            &["trivial"]
        };
        for &name in names {
            let root = root(tcx, name);
            let helper = helper(tcx, root);
            let body = tcx.instance_mir(helper.def);
            let drops = body
                .basic_blocks
                .iter()
                .filter_map(|block| {
                    let TerminatorKind::Drop { place, .. } = block.terminator().kind else {
                        return None;
                    };
                    Some(classify_rustc_drop_v1(tcx, helper, body, place).unwrap())
                })
                .collect::<Vec<_>>();
            let expected = if self.nontrivial {
                ProductionRustcDropClassV1::RequiresDropGlue
            } else {
                ProductionRustcDropClassV1::Trivial
            };
            assert_eq!(
                drops,
                [expected],
                "{name}: retain the actual helper Drop edge"
            );
            if self.nontrivial {
                let error = collect(tcx, name)
                    .expect_err("then_some authentication must not admit a payload destructor")
                    .to_string();
                assert!(error.contains("[FE2O3-FFI-EDGE001]"), "{name}: {error}");
                assert!(
                    error.contains("Drop requiring drop glue"),
                    "{name}: {error}"
                );
                for instance in [root, helper] {
                    assert!(
                        error.contains(tcx.symbol_name(instance).name),
                        "{name}: missing {instance:?} in {error}"
                    );
                }
            } else {
                let collection = collect(tcx, name)
                    .expect("non-Copy payload without a destructor is collectible");
                assert_eq!(collection.functions.len(), 2);
                for instance in [root, helper] {
                    assert_eq!(
                        collection
                            .functions
                            .iter()
                            .filter(|f| f.instance == instance)
                            .count(),
                        1,
                        "retain caller and original then_some instance"
                    );
                }
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(nontrivial: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=bool_then_some_collection".into(),
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
    let mut probe = Probe {
        nontrivial,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn then_some_collection_retains_trivial_noncopy_payload_drop() {
    run(false);
}

#[test]
fn then_some_collection_rejects_empty_and_panicking_payload_destructors() {
    run(true);
}
