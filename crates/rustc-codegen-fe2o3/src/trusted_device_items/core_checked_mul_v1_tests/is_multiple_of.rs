use super::*;
use rustc_middle::mir::AssertKind;

const SOURCE: &str = r#"
#![no_std]
pub fn route(a: u32, b: u32) -> bool { a.is_multiple_of(b) }
pub fn literal(a: u32, b: u32) -> bool { match b { 0 => a == 0, _ => a % b == 0 } }
pub fn width(a: u64, b: u64) -> bool { a.is_multiple_of(b) }
pub fn narrow(a: u16, b: u16) -> bool { a.is_multiple_of(b) }
pub fn word(a: usize, b: usize) -> bool { a.is_multiple_of(b) }
pub fn is_multiple_of(a: u32, b: u32) -> bool { a == b }
pub fn foreign(a: u32, b: u32) -> bool { is_multiple_of(a, b) }
"#;

struct Probe {
    ran: bool,
}

fn accepts<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> bool {
    reviewed_body(tcx, body, Helper::IsMultipleOf)
}

fn one<'tcx>(tcx: TyCtxt<'tcx>) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::Val(
            ConstValue::Scalar(rustc_middle::mir::interpret::Scalar::from_uint(
                1_u32,
                rustc_abi::Size::from_bytes(4),
            )),
            tcx.types.u32,
        ),
    }))
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("is_multiple_of_authentication.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let body = |name| {
            let id = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap();
            tcx.optimized_mir(id)
        };
        let called = |name| {
            body(name)
                .basic_blocks
                .iter()
                .find_map(|block| match &block.terminator().kind {
                    TerminatorKind::Call { func, .. } => resolve_callee(tcx, func),
                    _ => None,
                })
                .unwrap()
        };
        let instance = called("route");
        assert_eq!(helper_identity(tcx, instance), Some(Helper::IsMultipleOf));
        assert!(authenticate_reviewed_safe_core_arithmetic_helper_v1(
            tcx, instance
        ));
        for name in ["width", "narrow", "word", "foreign"] {
            assert!(
                !authenticate_reviewed_safe_core_arithmetic_helper_v1(tcx, called(name)),
                "{name}"
            );
        }
        assert!(!authenticate_reviewed_safe_core_arithmetic_helper_v1(
            tcx,
            Instance::mono(tcx, body("literal").source.def_id())
        ));
        for source in [tcx.instance_mir(instance.def), body("literal")] {
            let original = format!("{source:?}");
            assert!(
                accepts(tcx, source),
                "actual pinned core and independently compiled literal source"
            );
            assert_eq!(
                format!("{source:?}"),
                original,
                "source authentication must not remove the assertion"
            );
            assert_eq!(
                source
                    .basic_blocks
                    .iter()
                    .filter(|block| matches!(
                        block.terminator().kind,
                        TerminatorKind::Assert { .. }
                    ))
                    .count(),
                1
            );
            mir_tests::storage_and_moves(tcx, source, Helper::IsMultipleOf);
            mir_tests::budget_boundaries(tcx, source, Helper::IsMultipleOf);
            for (block, data) in source.basic_blocks.iter_enumerated() {
                for (index, statement) in data.statements.iter().enumerate() {
                    let StatementKind::Assign(assignment) = &statement.kind else {
                        continue;
                    };
                    let Rvalue::BinaryOp(original, _) = &assignment.1 else {
                        continue;
                    };
                    for wrong in [
                        BinOp::Eq,
                        BinOp::Ne,
                        BinOp::Lt,
                        BinOp::Le,
                        BinOp::Gt,
                        BinOp::Ge,
                        BinOp::Rem,
                        BinOp::Div,
                        BinOp::Add,
                        BinOp::BitAnd,
                    ] {
                        if wrong == *original {
                            continue;
                        }
                        let mut changed = source.clone();
                        let Rvalue::BinaryOp(op, _) = expression(&mut changed, block, index) else {
                            unreachable!()
                        };
                        *op = wrong;
                        assert!(!accepts(tcx, &changed), "{original:?} -> {wrong:?}");
                    }
                    for mutation in 0..3 {
                        let mut changed = source.clone();
                        let Rvalue::BinaryOp(_, operands) = expression(&mut changed, block, index)
                        else {
                            unreachable!()
                        };
                        match mutation {
                            0 => std::mem::swap(&mut operands.0, &mut operands.1),
                            1 => operands.1 = one(tcx),
                            2 => operands.0 = operands.1.clone(),
                            _ => unreachable!(),
                        }
                        assert!(!accepts(tcx, &changed), "operand substitution");
                    }
                }
                if let TerminatorKind::Assert { target, .. } = data.terminator().kind {
                    let mut changed = source.clone();
                    changed.basic_blocks_mut()[block].terminator_mut().kind =
                        TerminatorKind::Goto { target };
                    assert!(!accepts(tcx, &changed), "remainder assertion is mandatory");
                    for mutation in 0..7 {
                        let mut changed = source.clone();
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
                            1 => *cond = bool_operand(tcx, false),
                            2 => *unwind = UnwindAction::Cleanup(*target),
                            3 => *target = block,
                            4 => {
                                **msg = AssertKind::DivisionByZero(Operand::Copy(
                                    Local::from_usize(1).into(),
                                ))
                            }
                            5 => {
                                **msg = AssertKind::RemainderByZero(Operand::Copy(
                                    Local::from_usize(2).into(),
                                ))
                            }
                            6 => *target = BasicBlock::from_usize(source.basic_blocks.len()),
                            _ => unreachable!(),
                        }
                        assert!(!accepts(tcx, &changed), "assertion mutation {mutation}");
                    }
                }
                if let TerminatorKind::SwitchInt { targets, .. } = &data.terminator().kind {
                    for (zero, nonzero) in [
                        (targets.otherwise(), targets.target_for_value(0)),
                        (targets.otherwise(), targets.otherwise()),
                        (targets.target_for_value(0), targets.target_for_value(0)),
                    ] {
                        let mut changed = source.clone();
                        let TerminatorKind::SwitchInt { targets, .. } =
                            &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                        else {
                            unreachable!()
                        };
                        *targets = SwitchTargets::new([(0, zero)].into_iter(), nonzero);
                        assert!(!accepts(tcx, &changed), "zero-divisor control flow");
                    }
                }
                let mut changed = source.clone();
                changed.basic_blocks_mut()[block].terminator_mut().kind =
                    TerminatorKind::Goto { target: block };
                assert!(!accepts(tcx, &changed), "cycle");
                let mut changed = source.clone();
                changed.basic_blocks_mut()[block].is_cleanup = true;
                assert!(!accepts(tcx, &changed), "cleanup");
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

fn run(mir_opt_zero: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=is_multiple_of_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
    ];
    if mir_opt_zero {
        args.push("-Zmir-opt-level=0".into());
    }
    args.push("-".into());
    let mut probe = Probe { ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn core_is_multiple_of_pinned_source_mutations_and_budgets() {
    run(false);
}

#[test]
fn core_is_multiple_of_unoptimized_source_mutations_and_budgets() {
    run(true);
}
