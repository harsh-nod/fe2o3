use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{ConstOperand, SourceInfo, Statement, SwitchTargets};
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

#[path = "mutations.rs"]
mod mutations;

const SOURCE: &str = r#"
#![no_std]
#![feature(core_intrinsics)]
#![allow(dead_code, internal_features)]
pub fn word(a: usize, b: usize) -> Option<usize> { a.checked_div(b) }
pub fn byte(a: u8, b: u8) -> Option<u8> { a.checked_div(b) }
pub fn half(a: u16, b: u16) -> Option<u16> { a.checked_div(b) }
pub fn single(a: u32, b: u32) -> Option<u32> { a.checked_div(b) }
pub fn double(a: u64, b: u64) -> Option<u64> { a.checked_div(b) }
pub fn wide(a: u128, b: u128) -> Option<u128> { a.checked_div(b) }
pub fn signed(a: isize, b: isize) -> Option<isize> { a.checked_div(b) }
pub fn signed_byte(a: i8, b: i8) -> Option<i8> { a.checked_div(b) }
pub fn signed_wide(a: i128, b: i128) -> Option<i128> { a.checked_div(b) }
pub fn other(a: usize, b: usize) -> Option<usize> { a.checked_rem(b) }
pub fn product(a: usize, b: usize) -> Option<usize> { a.checked_mul(b) }
pub fn unsafe_intrinsic(a: usize, b: usize) -> usize {
    unsafe { core::intrinsics::unchecked_div(a, b) }
}
pub fn checked_div(a: usize, b: usize) -> Option<usize> { a.checked_div(b) }
pub struct Impostor;
impl Impostor {
    pub fn checked_div(a: usize, b: usize) -> Option<usize> { a.checked_div(b) }
}
pub fn foreign(a: usize, b: usize) -> Option<usize> { Impostor::checked_div(a, b) }
pub fn unlikely(a: bool) -> bool { a }
pub fn panic_hint(_: bool) -> bool { panic!("not a hint") }
pub fn fake_hint(a: bool) -> bool { unlikely(a) }
pub fn panicking_hint(a: bool) -> bool { panic_hint(a) }
pub fn real_hint(a: bool) -> bool { core::intrinsics::unlikely(a) }
"#;

#[derive(Clone, Copy)]
enum Check {
    Actual,
    Arithmetic,
    Flow,
    StateAndBounds,
    All,
}

struct Probe {
    check: Check,
    amdgpu: bool,
    ran: bool,
}

fn local<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .expect("fixture function");
    Instance::mono(tcx, definition.to_def_id())
}

fn called_operand<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Operand<'tcx> {
    tcx.instance_mir(local(tcx, name).def)
        .basic_blocks
        .iter()
        .find_map(|block| match &block.terminator().kind {
            TerminatorKind::Call { func, .. } => Some(func.clone()),
            _ => None,
        })
        .expect("retained fixture call")
}

fn called<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    resolve_callee(tcx, &called_operand(tcx, name)).expect("resolved fixture call")
}

fn accepts<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) -> bool {
    let word = identity(tcx, instance).expect("unsigned checked_div identity");
    reviewed_body(tcx, instance, word, body)
}

fn scalar<'tcx>(_tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, bits: u64, value: u128) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span: DUMMY_SP,
        user_ty: None,
        const_: Const::Val(
            ConstValue::Scalar(rustc_middle::mir::interpret::Scalar::from_uint(
                value,
                rustc_abi::Size::from_bits(bits),
            )),
            ty,
        ),
    }))
}

fn assignment<'tcx>(place: Place<'tcx>, value: Rvalue<'tcx>) -> Statement<'tcx> {
    Statement::new(
        SourceInfo::outermost(DUMMY_SP),
        StatementKind::Assign(Box::new((place, value))),
    )
}

fn binary_site(body: &Body<'_>, expected: BinOp) -> (BasicBlock, usize) {
    body.basic_blocks.iter_enumerated().find_map(|(block, data)| {
        data.statements.iter().enumerate().find_map(|(index, statement)| {
            matches!(&statement.kind, StatementKind::Assign(assignment)
                if matches!(&assignment.1, Rvalue::BinaryOp(operation, _) if *operation == expected))
                .then_some((block, index))
        })
    }).expect("retained binary operation")
}

fn binary_mut<'a, 'tcx>(
    body: &'a mut Body<'tcx>,
    site: (BasicBlock, usize),
) -> (&'a mut BinOp, &'a mut Box<(Operand<'tcx>, Operand<'tcx>)>) {
    let StatementKind::Assign(assignment) =
        &mut body.basic_blocks_mut()[site.0].statements[site.1].kind
    else {
        panic!("assignment")
    };
    let Rvalue::BinaryOp(operation, operands) = &mut assignment.1 else {
        panic!("binary")
    };
    (operation, operands)
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("checked_div_authentication.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let cases = [
            ("word", tcx.types.usize),
            ("byte", tcx.types.u8),
            ("half", tcx.types.u16),
            ("single", tcx.types.u32),
            ("double", tcx.types.u64),
            ("wide", tcx.types.u128),
        ];
        if self.amdgpu {
            assert_eq!(tcx.sess.target.llvm_target, "amdgcn-amd-amdhsa");
            assert_eq!(tcx.data_layout.pointer_size().bits(), 64);
        }
        for (name, word) in cases {
            let instance = called(tcx, name);
            let body = tcx.instance_mir(instance.def);
            assert_eq!(identity(tcx, instance), Some(word), "{name}");
            let original = format!("{body:?}");
            assert!(
                authenticate_reviewed_safe_core_checked_div_v1(tcx, instance),
                "actual core {word:?} checked_div: {:#?}",
                body.basic_blocks
            );
            assert_eq!(
                format!("{body:?}"),
                original,
                "authentication must not rewrite MIR"
            );
            if std::env::var_os("FE2O3_CHECKED_DIV_DUMP_MIR").is_some() {
                eprintln!("{instance:?}\n{body:#?}");
            }
            binary_site(body, BinOp::Eq);
            binary_site(body, BinOp::Div);
            // The cached AMD body retains unlikely as a real call. Inspect
            // its body and cold_path edge, not a synthesized terminal summary.
            let mut saw_hint = false;
            for block in body.basic_blocks.iter() {
                if let TerminatorKind::Call { func, .. } = &block.terminator().kind {
                    let callee = resolve_callee(tcx, func).expect("closed callee");
                    if closed_hint(tcx, callee) {
                        saw_hint = true;
                        assert_eq!(tcx.instance_mir(callee.def).arg_count, 1);
                        for hint_block in tcx.instance_mir(callee.def).basic_blocks.iter() {
                            if let TerminatorKind::Call { func, .. } = &hint_block.terminator().kind
                            {
                                assert!(is_cold_path(tcx, resolve_callee(tcx, func).unwrap()));
                            }
                        }
                    } else {
                        assert!(is_cold_path(tcx, callee));
                    }
                }
            }
            if self.amdgpu {
                assert!(saw_hint, "actual AMD core must retain unlikely");
            }
            match self.check {
                Check::Arithmetic => mutations::arithmetic(tcx, instance, body),
                Check::Flow => mutations::flow(tcx, instance, body),
                Check::StateAndBounds => mutations::state_and_bounds(tcx, instance, body),
                Check::All => {
                    mutations::arithmetic(tcx, instance, body);
                    mutations::flow(tcx, instance, body);
                    mutations::state_and_bounds(tcx, instance, body);
                }
                Check::Actual => {}
            }
        }
        for name in [
            "signed",
            "signed_byte",
            "signed_wide",
            "other",
            "product",
            "foreign",
        ] {
            assert!(
                !authenticate_reviewed_safe_core_checked_div_v1(tcx, called(tcx, name)),
                "{name}"
            );
        }
        for name in [
            "checked_div",
            "word",
            "unsafe_intrinsic",
            "unlikely",
            "panic_hint",
        ] {
            assert!(
                !authenticate_reviewed_safe_core_checked_div_v1(tcx, local(tcx, name)),
                "{name}"
            );
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(check: Check, amdgpu: bool, mir_opt_zero: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=checked_div_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
    ];
    if mir_opt_zero {
        args.push("-Zmir-opt-level=0".into());
    }
    if amdgpu {
        for (name, crate_name) in [
            ("FE2O3_WRAPPING_AMDGPU_CORE", "core"),
            ("FE2O3_WRAPPING_AMDGPU_BUILTINS", "compiler_builtins"),
        ] {
            let path = std::path::PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
                panic!("set {name} to pinned, already-built AMDGPU metadata; no host fallback")
            }));
            assert!(path.is_file(), "{name}: {}", path.display());
            args.extend([
                "--extern".into(),
                format!("noprelude,nounused:{crate_name}={}", path.display()),
                "-L".into(),
                format!("dependency={}", path.parent().unwrap().display()),
            ]);
        }
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Cdebuginfo=2".into(),
            "-Zalways-encode-mir".into(),
            "-Zunstable-options".into(),
        ]);
    }
    args.push("-".into());
    let mut probe = Probe {
        check,
        amdgpu,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran, "checked_div metadata callback did not run");
}

#[test]
fn core_checked_div_authenticates_unsigned_nominal_bodies() {
    run(Check::Actual, false, false);
}

#[test]
fn core_checked_div_rejects_divisor_operation_and_type_mutations() {
    run(Check::Arithmetic, false, false);
}

#[test]
fn core_checked_div_rejects_guard_effect_and_call_mutations() {
    run(Check::Flow, false, false);
}

#[test]
fn core_checked_div_tracks_alias_moves_and_bounds() {
    run(Check::StateAndBounds, false, false);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS cached metadata"]
fn core_checked_div_actual_amdgpu_metadata() {
    run(Check::Actual, true, false);
    run(Check::Actual, true, true);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS cached metadata"]
fn core_checked_div_actual_amdgpu_mutations() {
    run(Check::All, true, false);
}
