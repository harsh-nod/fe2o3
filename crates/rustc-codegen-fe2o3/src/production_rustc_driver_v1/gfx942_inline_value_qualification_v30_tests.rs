//! Opt-in, pinned-nightly observation of the actual source scalar projection.
//!
//! This does not install reference bindings, amend a ranked graph, or produce
//! proof/artifact/launch authority. The memory-only ranked graph is unchanged.
//! Run the ignored parent under the repository's serialized Cargo validation.
//! Dependencies and bounded process logs are retained in the reported directory.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fe2o3_pliron::{
    ProductionOverflowContractV2 as Overflow, ProductionSemanticBinaryOpV2 as Binary,
    ProductionSemanticExpressionV2 as Expression, ProductionSemanticScalarTypeV2 as Scalar,
};
use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, PortablePackageIdentityV1, RustcInvocationV2,
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    ordered_rustc_codegen_metadata_v1, portable_rustc_metadata_v1,
};
use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Callbacks, Compilation, Compiler, TyCtxt};

const CRATE_NAME: &str = "fe2o3_assembly_authoring_v30_fixture";
const CHILD_ENV: &str = "FE2O3_TEST_ISA_VALUE_OBSERVATION_V30";
const CONTROL_ENV: &str = "FE2O3_TEST_ISA_VALUE_PROCESS_CONTROL_V30";
const OUTPUT_ENV: &str = "FE2O3_TEST_ISA_VALUE_OUTPUT_V30";
const MODULE: &str = "production_rustc_driver_v1::gfx942_inline_value_qualification_v30_tests";
const CAP: usize = 16 * 1024 * 1024;
const DISK_CAP: u64 = 500 * 1024 * 1024;
const U32: Scalar = Scalar::Integer {
    signed: false,
    bits: 32,
};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assembly-authoring-v30")
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(format!("{} is not a bounded regular file", path.display()));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit || bytes.len() as u64 != metadata.len() {
        return Err("bounded file changed size during observation".into());
    }
    Ok(bytes)
}

fn sanitized(command: &mut Command) -> &mut Command {
    for (name, _) in std::env::vars_os() {
        let text = name.to_string_lossy();
        if text.starts_with("FE2O3_")
            || text.starts_with("CARGO_TARGET_") && text.ends_with("_RUSTFLAGS")
            || matches!(
                text.as_ref(),
                "RUSTFLAGS"
                    | "CARGO_ENCODED_RUSTFLAGS"
                    | "CARGO_BUILD_TARGET"
                    | "RUSTC_WRAPPER"
                    | "RUSTC_WORKSPACE_WRAPPER"
                    | "CARGO_BUILD_RUSTC_WRAPPER"
                    | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
            )
        {
            command.env_remove(name);
        }
    }
    command
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "0")
}

struct Capture {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

// Poll the fresh target only; never scan or remove a shared build directory.
fn directory_bytes(root: &Path) -> Result<u64, String> {
    if !root.exists() {
        return Ok(0);
    }
    let mut pending = vec![root.to_owned()];
    let mut entries = 0_usize;
    let mut bytes = 0_u64;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            entries += 1;
            if entries > 100_000 {
                return Err("fresh dependency target exceeds entry bound".into());
            }
            let metadata = match entry.path().symlink_metadata() {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.to_string()),
            };
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or("dependency size overflow")?;
                if bytes > DISK_CAP {
                    return Err("fresh dependency target exceeds 500 MiB".into());
                }
            }
        }
    }
    Ok(bytes)
}

fn run_bounded(
    command: &mut Command,
    timeout: Duration,
    output_cap: usize,
    fresh_target: Option<&Path>,
) -> Result<Capture, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let group = child.id();
    let (send, receive) = mpsc::sync_channel(4);
    let readers = [
        child
            .stdout
            .take()
            .map(|v| Box::new(v) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|v| Box::new(v) as Box<dyn Read + Send>),
    ];
    let mut threads = Vec::new();
    for (stream, reader) in readers.into_iter().enumerate() {
        let send = send.clone();
        threads.push(std::thread::spawn(move || {
            let mut reader = reader.expect("piped stream");
            loop {
                let mut chunk = vec![0; 4096];
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(count) => {
                        chunk.truncate(count);
                        if send.send(Ok((stream, chunk))).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = send.send(Err(error.to_string()));
                        break;
                    }
                }
            }
        }));
    }
    drop(send);
    let started = Instant::now();
    let mut disk_checked = Instant::now();
    let mut streams = [Vec::new(), Vec::new()];
    let mut observed = 0_usize;
    let mut disconnected = false;
    let result = loop {
        if started.elapsed() >= timeout {
            break Err("process exceeded timeout".to_owned());
        }
        if disk_checked.elapsed() >= Duration::from_secs(1) {
            if let Some(path) = fresh_target
                && let Err(error) = directory_bytes(path)
            {
                break Err(error);
            }
            disk_checked = Instant::now();
        }
        match receive.recv_timeout(Duration::from_millis(20)) {
            Ok(Ok((stream, chunk))) => {
                observed += chunk.len();
                if observed > output_cap {
                    break Err("process exceeded output bound".to_owned());
                }
                streams[stream].extend(chunk);
            }
            Ok(Err(error)) => break Err(error),
            Err(RecvTimeoutError::Disconnected) => disconnected = true,
            Err(RecvTimeoutError::Timeout) => {}
        }
        match child.try_wait() {
            Ok(Some(status)) if disconnected => break Ok(status),
            Ok(_) => {
                if disconnected {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            Err(error) => break Err(error.to_string()),
        }
    };
    if result.is_err() {
        // Safe std API establishes a dedicated process group. No pre_exec or
        // unsafe process-wide environment mutation is used in this test suite.
        let killed = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{group}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !killed.is_ok_and(|status| status.success()) {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
    drop(receive);
    for thread in threads {
        thread.join().map_err(|_| "process reader panicked")?;
    }
    let [stdout, stderr] = streams;
    Ok(Capture {
        status: result?,
        stdout,
        stderr,
    })
}

fn checked(command: &mut Command, directory: &Path, name: &str, target: Option<&Path>) -> Vec<u8> {
    let capture =
        run_bounded(command, Duration::from_secs(300), CAP, target).unwrap_or_else(|error| {
            panic!(
                "{name}: {error}; retained directory {}",
                directory.display()
            )
        });
    fs::write(directory.join(format!("{name}.stdout")), &capture.stdout).unwrap();
    fs::write(directory.join(format!("{name}.stderr")), &capture.stderr).unwrap();
    assert!(
        capture.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&capture.stderr)
    );
    if let Some(path) = target {
        directory_bytes(path).unwrap();
    }
    capture.stdout
}

fn select_artifact(bytes: &[u8], name: &str, manifest: &Path) -> Result<PathBuf, String> {
    if bytes.len() > CAP {
        return Err("artifact JSON exceeds bound".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut selected = None;
    let mut finished = false;
    for line in text.lines().filter(|line| !line.is_empty()) {
        let item: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if item["reason"] == "build-finished" {
            if finished || item["success"] != true {
                return Err("invalid Cargo completion".into());
            }
            finished = true;
        }
        if item["reason"] != "compiler-artifact" || item["target"]["name"] != name {
            continue;
        }
        if item["manifest_path"].as_str().map(Path::new) != Some(manifest)
            || item["target"]["kind"] != json!(["lib"])
            || item["profile"]["test"] != false
        {
            return Err(format!("unexpected artifact identity for {name}"));
        }
        let files = item["filenames"]
            .as_array()
            .ok_or("missing artifact filenames")?;
        let candidates = files
            .iter()
            .filter_map(Value::as_str)
            .map(PathBuf::from)
            .filter(|path| path.extension().is_some_and(|ext| ext == "rmeta"))
            .collect::<Vec<_>>();
        let [path] = candidates.as_slice() else {
            return Err(format!("ambiguous metadata artifact for {name}"));
        };
        if selected.replace(path.clone()).is_some() {
            return Err(format!("duplicate artifact for {name}"));
        }
    }
    if !finished {
        return Err("missing successful Cargo completion".into());
    }
    selected.ok_or_else(|| format!("missing artifact for {name}"))
}

fn artifact_in_target(bytes: &[u8], name: &str, manifest: &Path, target: &Path) -> PathBuf {
    let path = select_artifact(bytes, name, manifest)
        .unwrap()
        .canonicalize()
        .unwrap();
    assert!(path.starts_with(target.canonicalize().unwrap()));
    let metadata = path.metadata().unwrap();
    assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= DISK_CAP);
    path
}

fn invocation(directory: &Path) -> (Vec<String>, String, String) {
    let fixture = fixture().canonicalize().unwrap();
    let manifest = fixture.join("Cargo.toml");
    let metadata: Value =
        serde_json::from_slice(&read_bounded(&directory.join("metadata.stdout"), CAP).unwrap())
            .unwrap();
    let packages = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|package| {
            package["manifest_path"].as_str().map(Path::new) == Some(manifest.as_path())
        })
        .collect::<Vec<_>>();
    let [package] = packages.as_slice() else {
        panic!("expected exactly one fixture package");
    };
    assert_eq!(package["name"], "fe2o3-assembly-authoring-v30-fixture");
    let identity = PortablePackageIdentityV1::new(
        package["name"].as_str().unwrap(),
        package["version"].as_str().unwrap(),
        Sha256::digest(read_bounded(&manifest, 1024 * 1024).unwrap()).into(),
    )
    .unwrap();
    let sysroot =
        String::from_utf8(read_bounded(&directory.join("sysroot.stdout"), 4096).unwrap()).unwrap();
    let sysroot = Path::new(sysroot.trim_end());
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    let artifacts = read_bounded(&directory.join("dependencies.stdout"), CAP).unwrap();
    let target = directory.join("dependencies");
    let device_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fe2o3-device/Cargo.toml")
        .canonicalize()
        .unwrap();
    let device = artifact_in_target(&artifacts, "fe2o3_device", &device_manifest, &target);
    let core_manifest = sysroot
        .join("lib/rustlib/src/rust/library/core/Cargo.toml")
        .canonicalize()
        .unwrap();
    let core = artifact_in_target(&artifacts, "core", &core_manifest, &target);
    let mut args = vec![
        sysroot.join("bin/rustc").to_str().unwrap().to_owned(),
        "--crate-name".into(),
        CRATE_NAME.into(),
        fixture.join("src/lib.rs").to_str().unwrap().into(),
        "--edition=2024".into(),
        "--crate-type=lib".into(),
        "--target=amdgcn-amd-amdhsa".into(),
        "--emit=metadata".into(),
        "-Copt-level=3".into(),
        "-Cpanic=abort".into(),
        "-Cembed-bitcode=no".into(),
        "-Cdebug-assertions=off".into(),
        "-Coverflow-checks=on".into(),
        "-Ctarget-cpu=gfx942".into(),
        "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".into(),
        "-Zalways-encode-mir".into(),
        "-Zunstable-options".into(),
        "--sysroot".into(),
        sysroot.to_str().unwrap().into(),
        "--out-dir".into(),
        directory.join("analysis-output").to_str().unwrap().into(),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", target.join("release/deps").display()),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "--extern".into(),
        format!("noprelude:core={}", core.display()),
    ];
    let os = args.iter().map(OsString::from).collect::<Vec<_>>();
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&os).unwrap() else {
        panic!("not compile");
    };
    let portable = portable_rustc_metadata_v1(compile, &identity).unwrap();
    args.push(format!("-Cmetadata={portable}"));
    let os = args.iter().map(OsString::from).collect::<Vec<_>>();
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&os).unwrap() else {
        panic!("not compile");
    };
    let ordered = ordered_rustc_codegen_metadata_v1(compile).unwrap();
    assert_eq!(ordered, [portable.as_str()]);
    // Observe this actual direct test invocation, not an invented original
    // Cargo metadata salt and not a compiler-closure attestation.
    let observation = derive_cargo_metadata_build_observation_v2(&ordered).to_hex();
    let binding = derive_crate_binding_id_v1(CRATE_NAME, [portable.as_str()]).to_hex();
    super::require_canonical_overflow_checks_v1(&args).unwrap();
    (args, binding, observation)
}

fn expected_expression() -> Expression {
    let symbol = |argument| Expression::Symbol {
        symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(argument).unwrap(),
        scalar: U32,
    };
    let constant = |bits| Expression::Constant { scalar: U32, bits };
    let binary = |operation, lhs, rhs| Expression::Binary {
        operation,
        scalar: U32,
        overflow: Overflow::Wrapping,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    };
    // Source arguments: out, a, b. Both moves preserve a's exact scalar leaf.
    let sum = binary(Binary::Add, symbol(1), symbol(2));
    let difference = binary(Binary::Subtract, sum, symbol(2));
    let toggled = binary(Binary::BitXor, difference, symbol(2));
    let low = binary(Binary::BitAnd, toggled, constant(255));
    binary(Binary::BitOr, low, constant(256))
}

#[derive(Default)]
struct ObservationCallbacks {
    result: Option<Result<Expression, String>>,
}

impl Callbacks for ObservationCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let ranked = super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?
            .verify_general_kernel_checks()
            .map_err(|e| e.to_string())?;
            let [root] = ranked.ranked_roots() else {
                return Err("expected one ranked root".into());
            };
            if root.function_name() != "assembly_chain"
                || !ranked.all_kernel_checks_are_clean()
                || !ranked.bounds_are_clean()
                || ranked.grants_artifact_or_launch_authority()
            {
                return Err("unexpected ranked observation contract".into());
            }
            let [write] = root.observed_reference_writes() else {
                return Err("expected one projected GPU write".into());
            };
            let expression = write.value.clone().map_err(str::to_owned)?;
            if expression != expected_expression() {
                return Err(format!("actual source expression differs: {expression:#?}"));
            }
            expression.validate().map_err(|e| e.to_string())?;
            expression
                .validate_static_domains()
                .map_err(|e| e.to_string())?;
            Ok(expression)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "pinned nightly real AMD rustc; run parent under serialized Cargo validation"]
fn actual_source_seven_call_expression() {
    let directory = std::env::var_os(OUTPUT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "fe2o3-isa-value-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
        });
    fs::create_dir(&directory).expect("observation directory must be new");
    eprintln!(
        "retaining actual source-value observation in {}",
        directory.display()
    );
    let original = read_bounded(&fixture().join("src/lib.rs"), 1024 * 1024).unwrap();
    assert_eq!(
        original,
        include_bytes!("../../tests/fixtures/assembly-authoring-v30/src/lib.rs")
    );
    let mut rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    let sysroot = checked(
        sanitized(&mut rustc).args(["--print", "sysroot"]),
        &directory,
        "sysroot",
        None,
    );
    let sysroot = PathBuf::from(std::str::from_utf8(&sysroot).unwrap().trim_end());
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    let mut metadata = Command::new(sysroot.join("bin/cargo"));
    checked(
        sanitized(&mut metadata)
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(fixture().join("Cargo.toml")),
        &directory,
        "metadata",
        None,
    );
    let target = directory.join("dependencies");
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut cargo).args(["check", "--release", "--locked", "--offline", "-Zbuild-std=core",
        "-p", "fe2o3-device", "--target", "amdgcn-amd-amdhsa", "--message-format=json", "--manifest-path"])
        .arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&target)
        .env("RUSTC", sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &directory, "dependencies", Some(&target));
    fs::create_dir(directory.join("analysis-output")).unwrap();
    let (args, binding, observation) = invocation(&directory);
    fs::write(
        directory.join("rustc-argv.json"),
        serde_json::to_vec_pretty(&args).unwrap(),
    )
    .unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap());
    checked(
        sanitized(&mut child)
            .args([
                "--exact",
                &format!("{MODULE}::actual_source_expression_child"),
                "--ignored",
                "--nocapture",
            ])
            .env(CHILD_ENV, &directory)
            .env(CRATE_BINDING_ID_ENV_V1, binding)
            .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, observation)
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", "fe2o3-assembly-authoring-v30-fixture")
            .env("CARGO_PKG_VERSION", "0.0.0")
            .env("CARGO_PRIMARY_PACKAGE", "1")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        &directory,
        "callback",
        None,
    );
    // A zero-test child exit must not be mistaken for callback execution.
    // This directory is new, and only the callback child writes this record.
    assert_eq!(
        read_bounded(&directory.join("actual-expression.txt"), 64 * 1024).unwrap(),
        format!("{:#?}\n", expected_expression()).as_bytes()
    );
    assert_eq!(
        read_bounded(&fixture().join("src/lib.rs"), 1024 * 1024).unwrap(),
        original
    );
    assert!(
        !fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .any(|entry| entry.is_ok()),
        "callback must stop before emitting a linkable artifact"
    );
    fs::write(directory.join("observation.json"), serde_json::to_vec_pretty(&json!({
        "schema": "fe2o3-test-source-isa-value-observation-v30", "source_unchanged": true,
        "actual_rustc_callback": true, "projected_write_count": 1,
        "exact_source_expression": "u32_wrapping_or(and(xor(sub(add(a,b),b),b),255),256)",
        "moves": "erased only in scalar value abstraction; source has seven calls",
        "reference_binding_added": false, "ranked_graph_amended": false,
        "checked_owner_scalar_qualification": false, "compiler_closure_attestation": "unavailable",
        "grants_proof_authority": false, "grants_artifact_or_launch_authority": false,
        "hardware_observed": false
    })).unwrap()).unwrap();
}

#[test]
#[ignore = "isolated child entered only by actual_source_seven_call_expression"]
fn actual_source_expression_child() {
    let directory =
        PathBuf::from(std::env::var_os(CHILD_ENV).expect("missing parent observation directory"));
    let (args, binding, observation) = invocation(&directory);
    assert_eq!(std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(), binding);
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    let retained: Vec<String> = serde_json::from_slice(
        &read_bounded(&directory.join("rustc-argv.json"), 64 * 1024).unwrap(),
    )
    .unwrap();
    assert_eq!(args, retained);
    let mut callbacks = ObservationCallbacks::default();
    super::require_canonical_overflow_checks_v1(&args).unwrap();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let expression = callbacks
        .result
        .expect("actual callback was not reached")
        .unwrap();
    fs::write(
        directory.join("actual-expression.txt"),
        format!("{expression:#?}\n"),
    )
    .unwrap();
}

#[test]
fn expected_expression_controls_reject_wrong_bits_type_and_overflow() {
    let expected = expected_expression();
    expected.validate().unwrap();
    expected.validate_static_domains().unwrap();
    for mutation in 0..3 {
        let mut changed = expected.clone();
        let Expression::Binary {
            scalar,
            overflow,
            rhs,
            ..
        } = &mut changed
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                **rhs = Expression::Constant {
                    scalar: U32,
                    bits: 512,
                }
            }
            1 => {
                *scalar = Scalar::Integer {
                    signed: true,
                    bits: 32,
                }
            }
            _ => *overflow = Overflow::Checked,
        }
        assert_ne!(changed, expected);
    }
}

#[test]
fn bounded_file_admission_rejects_symlinks_fifos_and_oversize() {
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-isa-value-file-control-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let regular = directory.join("regular");
    let link = directory.join("link");
    let fifo = directory.join("fifo");
    fs::write(&regular, b"exact").unwrap();
    std::os::unix::fs::symlink(&regular, &link).unwrap();
    let result = run_bounded(
        Command::new("mkfifo").arg(&fifo),
        Duration::from_secs(5),
        4096,
        None,
    )
    .unwrap();
    assert!(result.status.success());
    assert_eq!(read_bounded(&regular, 5).unwrap(), b"exact");
    assert!(read_bounded(&regular, 4).is_err());
    assert!(read_bounded(&link, 5).is_err());
    assert!(read_bounded(&fifo, 5).is_err());
    assert!(read_bounded(&directory, 5).is_err());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn structured_artifact_selection_rejects_ambiguity_and_spoofed_identity() {
    let record = json!({"reason":"compiler-artifact", "manifest_path":"/device/Cargo.toml",
        "target":{"name":"fe2o3_device", "kind":["lib"]}, "profile":{"test":false},
        "filenames":["/target/libfe2o3_device-exact.rmeta"]});
    let finish = json!({"reason":"build-finished", "success":true});
    let encode = |records: &[Value]| {
        records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let select = |text: &str| {
        select_artifact(
            text.as_bytes(),
            "fe2o3_device",
            Path::new("/device/Cargo.toml"),
        )
    };
    assert_eq!(
        select(&encode(&[record.clone(), finish.clone()])).unwrap(),
        Path::new("/target/libfe2o3_device-exact.rmeta")
    );
    assert!(select(&encode(&[record.clone(), record.clone(), finish.clone()])).is_err());
    assert!(select(&encode(std::slice::from_ref(&record))).is_err());
    for field in ["manifest_path", "filenames"] {
        let mut changed = record.clone();
        changed[field] = if field == "manifest_path" {
            json!("/spoof/Cargo.toml")
        } else {
            json!(["/target/one.rmeta", "/target/two.rmeta"])
        };
        assert!(select(&encode(&[changed, finish.clone()])).is_err());
    }
    assert!(select("not JSON").is_err());
}

#[test]
#[ignore = "isolated process supervision control"]
fn bounded_process_child() {
    match std::env::var(CONTROL_ENV).unwrap().as_str() {
        "output" => {
            let bytes = vec![b'x'; 16 * 1024];
            std::io::stdout().write_all(&bytes).unwrap();
        }
        "timeout" => std::thread::sleep(Duration::from_secs(5)),
        _ => panic!("unknown process control"),
    }
}

#[test]
fn process_capture_enforces_output_and_time_bounds() {
    for (mode, timeout, cap, expected) in [
        ("output", Duration::from_secs(5), 512, "output bound"),
        ("timeout", Duration::from_millis(100), 4096, "timeout"),
    ] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        sanitized(&mut command)
            .args([
                "--exact",
                &format!("{MODULE}::bounded_process_child"),
                "--ignored",
                "--nocapture",
            ])
            .env(CONTROL_ENV, mode);
        let error = run_bounded(&mut command, timeout, cap, None)
            .err()
            .expect("control must fail closed");
        assert!(error.contains(expected), "{error}");
    }
}
