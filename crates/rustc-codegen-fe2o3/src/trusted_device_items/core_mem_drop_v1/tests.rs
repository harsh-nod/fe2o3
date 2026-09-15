use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::{
    mir::{
        BasicBlockData, Const, ConstOperand, Operand, Place, ProjectionElem, SourceInfo, Statement,
        StatementKind, Terminator, UnwindTerminateReason,
    },
    ty::TyKind,
};
use rustc_span::DUMMY_SP;
use std::{fs, path::PathBuf, process::Command};

const SOURCE: &str = r#"
#![no_std]
#![allow(dead_code, dropping_copy_types, dropping_references)]
pub struct NonCopy(pub u32);
pub struct Dropped;
impl Drop for Dropped { fn drop(&mut self) {} }
pub struct Panicking;
impl Drop for Panicking { fn drop(&mut self) { panic!("unchecked destructor") } }
pub trait Associated { type Type; }
impl Associated for NonCopy { type Type = u32; }
pub fn primitive(x: u32) { core::mem::drop(x) }
pub fn unit(x: ()) { core::mem::drop(x) }
pub fn reference(x: &u32) { core::mem::drop(x) }
pub fn mutable_reference(x: &mut u32) { core::mem::drop(x) }
pub fn noncopy(x: NonCopy) { core::mem::drop(x) }
pub fn associated(x: <NonCopy as Associated>::Type) { core::mem::drop(x) }
pub fn array(x: [NonCopy; 2]) { core::mem::drop(x) }
pub fn empty_option(x: Option<core::convert::Infallible>) { core::mem::drop(x) }
pub fn user_destructor(x: Dropped) { core::mem::drop(x) }
pub fn panicking_destructor(x: Panicking) { core::mem::drop(x) }
pub fn forgotten(x: NonCopy) { core::mem::forget(x) }
pub fn drop(_: u32) {}
pub fn impostor(x: u32) { drop(x) }
pub unsafe fn unsafe_drop(x: *mut NonCopy) { unsafe { core::ptr::drop_in_place(x) } }
pub fn trap() { panic!("unchecked function") }
"#;

const DEVICE_SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{
    CurrentTarget, Invocation3D, KernelCapabilityBrand, RegisteredLaunch, ReusablePhaseCompletion,
};
pub struct KernelMarker;
type Brand = KernelCapabilityBrand<'static, KernelMarker, CurrentTarget, RegisteredLaunch>;
pub fn tiled_invocation(x: Invocation3D<Brand>) { core::mem::drop(x) }
pub fn muon_completion(x: ReusablePhaseCompletion<'static, 'static, Brand>) { core::mem::drop(x) }
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
    let contract = contract(tcx, instance).expect("exact mem_drop identity and signature");
    assert!(
        authenticate_reviewed_safe_core_mem_drop_helper_v1(tcx, instance),
        "actual mem_drop: {caller}: {:#?}",
        tcx.instance_mir(instance.def),
    );
    assert!(
        !authenticate_reviewed_safe_core_mem_drop_helper_v1(tcx, contract.glue),
        "no source trust is transferred to drop glue",
    );
    assert!(
        matches!(
            tcx.instance_mir(instance.def).basic_blocks[BasicBlock::from_usize(0)]
                .terminator()
                .kind,
            TerminatorKind::Drop { .. }
        ),
        "original Drop remains a collector edge"
    );
    if caller.ends_with("destructor") {
        assert!(matches!(
            contract.glue.def,
            InstanceKind::DropGlue(_, Some(_))
        ));
        let TyKind::Adt(adt, _) = contract.input.kind() else {
            panic!("destructor ADT")
        };
        let destructor = adt.destructor(tcx).unwrap().did;
        assert!(destructor.is_local());
        assert!(
            !authenticate_reviewed_safe_core_mem_drop_helper_v1(
                tcx,
                Instance::mono(tcx, destructor),
            ),
            "even a safe or empty user destructor needs independent admission"
        );
    }
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

fn check_mutations(tcx: TyCtxt<'_>) {
    let instance = helper(tcx, "primitive");
    let contract = contract(tcx, instance).unwrap();
    let body = tcx.instance_mir(instance.def);
    assert!(reviewed_body(tcx, instance, body, &contract));
    let first = BasicBlock::from_usize(0);
    let last = BasicBlock::from_usize(1);
    for mutation in 0..14 {
        let mut changed = body.clone();
        match mutation {
            0 => changed.arg_count = 2,
            1 => changed.local_decls[Local::from_usize(0)].ty = tcx.types.u32,
            2 => changed.local_decls[Local::from_usize(1)].ty = tcx.types.u8,
            3 => {
                changed
                    .local_decls
                    .push(body.local_decls[Local::from_usize(0)].clone());
            }
            4 => append_block(&mut changed, TerminatorKind::Unreachable, false),
            5 => changed.basic_blocks.as_mut()[first]
                .statements
                .push(Statement::new(
                    SourceInfo::outermost(DUMMY_SP),
                    StatementKind::Nop,
                )),
            6 => changed.basic_blocks.as_mut()[first].terminator = None,
            7 => changed.basic_blocks.as_mut()[last].is_cleanup = true,
            8 => {
                let scope = body.local_decls[Local::from_usize(0)].source_info.scope;
                changed
                    .source_scopes
                    .push(body.source_scopes[scope].clone());
            }
            9 => changed.source.instance = helper(tcx, "forgotten").def,
            10 => {
                changed.required_consts = Some(vec![ConstOperand {
                    span: DUMMY_SP,
                    user_ty: None,
                    const_: Const::from_bool(tcx, false),
                }])
            }
            11 => changed.mentioned_items = None,
            12 => {
                let items = changed.mentioned_items.as_mut().unwrap();
                items.push(items[0].clone());
            }
            13 => {
                changed.mentioned_items.as_mut().unwrap()[0].node =
                    MentionedItem::Drop(tcx.types.u8)
            }
            _ => unreachable!(),
        }
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "body mutation {mutation}"
        );
    }
    for mutation in 0..9 {
        let mut changed = body.clone();
        let TerminatorKind::Drop {
            place,
            target,
            replace,
            drop,
            async_fut,
            ..
        } = &mut changed.basic_blocks.as_mut()[first].terminator_mut().kind
        else {
            panic!("Drop")
        };
        match mutation {
            0 => *place = Local::from_usize(0).into(),
            1 => *place = Local::from_usize(2).into(),
            2 => {
                *place = Place {
                    local: Local::from_usize(1),
                    projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
                }
            }
            3 => *target = first,
            4 => *target = BasicBlock::from_usize(2),
            5 => *replace = true,
            6 => *drop = Some(last),
            7 => *async_fut = Some(Local::from_usize(1)),
            8 => changed.basic_blocks.as_mut()[first].is_cleanup = true,
            _ => unreachable!(),
        }
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "Drop mutation {mutation}"
        );
    }
    for unwind in [
        UnwindAction::Cleanup(last),
        UnwindAction::Cleanup(BasicBlock::from_usize(2)),
        UnwindAction::Terminate(UnwindTerminateReason::Abi),
        UnwindAction::Terminate(UnwindTerminateReason::InCleanup),
    ] {
        let mut changed = body.clone();
        let TerminatorKind::Drop { unwind: edge, .. } =
            &mut changed.basic_blocks.as_mut()[first].terminator_mut().kind
        else {
            unreachable!()
        };
        *edge = unwind;
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "unreviewed unwind {unwind:?}"
        );
    }
    let mut changed = body.clone();
    let TerminatorKind::Drop { unwind, .. } =
        &mut changed.basic_blocks.as_mut()[first].terminator_mut().kind
    else {
        unreachable!()
    };
    *unwind = UnwindAction::Unreachable;
    assert_eq!(
        reviewed_body(tcx, instance, &changed, &contract),
        !tcx.sess.panic_strategy().unwinds()
    );
    for kind in [
        TerminatorKind::Return,
        TerminatorKind::Goto { target: last },
        TerminatorKind::Unreachable,
    ] {
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[first].terminator_mut().kind = kind;
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "cannot omit Drop even for u32"
        );
    }
    for kind in [
        TerminatorKind::Unreachable,
        TerminatorKind::UnwindResume,
        TerminatorKind::Goto { target: first },
    ] {
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[last].terminator_mut().kind = kind;
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "return cannot trap or cycle"
        );
    }
    let panic_call = fixture_body(tcx, "trap")
        .basic_blocks
        .iter()
        .find(|block| matches!(block.terminator().kind, TerminatorKind::Call { .. }))
        .unwrap()
        .terminator()
        .kind
        .clone();
    for cleanup in [false, true] {
        let mut changed = body.clone();
        append_block(&mut changed, panic_call.clone(), cleanup);
        assert!(
            !reviewed_body(tcx, instance, &changed, &contract),
            "dead/cleanup trap"
        );
    }
    let wrong = Contract {
        input: contract.input,
        glue: Instance::resolve_drop_in_place(tcx, tcx.types.u64),
    };
    assert!(
        !reviewed_body(tcx, instance, body, &wrong),
        "substituted glue specialization"
    );
}

struct CheckCallbacks {
    device: bool,
    completed: bool,
}
impl Callbacks for CheckCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.device {
            for caller in ["tiled_invocation", "muon_completion"] {
                check_positive(tcx, caller);
            }
        } else {
            for caller in [
                "primitive",
                "unit",
                "reference",
                "mutable_reference",
                "noncopy",
                "associated",
                "array",
                "empty_option",
                "user_destructor",
                "panicking_destructor",
            ] {
                check_positive(tcx, caller);
            }
            for caller in ["forgotten", "impostor", "unsafe_drop"] {
                assert!(!authenticate_reviewed_safe_core_mem_drop_helper_v1(
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
    let directory = TestTempDir::create("fe2o3-core-mem-drop");
    let source = directory.path().join("fixture.rs");
    fs::write(&source, if device { DEVICE_SOURCE } else { SOURCE }).unwrap();
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_core_mem_drop_fixture".into(),
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
fn core_mem_drop_exact_wrapper_and_mutations_unwind() {
    run_fixture(false, false, false);
}

#[test]
fn core_mem_drop_exact_wrapper_and_mutations_abort() {
    run_fixture(true, false, false);
}

#[test]
#[ignore = "requires existing cached FE2O3_CORE_TRY_DEVICE_RMETA and optional host dependency path"]
fn core_mem_drop_actual_device_types_host_abort() {
    run_fixture(true, true, false);
}

#[test]
#[ignore = "requires existing cached AMD core/builtins/device metadata and host dependency path"]
fn core_mem_drop_actual_device_types_amdgpu_abort() {
    run_fixture(true, true, true);
}
