use super::*;
use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
};
use rustc_abi::VariantIdx;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::DefId;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Local, ProjectionElem, Terminator};
use rustc_span::DUMMY_SP;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

// Historical Option name is shared immutable fixture context, not artifact authority.
const FIXTURE_METADATA: &str = "fe2o3_core_attention_option_fixture_v1";

const FIXTURE: &str = r#"
#![no_std]
use fe2o3_device::KernelError;
pub fn identity(value: KernelError) -> KernelError { <KernelError as core::convert::From<KernelError>>::from(value) }
pub fn identity_u32(value: u32) -> u32 { <u32 as core::convert::From<u32>>::from(value) }
pub fn identity_usize(value: usize) -> usize { <usize as core::convert::From<usize>>::from(value) }
pub mod fake { pub enum KernelError { Invalid } }
pub fn fake_provider(value: fake::KernelError) -> fake::KernelError {
    <fake::KernelError as core::convert::From<fake::KernelError>>::from(value)
}
pub fn widen(value: u32) -> u64 { <u64 as core::convert::From<u32>>::from(value) }
mod lookalike {
    use fe2o3_device::KernelError;
    pub trait From<T> { fn from(value: T) -> Self; }
    impl From<KernelError> for KernelError { fn from(value: KernelError) -> Self { value } }
}
pub fn user_from(value: KernelError) -> KernelError { <KernelError as lookalike::From<KernelError>>::from(value) }
pub fn local_identity(value: KernelError) -> KernelError { value }
pub unsafe fn unsafe_identity(value: KernelError) -> KernelError { value }
pub extern "C" fn foreign_identity(value: KernelError) -> KernelError { value }
"#;

#[derive(Clone, Default)]
struct Results { positives: usize, refusals: usize, mutations: [usize; 5] }

#[derive(Default)]
struct FixtureCallbacks { results: Option<Results> }

fn local_function(tcx: TyCtxt<'_>, name: &str) -> DefId {
    tcx.iter_local_def_id().find(|id| tcx.def_kind(id.to_def_id()) == DefKind::Fn
        && tcx.item_name(id.to_def_id()).as_str() == name).expect("fixture function").to_def_id()
}

fn fixture_call<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> (Instance<'tcx>, Terminator<'tcx>) {
    let caller = Instance::mono(tcx, local_function(tcx, name));
    tcx.instance_mir(caller.def).basic_blocks.iter().find_map(|block| {
        let term = block.terminator();
        let TerminatorKind::Call { func: Operand::Constant(function), .. } = &term.kind else { return None; };
        let TyKind::FnDef(def_id, args) = *function.const_.ty().kind() else { return None; };
        let args = caller.try_instantiate_mir_and_normalize_erasing_regions(
            tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(args),
        ).ok()?;
        let resolved = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def_id, args).ok()??;
        (tcx.item_name(resolved.def_id()).as_str() == "from").then(|| (resolved, term.clone()))
    }).expect("real resolved From call")
}

fn reject<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>,
    count: &mut usize, label: &str, mutate: impl FnOnce(&mut Body<'tcx>)) {
    let mut changed = body.clone();
    mutate(&mut changed);
    assert!(!identity_body_v1(tcx, instance, &changed), "accepted identity MIR mutation: {label}");
    *count += 1;
}

impl Callbacks for FixtureCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(tcx.sess.opts.cg.metadata.as_slice(), &[FIXTURE_METADATA.to_owned()]);
        let observation = derive_cargo_metadata_build_observation_v2(&tcx.sess.opts.cg.metadata);
        assert_eq!(std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)
            .expect("identity fixture requires shared immutable metadata observation"), observation.to_hex());
        let mut result = Results::default();
        let (instance, actual_call) = fixture_call(tcx, "identity");
        let body = tcx.instance_mir(instance.def);
        assert!(identity_signature_v1(tcx, instance).is_some(), "real identity signature: {instance:?}");
        assert!(identity_body_v1(tcx, instance, body), "real identity body: {instance:?}; args={} locals={} blocks={} scopes={}",
            body.arg_count, body.local_decls.len(), body.basic_blocks.len(), body.source_scopes.len());
        assert!(authenticate_reviewed_safe_core_kernel_error_identity_v1(tcx, instance));
        result.positives += 1;
        for name in ["identity_u32", "identity_usize", "fake_provider", "widen", "user_from"] {
            let (other, _) = fixture_call(tcx, name);
            assert!(identity_signature_v1(tcx, other).is_none(), "accepted signature: {name}");
            assert!(!authenticate_reviewed_safe_core_kernel_error_identity_v1(tcx, other));
            result.refusals += 1;
        }
        for name in ["local_identity", "unsafe_identity", "foreign_identity"] {
            let other = Instance::mono(tcx, local_function(tcx, name));
            assert!(identity_signature_v1(tcx, other).is_none(), "accepted local helper: {name}");
            assert!(!authenticate_reviewed_safe_core_kernel_error_identity_v1(tcx, other));
            result.refusals += 1;
        }
        let bb = BasicBlock::from_u32(0);
        let scope = SourceScope::from_u32(0);
        for args in [0, 2] {
            reject(tcx, instance, body, &mut result.mutations[0], "argument count", |b| b.arg_count = args);
        }
        reject(tcx, instance, body, &mut result.mutations[0], "missing local", |b| { b.local_decls.pop(); });
        reject(tcx, instance, body, &mut result.mutations[0], "extra local", |b| { b.local_decls.push(b.local_decls[Local::from_u32(1)].clone()); });
        reject(tcx, instance, body, &mut result.mutations[0], "missing block", |b| { b.basic_blocks.as_mut().pop(); });
        reject(tcx, instance, body, &mut result.mutations[0], "extra block", |b| { let extra = b.basic_blocks[bb].clone(); b.basic_blocks.as_mut().push(extra); });
        reject(tcx, instance, body, &mut result.mutations[0], "missing scope", |b| { b.source_scopes.pop(); });
        reject(tcx, instance, body, &mut result.mutations[0], "extra scope", |b| { b.source_scopes.push(b.source_scopes[scope].clone()); });
        for (index, _) in body.local_decls.iter_enumerated() {
            for ty in [tcx.types.u32, tcx.types.usize, tcx.types.bool, tcx.types.unit] {
                reject(tcx, instance, body, &mut result.mutations[1], "local type", |b| b.local_decls[index].ty = ty);
            }
            reject(tcx, instance, body, &mut result.mutations[1], "local scope", |b| b.local_decls[index].source_info.scope = SourceScope::from_u32(1));
        }
        let local_data = tcx.instance_mir(Instance::mono(tcx, local_function(tcx, "local_identity")).def)
            .source_scopes[scope].local_data.clone();
        assert!(matches!(&local_data, ClearCrossCrate::Set(_)));
        reject(tcx, instance, body, &mut result.mutations[2], "scope parent", |b| b.source_scopes[scope].parent_scope = Some(scope));
        reject(tcx, instance, body, &mut result.mutations[2], "inline origin", |b| b.source_scopes[scope].inlined = Some((instance, DUMMY_SP)));
        reject(tcx, instance, body, &mut result.mutations[2], "inline parent", |b| b.source_scopes[scope].inlined_parent_scope = Some(scope));
        reject(tcx, instance, body, &mut result.mutations[2], "scope local data", |b| b.source_scopes[scope].local_data = local_data);
        reject(tcx, instance, body, &mut result.mutations[2], "statement scope", |b| b.basic_blocks.as_mut()[bb].statements[0].source_info.scope = SourceScope::from_u32(1));
        reject(tcx, instance, body, &mut result.mutations[2], "terminator scope", |b| b.basic_blocks.as_mut()[bb].terminator_mut().source_info.scope = SourceScope::from_u32(1));
        reject(tcx, instance, body, &mut result.mutations[3], "replacement operation", |b| b.basic_blocks.as_mut()[bb].statements[0].kind = StatementKind::Nop);
        reject(tcx, instance, body, &mut result.mutations[3], "extra operation", |b| {
            let mut extra = b.basic_blocks[bb].statements[0].clone(); extra.kind = StatementKind::Nop;
            b.basic_blocks.as_mut()[bb].statements.push(extra);
        });
        reject(tcx, instance, body, &mut result.mutations[3], "missing operation", |b| b.basic_blocks.as_mut()[bb].statements.clear());
        reject(tcx, instance, body, &mut result.mutations[3], "cleanup", |b| b.basic_blocks.as_mut()[bb].is_cleanup = true);
        reject(tcx, instance, body, &mut result.mutations[3], "missing terminator", |b| b.basic_blocks.as_mut()[bb].terminator = None);
        reject(tcx, instance, body, &mut result.mutations[4], "output local", |b| {
            let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[bb].statements[0].kind else { unreachable!() };
            a.0 = Place::from(Local::from_u32(1));
        });
        reject(tcx, instance, body, &mut result.mutations[4], "output projection", |b| {
            let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[bb].statements[0].kind else { unreachable!() };
            a.0 = a.0.project_deeper(&[ProjectionElem::Downcast(None, VariantIdx::from_u32(0))], tcx);
        });
        reject(tcx, instance, body, &mut result.mutations[4], "copy not move", |b| {
            let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[bb].statements[0].kind else { unreachable!() };
            a.1 = Rvalue::Use(Operand::Copy(Place::from(Local::from_u32(1))));
        });
        reject(tcx, instance, body, &mut result.mutations[4], "input local", |b| {
            let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[bb].statements[0].kind else { unreachable!() };
            a.1 = Rvalue::Use(Operand::Move(Place::from(Local::from_u32(0))));
        });
        reject(tcx, instance, body, &mut result.mutations[4], "input projection", |b| {
            let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[bb].statements[0].kind else { unreachable!() };
            let place = Place::from(Local::from_u32(1))
                .project_deeper(&[ProjectionElem::Downcast(None, VariantIdx::from_u32(0))], tcx);
            a.1 = Rvalue::Use(Operand::Move(place));
        });
        let TerminatorKind::Call { func, .. } = &actual_call.kind else { unreachable!() };
        assert!(matches!(func, Operand::Constant(_)));
        reject(tcx, instance, body, &mut result.mutations[4], "constant not move", |b| {
            let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[bb].statements[0].kind else { unreachable!() };
            a.1 = Rvalue::Use(func.clone());
        });
        reject(tcx, instance, body, &mut result.mutations[4], "unreachable not return", |b| b.basic_blocks.as_mut()[bb].terminator_mut().kind = TerminatorKind::Unreachable);
        reject(tcx, instance, body, &mut result.mutations[4], "edge not return", |b| b.basic_blocks.as_mut()[bb].terminator_mut().kind = TerminatorKind::Goto { target: bb });
        reject(tcx, instance, body, &mut result.mutations[4], "call not return", |b| b.basic_blocks.as_mut()[bb].terminator = Some(actual_call));
        self.results = Some(result);
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
) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
    let manifest = fixture.0.join("Cargo.toml");
    let provider = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fe2o3-device")
        .canonicalize().expect("current source-pinned provider");
    let header = r#"[package]
name = "fe2o3_core_kernel_error_identity_fixture"
version = "0.0.0"
edition = "2024"
publish = false
[workspace]
[lib]
path = "fixture.rs"
"#;
    fs::write(&manifest, format!("{header}\n[dependencies]\nfe2o3-device = {{ path = {} }}\n",
        serde_json::to_string(provider.to_str().expect("provider path UTF-8")).expect("TOML string path")))
        .expect("write private target-core fixture manifest");
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
        .expect("build genuine AMD target core for KernelError identity callback");
    assert!(output.stdout.len() <= 4 << 20 && output.stderr.len() <= 4 << 20);
    assert!(output.status.success(), "target-core build failed: {}", String::from_utf8_lossy(&output.stderr));
    let mut expected_sources = vec![
        "lib/rustlib/src/rust/library/core/src/lib.rs",
        "lib/rustlib/src/rust/library/compiler-builtins/compiler-builtins/src/lib.rs",
    ].into_iter().map(|relative| sysroot.join(relative).canonicalize().expect("pinned rust-src library source"))
        .collect::<Vec<_>>();
    expected_sources.push(provider.join("src/lib.rs").canonicalize().expect("provider source"));
    let dependencies = target.join("amdgcn-amd-amdhsa/release/deps")
        .canonicalize().expect("private target-core dependency directory");
    let host_dependencies = target.join("release/deps")
        .canonicalize().expect("private provider proc-macro dependency directory");
    let mut selected: [Option<PathBuf>; 3] = [None, None, None];
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
            Some("fe2o3_device") => 2,
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
     selected[1].take().expect("Cargo selected actual target compiler_builtins"),
     selected[2].take().expect("Cargo selected actual target provider"), dependencies, host_dependencies)
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
            "fe2o3-core-kernel-error-identity-v1-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create core KernelError identity fixture directory");
        let fixture = FixtureDirectory(root);
        let source = fixture.0.join("fixture.rs");
        fs::write(&source, FIXTURE).expect("write compiler fixture");
        let mut command = Command::new("rustc");
        command.args(["--print", "sysroot"]);
        let sysroot = crate::process_execution::capture_output(&mut command).expect("query rustc sysroot");
        assert!(sysroot.status.success());
        let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 sysroot").trim().to_owned();
        let (core, builtins, provider, dependencies, host_dependencies) = target_core_metadata(&fixture, std::path::Path::new(&sysroot));
        let args = vec![
            "rustc".to_owned(), "--crate-name".to_owned(), "fe2o3_core_kernel_error_identity_fixture".to_owned(),
            format!("-Cmetadata={FIXTURE_METADATA}"),
            "--crate-type=lib".to_owned(), "--edition=2024".to_owned(), "--emit=metadata".to_owned(),
            "-Zmir-opt-level=0".to_owned(), "-Zinline-mir=no".to_owned(), "-Cpanic=abort".to_owned(),
            "--target=amdgcn-amd-amdhsa".to_owned(), "-Ctarget-cpu=gfx942".to_owned(),
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".to_owned(),
            "--extern".to_owned(), format!("core={}", core.display()),
            "--extern".to_owned(), format!("compiler_builtins={}", builtins.display()),
            "--extern".to_owned(), format!("fe2o3_device={}", provider.display()),
            "-L".to_owned(), format!("dependency={}", dependencies.display()),
            "-L".to_owned(), format!("dependency={}", host_dependencies.display()),
            "--sysroot".to_owned(), sysroot,
            "-o".to_owned(), fixture.0.join("fixture.rmeta").display().to_string(),
            source.display().to_string(),
        ];
        let mut callbacks = FixtureCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        callbacks.results.expect("core KernelError identity callback")
    }).map_err(|payload| {
        if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else {
            "core KernelError identity fixture initialization panicked with a non-string payload".to_owned()
        }
    }));
    result
        .as_ref()
        .unwrap_or_else(|message| panic!("cached core KernelError identity fixture failure: {message}"))
        .clone()
}

#[test]
fn kernel_error_identity_real_core_provider_identity_and_signature_refusals() {
    let result = compiler_results();
    assert_eq!(result.positives, 1);
    assert_eq!(result.refusals, 8);
}

#[test]
fn kernel_error_identity_real_body_rejects_shape_mutations() { assert_eq!(compiler_results().mutations[0], 8); }

#[test]
fn kernel_error_identity_real_body_rejects_local_type_and_scope_mutations() { assert_eq!(compiler_results().mutations[1], 10); }

#[test]
fn kernel_error_identity_real_body_rejects_source_scope_mutations() { assert_eq!(compiler_results().mutations[2], 6); }

#[test]
fn kernel_error_identity_real_body_rejects_extra_operations_and_cleanup() { assert_eq!(compiler_results().mutations[3], 5); }

#[test]
fn kernel_error_identity_real_body_rejects_operands_places_and_returns() { assert_eq!(compiler_results().mutations[4], 9); }
