use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::{
    mir::{
        BasicBlockData, Const, ConstOperand, Place, ProjectionElem, SourceInfo, Statement,
        SwitchTargets, Terminator, UnwindTerminateReason,
    },
    ty::TyKind,
};
use rustc_span::DUMMY_SP;
use std::{fs, path::PathBuf, process::Command};

const SOURCE: &str = r#"
#![no_std]
#![allow(dead_code)]
pub struct NonCopy(pub u32);
pub struct Dropped;
impl Drop for Dropped { fn drop(&mut self) {} }
pub struct Panicking;
impl Drop for Panicking { fn drop(&mut self) { panic!("unchecked destructor") } }
pub trait Associated { type Type; }
impl Associated for NonCopy { type Type = u32; }
pub fn primitive(flag: bool, value: u32) -> Option<u32> { flag.then_some(value) }
pub fn equal_bool(flag: bool, value: bool) -> Option<bool> { flag.then_some(value) }
pub fn unit(flag: bool, value: ()) -> Option<()> { flag.then_some(value) }
pub fn reference(flag: bool, value: &u32) -> Option<&u32> { flag.then_some(value) }
pub fn mutable_reference(flag: bool, value: &mut u32) -> Option<&mut u32> { flag.then_some(value) }
pub fn noncopy(flag: bool, value: NonCopy) -> Option<NonCopy> { flag.then_some(value) }
pub fn associated(flag: bool, value: <NonCopy as Associated>::Type) -> Option<u32> { flag.then_some(value) }
pub fn array(flag: bool, value: [NonCopy; 2]) -> Option<[NonCopy; 2]> { flag.then_some(value) }
pub fn nested(flag: bool, value: Option<core::convert::Infallible>) -> Option<Option<core::convert::Infallible>> { flag.then_some(value) }
pub fn user_destructor(flag: bool, value: Dropped) -> Option<Dropped> { flag.then_some(value) }
pub fn panicking_destructor(flag: bool, value: Panicking) -> Option<Panicking> { flag.then_some(value) }
pub fn lazy(flag: bool) -> Option<u32> { flag.then(|| 0) }
pub struct Impostor;
impl Impostor { pub fn then_some<T>(self, value: T) -> Option<T> { Some(value) } }
pub fn impostor(value: u32) -> Option<u32> { Impostor.then_some(value) }
pub fn trap() { panic!("unchecked helper") }
"#;

const DEVICE_SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{
    CurrentTarget, GlobalBf16MfmaMatrix, Invocation3D, KernelCapabilityBrand, KernelError,
    MfmaOperandA, RegisteredLaunch, ReusablePhaseCompletion,
};
pub struct KernelMarker;
type Brand = KernelCapabilityBrand<'static, KernelMarker, CurrentTarget, RegisteredLaunch>;
type Matrix = GlobalBf16MfmaMatrix<'static, 'static, MfmaOperandA, Brand, Brand>;
pub fn invocation(flag: bool, value: Invocation3D<Brand>) -> Option<Invocation3D<Brand>> { flag.then_some(value) }
pub fn completion(flag: bool, value: ReusablePhaseCompletion<'static, 'static, Brand>) -> Option<ReusablePhaseCompletion<'static, 'static, Brand>> { flag.then_some(value) }
pub fn matrix(flag: bool, value: Matrix) -> Option<Matrix> { flag.then_some(value) }
pub fn error(flag: bool, value: KernelError) -> Option<KernelError> { flag.then_some(value) }
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

fn helper<'tcx>(tcx: TyCtxt<'tcx>, caller: &str) -> Instance<'tcx> {
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
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args).unwrap()
        })
        .expect("fixture helper")
}

fn check_positive(tcx: TyCtxt<'_>, caller: &str) {
    let instance = helper(tcx, caller);
    let contract = contract(tcx, instance).expect("exact primitive bool member and signature");
    assert!(
        authenticate_reviewed_safe_core_bool_helper_v1(tcx, instance),
        "actual then_some {caller}: {:#?}",
        tcx.instance_mir(instance.def)
    );
    assert!(
        !authenticate_reviewed_safe_core_bool_helper_v1(tcx, contract.glue),
        "no drop-glue trust"
    );
    if caller.ends_with("destructor") {
        assert!(matches!(
            contract.glue.def,
            InstanceKind::DropGlue(_, Some(_))
        ));
        let TyKind::Adt(adt, _) = contract.payload.kind() else {
            panic!("destructor ADT")
        };
        assert!(
            !authenticate_reviewed_safe_core_bool_helper_v1(
                tcx,
                Instance::mono(tcx, adt.destructor(tcx).unwrap().did)
            ),
            "no user destructor trust"
        );
    }
    drop_flag_tests::check(tcx, caller);
}

fn append_block<'tcx>(body: &mut Body<'tcx>, kind: TerminatorKind<'tcx>, cleanup: bool) {
    body.basic_blocks.as_mut().push(BasicBlockData::new(
        Some(Terminator {
            source_info: SourceInfo::outermost(DUMMY_SP),
            kind,
        }),
        cleanup,
    ));
}

fn check_mutations<'tcx>(tcx: TyCtxt<'tcx>) {
    for caller in ["primitive", "equal_bool", "noncopy", "user_destructor"] {
        let instance = helper(tcx, caller);
        let contract = contract(tcx, instance).unwrap();
        let body = tcx.instance_mir(instance.def);
        let entry = BasicBlock::from_usize(0);
        let TerminatorKind::SwitchInt { targets, .. } = &body.basic_blocks[entry].terminator().kind
        else {
            panic!("switch")
        };
        let yes = targets.target_for_value(1);
        let no = targets.target_for_value(0);
        let TerminatorKind::Goto { target: done } = body.basic_blocks[yes].terminator().kind else {
            panic!("goto")
        };
        let reject = |body: &Body<'tcx>, label| {
            assert!(
                !reviewed_body(tcx, instance, body, &contract),
                "{caller}: {label}"
            )
        };
        for mutation in 0..19 {
            let mut changed = body.clone();
            match mutation {
                0 => changed.arg_count = 1,
                1 => changed.local_decls[Local::from_usize(1)].ty = tcx.types.u8,
                2 => changed.local_decls[Local::from_usize(0)].ty = tcx.types.unit,
                3 => {
                    changed
                        .local_decls
                        .push(body.local_decls[Local::from_usize(2)].clone());
                }
                4 => append_block(&mut changed, TerminatorKind::Unreachable, false),
                5 => changed.basic_blocks.as_mut()[done]
                    .statements
                    .push(Statement::new(
                        SourceInfo::outermost(DUMMY_SP),
                        StatementKind::Nop,
                    )),
                6 => changed.basic_blocks.as_mut()[entry].terminator = None,
                7 => changed.basic_blocks.as_mut()[no].is_cleanup = true,
                8 => {
                    let scope = body.local_decls[Local::from_usize(0)].source_info.scope;
                    changed
                        .source_scopes
                        .push(body.source_scopes[scope].clone());
                }
                9 => changed.source.instance = helper(tcx, "lazy").def,
                10 => changed.mentioned_items = None,
                11 => {
                    changed.mentioned_items.as_mut().unwrap()[0].node =
                        MentionedItem::Drop(tcx.types.u8)
                }
                12 => {
                    changed.basic_blocks.as_mut()[done].terminator_mut().kind =
                        TerminatorKind::Unreachable
                }
                13 => {
                    changed.basic_blocks.as_mut()[no].terminator_mut().kind =
                        TerminatorKind::Goto { target: done }
                }
                14 => {
                    changed.basic_blocks.as_mut()[yes].terminator_mut().kind =
                        body.basic_blocks[no].terminator().kind.clone()
                }
                15 => {
                    let scope = body.local_decls[Local::from_usize(0)].source_info.scope;
                    changed.source_scopes[scope].parent_scope = Some(scope);
                }
                16 => {
                    changed.local_decls[Local::from_usize(3)].ty = tcx.types.u8;
                }
                17 => {
                    changed.required_consts = Some(vec![ConstOperand {
                        span: DUMMY_SP,
                        user_ty: None,
                        const_: Const::from_bool(tcx, false),
                    }]);
                }
                18 => {
                    let items = changed.mentioned_items.as_mut().unwrap();
                    items.push(items[0].clone());
                }
                _ => unreachable!(),
            }
            reject(&changed, "shape, disposal, terminal, or metadata mutation");
        }
        for targets in [
            SwitchTargets::static_if(0, yes, no),
            SwitchTargets::static_if(0, no, no),
            SwitchTargets::static_if(0, BasicBlock::from_usize(4), yes),
            SwitchTargets::new([(0, no), (0, yes)].into_iter(), yes),
            SwitchTargets::static_if(2, no, yes),
        ] {
            let mut changed = body.clone();
            changed.basic_blocks.as_mut()[entry].terminator_mut().kind =
                TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Local::from_usize(1).into()),
                    targets,
                };
            reject(
                &changed,
                "wrong polarity, duplicate, or foreign switch targets",
            );
        }
        for operand in [
            Operand::Copy(Local::from_usize(2).into()),
            Operand::Move(Local::from_usize(1).into()),
            Operand::Constant(Box::new(ConstOperand {
                span: DUMMY_SP,
                user_ty: None,
                const_: Const::from_bool(tcx, true),
            })),
        ] {
            let mut changed = body.clone();
            let TerminatorKind::SwitchInt { discr, .. } =
                &mut changed.basic_blocks.as_mut()[entry].terminator_mut().kind
            else {
                unreachable!()
            };
            *discr = operand;
            reject(
                &changed,
                "predicate substitution, including equal bool payload",
            );
        }
        for mutation in 0..6 {
            let mut changed = body.clone();
            let statements = &mut changed.basic_blocks.as_mut()[yes].statements;
            match mutation {
                0 => {
                    let StatementKind::Assign(a) = &mut statements[1].kind else {
                        unreachable!()
                    };
                    a.1 = Rvalue::Use(Operand::Copy(Local::from_usize(2).into()));
                }
                1 => {
                    let StatementKind::Assign(a) = &mut statements[1].kind else {
                        unreachable!()
                    };
                    a.1 = Rvalue::Use(Operand::Move(Local::from_usize(1).into()));
                }
                2 => {
                    let StatementKind::Assign(a) = &mut statements[2].kind else {
                        unreachable!()
                    };
                    let Rvalue::Aggregate(_, operands) = &mut a.1 else {
                        unreachable!()
                    };
                    operands.raw[0] = Operand::Move(Local::from_usize(2).into());
                }
                3 => {
                    let StatementKind::Assign(a) = &mut statements[2].kind else {
                        unreachable!()
                    };
                    let Rvalue::Aggregate(kind, _) = &mut a.1 else {
                        unreachable!()
                    };
                    let AggregateKind::Adt(_, variant, _, _, _) = &mut **kind else {
                        unreachable!()
                    };
                    *variant = contract.none;
                }
                4 => statements[3].kind = StatementKind::StorageDead(Local::from_usize(0)),
                5 => statements.insert(2, statements[1].clone()),
                _ => unreachable!(),
            }
            reject(
                &changed,
                "payload copy, substitution, double move, or wrong variant/lifetime",
            );
        }
        for mutation in 0..7 {
            let mut changed = body.clone();
            let TerminatorKind::Drop {
                place,
                target,
                replace,
                drop,
                async_fut,
                ..
            } = &mut changed.basic_blocks.as_mut()[no].terminator_mut().kind
            else {
                unreachable!()
            };
            match mutation {
                0 => *place = Local::from_usize(0).into(),
                1 => *place = Local::from_usize(3).into(),
                2 => {
                    *place = Place {
                        local: Local::from_usize(2),
                        projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
                    }
                }
                3 => *target = entry,
                4 => *replace = true,
                5 => *drop = Some(done),
                6 => *async_fut = Some(Local::from_usize(2)),
                _ => unreachable!(),
            }
            reject(&changed, "wrong Drop place, target, or asynchronous form");
        }
        for mutation in 0..8 {
            let mut changed = body.clone();
            let StatementKind::Assign(assignment) =
                &mut changed.basic_blocks.as_mut()[yes].statements[2].kind
            else {
                unreachable!()
            };
            let Rvalue::Aggregate(kind, operands) = &mut assignment.1 else {
                unreachable!()
            };
            let AggregateKind::Adt(definition, _, args, user_ty, active_field) = &mut **kind else {
                unreachable!()
            };
            match mutation {
                0 => *definition = tcx.get_diagnostic_item(Symbol::intern("Result")).unwrap(),
                1 => *args = tcx.mk_args(&[tcx.types.u8.into()]),
                2 => operands.raw.clear(),
                3 => operands.raw.push(operands.raw[0].clone()),
                4 => operands.raw[0] = Operand::Copy(Local::from_usize(3).into()),
                5 => {
                    assignment.0.projection = tcx.mk_place_elems(&[ProjectionElem::Deref]);
                }
                6 => *user_ty = Some(rustc_middle::ty::UserTypeAnnotationIndex::from_usize(0)),
                7 => *active_field = Some(rustc_abi::FieldIdx::from_usize(0)),
                _ => unreachable!(),
            }
            reject(
                &changed,
                "aggregate identity, type, operand, or destination mutation",
            );
        }
        let mut direct = body.clone();
        let mut some = direct.basic_blocks[yes].statements[2].clone();
        let StatementKind::Assign(assignment) = &mut some.kind else {
            unreachable!()
        };
        let Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
            unreachable!()
        };
        operands.raw[0] = Operand::Move(Local::from_usize(2).into());
        direct.local_decls.raw.truncate(3);
        direct.basic_blocks.as_mut()[yes].statements = vec![some];
        assert!(
            reviewed_body(tcx, instance, &direct, &contract),
            "direct move-only form"
        );
        let StatementKind::Assign(assignment) =
            &mut direct.basic_blocks.as_mut()[yes].statements[0].kind
        else {
            unreachable!()
        };
        let Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
            unreachable!()
        };
        operands.raw[0] = Operand::Copy(Local::from_usize(2).into());
        reject(
            &direct,
            "compact form must still move, even for a Copy payload",
        );
        for unwind in [
            UnwindAction::Cleanup(done),
            UnwindAction::Cleanup(BasicBlock::from_usize(4)),
            UnwindAction::Terminate(UnwindTerminateReason::Abi),
            UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
        ] {
            let mut changed = body.clone();
            let TerminatorKind::Drop { unwind: edge, .. } =
                &mut changed.basic_blocks.as_mut()[no].terminator_mut().kind
            else {
                unreachable!()
            };
            *edge = unwind;
            reject(&changed, "unreviewed unwind edge");
        }
        let mut changed = body.clone();
        let TerminatorKind::Drop { unwind, .. } =
            &mut changed.basic_blocks.as_mut()[no].terminator_mut().kind
        else {
            unreachable!()
        };
        *unwind = UnwindAction::Unreachable;
        assert_eq!(
            reviewed_body(tcx, instance, &changed, &contract),
            !tcx.sess.panic_strategy().unwinds()
        );
        let panic_call = fixture_body(tcx, "trap")
            .basic_blocks
            .iter()
            .find(|block| matches!(block.terminator().kind, TerminatorKind::Call { .. }))
            .unwrap()
            .terminator()
            .kind
            .clone();
        for block in [entry, yes, no, done] {
            let mut changed = body.clone();
            changed.basic_blocks.as_mut()[block].terminator_mut().kind = panic_call.clone();
            reject(&changed, "live trap or substituted call");
        }
        for cleanup in [false, true] {
            let mut changed = body.clone();
            append_block(&mut changed, panic_call.clone(), cleanup);
            reject(&changed, "dead/cleanup trap or extra call");
        }
    }
}

struct CheckCallbacks {
    device: bool,
    completed: bool,
}
impl Callbacks for CheckCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.device {
            for caller in ["invocation", "completion", "matrix", "error"] {
                check_positive(tcx, caller);
            }
        } else {
            for caller in [
                "primitive",
                "equal_bool",
                "unit",
                "reference",
                "mutable_reference",
                "noncopy",
                "associated",
                "array",
                "nested",
                "user_destructor",
                "panicking_destructor",
            ] {
                check_positive(tcx, caller);
            }
            for caller in ["lazy", "impostor"] {
                assert!(!authenticate_reviewed_safe_core_bool_helper_v1(
                    tcx,
                    helper(tcx, caller)
                ));
            }
            check_mutations(tcx);
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn configured_path(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set cached {name}")));
    assert!(path.exists(), "cached {name}: {}", path.display());
    path
}

fn run_fixture(abort: bool, device: bool, amdgpu: bool) {
    let directory = TestTempDir::create("fe2o3-core-bool");
    let source = directory.path().join("fixture.rs");
    fs::write(&source, if device { DEVICE_SOURCE } else { SOURCE }).unwrap();
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_core_bool_fixture".into(),
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
    if device {
        let metadata = configured_path("FE2O3_CORE_TRY_DEVICE_RMETA");
        args.extend([
            "--extern".into(),
            format!("fe2o3_device={}", metadata.display()),
            "-L".into(),
            format!("dependency={}", metadata.parent().unwrap().display()),
        ]);
        if std::env::var_os("FE2O3_CORE_TRY_HOST_DEPS").is_some() {
            args.extend([
                "-L".into(),
                format!(
                    "dependency={}",
                    configured_path("FE2O3_CORE_TRY_HOST_DEPS").display()
                ),
            ]);
        }
    }
    if amdgpu {
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Zunstable-options".into(),
            "--extern".into(),
            format!(
                "noprelude,nounused:core={}",
                configured_path("FE2O3_CORE_TRY_AMDGPU_CORE").display()
            ),
            "--extern".into(),
            format!(
                "noprelude,nounused:compiler_builtins={}",
                configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS").display()
            ),
        ]);
    }
    let mut callbacks = CheckCallbacks {
        device,
        completed: false,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}

#[test]
fn core_bool_then_some_actual_mir_and_mutations_unwind() {
    run_fixture(false, false, false);
}
#[test]
fn core_bool_then_some_actual_mir_and_mutations_abort() {
    run_fixture(true, false, false);
}
#[test]
#[ignore = "requires existing cached device metadata and optional host dependency path"]
fn core_bool_then_some_actual_device_types_host_abort() {
    run_fixture(true, true, false);
}
#[test]
#[ignore = "requires existing cached AMD core/builtins/device metadata and source-target/debug/deps for host macros"]
fn core_bool_then_some_actual_device_types_amdgpu_abort() {
    run_fixture(true, true, true);
}

#[path = "drop_flag_tests.rs"]
mod drop_flag_tests;
