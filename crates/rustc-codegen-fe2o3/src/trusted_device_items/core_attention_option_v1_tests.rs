use super::*;
use rustc_abi::{FieldIdx, VariantIdx};
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Local, SwitchTargets, Terminator};
use rustc_middle::ty::UserTypeAnnotationIndex;
use rustc_span::DUMMY_SP;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const FIXTURE: &str = r#"
#![no_std]
use fe2o3_device::{DisjointTile2D, Index1D, KernelError};
pub fn ok_usize(v: Option<usize>, e: KernelError) -> Result<usize, KernelError> { v.ok_or(e) }
pub fn ok_u32(v: Option<u32>, e: KernelError) -> Result<u32, KernelError> { v.ok_or(e) }
pub fn ok_tile(v: Option<DisjointTile2D<Index1D,64,16,16,4>>, e: KernelError)
    -> Result<DisjointTile2D<Index1D,64,16,16,4>, KernelError> { v.ok_or(e) }
pub fn then_add(v: Option<usize>, n: u32) -> Option<usize> { v.and_then(|x| x.checked_add(n as usize)) }
pub fn then_mul(v: Option<usize>, n: u32) -> Option<usize> { v.and_then(|x| x.checked_mul(n as usize)) }
pub fn wrong_payload(v: Option<u64>, e: KernelError) -> Result<u64, KernelError> { v.ok_or(e) }
pub fn wrong_error(v: Option<usize>, e: u32) -> Result<usize, u32> { v.ok_or(e) }
pub struct Error;
impl Drop for Error { fn drop(&mut self) {} }
pub fn dropping_error(v: Option<usize>, e: Error) -> Result<usize, Error> { v.ok_or(e) }
pub fn wrong_tile(v: Option<DisjointTile2D<Index1D,32,16,16,4>>, e: KernelError)
    -> Result<DisjointTile2D<Index1D,32,16,16,4>, KernelError> { v.ok_or(e) }
pub fn then_u32(v: Option<u32>, n: u32) -> Option<u32> { v.and_then(|x| x.checked_add(n)) }
pub fn then_result_u32(v: Option<usize>, n: u32) -> Option<u32> { v.and_then(|x| Some((x as u32).wrapping_add(n))) }
pub fn then_owned(v: Option<usize>, n: u32) -> Option<usize> { v.and_then(move |x| x.checked_add(n as usize)) }
pub fn then_mut(v: Option<usize>, mut n: u32) -> Option<usize> { v.and_then(|x| { n += 1; x.checked_add(n as usize) }) }
pub fn then_no_capture(v: Option<usize>) -> Option<usize> { v.and_then(|x| Some(x)) }
pub fn function_callback(x: usize) -> Option<usize> { Some(x) }
pub fn then_function(v: Option<usize>) -> Option<usize> { v.and_then(function_callback) }
pub fn unsafe_body_callback(x: usize) -> Option<usize> { Some(unsafe { core::ptr::read_volatile(&x) }) }
pub fn then_unsafe_function(v: Option<usize>) -> Option<usize> { v.and_then(unsafe_body_callback) }
pub unsafe fn unsafe_local(v: Option<usize>) -> Option<usize> { v }
pub extern "C" fn foreign_local(v: usize) -> usize { v }
pub fn local(v: usize) -> usize { v }
pub struct Lookalike;
impl Lookalike { pub fn ok_or(self, e: KernelError) -> Result<usize, KernelError> { Err(e) } }
pub fn fake_option(e: KernelError) -> Result<usize, KernelError> { Lookalike.ok_or(e) }
"#;

#[derive(Clone, Default)]
struct Results { positives: usize, refusals: usize, mutations: [usize; 8] }

#[derive(Default)]
struct FixtureCallbacks { results: Option<Results> }

fn local_function(tcx: TyCtxt<'_>, name: &str) -> DefId {
    tcx.iter_local_def_id().find(|id| tcx.def_kind(id.to_def_id()) == DefKind::Fn
        && tcx.item_name(id.to_def_id()).as_str() == name).expect("fixture function").to_def_id()
}

fn fixture_call<'tcx>(tcx: TyCtxt<'tcx>, name: &str, method: &str) -> (Instance<'tcx>, Terminator<'tcx>) {
    let caller = Instance::mono(tcx, local_function(tcx, name));
    tcx.instance_mir(caller.def).basic_blocks.iter().find_map(|block| {
        let term = block.terminator();
        let TerminatorKind::Call { func: Operand::Constant(function), .. } = &term.kind else { return None; };
        let TyKind::FnDef(def_id, args) = *function.const_.ty().kind() else { return None; };
        let args = caller.try_instantiate_mir_and_normalize_erasing_regions(
            tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(args),
        ).ok()?;
        let resolved = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def_id, args).ok()??;
        (tcx.item_name(resolved.def_id()).as_str() == method).then(|| (resolved, term.clone()))
    }).expect("real resolved Option call")
}

fn reject<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>,
    count: &mut usize, label: &str, mutate: impl FnOnce(&mut Body<'tcx>)) {
    let mut changed = body.clone();
    mutate(&mut changed);
    assert!(!body_matches(tcx, instance, &changed), "accepted Option MIR mutation: {label}");
    *count += 1;
}

impl Callbacks for FixtureCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let mut result = Results::default();
        for (name, method) in [("ok_usize", "ok_or"), ("ok_u32", "ok_or"), ("ok_tile", "ok_or"),
            ("then_add", "and_then"), ("then_mul", "and_then")] {
            let (instance, _) = fixture_call(tcx, name, method);
            assert!(signature(tcx, instance).is_some(), "real signature: {instance:?}");
            assert!(body_matches(tcx, instance, tcx.instance_mir(instance.def)), "real body: {instance:?}");
            assert!(authenticate_reviewed_safe_core_attention_option_v1(tcx, instance), "real family: {name}");
            result.positives += 1;
        }
        for (name, method) in [("wrong_payload", "ok_or"), ("wrong_error", "ok_or"),
            ("dropping_error", "ok_or"), ("wrong_tile", "ok_or"), ("fake_option", "ok_or"),
            ("then_u32", "and_then"), ("then_result_u32", "and_then"), ("then_owned", "and_then"),
            ("then_mut", "and_then"), ("then_no_capture", "and_then"), ("then_function", "and_then"),
            ("then_unsafe_function", "and_then")] {
            let (instance, _) = fixture_call(tcx, name, method);
            assert!(!authenticate_reviewed_safe_core_attention_option_v1(tcx, instance), "accepted family: {name}");
            result.refusals += 1;
        }
        for name in ["unsafe_local", "foreign_local", "local"] {
            assert!(!authenticate_reviewed_safe_core_attention_option_v1(tcx, Instance::mono(tcx, local_function(tcx, name))));
            result.refusals += 1;
        }
        let scope = SourceScope::from_u32(0);
        let local_data = tcx.instance_mir(Instance::mono(tcx, local_function(tcx, "local")).def)
            .source_scopes[scope].local_data.clone();
        assert!(matches!(&local_data, ClearCrossCrate::Set(_)));
        for (name, method) in [("ok_usize", "ok_or"), ("then_add", "and_then")] {
            let (instance, actual_call) = fixture_call(tcx, name, method);
            let body = tcx.instance_mir(instance.def);
            let bb = BasicBlock::from_u32(0);
            for args in [0, 1, 3] {
                reject(tcx, instance, body, &mut result.mutations[0], "argument count", |b| b.arg_count = args);
            }
            reject(tcx, instance, body, &mut result.mutations[0], "missing local", |b| { b.local_decls.pop(); });
            reject(tcx, instance, body, &mut result.mutations[0], "extra local", |b| { b.local_decls.push(b.local_decls[Local::from_u32(1)].clone()); });
            reject(tcx, instance, body, &mut result.mutations[0], "missing block", |b| { b.basic_blocks.as_mut().pop(); });
            reject(tcx, instance, body, &mut result.mutations[0], "extra block", |b| { let extra = b.basic_blocks[bb].clone(); b.basic_blocks.as_mut().push(extra); });
            reject(tcx, instance, body, &mut result.mutations[0], "missing scope", |b| { b.source_scopes.pop(); });
            reject(tcx, instance, body, &mut result.mutations[0], "extra scope", |b| { b.source_scopes.push(b.source_scopes[scope].clone()); });
            for (index, _) in body.local_decls.iter_enumerated() {
                reject(tcx, instance, body, &mut result.mutations[1], "local type", |b| b.local_decls[index].ty = tcx.types.u64);
                reject(tcx, instance, body, &mut result.mutations[1], "local scope", |b| b.local_decls[index].source_info.scope = SourceScope::from_u32(1));
            }
            for (index, _) in body.source_scopes.iter_enumerated() {
                reject(tcx, instance, body, &mut result.mutations[2], "scope parent", |b| b.source_scopes[index].parent_scope = Some(index));
                reject(tcx, instance, body, &mut result.mutations[2], "inline origin", |b| b.source_scopes[index].inlined = Some((instance, DUMMY_SP)));
                reject(tcx, instance, body, &mut result.mutations[2], "inline parent", |b| b.source_scopes[index].inlined_parent_scope = Some(scope));
                reject(tcx, instance, body, &mut result.mutations[2], "scope local data", |b| b.source_scopes[index].local_data = local_data.clone());
            }
            for (index, block) in body.basic_blocks.iter_enumerated() {
                reject(tcx, instance, body, &mut result.mutations[2], "terminator scope", |b| b.basic_blocks.as_mut()[index].terminator_mut().source_info.scope = SourceScope::from_u32(2));
                reject(tcx, instance, body, &mut result.mutations[3], "cleanup", |b| b.basic_blocks.as_mut()[index].is_cleanup = true);
                reject(tcx, instance, body, &mut result.mutations[3], "missing terminator", |b| b.basic_blocks.as_mut()[index].terminator = None);
                reject(tcx, instance, body, &mut result.mutations[3], "extra operation", |b| {
                    let mut extra = b.basic_blocks[bb].statements[0].clone(); extra.kind = StatementKind::Nop;
                    b.basic_blocks.as_mut()[index].statements.push(extra);
                });
                for statement in 0..block.statements.len() {
                    reject(tcx, instance, body, &mut result.mutations[2], "statement scope", |b| b.basic_blocks.as_mut()[index].statements[statement].source_info.scope = SourceScope::from_u32(2));
                    reject(tcx, instance, body, &mut result.mutations[3], "replacement operation", |b| b.basic_blocks.as_mut()[index].statements[statement].kind = StatementKind::Nop);
                }
            }
            reject(tcx, instance, body, &mut result.mutations[4], "switch operand", |b| {
                let TerminatorKind::SwitchInt { discr, .. } = &mut b.basic_blocks.as_mut()[bb].terminator_mut().kind else { unreachable!() };
                *discr = Operand::Copy(Place::from(Local::from_u32(3)));
            });
            for (pairs, otherwise) in [([(0, 3), (1, 2)], 1), ([(0, 2), (1, 3)], 2)] {
                reject(tcx, instance, body, &mut result.mutations[4], "switch targets", |b| {
                    let TerminatorKind::SwitchInt { targets, .. } = &mut b.basic_blocks.as_mut()[bb].terminator_mut().kind else { unreachable!() };
                    *targets = SwitchTargets::new(pairs.into_iter().map(|(value, target)| (value, BasicBlock::from_u32(target))), BasicBlock::from_u32(otherwise));
                });
            }
            reject(tcx, instance, body, &mut result.mutations[4], "unreachable", |b| b.basic_blocks.as_mut()[BasicBlock::from_u32(1)].terminator_mut().kind = TerminatorKind::Return);
            let last = BasicBlock::from_usize(body.basic_blocks.len() - 1);
            reject(tcx, instance, body, &mut result.mutations[4], "return", |b| b.basic_blocks.as_mut()[last].terminator_mut().kind = TerminatorKind::Unreachable);
            for copied in [true, false] {
                reject(tcx, instance, body, &mut result.mutations[5], "Some projected payload", |b| {
                    let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[BasicBlock::from_u32(3)].statements[0].kind else { unreachable!() };
                    a.1 = Rvalue::Use(if copied { Operand::Copy(Place::from(Local::from_u32(1))) } else { Operand::Move(Place::from(Local::from_u32(2))) });
                });
            }
            for variant in [true, false] {
                reject(tcx, instance, body, &mut result.mutations[5], "Some variant/type", |b| {
                    let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[BasicBlock::from_u32(3)].statements[0].kind else { unreachable!() };
                    let Rvalue::Use(Operand::Move(place)) = &mut a.1 else { unreachable!() };
                    let mut projection = place.projection.to_vec();
                    if variant { projection[0] = ProjectionElem::Downcast(None, VariantIdx::from_u32(0)); }
                    else { projection[1] = ProjectionElem::Field(FieldIdx::from_u32(0), tcx.types.bool); }
                    *place = Place::from(place.local).project_deeper(&projection, tcx);
                });
            }
            let aggregates: &[(u32, usize)] = if method == "ok_or" { &[(2, 2), (3, 1)] } else { &[(2, 0)] };
            for &(block, statement) in aggregates {
                let axes: &[u32] = if method == "ok_or" { &[0, 1, 2, 3, 4, 5, 6] } else { &[0, 3, 4, 5, 6] };
                for &axis in axes {
                    reject(tcx, instance, body, &mut result.mutations[5], "enum aggregate", |b| {
                        let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[BasicBlock::from_u32(block)].statements[statement].kind else { unreachable!() };
                        if axis == 0 { a.0 = Place::from(Local::from_u32(1)); return; }
                        let Rvalue::Aggregate(kind, operands) = &mut a.1 else { unreachable!() };
                        if axis == 1 {
                            operands[FieldIdx::from_u32(0)] = match &operands[FieldIdx::from_u32(0)] {
                                Operand::Copy(place) => Operand::Move(*place),
                                Operand::Move(place) => Operand::Copy(*place), _ => unreachable!(),
                            }; return;
                        }
                        if axis == 2 {
                            match &mut operands[FieldIdx::from_u32(0)] {
                                Operand::Copy(place) | Operand::Move(place) => place.local = Local::from_u32(1),
                                _ => unreachable!(),
                            } return;
                        }
                        let AggregateKind::Adt(_, variant, args, annotation, union) = &mut **kind else { unreachable!() };
                        match axis {
                            3 => *variant = VariantIdx::from_u32(1 - variant.as_u32()),
                            4 => *args = tcx.mk_args(&[]),
                            5 => *annotation = Some(UserTypeAnnotationIndex::from_u32(0)),
                            _ => *union = Some(FieldIdx::from_u32(0)),
                        }
                    });
                }
            }
            if method == "and_then" {
                for axis in 0..4 {
                    reject(tcx, instance, body, &mut result.mutations[5], "callback tuple", |b| {
                        let StatementKind::Assign(a) = &mut b.basic_blocks.as_mut()[BasicBlock::from_u32(3)].statements[4].kind else { unreachable!() };
                        if axis == 0 { a.0 = Place::from(Local::from_u32(5)); return; }
                        let Rvalue::Aggregate(_, values) = &mut a.1 else { unreachable!() };
                        match axis {
                            1 => values[FieldIdx::from_u32(0)] = Operand::Move(Place::from(Local::from_u32(4))),
                            2 => values[FieldIdx::from_u32(0)] = Operand::Copy(Place::from(Local::from_u32(3))),
                            _ => { values.pop(); },
                        }
                    });
                }
            }
            let drop_block = BasicBlock::from_u32(if method == "ok_or" { 3 } else { 2 });
            for change in 0..6 {
                reject(tcx, instance, body, &mut result.mutations[6], "Drop field", |b| {
                    let TerminatorKind::Drop { place, target, unwind, replace, drop, async_fut } = &mut b.basic_blocks.as_mut()[drop_block].terminator_mut().kind else { unreachable!() };
                    match change {
                        0 => *place = Place::from(Local::from_u32(1)),
                        1 => *target = bb,
                        2 => *unwind = UnwindAction::Continue,
                        3 => *replace = true,
                        4 => *drop = Some(bb),
                        _ => *async_fut = Some(Local::from_u32(2)),
                    }
                });
            }
            reject(tcx, instance, body, &mut result.mutations[6], "Drop replaced by Call", |b| b.basic_blocks.as_mut()[drop_block].terminator = Some(actual_call));
        }
        let (instance, _) = fixture_call(tcx, "then_add", "and_then");
        let body = tcx.instance_mir(instance.def);
        let call_block = BasicBlock::from_u32(3);
        let (_, wrong_call) = fixture_call(tcx, "then_function", "and_then");
        for change in 0..7 {
            reject(tcx, instance, body, &mut result.mutations[7], "callback Call", |b| {
                let TerminatorKind::Call { func, args, destination, target, unwind, .. } = &mut b.basic_blocks.as_mut()[call_block].terminator_mut().kind else { unreachable!() };
                match change {
                    0 => { let TerminatorKind::Call { func: other, .. } = &wrong_call.kind else { unreachable!() }; *func = other.clone(); }
                    1 => args[0].node = Operand::Copy(Place::from(Local::from_u32(5))),
                    2 => args[1].node = Operand::Move(Place::from(Local::from_u32(5))),
                    3 => *args = args[..1].to_vec().into_boxed_slice(),
                    4 => *destination = Place::from(Local::from_u32(1)),
                    5 => *target = Some(BasicBlock::from_u32(5)),
                    _ => *unwind = UnwindAction::Continue,
                }
            });
        }
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
name = "fe2o3_core_attention_option_fixture"
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
        .expect("build genuine AMD target core for Option callback");
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
            "fe2o3-core-attention-option-v1-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create core Option fixture directory");
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
            "rustc".to_owned(), "--crate-name".to_owned(), "fe2o3_core_attention_option_fixture".to_owned(),
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
        callbacks.results.expect("core Option callback")
    }).map_err(|payload| {
        if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else {
            "core Option fixture initialization panicked with a non-string payload".to_owned()
        }
    }));
    result
        .as_ref()
        .unwrap_or_else(|message| panic!("cached core Option fixture failure: {message}"))
        .clone()
}

#[test]
fn attention_option_real_provider_and_closure_positives() { assert_eq!(compiler_results().positives, 5); }

#[test]
fn attention_option_real_identity_payload_and_callback_refusals() { assert_eq!(compiler_results().refusals, 15); }

#[test]
fn attention_option_real_shape_mutations() { assert_eq!(compiler_results().mutations[0], 18); }

#[test]
fn attention_option_real_local_type_and_scope_mutations() { assert_eq!(compiler_results().mutations[1], 26); }

#[test]
fn attention_option_real_source_scope_mutations() { assert_eq!(compiler_results().mutations[2], 43); }

#[test]
fn attention_option_real_operations_and_cleanup_mutations() { assert_eq!(compiler_results().mutations[3], 49); }

#[test]
fn attention_option_real_switch_and_return_mutations() { assert_eq!(compiler_results().mutations[4], 10); }

#[test]
fn attention_option_real_payload_and_aggregate_mutations() { assert_eq!(compiler_results().mutations[5], 31); }

#[test]
fn attention_option_real_drop_mutations() { assert_eq!(compiler_results().mutations[6], 14); }

#[test]
fn attention_option_real_callback_call_mutations() { assert_eq!(compiler_results().mutations[7], 7); }
