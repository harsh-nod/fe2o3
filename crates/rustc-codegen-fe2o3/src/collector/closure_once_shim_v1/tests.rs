use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{SourceInfo, Statement};
use rustc_session::config::Input;
use rustc_span::{DUMMY_SP, FileName};

const SOURCE: &str = r#"
#![no_std]
#![feature(fn_traits)]
#![allow(dead_code)]
pub fn invoke<F: Fn(u32) -> u32>(f: F, x: u32) -> u32 {
    core::ops::FnOnce::call_once(f, (x,))
}
pub fn invoke_mut<F: FnMut(u32) -> u32>(f: F, x: u32) -> u32 {
    core::ops::FnOnce::call_once(f, (x,))
}
pub fn entry(v: u32) -> u32 { invoke(|x| x ^ v, 7) }
pub fn entry_mut(mut v: u32) -> u32 { invoke_mut(|x| { v ^= x; v }, 7) }
pub fn option<F: Fn(usize) -> Option<usize>>(f: F) -> Option<usize> { Some(3).and_then(f) }
pub fn option_entry(v: usize) -> Option<usize> { option(|x| Some(x ^ v)) }
struct Dropping(u32);
impl Dropping { fn value(&self) -> u32 { self.0 } }
impl Drop for Dropping { fn drop(&mut self) { self.0 = 0; } }
pub fn entry_drop() -> u32 {
    let v = Dropping(7);
    invoke(move |x| x ^ v.value(), 7)
}
fn identity(x: u32) -> u32 { x }
pub fn entry_fn_pointer() -> u32 { invoke(identity as fn(u32) -> u32, 7) }
pub fn fake_call_once(x: u32) -> u32 { x }
unsafe fn unsafe_leaf(x: u32) -> u32 { x }
pub fn entry_unsafe(v: u32) -> u32 { invoke(|x| unsafe { unsafe_leaf(x ^ v) }, 7) }
"#;

pub(crate) fn entry<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .expect("fixture function");
    Instance::mono(tcx, definition.to_def_id())
}

pub(crate) fn first_callee<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) -> Instance<'tcx> {
    let body = tcx.instance_mir(caller.def);
    body.basic_blocks
        .iter()
        .find_map(|data| {
            let TerminatorKind::Call {
                func: Operand::Constant(callee),
                ..
            } = &data.terminator().kind
            else {
                return None;
            };
            let callee_ty = normalized_ty(tcx, caller, callee.const_.ty())?;
            let TyKind::FnDef(definition, arguments) = callee_ty.kind() else {
                return None;
            };
            Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                *definition,
                arguments,
            )
            .ok()
            .flatten()
        })
        .expect("fixture retained direct call")
}

pub(crate) fn shim<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    first_callee(tcx, first_callee(tcx, entry(tcx, name)))
}

struct Probe {
    check: for<'tcx> fn(TyCtxt<'tcx>),
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("closure_once_shim_authentication.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        (self.check)(tcx);
        self.ran = true;
        Compilation::Stop
    }
}

pub(crate) fn run(check: for<'tcx> fn(TyCtxt<'tcx>), panic: &str) {
    let mut command = std::process::Command::new("rustc");
    command.args(["--print", "sysroot"]);
    let sysroot = crate::process_execution::capture_output(&mut command).unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=closure_once_shim_authentication".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Copt-level=0".into(),
        format!("-Cpanic={panic}"),
        "-".into(),
    ];
    let mut probe = Probe { check, ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran, "compiler callback did not run");
}

#[test]
fn closure_once_shim_authenticates_actual_fn_fnmut_and_option_adapters() {
    for panic in ["abort", "unwind"] {
        run(
            |tcx| {
                for name in ["entry", "entry_mut"] {
                    let instance = shim(tcx, name);
                    assert!(matches!(instance.def, InstanceKind::ClosureOnceShim { .. }));
                    assert!(
                        !tcx.is_mir_available(instance.def_id()),
                        "trait declaration has no MIR"
                    );
                    assert!(authenticate_closure_once_shim_v1(tcx, instance), "{name}");
                    let contract = shim_identity(tcx, instance).unwrap();
                    assert_eq!(first_callee(tcx, instance), contract.closure);
                    assert!(tcx.is_mir_available(contract.closure.def_id()));
                    assert!(authenticated_closure_once_receiver_v1(
                        tcx,
                        instance,
                        local(1),
                        contract.environment
                    ));
                    assert!(!authenticated_closure_once_receiver_v1(
                        tcx,
                        instance,
                        local(2),
                        contract.environment
                    ));
                    assert!(!authenticated_closure_once_receiver_v1(
                        tcx,
                        instance,
                        local(1),
                        contract.tuple
                    ));
                    let caller = first_callee(tcx, entry(tcx, name));
                    assert!(!authenticated_closure_once_receiver_v1(
                        tcx,
                        caller,
                        local(1),
                        contract.environment
                    ));
                }
                let option = shim(tcx, "option_entry");
                let adapter = first_callee(tcx, option);
                assert!(authenticate_closure_once_shim_v1(tcx, adapter));
            },
            panic,
        );
    }
}

#[test]
fn closure_once_shim_rejects_nonclosure_dropping_and_forged_instance_identities() {
    run(
        |tcx| {
            for name in ["entry_drop", "entry_fn_pointer"] {
                assert!(
                    !authenticate_closure_once_shim_v1(tcx, shim(tcx, name)),
                    "{name}"
                );
            }
            let instance = shim(tcx, "entry");
            let contract = shim_identity(tcx, instance).unwrap();
            for def in [
                InstanceKind::Item(instance.def_id()),
                InstanceKind::ClosureOnceShim {
                    call_once: instance.def_id(),
                    track_caller: true,
                },
                InstanceKind::ClosureOnceShim {
                    call_once: contract.call_mut,
                    track_caller: false,
                },
                InstanceKind::ClosureOnceShim {
                    call_once: entry(tcx, "fake_call_once").def_id(),
                    track_caller: false,
                },
            ] {
                assert!(!authenticate_closure_once_shim_v1(
                    tcx,
                    Instance { def, ..instance }
                ));
            }
            for types in [
                vec![contract.environment],
                vec![tcx.types.u32, contract.tuple],
                vec![contract.environment, tcx.types.u32],
                vec![contract.environment, Ty::new_tup(tcx, &[tcx.types.usize])],
                vec![
                    contract.environment,
                    Ty::new_tup(tcx, &[tcx.types.u32; MAX_TUPLE_FIELDS + 1]),
                ],
            ] {
                let arguments = tcx
                    .mk_args_from_iter(types.into_iter().map(rustc_middle::ty::GenericArg::from));
                assert!(!authenticate_closure_once_shim_v1(
                    tcx,
                    Instance {
                        args: arguments,
                        ..instance
                    }
                ));
            }
        },
        "abort",
    );
}

#[test]
fn closure_once_shim_rejects_changed_source_signature_and_receiver_roles() {
    run(
        |tcx| {
            let instance = shim(tcx, "entry");
            let contract = shim_identity(tcx, instance).unwrap();
            let signature = tcx.normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                tcx.instantiate_bound_regions_with_erased(
                    tcx.fn_sig(instance.def_id())
                        .instantiate(tcx, instance.args),
                ),
            );
            assert!(signature_matches(signature, &contract));
            for signature in [
                FnSig {
                    safety: Safety::Unsafe,
                    ..signature
                },
                FnSig {
                    abi: ExternAbi::Rust,
                    ..signature
                },
                FnSig {
                    c_variadic: true,
                    ..signature
                },
                FnSig {
                    inputs_and_output: tcx.mk_type_list(&[
                        contract.tuple,
                        contract.environment,
                        contract.output,
                    ]),
                    ..signature
                },
                FnSig {
                    inputs_and_output: tcx.mk_type_list(&[
                        contract.environment,
                        contract.tuple,
                        tcx.types.unit,
                    ]),
                    ..signature
                },
            ] {
                assert!(!signature_matches(signature, &contract));
            }
            let original = tcx.instance_mir(instance.def);
            for index in 0..4 {
                let mut changed = original.clone();
                changed.local_decls[local(index)].ty = tcx.types.unit;
                assert!(!reviewed_body(tcx, instance, &changed, &contract));
            }
            let mut changed = original.clone();
            changed.spread_arg = None;
            assert!(!reviewed_body(tcx, instance, &changed, &contract));
            changed = original.clone();
            changed.arg_count = 1;
            assert!(!reviewed_body(tcx, instance, &changed, &contract));
            changed = original.clone();
            changed.source.instance = InstanceKind::Item(instance.def_id());
            assert!(!reviewed_body(tcx, instance, &changed, &contract));
            changed = original.clone();
            changed
                .local_decls
                .push(changed.local_decls[local(3)].clone());
            assert!(!reviewed_body(tcx, instance, &changed, &contract));
        },
        "abort",
    );
}

#[test]
fn closure_once_shim_rejects_changed_borrow_call_return_and_drop_effects() {
    run(
        |tcx| {
            let instance = shim(tcx, "entry");
            let contract = shim_identity(tcx, instance).unwrap();
            let original = tcx.instance_mir(instance.def);
            for borrow in [
                Rvalue::Ref(
                    tcx.lifetimes.re_erased,
                    BorrowKind::Shared,
                    Place::from(local(1)),
                ),
                Rvalue::Ref(
                    tcx.lifetimes.re_erased,
                    BorrowKind::Mut {
                        kind: MutBorrowKind::Default,
                    },
                    Place::from(local(2)),
                ),
                Rvalue::Use(Operand::Move(Place::from(local(1)))),
            ] {
                let mut changed = original.clone();
                let StatementKind::Assign(assignment) =
                    &mut changed.basic_blocks_mut()[block(0)].statements[0].kind
                else {
                    unreachable!()
                };
                assignment.1 = borrow;
                assert!(!reviewed_body(tcx, instance, &changed, &contract));
            }
            for mutation in 0..7 {
                let mut changed = original.clone();
                let TerminatorKind::Call {
                    func,
                    args,
                    destination,
                    target,
                    unwind,
                    ..
                } = &mut changed.basic_blocks_mut()[block(0)].terminator_mut().kind
                else {
                    unreachable!()
                };
                match mutation {
                    0 => args.swap(0, 1),
                    1 => args[0].node = Operand::Copy(Place::from(local(3))),
                    2 => args[1].node = Operand::Copy(Place::from(local(2))),
                    3 => *destination = Place::from(local(2)),
                    4 => *target = Some(block(0)),
                    5 => *unwind = UnwindAction::Continue,
                    6 => {
                        *func = Operand::function_handle(
                            tcx,
                            entry(tcx, "fake_call_once").def_id(),
                            [],
                            DUMMY_SP,
                        )
                    }
                    _ => unreachable!(),
                }
                assert!(
                    !reviewed_body(tcx, instance, &changed, &contract),
                    "call mutation {mutation}"
                );
            }
            for mutation in 0..4 {
                let mut changed = original.clone();
                let TerminatorKind::Drop {
                    place,
                    target,
                    unwind,
                    replace,
                    ..
                } = &mut changed.basic_blocks_mut()[block(1)].terminator_mut().kind
                else {
                    unreachable!()
                };
                match mutation {
                    0 => *place = Place::from(local(2)),
                    1 => *target = block(1),
                    2 => *unwind = UnwindAction::Continue,
                    3 => *replace = true,
                    _ => unreachable!(),
                }
                assert!(!reviewed_body(tcx, instance, &changed, &contract));
            }
        },
        "abort",
    );
}

#[test]
fn closure_once_shim_checks_every_cleanup_block_and_bounded_body_inventory() {
    run(
        |tcx| {
            let instance = shim(tcx, "entry");
            let contract = shim_identity(tcx, instance).unwrap();
            let original = tcx.instance_mir(instance.def);
            for index in 0..original.basic_blocks.len() {
                let mut changed = original.clone();
                changed.basic_blocks_mut()[block(index)]
                    .statements
                    .push(Statement::new(
                        SourceInfo::outermost(DUMMY_SP),
                        StatementKind::Nop,
                    ));
                assert!(!reviewed_body(tcx, instance, &changed, &contract));
                changed = original.clone();
                changed.basic_blocks_mut()[block(index)].is_cleanup =
                    !original.basic_blocks[block(index)].is_cleanup;
                assert!(!reviewed_body(tcx, instance, &changed, &contract));
                changed = original.clone();
                changed.basic_blocks_mut()[block(index)]
                    .terminator_mut()
                    .kind = TerminatorKind::Unreachable;
                assert!(!reviewed_body(tcx, instance, &changed, &contract));
            }
            let mut changed = original.clone();
            changed
                .basic_blocks_mut()
                .push(original.basic_blocks[block(2)].clone());
            assert!(!reviewed_body(tcx, instance, &changed, &contract));
            changed = original.clone();
            let scope = changed.source_scopes.iter().next().unwrap().clone();
            while changed.source_scopes.len() <= 4 {
                changed.source_scopes.push(scope.clone());
            }
            assert!(!reviewed_body(tcx, instance, &changed, &contract));
        },
        "unwind",
    );
}
