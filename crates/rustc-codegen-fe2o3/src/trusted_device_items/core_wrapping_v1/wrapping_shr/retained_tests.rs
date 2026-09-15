//! Actual cached-profile mutation tests plus a local replay of the captured
//! retained MIR. Replay exercises the matcher, not device-metadata provenance;
//! the public positive/producer tests still require real cached AMDGPU core.

use super::*;
use rustc_middle::mir::{AssertMessage, CastKind, ProjectionElem, SwitchTargets, UnOp};
use rustc_span::DUMMY_SP;

#[path = "retained_regression_tests.rs"]
mod regressions;

const HELPERS: [Helper; 9] = [
    Helper::Wrapping,
    Helper::Unchecked,
    Helper::LanguageUb,
    Helper::Runtime,
    Helper::Precondition,
    Helper::FormatFromStr,
    Helper::StrAsPtr,
    Helper::StrLen,
    Helper::StrAsBytes,
];

pub(crate) fn diagnose<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
    if let Some(&helper) = HELPERS
        .iter()
        .find(|&&helper| identity(tcx, instance, helper))
    {
        let mut budget = 16;
        let accepted = check_instance(
            tcx,
            instance,
            helper,
            &|callee| tcx.instance_mir(callee.def),
            &mut budget,
        );
        eprintln!("proof role={helper:?} accepted={accepted} remaining_budget={budget}");
    }
}

fn graph<'tcx>(tcx: TyCtxt<'tcx>, root: Instance<'tcx>) -> Vec<Instance<'tcx>> {
    let mut pending = vec![root];
    let mut seen = Vec::new();
    while let Some(instance) = pending.pop() {
        if seen.contains(&instance) {
            continue;
        }
        assert!(seen.len() < 16, "bounded test graph");
        seen.push(instance);
        let body = tcx.instance_mir(instance.def);
        let calls = body
            .basic_blocks
            .iter()
            .filter_map(|block| match &block.terminator().kind {
                TerminatorKind::Call { func, .. } => resolve(tcx, func),
                _ => None,
            });
        let scopes = body
            .source_scopes
            .iter()
            .filter_map(|scope| scope.inlined.map(|(callee, _)| callee));
        for callee in calls.chain(scopes) {
            if HELPERS.iter().any(|&helper| identity(tcx, callee, helper)) {
                pending.push(callee);
            }
        }
    }
    HELPERS
        .iter()
        .map(|&helper| {
            *seen
                .iter()
                .find(|&&instance| identity(tcx, instance, helper))
                .unwrap_or_else(|| panic!("missing {helper:?}"))
        })
        .collect()
}

pub(crate) fn actual_profile<'tcx>(tcx: TyCtxt<'tcx>, root: Instance<'tcx>) {
    assert_eq!(
        tcx.instance_mir(root.def).source_scopes.len(),
        1,
        "retained cached-core profile required"
    );
    let instances = graph(tcx, root);
    exercise(tcx, root, &instances, &|instance| {
        tcx.instance_mir(instance.def)
    });
}

fn exercise<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    instances: &[Instance<'tcx>],
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
) where
    'tcx: 'a,
{
    let proof = prove_with(tcx, root, fetch).unwrap_or_else(|| {
        for (&instance, helper) in instances.iter().zip(HELPERS) {
            let mut budget = 16;
            eprintln!(
                "retained {helper:?}: accepted={} remaining={budget}",
                check_instance(tcx, instance, helper, fetch, &mut budget)
            );
        }
        panic!("complete retained source proof");
    });
    regressions::exercise(tcx, root, proof, instances, fetch);
    let bits_constant = fetch(root).required_consts.as_ref().unwrap()[0];
    assert!(retained::bits(tcx, &constant(bits_constant.const_)));
    for value in [0, 31, 32, 33, u32::MAX] {
        assert!(
            !retained::bits(tcx, &word(tcx, value)),
            "retained BITS requires its exact core origin"
        );
    }
    let Const::Unevaluated(original, ty) = bits_constant.const_ else {
        panic!("retained core BITS")
    };
    for change in 0..4 {
        let mut changed = original;
        let mut changed_ty = ty;
        match change {
            0 => changed.def = root.def_id(),
            1 => changed.args = tcx.mk_args(&[tcx.types.u64.into()]),
            2 => changed.promoted = Some(rustc_middle::mir::Promoted::from_usize(0)),
            3 => changed_ty = tcx.types.u64,
            _ => unreachable!(),
        }
        assert!(!retained::bits(
            tcx,
            &constant(Const::Unevaluated(changed, changed_ty))
        ));
    }
    for (instance, helper) in instances.iter().copied().zip(HELPERS) {
        assert!(identity(tcx, instance, helper));
        assert!(!identity(
            tcx,
            Instance {
                def: instance.def,
                args: tcx.mk_args(&[tcx.types.u64.into()])
            },
            helper
        ));
        if instance != root {
            assert!(
                prove_with(tcx, instance, fetch).is_none(),
                "{helper:?} is not a safe terminal"
            );
        }
        let source = fetch(instance);
        let reject = |changed: &Body<'tcx>| {
            assert!(
                prove_with(tcx, root, &|callee| if callee == instance {
                    changed
                } else {
                    fetch(callee)
                })
                .is_none(),
                "accepted mutation of {helper:?}: {changed:?}"
            );
        };
        let mut changed = source.clone();
        changed.spread_arg = Some(Local::from_usize(1));
        reject(&changed);
        let mut changed = source.clone();
        changed.source.promoted = Some(rustc_middle::mir::Promoted::from_usize(0));
        reject(&changed);
        let mut changed = source.clone();
        changed.source.instance = instances[if instance == root { 1 } else { 0 }].def;
        reject(&changed);
        let mut changed = source.clone();
        changed.local_decls[Local::from_usize(0)].ty = tcx.types.u64;
        reject(&changed);
        let mut changed = source.clone();
        changed.source_scopes[SourceScope::from_usize(0)].parent_scope =
            Some(SourceScope::from_usize(0));
        reject(&changed);
        let mut changed = source.clone();
        changed.source_scopes[SourceScope::from_usize(0)].inlined = Some((root, DUMMY_SP));
        reject(&changed);
        let mut changed = source.clone();
        changed.source_scopes[SourceScope::from_usize(0)].inlined_parent_scope =
            Some(SourceScope::from_usize(0));
        reject(&changed);
        let mut changed = source.clone();
        changed.required_consts = None;
        reject(&changed);
        for (block, data) in source.basic_blocks.iter_enumerated() {
            let mut changed = source.clone();
            changed.basic_blocks_mut()[block].is_cleanup = true;
            reject(&changed);
            let mut changed = source.clone();
            changed.basic_blocks_mut()[block].terminator = None;
            reject(&changed);
            for kind in [
                StatementKind::Nop,
                StatementKind::StorageDead(Local::from_usize(1)),
                StatementKind::StorageLive(Local::from_usize(1)),
            ] {
                let mut changed = source.clone();
                changed.basic_blocks_mut()[block]
                    .statements
                    .insert(0, Statement::new(SourceInfo::outermost(DUMMY_SP), kind));
                reject(&changed);
            }
            for (index, statement) in data.statements.iter().enumerate() {
                let StatementKind::Assign(assigned) = &statement.kind else {
                    continue;
                };
                let mut changed = source.clone();
                set_value(&mut changed, block, index, Rvalue::Use(copy(0)));
                reject(&changed);
                if let Rvalue::BinaryOp(original, operands) = &assigned.1 {
                    for wrong in [
                        BinOp::Add,
                        BinOp::Sub,
                        BinOp::SubWithOverflow,
                        BinOp::BitAnd,
                        BinOp::BitOr,
                        BinOp::ShlUnchecked,
                        BinOp::ShrUnchecked,
                        BinOp::Ge,
                    ] {
                        if wrong == *original {
                            continue;
                        }
                        let mut changed = source.clone();
                        set_value(
                            &mut changed,
                            block,
                            index,
                            Rvalue::BinaryOp(wrong, operands.clone()),
                        );
                        reject(&changed);
                    }
                    for wrong in [
                        (operands.1.clone(), operands.0.clone()),
                        (copy(0), operands.1.clone()),
                        (operands.0.clone(), word(tcx, 0)),
                    ] {
                        let mut changed = source.clone();
                        set_value(
                            &mut changed,
                            block,
                            index,
                            Rvalue::BinaryOp(*original, Box::new(wrong)),
                        );
                        reject(&changed);
                    }
                }
            }
            match &data.terminator().kind {
                TerminatorKind::Call { args, .. } => {
                    for change in 0..(6 + args.len() * 2) {
                        let mut changed = source.clone();
                        let TerminatorKind::Call {
                            func,
                            args,
                            destination,
                            target,
                            unwind,
                            ..
                        } = &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                        else {
                            unreachable!()
                        };
                        match change {
                            0 => *target = Some(block),
                            1 => *unwind = UnwindAction::Continue,
                            2 => *unwind = UnwindAction::Cleanup(block),
                            3 => {
                                *destination =
                                    Local::from_usize(if local(*destination, 1) { 0 } else { 1 })
                                        .into()
                            }
                            4 => *func = function(tcx, root),
                            5 => {
                                *target = if target.is_some() {
                                    None
                                } else {
                                    Some(BasicBlock::from_usize(0))
                                }
                            }
                            index if index < 6 + args.len() => args[index - 6].node = copy(0),
                            index => {
                                let count = args.len();
                                let arg = &mut args[index - 6 - count].node;
                                *arg = match arg {
                                    Operand::Copy(place) => Operand::Move(*place),
                                    Operand::Move(place) => Operand::Copy(*place),
                                    _ => boolean(tcx, true),
                                };
                            }
                        }
                        reject(&changed);
                    }
                }
                TerminatorKind::Assert { .. } => {
                    for change in 0..8 {
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
                        match change {
                            0 => *expected = !*expected,
                            1 => *target = block,
                            2 => *unwind = UnwindAction::Continue,
                            3 => *unwind = UnwindAction::Cleanup(block),
                            4 => *cond = boolean(tcx, *expected),
                            _ => {
                                let AssertMessage::Overflow(op, left, right) = &mut **msg else {
                                    unreachable!()
                                };
                                match change {
                                    5 => *op = BinOp::Mul,
                                    6 => *left = copy(0),
                                    7 => *right = word(tcx, 0),
                                    _ => unreachable!(),
                                }
                            }
                        }
                        reject(&changed);
                    }
                }
                TerminatorKind::SwitchInt { .. } => {
                    let mut changed = source.clone();
                    let TerminatorKind::SwitchInt { discr, .. } =
                        &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                    else {
                        unreachable!()
                    };
                    *discr = boolean(tcx, true);
                    reject(&changed);
                    let mut changed = source.clone();
                    let TerminatorKind::SwitchInt { targets, .. } =
                        &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                    else {
                        unreachable!()
                    };
                    *targets = SwitchTargets::new([(0, block)].into_iter(), block);
                    reject(&changed);
                }
                _ => {
                    let mut changed = source.clone();
                    changed.basic_blocks_mut()[block].terminator_mut().kind =
                        TerminatorKind::Goto { target: block };
                    reject(&changed);
                }
            }
        }
        // A semantically irrelevant span still changes the source-closure
        // binding, even when the exact same expansion remains justified.
        let mut changed = source.clone();
        changed.span = DUMMY_SP;
        if changed.span != source.span {
            let new_proof = prove_with(tcx, root, &|callee| {
                if callee == instance {
                    &changed
                } else {
                    fetch(callee)
                }
            })
            .expect("span-independent semantics");
            assert_ne!(proof.closure_fingerprint, new_proof.closure_fingerprint);
        }
    }
    for (limit, expected) in [(8, false), (9, true)] {
        let mut budget = limit;
        let visits = std::cell::Cell::new(0);
        assert_eq!(
            check_instance(
                tcx,
                root,
                Helper::Wrapping,
                &|instance| {
                    visits.set(visits.get() + 1);
                    fetch(instance)
                },
                &mut budget
            ),
            expected,
            "retained graph budget={limit}"
        );
        assert_eq!(budget, 0);
        assert_eq!(visits.get(), limit);
    }
}

fn copy<'tcx>(index: usize) -> Operand<'tcx> {
    Operand::Copy(Local::from_usize(index).into())
}
fn moved<'tcx>(index: usize) -> Operand<'tcx> {
    Operand::Move(Local::from_usize(index).into())
}

fn word<'tcx>(tcx: TyCtxt<'tcx>, value: u32) -> Operand<'tcx> {
    constant(Const::from_bits(
        tcx,
        value.into(),
        TypingEnv::fully_monomorphized(),
        tcx.types.u32,
    ))
}
fn boolean<'tcx>(tcx: TyCtxt<'tcx>, value: bool) -> Operand<'tcx> {
    constant(Const::from_bool(tcx, value))
}
fn constant(value: Const<'_>) -> Operand<'_> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: value,
    }))
}
fn function<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Operand<'tcx> {
    constant(Const::Val(
        ConstValue::ZeroSized,
        Ty::new_fn_def(tcx, instance.def_id(), instance.args),
    ))
}
fn set_value<'tcx>(body: &mut Body<'tcx>, block: BasicBlock, index: usize, value: Rvalue<'tcx>) {
    let StatementKind::Assign(assigned) =
        &mut body.basic_blocks_mut()[block].statements[index].kind
    else {
        unreachable!()
    };
    assigned.1 = value;
}

pub(super) fn replay<'tcx>(tcx: TyCtxt<'tcx>, root: Instance<'tcx>) {
    let instances = graph(tcx, root);
    let mut bodies: Vec<_> = instances
        .iter()
        .map(|instance| tcx.instance_mir(instance.def).clone())
        .collect();
    let bits_constant = bodies[0].required_consts.as_ref().unwrap()[0];
    assert!(retained::bits(tcx, &constant(bits_constant.const_)));
    let call_template = bodies[0]
        .basic_blocks
        .iter()
        .find_map(|block| {
            matches!(block.terminator().kind, TerminatorKind::Call { .. })
                .then(|| block.terminator().kind.clone())
        })
        .unwrap();
    let call = |instance, args: Vec<Operand<'tcx>>, dest, next: Option<usize>| {
        let mut kind = call_template.clone();
        let TerminatorKind::Call {
            func,
            args: arguments,
            destination,
            target,
            unwind,
            ..
        } = &mut kind
        else {
            unreachable!()
        };
        *func = function(tcx, instance);
        *arguments = args
            .into_iter()
            .map(|node| rustc_span::Spanned {
                node,
                span: DUMMY_SP,
            })
            .collect();
        *destination = Local::from_usize(dest).into();
        *target = next.map(BasicBlock::from_usize);
        *unwind = UnwindAction::Unreachable;
        kind
    };
    let panic = bodies[4]
        .basic_blocks
        .iter()
        .find_map(|block| match &block.terminator().kind {
            TerminatorKind::Call {
                func, target: None, ..
            } => resolve(tcx, func),
            _ => None,
        })
        .unwrap();
    let message = bodies[4]
        .basic_blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| {
            let StatementKind::Assign(assigned) = &statement.kind else {
                return None;
            };
            match &assigned.1 {
                Rvalue::Use(Operand::Constant(value))
                    if matches!(value.const_, Const::Val(ConstValue::Slice { .. }, _)) =>
                {
                    Some(Operand::Constant(value.clone()))
                }
                _ => None,
            }
        })
        .unwrap();
    let format = bodies[5].local_decls[Local::from_usize(0)].ty;
    let template = bodies[5].local_decls[Local::from_usize(2)].ty;
    let encoded = bodies[5].local_decls[Local::from_usize(4)].ty;
    let aggregate = bodies[5]
        .basic_blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| {
            let StatementKind::Assign(assigned) = &statement.kind else {
                return None;
            };
            match &assigned.1 {
                Rvalue::Aggregate(kind, _) => Some(kind.clone()),
                _ => None,
            }
        })
        .unwrap();
    let text = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, tcx.types.str_);
    let bytes = Ty::new_imm_ref(
        tcx,
        tcx.lifetimes.re_erased,
        Ty::new_slice(tcx, tcx.types.u8),
    );
    let ptr = Ty::new_imm_ptr(tcx, tcx.types.u8);
    let ptr_str = Ty::new_imm_ptr(tcx, tcx.types.str_);
    let si = SourceInfo::outermost(DUMMY_SP);
    let assign = |dest, value| {
        Statement::new(
            si,
            StatementKind::Assign(Box::new((Local::from_usize(dest).into(), value))),
        )
    };
    let binary = |op, left, right| Rvalue::BinaryOp(op, Box::new((left, right)));
    let bits = || constant(bits_constant.const_);
    let field = |index, ty| {
        Operand::Move(Place {
            local: Local::from_usize(5),
            projection: tcx.mk_place_elems(&[ProjectionElem::Field(
                rustc_abi::FieldIdx::from_usize(index),
                ty,
            )]),
        })
    };
    let switch = |discr, zero, otherwise| TerminatorKind::SwitchInt {
        discr,
        targets: SwitchTargets::new(
            [(0, BasicBlock::from_usize(zero))].into_iter(),
            BasicBlock::from_usize(otherwise),
        ),
    };
    let goto = |target| TerminatorKind::Goto {
        target: BasicBlock::from_usize(target),
    };
    let mut replace =
        |index: usize,
         types: &[Ty<'tcx>],
         blocks: Vec<(Vec<Statement<'tcx>>, TerminatorKind<'tcx>)>| {
            let body = &mut bodies[index];
            body.local_decls.raw = types
                .iter()
                .map(|&ty| LocalDecl::new(ty, DUMMY_SP))
                .collect();
            body.var_debug_info.clear();
            body.source_scopes.truncate(if index == 4 { 2 } else { 1 });
            body.required_consts = Some(match index {
                0 => vec![bits_constant; 2],
                4 => vec![bits_constant],
                _ => vec![],
            });
            body.basic_blocks_mut().raw.clear();
            for (statements, kind) in blocks {
                body.basic_blocks_mut().push(BasicBlockData::new_stmts(
                    statements,
                    Some(Terminator {
                        source_info: si,
                        kind,
                    }),
                    false,
                ));
            }
        };
    replace(
        0,
        &[
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            Ty::new_tup(tcx, &[tcx.types.u32, tcx.types.bool]),
        ],
        vec![
            (
                vec![assign(
                    5,
                    binary(BinOp::SubWithOverflow, bits(), word(tcx, 1)),
                )],
                TerminatorKind::Assert {
                    cond: field(1, tcx.types.bool),
                    expected: false,
                    msg: Box::new(AssertMessage::Overflow(BinOp::Sub, bits(), word(tcx, 1))),
                    target: BasicBlock::from_usize(1),
                    unwind: UnwindAction::Unreachable,
                },
            ),
            (
                vec![
                    assign(4, Rvalue::Use(field(0, tcx.types.u32))),
                    assign(3, binary(BinOp::BitAnd, copy(2), moved(4))),
                ],
                call(instances[1], vec![copy(1), moved(3)], 0, Some(2)),
            ),
            (vec![], TerminatorKind::Return),
        ],
    );
    replace(
        1,
        &[
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.bool,
            tcx.types.unit,
        ],
        vec![
            (vec![], call(instances[2], vec![], 3, Some(1))),
            (vec![], switch(moved(3), 3, 2)),
            (vec![], call(instances[4], vec![copy(2)], 4, Some(3))),
            (
                vec![assign(0, binary(BinOp::ShrUnchecked, copy(1), copy(2)))],
                TerminatorKind::Return,
            ),
        ],
    );
    replace(
        2,
        &[tcx.types.bool, tcx.types.bool],
        vec![
            (vec![], call(instances[3], vec![], 1, Some(1))),
            (vec![], switch(moved(1), 3, 2)),
            (
                vec![assign(
                    0,
                    Rvalue::Use(Operand::RuntimeChecks(RuntimeChecks::UbChecks)),
                )],
                goto(4),
            ),
            (vec![assign(0, Rvalue::Use(boolean(tcx, false)))], goto(4)),
            (vec![], TerminatorKind::Return),
        ],
    );
    replace(
        3,
        &[tcx.types.bool],
        vec![(
            vec![assign(0, Rvalue::UnaryOp(UnOp::Not, boolean(tcx, false)))],
            TerminatorKind::Return,
        )],
    );
    replace(
        4,
        &[
            tcx.types.unit,
            tcx.types.u32,
            tcx.types.bool,
            tcx.types.never,
            format,
            text,
        ],
        vec![
            (
                vec![assign(2, binary(BinOp::Lt, copy(1), bits()))],
                switch(moved(2), 2, 1),
            ),
            (vec![], TerminatorKind::Return),
            (
                vec![assign(5, Rvalue::Use(message))],
                call(instances[5], vec![moved(5)], 4, Some(3)),
            ),
            (
                vec![],
                call(panic, vec![moved(4), boolean(tcx, false)], 3, None),
            ),
        ],
    );
    let one_i32 = || {
        constant(Const::from_bits(
            tcx,
            1,
            TypingEnv::fully_monomorphized(),
            tcx.types.i32,
        ))
    };
    let one_usize = || {
        constant(Const::from_bits(
            tcx,
            1,
            TypingEnv::fully_monomorphized(),
            tcx.types.usize,
        ))
    };
    replace(
        5,
        &[
            format,
            text,
            template,
            ptr,
            encoded,
            tcx.types.usize,
            tcx.types.usize,
            tcx.types.usize,
            tcx.types.u32,
            tcx.types.bool,
        ],
        vec![
            (vec![], call(instances[6], vec![copy(1)], 3, Some(1))),
            (
                vec![assign(
                    2,
                    Rvalue::Cast(CastKind::Transmute, moved(3), template),
                )],
                call(instances[7], vec![copy(1)], 7, Some(2)),
            ),
            (
                vec![
                    assign(
                        8,
                        Rvalue::Cast(CastKind::IntToInt, one_i32(), tcx.types.u32),
                    ),
                    assign(9, binary(BinOp::Lt, moved(8), word(tcx, 64))),
                ],
                TerminatorKind::Assert {
                    cond: moved(9),
                    expected: true,
                    msg: Box::new(AssertMessage::Overflow(BinOp::Shl, copy(7), one_i32())),
                    target: BasicBlock::from_usize(3),
                    unwind: UnwindAction::Unreachable,
                },
            ),
            (
                vec![
                    assign(6, binary(BinOp::Shl, moved(7), one_i32())),
                    assign(5, binary(BinOp::BitOr, moved(6), one_usize())),
                    assign(4, Rvalue::Cast(CastKind::Transmute, moved(5), encoded)),
                    assign(0, Rvalue::Aggregate(aggregate, [moved(2), moved(4)].into())),
                ],
                TerminatorKind::Return,
            ),
        ],
    );
    replace(
        6,
        &[ptr, text, ptr_str],
        vec![(
            vec![
                assign(
                    2,
                    Rvalue::RawPtr(
                        rustc_middle::mir::RawPtrKind::Const,
                        Place {
                            local: Local::from_usize(1),
                            projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
                        },
                    ),
                ),
                assign(0, Rvalue::Cast(CastKind::PtrToPtr, moved(2), ptr)),
            ],
            TerminatorKind::Return,
        )],
    );
    replace(
        7,
        &[tcx.types.usize, text, bytes],
        vec![
            (vec![], call(instances[8], vec![copy(1)], 2, Some(1))),
            (
                vec![assign(0, Rvalue::UnaryOp(UnOp::PtrMetadata, copy(2)))],
                TerminatorKind::Return,
            ),
        ],
    );
    replace(
        8,
        &[bytes, text],
        vec![(
            vec![assign(0, Rvalue::Cast(CastKind::Transmute, copy(1), bytes))],
            TerminatorKind::Return,
        )],
    );
    for (&instance, source) in instances.iter().zip(&bodies) {
        assert_eq!(source.source.instance, instance.def);
    }
    exercise(tcx, root, &instances, &|instance| {
        &bodies[instances
            .iter()
            .position(|actual| *actual == instance)
            .expect("closed replay graph")]
    });
}
