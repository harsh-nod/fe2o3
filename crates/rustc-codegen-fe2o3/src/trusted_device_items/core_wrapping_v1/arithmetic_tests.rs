use super::profile_tests::{Profile, called};
use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

struct Probe {
    ran: bool,
}
const TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        let mut source = String::from("#![no_std]\n");
        for ty in TYPES {
            for op in ["add", "sub", "mul"] {
                source.push_str(&format!(
                    "pub fn {ty}_{op}(a:{ty},b:{ty})->{ty}{{a.wrapping_{op}(b)}}\n"
                ));
            }
        }
        source.push_str("pub fn wrapping_sub(a:u32,b:u32)->u32{a.wrapping_add(b)}\npub fn impostor(a:u32,b:u32)->u32{wrapping_sub(a,b)}\npub fn unchecked(a:u32,b:u32)->u32{unsafe{a.unchecked_sub(b)}}\n");
        config.input = Input::Str {
            name: FileName::Custom("plain_wrapping_arithmetic.rs".into()),
            input: source,
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use rustc_middle::mir::{BasicBlock, Local};
        let entry = BasicBlock::from_usize(0);
        for ty in TYPES {
            for (name, op) in [
                ("add", WrappingOperationV1::Add),
                ("sub", WrappingOperationV1::Subtract),
                ("mul", WrappingOperationV1::Multiply),
            ] {
                let instance = called(tcx, &format!("{ty}_{name}"));
                assert!(
                    authenticate_reviewed_safe_core_wrapping_helper_v1(tcx, instance),
                    "{instance:?}"
                );
                let body = tcx.instance_mir(instance.def);
                let selected = super::super::production_mir_v1::production_mir_v1(tcx, instance);
                assert!(!selected.is_source_expansion());
                assert!(
                    std::ptr::eq(selected.body(), body),
                    "ordinary original MIR remains collected"
                );
                let element = body.local_decls[Local::from_usize(1)].ty;
                assert!(!reviewed_wrapping_body_v1(
                    tcx,
                    body,
                    element,
                    if op == WrappingOperationV1::Subtract {
                        WrappingOperationV1::Add
                    } else {
                        WrappingOperationV1::Subtract
                    }
                ));
                let mut changed = body.clone();
                changed.basic_blocks_mut()[entry].is_cleanup = true;
                assert!(!reviewed_wrapping_body_v1(tcx, &changed, element, op));
                let mut changed = body.clone();
                changed.local_decls[Local::from_usize(2)].ty = if element == tcx.types.u8 {
                    tcx.types.u32
                } else {
                    tcx.types.u8
                };
                assert!(!reviewed_wrapping_body_v1(tcx, &changed, element, op));
                if let [statement] = body.basic_blocks[entry].statements.as_slice()
                    && matches!(statement.kind, StatementKind::Assign(_))
                {
                    let mut changed = body.clone();
                    let StatementKind::Assign(assignment) =
                        &mut changed.basic_blocks_mut()[entry].statements[0].kind
                    else {
                        unreachable!()
                    };
                    let Rvalue::BinaryOp(_, operands) = &mut assignment.1 else {
                        panic!("closed primitive binary")
                    };
                    std::mem::swap(&mut operands.0, &mut operands.1);
                    assert!(!reviewed_wrapping_body_v1(tcx, &changed, element, op));
                    let mut changed = body.clone();
                    let StatementKind::Assign(assignment) =
                        &mut changed.basic_blocks_mut()[entry].statements[0].kind
                    else {
                        unreachable!()
                    };
                    let Rvalue::BinaryOp(operation, _) = &mut assignment.1 else {
                        unreachable!()
                    };
                    *operation = BinOp::SubUnchecked;
                    assert!(!reviewed_wrapping_body_v1(tcx, &changed, element, op));
                }
            }
        }
        for name in ["impostor", "unchecked"] {
            assert!(!authenticate_reviewed_safe_core_wrapping_helper_v1(
                tcx,
                called(tcx, name)
            ));
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(profile: Profile, mir_opt: Option<u8>) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=plain_wrapping_arithmetic".into(),
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
    profile_tests::append_args(&mut args, profile, mir_opt);
    args.push("-".into());
    let mut probe = Probe { ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn core_wrapping_arithmetic_actual_core_all_primitive_types_and_mutations() {
    run(Profile::HostSysroot, None);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn core_wrapping_arithmetic_actual_amdgpu_all_primitive_types_and_mutations() {
    run(Profile::AmdgpuCached, None);
    run(Profile::AmdgpuCached, Some(0));
}
