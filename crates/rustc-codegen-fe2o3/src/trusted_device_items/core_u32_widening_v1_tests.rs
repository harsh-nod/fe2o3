use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::DefId;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Local, NonDivergingIntrinsic, ProjectionElem, Terminator};
use rustc_middle::ty::TyKind;
use rustc_span::DUMMY_SP;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const FIXTURE: &str = r#"
#![no_std]
pub fn widen(value: u32) -> u64 { <u64 as core::convert::From<u32>>::from(value) }
pub fn widen_u16(value: u16) -> u64 { <u64 as core::convert::From<u16>>::from(value) }
pub fn widen_u128(value: u32) -> u128 { <u128 as core::convert::From<u32>>::from(value) }
pub fn widen_signed(value: u32) -> i64 { <i64 as core::convert::From<u32>>::from(value) }
pub fn identity(value: u32) -> u32 { <u32 as core::convert::From<u32>>::from(value) }
mod lookalike {
    pub trait From<T> { fn from(value: T) -> Self; }
    impl From<u32> for u64 { fn from(value: u32) -> Self { value as Self } }
}
pub fn user_from(value: u32) -> u64 { <u64 as lookalike::From<u32>>::from(value) }
pub fn local_cast(value: u32) -> u64 { value as u64 }
pub unsafe fn unsafe_cast(value: u32) -> u64 { value as u64 }
pub extern "C" fn foreign_abi_cast(value: u32) -> u64 { value as u64 }
"#;

#[derive(Clone, Default)]
struct Results {
    genuine: usize,
    refusals: usize,
    shapes: usize,
    locals: usize,
    scopes: usize,
    operations: usize,
    casts: usize,
    places_and_returns: usize,
}

#[derive(Default)]
struct FixtureCallbacks {
    results: Option<Results>,
}

fn local_function(tcx: TyCtxt<'_>, name: &str) -> DefId {
    tcx.iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .expect("fixture function")
        .to_def_id()
}

fn fixture_call<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> (Instance<'tcx>, Terminator<'tcx>) {
    let caller = Instance::mono(tcx, local_function(tcx, name));
    tcx.instance_mir(caller.def).basic_blocks.iter().find_map(|block| {
        let terminator = block.terminator();
        let TerminatorKind::Call {
            func: Operand::Constant(function),
            ..
        } = &terminator.kind else {
            return None;
        };
        let TyKind::FnDef(definition, arguments) = *function.const_.ty().kind() else {
            return None;
        };
        let arguments = caller
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(arguments),
            )
            .ok()?;
        let instance = Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            definition,
            arguments,
        )
        .ok()??;
        (tcx.item_name(instance.def_id()).as_str() == "from")
            .then(|| (instance, terminator.clone()))
    }).expect("real resolved conversion in fixture MIR")
}

fn reject_mutation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    label: &str,
    mutate: impl FnOnce(&mut Body<'tcx>),
) {
    let mut changed = body.clone();
    mutate(&mut changed);
    assert!(!widening_body_v1(tcx, instance, &changed), "accepted widening MIR mutation: {label}");
}

impl Callbacks for FixtureCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let mut results = Results::default();
        let (instance, actual_call) = fixture_call(tcx, "widen");
        let body = tcx.instance_mir(instance.def);
        assert!(widening_signature_v1(tcx, instance), "widening signature: {instance:?}");
        assert!(widening_body_v1(tcx, instance, body),
            "widening body: {instance:?}; arguments={} locals={} blocks={} scopes={}",
            body.arg_count, body.local_decls.len(), body.basic_blocks.len(), body.source_scopes.len());
        assert!(authenticate_reviewed_safe_core_u32_widening_v1(tcx, instance));
        results.genuine = 1;
        for name in ["widen_u16", "widen_u128", "widen_signed", "identity", "user_from"] {
            let (other, _) = fixture_call(tcx, name);
            assert!(!widening_signature_v1(tcx, other), "accepted signature: {name}");
            assert!(!authenticate_reviewed_safe_core_u32_widening_v1(tcx, other));
            results.refusals += 1;
        }
        for name in ["local_cast", "unsafe_cast", "foreign_abi_cast"] {
            let other = Instance::mono(tcx, local_function(tcx, name));
            assert!(!widening_signature_v1(tcx, other), "accepted local helper: {name}");
            assert!(!authenticate_reviewed_safe_core_u32_widening_v1(tcx, other));
            results.refusals += 1;
        }

        let bb = BasicBlock::from_u32(0);
        let scope = SourceScope::from_u32(0);
        for count in [0, 2] {
            reject_mutation(tcx, instance, body, "argument count", |body| body.arg_count = count);
            results.shapes += 1;
        }
        reject_mutation(tcx, instance, body, "missing local", |body| { body.local_decls.pop(); });
        reject_mutation(tcx, instance, body, "extra local", |body| {
            let extra = body.local_decls[Local::from_u32(1)].clone();
            body.local_decls.push(extra);
        });
        reject_mutation(tcx, instance, body, "missing block", |body| { body.basic_blocks.as_mut().pop(); });
        reject_mutation(tcx, instance, body, "extra block", |body| {
            let extra = body.basic_blocks[bb].clone();
            body.basic_blocks.as_mut().push(extra);
        });
        reject_mutation(tcx, instance, body, "missing scope", |body| { body.source_scopes.pop(); });
        reject_mutation(tcx, instance, body, "extra scope", |body| {
            let extra = body.source_scopes[scope].clone();
            body.source_scopes.push(extra);
        });
        results.shapes += 6;
        for (index, types) in [
            (0, [tcx.types.u32, tcx.types.u128, tcx.types.i64, tcx.types.unit]),
            (1, [tcx.types.u64, tcx.types.u128, tcx.types.i32, tcx.types.unit]),
        ] {
            for ty in types {
                reject_mutation(tcx, instance, body, "local type", |body| {
                    body.local_decls[Local::from_u32(index)].ty = ty;
                });
                results.locals += 1;
            }
            reject_mutation(tcx, instance, body, "local scope", |body| {
                body.local_decls[Local::from_u32(index)].source_info.scope = SourceScope::from_u32(1);
            });
            results.locals += 1;
        }
        reject_mutation(tcx, instance, body, "scope parent", |body| {
            body.source_scopes[scope].parent_scope = Some(scope);
        });
        reject_mutation(tcx, instance, body, "inline origin", |body| {
            body.source_scopes[scope].inlined = Some((instance, DUMMY_SP));
        });
        reject_mutation(tcx, instance, body, "inline parent", |body| {
            body.source_scopes[scope].inlined_parent_scope = Some(scope);
        });
        let local_instance = Instance::mono(tcx, local_function(tcx, "local_cast"));
        let local_data = tcx.instance_mir(local_instance.def).source_scopes[scope].local_data.clone();
        assert!(matches!(&local_data, ClearCrossCrate::Set(_)));
        reject_mutation(tcx, instance, body, "non-clear scope data", |body| {
            body.source_scopes[scope].local_data = local_data;
        });
        reject_mutation(tcx, instance, body, "statement scope", |body| {
            body.basic_blocks.as_mut()[bb].statements[0].source_info.scope = SourceScope::from_u32(1);
        });
        reject_mutation(tcx, instance, body, "return scope", |body| {
            body.basic_blocks.as_mut()[bb].terminator_mut().source_info.scope = SourceScope::from_u32(1);
        });
        results.scopes = 6;

        reject_mutation(tcx, instance, body, "cleanup", |body| {
            body.basic_blocks.as_mut()[bb].is_cleanup = true;
        });
        reject_mutation(tcx, instance, body, "missing terminator", |body| {
            body.basic_blocks.as_mut()[bb].terminator = None;
        });
        reject_mutation(tcx, instance, body, "missing statement", |body| {
            body.basic_blocks.as_mut()[bb].statements.clear();
        });
        for kind in [
            StatementKind::Nop,
            StatementKind::StorageLive(Local::from_u32(1)),
            StatementKind::StorageDead(Local::from_u32(1)),
            StatementKind::Intrinsic(Box::new(NonDivergingIntrinsic::Assume(
                Operand::Copy(Place::from(Local::from_u32(1))),
            ))),
        ] {
            reject_mutation(tcx, instance, body, "replacement operation", |body| {
                body.basic_blocks.as_mut()[bb].statements[0].kind = kind;
            });
            results.operations += 1;
        }
        for extra_nop in [false, true] {
            reject_mutation(tcx, instance, body, "extra operation", |body| {
                let mut extra = body.basic_blocks[bb].statements[0].clone();
                if extra_nop { extra.kind = StatementKind::Nop; }
                body.basic_blocks.as_mut()[bb].statements.push(extra);
            });
            results.operations += 1;
        }
        reject_mutation(tcx, instance, body, "actual call", |body| {
            body.basic_blocks.as_mut()[bb].terminator = Some(actual_call.clone());
        });
        results.operations += 4;

        for kind in [CastKind::IntToFloat, CastKind::FloatToInt, CastKind::Transmute] {
            reject_mutation(tcx, instance, body, "cast kind", |body| {
                let StatementKind::Assign(assignment) =
                    &mut body.basic_blocks.as_mut()[bb].statements[0].kind
                else {
                    unreachable!()
                };
                let Rvalue::Cast(current, _, _) = &mut assignment.1 else {
                    unreachable!()
                };
                *current = kind;
            });
            results.casts += 1;
        }
        for ty in [tcx.types.u32, tcx.types.i64, tcx.types.u128] {
            reject_mutation(tcx, instance, body, "cast result type", |body| {
                let StatementKind::Assign(assignment) =
                    &mut body.basic_blocks.as_mut()[bb].statements[0].kind
                else {
                    unreachable!()
                };
                let Rvalue::Cast(_, _, output) = &mut assignment.1 else {
                    unreachable!()
                };
                *output = ty;
            });
            results.casts += 1;
        }
        let TerminatorKind::Call { func, .. } = &actual_call.kind else {
            unreachable!()
        };
        for operand in [
            Operand::Move(Place::from(Local::from_u32(1))),
            Operand::Copy(Place::from(Local::from_u32(0))),
            func.clone(),
        ] {
            reject_mutation(tcx, instance, body, "cast operand", |body| {
                let StatementKind::Assign(assignment) =
                    &mut body.basic_blocks.as_mut()[bb].statements[0].kind
                else {
                    unreachable!()
                };
                let Rvalue::Cast(_, input, _) = &mut assignment.1 else {
                    unreachable!()
                };
                *input = operand;
            });
            results.casts += 1;
        }
        reject_mutation(tcx, instance, body, "missing cast", |body| {
            let StatementKind::Assign(assignment) =
                &mut body.basic_blocks.as_mut()[bb].statements[0].kind
            else {
                unreachable!()
            };
            assignment.1 = Rvalue::Use(Operand::Copy(Place::from(Local::from_u32(1))));
        });
        results.casts += 1;

        for axis in 0..3 {
            reject_mutation(tcx, instance, body, "non-plain input/output place", |body| {
                let StatementKind::Assign(assignment) =
                    &mut body.basic_blocks.as_mut()[bb].statements[0].kind
                else {
                    unreachable!()
                };
                match axis {
                    0 => assignment.0 = Place::from(Local::from_u32(1)),
                    1 => assignment.0 = assignment.0.project_deeper(&[ProjectionElem::Deref], tcx),
                    2 => {
                        let Rvalue::Cast(_, Operand::Copy(input), _) = &mut assignment.1 else {
                            unreachable!()
                        };
                        *input = input.project_deeper(&[ProjectionElem::Deref], tcx);
                    }
                    _ => unreachable!(),
                }
            });
            results.places_and_returns += 1;
        }
        for kind in [TerminatorKind::Unreachable, TerminatorKind::Goto { target: bb }] {
            reject_mutation(tcx, instance, body, "non-return terminator", |body| {
                body.basic_blocks.as_mut()[bb].terminator_mut().kind = kind;
            });
            results.places_and_returns += 1;
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
name = "fe2o3_core_u32_widening_fixture"
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
        .expect("build genuine AMD target core for widening callback");
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
            "fe2o3-core-u32-widening-v1-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create core widening fixture directory");
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
            "rustc".to_owned(), "--crate-name".to_owned(), "fe2o3_core_u32_widening_fixture".to_owned(),
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
        callbacks.results.expect("core widening callback")
    }).map_err(|payload| {
        if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else {
            "core widening fixture initialization panicked with a non-string payload".to_owned()
        }
    }));
    result
        .as_ref()
        .unwrap_or_else(|message| panic!("cached core widening fixture failure: {message}"))
        .clone()
}

#[test]
fn u32_widening_real_core_identity_and_signature_refusals() {
    let results = compiler_results();
    assert_eq!((results.genuine, results.refusals), (1, 8));
}

#[test]
fn u32_widening_real_body_rejects_shape_and_local_mutations() {
    let results = compiler_results();
    assert_eq!((results.shapes, results.locals), (8, 10));
}

#[test]
fn u32_widening_real_body_rejects_source_scope_mutations() {
    assert_eq!(compiler_results().scopes, 6);
}

#[test]
fn u32_widening_real_body_rejects_extra_operations_and_cleanup() {
    assert_eq!(compiler_results().operations, 10);
}

#[test]
fn u32_widening_real_body_rejects_cast_kind_type_and_operand_mutations() {
    assert_eq!(compiler_results().casts, 10);
}

#[test]
fn u32_widening_real_body_rejects_places_and_terminators() {
    assert_eq!(compiler_results().places_and_returns, 5);
}
