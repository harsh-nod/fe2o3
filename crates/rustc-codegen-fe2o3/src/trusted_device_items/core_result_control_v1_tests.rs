use super::*;
use rustc_abi::{FieldIdx, VariantIdx};
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Const, ConstValue, Local, SwitchTargets};
use rustc_middle::ty::UserTypeAnnotationIndex;
use rustc_span::DUMMY_SP;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn result_family_table_keeps_exact_source_pairs() {
    use ResultErrorFamilyV1 as E;
    use ResultPayloadFamilyV1 as P;
    let pairs = [
        (P::Unit, E::Kernel),
        (P::Unit, E::Matrix),
        (P::Unit, E::Strided),
        (P::Unit, E::CheckedExtent),
        (P::U32, E::Kernel),
        (P::Usize, E::Kernel),
        (P::Usize, E::CheckedExtent),
        (P::MatrixA, E::Matrix),
        (P::MatrixB, E::Matrix),
        (P::StridedF32, E::Strided),
        (P::AttentionTile, E::Kernel),
    ];
    for payload in [P::Unit, P::U32, P::Usize, P::MatrixA, P::MatrixB, P::StridedF32, P::AttentionTile] {
        for error in [E::Kernel, E::Matrix, E::Strided, E::CheckedExtent] {
            assert_eq!(selected_pair_v1(payload, error), pairs.contains(&(payload, error)));
        }
    }
}

const FIXTURE: &str = r#"
#![no_std]
#![feature(try_trait_v2)]
use core::convert::Infallible;
use core::ops::{ControlFlow, FromResidual, Try};
pub fn branch_u32(value: Result<u32, u32>) -> ControlFlow<Result<Infallible, u32>, u32> {
    <Result<u32, u32> as Try>::branch(value)
}
pub fn branch_unit(value: Result<(), u32>) -> ControlFlow<Result<Infallible, u32>, ()> {
    <Result<(), u32> as Try>::branch(value)
}
pub fn option_branch(value: Option<u32>) -> ControlFlow<Option<Infallible>, u32> {
    <Option<u32> as Try>::branch(value)
}
pub fn residual_unit(value: Result<Infallible, u32>) -> Result<(), u32> {
    <Result<(), u32> as FromResidual<Result<Infallible, u32>>>::from_residual(value)
}
pub fn residual_u32(value: Result<Infallible, u32>) -> Result<u32, u32> {
    <Result<u32, u32> as FromResidual<Result<Infallible, u32>>>::from_residual(value)
}
pub fn option_residual(value: Option<Infallible>) -> Option<u32> {
    <Option<u32> as FromResidual<Option<Infallible>>>::from_residual(value)
}
pub enum ResultLookalike { Ok(u32), Err(u32) }
pub fn lookalike(value: ResultLookalike) -> ResultLookalike { value }
"#;

#[derive(Clone, Default)]
struct Results {
    genuine: usize,
    nominal_refusals: usize,
    shapes: usize,
    locals: usize,
    scopes: usize,
    blocks: usize,
    statements: usize,
    control: usize,
    fields: usize,
    aggregates: usize,
    residual_genuine: usize,
    residual_nominal_refusals: usize,
    residual_shapes: usize,
    residual_locals: usize,
    residual_scopes: usize,
    residual_operations: usize,
    residual_assume: usize,
    residual_calls: usize,
    residual_payloads: usize,
}

#[derive(Default)]
struct FixtureCallbacks {
    results: Option<Results>,
}

fn local_function(tcx: TyCtxt<'_>, name: &str) -> DefId {
    tcx.iter_local_def_id()
        .find(|definition| tcx.def_kind(definition.to_def_id()) == DefKind::Fn && tcx.item_name(definition.to_def_id()).as_str() == name)
        .expect("fixture function")
        .to_def_id()
}

fn resolved_call<'tcx>(tcx: TyCtxt<'tcx>, name: &str, method: &str) -> Instance<'tcx> {
    let caller = Instance::mono(tcx, local_function(tcx, name));
    tcx.instance_mir(caller.def).basic_blocks.iter().find_map(|block| {
        let TerminatorKind::Call { func: Operand::Constant(function), .. } = &block.terminator().kind else { return None; };
        let TyKind::FnDef(definition, arguments) = *function.const_.ty().kind() else { return None; };
        let arguments = caller.try_instantiate_mir_and_normalize_erasing_regions(
            tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(arguments),
        ).ok()?;
        let instance = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), definition, arguments).ok()??;
        (tcx.item_name(instance.def_id()).as_str() == method).then_some(instance)
    }).expect("genuine resolved core helper")
}

fn resolved_branch<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    resolved_call(tcx, name, "branch")
}

fn reject_mutation<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>, label: &str,
    mutate: impl FnOnce(&mut Body<'tcx>),
) {
    let mut changed = body.clone();
    mutate(&mut changed);
    assert!(!branch_body_v1(tcx, instance, &changed), "accepted MIR mutation: {label}");
}

fn reject_residual_mutation<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>, label: &str,
    mutate: impl FnOnce(&mut Body<'tcx>),
) {
    let mut changed = body.clone();
    mutate(&mut changed);
    assert!(!residual_body_v1(tcx, instance, &changed), "accepted residual MIR mutation: {label}");
}

impl Callbacks for FixtureCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let mut results = Results::default();
        for name in ["branch_u32", "branch_unit"] {
            let instance = resolved_branch(tcx, name);
            assert!(branch_signature_v1(tcx, instance).is_some(), "branch signature: {instance:?}");
            let body = tcx.instance_mir(instance.def);
            assert!(branch_body_v1(tcx, instance, body),
                "branch body: {instance:?}; arguments={} locals={} blocks={} scopes={}",
                body.arg_count, body.local_decls.len(), body.basic_blocks.len(), body.source_scopes.len());
            results.genuine += 1;
            // Body safety alone does not admit unselected scalar error types.
            assert!(!authenticate_reviewed_safe_core_result_branch_v1(tcx, instance));
            results.nominal_refusals += 1;
        }
        let option = resolved_branch(tcx, "option_branch");
        assert!(branch_signature_v1(tcx, option).is_none());
        let lookalike = Instance::mono(tcx, local_function(tcx, "lookalike"));
        assert!(branch_signature_v1(tcx, lookalike).is_none());
        results.nominal_refusals += 2;

        let instance = resolved_branch(tcx, "branch_u32");
        let body = tcx.instance_mir(instance.def);
        let bb = BasicBlock::from_u32;
        reject_mutation(tcx, instance, body, "argument count", |body| body.arg_count = 2);
        reject_mutation(tcx, instance, body, "local count", |body| { body.local_decls.pop(); });
        reject_mutation(tcx, instance, body, "block count", |body| { body.basic_blocks.as_mut().pop(); });
        reject_mutation(tcx, instance, body, "scope count", |body| { body.source_scopes.pop(); });
        results.shapes = 4;
        for index in 0..6 {
            let local = Local::from_u32(index);
            reject_mutation(tcx, instance, body, "local type", |body| body.local_decls[local].ty = tcx.types.bool);
            reject_mutation(tcx, instance, body, "local scope", |body| body.local_decls[local].source_info.scope = SourceScope::from_u32(1));
            results.locals += 2;
        }
        for index in 0..3 {
            let scope = SourceScope::from_u32(index);
            reject_mutation(tcx, instance, body, "scope parent", |body| {
                body.source_scopes[scope].parent_scope = if index == 0 { Some(scope) } else { None };
            });
            reject_mutation(tcx, instance, body, "inline parent", |body| body.source_scopes[scope].inlined_parent_scope = Some(SourceScope::from_u32(0)));
            reject_mutation(tcx, instance, body, "inline origin", |body| body.source_scopes[scope].inlined = Some((instance, DUMMY_SP)));
            results.scopes += 3;
        }
        for index in 0..5 {
            reject_mutation(tcx, instance, body, "cleanup block", |body| body.basic_blocks.as_mut()[bb(index)].is_cleanup = true);
            reject_mutation(tcx, instance, body, "missing terminator", |body| body.basic_blocks.as_mut()[bb(index)].terminator = None);
            reject_mutation(tcx, instance, body, "extra statement", |body| {
                let mut extra = body.basic_blocks[bb(0)].statements[0].clone();
                extra.kind = StatementKind::Nop;
                body.basic_blocks.as_mut()[bb(index)].statements.push(extra);
            });
            results.blocks += 3;
            for statement in 0..body.basic_blocks[bb(index)].statements.len() {
                reject_mutation(tcx, instance, body, "statement kind", |body| body.basic_blocks.as_mut()[bb(index)].statements[statement].kind = StatementKind::Nop);
                reject_mutation(tcx, instance, body, "statement scope", |body| {
                    let source = &mut body.basic_blocks.as_mut()[bb(index)].statements[statement].source_info;
                    source.scope = SourceScope::from_u32((source.scope.as_u32() + 1) % 3);
                });
                results.statements += 2;
            }
            reject_mutation(tcx, instance, body, "terminator kind", |body| {
                body.basic_blocks.as_mut()[bb(index)].terminator_mut().kind = if index == 4 { TerminatorKind::Unreachable } else { TerminatorKind::Return };
            });
            reject_mutation(tcx, instance, body, "terminator scope", |body| {
                body.basic_blocks.as_mut()[bb(index)].terminator_mut().source_info.scope = SourceScope::from_u32(1);
            });
            results.control += 2;
        }
        reject_mutation(tcx, instance, body, "copied discriminant", |body| {
            let TerminatorKind::SwitchInt { discr, .. } = &mut body.basic_blocks.as_mut()[bb(0)].terminator_mut().kind else { unreachable!() };
            *discr = Operand::Copy(Place::from(Local::from_u32(2)));
        });
        reject_mutation(tcx, instance, body, "swapped branches", |body| {
            let TerminatorKind::SwitchInt { targets, .. } = &mut body.basic_blocks.as_mut()[bb(0)].terminator_mut().kind else { unreachable!() };
            *targets = SwitchTargets::new([(0, bb(2)), (1, bb(3))].into_iter(), bb(1));
        });
        reject_mutation(tcx, instance, body, "reachable invalid discriminant", |body| {
            let TerminatorKind::SwitchInt { targets, .. } = &mut body.basic_blocks.as_mut()[bb(0)].terminator_mut().kind else { unreachable!() };
            *targets = SwitchTargets::new([(0, bb(3)), (1, bb(2))].into_iter(), bb(4));
        });
        results.control += 3;
        for index in [2, 3] {
            for axis in 0..4 {
                reject_mutation(tcx, instance, body, "payload field", |body| {
                    let StatementKind::Assign(assignment) = &mut body.basic_blocks.as_mut()[bb(index)].statements[0].kind else { unreachable!() };
                    let Rvalue::Use(operand) = &mut assignment.1 else { unreachable!() };
                    let Operand::Move(place) = operand else { unreachable!() };
                    let mut changed = *place;
                    match axis {
                        0 => { *operand = Operand::Copy(changed); }
                        1 => { place.local = Local::from_u32(0); }
                        _ => {
                            let mut projection = changed.projection.to_vec();
                            if axis == 2 {
                                projection[0] = ProjectionElem::Downcast(None, VariantIdx::from_u32(if index == 2 { 0 } else { 1 }));
                            } else {
                                projection[1] = ProjectionElem::Field(FieldIdx::from_u32(0), tcx.types.bool);
                            }
                            changed = Place::from(changed.local).project_deeper(&projection, tcx);
                            *place = changed;
                        }
                    }
                });
                results.fields += 1;
            }
        }
        for (block, statement) in [(2, 2), (2, 3), (3, 1)] {
            for axis in 0..7 {
                reject_mutation(tcx, instance, body, "aggregate payload", |body| {
                    let StatementKind::Assign(assignment) = &mut body.basic_blocks.as_mut()[bb(block)].statements[statement].kind else { unreachable!() };
                    if axis == 0 { assignment.0.local = Local::from_u32(1); return; }
                    let Rvalue::Aggregate(kind, operands) = &mut assignment.1 else { unreachable!() };
                    if axis == 1 {
                        operands[FieldIdx::from_u32(0)] = match &operands[FieldIdx::from_u32(0)] {
                            Operand::Copy(place) => Operand::Move(*place),
                            Operand::Move(place) => Operand::Copy(*place),
                            _ => unreachable!(),
                        };
                        return;
                    }
                    if axis == 2 {
                        match &mut operands[FieldIdx::from_u32(0)] {
                            Operand::Copy(place) | Operand::Move(place) => place.local = Local::from_u32(1),
                            _ => unreachable!(),
                        }
                        return;
                    }
                    let AggregateKind::Adt(_, variant, arguments, annotation, union_field) = &mut **kind else { unreachable!() };
                    match axis {
                        3 => *variant = VariantIdx::from_u32(1 - variant.as_u32()),
                        4 => *arguments = tcx.mk_args(&[]),
                        5 => *annotation = Some(UserTypeAnnotationIndex::from_u32(0)),
                        6 => *union_field = Some(FieldIdx::from_u32(0)),
                        _ => unreachable!(),
                    }
                });
                results.aggregates += 1;
            }
        }
        inspect_residual_bodies(tcx, &mut results);
        self.results = Some(results);
        Compilation::Stop
    }
}

fn residual_fixture_diagnostic<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>,
) -> String {
    use std::fmt::{self, Write as _};
    struct Bounded(String);
    impl fmt::Write for Bounded {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            if text.len() > 7168_usize.saturating_sub(self.0.len()) {
                return Err(fmt::Error);
            }
            self.0.try_reserve(text.len()).map_err(|_| fmt::Error)?;
            self.0.push_str(text);
            Ok(())
        }
    }
    let mut output = Bounded(String::new());
    let rendered = (|| -> fmt::Result {
        writeln!(output, "CORE_RESULT_HOST_RESIDUAL_BEGIN authority=none cap=8192")?;
        writeln!(output, "instance={instance:?}")?;
        writeln!(output, "arguments={} locals={} blocks={} scopes={}",
            body.arg_count, body.local_decls.len(), body.basic_blocks.len(), body.source_scopes.len())?;
        if body.arg_count != 1 || body.local_decls.len() != 6
            || body.basic_blocks.len() != 2 || body.source_scopes.len() != 2
            || body.basic_blocks.iter().any(|block| block.statements.len() > 8 || block.terminator.is_none())
        {
            return writeln!(output, "shape_outside_diagnostic_bound=true");
        }
        let conversion = residual_signature_v1(tcx, instance)
            .map(|types| conversion_call_v1(tcx, instance, body, &types));
        writeln!(output, "conversion_call_v1={conversion:?}")?;
        for (index, local) in body.local_decls.iter_enumerated() {
            writeln!(output, "local[{index:?}] raw={:?} normalized={:?} scope={:?}",
                local.ty, normalize_type_v1(tcx, instance, local.ty), local.source_info.scope)?;
        }
        for (index, scope) in body.source_scopes.iter_enumerated() {
            writeln!(output, "scope[{index:?}]={scope:?}")?;
        }
        for (index, block) in body.basic_blocks.iter_enumerated() {
            writeln!(output, "block[{index:?}] cleanup={}", block.is_cleanup)?;
            for (statement_index, statement) in block.statements.iter().enumerate() {
                writeln!(output, "statement[{statement_index}] scope={:?} kind={:?}",
                    statement.source_info.scope, statement.kind)?;
            }
            let terminator = block.terminator();
            writeln!(output, "terminator scope={:?} kind={:?}",
                terminator.source_info.scope, terminator.kind)?;
        }
        Ok(())
    })();
    let status = if rendered.is_ok() { "complete" } else { "truncated_or_render_error" };
    let body_bytes = output.0.len();
    // Reserved space keeps the entire failure diagnostic below 8 KiB.
    let _ = writeln!(output.0, "CORE_RESULT_HOST_RESIDUAL_END status={status} body_bytes={body_bytes} authority=none");
    output.0
}

fn inspect_residual_bodies<'tcx>(tcx: TyCtxt<'tcx>, results: &mut Results) {
    for name in ["residual_unit", "residual_u32"] {
        let instance = resolved_call(tcx, name, "from_residual");
        assert!(residual_signature_v1(tcx, instance).is_some(), "residual signature: {instance:?}");
        let body = tcx.instance_mir(instance.def);
        assert!(residual_body_v1(tcx, instance, body),
            "residual body rejected; {}", residual_fixture_diagnostic(tcx, instance, body));
        results.residual_genuine += 1;
        assert!(!authenticate_reviewed_safe_core_result_residual_v1(tcx, instance));
        results.residual_nominal_refusals += 1;
    }
    let option = resolved_call(tcx, "option_residual", "from_residual");
    assert!(residual_signature_v1(tcx, option).is_none());
    let lookalike = Instance::mono(tcx, local_function(tcx, "lookalike"));
    assert!(residual_signature_v1(tcx, lookalike).is_none());
    results.residual_nominal_refusals += 2;

    let instance = resolved_call(tcx, "residual_unit", "from_residual");
    let body = tcx.instance_mir(instance.def);
    let bb = BasicBlock::from_u32;
    reject_residual_mutation(tcx, instance, body, "argument count", |body| body.arg_count = 2);
    reject_residual_mutation(tcx, instance, body, "local count", |body| { body.local_decls.pop(); });
    reject_residual_mutation(tcx, instance, body, "block count", |body| { body.basic_blocks.as_mut().pop(); });
    reject_residual_mutation(tcx, instance, body, "scope count", |body| { body.source_scopes.pop(); });
    results.residual_shapes = 4;
    for index in 0..6 {
        let local = Local::from_u32(index);
        reject_residual_mutation(tcx, instance, body, "local type", |body| {
            body.local_decls[local].ty = if index == 5 { tcx.types.u32 } else { tcx.types.bool };
        });
        reject_residual_mutation(tcx, instance, body, "local scope", |body| body.local_decls[local].source_info.scope = SourceScope::from_u32(1));
        results.residual_locals += 2;
    }
    for index in 0..2 {
        let scope = SourceScope::from_u32(index);
        reject_residual_mutation(tcx, instance, body, "scope parent", |body| {
            body.source_scopes[scope].parent_scope = if index == 0 { Some(scope) } else { None };
        });
        reject_residual_mutation(tcx, instance, body, "inline parent", |body| body.source_scopes[scope].inlined_parent_scope = Some(SourceScope::from_u32(0)));
        reject_residual_mutation(tcx, instance, body, "inline origin", |body| body.source_scopes[scope].inlined = Some((instance, DUMMY_SP)));
        results.residual_scopes += 3;
    }
    for index in 0..2 {
        reject_residual_mutation(tcx, instance, body, "cleanup block", |body| body.basic_blocks.as_mut()[bb(index)].is_cleanup = true);
        reject_residual_mutation(tcx, instance, body, "missing terminator", |body| body.basic_blocks.as_mut()[bb(index)].terminator = None);
        reject_residual_mutation(tcx, instance, body, "extra statement", |body| {
            let mut extra = body.basic_blocks[bb(0)].statements[0].clone();
            extra.kind = StatementKind::Nop;
            body.basic_blocks.as_mut()[bb(index)].statements.push(extra);
        });
        reject_residual_mutation(tcx, instance, body, "terminator kind", |body| body.basic_blocks.as_mut()[bb(index)].terminator_mut().kind = TerminatorKind::Unreachable);
        reject_residual_mutation(tcx, instance, body, "terminator scope", |body| {
            let source = &mut body.basic_blocks.as_mut()[bb(index)].terminator_mut().source_info;
            source.scope = SourceScope::from_u32(1 - source.scope.as_u32());
        });
        results.residual_operations += 5;
        for statement in 0..body.basic_blocks[bb(index)].statements.len() {
            reject_residual_mutation(tcx, instance, body, "statement kind", |body| body.basic_blocks.as_mut()[bb(index)].statements[statement].kind = StatementKind::Nop);
            reject_residual_mutation(tcx, instance, body, "statement scope", |body| {
                let source = &mut body.basic_blocks.as_mut()[bb(index)].statements[statement].source_info;
                source.scope = SourceScope::from_u32(1 - source.scope.as_u32());
            });
            results.residual_operations += 2;
        }
    }
    for axis in 0..7 {
        reject_residual_mutation(tcx, instance, body, "discriminant/assume", |body| {
            let entry = &mut body.basic_blocks.as_mut()[bb(0)];
            if axis < 2 {
                let StatementKind::Assign(assignment) = &mut entry.statements[0].kind else { unreachable!() };
                if axis == 0 { assignment.0.local = Local::from_u32(5); }
                else { assignment.1 = Rvalue::Discriminant(Place::from(Local::from_u32(0))); }
            } else if axis < 5 {
                let StatementKind::Assign(assignment) = &mut entry.statements[1].kind else { unreachable!() };
                let Rvalue::BinaryOp(operation, operands) = &mut assignment.1 else { unreachable!() };
                match axis {
                    2 => *operation = BinOp::Ne,
                    3 => operands.0 = Operand::Move(Place::from(Local::from_u32(2))),
                    4 => {
                        let Operand::Constant(one) = &mut operands.1 else { unreachable!() };
                        one.const_ = Const::Val(ConstValue::from_target_usize(0, &tcx), tcx.types.isize);
                    }
                    _ => unreachable!(),
                }
            } else {
                let StatementKind::Intrinsic(intrinsic) = &mut entry.statements[2].kind else { unreachable!() };
                **intrinsic = NonDivergingIntrinsic::Assume(if axis == 5 {
                    Operand::Copy(Place::from(Local::from_u32(5)))
                } else { Operand::Move(Place::from(Local::from_u32(2))) });
            }
        });
        results.residual_assume += 1;
    }
    let branch_caller = Instance::mono(tcx, local_function(tcx, "branch_u32"));
    let different_function = tcx.instance_mir(branch_caller.def).basic_blocks.iter().find_map(|block| {
        if let TerminatorKind::Call { func, .. } = &block.terminator().kind { Some(func.clone()) } else { None }
    }).expect("different genuine trait call");
    for axis in 0..10 {
        reject_residual_mutation(tcx, instance, body, "conversion call", |body| {
            let TerminatorKind::Call { func, args, destination, target, unwind, .. } = &mut body.basic_blocks.as_mut()[bb(0)].terminator_mut().kind else { unreachable!() };
            match axis {
                0 => *func = different_function.clone(),
                1 => *func = Operand::Copy(Place::from(Local::from_u32(3))),
                2 => args[0].node = Operand::Copy(Place::from(Local::from_u32(3))),
                3 => args[0].node = Operand::Move(Place::from(Local::from_u32(1))),
                4 => *args = Vec::new().into_boxed_slice(),
                5 => *destination = Place::from(Local::from_u32(0)),
                6 => *target = None,
                7 => *target = Some(bb(0)),
                8 => *unwind = UnwindAction::Continue,
                9 => *unwind = UnwindAction::Cleanup(bb(1)),
                _ => unreachable!(),
            }
        });
        results.residual_calls += 1;
    }
    for axis in 0..4 {
        reject_residual_mutation(tcx, instance, body, "residual input field", |body| {
            let StatementKind::Assign(assignment) = &mut body.basic_blocks.as_mut()[bb(0)].statements[3].kind else { unreachable!() };
            let Rvalue::Use(operand) = &mut assignment.1 else { unreachable!() };
            let Operand::Move(place) = operand else { unreachable!() };
            match axis {
                0 => *operand = Operand::Copy(*place),
                1 => place.local = Local::from_u32(0),
                _ => {
                    let mut projection = place.projection.to_vec();
                    if axis == 2 { projection[0] = ProjectionElem::Downcast(None, VariantIdx::from_u32(0)); }
                    else { projection[1] = ProjectionElem::Field(FieldIdx::from_u32(0), tcx.types.bool); }
                    *place = Place::from(place.local).project_deeper(&projection, tcx);
                }
            }
        });
        results.residual_payloads += 1;
    }
    for axis in 0..7 {
        reject_residual_mutation(tcx, instance, body, "residual output", |body| {
            let StatementKind::Assign(assignment) = &mut body.basic_blocks.as_mut()[bb(1)].statements[0].kind else { unreachable!() };
            if axis == 0 { assignment.0.local = Local::from_u32(1); return; }
            let Rvalue::Aggregate(kind, operands) = &mut assignment.1 else { unreachable!() };
            if axis == 1 { operands[FieldIdx::from_u32(0)] = Operand::Copy(Place::from(Local::from_u32(4))); return; }
            if axis == 2 { operands[FieldIdx::from_u32(0)] = Operand::Move(Place::from(Local::from_u32(3))); return; }
            let AggregateKind::Adt(_, variant, arguments, annotation, union_field) = &mut **kind else { unreachable!() };
            match axis {
                3 => *variant = VariantIdx::from_u32(0),
                4 => *arguments = tcx.mk_args(&[]),
                5 => *annotation = Some(UserTypeAnnotationIndex::from_u32(0)),
                6 => *union_field = Some(FieldIdx::from_u32(0)),
                _ => unreachable!(),
            }
        });
        results.residual_payloads += 1;
    }
}

struct FixtureDirectory(PathBuf);

impl Drop for FixtureDirectory {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

// Host prebuilt core retains unwind Continue even with callback panic=abort.
// Build genuine target core with the measured matrix's gfx942 release flags;
// this is a compiler fixture, not gfx950 device qualification.
fn target_core_metadata(fixture: &FixtureDirectory, sysroot: &std::path::Path) -> (PathBuf, PathBuf) {
    let manifest = fixture.0.join("Cargo.toml");
    fs::write(&manifest, r#"[package]
name = "fe2o3_core_result_fixture"
version = "0.0.0"
edition = "2024"
publish = false
[workspace]
[lib]
path = "fixture.rs"
"#).expect("write private target-core fixture manifest");
    let target = fixture.0.join("target-core");
    let configure = |command: &mut Command| {
        command.current_dir(&fixture.0)
            .env("RUSTC", sysroot.join("bin/rustc"))
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env("CARGO_BUILD_JOBS", "2")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_CACHE_AUTO_CLEAN_FREQUENCY", "never")
            .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32");
    };
    let mut lock = Command::new(env!("CARGO"));
    configure(&mut lock);
    lock.args(["generate-lockfile", "--offline", "--manifest-path"]).arg(&manifest);
    let lock = crate::process_execution::capture_output(&mut lock)
        .expect("generate private target-core fixture lockfile");
    assert!(lock.stdout.len() <= 4 << 20 && lock.stderr.len() <= 4 << 20);
    assert!(lock.status.success(), "target-core fixture lockfile failed: {}", String::from_utf8_lossy(&lock.stderr));
    let mut command = Command::new(env!("CARGO"));
    configure(&mut command);
    command.args(["check", "--offline", "--locked", "--release", "-Zbuild-std=core",
        "--target", "amdgcn-amd-amdhsa", "--message-format=json", "--target-dir"])
        .arg(&target).args(["--manifest-path"]).arg(&manifest).arg("--lib");
    let output = crate::process_execution::capture_output(&mut command)
        .expect("build genuine AMD target core for Result callback");
    assert!(output.stdout.len() <= 4 << 20 && output.stderr.len() <= 4 << 20);
    assert!(output.status.success(), "target-core build failed: {}", String::from_utf8_lossy(&output.stderr));
    let expected_source = sysroot.join("lib/rustlib/src/rust/library/core/src/lib.rs")
        .canonicalize().expect("pinned rust-src core source");
    let dependencies = target.join("amdgcn-amd-amdhsa/release/deps")
        .canonicalize().expect("private target-core dependency directory");
    let mut selected = None;
    let mut finished = 0;
    for line in String::from_utf8(output.stdout).expect("Cargo JSON UTF-8").lines() {
        if line.trim().is_empty() { continue; }
        let record: serde_json::Value = serde_json::from_str(line).expect("structured Cargo message");
        if record["reason"].as_str() == Some("build-finished") {
            assert_eq!(record["success"].as_bool(), Some(true));
            finished += 1;
        }
        if record["reason"].as_str() != Some("compiler-artifact")
            || record["target"]["name"].as_str() != Some("core") { continue; }
        assert!(selected.is_none(), "duplicate core artifact");
        for field in ["kind", "crate_types"] {
            let values = record["target"][field].as_array().expect("core target kinds");
            assert!(values.len() == 1 && values[0].as_str() == Some("lib"));
        }
        assert_eq!(record["profile"]["test"].as_bool(), Some(false));
        assert_eq!(record["profile"]["opt_level"].as_str(), Some("3"));
        let source = PathBuf::from(record["target"]["src_path"].as_str().expect("core source path"));
        assert_eq!(source.canonicalize().expect("core artifact source"), expected_source);
        let mut metadata = Vec::new();
        let mut libraries = Vec::new();
        for filename in record["filenames"].as_array().expect("core artifact filenames") {
            let path = PathBuf::from(filename.as_str().expect("core artifact filename"));
            let extension = path.extension().and_then(|value| value.to_str());
            if !matches!(extension, Some("rmeta" | "rlib")) { continue; }
            assert!(fs::symlink_metadata(&path).expect("core artifact metadata").file_type().is_file());
            let path = path.canonicalize().expect("canonical core artifact");
            assert_eq!(path.parent(), Some(dependencies.as_path()));
            if extension == Some("rmeta") { metadata.push(path); } else { libraries.push(path); }
        }
        assert!(metadata.len() <= 1 && libraries.len() <= 1, "unique core metadata/library artifacts");
        selected = metadata.pop().or_else(|| libraries.pop());
        assert!(selected.is_some(), "Cargo omitted core metadata/library");
    }
    assert_eq!(finished, 1, "one successful Cargo build-finished message");
    (selected.expect("Cargo selected actual target core"), dependencies)
}

fn compiler_results() -> Results {
    static RESULTS: OnceLock<Result<Results, String>> = OnceLock::new();
    let result = RESULTS.get_or_init(|| std::panic::catch_unwind(|| {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        static DRIVER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = DRIVER_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poison| poison.into_inner());
        let root = std::env::temp_dir().join(format!("fe2o3-core-result-v1-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).expect("create core Result fixture directory");
        let fixture = FixtureDirectory(root);
        let source = fixture.0.join("fixture.rs");
        fs::write(&source, FIXTURE).expect("write compiler fixture");
        let mut command = Command::new("rustc");
        command.args(["--print", "sysroot"]);
        let sysroot = crate::process_execution::capture_output(&mut command).expect("query rustc sysroot");
        assert!(sysroot.status.success());
        let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 sysroot").trim().to_owned();
        let (core, dependencies) = target_core_metadata(&fixture, std::path::Path::new(&sysroot));
        let args = vec![
            "rustc".to_owned(), "--crate-name".to_owned(), "fe2o3_core_result_fixture".to_owned(),
            "--crate-type=lib".to_owned(), "--edition=2024".to_owned(), "--emit=metadata".to_owned(),
            "-Zmir-opt-level=0".to_owned(), "-Zinline-mir=no".to_owned(), "-Cpanic=abort".to_owned(),
            "--target=amdgcn-amd-amdhsa".to_owned(), "-Ctarget-cpu=gfx942".to_owned(),
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".to_owned(),
            "--extern".to_owned(), format!("core={}", core.display()),
            "-L".to_owned(), format!("dependency={}", dependencies.display()),
            "--sysroot".to_owned(), sysroot, "-o".to_owned(), fixture.0.join("fixture.rmeta").display().to_string(),
            source.display().to_string(),
        ];
        let mut callbacks = FixtureCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        callbacks.results.expect("core Result callback")
    }).map_err(|payload| {
        if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else {
            "core Result fixture initialization panicked with a non-string payload".to_owned()
        }
    }));
    result.as_ref().unwrap_or_else(|message| panic!("cached core Result fixture failure: {message}")).clone()
}

#[test]
fn result_branch_real_core_bodies_and_nominal_refusals() {
    let results = compiler_results();
    assert_eq!((results.genuine, results.nominal_refusals), (2, 4));
}

#[test]
fn result_branch_real_body_rejects_count_and_local_mutations() {
    let results = compiler_results();
    assert_eq!((results.shapes, results.locals), (4, 12));
}

#[test]
fn result_branch_real_body_rejects_scope_and_inline_origin_mutations() {
    assert_eq!(compiler_results().scopes, 9);
}

#[test]
fn result_branch_real_body_rejects_extra_operations_and_cleanup() {
    let results = compiler_results();
    assert_eq!((results.blocks, results.statements), (15, 16));
}

#[test]
fn result_branch_real_body_rejects_changed_edges_and_discriminants() {
    assert_eq!(compiler_results().control, 13);
}

#[test]
fn result_branch_real_body_rejects_changed_payload_fields() {
    assert_eq!(compiler_results().fields, 8);
}

#[test]
fn result_branch_real_body_rejects_changed_aggregate_identity_and_moves() {
    assert_eq!(compiler_results().aggregates, 21);
}

#[test]
fn result_residual_real_core_bodies_and_nominal_refusals() {
    let results = compiler_results();
    assert_eq!((results.residual_genuine, results.residual_nominal_refusals), (2, 4));
}

#[test]
fn result_residual_real_body_rejects_count_and_local_mutations() {
    let results = compiler_results();
    assert_eq!((results.residual_shapes, results.residual_locals), (4, 12));
}

#[test]
fn result_residual_real_body_rejects_scope_and_inline_origin_mutations() {
    assert_eq!(compiler_results().residual_scopes, 6);
}

#[test]
fn result_residual_real_body_rejects_extra_operations_and_cleanup() {
    assert_eq!(compiler_results().residual_operations, 24);
}

#[test]
fn result_residual_real_body_rejects_changed_assume_and_discriminant() {
    assert_eq!(compiler_results().residual_assume, 7);
}

#[test]
fn result_residual_real_body_rejects_changed_conversion_call() {
    assert_eq!(compiler_results().residual_calls, 10);
}

#[test]
fn result_residual_real_body_rejects_changed_payload_and_output() {
    assert_eq!(compiler_results().residual_payloads, 11);
}
