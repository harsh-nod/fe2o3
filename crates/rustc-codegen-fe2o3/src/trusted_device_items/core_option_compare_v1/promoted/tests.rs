use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
pub fn some_u64(value: Option<u64>) -> bool { value != Some(1) }
pub fn some_u8(value: Option<u8>) -> bool { value == Some(7) }
pub fn none_u64(value: Option<u64>) -> bool { value != None }
pub fn some_i32(value: Option<i32>) -> bool { value != Some(-7) }
pub fn some_bool(value: Option<bool>) -> bool { value == Some(true) }
pub fn some_f32(value: Option<f32>) -> bool { value != Some(1.5) }
pub fn some_char(value: Option<char>) -> bool { value == Some('x') }
macro_rules! integer_case {
    ($name:ident, $ty:ty) => { pub fn $name(value: Option<$ty>) -> bool { value != Some(1) } };
}
integer_case!(some_u16, u16);
integer_case!(some_u32, u32);
integer_case!(some_u128, u128);
integer_case!(some_usize, usize);
integer_case!(some_i8, i8);
integer_case!(some_i16, i16);
integer_case!(some_i64, i64);
integer_case!(some_i128, i128);
integer_case!(some_isize, isize);
pub fn some_f64(value: Option<f64>) -> bool { value == Some(-1.5) }
pub fn generic<const N: u64>(value: Option<u64>) -> bool { value != Some(N) }
pub fn generic_caller(value: Option<u64>) -> bool { generic::<3>(value) }
pub fn escapes() -> &'static Option<u64> { &Some(1) }
pub fn observes(value: Option<u64>) -> bool {
    let reference = &Some(1);
    let equal = Option::ne(&value, reference);
    equal && core::ptr::eq(reference, &Some(1))
}
pub struct Custom(u64);
impl PartialEq for Custom {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 || core::ptr::eq(self, other) }
}
pub fn custom(value: Option<Custom>) -> bool { value != Some(Custom(1)) }
pub fn references(value: Option<&'static u64>) -> bool { value != Some(&1) }
pub fn nested(value: Option<Option<u64>>) -> bool { value != Some(Some(1)) }
pub fn borrowed(value: &Option<u64>, other: &Option<u64>) -> bool { value != other }
pub fn callee_impostor(_: &Option<u64>, _: &Option<u64>) -> bool { true }
pub fn through_impostor(value: Option<u64>) -> bool { callee_impostor(&value, &Some(1)) }
"#;

fn instance<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let def = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .unwrap();
    Instance::mono(tcx, def.to_def_id())
}

fn positive(tcx: TyCtxt<'_>) {
    let mut instances = [
        "some_u64",
        "some_u8",
        "none_u64",
        "some_i32",
        "some_bool",
        "some_f32",
        "some_char",
        "some_u16",
        "some_u32",
        "some_u128",
        "some_usize",
        "some_i8",
        "some_i16",
        "some_i64",
        "some_i128",
        "some_isize",
        "some_f64",
    ]
    .map(|name| instance(tcx, name))
    .to_vec();
    let caller = instance(tcx, "generic_caller");
    let generic = tcx
        .instance_mir(caller.def)
        .basic_blocks
        .iter()
        .find_map(|block| {
            let TerminatorKind::Call {
                func: Operand::Constant(c),
                ..
            } = &block.terminator().kind
            else {
                return None;
            };
            let TyKind::FnDef(def, args) = *c.const_.ty().kind() else {
                return None;
            };
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def, args)
                .ok()
                .flatten()
        })
        .unwrap();
    instances.push(generic);
    for instance in instances {
        let original = tcx.instance_mir(instance.def);
        let proof = prove_promoted_option_comparisons_v1(tcx, instance).unwrap_or_else(|| {
            eprintln!("{instance:?}: {original:#?}");
            for p in tcx.promoted_mir(instance.def_id()) {
                eprintln!("PROMOTED {p:#?}");
            }
            panic!("exact source promotion and core comparison");
        });
        assert_eq!(proof.instance(), instance);
        assert_eq!(proof.materializations.len(), 1);
        let m = &proof.materializations[0];
        let expanded = proof.expand_mir(tcx);
        let selected = super::super::super::production_mir_v1::production_mir_v1(tcx, instance);
        assert!(selected.is_source_expansion());
        assert_eq!(selected.instance(), instance);
        assert_eq!(format!("{expanded:?}"), format!("{:?}", selected.body()));
        assert_eq!(
            selected.expansion_fingerprint(tcx),
            Some(proof.expansion_fingerprint(tcx, &expanded))
        );
        assert_eq!(original.basic_blocks.len(), expanded.basic_blocks.len());
        assert_eq!(expanded.local_decls.len(), original.local_decls.len() + 1);
        for (id, block) in original.basic_blocks.iter_enumerated() {
            assert_eq!(
                format!("{:?}", block.terminator()),
                format!("{:?}", expanded.basic_blocks[id].terminator()),
                "every original call, branch and unwind edge is retained"
            );
            if id != m.block {
                assert_eq!(
                    format!("{:?}", block.statements),
                    format!("{:?}", expanded.basic_blocks[id].statements)
                );
            }
        }
        assert!(
            matches!(expanded.basic_blocks[m.block].statements[m.statement].kind,
            StatementKind::Assign(ref a) if matches!(a.1, Rvalue::Aggregate(..)))
        );
        assert!(
            matches!(expanded.basic_blocks[m.block].statements[m.statement + 1].kind,
            StatementKind::Assign(ref a) if matches!(a.1, Rvalue::Ref(_, BorrowKind::Shared, _)))
        );
        assert_eq!(
            format!("{original:?}"),
            format!("{:?}", tcx.instance_mir(instance.def)),
            "never mutate source MIR"
        );
        let mut changed = expanded.clone();
        changed.basic_blocks_mut()[m.block].statements[m.statement].kind = StatementKind::Nop;
        assert_ne!(
            proof.expansion_fingerprint(tcx, &expanded),
            proof.expansion_fingerprint(tcx, &changed)
        );
    }
}

fn source_negatives(tcx: TyCtxt<'_>) {
    for name in [
        "escapes",
        "observes",
        "custom",
        "references",
        "nested",
        "borrowed",
        "through_impostor",
    ] {
        assert!(
            prove_promoted_option_comparisons_v1(tcx, instance(tcx, name)).is_none(),
            "{name}"
        );
    }
    let root = instance(tcx, "some_u64");
    let proof = prove_promoted_option_comparisons_v1(tcx, root).unwrap();
    let block = &tcx.instance_mir(root.def).basic_blocks[proof.materializations[0].block];
    let TerminatorKind::Call {
        func: Operand::Constant(c),
        ..
    } = &block.terminator().kind
    else {
        unreachable!()
    };
    let TyKind::FnDef(def, args) = *c.const_.ty().kind() else {
        unreachable!()
    };
    let external = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def, args)
        .unwrap()
        .unwrap();
    assert!(!external.def_id().is_local());
    assert!(
        prove_promoted_option_comparisons_v1(tcx, external).is_none(),
        "no cross-crate source-safety authority"
    );
}

fn mutations(tcx: TyCtxt<'_>) {
    let root = instance(tcx, "some_u64");
    let source = tcx.instance_mir(root.def);
    let proof = prove_promoted_option_comparisons_v1(tcx, root).unwrap();
    let m = &proof.materializations[0];
    for mutation in 0..12 {
        let mut changed = source.clone();
        match mutation {
            0 => changed.source.instance = instance(tcx, "some_u8").def,
            1 => changed.basic_blocks_mut()[m.block].is_cleanup = true,
            2 => changed.basic_blocks_mut()[m.block]
                .statements
                .push(Statement::new(
                    source.basic_blocks[m.block].statements[m.statement].source_info,
                    StatementKind::Nop,
                )),
            3 => {
                changed.local_decls[m.reference].ty =
                    Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, m.value_ty)
            }
            4 => {
                let StatementKind::Assign(a) =
                    &mut changed.basic_blocks_mut()[m.block].statements[m.statement].kind
                else {
                    unreachable!()
                };
                let Rvalue::Use(Operand::Constant(c)) = &mut a.1 else {
                    unreachable!()
                };
                let Const::Unevaluated(mut value, ty) = c.const_ else {
                    unreachable!()
                };
                value.def = instance(tcx, "some_u8").def_id();
                c.const_ = Const::Unevaluated(value, ty);
            }
            5 => {
                let StatementKind::Assign(a) =
                    &mut changed.basic_blocks_mut()[m.block].statements[m.statement].kind
                else {
                    unreachable!()
                };
                let Rvalue::Use(Operand::Constant(c)) = &mut a.1 else {
                    unreachable!()
                };
                let Const::Unevaluated(mut value, ty) = c.const_ else {
                    unreachable!()
                };
                value.promoted = Some(Promoted::from_usize(9999));
                c.const_ = Const::Unevaluated(value, ty);
            }
            6 => {
                let reference = source.basic_blocks[m.block].statements[m.statement].source_info;
                let read = Statement::new(
                    reference,
                    StatementKind::PlaceMention(Box::new(m.reference.into())),
                );
                let target = match changed.basic_blocks[m.block].terminator().kind {
                    TerminatorKind::Call {
                        target: Some(t), ..
                    } => t,
                    _ => unreachable!(),
                };
                changed.basic_blocks_mut()[target]
                    .statements
                    .insert(0, read);
            }
            7 => {
                let TerminatorKind::Call { args, .. } =
                    &mut changed.basic_blocks_mut()[m.block].terminator_mut().kind
                else {
                    unreachable!()
                };
                args[0].node = Operand::Copy(m.reference.into());
            }
            8 => {
                let TerminatorKind::Call { unwind, .. } =
                    &mut changed.basic_blocks_mut()[m.block].terminator_mut().kind
                else {
                    unreachable!()
                };
                *unwind = UnwindAction::Cleanup(m.block);
            }
            9 | 10 | 11 => {
                let target = match changed.basic_blocks[m.block].terminator().kind {
                    TerminatorKind::Call {
                        target: Some(t), ..
                    } => t,
                    _ => unreachable!(),
                };
                let extra = changed
                    .local_decls
                    .push(source.local_decls[m.reference].clone());
                let (destination, value) = match mutation {
                    9 => (m.reference, Rvalue::Use(Operand::Copy(extra.into()))),
                    10 => (extra, Rvalue::Use(Operand::Copy(m.reference.into()))),
                    11 => (
                        extra,
                        Rvalue::Ref(
                            tcx.lifetimes.re_erased,
                            BorrowKind::Shared,
                            m.reference.into(),
                        ),
                    ),
                    _ => unreachable!(),
                };
                changed.basic_blocks_mut()[target].statements.insert(
                    0,
                    Statement::new(
                        source.basic_blocks[m.block].statements[m.statement].source_info,
                        StatementKind::Assign(Box::new((destination.into(), value))),
                    ),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            prove_body(tcx, root, &changed).is_none(),
            "caller mutation {mutation}"
        );
    }
    let promoted = &tcx.promoted_mir(root.def_id())[m.promotion];
    for mutation in 0..6 {
        let mut changed = promoted.clone();
        match mutation {
            0 => changed.source.promoted = None,
            1 => changed.source.instance = instance(tcx, "some_u8").def,
            2 => changed.basic_blocks_mut()[BasicBlock::from_usize(0)].is_cleanup = true,
            3 => changed.local_decls[Local::from_usize(1)].ty = tcx.types.u64,
            4 => {
                let StatementKind::Assign(a) =
                    &mut changed.basic_blocks_mut()[BasicBlock::from_usize(0)].statements[1].kind
                else {
                    unreachable!()
                };
                a.1 = Rvalue::Use(Operand::Copy(Local::from_usize(0).into()));
            }
            5 => {
                changed.basic_blocks_mut()[BasicBlock::from_usize(0)].statements[0].kind =
                    StatementKind::Nop
            }
            _ => unreachable!(),
        }
        assert!(
            promoted_initializer(tcx, root, m.promotion, &changed, m.value_ty).is_none(),
            "promotion mutation {mutation}"
        );
    }
}

fn scalar_and_budget_negatives(tcx: TyCtxt<'_>) {
    use rustc_abi::Size;
    use rustc_middle::mir::interpret::Scalar;
    for (name, bits, bytes, wrong_type) in [
        ("some_u64", 1_u128, 1, false),
        ("some_u64", 1, 8, true),
        ("some_bool", 2, 1, false),
        ("some_char", 0xd800, 4, false),
    ] {
        let root = instance(tcx, name);
        let proof = prove_promoted_option_comparisons_v1(tcx, root).unwrap();
        let m = &proof.materializations[0];
        let mut changed = tcx.promoted_mir(root.def_id())[m.promotion].clone();
        let StatementKind::Assign(a) =
            &mut changed.basic_blocks_mut()[BasicBlock::from_usize(0)].statements[0].kind
        else {
            unreachable!()
        };
        let Rvalue::Aggregate(_, operands) = &mut a.1 else {
            unreachable!()
        };
        let Operand::Constant(c) = &mut operands.raw[0] else {
            unreachable!()
        };
        let ty = if wrong_type {
            tcx.types.i64
        } else {
            c.const_.ty()
        };
        c.const_ = Const::from_scalar(tcx, Scalar::from_uint(bits, Size::from_bytes(bytes)), ty);
        assert!(
            promoted_initializer(tcx, root, m.promotion, &changed, m.value_ty).is_none(),
            "{name}, {bits}, {bytes}, {wrong_type}"
        );
    }
    let root = instance(tcx, "some_u64");
    let source = tcx.instance_mir(root.def);
    for mutation in 0..3 {
        let mut changed = source.clone();
        match mutation {
            0 => changed.local_decls.resize(
                MAX_LOCALS + 1,
                source.local_decls[Local::from_usize(1)].clone(),
            ),
            1 => {
                let last = source.basic_blocks.iter().last().unwrap().clone();
                for _ in 0..MAX_BLOCKS {
                    changed.basic_blocks_mut().push(last.clone());
                }
            }
            2 => {
                let entry = BasicBlock::from_usize(0);
                let info = source.basic_blocks[entry].statements[0].source_info;
                changed.basic_blocks_mut()[BasicBlock::from_usize(1)]
                    .statements
                    .extend((0..MAX_STATEMENTS).map(|_| Statement::new(info, StatementKind::Nop)));
            }
            _ => unreachable!(),
        }
        assert!(
            prove_body(tcx, root, &changed).is_none(),
            "bounded input {mutation}"
        );
    }
    let mut uses = SoleUses {
        slots: vec![],
        remaining: 1,
        valid: true,
    };
    assert!(uses.charge(1));
    assert!(!uses.charge(1));
    assert_eq!(uses.remaining, 0);
    assert!(!uses.valid);
    assert!(!uses.charge(0), "exhausted proof cannot regain authority");
}

struct Probe {
    mode: u8,
    ran: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("promoted_option_comparison.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        match self.mode {
            0 => positive(tcx),
            1 => source_negatives(tcx),
            2 => mutations(tcx),
            3 => scalar_and_budget_negatives(tcx),
            _ => unreachable!(),
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(mode: u8, amd: bool, mir_opt: Option<u8>) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=promoted_option_comparison".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cdebuginfo=2".into(),
        "-Cpanic=abort".into(),
    ];
    if amd {
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Zunstable-options".into(),
        ]);
        for (variable, name) in [
            ("FE2O3_WRAPPING_AMDGPU_CORE", "core"),
            ("FE2O3_WRAPPING_AMDGPU_BUILTINS", "compiler_builtins"),
        ] {
            let path = std::path::PathBuf::from(
                std::env::var_os(variable)
                    .expect("complete opt-in cached metadata; no Cargo/fallback"),
            );
            assert!(path.is_file());
            args.extend([
                "--extern".into(),
                format!("noprelude,nounused:{name}={}", path.display()),
                "-L".into(),
                format!("dependency={}", path.parent().unwrap().display()),
            ]);
        }
    }
    if let Some(level) = mir_opt {
        args.push(format!("-Zmir-opt-level={level}"));
    }
    args.push("-".into());
    let mut probe = Probe { mode, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn promoted_option_compare_actual_core_retains_calls() {
    run(0, false, None);
}
#[test]
fn promoted_option_compare_actual_core_source_negatives() {
    run(1, false, None);
}
#[test]
fn promoted_option_compare_actual_core_mutation_negatives() {
    run(2, false, None);
}
#[test]
fn promoted_option_compare_actual_core_scalar_and_budget_negatives() {
    run(3, false, None);
}
#[test]
#[ignore = "requires complete FE2O3_WRAPPING_AMDGPU_CORE/BUILTINS; no Cargo"]
fn promoted_option_compare_actual_amdgpu_profiles() {
    for level in [None, Some(0)] {
        for mode in 0..4 {
            run(mode, true, level);
        }
    }
}
