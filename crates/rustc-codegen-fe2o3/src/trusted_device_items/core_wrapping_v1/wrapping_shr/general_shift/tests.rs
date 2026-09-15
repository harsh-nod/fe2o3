use super::super::super::profile_tests::{self, Profile};
use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::SwitchTargets;
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

fn source() -> String {
    let mut source = String::from("#![no_std]\n#![allow(dead_code)]\n");
    for ty in [
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    ] {
        for direction in ["l", "r"] {
            source.push_str(&format!(
                "pub fn {ty}_{direction}(x:{ty},n:u32)->{ty}{{x.wrapping_sh{direction}(n)}}\n"
            ));
        }
    }
    source.push_str("pub fn raw_left(x:u8,n:u32)->u8{unsafe{x.unchecked_shl(n)}}\npub fn raw_right(x:u64,n:u32)->u64{unsafe{x.unchecked_shr(n)}}\npub fn wrapping_shl(x:u8,n:u32)->u8{x}\npub fn foreign(x:u8,n:u32)->u8{wrapping_shl(x,n)}\npub fn panic_source(x:u8,n:u32)->u8{if n==7{panic!(\"reachable\")}x}\n");
    source
}

#[derive(Clone, Copy)]
enum Check {
    Positive,
    Arithmetic,
    Flow,
    Identity,
    Budget,
}

struct Probe {
    check: Check,
    ran: bool,
}

fn called_at<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, block: usize) -> Instance<'tcx> {
    let TerminatorKind::Call { func, .. } = &retained::bb(tcx.instance_mir(instance.def), block)
        .terminator()
        .kind
    else {
        panic!("retained call")
    };
    resolve(tcx, func).unwrap()
}

fn reject<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    replaced: Instance<'tcx>,
    changed: &Body<'tcx>,
    label: &str,
) {
    assert!(
        prove_with_budget(
            tcx,
            root,
            &|instance| if instance == replaced {
                changed
            } else {
                tcx.instance_mir(instance.def)
            },
            &mut 16
        )
        .is_none(),
        "{root:?}: {label}"
    );
}

fn binary_mut<'a, 'tcx>(
    body: &'a mut Body<'tcx>,
    block: usize,
    statement: usize,
) -> (&'a mut BinOp, &'a mut Box<(Operand<'tcx>, Operand<'tcx>)>) {
    let StatementKind::Assign(value) =
        &mut body.basic_blocks_mut()[BasicBlock::from_usize(block)].statements[statement].kind
    else {
        panic!("assignment")
    };
    let Rvalue::BinaryOp(operation, operands) = &mut value.1 else {
        panic!("binary")
    };
    (operation, operands)
}

fn word<'tcx>(tcx: TyCtxt<'tcx>, value: u32) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::from_bits(
            tcx,
            value.into(),
            TypingEnv::fully_monomorphized(),
            tcx.types.u32,
        ),
    }))
}

fn function_operand<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::Val(
            ConstValue::ZeroSized,
            Ty::new_fn_def(tcx, instance.def_id(), instance.args),
        ),
    }))
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("general_wrapping_shift.rs".into()),
            input: source(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(tcx.sess.target.llvm_target.as_ref(), "amdgcn-amd-amdhsa");
        for name in [
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        ] {
            for direction in ["l", "r"] {
                let root = profile_tests::called(tcx, &format!("{name}_{direction}"));
                let spec = ShiftSpec::from_root(tcx, root).unwrap();
                let body = tcx.instance_mir(root.def);
                let unchecked = called_at(tcx, root, 1);
                let precondition = called_at(tcx, unchecked, 2);
                let unchecked_body = tcx.instance_mir(unchecked.def);
                let precondition_body = tcx.instance_mir(precondition.def);
                let mut budget = 16;
                let proof = prove_with_budget(
                    tcx,
                    root,
                    &|instance| tcx.instance_mir(instance.def),
                    &mut budget,
                )
                .unwrap_or_else(|| panic!("actual {root:?} source closure"));
                let expanded = proof.expand_mir(tcx);
                match self.check {
                    Check::Positive => {
                        assert_eq!(proof.instance(), root);
                        let selected =
                            super::super::super::super::production_mir_v1::production_mir_v1(
                                tcx, root,
                            );
                        assert!(
                            selected.is_source_expansion(),
                            "production selector for {root:?}"
                        );
                        assert_eq!(selected.instance(), root);
                        assert_eq!(selected.body().source.instance, root.def);
                        assert_eq!(
                            expanded
                                .local_decls
                                .iter()
                                .map(|local| local.ty)
                                .collect::<Vec<_>>(),
                            [spec.element, spec.element, tcx.types.u32, tcx.types.u32]
                        );
                        assert_eq!(expanded.basic_blocks.len(), 1);
                        assert_eq!(retained::bb(&expanded, 0).statements.len(), 2);
                        let Some(Rvalue::BinaryOp(BinOp::BitAnd, mask)) =
                            assignment(&retained::bb(&expanded, 0).statements[0], 3)
                        else {
                            panic!("masked count")
                        };
                        assert!(retained::operand(&mask.0, 2, false));
                        assert_eq!(
                            scalar(tcx, &mask.1, tcx.types.u32),
                            Some((spec.bits - 1).into())
                        );
                        assert!(retained::binary(
                            &expanded,
                            0,
                            1,
                            0,
                            spec.operation(false),
                            1,
                            false,
                            3,
                            true
                        ));
                        assert!(retained::returns(&expanded, 0));
                        assert!(prove_core_wrapping_shift_v1(tcx, unchecked).is_none());
                        assert!(prove_core_wrapping_shift_v1(tcx, precondition).is_none());
                        assert!(
                            !super::super::super::super::production_mir_v1::production_mir_v1(
                                tcx, unchecked
                            )
                            .is_source_expansion()
                        );
                        assert_eq!(
                            proof.expansion_fingerprint(tcx, &expanded),
                            prove_with_budget(
                                tcx,
                                root,
                                &|instance| tcx.instance_mir(instance.def),
                                &mut 16
                            )
                            .unwrap()
                            .expansion_fingerprint(tcx, &expanded)
                        );
                    }
                    Check::Arithmetic => {
                        for value in [0, 2, spec.bits, u32::MAX] {
                            let mut changed = body.clone();
                            binary_mut(&mut changed, 0, 0).1.1 = word(tcx, value);
                            reject(tcx, root, root, &changed, "mask subtraction is exact");
                        }
                        for op in [BinOp::BitOr, BinOp::BitXor, BinOp::Add, BinOp::Shr] {
                            let mut changed = body.clone();
                            *binary_mut(&mut changed, 1, 1).0 = op;
                            reject(tcx, root, root, &changed, "mask operation");
                        }
                        let mut changed = body.clone();
                        binary_mut(&mut changed, 1, 1).1.0 =
                            Operand::Move(Local::from_usize(2).into());
                        reject(tcx, root, root, &changed, "unexpected source move");
                        let mut changed = body.clone();
                        binary_mut(&mut changed, 1, 1).1.1 = word(tcx, spec.bits - 1);
                        reject(tcx, root, root, &changed, "bypassed checked mask result");
                        let mut changed = unchecked_body.clone();
                        *binary_mut(&mut changed, 3, 0).0 = if spec.left {
                            BinOp::ShrUnchecked
                        } else {
                            BinOp::ShlUnchecked
                        };
                        reject(tcx, root, unchecked, &changed, "opposite direction");
                        let mut changed = unchecked_body.clone();
                        let operands = binary_mut(&mut changed, 3, 0).1;
                        std::mem::swap(&mut operands.0, &mut operands.1);
                        reject(tcx, root, unchecked, &changed, "shift operands swapped");
                        let mut changed = precondition_body.clone();
                        binary_mut(&mut changed, 0, 0).1.1 = word(tcx, spec.bits);
                        reject(
                            tcx,
                            root,
                            precondition,
                            &changed,
                            "width literal cannot substitute nominal BITS",
                        );
                        let mut changed = precondition_body.clone();
                        *binary_mut(&mut changed, 0, 0).0 = BinOp::Le;
                        reject(tcx, root, precondition, &changed, "off-by-one precondition");
                    }
                    Check::Flow => {
                        for (instance, source, block) in [
                            (root, body, 0),
                            (unchecked, unchecked_body, 1),
                            (precondition, precondition_body, 0),
                        ] {
                            let mut changed = source.clone();
                            changed.basic_blocks_mut()[BasicBlock::from_usize(block)]
                                .terminator_mut()
                                .kind = TerminatorKind::Goto {
                                target: BasicBlock::from_usize(1),
                            };
                            reject(tcx, root, instance, &changed, "bypass guard or cycle");
                        }
                        let mut changed = body.clone();
                        if let TerminatorKind::Assert { expected, .. } = &mut changed
                            .basic_blocks_mut()[BasicBlock::from_usize(0)]
                        .terminator_mut()
                        .kind
                        {
                            *expected = true;
                        }
                        reject(tcx, root, root, &changed, "wrong overflow edge");
                        let mut changed = precondition_body.clone();
                        if let TerminatorKind::SwitchInt { targets, .. } = &mut changed
                            .basic_blocks_mut()[BasicBlock::from_usize(0)]
                        .terminator_mut()
                        .kind
                        {
                            *targets = SwitchTargets::new(
                                [(0, BasicBlock::from_usize(1))].into_iter(),
                                BasicBlock::from_usize(2),
                            );
                        }
                        reject(
                            tcx,
                            root,
                            precondition,
                            &changed,
                            "panic made reachable for masked count",
                        );
                        let mut changed = precondition_body.clone();
                        changed.basic_blocks_mut()[BasicBlock::from_usize(1)].terminator =
                            precondition_body.basic_blocks[BasicBlock::from_usize(3)]
                                .terminator
                                .clone();
                        reject(tcx, root, precondition, &changed, "panic on success path");
                        let mut changed = unchecked_body.clone();
                        if let TerminatorKind::Call { args, unwind, .. } = &mut changed
                            .basic_blocks_mut()[BasicBlock::from_usize(2)]
                        .terminator_mut()
                        .kind
                        {
                            args[0].node = Operand::Move(Local::from_usize(2).into());
                            *unwind = UnwindAction::Continue;
                        }
                        reject(
                            tcx,
                            root,
                            unchecked,
                            &changed,
                            "consumed count and executable unwind",
                        );
                        let mut changed = body.clone();
                        changed.basic_blocks_mut()[BasicBlock::from_usize(1)]
                            .statements
                            .insert(
                                0,
                                Statement::new(
                                    SourceInfo::outermost(DUMMY_SP),
                                    StatementKind::StorageDead(Local::from_usize(2)),
                                ),
                            );
                        reject(tcx, root, root, &changed, "count killed before mask");
                    }
                    Check::Identity => {
                        for changed_ty in
                            [tcx.types.bool, tcx.types.u32, tcx.types.i64, tcx.types.u64]
                        {
                            if changed_ty == spec.element {
                                continue;
                            }
                            let mut changed = body.clone();
                            changed.local_decls[Local::from_usize(1)].ty = changed_ty;
                            reject(
                                tcx,
                                root,
                                root,
                                &changed,
                                "signedness or nominal width changed",
                            );
                        }
                        let opposite = profile_tests::called(
                            tcx,
                            &format!("{name}_{}", if spec.left { "r" } else { "l" }),
                        );
                        let mut changed = body.clone();
                        if let TerminatorKind::Call { func, .. } = &mut changed.basic_blocks_mut()
                            [BasicBlock::from_usize(1)]
                        .terminator_mut()
                        .kind
                        {
                            *func = function_operand(tcx, called_at(tcx, opposite, 1));
                        }
                        reject(tcx, root, root, &changed, "opposite helper identity");
                        let other = profile_tests::called(
                            tcx,
                            if spec.element == tcx.types.u64 {
                                "usize_r"
                            } else {
                                "u64_r"
                            },
                        );
                        let mut changed = body.clone();
                        changed.required_consts =
                            tcx.instance_mir(other.def).required_consts.clone();
                        reject(
                            tcx,
                            root,
                            root,
                            &changed,
                            "another primitive BITS even at same bit width",
                        );
                        let mut changed = body.clone();
                        changed.source.instance = opposite.def;
                        reject(tcx, root, root, &changed, "source owner mismatch");
                        let mut changed = precondition_body.clone();
                        let other_body =
                            tcx.instance_mir(called_at(tcx, called_at(tcx, opposite, 1), 2).def);
                        changed.basic_blocks_mut()[BasicBlock::from_usize(2)].statements[0] =
                            other_body.basic_blocks[BasicBlock::from_usize(2)].statements[0]
                                .clone();
                        reject(
                            tcx,
                            root,
                            precondition,
                            &changed,
                            "failure message belongs to opposite operation",
                        );
                    }
                    Check::Budget => {
                        let required = 16 - budget;
                        assert!((1..=16).contains(&required));
                        for (limit, expected) in
                            [(0, false), (required - 1, false), (required, true)]
                        {
                            let visits = std::cell::Cell::new(0);
                            let mut available = limit;
                            let proved = prove_with_budget(
                                tcx,
                                root,
                                &|instance| {
                                    visits.set(visits.get() + 1);
                                    tcx.instance_mir(instance.def)
                                },
                                &mut available,
                            );
                            assert_eq!(proved.is_some(), expected);
                            assert!(visits.get() <= limit);
                            assert_eq!(
                                available + visits.get(),
                                limit,
                                "closed callees share the same budget"
                            );
                        }
                        let mut changed = precondition_body.clone();
                        changed.span = DUMMY_SP;
                        let changed_proof = prove_with_budget(
                            tcx,
                            root,
                            &|instance| {
                                if instance == precondition {
                                    &changed
                                } else {
                                    tcx.instance_mir(instance.def)
                                }
                            },
                            &mut 16,
                        )
                        .unwrap();
                        assert_ne!(
                            proof.expansion_fingerprint(tcx, &expanded),
                            changed_proof.expansion_fingerprint(tcx, &expanded),
                            "fingerprint binds downstream original MIR"
                        );
                        let mut changed = body.clone();
                        changed
                            .local_decls
                            .push(LocalDecl::new(tcx.types.u32, DUMMY_SP));
                        reject(tcx, root, root, &changed, "extra unreviewed local");
                    }
                }
            }
        }
        for name in ["raw_left", "raw_right", "foreign"] {
            assert!(prove_core_wrapping_shift_v1(tcx, profile_tests::called(tcx, name)).is_none());
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(check: Check, mir_opt: Option<u8>) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=general_wrapping_shift".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cdebuginfo=2".into(),
    ];
    profile_tests::append_args(&mut args, Profile::AmdgpuCached, mir_opt);
    args.push("-".into());
    let mut probe = Probe { check, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn general_wrapping_shift_actual_amdgpu_all_primitive_widths_and_directions() {
    run(Check::Positive, None);
    run(Check::Positive, Some(0));
}
#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn general_wrapping_shift_actual_amdgpu_rejects_mask_operation_operand_mutations() {
    run(Check::Arithmetic, None);
}
#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn general_wrapping_shift_actual_amdgpu_rejects_guard_move_panic_and_unwind_mutations() {
    run(Check::Flow, None);
}
#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn general_wrapping_shift_actual_amdgpu_rejects_nominal_identity_and_source_mutations() {
    run(Check::Identity, None);
}
#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn general_wrapping_shift_actual_amdgpu_shares_budget_and_binds_complete_proof() {
    run(Check::Budget, None);
}
