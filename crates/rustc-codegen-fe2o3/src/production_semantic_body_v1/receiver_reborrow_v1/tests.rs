//! Component checks of the observation leaf, not source-pipeline admission.

use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{
    AggregateKind, BasicBlock, BasicBlockData, Const, ConstOperand, ProjectionElem, StatementKind,
};
use rustc_middle::ty::InstanceKind;

const SOURCE: &str = r#"
#![allow(dead_code)]
struct Token(u32);
#[inline(never)]
fn invoke<F: FnOnce(u32) -> u32>(f: F, value: u32) -> u32 { f(value) }
fn invoke_zero<F: FnOnce() -> u32>(f: F) -> u32 { f() }
fn invoke_unit<F: FnOnce(()) -> u32>(f: F) -> u32 { f(()) }
fn invoke_pair<F: FnOnce(u32, u32) -> u32>(f: F) -> u32 { f(7, 3) }
fn invoke_tuple<F: FnOnce((u32, u32)) -> (u32, ())>(f: F) -> (u32, ()) { f((7, 3)) }
#[inline(never)]
fn through_fn<F: Fn(u32) -> u32>(f: F, value: u32) -> u32 { invoke(f, value) }
#[inline(never)]
fn through_mut<F: FnMut(u32) -> u32>(f: F, value: u32) -> u32 { invoke(f, value) }
#[inline(never)]
fn through_zero<F: Fn() -> u32>(f: F) -> u32 { invoke_zero(f) }
#[inline(never)]
fn through_unit<F: Fn(()) -> u32>(f: F) -> u32 { invoke_unit(f) }
#[inline(never)]
fn through_pair<F: Fn(u32, u32) -> u32>(f: F) -> u32 { invoke_pair(f) }
#[inline(never)]
fn through_tuple<F: Fn((u32, u32)) -> (u32, ())>(f: F) -> (u32, ()) { invoke_tuple(f) }
pub fn shared(seed: u32) -> u32 { through_fn(move |value| seed ^ value, 7) }
pub fn mutable(mut seed: u32) -> u32 {
    through_mut(move |value| { seed ^= value; seed }, 7)
}
pub fn once(seed: u32) -> u32 {
    let token = Token(seed);
    invoke(move |value| { let moved = token; moved.0 ^ value }, 7)
}
pub fn zero() -> u32 { through_zero(|| 7) }
pub fn unit() -> u32 { through_unit(|()| 7) }
pub fn pair() -> u32 { through_pair(|a, b| a ^ b) }
pub fn tuple() -> (u32, ()) { through_tuple(|(a, b)| (a ^ b, ())) }
pub fn plain(value: u32) -> u32 { value }
"#;

fn local_function(tcx: TyCtxt<'_>, name: &str) -> rustc_hir::def_id::DefId {
    tcx.iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing fixture function {name}"))
        .to_def_id()
}

fn closure<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let caller = Instance::mono(tcx, local_function(tcx, name));
    tcx.instance_mir(caller.def)
        .local_decls
        .iter()
        .find_map(|local| match local.ty.kind() {
            TyKind::Closure(definition, arguments) => {
                Some(Instance::new_raw(*definition, arguments))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing actual closure in {name}"))
}

fn derive<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) -> Option<ReceiverReborrowV1<'tcx>> {
    derive_fn_receiver_reborrow_v1(tcx, instance, body, |_| Ok::<_, ()>(()))
        .unwrap_or_else(|error| panic!("receiver observation: {error:?}"))
}

fn reject<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>, label: &str) {
    assert!(
        matches!(
            derive_fn_receiver_reborrow_v1(tcx, instance, body, |_| Ok::<_, ()>(())),
            Err(ReceiverReborrowErrorV1::Unsupported(_))
        ),
        "{label} must fail closed"
    );
}

fn call_mut<'a, 'tcx>(body: &'a mut Body<'tcx>, block: BasicBlock) -> &'a mut TerminatorKind<'tcx> {
    &mut body.basic_blocks.as_mut()[block].terminator_mut().kind
}

#[derive(Default)]
struct ReborrowCallbacks {
    completed: bool,
}

impl Callbacks for ReborrowCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        for name in ["shared", "zero", "unit", "pair", "tuple", "mutable"] {
            let actual = closure(tcx, name);
            assert_eq!(
                actual.args.as_closure().kind(),
                if name == "mutable" {
                    ClosureKind::FnMut
                } else {
                    ClosureKind::Fn
                },
                "{name}: the source bound must establish the actual closure kind"
            );
            let shim =
                Instance::resolve_closure(tcx, actual.def_id(), actual.args, ClosureKind::FnOnce);
            assert!(matches!(shim.def, InstanceKind::ClosureOnceShim { .. }));
            let body = tcx.instance_mir(shim.def);
            let shim_signature = source_signature_v1(tcx, shim).unwrap();
            assert_eq!(shim_signature.abi, ExternAbi::RustCall, "{name}");
            assert_eq!(
                shim_signature.inputs().len(),
                2,
                "owned self and packed tuple"
            );
            assert!(matches!(
                shim_signature.inputs()[1].kind(),
                TyKind::Tuple(_)
            ));
            assert_eq!(body.arg_count, 2, "{name}: shim MIR keeps the tuple packed");
            assert_eq!(
                body.spread_arg,
                Some(Local::from_usize(shim_signature.inputs().len())),
                "{name}: live FnOnce shim spreads its final MIR argument (_2)"
            );
            assert_eq!(
                tcx.instance_mir(actual.def).spread_arg,
                None,
                "{name}: the actual closure Item has expanded MIR arguments"
            );
            let before = format!("{body:?}");
            let observation = derive(tcx, shim, body);
            if name == "mutable" {
                assert!(observation.is_none(), "FnMut retains its mutable receiver");
                let mut changed = body.clone();
                for data in changed.basic_blocks.as_mut().iter_mut() {
                    data.statements.clear();
                }
                reject(tcx, shim, &changed, "malformed FnMut shim");
            } else {
                let observation = observation.expect("Fn requires a shared receiver");
                assert_eq!(observation.callee(), actual);
                assert!(observation.receiver().as_local().is_some());
                assert!(matches!(
                    observation.shared_receiver_type().kind(),
                    TyKind::Ref(_, _, Mutability::Not)
                ));
                let signature = source_signature_v1(tcx, actual).unwrap();
                assert_eq!(signature.inputs()[0], observation.shared_receiver_type());
            }
            assert_eq!(format!("{body:?}"), before, "derive must not change MIR");
        }

        let actual_once = closure(tcx, "once");
        assert_eq!(actual_once.args.as_closure().kind(), ClosureKind::FnOnce);
        let once = Instance::resolve_closure(
            tcx,
            actual_once.def_id(),
            actual_once.args,
            ClosureKind::FnOnce,
        );
        assert_eq!(once, actual_once, "actual FnOnce has no owned adapter");
        assert!(derive(tcx, once, tcx.instance_mir(once.def)).is_none());
        let plain = Instance::mono(tcx, local_function(tcx, "plain"));
        assert!(derive(tcx, plain, tcx.instance_mir(plain.def)).is_none());

        let actual = closure(tcx, "shared");
        let shim =
            Instance::resolve_closure(tcx, actual.def_id(), actual.args, ClosureKind::FnOnce);
        let body = tcx.instance_mir(shim.def);
        let observation = derive(tcx, shim, body).unwrap();
        let block = BasicBlock::from_usize(observation.block() as usize);
        let receiver = observation.receiver().local;
        let source_info = body.basic_blocks[block].terminator().source_info;
        let borrow = body.basic_blocks[block]
            .statements
            .iter()
            .position(|statement| {
                statement
                    .kind
                    .as_assign()
                    .is_some_and(|(destination, _)| destination.as_local() == Some(receiver))
            })
            .expect("owned-self borrow statement");

        let InstanceKind::ClosureOnceShim { call_once, .. } = shim.def else {
            unreachable!();
        };
        reject(
            tcx,
            Instance {
                def: InstanceKind::ClosureOnceShim {
                    call_once,
                    track_caller: true,
                },
                args: shim.args,
            },
            body,
            "unsupported track-caller shim",
        );
        reject(
            tcx,
            Instance {
                def: InstanceKind::ClosureOnceShim {
                    call_once: actual.def_id(),
                    track_caller: false,
                },
                args: shim.args,
            },
            body,
            "closure definition substituted for FnOnce method",
        );

        let mut changed = body.clone();
        changed.source.instance = actual.def;
        reject(tcx, shim, &changed, "substituted source");
        let mut changed = body.clone();
        changed.arg_count = 1;
        reject(tcx, shim, &changed, "argument count");
        for spread_arg in [None, Some(RETURN_PLACE), Some(Local::from_usize(1))] {
            let mut changed = body.clone();
            changed.spread_arg = spread_arg;
            reject(
                tcx,
                shim,
                &changed,
                "missing or wrong RustCall tuple mapping",
            );
        }
        for local in [RETURN_PLACE, Local::from_usize(1), Local::from_usize(2)] {
            let mut changed = body.clone();
            changed.local_decls[local].ty = tcx.types.bool;
            reject(tcx, shim, &changed, "argument or output type");
        }
        let mut changed = body.clone();
        changed.local_decls[receiver].ty = observation.shared_receiver_type();
        reject(tcx, shim, &changed, "retagged original receiver");
        let mut changed = body.clone();
        *call_mut(&mut changed, block) = TerminatorKind::Return;
        reject(tcx, shim, &changed, "missing receiver call");
        let mut changed = body.clone();
        changed
            .basic_blocks
            .as_mut()
            .push(body.basic_blocks[block].clone());
        reject(tcx, shim, &changed, "duplicate unreachable receiver call");

        for replacement in [
            Operand::Copy(observation.receiver()),
            Operand::Move(Place {
                local: receiver,
                projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
            }),
            Operand::Move(Place::from(Local::from_usize(1))),
        ] {
            let mut changed = body.clone();
            let TerminatorKind::Call { args, .. } = call_mut(&mut changed, block) else {
                unreachable!();
            };
            args[0].node = replacement;
            reject(tcx, shim, &changed, "unsupported receiver operand");
        }
        let mut changed = body.clone();
        let TerminatorKind::Call { args, .. } = call_mut(&mut changed, block) else {
            unreachable!();
        };
        args[1].node = Operand::Move(Place::return_place());
        reject(tcx, shim, &changed, "substituted tuple");

        let mut changed = body.clone();
        let fn_trait = tcx.lang_items().fn_trait().unwrap();
        let wrong_method = tcx
            .associated_items(fn_trait)
            .in_definition_order()
            .find(|item| item.is_fn())
            .unwrap()
            .def_id;
        let TerminatorKind::Call { func, .. } = call_mut(&mut changed, block) else {
            unreachable!();
        };
        *func = Operand::Constant(Box::new(ConstOperand {
            span: source_info.span,
            user_ty: None,
            const_: Const::zero_sized(tcx.type_of(wrong_method).instantiate_identity()),
        }));
        reject(
            tcx,
            shim,
            &changed,
            "Fn method substituted for FnMut method",
        );

        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[block]
            .statements
            .remove(borrow);
        reject(tcx, shim, &changed, "supplied body missing borrow");
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[block]
            .statements
            .push(body.basic_blocks[block].statements[borrow].clone());
        reject(tcx, shim, &changed, "duplicate receiver assignment");
        let mut changed = body.clone();
        let StatementKind::Assign(assignment) =
            &mut changed.basic_blocks.as_mut()[block].statements[borrow].kind
        else {
            unreachable!();
        };
        assignment.1 = Rvalue::Ref(
            tcx.lifetimes.re_erased,
            BorrowKind::Shared,
            Place::from(Local::from_usize(1)),
        );
        reject(
            tcx,
            shim,
            &changed,
            "shared borrow substituted for owned mutable borrow",
        );
        let mut changed = body.clone();
        let StatementKind::Assign(assignment) =
            &mut changed.basic_blocks.as_mut()[block].statements[borrow].kind
        else {
            unreachable!();
        };
        assignment.1 = Rvalue::Ref(
            tcx.lifetimes.re_erased,
            BorrowKind::Mut {
                kind: MutBorrowKind::Default,
            },
            Place::from(Local::from_usize(2)),
        );
        reject(tcx, shim, &changed, "borrow of tuple instead of owned self");

        let mut changed = body.clone();
        changed
            .basic_blocks
            .as_mut()
            .push(BasicBlockData::new_stmts(
                vec![Statement::new(
                    source_info,
                    StatementKind::StorageDead(receiver),
                )],
                Some(Terminator {
                    source_info,
                    kind: TerminatorKind::Unreachable,
                }),
                false,
            ));
        reject(tcx, shim, &changed, "unreachable receiver lifetime use");
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[block].statements[borrow]
            .debuginfos
            .push(StmtDebugInfo::InvalidAssign(receiver));
        assert_eq!(derive(tcx, shim, &changed), Some(observation));

        let mut work = 0;
        derive_fn_receiver_reborrow_v1(tcx, shim, body, |amount| {
            work += amount;
            Ok::<_, ()>(())
        })
        .unwrap();
        for limit in [0, work - 1, work] {
            let mut spent = 0;
            let result = derive_fn_receiver_reborrow_v1(tcx, shim, body, |amount| {
                spent += amount;
                if spent > limit { Err(()) } else { Ok(()) }
            });
            if limit == work {
                assert_eq!(result.unwrap(), Some(observation));
            } else {
                assert!(matches!(result, Err(ReceiverReborrowErrorV1::Resource(()))));
            }
        }
        let large = Rvalue::Aggregate(
            Box::new(AggregateKind::Tuple),
            std::iter::repeat_n(Operand::Copy(observation.receiver()), 1024).collect(),
        );
        let mut charges = Vec::new();
        let mut charge = |amount| {
            charges.push(amount);
            Err(amount)
        };
        let mut visitor = ReceiverUsesV1 {
            receiver,
            count: 0,
            local_count: body.local_decls.len(),
            charge: &mut charge,
            error: None,
        };
        visitor.visit_rvalue(
            &large,
            Location {
                block,
                statement_index: borrow,
            },
        );
        assert!(matches!(
            visitor.error,
            Some(ReceiverReborrowErrorV1::Resource(1025))
        ));
        assert_eq!(visitor.count, 0, "denied traversal cannot visit children");
        assert_eq!(charges, [1025]);
        assert_eq!(derive(tcx, shim, body), Some(observation));
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn fn_receiver_observation_checks_supplied_mir_signatures_uses_and_work() {
    let directory = TestTempDir::create("fe2o3-fn-receiver-reborrow");
    let source = directory.path().join("fixture.rs");
    std::fs::write(&source, SOURCE).unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    let args = vec![
        "rustc".into(),
        "--crate-name".into(),
        "fe2o3_fn_receiver_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = ReborrowCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
