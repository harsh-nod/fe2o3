use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{
    BasicBlockData, ConstOperand, SourceInfo, Statement, Terminator, UnwindTerminateReason,
};
use rustc_span::DUMMY_SP;
use std::{fs, path::PathBuf, process::Command};

#[path = "encoded_tests.rs"]
mod encoded_tests;

const SOURCE: &str = r#"
#![no_std]
#![allow(dead_code)]
pub struct NonCopy(pub u64);
impl PartialEq for NonCopy {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
    fn ne(&self, _: &Self) -> bool { panic!("must not use payload ne") }
}
impl Drop for NonCopy { fn drop(&mut self) { panic!("must not drop borrowed payload") } }
pub struct Panicking;
impl PartialEq for Panicking { fn eq(&self, _: &Self) -> bool { panic!("unchecked comparison") } }
pub struct Effectful;
static COUNT: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
impl PartialEq for Effectful {
    fn eq(&self, _: &Self) -> bool { COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed) == 0 }
}
pub trait Associated { type Type; }
impl Associated for NonCopy { type Type = u64; }
macro_rules! cases {
    ($eq:ident, $ne:ident, $ty:ty) => {
        pub fn $eq(left: &Option<$ty>, right: &Option<$ty>) -> bool { Option::<$ty>::eq(left, right) }
        pub fn $ne(left: &Option<$ty>, right: &Option<$ty>) -> bool { Option::<$ty>::ne(left, right) }
    };
}
cases!(eq_u64, ne_u64, u64);
cases!(eq_bool, ne_bool, bool);
cases!(eq_unit, ne_unit, ());
cases!(eq_array, ne_array, [u64; 2]);
cases!(eq_nested, ne_nested, Option<u64>);
cases!(eq_noncopy, ne_noncopy, NonCopy);
cases!(eq_panic, ne_panic, Panicking);
cases!(eq_effect, ne_effect, Effectful);
cases!(eq_associated, ne_associated, <NonCopy as Associated>::Type);
cases!(eq_reference, ne_reference, &'static u64);
pub fn primitive(left: &u64, right: &u64) -> bool { u64::eq(left, right) }
pub fn user_eq(left: &NonCopy, right: &NonCopy) -> bool { NonCopy::eq(left, right) }
pub fn user_ne(left: &NonCopy, right: &NonCopy) -> bool { NonCopy::ne(left, right) }
pub fn default_nonoption(left: &Panicking, right: &Panicking) -> bool { Panicking::ne(left, right) }
pub fn reference_layer(left: &&Option<u64>, right: &&Option<u64>) -> bool { <&Option<u64>>::ne(left, right) }
pub struct Impostor;
impl Impostor { pub fn eq(&self, _: &Self) -> bool { true } }
pub fn impostor(left: &Impostor, right: &Impostor) -> bool { left.eq(right) }
pub fn trap() { panic!("unreviewed call") }
"#;

fn fixture<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> &'tcx Body<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .expect("fixture");
    tcx.instance_mir(Instance::mono(tcx, definition.to_def_id()).def)
}

fn helper<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    fixture(tcx, name)
        .basic_blocks
        .iter()
        .find_map(|b| {
            let TerminatorKind::Call {
                func: Operand::Constant(c),
                ..
            } = &b.terminator().kind
            else {
                return None;
            };
            let TyKind::FnDef(id, args) = c.const_.ty().kind() else {
                return None;
            };
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *id, args).unwrap()
        })
        .expect("helper call")
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

fn check_positive(tcx: TyCtxt<'_>, name: &str) {
    let instance = helper(tcx, name);
    let c = contract(tcx, instance).unwrap_or_else(|| panic!("contract {name}: {instance:?}"));
    let original_body = format!("{:?}", tcx.instance_mir(instance.def));
    assert!(
        authenticate_reviewed_safe_core_option_compare_helper_v1(tcx, instance),
        "actual {name}: {:#?}",
        tcx.instance_mir(instance.def)
    );
    assert_eq!(
        original_body,
        format!("{:?}", tcx.instance_mir(instance.def)),
        "authentication must not rewrite MIR"
    );
    let eq = if c.operation == Operation::Ne {
        c.callee
    } else {
        instance
    };
    assert!(authenticate_reviewed_safe_core_option_compare_helper_v1(
        tcx, eq
    ));
    let payload = contract(tcx, eq).unwrap().callee;
    if name.ends_with("nested") {
        assert!(authenticate_reviewed_safe_core_option_compare_helper_v1(
            tcx, payload
        ));
        let leaf = contract(tcx, payload).unwrap().callee;
        assert!(!authenticate_reviewed_safe_core_option_compare_helper_v1(
            tcx, leaf
        ));
    } else {
        assert!(
            !authenticate_reviewed_safe_core_option_compare_helper_v1(tcx, payload),
            "no payload trust: {name}"
        );
    }
    if name.ends_with("noncopy") {
        let TyKind::Adt(adt, _) = c.payload.kind() else {
            unreachable!()
        };
        assert!(!authenticate_reviewed_safe_core_option_compare_helper_v1(
            tcx,
            Instance::mono(tcx, adt.destructor(tcx).unwrap().did)
        ));
    }
}

fn check_mutations<'tcx>(tcx: TyCtxt<'tcx>, name: &str) {
    let instance = helper(tcx, name);
    let c = contract(tcx, instance).unwrap();
    let body = tcx.instance_mir(instance.def);
    let entry = BasicBlock::from_usize(0);
    let (call, done) = body
        .basic_blocks
        .iter_enumerated()
        .find_map(|(bb, b)| {
            if let TerminatorKind::Call {
                target: Some(target),
                ..
            } = b.terminator().kind
            {
                Some((bb, target))
            } else {
                None
            }
        })
        .unwrap();
    let reject =
        |b: &Body<'tcx>, why| assert!(!reviewed_body(tcx, instance, b, &c), "{name}: {why}");
    for mutation in 0..16 {
        let mut b = body.clone();
        match mutation {
            0 => b.arg_count = 1,
            1 => b.local_decls[Local::from_usize(0)].ty = tcx.types.u64,
            2 => b.local_decls[Local::from_usize(1)].ty = shared(tcx, c.payload),
            3 => {
                b.local_decls
                    .push(body.local_decls[Local::from_usize(0)].clone());
            }
            4 => append_block(&mut b, TerminatorKind::Unreachable, false),
            5 => b.basic_blocks.as_mut()[entry].terminator = None,
            6 => b.basic_blocks.as_mut()[call].is_cleanup = true,
            7 => b.basic_blocks.as_mut()[done]
                .statements
                .push(Statement::new(
                    SourceInfo::outermost(DUMMY_SP),
                    StatementKind::Nop,
                )),
            8 => b.source.instance = helper(tcx, "impostor").def,
            9 => b.mentioned_items = None,
            10 => {
                let items = b.mentioned_items.as_mut().unwrap();
                items.push(items[0].clone());
            }
            11 => b.mentioned_items.as_mut().unwrap()[0].node = MentionedItem::Drop(c.payload),
            12 => {
                b.required_consts = Some(vec![ConstOperand {
                    span: DUMMY_SP,
                    user_ty: None,
                    const_: Const::from_bool(tcx, false),
                }])
            }
            13 => {
                let scope = body.local_decls[Local::from_usize(0)].source_info.scope;
                b.source_scopes[scope].parent_scope = Some(scope);
            }
            14 => b.basic_blocks.as_mut()[done].terminator_mut().kind = TerminatorKind::Unreachable,
            15 => {
                b.basic_blocks.as_mut()[done].terminator_mut().kind =
                    TerminatorKind::Goto { target: entry }
            }
            _ => unreachable!(),
        }
        reject(&b, "shape, types, metadata, budget, or terminal");
    }
    for mutation in 0..11 {
        let mut b = body.clone();
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } = &mut b.basic_blocks.as_mut()[call].terminator_mut().kind
        else {
            unreachable!()
        };
        match mutation {
            0 => args.swap(0, 1),
            1 => args[1].node = args[0].node.clone(),
            2 => {
                args[0].node = match args[0].node {
                    Operand::Move(place) => Operand::Copy(place),
                    Operand::Copy(place) => Operand::Move(place),
                    Operand::Constant(_) | Operand::RuntimeChecks(_) => unreachable!(),
                };
            }
            3 => *destination = Local::from_usize(2).into(),
            4 => *target = None,
            5 => *target = Some(entry),
            6 => *target = Some(BasicBlock::from_usize(body.basic_blocks.len())),
            7 => *unwind = UnwindAction::Cleanup(done),
            8 => *unwind = UnwindAction::Terminate(UnwindTerminateReason::Abi),
            9 | 10 => {
                let id = if mutation == 9 {
                    tcx.get_diagnostic_item(Symbol::intern("cmp_partialeq_ne"))
                        .unwrap()
                } else {
                    c.eq_item
                };
                let ty = if mutation == 9 {
                    c.call_ty
                } else {
                    tcx.types.u8
                };
                let function = Ty::new_fn_def(tcx, id, tcx.mk_args(&[ty.into(), ty.into()]));
                *func = Operand::Constant(Box::new(ConstOperand {
                    span: DUMMY_SP,
                    user_ty: None,
                    const_: Const::Val(ConstValue::ZeroSized, function),
                }));
            }
            _ => unreachable!(),
        }
        reject(
            &b,
            "callee identity, ordered arguments, result, target, or unwind",
        );
    }
    let mut abort = body.clone();
    let TerminatorKind::Call { unwind, .. } =
        &mut abort.basic_blocks.as_mut()[call].terminator_mut().kind
    else {
        unreachable!()
    };
    *unwind = UnwindAction::Unreachable;
    assert_eq!(
        reviewed_body(tcx, instance, &abort, &c),
        !tcx.sess.panic_strategy().unwinds()
    );
    let trap = fixture(tcx, "trap")
        .basic_blocks
        .iter()
        .find(|b| matches!(b.terminator().kind, TerminatorKind::Call { .. }))
        .unwrap()
        .terminator()
        .kind
        .clone();
    for bb in body.basic_blocks.indices() {
        let mut b = body.clone();
        b.basic_blocks.as_mut()[bb].terminator_mut().kind = trap.clone();
        reject(&b, "live or invalid-discriminant trap/call");
    }
    for cleanup in [false, true] {
        let mut b = body.clone();
        append_block(&mut b, trap.clone(), cleanup);
        reject(&b, "extra dead/cleanup call");
    }
    if c.operation == Operation::Ne {
        if body.basic_blocks[entry].statements.is_empty() {
            encoded_tests::check_ne_mutations(tcx, instance, body, &c);
        } else {
            check_ne_mutations(tcx, instance, body, &c);
        }
    } else if body.basic_blocks.len() == 9 {
        encoded_tests::check_eq_mutations(tcx, instance, body, &c);
    } else {
        check_eq_mutations(tcx, instance, body, &c);
    }
}

fn check_ne_mutations<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    c: &Contract<'tcx>,
) {
    for mutation in 0..4 {
        let mut b = body.clone();
        match mutation {
            0 => {
                b.basic_blocks.as_mut()[BasicBlock::from_usize(0)].statements[0].kind =
                    StatementKind::StorageLive(Local::from_usize(0))
            }
            1 => {
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks.as_mut()[BasicBlock::from_usize(1)].statements[0].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::Use(Operand::Move(Local::from_usize(3).into()));
            }
            2 => {
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks.as_mut()[BasicBlock::from_usize(1)].statements[0].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::UnaryOp(UnOp::Not, Operand::Copy(Local::from_usize(3).into()));
            }
            3 => {
                b.basic_blocks.as_mut()[BasicBlock::from_usize(1)].statements[1].kind =
                    StatementKind::StorageDead(Local::from_usize(1))
            }
            _ => unreachable!(),
        }
        assert!(
            !reviewed_body(tcx, instance, &b, c),
            "negation/value/lifetime substitution"
        );
    }
}

fn check_eq_mutations<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    c: &Contract<'tcx>,
) {
    let first = switch(
        &body.basic_blocks[BasicBlock::from_usize(0)]
            .terminator()
            .kind,
        5,
        c,
    )
    .unwrap();
    let some = first.target_for_value(c.some_discriminant);
    let none = first.target_for_value(c.none_discriminant);
    let invalid = first.otherwise();
    let second = switch(&body.basic_blocks[some].terminator().kind, 3, c).unwrap();
    let both = second.target_for_value(c.some_discriminant);
    let mixed = second.target_for_value(c.none_discriminant);
    for bb in [BasicBlock::from_usize(0), some] {
        for mutation in 0..7 {
            let mut b = body.clone();
            let TerminatorKind::SwitchInt { discr, targets } =
                &mut b.basic_blocks.as_mut()[bb].terminator_mut().kind
            else {
                unreachable!()
            };
            let n = targets.target_for_value(c.none_discriminant);
            let s = targets.target_for_value(c.some_discriminant);
            match mutation {
                0 => {
                    *targets = SwitchTargets::new(
                        [(c.none_discriminant, s), (c.some_discriminant, n)].into_iter(),
                        invalid,
                    )
                }
                1 => {
                    *targets = SwitchTargets::new(
                        [(c.none_discriminant, n), (c.some_discriminant, s)].into_iter(),
                        mixed,
                    )
                }
                2 => {
                    *targets = SwitchTargets::new(
                        [(c.none_discriminant, n), (c.none_discriminant, s)].into_iter(),
                        invalid,
                    )
                }
                3 => {
                    *targets = SwitchTargets::new(
                        [
                            (c.none_discriminant, n),
                            (c.some_discriminant, BasicBlock::from_usize(7)),
                        ]
                        .into_iter(),
                        invalid,
                    )
                }
                4 => *targets = SwitchTargets::static_if(c.none_discriminant, n, s),
                5 => *discr = Operand::Move(Local::from_usize(4).into()),
                6 => {
                    let Operand::Move(place) = *discr else {
                        unreachable!()
                    };
                    *discr = Operand::Copy(place);
                }
                _ => unreachable!(),
            }
            assert!(
                !reviewed_body(tcx, instance, &b, c),
                "discriminant coverage/polarity/liveness"
            );
        }
    }
    for mutation in 0..9 {
        let mut b = body.clone();
        let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[both].statements[0].kind else {
            unreachable!()
        };
        let Rvalue::Ref(_, borrow, place) = &mut a.1 else {
            unreachable!()
        };
        match mutation {
            0 => place.local = Local::from_usize(2),
            1 => place.projection = tcx.mk_place_elems(&[]),
            2 => place.projection = tcx.mk_place_elems(&[ProjectionElem::Deref]),
            3 => {
                let mut p = place.projection.to_vec();
                p[1] = ProjectionElem::Downcast(None, rustc_abi::VariantIdx::from_usize(0));
                place.projection = tcx.mk_place_elems(&p);
            }
            4 => {
                let mut p = place.projection.to_vec();
                p[2] = ProjectionElem::Field(rustc_abi::FieldIdx::from_usize(1), c.payload);
                place.projection = tcx.mk_place_elems(&p);
            }
            5 => {
                let mut p = place.projection.to_vec();
                p[2] = ProjectionElem::Field(rustc_abi::FieldIdx::from_usize(0), tcx.types.u8);
                place.projection = tcx.mk_place_elems(&p);
            }
            6 => {
                *borrow = BorrowKind::Mut {
                    kind: rustc_middle::mir::MutBorrowKind::Default,
                }
            }
            7 => a.0 = Local::from_usize(7).into(),
            8 => a.1 = Rvalue::Use(Operand::Move(*place)),
            _ => unreachable!(),
        }
        assert!(
            !reviewed_body(tcx, instance, &b, c),
            "payload provenance, shared borrow, field, or move-out"
        );
    }
    for mutation in 0..4 {
        let mut b = body.clone();
        match mutation {
            0 => {
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks.as_mut()[mixed].statements[0].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::Use(Operand::Constant(Box::new(ConstOperand {
                    span: DUMMY_SP,
                    user_ty: None,
                    const_: Const::from_bool(tcx, true),
                })));
            }
            1 => {
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks.as_mut()[none].statements[1].kind
                else {
                    unreachable!()
                };
                let Rvalue::BinaryOp(op, _) = &mut a.1 else {
                    unreachable!()
                };
                *op = BinOp::Ne;
            }
            2 => {
                let StatementKind::Assign(a) =
                    &mut b.basic_blocks.as_mut()[none].statements[0].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::Discriminant(Place {
                    local: Local::from_usize(1),
                    projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
                });
            }
            3 => {
                b.basic_blocks.as_mut()[mixed].terminator_mut().kind =
                    TerminatorKind::Goto { target: both }
            }
            _ => unreachable!(),
        }
        assert!(
            !reviewed_body(tcx, instance, &b, c),
            "wrong result or invalid payload path"
        );
    }
}

struct Check {
    done: bool,
    amdgpu: bool,
}
impl Callbacks for Check {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.amdgpu {
            encoded_tests::check_profiles(tcx);
        }
        for suffix in [
            "u64",
            "bool",
            "unit",
            "array",
            "nested",
            "noncopy",
            "panic",
            "effect",
            "associated",
            "reference",
        ] {
            for prefix in ["eq", "ne"] {
                check_positive(tcx, &format!("{prefix}_{suffix}"));
            }
        }
        for name in [
            "primitive",
            "user_eq",
            "user_ne",
            "default_nonoption",
            "reference_layer",
            "impostor",
        ] {
            assert!(
                !authenticate_reviewed_safe_core_option_compare_helper_v1(tcx, helper(tcx, name)),
                "nominal negative {name}"
            );
        }
        for name in ["eq_u64", "ne_u64", "eq_noncopy", "ne_noncopy"] {
            check_mutations(tcx, name);
        }
        self.done = true;
        Compilation::Stop
    }
}

fn configured_path(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set cached {name}")));
    assert!(path.exists());
    path
}

fn run_fixture(abort: bool, amdgpu: bool) {
    let directory = TestTempDir::create("fe2o3-option-compare");
    let source = directory.path().join("fixture.rs");
    fs::write(&source, SOURCE).unwrap();
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_option_compare_fixture".into(),
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
    if amdgpu {
        let core = configured_path("FE2O3_CORE_TRY_AMDGPU_CORE");
        let builtins = configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS");
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
    }
    let mut callbacks = Check {
        done: false,
        amdgpu,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.done);
}

#[test]
fn option_compare_actual_mir_and_mutations_unwind() {
    run_fixture(false, false);
}
#[test]
fn option_compare_actual_mir_and_mutations_abort() {
    run_fixture(true, false);
}
#[test]
#[ignore = "requires cached pinned AMD core and compiler_builtins metadata"]
fn option_compare_actual_mir_and_mutations_amdgpu_abort() {
    run_fixture(true, true);
}
