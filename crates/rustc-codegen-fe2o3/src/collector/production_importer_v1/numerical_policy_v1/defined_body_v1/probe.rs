//! Standalone rustc observation of original cached provider MIR, not admission.
//! Compile directly with the pinned rustc-dev toolchain; never builds dependencies.
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{Body, TerminatorKind};
use rustc_middle::ty::{EarlyBinder, Instance, TyCtxt, TyKind, TypingEnv};
use rustc_session::config::Input;
use rustc_span::FileName;

mod body;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{CurrentTarget, DeviceMath, KernelCapabilityBrand, KernelContext,
    NumericalPolicyCapability, PolicyDeviceMath, RegisteredLaunch, StrictIeee};
type Brand<'a> = KernelCapabilityBrand<'a, u8, CurrentTarget, RegisteredLaunch>;
pub fn getter<'a>(context: &KernelContext<'a, u8>) -> DeviceMath<Brand<'a>> {
    context.math()
}
pub fn bind<'a>(math: &'a DeviceMath<Brand<'a>>,
    policy: &'a NumericalPolicyCapability<Brand<'a>, StrictIeee>)
    -> PolicyDeviceMath<'a, Brand<'a>, StrictIeee> {
    math.with_numerical_policy(policy)
}
"#;

struct Probe;

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("policy_defined_body_observation.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        println!("target={}", tcx.sess.target.llvm_target);
        for name in ["getter", "bind"] {
            let definition = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .expect("probe definition");
            let body = tcx.optimized_mir(definition);
            let callees = callees(tcx, None, body);
            assert_eq!(callees.len(), 1, "one original source call in {name}");
            dump(tcx, callees[0], 0);
            check(tcx, name, callees[0]);
        }
        Compilation::Stop
    }
}

fn check<'tcx>(tcx: TyCtxt<'tcx>, name: &str, instance: Instance<'tcx>) {
    let original = tcx.instance_mir(instance.def);
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if name == "getter" {
        let bridge = callees(tcx, Some(instance), original)[0];
        let validate = |body: &Body<'tcx>| {
            body::kernel_math_getter(
                tcx,
                instance,
                body,
                bridge,
                signature.inputs()[0],
                signature.output(),
            )
        };
        assert!(validate(original));
        let mut wrong_owner = original.clone();
        wrong_owner.local_decls[rustc_middle::mir::Local::from_usize(1)].ty = tcx.types.u32;
        assert!(!validate(&wrong_owner));
        let mut wrong_destination = original.clone();
        let TerminatorKind::Call { destination, .. } = &mut wrong_destination.basic_blocks.as_mut()
            [rustc_middle::mir::START_BLOCK]
            .terminator_mut()
            .kind
        else {
            panic!()
        };
        *destination = rustc_middle::mir::Local::from_usize(1).into();
        assert!(!validate(&wrong_destination));
        let mut no_call = original.clone();
        no_call.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK]
            .terminator_mut()
            .kind = TerminatorKind::Return;
        assert!(!validate(&no_call));
        let bridge_body = tcx.instance_mir(bridge.def);
        let terminal = callees(tcx, Some(bridge), bridge_body)[0];
        let terminal_output = tcx
            .instantiate_bound_regions_with_erased(
                tcx.fn_sig(terminal.def_id())
                    .instantiate(tcx, terminal.args),
            )
            .output();
        assert!(body::branded_math_bridge(
            tcx,
            bridge,
            bridge_body,
            terminal,
            signature.output(),
            terminal_output
        ));
        assert!(!body::branded_math_bridge(
            tcx,
            bridge,
            bridge_body,
            bridge,
            signature.output(),
            terminal_output
        ));
        println!(
            "closed_predicates=getter,bridge accepted; wrong-owner,destination,missing-call,bridge-callee rejected"
        );
    } else {
        let validate = |body: &Body<'tcx>| {
            body::policy_math_bind(
                tcx,
                instance,
                body,
                signature.inputs()[0],
                signature.inputs()[1],
                signature.output(),
            )
        };
        assert!(validate(original));
        let mut swapped = original.clone();
        let rustc_middle::mir::StatementKind::Assign(assignment) =
            &mut swapped.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK].statements[0].kind
        else {
            panic!()
        };
        let rustc_middle::mir::Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
            panic!()
        };
        operands.raw.swap(0, 1);
        assert!(!validate(&swapped));
        let mut missing = original.clone();
        missing.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK]
            .statements
            .clear();
        assert!(!validate(&missing));
        let mut extra = original.clone();
        extra.basic_blocks.as_mut()[rustc_middle::mir::START_BLOCK]
            .statements
            .push(original.basic_blocks[rustc_middle::mir::START_BLOCK].statements[0].clone());
        assert!(!validate(&extra));
        println!(
            "closed_predicates=bind accepted; swapped-references,missing-aggregate,extra-statement rejected"
        );
    }
}

fn callees<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Option<Instance<'tcx>>,
    body: &Body<'tcx>,
) -> Vec<Instance<'tcx>> {
    body.basic_blocks
        .iter()
        .filter_map(|block| {
            let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                return None;
            };
            let mut ty = func.ty(&body.local_decls, tcx);
            if let Some(caller) = caller {
                ty = caller
                    .try_instantiate_mir_and_normalize_erasing_regions(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                        EarlyBinder::bind(ty),
                    )
                    .expect("concrete called type");
            }
            let TyKind::FnDef(definition, args) = *ty.kind() else {
                return None;
            };
            Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                definition,
                tcx.erase_and_anonymize_regions(args),
            )
            .expect("resolved source instance")
        })
        .collect()
}

fn dump<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, depth: usize) {
    if !tcx.is_mir_available(instance.def_id()) {
        println!(
            "leaf={} original_mir_available=false",
            tcx.def_path_str(instance.def_id())
        );
        return;
    }
    let body = tcx.instance_mir(instance.def);
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    println!(
        "definition={} instance={instance:?}",
        tcx.def_path_str(instance.def_id())
    );
    println!(
        "signature={signature:?} phase={:?} args={} locals={} blocks={}",
        body.phase,
        body.arg_count,
        body.local_decls.len(),
        body.basic_blocks.len()
    );
    let abi = tcx
        .fn_abi_of_instance(
            TypingEnv::fully_monomorphized()
                .as_query_input((instance, rustc_middle::ty::List::empty())),
        )
        .expect("observed ABI");
    println!(
        "abi_arguments={:?} abi_return={:?} can_unwind={}",
        abi.args.iter().map(|arg| &arg.mode).collect::<Vec<_>>(),
        abi.ret.mode,
        abi.can_unwind
    );
    for (local, declaration) in body.local_decls.iter_enumerated() {
        let ty = instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(declaration.ty),
            )
            .expect("concrete local type");
        println!("  {local:?}: {ty:?}");
    }
    for (block, data) in body.basic_blocks.iter_enumerated() {
        println!("  {block:?} cleanup={}", data.is_cleanup);
        for (index, statement) in data.statements.iter().enumerate() {
            println!("    {index}: {:?}", statement.kind);
        }
        println!("    {:?}", data.terminator().kind);
    }
    if depth < 2 {
        for callee in callees(tcx, Some(instance), body) {
            if tcx.crate_name(callee.def_id().krate).as_str() == "fe2o3_device" {
                dump(tcx, callee, depth + 1);
            }
        }
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    args[0] = "rustc".into();
    rustc_driver::run_compiler(&args, &mut Probe);
}
