use super::super::super::{ProductionMirV1, production_mir_v1};
use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{ConstOperand, SwitchTargets};
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

const SOURCE: &str = r#"
#![no_std]
#![allow(dead_code)]
pub fn route(a: u32, b: u32) -> u32 { a.wrapping_shr(b) }
pub fn cast_unsigned(a: u32) -> u64 { u64::from(a) }
pub fn cast_float(a: u8) -> f32 { f32::from(a) }
pub fn add(a: u32, b: u32) -> u32 { a.wrapping_add(b) }
pub fn mul(a: u32, b: u32) -> u32 { a.wrapping_mul(b) }
pub fn wrong_width(a: u64, b: u32) -> u64 { a.wrapping_shr(b) }
pub fn wrong_sign(a: i32, b: u32) -> i32 { a.wrapping_shr(b) }
pub fn wrong_direction(a: u32, b: u32) -> u32 { a.wrapping_shl(b) }
pub fn unchecked(a: u32, b: u32) -> u32 { unsafe { a.unchecked_shr(b) } }
pub fn wrapping_shr(a: u32, b: u32) -> u32 { a.wrapping_shr(b) }
pub fn foreign(a: u32, b: u32) -> u32 { wrapping_shr(a, b) }
pub struct Impostor;
impl Impostor { pub fn wrapping_shr(a: u32, b: u32) -> u32 { a.wrapping_shr(b) } }
pub fn foreign_method(a: u32, b: u32) -> u32 { Impostor::wrapping_shr(a, b) }
pub async fn coroutine_header(a: u32) -> u32 { a }
"#;

#[derive(Clone, Copy)]
enum Check {
    Positive,
    Arithmetic,
    ControlFlow,
    HelperGraph,
    EffectsAndBudget,
    RetainedReplay,
}

struct Probe {
    check: Check,
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("wrapping_shr_authentication.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let body = |name| {
            let definition = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .expect("fixture function");
            tcx.optimized_mir(definition)
        };
        let called = |name| {
            body(name)
                .basic_blocks
                .iter()
                .find_map(|block| {
                    let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                        return None;
                    };
                    resolve(tcx, func)
                })
                .expect("fixture retained call")
        };
        let root = called("route");
        let actual = tcx.instance_mir(root.def);
        let proof: super::super::ReviewedU32WrappingShrV1<'tcx> =
            super::super::prove_core_u32_wrapping_shr_v1(tcx, root)
                .expect("pinned core source proof");
        let precondition = resolve(
            tcx,
            match &actual.basic_blocks[BasicBlock::from_usize(1)]
                .terminator()
                .kind
            {
                TerminatorKind::Call { func, .. } => func,
                _ => panic!("pinned precondition call"),
            },
        )
        .unwrap();
        let unchecked = actual.source_scopes[SourceScope::from_usize(1)]
            .inlined
            .unwrap()
            .0;
        let language_ub = actual.source_scopes[SourceScope::from_usize(2)]
            .inlined
            .unwrap()
            .0;
        let runtime = actual.source_scopes[SourceScope::from_usize(3)]
            .inlined
            .unwrap()
            .0;
        match self.check {
            Check::RetainedReplay => retained_tests::replay(tcx, root),
            Check::Positive => {
                for name in ["cast_unsigned", "cast_float"] {
                    let instance = called(name);
                    let selected = production_mir_v1(tcx, instance);
                    assert_eq!(selected.instance(), instance);
                    assert!(selected.is_source_expansion());
                    assert_eq!(selected.body().basic_blocks.len(), 1);
                    assert_eq!(selected.body().local_decls.len(), 2);
                    assert!(selected.expansion_fingerprint(tcx).is_some());
                }
                assert_eq!(proof.instance(), root);
                let selected: ProductionMirV1<'tcx> = production_mir_v1(tcx, root);
                assert_eq!(selected.instance(), root);
                assert!(selected.is_source_expansion());
                assert_eq!(selected.body().basic_blocks.len(), 1);
                let fingerprint = selected.expansion_fingerprint(tcx).unwrap();
                assert_eq!(
                    Some(fingerprint),
                    production_mir_v1(tcx, root).expansion_fingerprint(tcx),
                );
                let mut changed = selected.body().clone();
                let StatementKind::Assign(changed_assignment) =
                    &mut changed.basic_blocks_mut()[BasicBlock::from_usize(0)].statements[1].kind
                else {
                    panic!("expanded assignment")
                };
                let Rvalue::BinaryOp(operation, _) = &mut changed_assignment.1 else {
                    panic!("expanded binary")
                };
                *operation = BinOp::Shl;
                assert_ne!(fingerprint, proof.expansion_fingerprint(tcx, &changed));
                let expanded = proof.expand_mir(tcx);
                assert_eq!(expanded.arg_count, 2);
                assert_eq!(expanded.local_decls.len(), 4);
                assert!(
                    expanded
                        .local_decls
                        .iter()
                        .all(|decl| decl.ty == tcx.types.u32)
                );
                assert_eq!(expanded.basic_blocks.len(), 1);
                assert_eq!(expanded.source_scopes.len(), 1);
                assert!(
                    expanded
                        .source_scopes
                        .iter()
                        .all(|scope| scope.inlined.is_none())
                );
                let block = &expanded.basic_blocks[BasicBlock::from_usize(0)];
                let [mask, shift] = &block.statements[..] else {
                    panic!("exact two-operation expansion")
                };
                let Some(Rvalue::BinaryOp(BinOp::BitAnd, operands)) = assignment(mask, 3) else {
                    panic!("explicit modulo mask")
                };
                assert_eq!(operand_local_v1(&operands.0), Some(2));
                assert_eq!(scalar(tcx, &operands.1, tcx.types.u32), Some(31));
                let Some(Rvalue::BinaryOp(BinOp::Shr, operands)) = assignment(shift, 0) else {
                    panic!("safe unsigned shift")
                };
                assert_eq!(operand_local_v1(&operands.0), Some(1));
                assert_eq!(operand_local_v1(&operands.1), Some(3));
                assert!(matches!(block.terminator().kind, TerminatorKind::Return));
                assert_eq!(u32::BITS - 1, 31);
                for name in [
                    "wrong_width",
                    "wrong_sign",
                    "wrong_direction",
                    "unchecked",
                    "foreign",
                    "foreign_method",
                ] {
                    assert!(
                        prove_core_u32_wrapping_shr_v1(tcx, called(name)).is_none(),
                        "{name}"
                    );
                    let selected = production_mir_v1(tcx, called(name));
                    assert!(!selected.is_source_expansion());
                    assert_eq!(selected.expansion_fingerprint(tcx), None);
                    assert!(std::ptr::eq(
                        selected.body(),
                        tcx.instance_mir(called(name).def)
                    ));
                }
                for instance in [root, unchecked, precondition, language_ub, runtime] {
                    assert!(
                        !authenticate_reviewed_safe_core_wrapping_helper_v1(tcx, instance),
                        "shift proof is not a source-safety exemption"
                    );
                    if instance != root {
                        assert!(prove_core_u32_wrapping_shr_v1(tcx, instance).is_none());
                        let selected = production_mir_v1(tcx, instance);
                        assert!(!selected.is_source_expansion());
                        assert!(std::ptr::eq(
                            selected.body(),
                            tcx.instance_mir(instance.def)
                        ));
                    }
                }
                for name in ["add", "mul"] {
                    assert!(authenticate_reviewed_safe_core_wrapping_helper_v1(
                        tcx,
                        called(name)
                    ));
                }
                let generic = Instance {
                    def: root.def,
                    args: tcx.mk_args(&[tcx.types.u32.into()]),
                };
                assert!(prove_core_u32_wrapping_shr_v1(tcx, generic).is_none());
            }
            Check::Arithmetic => {
                for (block, data) in actual.basic_blocks.iter_enumerated() {
                    for (index, statement) in data.statements.iter().enumerate() {
                        let StatementKind::Assign(assignment) = &statement.kind else {
                            continue;
                        };
                        let Rvalue::BinaryOp(observed, _) = &assignment.1 else {
                            continue;
                        };
                        for wrong in [
                            BinOp::BitAnd,
                            BinOp::BitOr,
                            BinOp::BitXor,
                            BinOp::Shr,
                            BinOp::ShrUnchecked,
                            BinOp::ShlUnchecked,
                            BinOp::Add,
                        ] {
                            if wrong == *observed {
                                continue;
                            }
                            let mut changed = actual.clone();
                            let Rvalue::BinaryOp(operation, _) =
                                assigned_value(&mut changed, block, index)
                            else {
                                unreachable!()
                            };
                            *operation = wrong;
                            rejects(tcx, root, root, &changed);
                        }
                        for wrong_local in [0, 1, 2, 3] {
                            let mut changed = actual.clone();
                            let Rvalue::BinaryOp(_, operands) =
                                assigned_value(&mut changed, block, index)
                            else {
                                unreachable!()
                            };
                            if operand_local_v1(&operands.1) == Some(wrong_local) {
                                continue;
                            }
                            operands.1 = Operand::Copy(Local::from_usize(wrong_local).into());
                            rejects(tcx, root, root, &changed);
                        }
                        let mut changed = actual.clone();
                        let Rvalue::BinaryOp(_, operands) =
                            assigned_value(&mut changed, block, index)
                        else {
                            unreachable!()
                        };
                        std::mem::swap(&mut operands.0, &mut operands.1);
                        rejects(tcx, root, root, &changed);
                    }
                }
                for mask in [0, 1, 30, 32, 63, u32::MAX] {
                    let mut changed = actual.clone();
                    let Rvalue::BinaryOp(_, operands) =
                        assigned_value(&mut changed, BasicBlock::from_usize(0), 1)
                    else {
                        unreachable!()
                    };
                    operands.1 = word(tcx, mask);
                    rejects(tcx, root, root, &changed);
                }
            }
            Check::ControlFlow => {
                for target in [0, 1, 3] {
                    let mut changed = actual.clone();
                    let TerminatorKind::SwitchInt { targets, .. } = &mut changed.basic_blocks_mut()
                        [BasicBlock::from_usize(0)]
                    .terminator_mut()
                    .kind
                    else {
                        unreachable!()
                    };
                    *targets = SwitchTargets::new(
                        [(0, BasicBlock::from_usize(target))].into_iter(),
                        BasicBlock::from_usize(1),
                    );
                    rejects(tcx, root, root, &changed);
                }
                for flag in [false, true] {
                    let mut changed = actual.clone();
                    let TerminatorKind::SwitchInt { discr, .. } = &mut changed.basic_blocks_mut()
                        [BasicBlock::from_usize(0)]
                    .terminator_mut()
                    .kind
                    else {
                        unreachable!()
                    };
                    *discr = Operand::Constant(Box::new(ConstOperand {
                        span: DUMMY_SP,
                        user_ty: None,
                        const_: Const::from_bool(tcx, flag),
                    }));
                    rejects(tcx, root, root, &changed);
                }
                for change in 0..6 {
                    let mut changed = actual.clone();
                    let TerminatorKind::Call {
                        args,
                        target,
                        unwind,
                        destination,
                        func,
                        ..
                    } = &mut changed.basic_blocks_mut()[BasicBlock::from_usize(1)]
                        .terminator_mut()
                        .kind
                    else {
                        unreachable!()
                    };
                    match change {
                        0 => args[0].node = Operand::Copy(Local::from_usize(2).into()),
                        1 => *target = None,
                        2 => *unwind = UnwindAction::Cleanup(BasicBlock::from_usize(2)),
                        3 => *destination = Local::from_usize(0).into(),
                        4 => *func = function_operand(tcx, called("foreign")),
                        5 => args[0].node = Operand::Move(Local::from_usize(3).into()),
                        _ => unreachable!(),
                    }
                    rejects(tcx, root, root, &changed);
                }
            }
            Check::HelperGraph => {
                let source = tcx.instance_mir(precondition.def);
                for change in 0..3 {
                    let mut changed = source.clone();
                    let Rvalue::BinaryOp(_, operands) =
                        assigned_value(&mut changed, BasicBlock::from_usize(0), 1)
                    else {
                        unreachable!()
                    };
                    match change {
                        0 => operands.0 = word(tcx, 0),
                        1 => operands.0 = Operand::Copy(Local::from_usize(0).into()),
                        2 => std::mem::swap(&mut operands.0, &mut operands.1),
                        _ => unreachable!(),
                    }
                    rejects(tcx, root, precondition, &changed);
                }
                for change in 0..4 {
                    let mut changed = source.clone();
                    let TerminatorKind::SwitchInt { discr, targets } = &mut changed
                        .basic_blocks_mut()[BasicBlock::from_usize(0)]
                    .terminator_mut()
                    .kind
                    else {
                        unreachable!()
                    };
                    match change {
                        0 => *discr = Operand::Copy(Local::from_usize(1).into()),
                        1 => {
                            *targets = SwitchTargets::new(
                                [(0, BasicBlock::from_usize(1))].into_iter(),
                                BasicBlock::from_usize(2),
                            )
                        }
                        2 => {
                            *targets = SwitchTargets::new(
                                [(0, BasicBlock::from_usize(2))].into_iter(),
                                BasicBlock::from_usize(2),
                            )
                        }
                        3 => {
                            *targets = SwitchTargets::new(
                                [(1, BasicBlock::from_usize(2))].into_iter(),
                                BasicBlock::from_usize(1),
                            )
                        }
                        _ => unreachable!(),
                    }
                    rejects(tcx, root, precondition, &changed);
                }
                for change in 0..7 {
                    let mut changed = source.clone();
                    let TerminatorKind::Call {
                        args,
                        destination,
                        target,
                        unwind,
                        ..
                    } = &mut changed.basic_blocks_mut()[BasicBlock::from_usize(2)]
                        .terminator_mut()
                        .kind
                    else {
                        unreachable!()
                    };
                    match change {
                        0 => {
                            args[1].node = Operand::Constant(Box::new(ConstOperand {
                                span: DUMMY_SP,
                                user_ty: None,
                                const_: Const::from_bool(tcx, true),
                            }))
                        }
                        1 => args[0].node = Operand::Copy(Local::from_usize(3).into()),
                        2 => args.swap(0, 1),
                        3 => *destination = Local::from_usize(0).into(),
                        4 => *target = Some(BasicBlock::from_usize(1)),
                        5 => *unwind = UnwindAction::Cleanup(BasicBlock::from_usize(1)),
                        6 => *unwind = UnwindAction::Continue,
                        _ => unreachable!(),
                    }
                    rejects(tcx, root, precondition, &changed);
                }
                for limit in [0, 1, 31, 33, 64, u32::MAX] {
                    let mut changed = source.clone();
                    let Rvalue::BinaryOp(_, operands) =
                        assigned_value(&mut changed, BasicBlock::from_usize(0), 1)
                    else {
                        unreachable!()
                    };
                    operands.1 = word(tcx, limit);
                    rejects(tcx, root, precondition, &changed);
                }
                for wrong in [BinOp::Le, BinOp::Gt, BinOp::Ge, BinOp::Eq, BinOp::Ne] {
                    let mut changed = source.clone();
                    let Rvalue::BinaryOp(operation, _) =
                        assigned_value(&mut changed, BasicBlock::from_usize(0), 1)
                    else {
                        unreachable!()
                    };
                    *operation = wrong;
                    rejects(tcx, root, precondition, &changed);
                }
                let mut changed = source.clone();
                changed.basic_blocks_mut()[BasicBlock::from_usize(1)]
                    .terminator_mut()
                    .kind = TerminatorKind::Goto {
                    target: BasicBlock::from_usize(2),
                };
                rejects(tcx, root, precondition, &changed);
                let mut changed = source.clone();
                let TerminatorKind::Call { func, .. } = &mut changed.basic_blocks_mut()
                    [BasicBlock::from_usize(2)]
                .terminator_mut()
                .kind
                else {
                    unreachable!()
                };
                *func = function_operand(tcx, called("foreign"));
                rejects(tcx, root, precondition, &changed);
                let mut changed = tcx.instance_mir(unchecked.def).clone();
                let Rvalue::BinaryOp(operation, _) =
                    assigned_value(&mut changed, BasicBlock::from_usize(2), 0)
                else {
                    unreachable!()
                };
                *operation = BinOp::ShlUnchecked;
                rejects(tcx, root, unchecked, &changed);
                let mut changed = tcx.instance_mir(runtime.def).clone();
                *assigned_value(&mut changed, BasicBlock::from_usize(0), 0) =
                    Rvalue::Use(Operand::Constant(Box::new(ConstOperand {
                        span: DUMMY_SP,
                        user_ty: None,
                        const_: Const::from_bool(tcx, false),
                    })));
                rejects(tcx, root, runtime, &changed);
            }
            Check::EffectsAndBudget => {
                let coroutine = tcx
                    .iter_local_def_id()
                    .filter(|id| tcx.def_kind(*id) == DefKind::Closure)
                    .find_map(|id| tcx.optimized_mir(id).coroutine.clone())
                    .expect("actual pinned coroutine header");
                for instance in [root, unchecked, precondition, language_ub, runtime] {
                    let source = tcx.instance_mir(instance.def);
                    let mut changed = source.clone();
                    changed.coroutine = Some(coroutine.clone());
                    rejects(tcx, root, instance, &changed);
                    let mut changed = source.clone();
                    changed.source.promoted = Some(rustc_middle::mir::Promoted::from_usize(0));
                    rejects(tcx, root, instance, &changed);
                    let mut changed = source.clone();
                    changed.source.instance = called("foreign").def;
                    rejects(tcx, root, instance, &changed);
                    let mut changed = source.clone();
                    changed.spread_arg = Some(Local::from_usize(1));
                    rejects(tcx, root, instance, &changed);
                    if let Some(debug) = source.var_debug_info.first() {
                        let mut changed = source.clone();
                        changed.var_debug_info = vec![debug.clone(); 33];
                        rejects(tcx, root, instance, &changed);
                    }
                    for (block, _) in source.basic_blocks.iter_enumerated() {
                        let mut changed = source.clone();
                        changed.basic_blocks_mut()[block]
                            .statements
                            .push(Statement::new(
                                SourceInfo::outermost(DUMMY_SP),
                                StatementKind::Nop,
                            ));
                        rejects(tcx, root, instance, &changed);
                        let mut changed = source.clone();
                        changed.basic_blocks_mut()[block].is_cleanup = true;
                        rejects(tcx, root, instance, &changed);
                    }
                    let mut changed = source.clone();
                    changed.local_decls[Local::from_usize(0)].ty = tcx.types.u64;
                    rejects(tcx, root, instance, &changed);
                    let mut changed = source.clone();
                    let local = changed.local_decls[Local::from_usize(0)].clone();
                    while changed.local_decls.len() <= 16 {
                        changed.local_decls.push(local.clone());
                    }
                    rejects(tcx, root, instance, &changed);
                    let mut changed = source.clone();
                    changed.source_scopes[SourceScope::from_usize(0)].inlined =
                        Some((root, DUMMY_SP));
                    rejects(tcx, root, instance, &changed);
                    let mut changed = source.clone();
                    changed
                        .basic_blocks_mut()
                        .push(source.basic_blocks[BasicBlock::from_usize(0)].clone());
                    rejects(tcx, root, instance, &changed);
                }
                let mut exhausted = 0;
                assert!(!check_instance(
                    tcx,
                    root,
                    Helper::Wrapping,
                    &|instance| tcx.instance_mir(instance.def),
                    &mut exhausted
                ));
                let mut bound = 16;
                assert!(check_instance(
                    tcx,
                    root,
                    Helper::Wrapping,
                    &|instance| tcx.instance_mir(instance.def),
                    &mut bound
                ));
                assert_eq!(
                    bound, 6,
                    "the pinned closed graph checks ten bounded bodies"
                );
                for (limit, expected) in [(9, false), (10, true)] {
                    let mut budget = limit;
                    let visits = std::cell::Cell::new(0);
                    assert_eq!(
                        check_instance(
                            tcx,
                            root,
                            Helper::Wrapping,
                            &|instance| {
                                visits.set(visits.get() + 1);
                                tcx.instance_mir(instance.def)
                            },
                            &mut budget,
                        ),
                        expected,
                        "budget={limit}"
                    );
                    assert_eq!(visits.get(), limit);
                    assert_eq!(budget, 0);
                }
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn assigned_value<'a, 'tcx>(
    body: &'a mut Body<'tcx>,
    block: BasicBlock,
    index: usize,
) -> &'a mut Rvalue<'tcx> {
    let StatementKind::Assign(assignment) =
        &mut body.basic_blocks_mut()[block].statements[index].kind
    else {
        panic!("pinned assignment")
    };
    &mut assignment.1
}

fn rejects<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    instance: Instance<'tcx>,
    changed: &Body<'tcx>,
) {
    assert!(
        prove_with(tcx, root, &|callee| {
            if callee == instance {
                changed
            } else {
                tcx.instance_mir(callee.def)
            }
        })
        .is_none(),
        "changed source body of {}",
        tcx.def_path_str(instance.def_id())
    );
}

fn word<'tcx>(tcx: TyCtxt<'tcx>, value: u32) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::from_bits(
            tcx,
            value.into(),
            TypingEnv::fully_monomorphized(),
            tcx.types.u32,
        ),
    }))
}

fn function_operand<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::Val(
            ConstValue::ZeroSized,
            Ty::new_fn_def(tcx, instance.def_id(), instance.args),
        ),
    }))
}

fn run(check: Check) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=wrapping_shr_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-".into(),
    ];
    let mut probe = Probe { check, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn core_wrapping_shr_pinned_identity_and_masked_expansion() {
    run(Check::Positive);
}
#[test]
fn core_wrapping_shr_rejects_wrong_mask_operation_and_operands() {
    run(Check::Arithmetic);
}
#[test]
fn core_wrapping_shr_rejects_changed_control_flow_and_calls() {
    run(Check::ControlFlow);
}
#[test]
fn core_wrapping_shr_rejects_changed_helper_predicate_and_failure_path() {
    run(Check::HelperGraph);
}
#[test]
fn core_wrapping_shr_rejects_effects_types_cycles_and_exhausted_budgets() {
    run(Check::EffectsAndBudget);
}
#[test]
fn core_wrapping_shr_retained_capture_replay_and_mutations() {
    run(Check::RetainedReplay);
}
