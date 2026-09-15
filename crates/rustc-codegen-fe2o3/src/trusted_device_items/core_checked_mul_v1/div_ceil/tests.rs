use super::super::{
    authenticate_reviewed_safe_core_arithmetic_helper_v1 as authenticate,
    reviewed_body as arithmetic_body, tests::mir_tests,
};
use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{
    ConstOperand, Local, Promoted, SourceInfo, SourceScope, Statement, SwitchTargets,
};
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

const SOURCE: &str = r#"
#![no_std]
#![feature(int_roundings)]
#![allow(dead_code)]
pub fn route(a: usize, b: usize) -> usize { a.div_ceil(b) }
pub fn checked_source(a: usize, b: usize) -> usize {
    let d = a / b; let r = a % b; if r > 0 { d + 1 } else { d }
}
pub fn width(a: u32, b: u32) -> u32 { a.div_ceil(b) }
pub fn same_width(a: u64, b: u64) -> u64 { a.div_ceil(b) }
pub fn signed(a: isize, b: isize) -> isize { a.div_ceil(b) }
pub fn other(a: usize, b: usize) -> usize { a.div_floor(b) }
pub fn div_ceil(a: usize, b: usize) -> usize { a / b }
pub struct Impostor;
impl Impostor { pub fn div_ceil(a: usize, b: usize) -> usize { a / b } }
pub fn foreign(a: usize, b: usize) -> usize { div_ceil(a, b) }
pub fn foreign_method(a: usize, b: usize) -> usize { Impostor::div_ceil(a, b) }
pub async fn coroutine_header(a: usize) -> usize { a }
"#;

#[derive(Clone, Copy)]
enum Check {
    Positive,
    Arithmetic,
    Assertions,
    Flow,
    Effects,
    StorageAndMoves,
    BudgetBoundaries,
}

struct Probe {
    check: Check,
    ran: bool,
}

fn accepts<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> bool {
    arithmetic_body(tcx, body, Helper::DivCeil)
}

fn word<'tcx>(tcx: TyCtxt<'tcx>, value: u128) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::Val(
            ConstValue::Scalar(rustc_middle::mir::interpret::Scalar::from_uint(
                value,
                tcx.data_layout.pointer_size(),
            )),
            tcx.types.usize,
        ),
    }))
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("div_ceil_authentication.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let local = |name| {
            let definition = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap();
            Instance::mono(tcx, definition.to_def_id())
        };
        let called = |name| {
            tcx.instance_mir(local(name).def)
                .basic_blocks
                .iter()
                .find_map(|block| {
                    let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                        return None;
                    };
                    resolve_callee(tcx, func)
                })
                .unwrap()
        };
        let root = called("route");
        let actual = tcx.instance_mir(root.def);
        let checked = tcx.instance_mir(local("checked_source").def);
        assert!(authenticate(tcx, root), "pinned core div_ceil source");
        assert!(
            accepts(tcx, checked),
            "actual checked-MIR representation of the reviewed expression"
        );
        match self.check {
            Check::StorageAndMoves => {
                for body in [actual, checked] {
                    mir_tests::storage_and_moves(tcx, body, Helper::DivCeil);
                }
                let mut moved = checked.clone();
                // The checked pair's condition and result are distinct move
                // paths. Consuming the flag must not consume the result field.
                for data in moved.basic_blocks_mut().iter_mut() {
                    if let TerminatorKind::Assert { cond, msg, .. } =
                        &mut data.terminator_mut().kind
                    {
                        if let Operand::Copy(place) = cond {
                            *cond = Operand::Move(*place);
                        }
                        match &mut **msg {
                            AssertKind::DivisionByZero(operand)
                            | AssertKind::RemainderByZero(operand) => {
                                if let Operand::Copy(place) = operand {
                                    *operand = Operand::Move(*place);
                                }
                            }
                            _ => {}
                        }
                    }
                    for statement in &mut data.statements {
                        if let StatementKind::Assign(assignment) = &mut statement.kind
                            && let Rvalue::Use(Operand::Copy(place)) = assignment.1
                            && !place.projection.is_empty()
                        {
                            assignment.1 = Rvalue::Use(Operand::Move(place));
                        }
                    }
                }
                assert!(
                    accepts(tcx, &moved),
                    "independent pair moves and failure-only diagnostic moves"
                );
                for (block, data) in moved.basic_blocks.iter_enumerated() {
                    for (index, statement) in data.statements.iter().enumerate() {
                        if let StatementKind::Assign(assignment) = &statement.kind
                            && matches!(&assignment.1, Rvalue::Use(Operand::Move(place)) if !place.projection.is_empty())
                        {
                            let mut changed = moved.clone();
                            changed.basic_blocks_mut()[block]
                                .statements
                                .insert(index, statement.clone());
                            assert!(!accepts(tcx, &changed), "same tuple field moved twice");
                        }
                    }
                }
            }
            Check::BudgetBoundaries => {
                for body in [actual, checked] {
                    mir_tests::budget_boundaries(tcx, body, Helper::DivCeil);
                }
            }
            Check::Positive => {
                assert_eq!(helper_identity(tcx, root), Some(Helper::DivCeil));
                for name in [
                    "width",
                    "same_width",
                    "signed",
                    "other",
                    "foreign",
                    "foreign_method",
                ] {
                    assert!(!authenticate(tcx, called(name)), "{name}");
                }
                assert!(
                    !authenticate(tcx, local("checked_source")),
                    "body equality grants no origin authority"
                );
                let generic = Instance {
                    def: root.def,
                    args: tcx.mk_args(&[tcx.types.usize.into()]),
                };
                assert!(!authenticate(tcx, generic));
                let sig = tcx.instantiate_bound_regions_with_erased(
                    tcx.fn_sig(root.def_id()).instantiate(tcx, root.args),
                );
                for wrong in [
                    FnSig {
                        safety: Safety::Unsafe,
                        ..sig
                    },
                    FnSig {
                        abi: ExternAbi::C { unwind: false },
                        ..sig
                    },
                    FnSig {
                        c_variadic: true,
                        ..sig
                    },
                    FnSig {
                        inputs_and_output: tcx.mk_type_list(&[
                            tcx.types.usize,
                            tcx.types.usize,
                            tcx.types.u32,
                        ]),
                        ..sig
                    },
                ] {
                    assert!(!signature_matches(tcx, Helper::DivCeil, wrong));
                }
                for (body, expected_asserts) in [(actual, 1), (checked, 3)] {
                    let original = format!("{body:?}");
                    assert!(accepts(tcx, body));
                    assert_eq!(
                        format!("{body:?}"),
                        original,
                        "authentication must never rewrite source MIR"
                    );
                    assert_eq!(
                        body.basic_blocks
                            .iter()
                            .filter(|block| matches!(
                                block.terminator().kind,
                                TerminatorKind::Assert { .. }
                            ))
                            .count(),
                        expected_asserts
                    );
                    assert!(body.basic_blocks.iter().all(|block| !matches!(
                        block.terminator().kind,
                        TerminatorKind::Call { .. }
                    )));
                }
            }
            Check::Arithmetic => {
                for body in [actual, checked] {
                    for (block, data) in body.basic_blocks.iter_enumerated() {
                        for (index, statement) in data.statements.iter().enumerate() {
                            let StatementKind::Assign(assignment) = &statement.kind else {
                                continue;
                            };
                            let Rvalue::BinaryOp(original, _) = &assignment.1 else {
                                continue;
                            };
                            for wrong in [
                                BinOp::Add,
                                BinOp::Sub,
                                BinOp::Mul,
                                BinOp::Div,
                                BinOp::Rem,
                                BinOp::Eq,
                                BinOp::Gt,
                                BinOp::Ge,
                                BinOp::Lt,
                                BinOp::AddUnchecked,
                                BinOp::AddWithOverflow,
                            ] {
                                if wrong == *original {
                                    continue;
                                }
                                let mut changed = body.clone();
                                let Rvalue::BinaryOp(op, _) =
                                    expression(&mut changed, block, index)
                                else {
                                    unreachable!()
                                };
                                *op = wrong;
                                assert!(!accepts(tcx, &changed), "{original:?} -> {wrong:?}");
                            }
                            for mutation in 0..3 {
                                let mut changed = body.clone();
                                let Rvalue::BinaryOp(_, operands) =
                                    expression(&mut changed, block, index)
                                else {
                                    unreachable!()
                                };
                                match mutation {
                                    0 => std::mem::swap(&mut operands.0, &mut operands.1),
                                    1 => operands.0 = word(tcx, 1),
                                    2 => operands.1 = word(tcx, 2),
                                    _ => unreachable!(),
                                }
                                assert!(
                                    !accepts(tcx, &changed),
                                    "{original:?} operands mutation={mutation}"
                                );
                            }
                        }
                    }
                }
            }
            Check::Assertions => {
                for body in [actual, checked] {
                    for (block, data) in body.basic_blocks.iter_enumerated() {
                        let TerminatorKind::Assert { target, .. } = data.terminator().kind else {
                            continue;
                        };
                        let mut changed = body.clone();
                        changed.basic_blocks_mut()[block].terminator_mut().kind =
                            TerminatorKind::Goto { target };
                        let is_remainder = matches!(&data.terminator().kind, TerminatorKind::Assert { msg, .. } if matches!(&**msg, AssertKind::RemainderByZero(_)));
                        if !is_remainder {
                            assert!(
                                !accepts(tcx, &changed),
                                "division/increment asserts are mandatory"
                            );
                        }
                        for mutation in 0..6 {
                            let mut changed = body.clone();
                            let TerminatorKind::Assert {
                                cond,
                                expected,
                                msg,
                                target,
                                unwind,
                            } = &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                            else {
                                unreachable!()
                            };
                            match mutation {
                                0 => *expected = true,
                                1 => *cond = Operand::Copy(Local::from_usize(1).into()),
                                2 => *unwind = UnwindAction::Cleanup(*target),
                                3 => *target = block,
                                4 => **msg = AssertKind::DivisionByZero(word(tcx, 0)),
                                5 => {
                                    **msg =
                                        AssertKind::Overflow(BinOp::Sub, word(tcx, 0), word(tcx, 1))
                                }
                                _ => unreachable!(),
                            }
                            assert!(!accepts(tcx, &changed), "assert mutation={mutation}");
                        }
                    }
                }
            }
            Check::Flow => {
                for body in [actual, checked] {
                    for (block, data) in body.basic_blocks.iter_enumerated() {
                        let mut changed = body.clone();
                        changed.basic_blocks_mut()[block].terminator_mut().kind =
                            TerminatorKind::Goto { target: block };
                        assert!(!accepts(tcx, &changed), "loop");
                        if let TerminatorKind::SwitchInt { targets, .. } = &data.terminator().kind {
                            let zero = targets.target_for_value(0);
                            let one = targets.otherwise();
                            for (zero, one) in [(one, zero), (zero, zero), (one, one)] {
                                let mut changed = body.clone();
                                let TerminatorKind::SwitchInt { targets, .. } =
                                    &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                                else {
                                    unreachable!()
                                };
                                *targets = SwitchTargets::new([(0, zero)].into_iter(), one);
                                assert!(!accepts(tcx, &changed), "wrong branch");
                            }
                        }
                    }
                    let mut changed = body.clone();
                    changed
                        .basic_blocks_mut()
                        .push(body.basic_blocks[BasicBlock::from_usize(0)].clone());
                    assert!(!accepts(tcx, &changed), "unreachable extra block");
                }
            }
            Check::Effects => {
                let coroutine = tcx
                    .iter_local_def_id()
                    .filter(|id| tcx.def_kind(*id) == DefKind::Closure)
                    .find_map(|id| tcx.optimized_mir(id).coroutine.clone())
                    .unwrap();
                for body in [actual, checked] {
                    for (block, _) in body.basic_blocks.iter_enumerated() {
                        let mut changed = body.clone();
                        changed.basic_blocks_mut()[block]
                            .statements
                            .push(Statement::new(
                                SourceInfo::outermost(DUMMY_SP),
                                StatementKind::Nop,
                            ));
                        assert!(!accepts(tcx, &changed), "extra effect");
                        let mut changed = body.clone();
                        changed.basic_blocks_mut()[block].is_cleanup = true;
                        assert!(!accepts(tcx, &changed), "cleanup");
                    }
                    let mut changed = body.clone();
                    changed.spread_arg = Some(Local::from_usize(2));
                    assert!(!accepts(tcx, &changed));
                    let mut changed = body.clone();
                    changed.coroutine = Some(coroutine.clone());
                    assert!(!accepts(tcx, &changed));
                    let mut changed = body.clone();
                    changed.source.promoted = Some(Promoted::from_usize(0));
                    assert!(!accepts(tcx, &changed));
                    let mut changed = body.clone();
                    changed.local_decls[Local::from_usize(1)].ty = tcx.types.u64;
                    assert!(!accepts(tcx, &changed));
                    let mut changed = body.clone();
                    changed.source_scopes[SourceScope::from_usize(0)].inlined =
                        Some((root, DUMMY_SP));
                    assert!(!accepts(tcx, &changed));
                    let mut changed = body.clone();
                    while changed.local_decls.len() <= MAX_LOCALS {
                        changed
                            .local_decls
                            .push(body.local_decls[Local::from_usize(0)].clone());
                    }
                    assert!(!accepts(tcx, &changed));
                    let mut changed = body.clone();
                    while changed.basic_blocks.len() <= MAX_BLOCKS {
                        changed
                            .basic_blocks_mut()
                            .push(body.basic_blocks[BasicBlock::from_usize(0)].clone());
                    }
                    assert!(!accepts(tcx, &changed));
                }
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn expression<'a, 'tcx>(
    body: &'a mut Body<'tcx>,
    block: BasicBlock,
    index: usize,
) -> &'a mut Rvalue<'tcx> {
    let StatementKind::Assign(assignment) =
        &mut body.basic_blocks_mut()[block].statements[index].kind
    else {
        unreachable!()
    };
    &mut assignment.1
}

fn run(check: Check) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=div_ceil_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Coverflow-checks=yes".into(),
        "-".into(),
    ];
    let mut probe = Probe { check, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn core_div_ceil_tracks_storage_and_consumes_moves() {
    run(Check::StorageAndMoves);
}
#[test]
fn core_div_ceil_accepts_valid_budget_boundaries() {
    run(Check::BudgetBoundaries);
}
#[test]
fn core_div_ceil_pinned_identity_and_assert_preservation() {
    run(Check::Positive);
}
#[test]
fn core_div_ceil_rejects_arithmetic_and_operand_mutations() {
    run(Check::Arithmetic);
}
#[test]
fn core_div_ceil_rejects_assertion_mutations() {
    run(Check::Assertions);
}
#[test]
fn core_div_ceil_rejects_control_flow_mutations() {
    run(Check::Flow);
}
#[test]
fn core_div_ceil_rejects_headers_effects_and_budgets() {
    run(Check::Effects);
}
