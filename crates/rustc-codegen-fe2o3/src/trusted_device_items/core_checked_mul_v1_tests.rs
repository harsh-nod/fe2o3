use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{Local, SourceInfo, Statement, SwitchTargets};
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

const SOURCE: &str = r#"
#![no_std]
#![feature(core_intrinsics)]
#![allow(dead_code, internal_features)]
pub fn checked(a: usize, b: usize) -> Option<usize> { a.checked_mul(b) }
pub fn overflowing(a: usize, b: usize) -> (usize, bool) { a.overflowing_mul(b) }
pub fn overflowing_add(a: usize, b: usize) -> (usize, bool) { a.overflowing_add(b) }
pub fn hint(a: bool) -> bool { core::intrinsics::unlikely(a) }
pub fn wrong_operation(a: usize, b: usize) -> Option<usize> { a.checked_add(b) }
pub fn wrong_width(a: u32, b: u32) -> Option<u32> { a.checked_mul(b) }
pub fn wrong_safety(a: usize, b: usize) -> usize { unsafe { a.unchecked_mul(b) } }
pub fn route(a: usize, b: usize) -> Option<usize> {
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
    }
}
pub fn foreign_hint(a: bool) -> bool { fake_core::intrinsics::unlikely(a) }
pub fn foreign_method(a: usize, b: usize) -> Option<usize> { Impostor::checked_mul(a, b) }
pub fn foreign_function(a: usize, b: usize) -> Option<usize> { checked_mul(a, b) }
"#;

#[derive(Clone, Copy)]
enum Check {
    IdentityAndAbi,
    ActualCoreAndRetainedRoute,
    Arithmetic,
    ControlFlow,
    EffectsAndBudget,
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
        let checked = called("checked");
        let actual = tcx.instance_mir(checked.def);
        let route = body("route");
        match self.check {
            Check::IdentityAndAbi => {
                for (name, kind) in [
                    ("checked", Helper::CheckedMul),
                    ("overflowing", Helper::OverflowingMul),
                    ("hint", Helper::Unlikely),
                ] {
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
                    "wrong_safety",
                    "foreign_hint",
                    "foreign_method",
                    "foreign_function",
                ] {
                    assert_eq!(helper_identity(tcx, called(name)), None, "{name}");
                    assert!(
                        !authenticate_reviewed_safe_core_checked_mul_helper_v1(tcx, called(name)),
                        "{name}"
                    );
                }
            }
            Check::ActualCoreAndRetainedRoute => {
                for name in ["checked", "overflowing", "hint"] {
                    assert!(
                        authenticate_reviewed_safe_core_checked_mul_helper_v1(tcx, called(name)),
                        "{name}"
                    );
                }
                assert!(reviewed_body(tcx, route, Helper::CheckedMul));
                assert_eq!(
                    helper_identity(tcx, Instance::mono(tcx, route.source.def_id())),
                    None
                );
            }
            Check::Arithmetic => {
                let mut mutated = 0;
                for (block, data) in actual.basic_blocks.iter_enumerated() {
                    for (index, statement) in data.statements.iter().enumerate() {
                        if !matches!(&statement.kind, StatementKind::Assign(assignment)
                            if matches!(assignment.1, Rvalue::BinaryOp(BinOp::MulWithOverflow, _)))
                        {
                            continue;
                        }
                        for wrong in [
                            BinOp::AddWithOverflow,
                            BinOp::SubWithOverflow,
                            BinOp::Mul,
                            BinOp::MulUnchecked,
                        ] {
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
                                !reviewed_body(tcx, &changed, Helper::CheckedMul),
                                "{wrong:?}"
                            );
                            mutated += 1;
                        }
                        let mut changed = actual.clone();
                        let StatementKind::Assign(assignment) =
                            &mut changed.basic_blocks_mut()[block].statements[index].kind
                        else {
                            unreachable!()
                        };
                        let Rvalue::BinaryOp(_, operands) = &mut assignment.1 else {
                            unreachable!()
                        };
                        operands.1 = operands.0.clone();
                        assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                        mutated += 1;
                    }
                }
                assert_eq!(
                    mutated, 5,
                    "actual checked_mul contains one overflow operation"
                );
            }
            Check::ControlFlow => {
                assert!(reviewed_body(tcx, route, Helper::CheckedMul));
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
                        assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                        switches += 1;
                    }
                    if matches!(data.terminator().kind, TerminatorKind::Goto { .. }) {
                        let mut changed = route.clone();
                        changed.basic_blocks_mut()[block].terminator_mut().kind =
                            TerminatorKind::Goto {
                                target: BasicBlock::from_usize(0),
                            };
                        assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                    }
                }
                assert_eq!(switches, 1);
                let wrong = body("overflowing_add")
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
                assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
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
                        assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                        payloads += 1;
                    }
                }
                assert_eq!(payloads, 1, "Some must carry the product, not an input");
            }
            Check::EffectsAndBudget => {
                let entry = BasicBlock::from_usize(0);
                let mut casts = 0;
                for (block, data) in actual.basic_blocks.iter_enumerated() {
                    for (index, statement) in data.statements.iter().enumerate() {
                        if !matches!(&statement.kind, StatementKind::Assign(assignment)
                            if matches!(assignment.1, Rvalue::Cast(CastKind::IntToInt, ..)))
                        {
                            continue;
                        }
                        let mut changed = actual.clone();
                        let StatementKind::Assign(assignment) =
                            &mut changed.basic_blocks_mut()[block].statements[index].kind
                        else {
                            unreachable!()
                        };
                        let Rvalue::Cast(_, _, ty) = &mut assignment.1 else {
                            unreachable!()
                        };
                        *ty = tcx.types.u8;
                        assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                        casts += 1;
                    }
                }
                assert_eq!(casts, 3);
                for extra in [
                    StatementKind::Nop,
                    StatementKind::StorageDead(Local::from_usize(1)),
                    StatementKind::StorageLive(Local::from_usize(MAX_LOCALS)),
                ] {
                    let mut changed = actual.clone();
                    changed.basic_blocks_mut()[entry]
                        .statements
                        .push(Statement::new(SourceInfo::outermost(DUMMY_SP), extra));
                    assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                }
                let mut changed = actual.clone();
                changed.basic_blocks_mut()[entry].is_cleanup = true;
                assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                let mut changed = actual.clone();
                changed.local_decls[Local::from_usize(1)].ty =
                    Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, tcx.types.usize);
                assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                let mut changed = actual.clone();
                let local = changed.local_decls[Local::from_usize(1)].clone();
                while changed.local_decls.len() <= MAX_LOCALS {
                    changed.local_decls.push(local.clone());
                }
                assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                let mut changed = actual.clone();
                let scope = changed.source_scopes.iter().next().unwrap().clone();
                while changed.source_scopes.len() <= MAX_SCOPES {
                    changed.source_scopes.push(scope.clone());
                }
                assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
                let mut changed = actual.clone();
                changed.basic_blocks_mut()[entry]
                    .statements
                    .resize_with(MAX_STATEMENTS + 1, || {
                        Statement::new(SourceInfo::outermost(DUMMY_SP), StatementKind::Nop)
                    });
                assert!(!reviewed_body(tcx, &changed, Helper::CheckedMul));
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(check: Check) {
    let mut command = std::process::Command::new("rustc");
    command.args(["--print", "sysroot"]);
    let sysroot = crate::process_execution::capture_output(&mut command).unwrap();
    assert!(sysroot.status.success());
    let args = vec![
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
        "-".into(),
    ];
    let mut probe = Probe { check, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran, "compiler callback did not run");
}

#[test]
fn core_checked_mul_binds_actual_core_identity_and_typed_abi() {
    run(Check::IdentityAndAbi);
}

#[test]
fn core_checked_mul_authenticates_actual_core_and_retained_call_mir() {
    run(Check::ActualCoreAndRetainedRoute);
}

#[test]
fn core_checked_mul_rejects_changed_overflow_operation_and_operands() {
    run(Check::Arithmetic);
}

#[test]
fn core_checked_mul_rejects_inverted_branch_cycles_and_callee_substitution() {
    run(Check::ControlFlow);
}

#[test]
fn core_checked_mul_rejects_extra_effects_references_and_excess_work() {
    run(Check::EffectsAndBudget);
}
