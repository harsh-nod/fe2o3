use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{
    AssertKind, CallSource, Const, ConstOperand, ConstValue, ProjectionElem, Promoted, SourceInfo,
    SourceScope, Statement, UnwindAction,
};
use rustc_middle::ty;
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

const SOURCE: &str = r#"
#![no_std]
#![allow(dead_code, unused_unsafe)]
pub enum Error { Bounds, Size }
pub struct Owned(pub u64);
impl Drop for Owned { fn drop(&mut self) {} }
pub fn scalar(a: u32) -> u32 { u32::from(a) }
pub fn error(a: Error) -> Error { Error::from(a) }
pub fn owned(a: Owned) -> Owned { Owned::from(a) }
pub fn reference(a: &mut Owned) -> &mut Owned { <&mut Owned>::from(a) }
pub fn slice_reference(a: &str) -> &str { <&str>::from(a) }
pub fn tuple(a: (Owned, Error)) -> (Owned, Error) { <(Owned, Error)>::from(a) }
pub fn array(a: [Owned; 2]) -> [Owned; 2] { <[Owned; 2]>::from(a) }
pub fn unit(a: ()) { <()>::from(a) }
pub fn pointer(a: *mut Owned) -> *mut Owned { <*mut Owned>::from(a) }
pub fn closure(a: Owned) -> impl FnOnce() {
    let mut c = move || core::mem::drop(a);
    c = From::from(c);
    c
}
pub fn converted(a: u32) -> u64 { u64::from(a) }
pub fn into(a: Owned) -> Owned { a.into() }
pub fn other_core(a: Owned) -> Owned { core::convert::identity(a) }
pub fn from<T>(a: T) -> T { a }
pub fn foreign(a: Owned) -> Owned { from(a) }
unsafe fn unsafe_identity<T>(a: T) -> T { a }
pub fn unsafe_route(a: Owned) -> Owned { unsafe { unsafe_identity(a) } }
fn hidden_unsafe<T>(a: T) -> T { unsafe { a } }
pub fn hidden_unsafe_route(a: Owned) -> Owned { hidden_unsafe(a) }
mod fake_core {
    pub trait From<T> { fn from(a: T) -> Self; }
    impl<T> From<T> for T { fn from(a: T) -> Self { a } }
}
pub fn foreign_trait(a: Owned) -> Owned { <Owned as fake_core::From<Owned>>::from(a) }
pub async fn coroutine_header() {}
"#;

const POSITIVES: &[&str] = &[
    "scalar",
    "error",
    "owned",
    "reference",
    "slice_reference",
    "tuple",
    "array",
    "unit",
    "pointer",
    "closure",
];

#[derive(Clone, Copy)]
enum Check {
    Identity,
    Moves,
    Headers,
    Bounds,
}

struct Probe {
    check: Check,
    mir_opt: u8,
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("core_identity_authentication.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let called = |name: &str| {
            let local = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap();
            tcx.optimized_mir(local)
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
                    let TyKind::FnDef(definition, arguments) = *callee.const_.ty().kind() else {
                        return None;
                    };
                    Instance::try_resolve(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                        definition,
                        arguments,
                    )
                    .ok()?
                })
                .unwrap_or_else(|| panic!("retained call: {name}, MIR opt {}", self.mir_opt))
        };
        for name in POSITIVES {
            let instance = called(name);
            assert!(
                authenticate_reviewed_safe_core_identity_helper_v1(tcx, instance),
                "{name}"
            );
        }
        let instance = called("owned");
        let source = tcx.instance_mir(instance.def);
        let parameter = identity(tcx, instance).unwrap();
        let accepts = |body: &Body<'tcx>| reviewed_body(tcx, instance, body, parameter);
        match self.check {
            Check::Identity => {
                let before = format!("{source:?}");
                for name in POSITIVES {
                    let concrete = called(name);
                    assert_eq!(concrete.def, instance.def, "same generic impl: {name}");
                    assert!(std::ptr::eq(tcx.instance_mir(concrete.def), source));
                    let value = concrete.args[0].expect_ty();
                    if ["owned", "reference", "tuple", "array", "closure"].contains(name) {
                        assert!(
                            !tcx.type_is_copy_modulo_regions(
                                TypingEnv::fully_monomorphized(),
                                value
                            ),
                            "{name}"
                        );
                    }
                    for _ in 0..2 {
                        assert!(authenticate_reviewed_safe_core_identity_helper_v1(
                            tcx, concrete
                        ));
                    }
                }
                assert_eq!(
                    format!("{source:?}"),
                    before,
                    "authentication never rewrites MIR"
                );
                for name in [
                    "converted",
                    "into",
                    "other_core",
                    "foreign",
                    "foreign_trait",
                    "unsafe_route",
                    "hidden_unsafe_route",
                ] {
                    assert!(
                        !authenticate_reviewed_safe_core_identity_helper_v1(tcx, called(name)),
                        "{name}"
                    );
                }
                for name in ["foreign", "foreign_trait", "hidden_unsafe_route"] {
                    let foreign = called(name);
                    assert!(
                        reviewed_body(tcx, foreign, tcx.instance_mir(foreign.def), parameter),
                        "same closed body does not grant nominal core authority: {name}"
                    );
                }
                for arguments in [
                    vec![],
                    vec![tcx.types.u32.into(), tcx.types.u32.into()],
                    vec![tcx.lifetimes.re_erased.into()],
                    vec![parameter.into()],
                    vec![tcx.types.str_.into()],
                    vec![Ty::new_slice(tcx, tcx.types.u8).into()],
                ] {
                    assert!(
                        !authenticate_reviewed_safe_core_identity_helper_v1(
                            tcx,
                            Instance {
                                def: instance.def,
                                args: tcx.mk_args(&arguments),
                            }
                        ),
                        "wrong/unresolved/unsized arguments: {arguments:?}"
                    );
                }
                assert!(!authenticate_reviewed_safe_core_identity_helper_v1(
                    tcx,
                    Instance {
                        def: InstanceKind::Intrinsic(instance.def_id()),
                        args: instance.args,
                    }
                ));
                let signature = tcx.instantiate_bound_regions_with_erased(
                    tcx.fn_sig(instance.def_id()).instantiate_identity(),
                );
                for changed in [
                    FnSig {
                        safety: Safety::Unsafe,
                        ..signature
                    },
                    FnSig {
                        abi: ExternAbi::RustCall,
                        ..signature
                    },
                    FnSig {
                        abi: ExternAbi::C { unwind: false },
                        ..signature
                    },
                    FnSig {
                        c_variadic: true,
                        ..signature
                    },
                    FnSig {
                        inputs_and_output: tcx.mk_type_list(&[parameter, tcx.types.u64]),
                        ..signature
                    },
                    FnSig {
                        inputs_and_output: tcx.mk_type_list(&[parameter, parameter, parameter]),
                        ..signature
                    },
                ] {
                    assert!(!signature_matches(changed, parameter));
                }
            }
            Check::Moves => {
                for mutation in 0..9 {
                    let mut changed = source.clone();
                    let StatementKind::Assign(assignment) =
                        &mut changed.basic_blocks_mut()[START_BLOCK].statements[0].kind
                    else {
                        unreachable!()
                    };
                    match mutation {
                        0 => assignment.1 = Rvalue::Use(Operand::Copy(Local::from_usize(1).into())),
                        1 => assignment.1 = Rvalue::Use(Operand::Move(RETURN_PLACE.into())),
                        2 => assignment.0 = Local::from_usize(1).into(),
                        3 => assignment.1 = Rvalue::Use(Operand::Move(Local::from_usize(2).into())),
                        4 => changed.local_decls[RETURN_PLACE].ty = tcx.types.u64,
                        5 => changed.local_decls[Local::from_usize(1)].ty = tcx.types.u64,
                        6 => {
                            assignment.1 = Rvalue::Use(Operand::Constant(Box::new(ConstOperand {
                                span: DUMMY_SP,
                                user_ty: None,
                                const_: Const::from_bool(tcx, false),
                            })))
                        }
                        7 => {
                            assignment.1 = Rvalue::Use(Operand::Move(
                                rustc_middle::mir::Place::from(Local::from_usize(1))
                                    .project_deeper(&[ProjectionElem::Deref], tcx),
                            ))
                        }
                        8 => {
                            assignment.0 =
                                assignment.0.project_deeper(&[ProjectionElem::Deref], tcx)
                        }
                        _ => unreachable!(),
                    }
                    assert!(!accepts(&changed), "changed ownership/type: {mutation}");
                }
                for kind in [
                    StatementKind::Nop,
                    StatementKind::StorageLive(Local::from_usize(1)),
                    StatementKind::StorageDead(Local::from_usize(1)),
                    source.basic_blocks[START_BLOCK].statements[0].kind.clone(),
                ] {
                    let mut changed = source.clone();
                    changed.basic_blocks_mut()[START_BLOCK]
                        .statements
                        .push(Statement::new(SourceInfo::outermost(DUMMY_SP), kind));
                    assert!(!accepts(&changed), "extra statement/use after move");
                }
                for kind in [
                    TerminatorKind::Unreachable,
                    TerminatorKind::Goto {
                        target: START_BLOCK,
                    },
                    TerminatorKind::Drop {
                        place: RETURN_PLACE.into(),
                        target: START_BLOCK,
                        unwind: UnwindAction::Unreachable,
                        replace: false,
                        drop: None,
                        async_fut: None,
                    },
                ] {
                    let mut changed = source.clone();
                    changed.basic_blocks_mut()[START_BLOCK]
                        .terminator_mut()
                        .kind = kind;
                    assert!(!accepts(&changed), "changed return/effect");
                }
                for unwind in [
                    UnwindAction::Unreachable,
                    UnwindAction::Continue,
                    UnwindAction::Cleanup(START_BLOCK),
                ] {
                    let mut changed = source.clone();
                    let foreign = called("foreign");
                    changed.basic_blocks_mut()[START_BLOCK]
                        .terminator_mut()
                        .kind = TerminatorKind::Call {
                        func: Operand::Constant(Box::new(ConstOperand {
                            span: DUMMY_SP,
                            user_ty: None,
                            const_: Const::Val(
                                ConstValue::ZeroSized,
                                Ty::new_fn_def(tcx, foreign.def_id(), foreign.args),
                            ),
                        })),
                        args: Box::new([]),
                        destination: RETURN_PLACE.into(),
                        target: None,
                        unwind,
                        call_source: CallSource::Normal,
                        fn_span: DUMMY_SP,
                    };
                    assert!(!accepts(&changed), "no callee/unwind waiver");
                    changed.basic_blocks_mut()[START_BLOCK]
                        .terminator_mut()
                        .kind = TerminatorKind::Assert {
                        cond: Operand::Constant(Box::new(ConstOperand {
                            span: DUMMY_SP,
                            user_ty: None,
                            const_: Const::from_bool(tcx, true),
                        })),
                        expected: true,
                        msg: Box::new(AssertKind::DivisionByZero(Operand::Move(
                            Local::from_usize(1).into(),
                        ))),
                        target: START_BLOCK,
                        unwind,
                    };
                    assert!(!accepts(&changed), "no constant assertion waiver");
                }
            }
            Check::Headers => {
                for mutation in 0..13 {
                    let mut changed = source.clone();
                    match mutation {
                        0 => changed.source.instance = called("foreign").def,
                        1 => changed.source.instance = InstanceKind::Intrinsic(instance.def_id()),
                        2 => changed.source.promoted = Some(Promoted::from_usize(0)),
                        3 => changed.phase = MirPhase::Built,
                        4 => changed.injection_phase = Some(MirPhase::Built),
                        5 => changed.is_polymorphic = false,
                        6 => changed.spread_arg = Some(Local::from_usize(1)),
                        7 => changed.arg_count = 0,
                        8 => changed.required_consts = None,
                        9 => {
                            changed.required_consts = Some(vec![ConstOperand {
                                span: DUMMY_SP,
                                user_ty: None,
                                const_: Const::from_bool(tcx, false),
                            }])
                        }
                        10 => changed.basic_blocks_mut()[START_BLOCK].is_cleanup = true,
                        11 => changed.basic_blocks_mut()[START_BLOCK].terminator = None,
                        12 => {
                            #[allow(deprecated)]
                            let error = rustc_span::ErrorGuaranteed::unchecked_error_guaranteed();
                            changed.tainted_by_errors = Some(error);
                        }
                        _ => unreachable!(),
                    }
                    assert!(!accepts(&changed), "header mutation={mutation}");
                }
                let coroutine = tcx
                    .iter_local_def_id()
                    .filter(|id| tcx.def_kind(*id) == DefKind::Closure)
                    .find_map(|id| tcx.optimized_mir(id).coroutine.clone())
                    .unwrap();
                let mut changed = source.clone();
                changed.coroutine = Some(coroutine);
                assert!(!accepts(&changed));
                for mutation in 0..6 {
                    let mut changed = source.clone();
                    let wrong = SourceScope::from_usize(1);
                    match mutation {
                        0 => changed.source_scopes.raw[0].parent_scope = Some(wrong),
                        1 => changed.source_scopes.raw[0].inlined = Some((instance, DUMMY_SP)),
                        2 => changed.source_scopes.raw[0].inlined_parent_scope = Some(wrong),
                        3 => changed.local_decls[RETURN_PLACE].source_info.scope = wrong,
                        4 => {
                            changed.basic_blocks_mut()[START_BLOCK].statements[0]
                                .source_info
                                .scope = wrong
                        }
                        5 => {
                            changed.basic_blocks_mut()[START_BLOCK]
                                .terminator_mut()
                                .source_info
                                .scope = wrong
                        }
                        _ => unreachable!(),
                    }
                    assert!(!accepts(&changed), "scope mutation={mutation}");
                }
            }
            Check::Bounds => {
                for count in [1, 2, 3] {
                    let mut changed = source.clone();
                    changed
                        .local_decls
                        .raw
                        .resize(count, source.local_decls[RETURN_PLACE].clone());
                    assert_eq!(accepts(&changed), count == 2, "locals={count}");
                }
                for count in [0, 1, 2] {
                    let mut changed = source.clone();
                    changed
                        .basic_blocks_mut()
                        .raw
                        .resize(count, source.basic_blocks[START_BLOCK].clone());
                    assert_eq!(accepts(&changed), count == 1, "blocks={count}");
                    let mut changed = source.clone();
                    changed
                        .source_scopes
                        .raw
                        .resize(count, source.source_scopes.raw[0].clone());
                    assert_eq!(accepts(&changed), count == 1, "scopes={count}");
                    let mut changed = source.clone();
                    changed.basic_blocks_mut()[START_BLOCK].statements.resize(
                        count,
                        source.basic_blocks[START_BLOCK].statements[0].clone(),
                    );
                    assert_eq!(accepts(&changed), count == 1, "statements={count}");
                }
                let debug = source.var_debug_info.first().unwrap();
                for count in [MAX_DEBUG_INFO - 1, MAX_DEBUG_INFO, MAX_DEBUG_INFO + 1] {
                    let mut changed = source.clone();
                    changed.var_debug_info = vec![debug.clone(); count];
                    assert_eq!(
                        accepts(&changed),
                        count <= MAX_DEBUG_INFO,
                        "debug entries={count}"
                    );
                }
                let mut changed = source.clone();
                changed
                    .user_type_annotations
                    .push(ty::CanonicalUserTypeAnnotation {
                        user_ty: Box::new(ty::CanonicalUserType {
                            max_universe: ty::UniverseIndex::ROOT,
                            var_kinds: ty::List::empty(),
                            value: ty::UserType::new(ty::UserTypeKind::Ty(tcx.types.u64)),
                        }),
                        span: DUMMY_SP,
                        inferred_ty: tcx.types.u64,
                    });
                assert!(!accepts(&changed), "unexpected annotation");
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(check: Check, mir_opt: u8) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=core_identity_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Cpanic=abort".into(),
        format!("-Zmir-opt-level={mir_opt}"),
        "-".into(),
    ];
    let mut probe = Probe {
        check,
        mir_opt,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn exact_core_identity_and_normalized_noncopy_instances() {
    run(Check::Identity, 0);
}
#[test]
fn exact_core_identity_at_normal_mir_optimization() {
    run(Check::Identity, 2);
}
#[test]
fn identity_requires_one_whole_value_move_and_return() {
    run(Check::Moves, 0);
}
#[test]
fn identity_rejects_source_header_scope_and_error_mutations() {
    run(Check::Headers, 0);
}
#[test]
fn identity_cardinality_and_metadata_boundaries() {
    run(Check::Bounds, 0);
}
