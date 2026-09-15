use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{ConstOperand, ConstValue, Local, SourceInfo, Statement, SwitchTargets};
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

#[path = "core_checked_mul_v1_tests/is_multiple_of.rs"]
mod is_multiple_of;
#[path = "core_checked_mul_v1_tests/mir_tests.rs"]
pub(super) mod mir_tests;
#[path = "core_checked_mul_v1_tests/width_tests.rs"]
mod width_tests;

const SOURCE: &str = r#"
#![no_std]
#![feature(core_intrinsics)]
#![allow(dead_code, internal_features)]
pub async fn coroutine_header(a: usize) -> usize { a }
pub fn closure_header(input: f32) -> f32 { let identity = |value: f32| value; identity(input) }
pub fn checked(a: usize, b: usize) -> Option<usize> { a.checked_mul(b) }
pub fn overflowing(a: usize, b: usize) -> (usize, bool) { a.overflowing_mul(b) }
pub fn checked_u32(a: u32, b: u32) -> Option<u32> { a.checked_mul(b) }
pub fn checked_u64(a: u64, b: u64) -> Option<u64> { a.checked_mul(b) }
pub fn overflowing_u32(a: u32, b: u32) -> (u32, bool) { a.overflowing_mul(b) }
pub fn overflowing_u64(a: u64, b: u64) -> (u64, bool) { a.overflowing_mul(b) }
pub fn literal_overflowing_u32(a: u32, b: u32) -> (u32, bool) {
    let (a, b) = core::intrinsics::mul_with_overflow(a, b); (a, b)
}
pub fn literal_overflowing_u64(a: u64, b: u64) -> (u64, bool) {
    let (a, b) = core::intrinsics::mul_with_overflow(a, b); (a, b)
}
pub fn multiple_u32(a: u32, b: u32) -> bool { a.is_multiple_of(b) }
pub fn checked_add(a: usize, b: usize) -> Option<usize> { a.checked_add(b) }
pub fn checked_add_u64(a: u64, b: u64) -> Option<u64> { a.checked_add(b) }
pub fn checked_sub(a: usize, b: usize) -> Option<usize> { a.checked_sub(b) }
pub fn wrapping_sub(a: usize, b: usize) -> usize { a.wrapping_sub(b) }
pub fn wrapping_add(a: usize, b: usize) -> usize { a.wrapping_add(b) }
pub fn wrapping_mul(a: usize, b: usize) -> usize { a.wrapping_mul(b) }
pub fn intrinsic_sub() -> impl Copy { core::intrinsics::wrapping_sub::<usize> }
pub fn intrinsic_add() -> impl Copy { core::intrinsics::wrapping_add::<usize> }
pub fn intrinsic_mul() -> impl Copy { core::intrinsics::wrapping_mul::<usize> }
pub fn intrinsic_sub_u32() -> impl Copy { core::intrinsics::wrapping_sub::<u32> }
pub fn overflowing_add(a: usize, b: usize) -> (usize, bool) { a.overflowing_add(b) }
pub fn overflowing_add_u64(a: u64, b: u64) -> (u64, bool) { a.overflowing_add(b) }
pub fn overflowing_sub(a: usize, b: usize) -> (usize, bool) { a.overflowing_sub(b) }
pub fn hint(a: bool) -> bool { core::intrinsics::unlikely(a) }
pub fn wrong_operation(a: usize, b: usize) -> Option<usize> { a.checked_div(b) }
pub fn wrong_width(a: u16, b: u16) -> Option<u16> { a.checked_mul(b) }
pub fn wrong_add_width(a: u32, b: u32) -> Option<u32> { a.checked_add(b) }
pub fn wrong_sub_width(a: u32, b: u32) -> Option<u32> { a.checked_sub(b) }
pub fn wrong_same_width(a: u64, b: u64) -> Option<u64> { a.checked_sub(b) }
pub fn wrong_wrapping_width(a: u32, b: u32) -> u32 { a.wrapping_sub(b) }
pub fn wrong_signedness(a: isize, b: isize) -> Option<isize> { a.checked_sub(b) }
pub fn wrong_safety(a: usize, b: usize) -> usize { unsafe { a.unchecked_mul(b) } }
pub fn wrong_add_safety(a: usize, b: usize) -> usize { unsafe { a.unchecked_add(b) } }
pub fn wrong_sub_safety(a: usize, b: usize) -> usize { unsafe { a.unchecked_sub(b) } }
pub fn route(a: usize, b: usize) -> Option<usize> {
    let (product, overflow) = a.overflowing_mul(b);
    if core::intrinsics::unlikely(overflow) { None } else { Some(product) }
}
pub fn route_add(a: usize, b: usize) -> Option<usize> {
    let (sum, overflow) = a.overflowing_add(b);
    if core::intrinsics::unlikely(overflow) { None } else { Some(sum) }
}
pub fn route_add_u64(a: u64, b: u64) -> Option<u64> {
    let (sum, overflow) = a.overflowing_add(b);
    if core::intrinsics::unlikely(overflow) { None } else { Some(sum) }
}
pub fn route_sub(a: usize, b: usize) -> Option<usize> {
    let (difference, overflow) = a.overflowing_sub(b);
    if core::intrinsics::unlikely(overflow) { None } else { Some(difference) }
}
pub fn route_u32(a: u32, b: u32) -> Option<u32> {
    let (product, overflow) = a.overflowing_mul(b);
    if core::intrinsics::unlikely(overflow) { None } else { Some(product) }
}
pub fn route_u64(a: u64, b: u64) -> Option<u64> {
    let (product, overflow) = a.overflowing_mul(b);
    if core::intrinsics::unlikely(overflow) { None } else { Some(product) }
}
pub fn checked_mul(a: usize, b: usize) -> Option<usize> { a.checked_mul(b) }
pub struct Impostor;
impl Impostor {
    pub fn checked_mul(a: usize, b: usize) -> Option<usize> { a.checked_mul(b) }
}
mod fake_core {
    pub mod intrinsics {
        pub fn unlikely(a: bool) -> bool { a }
        pub fn wrapping_sub(a: usize, b: usize) -> usize { a.wrapping_sub(b) }
    }
}
pub fn foreign_hint(a: bool) -> bool { fake_core::intrinsics::unlikely(a) }
pub fn foreign_method(a: usize, b: usize) -> Option<usize> { Impostor::checked_mul(a, b) }
pub fn foreign_function(a: usize, b: usize) -> Option<usize> { checked_mul(a, b) }
pub fn foreign_wrapping(a: usize, b: usize) -> usize { fake_core::intrinsics::wrapping_sub(a, b) }
"#;

const HELPERS: &[(&str, Helper)] = &[
    ("checked", Helper::Checked(Operation::Mul, Word::Usize)),
    ("checked_add", Helper::Checked(Operation::Add, Word::Usize)),
    (
        "checked_add_u64",
        Helper::Checked(Operation::Add, Word::U64),
    ),
    ("checked_sub", Helper::Checked(Operation::Sub, Word::Usize)),
    (
        "overflowing",
        Helper::Overflowing(Operation::Mul, Word::Usize),
    ),
    (
        "overflowing_add",
        Helper::Overflowing(Operation::Add, Word::Usize),
    ),
    (
        "overflowing_add_u64",
        Helper::Overflowing(Operation::Add, Word::U64),
    ),
    (
        "overflowing_sub",
        Helper::Overflowing(Operation::Sub, Word::Usize),
    ),
    ("checked_u32", Helper::Checked(Operation::Mul, Word::U32)),
    ("checked_u64", Helper::Checked(Operation::Mul, Word::U64)),
    (
        "overflowing_u32",
        Helper::Overflowing(Operation::Mul, Word::U32),
    ),
    (
        "overflowing_u64",
        Helper::Overflowing(Operation::Mul, Word::U64),
    ),
    ("wrapping_sub", Helper::WrappingSub),
    ("hint", Helper::Unlikely),
];

const ROUTES: &[(&str, Operation, Word)] = &[
    ("route", Operation::Mul, Word::Usize),
    ("route_add", Operation::Add, Word::Usize),
    ("route_add_u64", Operation::Add, Word::U64),
    ("route_sub", Operation::Sub, Word::Usize),
    ("route_u32", Operation::Mul, Word::U32),
    ("route_u64", Operation::Mul, Word::U64),
];

#[derive(Clone, Copy)]
enum Check {
    IdentityAndAbi,
    ActualCoreAndRetainedRoute,
    Arithmetic,
    ControlFlow,
    EffectsAndBudget,
    GuardedArithmetic,
    RetainedIntrinsic,
    StorageAndMoves,
    BudgetBoundaries,
    WidthBindings,
}

struct Probe {
    check: Check,
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("checked_mul_authentication.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let body = |name: &str| {
            let definition = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .expect("fixture function");
            tcx.optimized_mir(definition)
        };
        let called = |name| {
            body(name)
                .basic_blocks
                .iter()
                .find_map(|block| {
                    let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                        return None;
                    };
                    resolve_callee(tcx, func)
                })
                .expect("fixture retained call")
        };
        let route = body("route");
        match self.check {
            Check::WidthBindings => width_tests::check(tcx, called, body),
            Check::StorageAndMoves => {
                for &(name, helper) in HELPERS {
                    let actual = tcx.instance_mir(called(name).def);
                    mir_tests::storage_and_moves(tcx, actual, helper);
                    if matches!(helper, Helper::Overflowing(..)) {
                        mir_tests::tuple_moves(tcx, actual, helper);
                    }
                }
                for &(name, operation, word) in ROUTES {
                    mir_tests::storage_and_moves(tcx, body(name), Helper::Checked(operation, word));
                }
            }
            Check::BudgetBoundaries => {
                for &(name, helper) in HELPERS {
                    mir_tests::budget_boundaries(tcx, tcx.instance_mir(called(name).def), helper);
                }
            }
            Check::IdentityAndAbi => {
                for &(name, kind) in HELPERS {
                    let instance = called(name);
                    assert_eq!(helper_identity(tcx, instance), Some(kind));
                    let signature = tcx.instantiate_bound_regions_with_erased(
                        tcx.fn_sig(instance.def_id())
                            .instantiate(tcx, instance.args),
                    );
                    assert!(signature_matches(tcx, kind, signature));
                    assert!(!signature_matches(
                        tcx,
                        kind,
                        FnSig {
                            safety: Safety::Unsafe,
                            ..signature
                        }
                    ));
                    assert!(!signature_matches(
                        tcx,
                        kind,
                        FnSig {
                            abi: ExternAbi::C { unwind: false },
                            ..signature
                        }
                    ));
                    assert!(!signature_matches(
                        tcx,
                        kind,
                        FnSig {
                            c_variadic: true,
                            ..signature
                        }
                    ));
                    for types in [
                        vec![tcx.types.usize],
                        vec![tcx.types.u32, tcx.types.u32, tcx.types.u32],
                        vec![tcx.types.usize, tcx.types.usize, tcx.types.unit],
                    ] {
                        assert!(!signature_matches(
                            tcx,
                            kind,
                            FnSig {
                                inputs_and_output: tcx.mk_type_list(&types),
                                ..signature
                            }
                        ));
                    }
                }
                for name in [
                    "wrong_operation",
                    "wrong_width",
                    "wrong_add_width",
                    "wrong_sub_width",
                    "wrong_same_width",
                    "wrong_wrapping_width",
                    "wrong_signedness",
                    "wrong_safety",
                    "wrong_add_safety",
                    "wrong_sub_safety",
                    "foreign_hint",
                    "foreign_method",
                    "foreign_function",
                    "foreign_wrapping",
                    "wrapping_add",
                    "wrapping_mul",
                ] {
                    assert_eq!(helper_identity(tcx, called(name)), None, "{name}");
                    assert!(
                        !authenticate_reviewed_safe_core_arithmetic_helper_v1(tcx, called(name)),
                        "{name}"
                    );
                }
            }
            Check::ActualCoreAndRetainedRoute => {
                let closure = tcx
                    .iter_local_def_id()
                    .find(|id| tcx.def_kind(*id) == DefKind::Closure)
                    .expect("fixture closure");
                let closure = Instance {
                    def: InstanceKind::Item(closure.to_def_id()),
                    args: tcx.mk_args(&[]),
                };
                assert!(
                    !super::super::authenticate_reviewed_safe_core_f32_is_finite_helper_v1(
                        tcx, closure,
                    ),
                    "non-function candidates must reject before querying fn_sig"
                );
                assert!(
                    !super::super::authenticate_reviewed_safe_core_scalar_bitcast_helper_v1(
                        tcx, closure
                    )
                );
                assert!(
                    !super::super::authenticate_reviewed_safe_core_fabs_f32_helper_v1(tcx, closure)
                );
                if std::env::var_os("FE2O3_ARITHMETIC_DUMP_MIR").is_some() {
                    for name in [
                        "checked_add_u64",
                        "overflowing_add_u64",
                        "checked_u32",
                        "overflowing_u32",
                        "checked_u64",
                        "overflowing_u64",
                        "multiple_u32",
                    ] {
                        let actual = tcx.instance_mir(called(name).def);
                        eprintln!("{name}: {actual:#?}");
                    }
                }
                for &(name, helper) in HELPERS {
                    assert!(
                        authenticate_reviewed_safe_core_arithmetic_helper_v1(tcx, called(name)),
                        "{name}"
                    );
                    for &(_, wrong) in HELPERS {
                        if wrong != helper {
                            assert!(
                                !reviewed_body(tcx, tcx.instance_mir(called(name).def), wrong),
                                "{name} cannot stand in for {wrong:?}"
                            );
                        }
                    }
                }
                for &(name, operation, word) in ROUTES {
                    assert!(
                        reviewed_body(tcx, body(name), Helper::Checked(operation, word)),
                        "{name}"
                    );
                }
                for (name, word) in [
                    ("literal_overflowing_u32", Word::U32),
                    ("literal_overflowing_u64", Word::U64),
                ] {
                    assert!(
                        reviewed_body(tcx, body(name), Helper::Overflowing(Operation::Mul, word)),
                        "literal overflowing source"
                    );
                    assert!(!authenticate_reviewed_safe_core_arithmetic_helper_v1(
                        tcx,
                        Instance::mono(tcx, body(name).source.def_id())
                    ));
                }
                assert_eq!(
                    helper_identity(tcx, Instance::mono(tcx, route.source.def_id())),
                    None
                );
            }
            Check::Arithmetic => {
                for &(name, helper) in HELPERS {
                    if helper == Helper::Unlikely {
                        continue;
                    }
                    let actual = tcx.instance_mir(called(name).def);
                    assert!(reviewed_body(tcx, actual, helper), "{name}");
                    let mut operations = 0;
                    for (block, data) in actual.basic_blocks.iter_enumerated() {
                        // AMD metadata can retain the arithmetic helper instead
                        // of inlining its BinaryOp. Mutate that exact call too.
                        if let TerminatorKind::Call { func, args, .. } = &data.terminator().kind
                            && args.len() == 2
                        {
                            let callee = resolve_callee(tcx, func).unwrap();
                            assert!(
                                helper_identity(tcx, callee)
                                    .is_some_and(|kind| helper.may_call(kind))
                                    || (helper == Helper::WrappingSub
                                        && is_wrapping_sub(tcx, callee)),
                                "{name}: retained arithmetic identity",
                            );
                            for swap in [false, true] {
                                let mut changed = actual.clone();
                                let TerminatorKind::Call { args, .. } =
                                    &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                                else {
                                    unreachable!()
                                };
                                if swap {
                                    args.swap(0, 1);
                                } else {
                                    args[1] = args[0].clone();
                                }
                                assert!(
                                    !reviewed_body(tcx, &changed, helper),
                                    "{name}: retained call operands"
                                );
                            }
                            let mut changed = actual.clone();
                            let TerminatorKind::Call { func, .. } =
                                &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                            else {
                                unreachable!()
                            };
                            *func = body("wrong_operation")
                                .basic_blocks
                                .iter()
                                .find_map(|data| match &data.terminator().kind {
                                    TerminatorKind::Call { func, .. } => Some(func.clone()),
                                    _ => None,
                                })
                                .unwrap();
                            assert!(
                                !reviewed_body(tcx, &changed, helper),
                                "{name}: retained call operation"
                            );
                            operations += 1;
                        }
                        for (index, statement) in data.statements.iter().enumerate() {
                            let StatementKind::Assign(assignment) = &statement.kind else {
                                continue;
                            };
                            let Rvalue::BinaryOp(observed, _) = &assignment.1 else {
                                continue;
                            };
                            for wrong in [
                                BinOp::AddWithOverflow,
                                BinOp::SubWithOverflow,
                                BinOp::MulWithOverflow,
                                BinOp::Add,
                                BinOp::Sub,
                                BinOp::Mul,
                                BinOp::AddUnchecked,
                                BinOp::SubUnchecked,
                                BinOp::MulUnchecked,
                                BinOp::Lt,
                                BinOp::Le,
                                BinOp::Gt,
                                BinOp::Ge,
                                BinOp::Eq,
                                BinOp::Ne,
                            ] {
                                if wrong == *observed {
                                    continue;
                                }
                                let mut changed = actual.clone();
                                let StatementKind::Assign(assignment) =
                                    &mut changed.basic_blocks_mut()[block].statements[index].kind
                                else {
                                    unreachable!()
                                };
                                let Rvalue::BinaryOp(operation, _) = &mut assignment.1 else {
                                    unreachable!()
                                };
                                *operation = wrong;
                                assert!(
                                    !reviewed_body(tcx, &changed, helper),
                                    "{name}: {observed:?} replaced with {wrong:?}"
                                );
                            }
                            for swap in [false, true] {
                                let mut changed = actual.clone();
                                let StatementKind::Assign(assignment) =
                                    &mut changed.basic_blocks_mut()[block].statements[index].kind
                                else {
                                    unreachable!()
                                };
                                let Rvalue::BinaryOp(_, operands) = &mut assignment.1 else {
                                    unreachable!()
                                };
                                if swap {
                                    std::mem::swap(&mut operands.0, &mut operands.1);
                                } else {
                                    operands.1 = operands.0.clone();
                                }
                                assert!(
                                    !reviewed_body(tcx, &changed, helper),
                                    "{name}: changed operands"
                                );
                            }
                            operations += 1;
                        }
                    }
                    let expected =
                        if matches!(helper, Helper::Checked(Operation::Add | Operation::Sub, _)) {
                            2
                        } else {
                            1
                        };
                    assert_eq!(operations, expected, "{name}: pinned core operations");
                }
            }
            Check::ControlFlow => {
                for &(name, operation, word) in ROUTES {
                    let route = body(name);
                    let helper = Helper::Checked(operation, word);
                    assert!(reviewed_body(tcx, route, helper));
                    let mut switches = 0;
                    for (block, data) in route.basic_blocks.iter_enumerated() {
                        if let TerminatorKind::SwitchInt { targets, .. } = &data.terminator().kind {
                            let mut changed = route.clone();
                            let TerminatorKind::SwitchInt {
                                targets: changed_targets,
                                ..
                            } = &mut changed.basic_blocks_mut()[block].terminator_mut().kind
                            else {
                                unreachable!()
                            };
                            *changed_targets = SwitchTargets::new(
                                [(0, targets.target_for_value(1))].into_iter(),
                                targets.target_for_value(0),
                            );
                            assert!(!reviewed_body(tcx, &changed, helper));
                            switches += 1;
                        }
                        if matches!(data.terminator().kind, TerminatorKind::Goto { .. }) {
                            let mut changed = route.clone();
                            changed.basic_blocks_mut()[block].terminator_mut().kind =
                                TerminatorKind::Goto {
                                    target: BasicBlock::from_usize(0),
                                };
                            assert!(!reviewed_body(tcx, &changed, helper));
                        }
                    }
                    assert_eq!(switches, 1);
                    let wrong = body(if operation == Operation::Add {
                        "overflowing_sub"
                    } else {
                        "overflowing_add"
                    })
                    .basic_blocks
                    .iter()
                    .find_map(|block| {
                        if let TerminatorKind::Call { func, .. } = &block.terminator().kind {
                            Some(func.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap();
                    let mut changed = route.clone();
                    let TerminatorKind::Call { func, .. } = &mut changed.basic_blocks_mut()
                        [BasicBlock::from_usize(0)]
                    .terminator_mut()
                    .kind
                    else {
                        unreachable!()
                    };
                    *func = wrong;
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut payloads = 0;
                    for (block, data) in route.basic_blocks.iter_enumerated() {
                        for (index, statement) in data.statements.iter().enumerate() {
                            if !matches!(&statement.kind, StatementKind::Assign(assignment)
                            if matches!(&assignment.1, Rvalue::Aggregate(kind, operands)
                                if matches!(&**kind, AggregateKind::Adt(..)) && operands.len() == 1))
                            {
                                continue;
                            }
                            let mut changed = route.clone();
                            let StatementKind::Assign(assignment) =
                                &mut changed.basic_blocks_mut()[block].statements[index].kind
                            else {
                                unreachable!()
                            };
                            let Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
                                unreachable!()
                            };
                            operands.raw[0] = Operand::Copy(Local::from_usize(1).into());
                            assert!(!reviewed_body(tcx, &changed, helper));
                            payloads += 1;
                        }
                    }
                    assert_eq!(payloads, 1, "Some must carry the arithmetic result");
                }
            }
            Check::EffectsAndBudget => {
                let entry = BasicBlock::from_usize(0);
                let coroutine = tcx
                    .iter_local_def_id()
                    .filter(|id| tcx.def_kind(*id) == DefKind::Closure)
                    .find_map(|id| tcx.optimized_mir(id).coroutine.clone())
                    .expect("actual pinned coroutine header");
                for &(name, helper) in HELPERS {
                    let actual = tcx.instance_mir(called(name).def);
                    assert!(reviewed_body(tcx, actual, helper), "{name}");
                    let mut changed = actual.clone();
                    changed.spread_arg = Some(Local::from_usize(1));
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    changed.coroutine = Some(coroutine.clone());
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    changed.source.promoted = Some(rustc_middle::mir::Promoted::from_usize(0));
                    assert!(!reviewed_body(tcx, &changed, helper));
                    for (block, data) in actual.basic_blocks.iter_enumerated() {
                        for (index, statement) in data.statements.iter().enumerate() {
                            if !matches!(&statement.kind, StatementKind::Assign(assignment)
                            if matches!(assignment.1, Rvalue::Cast(CastKind::IntToInt, ..)))
                            {
                                continue;
                            }
                            for wrong_type in [tcx.types.u8, tcx.types.usize, tcx.types.u64] {
                                let mut changed = actual.clone();
                                let StatementKind::Assign(assignment) =
                                    &mut changed.basic_blocks_mut()[block].statements[index].kind
                                else {
                                    unreachable!()
                                };
                                let Rvalue::Cast(_, _, ty) = &mut assignment.1 else {
                                    unreachable!()
                                };
                                if wrong_type == *ty {
                                    continue;
                                }
                                *ty = wrong_type;
                                assert!(!reviewed_body(tcx, &changed, helper));
                            }
                        }
                    }
                    for extra in [
                        StatementKind::Nop,
                        StatementKind::StorageDead(Local::from_usize(1)),
                        StatementKind::StorageLive(Local::from_usize(MAX_LOCALS)),
                    ] {
                        let mut changed = actual.clone();
                        changed.basic_blocks_mut()[entry]
                            .statements
                            .push(Statement::new(SourceInfo::outermost(DUMMY_SP), extra));
                        assert!(!reviewed_body(tcx, &changed, helper));
                    }
                    let mut changed = actual.clone();
                    changed.basic_blocks_mut()[entry].is_cleanup = true;
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    changed.local_decls[Local::from_usize(1)].ty =
                        Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, tcx.types.usize);
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    let local = changed.local_decls[Local::from_usize(1)].clone();
                    while changed.local_decls.len() <= MAX_LOCALS {
                        changed.local_decls.push(local.clone());
                    }
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    let scope = changed.source_scopes.iter().next().unwrap().clone();
                    while changed.source_scopes.len() <= MAX_SCOPES {
                        changed.source_scopes.push(scope.clone());
                    }
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    changed.basic_blocks_mut()[entry]
                        .statements
                        .resize_with(MAX_STATEMENTS + 1, || {
                            Statement::new(SourceInfo::outermost(DUMMY_SP), StatementKind::Nop)
                        });
                    assert!(!reviewed_body(tcx, &changed, helper));
                    let mut changed = actual.clone();
                    let block = actual.basic_blocks[entry].clone();
                    changed.basic_blocks_mut().push(block.clone());
                    assert!(
                        !reviewed_body(tcx, &changed, helper),
                        "uncovered block: {name}"
                    );
                    while changed.basic_blocks.len() <= MAX_BLOCKS {
                        changed.basic_blocks_mut().push(block.clone());
                    }
                    assert!(
                        !reviewed_body(tcx, &changed, helper),
                        "block budget: {name}"
                    );
                    for callee in [
                        called(name),
                        called("foreign_method"),
                        called("wrong_operation"),
                    ] {
                        let mut changed = actual.clone();
                        changed.source_scopes.iter_mut().next().unwrap().inlined =
                            Some((callee, DUMMY_SP));
                        assert!(
                            !reviewed_body(tcx, &changed, helper),
                            "forbidden inlined edge: {name}"
                        );
                    }
                }
            }
            Check::RetainedIntrinsic => {
                let helper = Helper::WrappingSub;
                let intrinsic_operand = |name| {
                    let mir = body(name);
                    let ty = mir
                        .local_decls
                        .iter()
                        .map(|local| local.ty)
                        .find(|ty| matches!(ty.kind(), TyKind::FnDef(..)))
                        .expect("rustc retains the intrinsic function item type");
                    Operand::Constant(Box::new(ConstOperand {
                        span: DUMMY_SP,
                        user_ty: None,
                        const_: Const::Val(ConstValue::ZeroSized, ty),
                    }))
                };
                let func = intrinsic_operand("intrinsic_sub");
                let intrinsic = resolve_callee(tcx, &func).unwrap();
                assert!(is_wrapping_sub(tcx, intrinsic));
                let mut actual = body("wrapping_sub").clone();
                let entry = actual
                    .basic_blocks
                    .iter_enumerated()
                    .find_map(|(block, data)| {
                        let TerminatorKind::Call { .. } = &data.terminator().kind else {
                            return None;
                        };
                        Some(block)
                    })
                    .expect("fixture retained call");
                let TerminatorKind::Call {
                    func: callee,
                    unwind,
                    ..
                } = &mut actual.basic_blocks_mut()[entry].terminator_mut().kind
                else {
                    unreachable!()
                };
                *callee = func;
                *unwind = UnwindAction::Unreachable;
                assert!(reviewed_body(tcx, &actual, helper));
                assert!(
                    !authenticate_reviewed_safe_core_arithmetic_helper_v1(tcx, intrinsic),
                    "intrinsics still need normal intrinsic classification"
                );
                for wrong in ["intrinsic_add", "intrinsic_mul", "intrinsic_sub_u32"] {
                    let wrong_func = intrinsic_operand(wrong);
                    assert!(!is_wrapping_sub(
                        tcx,
                        resolve_callee(tcx, &wrong_func).unwrap()
                    ));
                    let mut changed = actual.clone();
                    let TerminatorKind::Call { func, .. } =
                        &mut changed.basic_blocks_mut()[entry].terminator_mut().kind
                    else {
                        unreachable!()
                    };
                    *func = wrong_func;
                    assert!(
                        !reviewed_body(tcx, &changed, helper),
                        "wrong intrinsic: {wrong}"
                    );
                }
                for swap in [false, true] {
                    let mut changed = actual.clone();
                    let TerminatorKind::Call { args, .. } =
                        &mut changed.basic_blocks_mut()[entry].terminator_mut().kind
                    else {
                        unreachable!()
                    };
                    if swap {
                        args.swap(0, 1);
                    } else {
                        args[1] = args[0].clone();
                    }
                    assert!(
                        !reviewed_body(tcx, &changed, helper),
                        "changed intrinsic arguments"
                    );
                }
                for action in [UnwindAction::Continue, UnwindAction::Cleanup(entry)] {
                    let mut changed = actual.clone();
                    let TerminatorKind::Call { unwind, .. } =
                        &mut changed.basic_blocks_mut()[entry].terminator_mut().kind
                    else {
                        unreachable!()
                    };
                    *unwind = action;
                    assert!(
                        !reviewed_body(tcx, &changed, helper),
                        "changed intrinsic unwind"
                    );
                }
            }
            Check::GuardedArithmetic => {
                for &(name, helper) in HELPERS {
                    let actual = tcx.instance_mir(called(name).def);
                    assert!(reviewed_body(tcx, actual, helper), "{name}");
                    let mut unchecked = 0;
                    for (block, data) in actual.basic_blocks.iter_enumerated() {
                        if let TerminatorKind::SwitchInt { targets, .. } = &data.terminator().kind {
                            for replacement in [
                                TerminatorKind::Goto {
                                    target: targets.target_for_value(0),
                                },
                                TerminatorKind::Goto {
                                    target: targets.target_for_value(1),
                                },
                                TerminatorKind::SwitchInt {
                                    discr: match &data.terminator().kind {
                                        TerminatorKind::SwitchInt { discr, .. } => discr.clone(),
                                        _ => unreachable!(),
                                    },
                                    targets: SwitchTargets::new(
                                        [(0, targets.target_for_value(1))].into_iter(),
                                        targets.target_for_value(0),
                                    ),
                                },
                                TerminatorKind::SwitchInt {
                                    discr: match &data.terminator().kind {
                                        TerminatorKind::SwitchInt { discr, .. } => discr.clone(),
                                        _ => unreachable!(),
                                    },
                                    targets: SwitchTargets::new(
                                        [
                                            (0, targets.target_for_value(0)),
                                            (1, targets.target_for_value(1)),
                                        ]
                                        .into_iter(),
                                        BasicBlock::from_usize(actual.basic_blocks.len()),
                                    ),
                                },
                            ] {
                                let mut changed = actual.clone();
                                changed.basic_blocks_mut()[block].terminator_mut().kind =
                                    replacement;
                                assert!(
                                    !reviewed_body(tcx, &changed, helper),
                                    "changed core branch: {name}"
                                );
                            }
                        }
                        for (index, statement) in data.statements.iter().enumerate() {
                            let StatementKind::Assign(assignment) = &statement.kind else {
                                continue;
                            };
                            if let Rvalue::Use(Operand::Copy(place) | Operand::Move(place)) =
                                &assignment.1
                                && matches!(place.projection.as_ref(), [ProjectionElem::Field(field, ty)]
                                    if field.as_usize() == 1 && *ty == tcx.types.bool)
                            {
                                for flag in [false, true] {
                                    let mut changed = actual.clone();
                                    let StatementKind::Assign(assignment) =
                                        &mut changed.basic_blocks_mut()[block].statements[index]
                                            .kind
                                    else {
                                        unreachable!()
                                    };
                                    assignment.1 = Rvalue::Use(bool_operand(tcx, flag));
                                    assert!(
                                        !reviewed_body(tcx, &changed, helper),
                                        "changed overflow projection: {name}"
                                    );
                                }
                            }
                            if matches!(
                                assignment.1,
                                Rvalue::BinaryOp(BinOp::AddUnchecked | BinOp::SubUnchecked, _)
                            ) {
                                // Hoist the exact core arithmetic before the guard, but keep the
                                // original return routing. Merely checking returned values is unsound.
                                let mut changed = actual.clone();
                                let arithmetic =
                                    changed.basic_blocks_mut()[block].statements.remove(index);
                                changed.basic_blocks_mut()[BasicBlock::from_usize(0)]
                                    .statements
                                    .push(arithmetic);
                                assert!(
                                    !reviewed_body(tcx, &changed, helper),
                                    "unguarded arithmetic: {name}"
                                );
                                unchecked += 1;
                            }
                            if let Rvalue::Aggregate(kind, operands) = &assignment.1
                                && matches!(&**kind, AggregateKind::Tuple)
                                && operands.len() == 2
                            {
                                for flag in [false, true] {
                                    let mut changed = actual.clone();
                                    let StatementKind::Assign(assignment) =
                                        &mut changed.basic_blocks_mut()[block].statements[index]
                                            .kind
                                    else {
                                        unreachable!()
                                    };
                                    let Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
                                        unreachable!()
                                    };
                                    operands.raw[1] = bool_operand(tcx, flag);
                                    assert!(
                                        !reviewed_body(tcx, &changed, helper),
                                        "constant overflow bit: {name}"
                                    );
                                }
                            }
                        }
                    }
                    assert_eq!(
                        unchecked,
                        usize::from(matches!(
                            helper,
                            Helper::Checked(Operation::Add | Operation::Sub, _)
                        )),
                        "{name}: pinned unchecked arithmetic"
                    );
                }
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn bool_operand<'tcx>(tcx: TyCtxt<'tcx>, flag: bool) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::from_bool(tcx, flag),
    }))
}

fn run(check: Check) {
    run_profile(check, false);
}

fn run_profile(check: Check, mir_opt_zero: bool) {
    run_target_profile(check, mir_opt_zero, false);
}

fn run_target_profile(check: Check, mir_opt_zero: bool, amdgpu: bool) {
    let mut command = std::process::Command::new("rustc");
    command.args(["--print", "sysroot"]);
    let sysroot = crate::process_execution::capture_output(&mut command).unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=checked_mul_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
    ];
    if mir_opt_zero {
        args.extend(["-Zmir-opt-level=0".into(), "-Cpanic=abort".into()]);
    }
    if amdgpu {
        for (name, crate_name) in [
            ("FE2O3_CORE_TRY_AMDGPU_CORE", "core"),
            ("FE2O3_CORE_TRY_AMDGPU_BUILTINS", "compiler_builtins"),
        ] {
            let path = std::path::PathBuf::from(std::env::var_os(name).expect(name));
            assert!(path.is_file(), "{name} must name cached AMDGPU metadata");
            args.push(format!("--extern={crate_name}={}", path.display()));
            args.push(format!("-Ldependency={}", path.parent().unwrap().display()));
        }
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Cdebuginfo=2".into(),
            "-Cpanic=abort".into(),
        ]);
    }
    args.push("-".into());
    let mut probe = Probe { check, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran, "compiler callback did not run");
}

#[test]
#[ignore = "requires the pinned cached AMDGPU core and compiler_builtins metadata"]
fn core_arithmetic_actual_amdgpu_add_u64_retains_exact_bodies() {
    run_target_profile(Check::ActualCoreAndRetainedRoute, false, true);
    run_target_profile(Check::Arithmetic, false, true);
    run_target_profile(Check::ControlFlow, false, true);
    run_target_profile(Check::GuardedArithmetic, false, true);
    run_target_profile(Check::IdentityAndAbi, false, true);
}

#[test]
fn core_arithmetic_retained_multiplication_without_mir_optimization() {
    run_profile(Check::ActualCoreAndRetainedRoute, true);
}

#[test]
fn core_arithmetic_binds_exact_width_callees_payloads_and_overflow() {
    run(Check::WidthBindings);
}

#[test]
fn core_arithmetic_tracks_storage_and_consumes_moves() {
    run(Check::StorageAndMoves);
}

#[test]
fn core_arithmetic_accepts_valid_budget_boundaries() {
    run(Check::BudgetBoundaries);
}

#[test]
fn core_arithmetic_binds_actual_core_identity_and_typed_abi() {
    run(Check::IdentityAndAbi);
}

#[test]
fn core_arithmetic_authenticates_actual_core_and_retained_call_mir() {
    run(Check::ActualCoreAndRetainedRoute);
}

#[test]
fn core_arithmetic_rejects_changed_overflow_operation_and_operands() {
    run(Check::Arithmetic);
}

#[test]
fn core_arithmetic_rejects_inverted_branch_cycles_and_callee_substitution() {
    run(Check::ControlFlow);
}

#[test]
fn core_arithmetic_rejects_extra_effects_references_and_excess_work() {
    run(Check::EffectsAndBudget);
}

#[test]
fn core_arithmetic_rejects_unguarded_arithmetic_and_changed_overflow_results() {
    run(Check::GuardedArithmetic);
}

#[test]
fn core_arithmetic_binds_retained_wrapping_intrinsic_and_rejects_mutations() {
    run(Check::RetainedIntrinsic);
}
