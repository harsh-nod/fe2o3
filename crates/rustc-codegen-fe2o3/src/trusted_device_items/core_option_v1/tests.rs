use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{
    BasicBlockData, BinOp, BorrowKind, Const, ConstOperand, ConstValue, Local, SourceInfo,
    Statement, Terminator,
};
use rustc_span::DUMMY_SP;
use std::{fs, process::Command};

const SOURCE: &str = r#"
#![allow(dead_code)]
pub fn unwrap_integer(value: Option<usize>, default: usize) -> usize {
    value.unwrap_or(default)
}
pub fn unwrap_float(value: Option<f32>, default: f32) -> f32 {
    value.unwrap_or(default)
}
pub fn unwrap_reference<'a>(value: Option<&'a bool>, default: &'a bool) -> &'a bool {
    value.unwrap_or(default)
}
pub fn panic_path() -> usize { panic!("unreviewed panic") }
pub const REVIEW_FLAG: bool = false;
pub fn chain(value: Option<usize>, bias: &usize) -> Option<usize> {
    value.and_then(|x| Some(x.wrapping_add(*bias)))
}
pub fn chain_change_type(value: Option<u32>) -> Option<f32> {
    value.and_then(|x| Some(x as f32))
}
pub fn chain_pointer(value: Option<usize>, f: fn(usize) -> Option<usize>) -> Option<usize> {
    value.and_then(f)
}
pub struct Dropped(usize);
impl Drop for Dropped { fn drop(&mut self) {} }
pub fn unwrap_dropped(value: Option<Dropped>, default: Dropped) -> Dropped {
    value.unwrap_or(default)
}
pub fn chain_dropped(value: Option<usize>, guard: Dropped) -> Option<usize> {
    value.and_then(move |x| { drop(guard); Some(x) })
}
pub fn other_method(value: Option<usize>) -> Option<usize> {
    value.map(|x| x)
}
pub struct Impostor;
impl Impostor {
    fn unwrap_or(self, default: usize) -> usize { default }
    fn and_then(self, f: impl FnOnce(usize) -> Option<usize>) -> Option<usize> { f(0) }
}
pub fn impostor_unwrap(default: usize) -> usize { Impostor.unwrap_or(default) }
pub fn impostor_chain() -> Option<usize> { Impostor.and_then(|x| Some(x)) }
"#;

fn fixture_body<'tcx>(tcx: TyCtxt<'tcx>, caller: &str) -> &'tcx Body<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(*definition) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == caller
        })
        .expect("fixture function");
    tcx.instance_mir(Instance::mono(tcx, definition.to_def_id()).def)
}

fn helper<'tcx>(tcx: TyCtxt<'tcx>, caller: &str, method: &str) -> Instance<'tcx> {
    fixture_body(tcx, caller)
        .basic_blocks
        .iter()
        .find_map(|block| {
            let TerminatorKind::Call {
                func: Operand::Constant(callee),
                ..
            } = &block.terminator().kind
            else {
                return None;
            };
            let TyKind::FnDef(definition, arguments) = callee.const_.ty().kind() else {
                return None;
            };
            if tcx.item_name(*definition).as_str() != method {
                return None;
            }
            Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                *definition,
                arguments,
            )
            .expect("resolve fixture helper")
        })
        .expect("fixture helper call")
}

struct CheckCallbacks {
    mutations: bool,
    completed: bool,
}

impl Callbacks for CheckCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.mutations {
            check_mutations(tcx);
        } else {
            for (caller, method) in [
                ("unwrap_integer", "unwrap_or"),
                ("unwrap_float", "unwrap_or"),
                ("unwrap_reference", "unwrap_or"),
                ("chain", "and_then"),
                ("chain_change_type", "and_then"),
            ] {
                let instance = helper(tcx, caller, method);
                assert!(
                    authenticate_reviewed_safe_core_option_helper_v1(tcx, instance),
                    "real core helper rejected: {caller}: {:#?}",
                    tcx.instance_mir(instance.def)
                );
            }
            for (caller, method) in [
                ("impostor_unwrap", "unwrap_or"),
                ("impostor_chain", "and_then"),
                ("other_method", "map"),
                ("chain_pointer", "and_then"),
                ("chain_dropped", "and_then"),
                ("unwrap_dropped", "unwrap_or"),
            ] {
                assert!(
                    !authenticate_reviewed_safe_core_option_helper_v1(
                        tcx,
                        helper(tcx, caller, method)
                    ),
                    "unreviewed identity, dispatch, or drop admitted: {caller}"
                );
            }
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn check_mutations(tcx: TyCtxt<'_>) {
    let unwrap = helper(tcx, "unwrap_integer", "unwrap_or");
    let chain = helper(tcx, "chain", "and_then");
    for instance in [unwrap, chain] {
        let expected = contract(tcx, instance).expect("real Option contract");
        let body = tcx.instance_mir(instance.def);
        assert!(
            reviewed_body(tcx, instance, body, &expected),
            "positive MIR control"
        );

        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[BasicBlock::from_usize(0)]
            .terminator_mut()
            .kind = TerminatorKind::Goto {
            target: BasicBlock::from_usize(0),
        };
        assert!(
            !reviewed_body(tcx, instance, &changed, &expected),
            "cyclic control flow"
        );

        let mut changed = body.clone();
        changed.arg_count = 1;
        assert!(
            !reviewed_body(tcx, instance, &changed, &expected),
            "argument role mutation"
        );

        let mut changed = body.clone();
        while changed.local_decls.len() <= MAX_LOCALS {
            changed
                .local_decls
                .push(body.local_decls[Local::from_usize(0)].clone());
        }
        assert!(
            !reviewed_body(tcx, instance, &changed, &expected),
            "local work budget"
        );

        let mut changed = body.clone();
        for block in changed.basic_blocks.as_mut() {
            if matches!(block.terminator().kind, TerminatorKind::Return) {
                block.statements.push(Statement::new(
                    SourceInfo::outermost(DUMMY_SP),
                    StatementKind::StorageDead(Local::from_usize(0)),
                ));
            }
        }
        assert!(
            !reviewed_body(tcx, instance, &changed, &expected),
            "uninitialized return"
        );

        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[BasicBlock::from_usize(0)]
            .statements
            .push(Statement::new(
                SourceInfo::outermost(DUMMY_SP),
                StatementKind::Assign(Box::new((
                    Local::from_usize(0).into(),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Box::new((
                            Operand::Copy(Local::from_usize(0).into()),
                            Operand::Copy(Local::from_usize(0).into()),
                        )),
                    ),
                ))),
            ));
        assert!(
            !reviewed_body(tcx, instance, &changed, &expected),
            "extra arithmetic effect"
        );
    }

    let expected = contract(tcx, unwrap).unwrap();
    let mut changed = tcx.instance_mir(unwrap.def).clone();
    let mut replaced = false;
    for block in changed.basic_blocks.as_mut() {
        for statement in &mut block.statements {
            if let StatementKind::Assign(assignment) = &mut statement.kind {
                if let Rvalue::Use(Operand::Move(place) | Operand::Copy(place)) = &mut assignment.1
                {
                    if !place.projection.is_empty() {
                        *place = Local::from_usize(2).into();
                        replaced = true;
                    }
                }
            }
        }
    }
    assert!(replaced, "find Some payload in actual core MIR");
    assert!(
        !reviewed_body(tcx, unwrap, &changed, &expected),
        "default substituted for payload"
    );

    let expected = contract(tcx, chain).unwrap();
    let body = tcx.instance_mir(chain.def);
    let call = body
        .basic_blocks
        .iter_enumerated()
        .find_map(|(index, block)| {
            matches!(block.terminator().kind, TerminatorKind::Call { .. }).then_some(index)
        })
        .expect("real FnOnce call");
    let mut changed = body.clone();
    let TerminatorKind::Call { args, .. } =
        &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
    else {
        unreachable!()
    };
    args.swap(0, 1);
    assert!(
        !reviewed_body(tcx, chain, &changed, &expected),
        "callback and payload argument roles"
    );

    let wrong = helper(tcx, "impostor_chain", "and_then");
    let TyKind::FnDef(_, _) = tcx
        .type_of(wrong.def_id())
        .instantiate(tcx, wrong.args)
        .kind()
    else {
        panic!("function item fixture")
    };
    let mut changed = body.clone();
    let TerminatorKind::Call {
        func: Operand::Constant(callee),
        ..
    } = &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
    else {
        unreachable!()
    };
    callee.const_ = rustc_middle::mir::Const::Val(
        rustc_middle::mir::ConstValue::ZeroSized,
        Ty::new_fn_def(tcx, wrong.def_id(), wrong.args),
    );
    assert!(
        !reviewed_body(tcx, chain, &changed, &expected),
        "non-lang-item callback substitution"
    );

    let mut changed = body.clone();
    let TerminatorKind::Call { target, .. } =
        &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
    else {
        unreachable!()
    };
    *target = None;
    assert!(
        !reviewed_body(tcx, chain, &changed, &expected),
        "missing normal continuation"
    );

    check_dead_blocks(tcx);
    check_dead_callback_shapes(tcx, chain);
    check_drop_edges(tcx, unwrap);
}

fn append_block<'tcx>(
    body: &mut Body<'tcx>,
    kind: TerminatorKind<'tcx>,
    cleanup: bool,
) -> BasicBlock {
    body.basic_blocks.as_mut().push(BasicBlockData::new(
        Some(Terminator {
            source_info: SourceInfo::outermost(DUMMY_SP),
            kind,
        }),
        cleanup,
    ))
}

fn closed_shapes<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) -> bool {
    let contract = contract(tcx, instance).unwrap();
    let local_types = body
        .local_decls
        .iter()
        .map(|local| normalized_ty(tcx, instance, local.ty).unwrap())
        .collect();
    BodyCheck {
        tcx,
        instance,
        body,
        contract: &contract,
        local_types,
    }
    .closed_body_shapes()
}

fn check_dead_blocks<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "unwrap_reference", "unwrap_or");
    let expected = contract(tcx, instance).unwrap();
    let mut body = tcx.instance_mir(instance.def).clone();
    let normal = append_block(&mut body, TerminatorKind::Unreachable, false);
    let cleanup = append_block(&mut body, TerminatorKind::UnwindResume, true);
    let boolean = Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::from_bool(tcx, false),
    }));
    let dead = append_block(
        &mut body,
        TerminatorKind::SwitchInt {
            discr: boolean.clone(),
            targets: SwitchTargets::static_if(0, normal, normal),
        },
        false,
    );
    assert!(
        reviewed_body(tcx, instance, &body, &expected),
        "dead-block positive control"
    );
    let outside = BasicBlock::from_usize(body.basic_blocks.len());
    let invalid_local = Local::from_usize(body.local_decls.len());
    let dereference = Place {
        local: Local::from_usize(2),
        projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
    };
    let flag = tcx
        .iter_local_def_id()
        .find(|definition| {
            matches!(tcx.def_kind(*definition), DefKind::Const { .. })
                && tcx.item_name(definition.to_def_id()).as_str() == "REVIEW_FLAG"
        })
        .unwrap()
        .to_def_id();
    let reject = |kind: TerminatorKind<'tcx>, label| {
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[dead].terminator_mut().kind = kind;
        assert!(!closed_shapes(tcx, instance, &changed), "{label}");
        assert!(
            !reviewed_body(tcx, instance, &changed, &expected),
            "{label}"
        );
    };
    for target in [outside, cleanup, dead] {
        reject(TerminatorKind::Goto { target }, "invalid dead goto");
        for targets in [
            SwitchTargets::static_if(0, target, normal),
            SwitchTargets::static_if(0, normal, target),
        ] {
            reject(
                TerminatorKind::SwitchInt {
                    discr: boolean.clone(),
                    targets,
                },
                "invalid explicit or fallback switch target",
            );
        }
    }
    for operand in [
        Operand::Copy(dereference),
        Operand::Move(dereference),
        Operand::Copy(invalid_local.into()),
        Operand::Copy(Local::from_usize(1).into()),
        Operand::unevaluated_constant(tcx, flag, &[], DUMMY_SP),
        Operand::Constant(Box::new(ConstOperand {
            span: DUMMY_SP,
            user_ty: None,
            const_: Const::Val(ConstValue::ZeroSized, tcx.types.bool),
        })),
    ] {
        reject(
            TerminatorKind::SwitchInt {
                discr: operand,
                targets: SwitchTargets::static_if(0, normal, normal),
            },
            "unreviewed dead switch operand",
        );
    }
    for values in [vec![2], vec![0, 0], vec![0, 1, 2]] {
        reject(
            TerminatorKind::SwitchInt {
                discr: boolean.clone(),
                targets: SwitchTargets::new(
                    values.into_iter().map(|value| (value, normal)),
                    normal,
                ),
            },
            "invalid, duplicate, or excessive switch values",
        );
    }
    reject(TerminatorKind::UnwindResume, "resume outside cleanup");
    let panic = fixture_body(tcx, "panic_path")
        .basic_blocks
        .iter()
        .find(|block| matches!(block.terminator().kind, TerminatorKind::Call { .. }))
        .expect("actual panic call");
    reject(panic.terminator().kind.clone(), "panic in a dead block");

    for statement in [
        StatementKind::StorageLive(invalid_local),
        StatementKind::StorageDead(invalid_local),
        StatementKind::Assign(Box::new((
            Local::from_usize(0).into(),
            Rvalue::Use(Operand::Copy(Local::from_usize(1).into())),
        ))),
        StatementKind::Assign(Box::new((
            Local::from_usize(0).into(),
            Rvalue::Ref(tcx.lifetimes.re_erased, BorrowKind::Shared, dereference),
        ))),
    ] {
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[dead]
            .statements
            .push(Statement::new(SourceInfo::outermost(DUMMY_SP), statement));
        assert!(
            !closed_shapes(tcx, instance, &changed),
            "invalid dead statement"
        );
        assert!(!reviewed_body(tcx, instance, &changed, &expected));
    }
    for kind in [
        TerminatorKind::Return,
        TerminatorKind::Goto { target: normal },
    ] {
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[cleanup].terminator_mut().kind = kind;
        assert!(
            !closed_shapes(tcx, instance, &changed),
            "invalid cleanup exit"
        );
        assert!(!reviewed_body(tcx, instance, &changed, &expected));
    }
}

fn check_dead_callback_shapes<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
    let mut body = tcx.instance_mir(instance.def).clone();
    let call = body
        .basic_blocks
        .iter_enumerated()
        .find_map(|(index, block)| {
            matches!(block.terminator().kind, TerminatorKind::Call { .. }).then_some(index)
        })
        .unwrap();
    let original = body.basic_blocks[call].terminator().kind.clone();
    let TerminatorKind::Call {
        target: Some(normal),
        ..
    } = original
    else {
        unreachable!()
    };
    // Keep exactly one call site, but make it dead so path evaluation cannot
    // accidentally provide the structural rejection these cases exercise.
    body.basic_blocks.as_mut()[call].terminator_mut().kind =
        TerminatorKind::Goto { target: normal };
    let dead = append_block(&mut body, original, false);
    let cleanup = append_block(&mut body, TerminatorKind::UnwindResume, true);
    let outside = BasicBlock::from_usize(body.basic_blocks.len());
    let invalid_local = Local::from_usize(body.local_decls.len());
    assert!(
        closed_shapes(tcx, instance, &body),
        "dead call shape positive control"
    );

    #[derive(Debug)]
    enum Mutation {
        CalleeContents,
        ArgumentOrder,
        ArgumentCount,
        ArgumentProjection,
        ArgumentLocal,
        DestinationType,
        DestinationLocal,
        DestinationProjection,
        MissingTarget,
        TargetBounds,
        TargetCleanup,
        TargetCycle,
        UnwindBounds,
        UnwindNormal,
        UnwindTerminate,
        CleanupCall,
    }
    for mutation in [
        Mutation::CalleeContents,
        Mutation::ArgumentOrder,
        Mutation::ArgumentCount,
        Mutation::ArgumentProjection,
        Mutation::ArgumentLocal,
        Mutation::DestinationType,
        Mutation::DestinationLocal,
        Mutation::DestinationProjection,
        Mutation::MissingTarget,
        Mutation::TargetBounds,
        Mutation::TargetCleanup,
        Mutation::TargetCycle,
        Mutation::UnwindBounds,
        Mutation::UnwindNormal,
        Mutation::UnwindTerminate,
        Mutation::CleanupCall,
    ] {
        let mut changed = body.clone();
        let data = &mut changed.basic_blocks.as_mut()[dead];
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } = &mut data.terminator_mut().kind
        else {
            unreachable!()
        };
        match mutation {
            Mutation::CalleeContents => {
                let Operand::Constant(callee) = func else {
                    unreachable!()
                };
                callee.const_ = Const::Val(ConstValue::from_bool(false), callee.const_.ty());
            }
            Mutation::ArgumentOrder => args.swap(0, 1),
            Mutation::ArgumentCount => *args = Box::new([]),
            Mutation::ArgumentProjection => {
                args[0].node = Operand::Copy(Place {
                    local: Local::from_usize(2),
                    projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
                })
            }
            Mutation::ArgumentLocal => args[1].node = Operand::Copy(invalid_local.into()),
            Mutation::DestinationType => *destination = Local::from_usize(2).into(),
            Mutation::DestinationLocal => *destination = invalid_local.into(),
            Mutation::DestinationProjection => {
                destination.projection = tcx.mk_place_elems(&[ProjectionElem::Deref])
            }
            Mutation::MissingTarget => *target = None,
            Mutation::TargetBounds => *target = Some(outside),
            Mutation::TargetCleanup => *target = Some(cleanup),
            Mutation::TargetCycle => *target = Some(dead),
            Mutation::UnwindBounds => *unwind = UnwindAction::Cleanup(outside),
            Mutation::UnwindNormal => *unwind = UnwindAction::Cleanup(normal),
            Mutation::UnwindTerminate => {
                *unwind = UnwindAction::Terminate(UnwindTerminateReason::Abi)
            }
            Mutation::CleanupCall => data.is_cleanup = true,
        }
        assert!(!closed_shapes(tcx, instance, &changed), "{mutation:?}");
    }
}

fn check_drop_edges<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
    let expected = contract(tcx, instance).unwrap();
    let mut body = tcx.instance_mir(instance.def).clone();
    let normal = append_block(&mut body, TerminatorKind::Unreachable, false);
    let cleanup = append_block(&mut body, TerminatorKind::UnwindResume, true);
    let dead = append_block(
        &mut body,
        TerminatorKind::Drop {
            place: Local::from_usize(2).into(),
            target: normal,
            unwind: UnwindAction::Unreachable,
            replace: false,
            drop: None,
            async_fut: None,
        },
        false,
    );
    assert!(
        reviewed_body(tcx, instance, &body, &expected),
        "trivial dead drop control"
    );
    let outside = BasicBlock::from_usize(body.basic_blocks.len());
    let invalid_local = Local::from_usize(body.local_decls.len());
    #[derive(Debug)]
    enum Mutation {
        Place,
        Projection,
        Target,
        CleanupTarget,
        CycleTarget,
        Unwind,
        UnwindNormal,
        Terminate,
        AsyncDrop,
        AsyncFuture,
    }
    for mutation in [
        Mutation::Place,
        Mutation::Projection,
        Mutation::Target,
        Mutation::CleanupTarget,
        Mutation::CycleTarget,
        Mutation::Unwind,
        Mutation::UnwindNormal,
        Mutation::Terminate,
        Mutation::AsyncDrop,
        Mutation::AsyncFuture,
    ] {
        let mut changed = body.clone();
        let TerminatorKind::Drop {
            place,
            target,
            unwind,
            drop,
            async_fut,
            ..
        } = &mut changed.basic_blocks.as_mut()[dead].terminator_mut().kind
        else {
            unreachable!()
        };
        match mutation {
            Mutation::Place => *place = invalid_local.into(),
            Mutation::Projection => place.projection = tcx.mk_place_elems(&[ProjectionElem::Deref]),
            Mutation::Target => *target = outside,
            Mutation::CleanupTarget => *target = cleanup,
            Mutation::CycleTarget => *target = dead,
            Mutation::Unwind => *unwind = UnwindAction::Cleanup(outside),
            Mutation::UnwindNormal => *unwind = UnwindAction::Cleanup(normal),
            Mutation::Terminate => *unwind = UnwindAction::Terminate(UnwindTerminateReason::Abi),
            Mutation::AsyncDrop => *drop = Some(normal),
            Mutation::AsyncFuture => *async_fut = Some(Local::from_usize(2)),
        }
        assert!(!closed_shapes(tcx, instance, &changed), "{mutation:?}");
        assert!(
            !reviewed_body(tcx, instance, &changed, &expected),
            "{mutation:?}"
        );
    }
    let mut cleanup_body = body.clone();
    let data = &mut cleanup_body.basic_blocks.as_mut()[dead];
    data.is_cleanup = true;
    let TerminatorKind::Drop { target, unwind, .. } = &mut data.terminator_mut().kind else {
        unreachable!()
    };
    *target = cleanup;
    *unwind = UnwindAction::Terminate(UnwindTerminateReason::InCleanup);
    assert!(
        reviewed_body(tcx, instance, &cleanup_body, &expected),
        "trivial cleanup drop control"
    );
    for bad in [
        UnwindAction::Continue,
        UnwindAction::Cleanup(cleanup),
        UnwindAction::Terminate(UnwindTerminateReason::Abi),
    ] {
        let mut changed = cleanup_body.clone();
        let TerminatorKind::Drop { unwind, .. } =
            &mut changed.basic_blocks.as_mut()[dead].terminator_mut().kind
        else {
            unreachable!()
        };
        *unwind = bad;
        assert!(
            !closed_shapes(tcx, instance, &changed),
            "invalid cleanup drop unwind: {bad:?}"
        );
        assert!(!reviewed_body(tcx, instance, &changed, &expected));
    }
    let mut changed = body.clone();
    changed.local_decls[Local::from_usize(2)].ty =
        helper(tcx, "unwrap_dropped", "unwrap_or").args.type_at(0);
    assert!(
        !reviewed_body(tcx, instance, &changed, &expected),
        "nontrivial drop type"
    );
}

fn run_fixture(mutations: bool) {
    let directory = TestTempDir::create("fe2o3-core-option");
    let source = directory.path().join("fixture.rs");
    let output = directory.path().join("fixture.rmeta");
    fs::write(&source, SOURCE).expect("write Option compiler fixture");
    let mut command = Command::new("rustc");
    command.args(["--print", "sysroot"]);
    let sysroot = crate::process_execution::capture_output(&mut command).expect("rustc sysroot");
    assert!(sysroot.status.success());
    let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 sysroot");
    let args = vec![
        "rustc".to_owned(),
        "--crate-name".to_owned(),
        "fe2o3_core_option_fixture".to_owned(),
        "--crate-type=lib".to_owned(),
        "--edition=2024".to_owned(),
        "--emit=metadata".to_owned(),
        "-Zmir-opt-level=0".to_owned(),
        "-Coverflow-checks=off".to_owned(),
        "--sysroot".to_owned(),
        sysroot.trim().to_owned(),
        "-o".to_owned(),
        output.display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = CheckCallbacks {
        mutations,
        completed: false,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed, "compiler callback must execute");
}

#[test]
fn core_option_helpers_authenticate_real_core_and_reject_unreviewed_instances() {
    run_fixture(false);
}

#[test]
fn core_option_helpers_reject_mutated_actual_core_mir() {
    run_fixture(true);
}
