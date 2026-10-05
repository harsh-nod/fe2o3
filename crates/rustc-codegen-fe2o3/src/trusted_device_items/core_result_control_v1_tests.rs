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

fn inspect_residual_bodies<'tcx>(tcx: TyCtxt<'tcx>, results: &mut Results) {
    for name in ["residual_unit", "residual_u32"] {
        let instance = resolved_call(tcx, name, "from_residual");
        assert!(residual_signature_v1(tcx, instance).is_some(), "residual signature: {instance:?}");
        let body = tcx.instance_mir(instance.def);
        assert!(residual_body_v1(tcx, instance, body),
            "residual body: {instance:?}; arguments={} locals={} blocks={} scopes={}",
            body.arg_count, body.local_decls.len(), body.basic_blocks.len(), body.source_scopes.len());
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

fn compiler_results() -> Results {
    static RESULTS: OnceLock<Results> = OnceLock::new();
    RESULTS.get_or_init(|| {
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
        let args = vec![
            "rustc".to_owned(), "--crate-name".to_owned(), "fe2o3_core_result_fixture".to_owned(),
            "--crate-type=lib".to_owned(), "--edition=2024".to_owned(), "--emit=metadata".to_owned(),
            "-Zmir-opt-level=0".to_owned(), "-Zinline-mir=no".to_owned(), "-Cpanic=abort".to_owned(),
            "--sysroot".to_owned(), sysroot, "-o".to_owned(), fixture.0.join("fixture.rmeta").display().to_string(),
            source.display().to_string(),
        ];
        let mut callbacks = FixtureCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        callbacks.results.expect("core Result callback")
    }).clone()
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
