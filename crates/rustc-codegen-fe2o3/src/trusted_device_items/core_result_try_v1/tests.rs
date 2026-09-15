use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{BasicBlockData, ConstOperand, Local, SourceInfo, Statement, Terminator};
use rustc_span::DUMMY_SP;
use std::{fs, process::Command};

const SOURCE: &str = r#"
#![feature(try_trait_v2)]
#![allow(dead_code)]
use std::{convert::Infallible, ops::{ControlFlow, FromResidual, Try}};
macro_rules! residual {
    ($name:ident, $t:ty, $e:ty, $f:ty) => {
        pub fn $name(x: Result<Infallible, $e>) -> Result<$t, $f> {
            FromResidual::from_residual(x)
        }
    }
}
residual!(r01, (), u8, u16);
residual!(r02, u8, u16, u32);
residual!(r03, u16, u32, u64);
residual!(r04, u32, u64, u128);
residual!(r05, u64, i8, i16);
residual!(r06, f32, i16, i32);
residual!(r07, f64, i32, i64);
residual!(r08, [u32; 4], usize, usize);
residual!(r09, (u8, bool), (), ());
residual!(r10, &'static u32, &'static bool, &'static bool);
pub struct Error(pub u32);
pub struct Converted(pub u32);
impl From<Error> for Converted { fn from(e: Error) -> Self { Self(e.0 + 1) } }
residual!(r11, Option<u32>, Error, Converted);
pub trait Associated { type Type; }
impl Associated for Error { type Type = u32; }
residual!(r12, <Error as Associated>::Type, u32, u32);
pub fn branch(x: Option<u32>) -> ControlFlow<Option<Infallible>, u32> { Try::branch(x) }
pub fn branch_ref(x: Option<&'static u32>) -> ControlFlow<Option<Infallible>, &'static u32> { Try::branch(x) }
pub fn branch_noncopy(x: Option<Error>) -> ControlFlow<Option<Infallible>, Error> { Try::branch(x) }
pub fn branch_unit(x: Option<()>) -> ControlFlow<Option<Infallible>, ()> { Try::branch(x) }
pub struct Dropped;
impl Drop for Dropped { fn drop(&mut self) {} }
residual!(drop_output, Dropped, u32, u32);
residual!(drop_error, u32, Dropped, Dropped);
pub fn branch_drop(x: Option<Dropped>) -> ControlFlow<Option<Infallible>, Dropped> { Try::branch(x) }
pub fn option_residual(x: Option<Infallible>) -> Option<u32> { FromResidual::from_residual(x) }
pub fn option_residual_ref(x: Option<Infallible>) -> Option<&'static u32> { FromResidual::from_residual(x) }
pub fn option_residual_noncopy(x: Option<Infallible>) -> Option<Error> { FromResidual::from_residual(x) }
pub fn option_residual_unit(x: Option<Infallible>) -> Option<()> { FromResidual::from_residual(x) }
pub fn option_residual_never(x: Option<Infallible>) -> Option<Infallible> { FromResidual::from_residual(x) }
pub fn option_residual_drop(x: Option<Infallible>) -> Option<Dropped> { FromResidual::from_residual(x) }
pub fn ok_or(x: Option<u32>, e: u32) -> Result<u32, u32> { x.ok_or(e) }
pub fn ok_or_ref(x: Option<&'static u32>, e: &'static u32) -> Result<&'static u32, &'static u32> { x.ok_or(e) }
pub fn ok_or_noncopy(x: Option<Error>, e: Converted) -> Result<Error, Converted> { x.ok_or(e) }
pub fn ok_or_unit(x: Option<()>, e: ()) -> Result<(), ()> { x.ok_or(e) }
pub fn ok_or_associated(x: Option<<Error as Associated>::Type>, e: u8) -> Result<u32, u8> { x.ok_or(e) }
pub fn ok_or_drop_payload(x: Option<Dropped>, e: u32) -> Result<Dropped, u32> { x.ok_or(e) }
pub fn ok_or_drop_error(x: Option<u32>, e: Dropped) -> Result<u32, Dropped> { x.ok_or(e) }
pub fn ok_or_else(x: Option<u32>) -> Result<u32, u32> { x.ok_or_else(|| 0) }
pub fn result_branch(x: Result<u32, u32>) -> ControlFlow<Result<Infallible, u32>, u32> { Try::branch(x) }
pub fn result_branch_ref(x: Result<&'static u32, &'static u32>) -> ControlFlow<Result<Infallible, &'static u32>, &'static u32> { Try::branch(x) }
pub fn result_branch_noncopy(x: Result<Error, Converted>) -> ControlFlow<Result<Infallible, Converted>, Error> { Try::branch(x) }
pub fn result_branch_unit(x: Result<(), ()>) -> ControlFlow<Result<Infallible, ()>, ()> { Try::branch(x) }
pub fn result_branch_drop_output(x: Result<Dropped, u32>) -> ControlFlow<Result<Infallible, u32>, Dropped> { Try::branch(x) }
pub fn result_branch_drop_error(x: Result<u32, Dropped>) -> ControlFlow<Result<Infallible, Dropped>, u32> { Try::branch(x) }
pub fn result_from_output(x: u32) -> Result<u32, u32> { Try::from_output(x) }
pub fn from_output(x: u32) -> Option<u32> { Try::from_output(x) }
pub struct Impostor;
impl Impostor {
    pub fn branch(self) -> ControlFlow<Option<Infallible>, u32> { ControlFlow::Continue(0) }
    pub fn from_residual(_: Result<Infallible, u32>) -> Result<u32, u32> { Ok(0) }
    pub fn ok_or(self, _: u32) -> Result<u32, u32> { Ok(0) }
}
pub fn impostor_branch() -> ControlFlow<Option<Infallible>, u32> { Impostor.branch() }
pub fn impostor_residual(x: Result<Infallible, u32>) -> Result<u32, u32> { Impostor::from_residual(x) }
pub fn impostor_ok_or() -> Result<u32, u32> { Impostor.ok_or(0) }
pub struct Foreign;
impl FromResidual<Result<Infallible, u32>> for Foreign {
    fn from_residual(_: Result<Infallible, u32>) -> Self { Self }
}
impl Try for Foreign {
    type Output = u32;
    type Residual = Result<Infallible, u32>;
    fn from_output(_: u32) -> Self { Self }
    fn branch(self) -> ControlFlow<Self::Residual, u32> { ControlFlow::Continue(0) }
}
pub fn foreign_branch(x: Foreign) -> ControlFlow<Result<Infallible, u32>, u32> { Try::branch(x) }
pub fn foreign_residual(x: Result<Infallible, u32>) -> Foreign { FromResidual::from_residual(x) }
pub struct Trapping;
impl From<Error> for Trapping { fn from(_: Error) -> Self { panic!("unreviewed conversion") } }
residual!(unreviewed_conversion, u32, Error, Trapping);
pub fn panic_path() { panic!("unreviewed call") }
pub unsafe fn unsafe_from(_: u32) -> u32 { 0 }
pub fn ordinary_from(x: u32) -> u32 { x }
pub const FLAG: bool = false;
"#;

fn fixture_body<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> &'tcx Body<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
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
            let TyKind::FnDef(definition, args) = callee.const_.ty().kind() else {
                return None;
            };
            if tcx.item_name(*definition).as_str() != method {
                return None;
            }
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
                .expect("fixture call resolves")
        })
        .expect("fixture helper")
}

fn check_instances(tcx: TyCtxt<'_>) {
    for caller in [
        "r01",
        "r02",
        "r03",
        "r04",
        "r05",
        "r06",
        "r07",
        "r08",
        "r09",
        "r10",
        "r11",
        "r12",
        "branch",
        "branch_ref",
        "branch_noncopy",
        "branch_unit",
        "result_branch",
        "result_branch_ref",
        "result_branch_noncopy",
        "result_branch_unit",
        "unreviewed_conversion",
        "option_residual",
        "option_residual_ref",
        "option_residual_noncopy",
        "option_residual_unit",
        "option_residual_never",
        "ok_or",
        "ok_or_ref",
        "ok_or_noncopy",
        "ok_or_unit",
        "ok_or_associated",
    ] {
        let method = if caller.contains("branch") {
            "branch"
        } else if caller.starts_with("ok_or") {
            "ok_or"
        } else {
            "from_residual"
        };
        let instance = helper(tcx, caller, method);
        assert!(
            contract(tcx, instance).is_some(),
            "real contract: {caller}: {instance:?}"
        );
        assert!(
            authenticate_reviewed_safe_core_result_try_helper_v1(tcx, instance),
            "actual core body: {caller}: {:#?}",
            tcx.instance_mir(instance.def)
        );
    }
    for (caller, method) in [
        ("drop_output", "from_residual"),
        ("drop_error", "from_residual"),
        ("branch_drop", "branch"),
        ("option_residual_drop", "from_residual"),
        ("ok_or_drop_payload", "ok_or"),
        ("ok_or_drop_error", "ok_or"),
        ("ok_or_else", "ok_or_else"),
        ("impostor_ok_or", "ok_or"),
        ("result_branch_drop_output", "branch"),
        ("result_branch_drop_error", "branch"),
        ("result_from_output", "from_output"),
        ("from_output", "from_output"),
        ("impostor_branch", "branch"),
        ("impostor_residual", "from_residual"),
        ("foreign_residual", "from_residual"),
        ("foreign_branch", "branch"),
    ] {
        assert!(
            !authenticate_reviewed_safe_core_result_try_helper_v1(tcx, helper(tcx, caller, method)),
            "unreviewed instance: {caller}"
        );
    }
    for caller in ["r11", "unreviewed_conversion"] {
        let instance = helper(tcx, caller, "from_residual");
        let contract = contract(tcx, instance).unwrap();
        let (_, conversion) = contract.conversion.unwrap();
        assert!(
            conversion.def_id().is_local(),
            "real user From implementation"
        );
        assert!(
            !authenticate_reviewed_safe_core_result_try_helper_v1(tcx, conversion),
            "wrapper must not grant source trust to From"
        );
        assert_eq!(
            tcx.instance_mir(instance.def)
                .basic_blocks
                .iter()
                .filter(|b| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
                .count(),
            1,
            "conversion remains an ordinary recursive call"
        );
    }
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

fn statement<'tcx>(kind: StatementKind<'tcx>) -> Statement<'tcx> {
    Statement::new(SourceInfo::outermost(DUMMY_SP), kind)
}

fn check_mutations<'tcx>(tcx: TyCtxt<'tcx>) {
    for (caller, method) in [
        ("r12", "from_residual"),
        ("branch", "branch"),
        ("result_branch", "branch"),
        ("ok_or", "ok_or"),
        ("option_residual", "from_residual"),
    ] {
        let instance = helper(tcx, caller, method);
        let contract = contract(tcx, instance).unwrap();
        let body = tcx.instance_mir(instance.def);
        let reject = |changed: &Body<'tcx>, label| {
            assert!(
                !reviewed_body(tcx, instance, changed, &contract),
                "{caller}: {label}"
            )
        };
        assert!(reviewed_body(tcx, instance, body, &contract));
        let mut changed = body.clone();
        changed.arg_count += 1;
        reject(&changed, "argument count");
        let mut changed = body.clone();
        changed.local_decls[Local::from_usize(0)].ty = tcx.types.u8;
        reject(&changed, "return type");
        let mut changed = body.clone();
        while changed.local_decls.len() <= MAX_LOCALS {
            changed
                .local_decls
                .push(body.local_decls[Local::from_usize(0)].clone());
        }
        reject(&changed, "local budget");
        let mut changed = body.clone();
        while changed.basic_blocks.len() <= MAX_BLOCKS {
            append_block(&mut changed, TerminatorKind::Unreachable, false);
        }
        reject(&changed, "block budget");
        let mut changed = body.clone();
        while changed.source_scopes.len() <= MAX_SCOPES {
            changed.source_scopes.push(
                body.source_scopes[body.local_decls[Local::from_usize(0)].source_info.scope]
                    .clone(),
            );
        }
        reject(&changed, "scope budget");
        let mut changed = body.clone();
        for _ in 0..=MAX_STATEMENTS {
            changed.basic_blocks.as_mut()[BasicBlock::from_usize(0)]
                .statements
                .push(statement(StatementKind::Nop));
        }
        reject(&changed, "statement budget");
        for target in [
            BasicBlock::from_usize(0),
            BasicBlock::from_usize(body.basic_blocks.len()),
        ] {
            let mut changed = body.clone();
            changed.basic_blocks.as_mut()[BasicBlock::from_usize(0)]
                .terminator_mut()
                .kind = TerminatorKind::Goto { target };
            reject(&changed, "cycle or missing target");
        }
        let mut changed = body.clone();
        for block in changed.basic_blocks.as_mut() {
            if matches!(block.terminator().kind, TerminatorKind::Return) {
                block
                    .statements
                    .push(statement(StatementKind::StorageDead(Local::from_usize(0))));
            }
        }
        reject(&changed, "uninitialized return");
        if contract.operation != Operation::OptionFromResidual {
            let mut changed = body.clone();
            let mut found = false;
            for block in changed.basic_blocks.as_mut() {
                for stmt in &mut block.statements {
                    if let StatementKind::Assign(assignment) = &mut stmt.kind {
                        if let Rvalue::Use(Operand::Move(place)) = &mut assignment.1 {
                            if !place.projection.is_empty() {
                                place.projection = tcx.mk_place_elems(&[
                                    ProjectionElem::Downcast(
                                        None,
                                        VariantIdx::from_usize(
                                            1 - contract.input_payload_variant.as_usize(),
                                        ),
                                    ),
                                    ProjectionElem::Field(
                                        rustc_abi::FieldIdx::from_usize(0),
                                        contract.payload,
                                    ),
                                ]);
                                found = true;
                            }
                        }
                    }
                }
            }
            assert!(found);
            reject(&changed, "wrong input variant");
            let mut changed = body.clone();
            let mut found = false;
            for block in changed.basic_blocks.as_mut() {
                if let Some(position) = block.statements.iter().position(|stmt| matches!(&stmt.kind, StatementKind::Assign(a) if matches!(&a.1, Rvalue::Use(Operand::Move(p)) if !p.projection.is_empty()))) {
                block.statements.insert(position + 1, block.statements[position].clone());
                found = true;
            }
            }
            assert!(found);
            reject(&changed, "double move of payload");
        }
        for cleanup in [false, true] {
            let mut changed = body.clone();
            let sink = append_block(
                &mut changed,
                if cleanup {
                    TerminatorKind::UnwindResume
                } else {
                    TerminatorKind::Unreachable
                },
                cleanup,
            );
            assert!(
                reviewed_body(tcx, instance, &changed, &contract),
                "empty sink control"
            );
            changed.basic_blocks.as_mut()[sink]
                .statements
                .push(statement(StatementKind::Assign(Box::new((
                    Local::from_usize(0).into(),
                    Rvalue::Use(Operand::Move(Local::from_usize(0).into())),
                )))));
            reject(&changed, "dead uninitialized move");
            let mut changed = body.clone();
            let sink = append_block(
                &mut changed,
                TerminatorKind::Goto {
                    target: BasicBlock::from_usize(0),
                },
                cleanup,
            );
            changed.basic_blocks.as_mut()[sink].terminator_mut().kind =
                TerminatorKind::Goto { target: sink };
            reject(&changed, "dead cycle");
        }
    }
    check_result_edges(tcx);
    check_option_forwarding(tcx);
    check_result_branch_forwarding(tcx);
    check_option_residual_and_ok_or_forwarding(tcx);
    check_dead_shapes(tcx);
    check_trivial_drops(tcx);
}

fn check_option_residual_and_ok_or_forwarding<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "option_residual", "from_residual");
    let contract = contract(tcx, instance).unwrap();
    let body = tcx.instance_mir(instance.def);
    for mutation in 0..5 {
        let mut changed = body.clone();
        let mut found = false;
        for block in changed.basic_blocks.as_mut() {
            for stmt in &mut block.statements {
                match &mut stmt.kind {
                    StatementKind::Intrinsic(intrinsic) if mutation <= 1 => {
                        **intrinsic = NonDivergingIntrinsic::Assume(boolean(tcx, mutation == 1));
                        found = true;
                    }
                    StatementKind::Assign(a) => match &mut a.1 {
                        Rvalue::Aggregate(kind, _) if mutation == 2 => {
                            let AggregateKind::Adt(_, variant, _, _, _) = &mut **kind else {
                                unreachable!()
                            };
                            *variant = contract.output_payload_variant;
                            found = true;
                        }
                        Rvalue::BinaryOp(operation, _) if mutation == 3 => {
                            *operation = BinOp::Ne;
                            found = true;
                        }
                        Rvalue::Discriminant(place) if mutation == 4 => {
                            *place = Local::from_usize(0).into();
                            found = true;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        assert!(found, "Option residual mutation applied");
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "Option residual {mutation}"
        );
    }

    let instance = helper(tcx, "ok_or", "ok_or");
    let contract = super::contract(tcx, instance).unwrap();
    let body = tcx.instance_mir(instance.def);
    assert!(contract.conversion.is_none());
    assert_eq!(
        contract.error,
        Some(contract.payload),
        "equal types, distinct symbols"
    );
    for mutation in 0..4 {
        let mut changed = body.clone();
        let mut found = false;
        for block in changed.basic_blocks.as_mut() {
            for stmt in &mut block.statements {
                if let StatementKind::Assign(a) = &mut stmt.kind {
                    if let Rvalue::Aggregate(kind, operands) = &mut a.1 {
                        let AggregateKind::Adt(_, variant, _, _, _) = &mut **kind else {
                            continue;
                        };
                        if mutation == 0 {
                            *variant = if *variant == contract.output_payload_variant {
                                contract.output_break_variant
                            } else {
                                contract.output_payload_variant
                            };
                            found = true;
                        } else if *variant == contract.output_payload_variant && mutation <= 2 {
                            operands.raw[0] = if mutation == 1 {
                                Operand::Copy(Local::from_usize(2).into())
                            } else {
                                Operand::Copy(Local::from_usize(0).into())
                            };
                            found = true;
                        } else if *variant == contract.output_break_variant && mutation == 3 {
                            operands.raw[0] = Operand::Copy(Place {
                                local: Local::from_usize(1),
                                projection: tcx.mk_place_elems(&[
                                    ProjectionElem::Downcast(None, contract.input_payload_variant),
                                    ProjectionElem::Field(
                                        rustc_abi::FieldIdx::from_usize(0),
                                        contract.payload,
                                    ),
                                ]),
                            });
                            found = true;
                        }
                    }
                }
            }
        }
        assert!(found, "ok_or mutation applied");
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "ok_or {mutation}"
        );
    }
    // Neither wrapper may acquire a new conversion or trap, even in dead cleanup.
    for (caller, method) in [("ok_or", "ok_or"), ("option_residual", "from_residual")] {
        let instance = helper(tcx, caller, method);
        let contract = super::contract(tcx, instance).unwrap();
        let mut changed = tcx.instance_mir(instance.def).clone();
        let panic_call = fixture_body(tcx, "panic_path")
            .basic_blocks
            .iter()
            .find(|block| matches!(block.terminator().kind, TerminatorKind::Call { .. }))
            .unwrap()
            .terminator()
            .kind
            .clone();
        append_block(&mut changed, panic_call, true);
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "dead helper/trap"
        );
    }
}

fn check_trivial_drops<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "branch", "branch");
    let contract = contract(tcx, instance).unwrap();
    let mut body = tcx.instance_mir(instance.def).clone();
    let TerminatorKind::SwitchInt { targets, .. } = &body.basic_blocks[BasicBlock::from_usize(0)]
        .terminator()
        .kind
    else {
        panic!("Option switch")
    };
    let some = targets.target_for_value(contract.payload_discriminant);
    let TerminatorKind::Goto { target } = body.basic_blocks[some].terminator().kind else {
        panic!("Some continuation")
    };
    let payload = Local::from_usize(3);
    body.basic_blocks.as_mut()[some].terminator_mut().kind = TerminatorKind::Drop {
        place: payload.into(),
        target,
        unwind: UnwindAction::Unreachable,
        replace: false,
        drop: None,
        async_fut: None,
    };
    assert!(
        reviewed_body(tcx, instance, &body, &contract),
        "trivial live payload drop control"
    );
    let reject = |body: &Body<'tcx>, label| {
        assert!(
            !reviewed_body(tcx, instance, body, &contract),
            "drop: {label}"
        )
    };
    for place_value in [
        Local::from_usize(body.local_decls.len()).into(),
        Local::from_usize(1).into(),
        Local::from_usize(0).into(),
        Place {
            local: payload,
            projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
        },
    ] {
        let mut changed = body.clone();
        let TerminatorKind::Drop { place, .. } =
            &mut changed.basic_blocks.as_mut()[some].terminator_mut().kind
        else {
            unreachable!()
        };
        *place = place_value;
        reject(
            &changed,
            "invalid, partially moved, return, or projected place",
        );
    }
    for bad_target in [some, BasicBlock::from_usize(body.basic_blocks.len())] {
        let mut changed = body.clone();
        let TerminatorKind::Drop { target, .. } =
            &mut changed.basic_blocks.as_mut()[some].terminator_mut().kind
        else {
            unreachable!()
        };
        *target = bad_target;
        reject(&changed, "cycle or out-of-bounds target");
    }
    for bad_unwind in [
        UnwindAction::Cleanup(target),
        UnwindAction::Cleanup(BasicBlock::from_usize(body.basic_blocks.len())),
        UnwindAction::Terminate(UnwindTerminateReason::Abi),
        UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
    ] {
        let mut changed = body.clone();
        let TerminatorKind::Drop { unwind, .. } =
            &mut changed.basic_blocks.as_mut()[some].terminator_mut().kind
        else {
            unreachable!()
        };
        *unwind = bad_unwind;
        reject(&changed, "invalid unwind target or termination");
    }
    for future in [false, true] {
        let mut changed = body.clone();
        let TerminatorKind::Drop {
            drop, async_fut, ..
        } = &mut changed.basic_blocks.as_mut()[some].terminator_mut().kind
        else {
            unreachable!()
        };
        if future {
            *async_fut = Some(payload);
        } else {
            *drop = Some(target);
        }
        reject(&changed, "async drop form");
    }
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[target]
        .statements
        .push(statement(StatementKind::Assign(Box::new((
            payload.into(),
            Rvalue::Use(Operand::Copy(payload.into())),
        )))));
    reject(&changed, "read after trivial drop");

    let instance = helper(tcx, "r12", "from_residual");
    let contract = super::contract(tcx, instance).unwrap();
    let mut body = tcx.instance_mir(instance.def).clone();
    let resume = append_block(&mut body, TerminatorKind::UnwindResume, true);
    let cleanup = append_block(
        &mut body,
        TerminatorKind::Drop {
            place: payload.into(),
            target: resume,
            unwind: UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
            replace: false,
            drop: None,
            async_fut: None,
        },
        true,
    );
    let TerminatorKind::Call { args, unwind, .. } = &mut body.basic_blocks.as_mut()
        [BasicBlock::from_usize(0)]
    .terminator_mut()
    .kind
    else {
        panic!("From call")
    };
    args[0].node = Operand::Copy(payload.into());
    *unwind = UnwindAction::Cleanup(cleanup);
    assert!(
        reviewed_body(tcx, instance, &body, &contract),
        "trivial cleanup drop control"
    );
    for bad in [
        UnwindAction::Continue,
        UnwindAction::Cleanup(resume),
        UnwindAction::Terminate(UnwindTerminateReason::Abi),
    ] {
        let mut changed = body.clone();
        let TerminatorKind::Drop { unwind, .. } =
            &mut changed.basic_blocks.as_mut()[cleanup].terminator_mut().kind
        else {
            unreachable!()
        };
        *unwind = bad;
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "cleanup drop unwind context"
        );
    }
}

fn boolean<'tcx>(tcx: TyCtxt<'tcx>, value: bool) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::from_bool(tcx, value),
    }))
}

fn local_function<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Operand<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .unwrap();
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::Val(
            ConstValue::ZeroSized,
            Ty::new_fn_def(tcx, definition.to_def_id(), tcx.mk_args(&[])),
        ),
    }))
}

fn check_result_edges<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "r12", "from_residual");
    let contract = contract(tcx, instance).unwrap();
    let body = tcx.instance_mir(instance.def);
    let reject = |body: &Body<'tcx>, label| {
        assert!(
            !reviewed_body(tcx, instance, body, &contract),
            "Result: {label}"
        )
    };
    let call = body
        .basic_blocks
        .iter_enumerated()
        .find(|(_, b)| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
        .unwrap()
        .0;
    let outside = BasicBlock::from_usize(body.basic_blocks.len());
    let bad_local = Local::from_usize(body.local_decls.len());
    let source_info = SourceInfo::outermost(DUMMY_SP);
    for name in ["ordinary_from", "unsafe_from", "panic_path"] {
        let mut changed = body.clone();
        let TerminatorKind::Call { func, .. } =
            &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        *func = local_function(tcx, name);
        reject(&changed, "non-From function identity");
    }
    let mut changed = body.clone();
    let TerminatorKind::Call {
        func: Operand::Constant(callee),
        ..
    } = &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
    else {
        unreachable!()
    };
    let method = contract.conversion.unwrap().0;
    callee.const_ = Const::Val(
        ConstValue::ZeroSized,
        Ty::new_fn_def(tcx, method, [tcx.types.u64, tcx.types.u32]),
    );
    reject(&changed, "different From specialization");
    for target_value in [None, Some(outside), Some(call)] {
        let mut changed = body.clone();
        let TerminatorKind::Call { target, .. } =
            &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        *target = target_value;
        reject(&changed, "missing, outside, or cyclic call target");
    }
    for bad in [
        UnwindAction::Unreachable,
        UnwindAction::Cleanup(outside),
        UnwindAction::Cleanup(call),
        UnwindAction::Terminate(UnwindTerminateReason::Abi),
        UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
    ] {
        let mut changed = body.clone();
        let TerminatorKind::Call { unwind, .. } =
            &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        *unwind = bad;
        reject(&changed, "unreviewed conversion unwind");
    }
    for destination_value in [
        bad_local.into(),
        Local::from_usize(0).into(),
        Place {
            local: Local::from_usize(1),
            projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
        },
    ] {
        let mut changed = body.clone();
        let TerminatorKind::Call { destination, .. } =
            &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        *destination = destination_value;
        reject(&changed, "invalid conversion destination");
    }
    for operand in [
        Operand::Move(bad_local.into()),
        Operand::Copy(Local::from_usize(0).into()),
        Operand::Move(Local::from_usize(4).into()),
        boolean(tcx, false),
    ] {
        let mut changed = body.clone();
        let TerminatorKind::Call { args, .. } =
            &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        args[0].node = operand;
        reject(&changed, "invalid conversion input");
    }
    let mut changed = body.clone();
    let TerminatorKind::Call { args, .. } =
        &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
    else {
        unreachable!()
    };
    *args = Box::new([]);
    reject(&changed, "conversion argument count");
    let mut changed = body.clone();
    let duplicate = body.basic_blocks[call].terminator().kind.clone();
    append_block(&mut changed, duplicate, false);
    reject(&changed, "dead duplicate conversion");

    let mut changed = body.clone();
    let cleanup = append_block(&mut changed, TerminatorKind::UnwindResume, true);
    let TerminatorKind::Call { unwind, .. } =
        &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
    else {
        unreachable!()
    };
    *unwind = UnwindAction::Cleanup(cleanup);
    assert!(
        reviewed_body(tcx, instance, &changed, &contract),
        "conversion cleanup control"
    );
    for kind in [
        TerminatorKind::Return,
        TerminatorKind::Unreachable,
        TerminatorKind::UnwindTerminate(UnwindTerminateReason::InCleanup),
        TerminatorKind::Goto { target: call },
    ] {
        let mut bad = changed.clone();
        bad.basic_blocks.as_mut()[cleanup].terminator_mut().kind = kind;
        reject(&bad, "cleanup must resume without traps");
    }
    let mut bad = changed.clone();
    bad.basic_blocks.as_mut()[cleanup]
        .statements
        .push(statement(StatementKind::Assign(Box::new((
            Local::from_usize(3).into(),
            Rvalue::Use(Operand::Move(Local::from_usize(3).into())),
        )))));
    reject(&bad, "cleanup reads moved error");
    let mut bad = changed.clone();
    bad.basic_blocks.as_mut()[cleanup]
        .statements
        .push(statement(StatementKind::Assign(Box::new((
            Local::from_usize(3).into(),
            Rvalue::Use(Operand::Move(Local::from_usize(4).into())),
        )))));
    reject(&bad, "cleanup reads uninitialized conversion result");

    for replacement in [
        boolean(tcx, false),
        boolean(tcx, true),
        Operand::Copy(bad_local.into()),
    ] {
        let mut changed = body.clone();
        let mut found = false;
        for block in changed.basic_blocks.as_mut() {
            for stmt in &mut block.statements {
                if let StatementKind::Intrinsic(intrinsic) = &mut stmt.kind {
                    if let NonDivergingIntrinsic::Assume(operand) = &mut **intrinsic {
                        *operand = replacement.clone();
                        found = true;
                    }
                }
            }
        }
        assert!(found, "real discriminant assumption");
        reject(&changed, "assumption needs proven Err provenance");
    }
    let mut changed = body.clone();
    let mut found = false;
    for block in changed.basic_blocks.as_mut() {
        for stmt in &mut block.statements {
            if let StatementKind::Assign(assignment) = &mut stmt.kind {
                if let Rvalue::BinaryOp(operator, _) = &mut assignment.1 {
                    *operator = BinOp::Ne;
                    found = true;
                }
            }
        }
    }
    assert!(found);
    reject(&changed, "inverted discriminant assumption");

    for converted_result in [false, true] {
        let mut changed = body.clone();
        let mut found = false;
        for block in changed.basic_blocks.as_mut() {
            for stmt in &mut block.statements {
                if let StatementKind::Assign(assignment) = &mut stmt.kind {
                    if let Rvalue::Aggregate(kind, operands) = &mut assignment.1 {
                        if converted_result {
                            operands.raw[0] = Operand::Copy(Local::from_usize(3).into());
                        } else if let AggregateKind::Adt(_, variant, ..) = &mut **kind {
                            *variant = contract.output_break_variant;
                        }
                        found = true;
                    }
                }
            }
        }
        assert!(found);
        reject(&changed, "Err must wrap converted error");
    }
    // Keep the original error live to distinguish provenance from move checking.
    let mut changed = body.clone();
    let TerminatorKind::Call { args, .. } =
        &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
    else {
        unreachable!()
    };
    let Operand::Move(error) = args[0].node else {
        panic!("real error move")
    };
    args[0].node = Operand::Copy(error);
    for block in changed.basic_blocks.as_mut() {
        for stmt in &mut block.statements {
            if let StatementKind::Assign(assignment) = &mut stmt.kind {
                if let Rvalue::Aggregate(_, operands) = &mut assignment.1 {
                    operands.raw[0] = Operand::Copy(error);
                }
            }
        }
    }
    reject(
        &changed,
        "original error cannot replace From output even when E equals F",
    );
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[call].terminator = Some(Terminator {
        source_info,
        kind: TerminatorKind::Unreachable,
    });
    reject(&changed, "cannot terminalize residual conversion");
}

fn check_option_forwarding<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "branch", "branch");
    let contract = contract(tcx, instance).unwrap();
    let body = tcx.instance_mir(instance.def);
    let reject = |body: &Body<'tcx>, label| {
        assert!(
            !reviewed_body(tcx, instance, body, &contract),
            "Option: {label}"
        )
    };
    let switch = body
        .basic_blocks
        .iter_enumerated()
        .find(|(_, b)| matches!(b.terminator().kind, TerminatorKind::SwitchInt { .. }))
        .unwrap()
        .0;
    let TerminatorKind::SwitchInt { discr, targets } = &body.basic_blocks[switch].terminator().kind
    else {
        unreachable!()
    };
    let some = targets.target_for_value(contract.payload_discriminant);
    let none = targets.target_for_value(contract.empty_discriminant);
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[switch].terminator_mut().kind = TerminatorKind::SwitchInt {
        discr: discr.clone(),
        targets: SwitchTargets::new(
            [
                (contract.payload_discriminant, none),
                (contract.empty_discriminant, some),
            ]
            .into_iter(),
            targets.otherwise(),
        ),
    };
    reject(&changed, "swapped variant paths");
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[switch].terminator_mut().kind =
        TerminatorKind::Goto { target: some };
    reject(&changed, "Some path forced for None");
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[switch].terminator_mut().kind =
        TerminatorKind::Goto { target: none };
    reject(&changed, "None path forced for Some");
    for bad_operand in [
        Operand::Copy(Local::from_usize(0).into()),
        Operand::Move(Local::from_usize(1).into()),
        boolean(tcx, false),
    ] {
        let mut changed = body.clone();
        for stmt in &mut changed.basic_blocks.as_mut()[some].statements {
            if let StatementKind::Assign(assignment) = &mut stmt.kind {
                if let Rvalue::Aggregate(_, operands) = &mut assignment.1 {
                    operands.raw[0] = bad_operand.clone();
                }
            }
        }
        reject(&changed, "Continue must forward payload");
    }
    let mut changed = body.clone();
    for stmt in &mut changed.basic_blocks.as_mut()[none].statements {
        if let StatementKind::Assign(assignment) = &mut stmt.kind {
            if let Rvalue::Aggregate(_, operands) = &mut assignment.1 {
                let Operand::Constant(constant) = &mut operands.raw[0] else {
                    panic!("real None constant")
                };
                constant.const_ = Const::Val(ConstValue::ZeroSized, contract.input);
            }
        }
    }
    reject(&changed, "fabricated inhabited Option is not residual None");
    let mut changed = body.clone();
    for block in changed.basic_blocks.as_mut() {
        for stmt in &mut block.statements {
            if let StatementKind::Assign(assignment) = &mut stmt.kind {
                if let Rvalue::Aggregate(kind, _) = &mut assignment.1 {
                    let AggregateKind::Adt(_, variant, ..) = &mut **kind else {
                        unreachable!()
                    };
                    *variant = VariantIdx::from_usize(1 - variant.as_usize());
                }
            }
        }
    }
    reject(&changed, "swapped ControlFlow variants");
}

fn check_result_branch_forwarding<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "result_branch", "branch");
    let contract = contract(tcx, instance).unwrap();
    let body = tcx.instance_mir(instance.def);
    assert_eq!(contract.operation, Operation::ResultBranch);
    assert!(contract.conversion.is_none());
    let reject = |changed: &Body<'tcx>, label| {
        assert!(
            !reviewed_body(tcx, instance, changed, &contract),
            "Result branch: {label}"
        )
    };
    let (switch, discr, targets) = body
        .basic_blocks
        .iter_enumerated()
        .find_map(|(index, b)| {
            if let TerminatorKind::SwitchInt { discr, targets } = &b.terminator().kind {
                Some((index, discr.clone(), targets.clone()))
            } else {
                None
            }
        })
        .expect("Result discriminant switch");
    let ok = targets.target_for_value(contract.payload_discriminant);
    let err = targets.target_for_value(contract.empty_discriminant);
    for target in [ok, err, targets.otherwise()] {
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[switch].terminator_mut().kind =
            TerminatorKind::Goto { target };
        reject(
            &changed,
            "cannot force either variant or an impossible sink",
        );
    }
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[switch].terminator_mut().kind = TerminatorKind::SwitchInt {
        discr: discr.clone(),
        targets: SwitchTargets::new(
            [
                (contract.payload_discriminant, err),
                (contract.empty_discriminant, ok),
            ]
            .into_iter(),
            targets.otherwise(),
        ),
    };
    reject(&changed, "cannot exchange Ok and Err paths");
    let (error_statement, error_local) = body.basic_blocks[err]
        .statements
        .iter()
        .enumerate()
        .find_map(|(i, stmt)| {
            if let StatementKind::Assign(a) = &stmt.kind {
                if matches!(&a.1, Rvalue::Use(Operand::Move(p)) if !p.projection.is_empty()) {
                    return Some((i, a.0.local));
                }
            }
            None
        })
        .expect("move input error");
    let payload_local = body.basic_blocks[ok]
        .statements
        .iter()
        .find_map(|stmt| {
            if let StatementKind::Assign(a) = &stmt.kind {
                if matches!(&a.1, Rvalue::Use(Operand::Move(p)) if !p.projection.is_empty()) {
                    return Some(a.0.local);
                }
            }
            None
        })
        .expect("move input payload");
    let TyKind::Adt(residual_adt, _) = contract.residual.kind() else {
        unreachable!()
    };
    let residual_statement = body.basic_blocks[err].statements.iter().position(|stmt| {
        matches!(&stmt.kind, StatementKind::Assign(a)
            if matches!(&a.1, Rvalue::Aggregate(kind, _) if matches!(&**kind, AggregateKind::Adt(id, ..) if *id == residual_adt.did())))
    }).expect("construct exact Err residual");
    for mutation in 0..5 {
        let mut changed = body.clone();
        let StatementKind::Assign(a) =
            &mut changed.basic_blocks.as_mut()[err].statements[residual_statement].kind
        else {
            unreachable!()
        };
        let Rvalue::Aggregate(kind, operands) = &mut a.1 else {
            unreachable!()
        };
        let AggregateKind::Adt(_, variant, args, ..) = &mut **kind else {
            unreachable!()
        };
        match mutation {
            0 => *variant = VariantIdx::from_usize(1 - variant.as_usize()),
            1 => *args = tcx.mk_args(&[tcx.types.u32.into(), tcx.types.u32.into()]),
            2 => operands.raw[0] = Operand::Copy(payload_local.into()),
            3 => operands.raw.clear(),
            4 => operands.raw.push(Operand::Copy(error_local.into())),
            _ => unreachable!(),
        }
        reject(
            &changed,
            "residual variant, type, arity, and error provenance are exact",
        );
    }
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[err].statements.insert(
        residual_statement,
        statement(StatementKind::StorageDead(error_local)),
    );
    reject(&changed, "killed error cannot feed residual");
    let mut changed = body.clone();
    let duplicate = changed.basic_blocks[err].statements[error_statement].clone();
    changed.basic_blocks.as_mut()[err]
        .statements
        .insert(error_statement + 1, duplicate);
    reject(&changed, "Err payload cannot be moved twice");
    let (Operand::Move(discr_place) | Operand::Copy(discr_place)) = discr else {
        unreachable!()
    };
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[err].statements.insert(
        error_statement + 1,
        statement(StatementKind::Assign(Box::new((
            discr_place,
            Rvalue::Discriminant(Local::from_usize(1).into()),
        )))),
    );
    assert!(
        reviewed_body(tcx, instance, &changed, &contract),
        "partially moved Err retains its discriminant"
    );
    changed.basic_blocks.as_mut()[err].statements.insert(
        error_statement + 2,
        statement(StatementKind::Assign(Box::new((
            Local::from_usize(1).into(),
            Rvalue::Use(Operand::Copy(Local::from_usize(1).into())),
        )))),
    );
    reject(&changed, "partially moved Err cannot be copied as a whole");
    let mut moved = body.clone();
    let moved_input = moved
        .local_decls
        .push(body.local_decls[Local::from_usize(1)].clone());
    for block in moved.basic_blocks.as_mut() {
        for stmt in &mut block.statements {
            if let StatementKind::Assign(a) = &mut stmt.kind {
                match &mut a.1 {
                    Rvalue::Discriminant(p)
                    | Rvalue::Use(Operand::Move(p))
                    | Rvalue::Use(Operand::Copy(p))
                        if p.local == Local::from_usize(1) =>
                    {
                        p.local = moved_input
                    }
                    _ => {}
                }
            }
        }
    }
    moved.basic_blocks.as_mut()[BasicBlock::from_usize(0)]
        .statements
        .insert(
            0,
            statement(StatementKind::Assign(Box::new((
                moved_input.into(),
                Rvalue::Use(Operand::Move(Local::from_usize(1).into())),
            )))),
        );
    assert!(
        reviewed_body(tcx, instance, &moved, &contract),
        "whole input move preserves both variant paths"
    );
    moved.basic_blocks.as_mut()[BasicBlock::from_usize(0)]
        .statements
        .insert(
            1,
            statement(StatementKind::Assign(Box::new((
                discr_place,
                Rvalue::Discriminant(Local::from_usize(1).into()),
            )))),
        );
    reject(&moved, "whole moved input has no readable discriminant");
    let call = fixture_body(tcx, "panic_path")
        .basic_blocks
        .iter()
        .find(|b| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
        .unwrap()
        .terminator()
        .kind
        .clone();
    for cleanup in [false, true] {
        let mut changed = body.clone();
        append_block(&mut changed, call.clone(), cleanup);
        reject(
            &changed,
            "dead and cleanup calls cannot hide behind branch authentication",
        );
    }
    let mut changed = body.clone();
    changed.basic_blocks.as_mut()[ok].terminator_mut().kind = call;
    reject(&changed, "a branch path cannot trap");
}

fn check_dead_shapes<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "branch_ref", "branch");
    let contract = contract(tcx, instance).unwrap();
    let body = tcx.instance_mir(instance.def);
    let reject = |body: &Body<'tcx>, label| {
        assert!(
            !reviewed_body(tcx, instance, body, &contract),
            "whole body: {label}"
        )
    };
    for cleanup in [false, true] {
        let mut base = body.clone();
        let sink = append_block(
            &mut base,
            if cleanup {
                TerminatorKind::UnwindResume
            } else {
                TerminatorKind::Unreachable
            },
            cleanup,
        );
        let outside = BasicBlock::from_usize(base.basic_blocks.len());
        let bad_local = Local::from_usize(base.local_decls.len());
        for kind in [
            TerminatorKind::Goto { target: outside },
            TerminatorKind::Goto { target: sink },
            TerminatorKind::UnwindTerminate(UnwindTerminateReason::Abi),
        ] {
            let mut changed = base.clone();
            changed.basic_blocks.as_mut()[sink].terminator_mut().kind = kind;
            reject(&changed, "invalid dead edge or trap");
        }
        for targets in [
            SwitchTargets::static_if(0, outside, sink),
            SwitchTargets::static_if(0, sink, outside),
            SwitchTargets::new([(0, sink), (0, sink)].into_iter(), sink),
            SwitchTargets::static_if(2, sink, sink),
        ] {
            let mut changed = base.clone();
            changed.basic_blocks.as_mut()[sink].terminator_mut().kind = TerminatorKind::SwitchInt {
                discr: boolean(tcx, false),
                targets,
            };
            reject(&changed, "invalid dead switch targets");
        }
        for operand in [
            Operand::Copy(bad_local.into()),
            Operand::Move(Place {
                local: Local::from_usize(3),
                projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
            }),
            Operand::Constant(Box::new(ConstOperand {
                span: DUMMY_SP,
                user_ty: None,
                const_: Const::Val(ConstValue::ZeroSized, tcx.types.bool),
            })),
        ] {
            let mut changed = base.clone();
            changed.basic_blocks.as_mut()[sink].terminator_mut().kind = TerminatorKind::SwitchInt {
                discr: operand,
                targets: SwitchTargets::static_if(
                    0,
                    BasicBlock::from_usize(0),
                    BasicBlock::from_usize(0),
                ),
            };
            reject(&changed, "invalid dead switch operands");
        }
        let mut changed = base.clone();
        changed.basic_blocks.as_mut()[sink]
            .statements
            .push(statement(StatementKind::Intrinsic(Box::new(
                NonDivergingIntrinsic::Assume(boolean(tcx, false)),
            ))));
        reject(&changed, "dead false assumption");
        let panic_body = fixture_body(tcx, "panic_path");
        let panic_call = panic_body
            .basic_blocks
            .iter()
            .find(|b| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
            .unwrap()
            .terminator()
            .kind
            .clone();
        let mut changed = base.clone();
        changed.basic_blocks.as_mut()[sink].terminator_mut().kind = panic_call;
        reject(&changed, "dead panic call");
    }
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
            check_instances(tcx);
            if !tcx.sess.panic_strategy().unwinds() {
                check_abort_conversion_unwind(tcx);
            }
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn check_abort_conversion_unwind(tcx: TyCtxt<'_>) {
    for caller in ["r12", "unreviewed_conversion"] {
        let instance = helper(tcx, caller, "from_residual");
        let contract = contract(tcx, instance).unwrap();
        let mut body = tcx.instance_mir(instance.def).clone();
        let call = body
            .basic_blocks
            .iter_enumerated()
            .find(|(_, b)| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
            .unwrap()
            .0;
        let TerminatorKind::Call { unwind, .. } =
            &mut body.basic_blocks.as_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        *unwind = UnwindAction::Unreachable;
        assert!(
            reviewed_body(tcx, instance, &body, &contract),
            "abort conversion edge"
        );
        assert!(
            !authenticate_reviewed_safe_core_result_try_helper_v1(
                tcx,
                contract.conversion.unwrap().1
            ),
            "abort does not transfer From trust"
        );
        for bad in [
            UnwindAction::Terminate(UnwindTerminateReason::Abi),
            UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
        ] {
            let mut changed = body.clone();
            let TerminatorKind::Call { unwind, .. } =
                &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
            else {
                unreachable!()
            };
            *unwind = bad;
            assert!(
                !reviewed_body(tcx, instance, &changed, &contract),
                "explicit termination is not an abort edge"
            );
        }
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[call].terminator_mut().kind = TerminatorKind::Unreachable;
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "abort cannot terminalize the helper"
        );
        let mut changed = body.clone();
        let TerminatorKind::Call { func, .. } =
            &mut changed.basic_blocks.as_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        *func = local_function(tcx, "panic_path");
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "abort cannot substitute a panic call"
        );
    }
}

fn run_fixture(mutations: bool, abort: bool) {
    let directory = TestTempDir::create("fe2o3-core-result-try");
    let source = directory.path().join("fixture.rs");
    fs::write(&source, SOURCE).expect("write fixture");
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("pinned rustc");
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_core_result_try_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    if abort {
        args.push("-Cpanic=abort".into());
    }
    let mut callbacks = CheckCallbacks {
        mutations,
        completed: false,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}

#[test]
fn core_result_try_authenticates_actual_core_and_rejects_unreviewed_instances() {
    run_fixture(false, false);
}

#[test]
fn core_result_try_rejects_mutated_actual_mir() {
    run_fixture(true, false);
}

#[test]
fn core_result_try_abort_preserves_exact_conversion_and_rejects_traps() {
    run_fixture(false, true);
}
