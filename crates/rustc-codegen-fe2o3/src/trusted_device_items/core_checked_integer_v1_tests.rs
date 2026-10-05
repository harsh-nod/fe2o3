use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::DefId;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{BasicBlock, Local, SwitchTargets, Terminator};
use rustc_span::DUMMY_SP;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const FIXTURE: &str = r#"
#![no_std]
pub fn add(a: usize, b: usize) -> Option<usize> { a.checked_add(b) }
pub fn div(a: usize, b: usize) -> Option<usize> { a.checked_div(b) }
pub fn mul(a: u32, b: u32) -> Option<u32> { a.checked_mul(b) }
pub fn overflowing_mul(a: u32, b: u32) -> (u32, bool) { a.overflowing_mul(b) }
pub fn multiple(a: u32, b: u32) -> bool { a.is_multiple_of(b) }
pub fn other_add(a: u32, b: u32) -> Option<u32> { a.checked_add(b) }
pub fn other_div(a: u32, b: u32) -> Option<u32> { a.checked_div(b) }
pub fn other_mul(a: usize, b: usize) -> Option<usize> { a.checked_mul(b) }
pub fn other_multiple(a: usize, b: usize) -> bool { a.is_multiple_of(b) }
pub fn other_sub(a: usize, b: usize) -> Option<usize> { a.checked_sub(b) }
pub fn other_rem(a: usize, b: usize) -> Option<usize> { a.checked_rem(b) }
pub fn other_overflowing_mul(a: usize, b: usize) -> (usize, bool) { a.overflowing_mul(b) }
pub fn other_overflowing_add(a: u32, b: u32) -> (u32, bool) { a.overflowing_add(b) }
pub fn signed_add(a: isize, b: isize) -> Option<isize> { a.checked_add(b) }
pub fn identity(a: u32) -> u32 { <u32 as core::convert::From<u32>>::from(a) }
mod user {
    pub struct Value;
    impl Value {
        pub fn checked_add(a: usize, b: usize) -> Option<usize> { Some(a + b) }
    }
}
pub fn user_add(a: usize, b: usize) -> Option<usize> { user::Value::checked_add(a, b) }
pub fn local_add(a: usize, b: usize) -> Option<usize> { Some(a + b) }
pub unsafe fn unsafe_add(a: usize, b: usize) -> Option<usize> { Some(a + b) }
pub extern "C" fn foreign_add(a: usize, b: usize) -> usize { a + b }
"#;

#[derive(Clone, Default)]
struct Results {
    genuine: usize,
    refusals: usize,
    shapes: usize,
    locals: usize,
    scopes: usize,
    operations: usize,
    operands: usize,
    control: usize,
}

#[derive(Default)]
struct FixtureCallbacks {
    results: Option<Results>,
}

fn local_function(tcx: TyCtxt<'_>, name: &str) -> DefId {
    tcx.iter_local_def_id().find(|definition| {
        tcx.def_kind(definition.to_def_id()) == DefKind::Fn
            && tcx.item_name(definition.to_def_id()).as_str() == name
    }).expect("fixture function").to_def_id()
}

fn fixture_call<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> (Instance<'tcx>, Terminator<'tcx>) {
    let caller = Instance::mono(tcx, local_function(tcx, name));
    tcx.instance_mir(caller.def).basic_blocks.iter().find_map(|block| {
        let terminator = block.terminator();
        let TerminatorKind::Call { func: Operand::Constant(function), .. } = &terminator.kind else {
            return None;
        };
        let TyKind::FnDef(definition, arguments) = *function.const_.ty().kind() else {
            return None;
        };
        let arguments = caller.try_instantiate_mir_and_normalize_erasing_regions(
            tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(arguments),
        ).ok()?;
        let instance = Instance::try_resolve(
            tcx, TypingEnv::fully_monomorphized(), definition, arguments,
        ).ok()??;
        Some((instance, terminator.clone()))
    }).expect("actual resolved fixture method")
}

fn reject_mutation<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>,
    label: &str, mutate: impl FnOnce(&mut Body<'tcx>),
) {
    let mut changed = body.clone();
    mutate(&mut changed);
    assert!(!checked_body(tcx, instance, &changed), "accepted {instance:?} mutation: {label}");
}

impl Callbacks for FixtureCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let mut results = Results::default();
        for name in ["other_add", "other_div", "other_mul", "other_multiple", "other_sub",
            "other_rem", "other_overflowing_mul", "other_overflowing_add", "signed_add", "identity", "user_add"] {
            let (instance, _) = fixture_call(tcx, name);
            assert!(signature(tcx, instance).is_none(), "accepted signature {name}");
            assert!(!authenticate_reviewed_safe_core_checked_integer_v1(tcx, instance));
            results.refusals += 1;
        }
        for name in ["local_add", "unsafe_add", "foreign_add"] {
            let instance = Instance::mono(tcx, local_function(tcx, name));
            assert!(signature(tcx, instance).is_none());
            assert!(!authenticate_reviewed_safe_core_checked_integer_v1(tcx, instance));
            results.refusals += 1;
        }
        let local_instance = Instance::mono(tcx, local_function(tcx, "local_add"));
        let set_scope = tcx.instance_mir(local_instance.def).source_scopes[SourceScope::from_u32(0)]
            .local_data.clone();
        assert!(matches!(&set_scope, ClearCrossCrate::Set(_)));
        for name in ["add", "div", "mul", "overflowing_mul", "multiple"] {
            let (instance, call) = fixture_call(tcx, name);
            let body = tcx.instance_mir(instance.def);
            assert!(signature(tcx, instance).is_some());
            assert!(checked_body(tcx, instance, body),
                "actual integer body {instance:?}: args={} locals={} blocks={} scopes={}",
                body.arg_count, body.local_decls.len(), body.basic_blocks.len(), body.source_scopes.len());
            assert!(authenticate_reviewed_safe_core_checked_integer_v1(tcx, instance));
            results.genuine += 1;
            for count in [1, 3] {
                reject_mutation(tcx, instance, body, "argument count", |body| body.arg_count = count);
            }
            reject_mutation(tcx, instance, body, "missing local", |body| { body.local_decls.pop(); });
            reject_mutation(tcx, instance, body, "extra local", |body| {
                let extra = body.local_decls[Local::from_u32(1)].clone();
                body.local_decls.push(extra);
            });
            reject_mutation(tcx, instance, body, "missing block", |body| { body.basic_blocks.as_mut().pop(); });
            reject_mutation(tcx, instance, body, "extra block", |body| {
                let extra = body.basic_blocks[BasicBlock::from_u32(0)].clone();
                body.basic_blocks.as_mut().push(extra);
            });
            reject_mutation(tcx, instance, body, "missing scope", |body| { body.source_scopes.pop(); });
            reject_mutation(tcx, instance, body, "extra scope", |body| {
                let extra = body.source_scopes[SourceScope::from_u32(0)].clone();
                body.source_scopes.push(extra);
            });
            results.shapes += 8;
            for (local, _) in body.local_decls.iter_enumerated() {
                reject_mutation(tcx, instance, body, "local type", |body| body.local_decls[local].ty = tcx.types.u128);
                reject_mutation(tcx, instance, body, "local scope", |body| {
                    body.local_decls[local].source_info.scope = SourceScope::from_u32(99);
                });
                results.locals += 2;
            }
            for (scope, original) in body.source_scopes.iter_enumerated() {
                reject_mutation(tcx, instance, body, "scope parent", |body| {
                    body.source_scopes[scope].parent_scope = if original.parent_scope.is_none() {
                        Some(scope)
                    } else { None };
                });
                reject_mutation(tcx, instance, body, "inline parent", |body| {
                    body.source_scopes[scope].inlined_parent_scope = if original.inlined_parent_scope.is_none() {
                        Some(scope)
                    } else { None };
                });
                reject_mutation(tcx, instance, body, "inline identity", |body| {
                    body.source_scopes[scope].inlined = Some((instance, DUMMY_SP));
                });
                reject_mutation(tcx, instance, body, "non-clear scope data", |body| {
                    body.source_scopes[scope].local_data = set_scope.clone();
                });
                results.scopes += 4;
            }
            for (block, original) in body.basic_blocks.iter_enumerated() {
                reject_mutation(tcx, instance, body, "cleanup block", |body| {
                    body.basic_blocks.as_mut()[block].is_cleanup = true;
                });
                reject_mutation(tcx, instance, body, "extra operation", |body| {
                    let mut extra = body.basic_blocks.iter()
                        .find_map(|block| block.statements.first())
                        .expect("actual integer statement").clone();
                    extra.source_info = original.terminator().source_info;
                    extra.kind = StatementKind::Nop;
                    body.basic_blocks.as_mut()[block].statements.push(extra);
                });
                results.operations += 2;
                reject_mutation(tcx, instance, body, "missing terminator", |body| {
                    body.basic_blocks.as_mut()[block].terminator = None;
                });
                reject_mutation(tcx, instance, body, "wrong terminator", |body| {
                    body.basic_blocks.as_mut()[block].terminator_mut().kind = TerminatorKind::Unreachable;
                });
                reject_mutation(tcx, instance, body, "terminator scope", |body| {
                    body.basic_blocks.as_mut()[block].terminator_mut().source_info.scope = SourceScope::from_u32(99);
                });
                results.control += 3;
                for (index, original) in original.statements.iter().enumerate() {
                    reject_mutation(tcx, instance, body, "statement scope", |body| {
                        body.basic_blocks.as_mut()[block].statements[index].source_info.scope = SourceScope::from_u32(99);
                    });
                    results.scopes += 1;
                    reject_mutation(tcx, instance, body, "missing statement", |body| {
                        body.basic_blocks.as_mut()[block].statements.remove(index);
                    });
                    reject_mutation(tcx, instance, body, "replaced statement", |body| {
                        body.basic_blocks.as_mut()[block].statements[index].kind = StatementKind::Nop;
                    });
                    results.operations += 2;
                    if let StatementKind::Assign(assignment) = &original.kind {
                        reject_mutation(tcx, instance, body, "destination", |body| {
                            let StatementKind::Assign(assignment) = &mut body.basic_blocks.as_mut()[block].statements[index].kind else { unreachable!() };
                            assignment.0 = Place::from(Local::from_u32(99));
                        });
                        results.operands += 1;
                        match &assignment.1 {
                            Rvalue::BinaryOp(_, _) => {
                                for field in 0..3 {
                                    reject_mutation(tcx, instance, body, "binary operator/operand", |body| {
                                        let StatementKind::Assign(assignment) = &mut body.basic_blocks.as_mut()[block].statements[index].kind else { unreachable!() };
                                        let Rvalue::BinaryOp(op, args) = &mut assignment.1 else { unreachable!() };
                                        match field {
                                            0 => *op = BinOp::BitXor,
                                            1 => args.0 = Operand::Copy(Place::from(Local::from_u32(99))),
                                            _ => args.1 = Operand::Copy(Place::from(Local::from_u32(99))),
                                        }
                                    });
                                    results.operands += 1;
                                }
                            }
                            Rvalue::Use(_) => {
                                reject_mutation(tcx, instance, body, "use operand/None", |body| {
                                    let StatementKind::Assign(assignment) = &mut body.basic_blocks.as_mut()[block].statements[index].kind else { unreachable!() };
                                    assignment.1 = Rvalue::Use(Operand::Copy(Place::from(Local::from_u32(1))));
                                });
                                results.operands += 1;
                            }
                            Rvalue::Aggregate(_, _) => {
                                for field in 0..3 {
                                    reject_mutation(tcx, instance, body, "Some variant/arity/operand", |body| {
                                        let StatementKind::Assign(assignment) = &mut body.basic_blocks.as_mut()[block].statements[index].kind else { unreachable!() };
                                        let Rvalue::Aggregate(kind, operands) = &mut assignment.1 else { unreachable!() };
                                        match field {
                                            0 => {
                                                let AggregateKind::Adt(def, variant, _, _, _) = &mut **kind else { unreachable!() };
                                                *variant = tcx.adt_def(*def).variant_index_with_id(tcx.lang_items().option_none_variant().unwrap());
                                            }
                                            1 => { operands.pop(); }
                                            _ => { *operands.iter_mut().next().unwrap() = Operand::Copy(Place::from(Local::from_u32(99))); }
                                        }
                                    });
                                    results.operands += 1;
                                }
                            }
                            _ => panic!("unreviewed actual assignment"),
                        }
                    }
                }
                match &original.terminator().kind {
                    TerminatorKind::SwitchInt { .. } => {
                        for field in 0..4 {
                            reject_mutation(tcx, instance, body, "switch edge/value/mode", |body| {
                                let TerminatorKind::SwitchInt { discr, targets } = &mut body.basic_blocks.as_mut()[block].terminator_mut().kind else { unreachable!() };
                                match field {
                                    0 => *discr = Operand::Move(Place::from(Local::from_u32(2))),
                                    1 => *targets = SwitchTargets::new([(1, BasicBlock::from_u32(4))].into_iter(), BasicBlock::from_u32(2)),
                                    2 => *targets = SwitchTargets::new([(0, BasicBlock::from_u32(99))].into_iter(), targets.otherwise()),
                                    _ => *targets = SwitchTargets::new(targets.iter(), BasicBlock::from_u32(99)),
                                }
                            });
                            results.control += 1;
                        }
                    }
                    TerminatorKind::Call { .. } => {
                        for field in 0..4 {
                            reject_mutation(tcx, instance, body, "cold call identity/arguments/edge/unwind", |body| {
                                let TerminatorKind::Call { func, args, target, unwind, .. } = &mut body.basic_blocks.as_mut()[block].terminator_mut().kind else { unreachable!() };
                                match field {
                                    0 => {
                                        let TerminatorKind::Call { func: other, .. } = &call.kind else { unreachable!() };
                                        *func = other.clone();
                                    }
                                    1 => *args = vec![rustc_span::Spanned { node: Operand::Copy(Place::from(Local::from_u32(1))), span: DUMMY_SP }].into_boxed_slice(),
                                    2 => *target = Some(BasicBlock::from_u32(99)),
                                    _ => *unwind = UnwindAction::Continue,
                                }
                            });
                            results.control += 1;
                        }
                    }
                    TerminatorKind::Assert { .. } => {
                        for field in 0..5 {
                            reject_mutation(tcx, instance, body, "assert condition/message/edge/unwind", |body| {
                                let TerminatorKind::Assert { cond, expected, msg, target, unwind } = &mut body.basic_blocks.as_mut()[block].terminator_mut().kind else { unreachable!() };
                                match field {
                                    0 => *cond = Operand::Copy(Place::from(Local::from_u32(4))),
                                    1 => *expected = true,
                                    2 => **msg = AssertKind::DivisionByZero(Operand::Copy(Place::from(Local::from_u32(1)))),
                                    3 => *target = BasicBlock::from_u32(99),
                                    _ => *unwind = UnwindAction::Continue,
                                }
                            });
                            results.control += 1;
                        }
                    }
                    TerminatorKind::Goto { .. } => {
                        reject_mutation(tcx, instance, body, "goto edge", |body| {
                            let TerminatorKind::Goto { target } = &mut body.basic_blocks.as_mut()[block].terminator_mut().kind else { unreachable!() };
                            *target = BasicBlock::from_u32(99);
                        });
                        results.control += 1;
                    }
                    TerminatorKind::Return => {}
                    _ => panic!("unreviewed actual terminator"),
                }
            }
        }
        self.results = Some(results);
        Compilation::Stop
    }
}
struct FixtureDirectory(PathBuf);

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Keep the passing Result fixture untouched. This independent cohort builds
// genuine core with the same measured gfx942 release settings once per process.
fn target_core_metadata(
    fixture: &FixtureDirectory,
    sysroot: &std::path::Path,
) -> (PathBuf, PathBuf, PathBuf) {
    let manifest = fixture.0.join("Cargo.toml");
    fs::write(&manifest, r#"[package]
name = "fe2o3_core_checked_integer_fixture"
version = "0.0.0"
edition = "2024"
publish = false
[workspace]
[lib]
path = "fixture.rs"
"#).expect("write private target-core fixture manifest");
    let target = fixture.0.join("target-core");
    let configure = |command: &mut Command| {
        command.current_dir(&fixture.0)
            .env("RUSTC", sysroot.join("bin/rustc"))
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env("CARGO_BUILD_JOBS", "2")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_CACHE_AUTO_CLEAN_FREQUENCY", "never")
            .env(
                "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
            );
    };
    let mut lock = Command::new(env!("CARGO"));
    configure(&mut lock);
    lock.args(["generate-lockfile", "--offline", "--manifest-path"]).arg(&manifest);
    let lock = crate::process_execution::capture_output(&mut lock)
        .expect("generate private target-core fixture lockfile");
    assert!(lock.stdout.len() <= 4 << 20 && lock.stderr.len() <= 4 << 20);
    assert!(
        lock.status.success(),
        "target-core fixture lockfile failed: {}",
        String::from_utf8_lossy(&lock.stderr),
    );
    let mut command = Command::new(env!("CARGO"));
    configure(&mut command);
    command.args(["check", "--offline", "--locked", "--release", "-Zbuild-std=core",
        "--target", "amdgcn-amd-amdhsa", "--message-format=json", "--target-dir"])
        .arg(&target).args(["--manifest-path"]).arg(&manifest).arg("--lib");
    let output = crate::process_execution::capture_output(&mut command)
        .expect("build genuine AMD target core for checked integer callback");
    assert!(output.stdout.len() <= 4 << 20 && output.stderr.len() <= 4 << 20);
    assert!(output.status.success(), "target-core build failed: {}", String::from_utf8_lossy(&output.stderr));
    let expected_sources = [
        "lib/rustlib/src/rust/library/core/src/lib.rs",
        "lib/rustlib/src/rust/library/compiler-builtins/compiler-builtins/src/lib.rs",
    ].map(|relative| sysroot.join(relative).canonicalize().expect("pinned rust-src library source"));
    let dependencies = target.join("amdgcn-amd-amdhsa/release/deps")
        .canonicalize().expect("private target-core dependency directory");
    let mut selected: [Option<PathBuf>; 2] = [None, None];
    let mut finished = 0;
    for line in String::from_utf8(output.stdout).expect("Cargo JSON UTF-8").lines() {
        if line.trim().is_empty() { continue; }
        let record: serde_json::Value = serde_json::from_str(line).expect("structured Cargo message");
        if record["reason"].as_str() == Some("build-finished") {
            assert_eq!(record["success"].as_bool(), Some(true));
            finished += 1;
        }
        if record["reason"].as_str() != Some("compiler-artifact") { continue; }
        let role = match record["target"]["name"].as_str() {
            Some("core") => 0,
            Some("compiler_builtins") => 1,
            _ => continue,
        };
        assert!(selected[role].is_none(), "duplicate target library artifact");
        for field in ["kind", "crate_types"] {
            let values = record["target"][field].as_array().expect("target library kinds");
            assert!(values.len() == 1 && values[0].as_str() == Some("lib"));
        }
        assert_eq!(record["profile"]["test"].as_bool(), Some(false));
        assert_eq!(record["profile"]["opt_level"].as_str(), Some("3"));
        let source = PathBuf::from(
            record["target"]["src_path"].as_str().expect("target library source path"),
        );
        assert_eq!(source.canonicalize().expect("target library artifact source"), expected_sources[role]);
        let mut metadata = Vec::new();
        let mut libraries = Vec::new();
        for filename in record["filenames"].as_array().expect("target library artifact filenames") {
            let path = PathBuf::from(filename.as_str().expect("target library artifact filename"));
            let extension = path.extension().and_then(|value| value.to_str());
            if !matches!(extension, Some("rmeta" | "rlib")) { continue; }
            assert!(fs::symlink_metadata(&path)
                .expect("target library artifact metadata")
                .file_type()
                .is_file());
            let path = path.canonicalize().expect("canonical target library artifact");
            assert_eq!(path.parent(), Some(dependencies.as_path()));
            if extension == Some("rmeta") { metadata.push(path); } else { libraries.push(path); }
        }
        assert!(metadata.len() <= 1 && libraries.len() <= 1, "unique target library metadata/artifacts");
        selected[role] = metadata.pop().or_else(|| libraries.pop());
        assert!(selected[role].is_some(), "Cargo omitted target library metadata/artifact");
    }
    assert_eq!(finished, 1, "one successful Cargo build-finished message");
    (selected[0].take().expect("Cargo selected actual target core"),
     selected[1].take().expect("Cargo selected actual target compiler_builtins"), dependencies)
}

fn compiler_results() -> Results {
    static RESULTS: OnceLock<Result<Results, String>> = OnceLock::new();
    let result = RESULTS.get_or_init(|| std::panic::catch_unwind(|| {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        static DRIVER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = DRIVER_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let root = std::env::temp_dir().join(format!(
            "fe2o3-core-checked-integer-v1-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create core checked integer fixture directory");
        let fixture = FixtureDirectory(root);
        let source = fixture.0.join("fixture.rs");
        fs::write(&source, FIXTURE).expect("write compiler fixture");
        let mut command = Command::new("rustc");
        command.args(["--print", "sysroot"]);
        let sysroot = crate::process_execution::capture_output(&mut command).expect("query rustc sysroot");
        assert!(sysroot.status.success());
        let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 sysroot").trim().to_owned();
        let (core, builtins, dependencies) = target_core_metadata(&fixture, std::path::Path::new(&sysroot));
        let args = vec![
            "rustc".to_owned(), "--crate-name".to_owned(), "fe2o3_core_checked_integer_fixture".to_owned(),
            "--crate-type=lib".to_owned(), "--edition=2024".to_owned(), "--emit=metadata".to_owned(),
            "-Zmir-opt-level=0".to_owned(), "-Zinline-mir=no".to_owned(), "-Cpanic=abort".to_owned(),
            "--target=amdgcn-amd-amdhsa".to_owned(), "-Ctarget-cpu=gfx942".to_owned(),
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".to_owned(),
            "--extern".to_owned(), format!("core={}", core.display()),
            "--extern".to_owned(), format!("compiler_builtins={}", builtins.display()),
            "-L".to_owned(), format!("dependency={}", dependencies.display()),
            "--sysroot".to_owned(), sysroot,
            "-o".to_owned(), fixture.0.join("fixture.rmeta").display().to_string(),
            source.display().to_string(),
        ];
        let mut callbacks = FixtureCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        callbacks.results.expect("core checked integer callback")
    }).map_err(|payload| {
        if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else {
            "core checked integer fixture initialization panicked with a non-string payload".to_owned()
        }
    }));
    result
        .as_ref()
        .unwrap_or_else(|message| panic!("cached core checked integer fixture failure: {message}"))
        .clone()
}


#[test]
fn checked_integer_real_core_identity_and_signature_refusals() {
    let results = compiler_results();
    assert_eq!((results.genuine, results.refusals), (5, 14));
}

#[test]
fn checked_integer_real_body_rejects_shape_and_local_mutations() {
    let results = compiler_results();
    assert_eq!((results.shapes, results.locals), (40, 58));
}

#[test]
fn checked_integer_real_body_rejects_source_scope_and_inline_mutations() {
    assert_eq!(compiler_results().scopes, 82);
}

#[test]
fn checked_integer_real_body_rejects_extra_operations() {
    assert_eq!(compiler_results().operations, 110);
}

#[test]
fn checked_integer_real_body_rejects_operands_constants_and_variants() {
    assert_eq!(compiler_results().operands, 64);
}

#[test]
fn checked_integer_real_body_rejects_control_flow_and_unwind() {
    assert_eq!(compiler_results().control, 104);
}
