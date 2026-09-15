use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{SourceInfo, Statement, SwitchTargets};
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

#[path = "state_tests.rs"]
mod state_tests;

const SOURCE: &str = r#"
#![no_std]
#![allow(dead_code)]
macro_rules! ne { ($name:ident, $ty:ty) => {
    pub fn $name(a: &$ty, b: &$ty) -> bool { <$ty as PartialEq>::ne(a, b) }
}; }
ne!(ne_bool, bool); ne!(ne_char, char); ne!(ne_usize, usize);
ne!(ne_u8, u8); ne!(ne_u16, u16); ne!(ne_u32, u32); ne!(ne_u64, u64); ne!(ne_u128, u128);
ne!(ne_isize, isize); ne!(ne_i8, i8); ne!(ne_i16, i16); ne!(ne_i32, i32); ne!(ne_i64, i64); ne!(ne_i128, i128);
ne!(ne_f32, f32); ne!(ne_f64, f64);
macro_rules! eq { ($name:ident, $ty:ty) => {
    pub fn $name(a: &$ty, b: &$ty) -> bool { <$ty as PartialEq>::eq(a, b) }
}; }
eq!(eq_bool, bool); eq!(eq_char, char); eq!(eq_usize, usize);
eq!(eq_u8, u8); eq!(eq_u16, u16); eq!(eq_u32, u32); eq!(eq_u64, u64); eq!(eq_u128, u128);
eq!(eq_isize, isize); eq!(eq_i8, i8); eq!(eq_i16, i16); eq!(eq_i32, i32); eq!(eq_i64, i64); eq!(eq_i128, i128);
eq!(eq_f32, f32); eq!(eq_f64, f64);
pub fn from_u32(a: u32) -> u64 { u64::from(a) }
pub fn from_u8(a: u8) -> f32 { f32::from(a) }
pub fn wrong_eq(a: &u32, b: &u32) -> bool { <u32 as PartialEq>::eq(a, b) }
pub fn wrong_ref(a: &&u32, b: &&u32) -> bool { <&u32 as PartialEq>::ne(a, b) }
pub fn wrong_unit(a: &(), b: &()) -> bool { <() as PartialEq>::ne(a, b) }
pub fn wrong_ref_eq(a: &&u64, b: &&u64) -> bool { <&u64 as PartialEq>::eq(a, b) }
pub fn wrong_unit_eq(a: &(), b: &()) -> bool { <() as PartialEq>::eq(a, b) }
pub fn wrong_option_eq(a: &Option<u64>, b: &Option<u64>) -> bool { Option::<u64>::eq(a, b) }
pub fn wrong_width(a: u16) -> f32 { f32::from(a) }
pub fn wrong_target(a: u32) -> f64 { f64::from(a) }
pub fn wrong_identity(a: u32) -> u32 { u32::from(a) }
pub struct Impostor(u32);
impl PartialEq for Impostor {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
    fn ne(&self, other: &Self) -> bool { self.0 != other.0 }
}
impl From<u32> for Impostor { fn from(a: u32) -> Self { Self(a) } }
pub fn foreign_ne(a: &Impostor, b: &Impostor) -> bool { Impostor::ne(a, b) }
pub fn foreign_eq(a: &Impostor, b: &Impostor) -> bool { Impostor::eq(a, b) }
pub fn foreign_from(a: u32) -> Impostor { Impostor::from(a) }
pub fn fake_panic(_: &str) -> ! { loop {} }
pub const FOREIGN_MAX: u32 = u32::MAX;
pub fn foreign_constant() -> u32 { FOREIGN_MAX }
pub fn route_u32(small: u32) -> u64 {
    debug_assert!(u64::MIN as i128 <= u32::MIN as i128);
    debug_assert!(u32::MAX as u128 <= u64::MAX as u128);
    small as u64
}
pub fn route_u8(small: u8) -> f32 {
    debug_assert!(f32::MIN as i128 <= u8::MIN as i128);
    debug_assert!(u8::MAX as u128 <= f32::MAX as u128);
    small as f32
}
"#;

const NE: &[&str] = &[
    "ne_bool", "ne_char", "ne_usize", "ne_u8", "ne_u16", "ne_u32", "ne_u64", "ne_u128", "ne_isize",
    "ne_i8", "ne_i16", "ne_i32", "ne_i64", "ne_i128", "ne_f32", "ne_f64",
];

const EQ: &[&str] = &[
    "eq_bool", "eq_char", "eq_usize", "eq_u8", "eq_u16", "eq_u32", "eq_u64", "eq_u128", "eq_isize",
    "eq_i8", "eq_i16", "eq_i32", "eq_i64", "eq_i128", "eq_f32", "eq_f64",
];

#[derive(Clone, Copy)]
enum Check {
    Equality,
    Identity,
    Bodies,
    Operations,
    Branches,
    Closed,
    Constants,
    Production,
    Limits,
    State,
}
struct Probe {
    check: Check,
    debug: bool,
    ran: bool,
}

fn resolve<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<Instance<'tcx>> {
    let Operand::Constant(constant) = operand else {
        return None;
    };
    let TyKind::FnDef(definition, arguments) = *constant.const_.ty().kind() else {
        return None;
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), definition, arguments).ok()?
}

fn local_operand<'tcx>(index: usize) -> Operand<'tcx> {
    Operand::Copy(Local::from_usize(index).into())
}

fn bool_operand<'tcx>(tcx: TyCtxt<'tcx>, value: bool) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::from_bool(tcx, value),
    }))
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("primitive_value_authentication.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let definition = |name: &str| {
            tcx.iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap()
        };
        let body = |name: &str| tcx.optimized_mir(definition(name));
        let called = |name: &str| {
            body(name)
                .basic_blocks
                .iter()
                .find_map(|block| match &block.terminator().kind {
                    TerminatorKind::Call { func, .. } => resolve(tcx, func),
                    _ => None,
                })
                .unwrap()
        };
        let routes = [
            ("route_u32", Helper::U32ToU64),
            ("route_u8", Helper::U8ToF32),
        ];
        match self.check {
            Check::Equality => state_tests::primitive_equality(tcx, &called, &body),
            Check::Limits => {
                for name in ["from_u32", "from_u8"] {
                    let instance = called(name);
                    let helper = helper_identity(tcx, instance).unwrap();
                    let proof = prove_core_primitive_cast_v1(tcx, instance).unwrap();
                    state_tests::resource_limits(tcx, &proof.expand_mir(tcx), helper);
                }
            }
            Check::State => {
                for (name, helper) in routes {
                    state_tests::state_transfers(tcx, body(name), helper);
                }
            }
            Check::Production => super::production::tests::check(tcx, &called, &body),
            Check::Identity => {
                for name in NE.iter().copied().chain(["from_u32", "from_u8"]) {
                    let instance = called(name);
                    let helper = helper_identity(tcx, instance).expect(name);
                    assert!(
                        authenticate_reviewed_safe_core_primitive_value_helper_v1(tcx, instance),
                        "{name}"
                    );
                    let signature = tcx.instantiate_bound_regions_with_erased(
                        tcx.fn_sig(instance.def_id())
                            .instantiate(tcx, instance.args),
                    );
                    assert!(signature_matches(tcx, helper, signature));
                    for wrong in [
                        FnSig {
                            safety: Safety::Unsafe,
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
                            inputs_and_output: tcx.mk_type_list(&[tcx.types.u8, tcx.types.u64]),
                            ..signature
                        },
                    ] {
                        assert!(!signature_matches(tcx, helper, wrong), "{name}");
                    }
                    assert!(
                        helper_identity(
                            tcx,
                            Instance::new_raw(
                                instance.def_id(),
                                tcx.mk_args(&[tcx.types.u32.into()])
                            )
                        )
                        .is_none()
                    );
                    assert!(
                        helper_identity(
                            tcx,
                            Instance {
                                def: InstanceKind::Intrinsic(instance.def_id()),
                                args: instance.args
                            }
                        )
                        .is_none()
                    );
                }
                for name in [
                    "wrong_ref",
                    "wrong_unit",
                    "wrong_width",
                    "wrong_target",
                    "wrong_identity",
                    "foreign_ne",
                    "foreign_from",
                ] {
                    assert!(helper_identity(tcx, called(name)).is_none(), "{name}");
                    assert!(
                        !authenticate_reviewed_safe_core_primitive_value_helper_v1(
                            tcx,
                            called(name)
                        ),
                        "{name}"
                    );
                }
            }
            Check::Bodies => {
                for (name, helper) in routes {
                    let route = body(name);
                    assert!(reviewed_body(tcx, route, helper), "{name}");
                    assert!(
                        helper_identity(tcx, Instance::mono(tcx, definition(name).to_def_id()))
                            .is_none(),
                        "matching body does not authenticate local identity"
                    );
                    let panics = route
                        .basic_blocks
                        .iter()
                        .filter(|block| {
                            matches!(block.terminator().kind, TerminatorKind::Call { .. })
                        })
                        .count();
                    assert_eq!(panics, 2, "MIR opt level zero retains both cfg branches");
                    let cfg = route.basic_blocks[BasicBlock::from_usize(0)]
                        .statements
                        .iter()
                        .find_map(|statement| {
                            let StatementKind::Assign(assignment) = &statement.kind else {
                                return None;
                            };
                            let Rvalue::Use(Operand::Constant(value)) = &assignment.1 else {
                                return None;
                            };
                            constant(tcx, value)
                        })
                        .unwrap();
                    assert_eq!(
                        cfg,
                        Value {
                            ty: tcx.types.bool,
                            kind: Kind::Bits(u128::from(self.debug))
                        }
                    );
                    assert!(!reviewed_body(
                        tcx,
                        route,
                        if helper == Helper::U32ToU64 {
                            Helper::U8ToF32
                        } else {
                            Helper::U32ToU64
                        }
                    ));
                }
            }
            Check::Operations => {
                let cases = NE
                    .iter()
                    .map(|name| {
                        let instance = called(name);
                        (
                            tcx.instance_mir(instance.def),
                            helper_identity(tcx, instance).unwrap(),
                        )
                    })
                    .chain(routes.iter().map(|(name, helper)| (body(name), *helper)));
                for (actual, helper) in cases {
                    assert!(reviewed_body(tcx, actual, helper));
                    let mut binaries = 0;
                    let mut casts = 0;
                    for (block, data) in actual.basic_blocks.iter_enumerated() {
                        for (index, statement) in data.statements.iter().enumerate() {
                            let StatementKind::Assign(assignment) = &statement.kind else {
                                continue;
                            };
                            let mut changed = actual.clone();
                            let StatementKind::Assign(changed_assignment) =
                                &mut changed.basic_blocks_mut()[block].statements[index].kind
                            else {
                                unreachable!()
                            };
                            match &assignment.1 {
                                Rvalue::BinaryOp(_, _) => {
                                    binaries += 1;
                                    let Rvalue::BinaryOp(op, _) = &mut changed_assignment.1 else {
                                        unreachable!()
                                    };
                                    *op = BinOp::Gt;
                                }
                                Rvalue::Cast(_, _, _) => {
                                    casts += 1;
                                    let Rvalue::Cast(kind, _, _) = &mut changed_assignment.1 else {
                                        unreachable!()
                                    };
                                    *kind = CastKind::Transmute;
                                }
                                Rvalue::Use(Operand::Copy(place))
                                    if !place.projection.is_empty() =>
                                {
                                    changed_assignment.1 = Rvalue::Use(Operand::Move(*place));
                                }
                                _ => continue,
                            }
                            assert!(
                                !reviewed_body(tcx, &changed, helper),
                                "operation {helper:?} {block:?}:{index}"
                            );
                            let mut changed = actual.clone();
                            let StatementKind::Assign(changed_assignment) =
                                &mut changed.basic_blocks_mut()[block].statements[index].kind
                            else {
                                unreachable!()
                            };
                            match &mut changed_assignment.1 {
                                Rvalue::BinaryOp(_, operands)
                                    if matches!(helper, Helper::Ne(_)) =>
                                {
                                    operands.1 = operands.0.clone()
                                }
                                Rvalue::Cast(_, input, _) => *input = local_operand(0),
                                Rvalue::Use(Operand::Copy(place)) => {
                                    place.local = Local::from_usize(0)
                                }
                                _ => continue,
                            }
                            assert!(
                                !reviewed_body(tcx, &changed, helper),
                                "operand {helper:?} {block:?}:{index}"
                            );
                        }
                    }
                    assert_eq!(
                        (binaries, casts),
                        if matches!(helper, Helper::Ne(_)) {
                            (1, 0)
                        } else {
                            (2, 5)
                        }
                    );
                }
            }
            Check::Branches => {
                for (name, helper) in routes {
                    let actual = body(name);
                    let mut panics = 0;
                    let mut switches = 0;
                    for (block, data) in actual.basic_blocks.iter_enumerated() {
                        match &data.terminator().kind {
                            TerminatorKind::Call { .. } => {
                                panics += 1;
                                for unwind in [UnwindAction::Continue, UnwindAction::Cleanup(block)]
                                {
                                    let mut changed = actual.clone();
                                    let TerminatorKind::Call { unwind: action, .. } = &mut changed
                                        .basic_blocks_mut()[block]
                                        .terminator_mut()
                                        .kind
                                    else {
                                        unreachable!()
                                    };
                                    *action = unwind;
                                    assert!(!reviewed_body(tcx, &changed, helper));
                                }
                                let mut changed = actual.clone();
                                let TerminatorKind::Call { func, .. } =
                                    &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                                else {
                                    unreachable!()
                                };
                                *func = Operand::Constant(Box::new(ConstOperand {
                                    span: DUMMY_SP,
                                    user_ty: None,
                                    const_: Const::Val(
                                        ConstValue::ZeroSized,
                                        Ty::new_fn_def(
                                            tcx,
                                            definition("fake_panic").to_def_id(),
                                            std::iter::empty::<Ty<'tcx>>(),
                                        ),
                                    ),
                                }));
                                assert!(!reviewed_body(tcx, &changed, helper));
                                let mut changed = actual.clone();
                                changed.basic_blocks_mut()[BasicBlock::from_usize(0)]
                                    .terminator_mut()
                                    .kind = TerminatorKind::Goto { target: block };
                                assert!(!reviewed_body(tcx, &changed, helper), "reachable panic");
                            }
                            TerminatorKind::SwitchInt { targets, .. } => {
                                switches += 1;
                                let false_target = targets.iter().next().unwrap().1;
                                if matches!(
                                    actual.basic_blocks[false_target].terminator().kind,
                                    TerminatorKind::Call { .. }
                                ) {
                                    let mut changed = actual.clone();
                                    let TerminatorKind::SwitchInt {
                                        discr,
                                        targets: changed_targets,
                                    } = &mut changed.basic_blocks_mut()[block]
                                        .terminator_mut()
                                        .kind
                                    else {
                                        unreachable!()
                                    };
                                    *discr = bool_operand(tcx, true);
                                    *changed_targets = SwitchTargets::new(
                                        [(0, targets.otherwise())].into_iter(),
                                        false_target,
                                    );
                                    assert!(
                                        !reviewed_body(tcx, &changed, helper),
                                        "inverted precondition"
                                    );
                                }
                                let mut changed = actual.clone();
                                let TerminatorKind::SwitchInt { targets, .. } =
                                    &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                                else {
                                    unreachable!()
                                };
                                *targets = SwitchTargets::new(
                                    [(0, block)].into_iter(),
                                    targets.otherwise(),
                                );
                                assert!(
                                    !reviewed_body(tcx, &changed, helper),
                                    "cycle on infeasible edge"
                                );
                            }
                            _ => {}
                        }
                    }
                    assert_eq!((switches, panics), (4, 2));
                }
            }
            Check::Closed => {
                for (name, helper) in routes {
                    let actual = body(name);
                    for (block, _) in actual.basic_blocks.iter_enumerated() {
                        let mut changed = actual.clone();
                        changed.basic_blocks_mut()[block]
                            .statements
                            .push(Statement::new(
                                SourceInfo::outermost(DUMMY_SP),
                                StatementKind::Intrinsic(Box::new(
                                    rustc_middle::mir::NonDivergingIntrinsic::Assume(bool_operand(
                                        tcx, true,
                                    )),
                                )),
                            ));
                        assert!(!reviewed_body(tcx, &changed, helper), "effect in {block:?}");
                        let mut changed = actual.clone();
                        changed.basic_blocks_mut()[block].is_cleanup = true;
                        assert!(!reviewed_body(tcx, &changed, helper));
                        let mut changed = actual.clone();
                        changed.basic_blocks_mut()[block].terminator = None;
                        assert!(!reviewed_body(tcx, &changed, helper));
                    }
                    let mut changed = actual.clone();
                    changed
                        .basic_blocks_mut()
                        .push(actual.basic_blocks[BasicBlock::from_usize(0)].clone());
                    assert!(
                        !reviewed_body(tcx, &changed, helper),
                        "unvisited dead block"
                    );
                    let mut changed = actual.clone();
                    changed.local_decls[Local::from_usize(1)].ty = tcx.types.u16;
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    while changed.local_decls.len() <= MAX_LOCALS {
                        changed
                            .local_decls
                            .push(actual.local_decls[Local::from_usize(0)].clone());
                    }
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    while changed.basic_blocks.len() <= MAX_BLOCKS {
                        changed
                            .basic_blocks_mut()
                            .push(actual.basic_blocks[BasicBlock::from_usize(0)].clone());
                    }
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    changed.source_scopes.raw[0].inlined = Some((called("from_u32"), DUMMY_SP));
                    assert!(!reviewed_body(tcx, &changed, helper));
                }
            }
            Check::Constants => {
                for (name, helper) in routes {
                    let actual = body(name);
                    let mut conditions = 0;
                    for (block, data) in actual.basic_blocks.iter_enumerated() {
                        for (index, statement) in data.statements.iter().enumerate() {
                            let StatementKind::Assign(assignment) = &statement.kind else {
                                continue;
                            };
                            if matches!(assignment.1, Rvalue::BinaryOp(BinOp::Le, _)) {
                                conditions += 1;
                                let mut changed = actual.clone();
                                let StatementKind::Assign(assignment) =
                                    &mut changed.basic_blocks_mut()[block].statements[index].kind
                                else {
                                    unreachable!()
                                };
                                assignment.1 = Rvalue::Use(bool_operand(tcx, false));
                                assert!(
                                    !reviewed_body(tcx, &changed, helper),
                                    "false constant precondition"
                                );
                            }
                        }
                    }
                    assert_eq!(conditions, 2);
                }
                assert_eq!(
                    constant_cast(
                        tcx,
                        CastKind::FloatToInt,
                        tcx.types.f32,
                        0xff7fffff,
                        tcx.types.i128
                    ),
                    Some(i128::MIN as u128)
                );
                let maximum = u128::MAX - ((1_u128 << 104) - 1);
                assert_eq!(
                    constant_cast(
                        tcx,
                        CastKind::FloatToInt,
                        tcx.types.f32,
                        0x7f7fffff,
                        tcx.types.u128
                    ),
                    Some(maximum)
                );
                assert_eq!(maximum, f32::MAX as u128);
                assert_ne!(maximum, u128::MAX);
                for bits in [0, 0x7fc00000, 0x7f800000, 0xff800000, 0x3f800000] {
                    assert!(
                        constant_cast(
                            tcx,
                            CastKind::FloatToInt,
                            tcx.types.f32,
                            bits,
                            tcx.types.u128
                        )
                        .is_none()
                    );
                }
                let foreign = body("foreign_constant")
                    .basic_blocks
                    .iter()
                    .flat_map(|block| &block.statements)
                    .find_map(|statement| match &statement.kind {
                        StatementKind::Assign(assignment) => match &assignment.1 {
                            Rvalue::Use(Operand::Constant(value)) => Some(value),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap();
                assert!(
                    constant(tcx, foreign).is_none(),
                    "never evaluate foreign constants"
                );
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(check: Check, debug: bool) {
    let mut command = std::process::Command::new("rustc");
    command.args(["--print", "sysroot"]);
    let sysroot = crate::process_execution::capture_output(&mut command).unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=primitive_value_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Cpanic=abort".into(),
        format!("-Cdebug-assertions={debug}"),
        "-Zmir-opt-level=0".into(),
        "-Zinline-mir=no".into(),
        "-Zno-codegen".into(),
        "-".into(),
    ];
    let mut probe = Probe {
        check,
        debug,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn exact_core_trait_primitive_identity_and_signature() {
    run(Check::Identity, true);
}
#[test]
fn actual_core_and_retained_constant_precondition_bodies() {
    run(Check::Bodies, true);
    run(Check::Bodies, false);
}
#[test]
fn operations_require_exact_dereferences_casts_inputs_and_conditions() {
    run(Check::Operations, true);
}
#[test]
fn infeasible_panics_require_exact_callee_unwind_and_acyclic_edges() {
    run(Check::Branches, true);
}
#[test]
fn all_blocks_effects_local_types_and_resource_bounds_are_closed() {
    run(Check::Closed, true);
}
#[test]
fn constant_preconditions_use_exact_saturating_float_casts() {
    run(Check::Constants, true);
}

#[test]
fn production_cast_expansion_binds_original_mir_abi_and_exact_replay() {
    run(Check::Production, true);
    run(Check::Production, false);
}

#[test]
fn exact_statement_scope_and_path_work_limits() {
    run(Check::Limits, true);
}

#[test]
fn condition_reassignment_alias_move_and_storage_state_are_exact() {
    run(Check::State, true);
}

fn run_equality(abort: bool, amdgpu: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=primitive_eq_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zmir-opt-level=0".into(),
        "-Zinline-mir=no".into(),
        "-Zno-codegen".into(),
    ];
    if abort {
        args.push("-Cpanic=abort".into());
    }
    if amdgpu {
        let cached = |name| {
            let path = std::path::PathBuf::from(
                std::env::var_os(name).unwrap_or_else(|| panic!("set cached {name}")),
            );
            assert!(path.is_file());
            path
        };
        let core = cached("FE2O3_WRAPPING_AMDGPU_CORE");
        let builtins = cached("FE2O3_WRAPPING_AMDGPU_BUILTINS");
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Zunstable-options".into(),
            "--extern".into(),
            format!("noprelude,nounused:core={}", core.display()),
            "--extern".into(),
            format!(
                "noprelude,nounused:compiler_builtins={}",
                builtins.display()
            ),
            "-L".into(),
            format!("dependency={}", core.parent().unwrap().display()),
        ]);
        if core.parent() != builtins.parent() {
            args.extend([
                "-L".into(),
                format!("dependency={}", builtins.parent().unwrap().display()),
            ]);
        }
    }
    args.push("-".into());
    let mut probe = Probe {
        check: Check::Equality,
        debug: true,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn primitive_eq_actual_mir_identity_and_mutations_unwind() {
    run_equality(false, false);
}

#[test]
fn primitive_eq_actual_mir_identity_and_mutations_abort() {
    run_equality(true, false);
}

#[test]
#[ignore = "requires cached pinned FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn primitive_eq_actual_mir_identity_and_mutations_amdgpu_abort() {
    run_equality(true, true);
}
