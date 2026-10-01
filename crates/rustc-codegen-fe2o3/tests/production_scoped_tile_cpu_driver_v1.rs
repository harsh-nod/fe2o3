#![cfg(target_os = "linux")]
#![deny(warnings)]

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "production_scoped_tile_cpu_driver_v1/cases.rs"]
mod cases;
#[path = "production_scoped_tile_cpu_driver_v1/debug.rs"]
mod debug;
#[path = "production_scoped_tile_cpu_driver_v1/process.rs"]
mod process;

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-scoped-tile-cli-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        eprintln!("scoped tile CLI evidence retained at {}", path.display());
        Self(path)
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn read(path: &Path) -> Vec<u8> {
    use std::io::Read;
    const BOUND: u64 = 8 * 1024 * 1024;
    let metadata = std::fs::symlink_metadata(path).unwrap();
    assert!(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= BOUND,
        "invalid bounded publication {}",
        path.display()
    );
    let file = std::fs::File::open(path).unwrap();
    assert!(file.metadata().unwrap().is_file());
    let mut bytes = Vec::new();
    file.take(BOUND + 1).read_to_end(&mut bytes).unwrap();
    assert!(!bytes.is_empty() && bytes.len() as u64 <= BOUND);
    bytes
}

fn document(path: &Path) -> Value {
    serde_json::from_slice(&read(path)).unwrap()
}

fn command(binary: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(binary);
    command
        .current_dir(workspace())
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("HIP_VISIBLE_DEVICES", "")
        .env("ROCR_VISIBLE_DEVICES", "")
        .env_remove("FE2O3_CONTEXT_PROTOCOL_SOURCE");
    command
}

struct Binaries {
    exporter: PathBuf,
    simulator: PathBuf,
    debugger: PathBuf,
}

fn build_public_clis(scratch: &Scratch) -> Binaries {
    let mut build = command(env!("CARGO"));
    build.args([
        "build",
        "--offline",
        "--locked",
        "-j1",
        "--message-format=json-render-diagnostics",
        "-p",
        "fe2o3-kir-sim-cli",
        "--bin",
        "fe2o3-kir-sim",
        "-p",
        "fe2o3-debug-cli",
        "--bin",
        "fe2o3-debug",
    ]);
    let capture = process::run(&mut build, &scratch.0, "build-public-clis", 1800);
    process::success(&capture, "build public CLIs");
    let artifact = |name: &str| {
        let paths: Vec<PathBuf> = capture
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<Value>(line).expect("Cargo JSON"))
            .filter(|message| {
                message["reason"] == "compiler-artifact" && message["target"]["name"] == name
            })
            .filter_map(|message| message["executable"].as_str().map(PathBuf::from))
            .collect();
        assert_eq!(paths.len(), 1, "one built {name} executable");
        assert!(paths[0].is_file());
        paths[0].clone()
    };
    let exporter = PathBuf::from(env!("CARGO_BIN_EXE_fe2o3-export-sim"));
    assert!(exporter.with_file_name("fe2o3-rustc-extract").is_file());
    Binaries {
        exporter,
        simulator: artifact("fe2o3-kir-sim"),
        debugger: artifact("fe2o3-debug"),
    }
}

fn parse_marker(stderr: &[u8], order: &str) -> Result<Value, String> {
    const PREFIX: &str = "fe2o3 diagnostic scoped tile V18: actual Rust -> scoped semantic MIR V29 -> exact scalar candidate KIR V18; order=";
    const SUFFIX: &str = "; observation_only=true, source_authentication_exported=false, ranked/formal/protected/artifact/load/launch/hardware_authority=false; different orders need not implement equivalent whole-kernel output";
    let stderr = std::str::from_utf8(stderr).map_err(|e| e.to_string())?;
    let mut lines = stderr
        .lines()
        .filter(|line| line.contains("fe2o3 diagnostic scoped tile V18:"));
    let line = lines.next().ok_or("missing completed export marker")?;
    if lines.next().is_some() {
        return Err("multiple completed export markers".into());
    }
    let value = line
        .strip_prefix(PREFIX)
        .and_then(|line| line.strip_suffix(SUFFIX))
        .ok_or("malformed completed export marker")?;
    let (actual_order, value) = value
        .split_once(", canonical_identity ")
        .ok_or("missing order")?;
    let expected_order = match order {
        "blocked" => "Blocked",
        "striped" => "Striped",
        _ => return Err("unknown requested order".into()),
    };
    if actual_order != expected_order {
        return Err("changed order".into());
    }
    let (canonical, value) = value.split_once(", ").ok_or("missing canonical identity")?;
    let (bytes, value) = value
        .split_once(" byte(s), source_semantic ")
        .ok_or("missing length")?;
    let (source, value) = value
        .split_once(", pending_identity ")
        .ok_or("missing source identity")?;
    let (pending, schedule) = value
        .split_once(", schedule_identity ")
        .ok_or("missing schedule")?;
    for identity in [canonical, source, pending, schedule] {
        if identity.len() != 64
            || !identity
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || identity.bytes().all(|byte| byte == b'0')
        {
            return Err("invalid nonzero identity".into());
        }
    }
    let bytes: u64 = bytes.parse().map_err(|_| "invalid canonical length")?;
    if bytes == 0 {
        return Err("empty canonical bytes".into());
    }
    Ok(
        json!({"canonical":canonical,"bytes":bytes,"source":source,"pending":pending,"schedule":schedule}),
    )
}

fn check_inventory(report: &Value, marker: &Value, bytes: &[u8]) -> String {
    for (key, expected) in [
        ("schema", json!("fe2o3-kernel-inventory-v1")),
        ("status", json!("ok")),
        ("authority", json!("observation_only")),
        ("simulated", json!(false)),
        ("simulator_admission", json!("not_checked")),
        ("native_abi", json!("unavailable")),
        ("target_profile", json!("not_encoded")),
        (
            "additional_launch_requirements",
            json!("unavailable_from_kernel_metadata"),
        ),
    ] {
        assert_eq!(report[key], expected, "inventory {key}");
    }
    for key in [
        "source_authentication",
        "proof_authority",
        "compiler_execution_authority",
        "launch_authority",
        "hardware_observed",
        "performance_prediction",
    ] {
        assert_eq!(report[key], false, "inventory {key}");
    }
    assert_eq!(
        report["kir"],
        json!({"wire_version":18,"identity_sha256":marker["canonical"],
        "raw_sha256":sha256(bytes),"canonical_bytes":bytes.len()})
    );
    assert_eq!(marker["bytes"], bytes.len());
    assert!(report["storage_layouts"].as_u64().unwrap() > 0);
    assert!(
        report["module"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    let kernels = report["kernels"].as_array().unwrap();
    assert_eq!(kernels.len(), 1);
    let kernel = &kernels[0];
    let id = kernel["id"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap()
        .to_owned();
    assert!(
        kernel["entry"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(kernel["request_abi"], "entry_parameter_order");
    assert_eq!(kernel["source_parameter_names"], "unavailable");
    assert_eq!(kernel["workgroup_size"], json!([64, 1, 1]));
    assert_eq!(kernel["results"], json!([]));
    let slice = |index: u32, access: &str| {
        json!({
            "index":index,"type":{"kind":"slice","address_space":"global","access":access,
              "element":{"kind":"scalar","type":"u32","bits":32}},
            "request_encoding":{"kind":"buffer_or_buffer_view","element":"u32",
              "required_access":access,"alignment":"caller_selected_and_checked_at_preflight"}
        })
    };
    assert_eq!(
        kernel["parameters"],
        json!([slice(0,"read_only"),
        {"index":1,"type":{"kind":"scalar","type":"u64","bits":64},
         "request_encoding":{"kind":"scalar","type":"u64"}},slice(2,"read_write")])
    );
    id
}

fn export(scratch: &Scratch, bins: &Binaries, order: &str) -> (PathBuf, Value, Value, String) {
    let kir = scratch.0.join(format!("{order}.kir"));
    let mut export = command(&bins.exporter);
    export
        .args([
            "--diagnostic-kir-v18",
            "--diagnostic-tile-order",
            order,
            "--crate",
            "fe2o3_workgroup_sync_v1",
            "--target",
            "gfx942",
            "--output",
        ])
        .arg(&kir)
        .arg("--target-dir")
        .arg(scratch.0.join("export-target"))
        .args(["--", "--offline", "--manifest-path"])
        .arg(workspace().join("examples/workgroup_sync_v1/Cargo.toml"))
        .args([
            "--no-default-features",
            "--features",
            "mixed-tile-u32-kernel",
        ]);
    let capture = process::run(&mut export, &scratch.0, &format!("{order}-export"), 1800);
    process::success(&capture, order);
    let marker = parse_marker(&capture.stderr, order).expect("one exact completed export");
    let bytes = read(&kir);
    let inventory_path = scratch.0.join(format!("{order}.inventory.json"));
    let mut inspect = command(&bins.simulator);
    inspect
        .args(["inspect", "--diagnostic-kir-v18"])
        .arg(&kir)
        .arg("--output")
        .arg(&inventory_path);
    process::silent(
        &process::run(&mut inspect, &scratch.0, &format!("{order}-inventory"), 120),
        "inventory",
    );
    let inventory = document(&inventory_path);
    let kernel = check_inventory(&inventory, &marker, &bytes);
    assert_eq!(read(&kir), bytes, "inventory changed canonical bytes");
    (kir, marker, inventory, kernel)
}

fn simulate(
    scratch: &Scratch,
    bins: &Binaries,
    kir: &Path,
    inventory: &Value,
    kernel: &str,
    order: &str,
    case: cases::Case,
    ordinal: usize,
) -> PathBuf {
    let request = cases::request(kernel, case);
    let path = scratch.0.join(format!("{order}-{ordinal}.request.json"));
    let result = scratch.0.join(format!("{order}-{ordinal}.result.json"));
    process::write_json(&path, &request);
    let mut simulate = command(&bins.simulator);
    simulate
        .arg("--diagnostic-kir-v18")
        .arg(kir)
        .arg("--request")
        .arg(&path)
        .arg("--output")
        .arg(&result);
    process::silent(
        &process::run(
            &mut simulate,
            &scratch.0,
            &format!("{order}-sim-{ordinal}"),
            120,
        ),
        "simulation",
    );
    cases::check_result(&document(&result), &request, case, order, &inventory["kir"]);
    path
}

fn refusals(scratch: &Scratch, bins: &Binaries, kir: &Path, request: &Path, order: &str) {
    let mut wrong = document(request);
    wrong["arguments"][1]["type"] = json!("index");
    let wrong_path = scratch.0.join(format!("{order}-wrong-base.json"));
    process::write_json(&wrong_path, &wrong);
    let mut command = command(&bins.simulator);
    command
        .arg("--diagnostic-kir-v18")
        .arg(kir)
        .arg("--request")
        .arg(&wrong_path);
    let capture = process::run(
        &mut command,
        &scratch.0,
        &format!("{order}-wrong-base"),
        120,
    );
    assert_eq!(capture.status.code(), Some(1));
    assert!(capture.stdout.is_empty());
    let error: Value = serde_json::from_slice(&capture.stderr).unwrap();
    assert_eq!(error["stage"], "preflight");
    assert_eq!(error["kind"], "preflight_argument_type");
    let schedule = scratch.0.join(format!("{order}-forbidden-schedule.json"));
    let mut record = self::command(&bins.simulator);
    record
        .arg("--diagnostic-kir-v18")
        .arg(kir)
        .arg("--request")
        .arg(request)
        .arg("--record-canonical-schedule")
        .arg(&schedule);
    let capture = process::run(
        &mut record,
        &scratch.0,
        &format!("{order}-forbidden-schedule"),
        120,
    );
    assert_eq!(capture.status.code(), Some(1));
    assert!(capture.stdout.is_empty() && !schedule.exists());
    let error: Value = serde_json::from_slice(&capture.stderr).unwrap();
    assert_eq!(error["stage"], "arguments");
    assert_eq!(error["kind"], "schedule_input_unsupported");
    let mut wave = self::command(&bins.debugger);
    wave.args(["sim", "--diagnostic-kir-v18"])
        .arg(kir)
        .arg("--request")
        .arg(request)
        .args(["--protocol", "jsonl", "--wave-width", "32"]);
    let capture = process::run(&mut wave, &scratch.0, &format!("{order}-wave32"), 120);
    assert_eq!(capture.status.code(), Some(1));
    assert!(capture.stdout.is_empty());
    let error: Value = serde_json::from_slice(&capture.stderr).unwrap();
    assert_eq!(
        error,
        json!({"schema":"fe2o3-debug-bootstrap-error-v1","status":"error",
        "stage":"arguments","code":"invalid_command_line",
        "message":"diagnostic KIR V18 requires wave64 and does not support source maps or persisted schedule replay"})
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD source target; CPU only"]
fn ordinary_mixed_tile_source_executes_public_cpu_cli_paths() {
    let scratch = Scratch::new("ordinary");
    let bins = build_public_clis(&scratch);
    let mut identities = Vec::new();
    let mut simulations = 0;
    let mut sessions = Vec::new();
    for order in ["blocked", "striped"] {
        let (kir, marker, inventory, kernel) = export(&scratch, &bins, order);
        let original = read(&kir);
        let mut debug_request = None;
        for (ordinal, case) in cases::cases().into_iter().enumerate() {
            let request = simulate(
                &scratch, &bins, &kir, &inventory, &kernel, order, case, ordinal,
            );
            simulations += 1;
            if ordinal == 25 {
                debug_request = Some(request);
            }
        }
        let request = debug_request.unwrap();
        sessions.push(debug::run(&scratch, &bins.debugger, &kir, &request, order));
        refusals(&scratch, &bins, &kir, &request, order);
        assert_eq!(
            read(&kir),
            original,
            "CLI observations changed exported KIR"
        );
        identities.push(marker);
    }
    assert_eq!(identities[0]["source"], identities[1]["source"]);
    assert_eq!(identities[0]["pending"], identities[1]["pending"]);
    assert_ne!(identities[0]["canonical"], identities[1]["canonical"]);
    assert_ne!(identities[0]["schedule"], identities[1]["schedule"]);
    assert_eq!(simulations, 52);
    process::write_json(
        &scratch.0.join("summary.json"),
        &json!({
            "schema":"fe2o3-scoped-tile-cli-test-v1","authority":"observation_only",
            "hardware_observed":false,"performance_prediction":false,
            "source_manifest":"examples/workgroup_sync_v1/Cargo.toml",
            "source_feature":"mixed-tile-u32-kernel","fixture_source_injection":false,
            "inventories":2,"simulations":simulations,"debugger_sessions":sessions,
            "negative_controls":10,"orders":identities,"full_simt_tile_pair_qualified":false,
            "host_oracle_unit_tests":"separate CI gate"
        }),
    );
    eprintln!(
        "SCOPED_TILE_PUBLIC_CPU_CLI_PASS inventories=2 simulations=52 debugger_sessions=2 negative_controls=10 hardware_observed=false"
    );
}

#[test]
fn completed_export_marker_is_exact_and_nonempty() {
    let hash = "1".repeat(64);
    let marker = format!(
        "fe2o3 diagnostic scoped tile V18: actual Rust -> scoped semantic MIR V29 -> exact scalar candidate KIR V18; order=Blocked, canonical_identity {hash}, 1 byte(s), source_semantic {hash}, pending_identity {hash}, schedule_identity {hash}; observation_only=true, source_authentication_exported=false, ranked/formal/protected/artifact/load/launch/hardware_authority=false; different orders need not implement equivalent whole-kernel output"
    );
    assert!(parse_marker(marker.as_bytes(), "blocked").is_ok());
    assert!(parse_marker(b"", "blocked").is_err());
    assert!(parse_marker(format!("{marker}\n{marker}").as_bytes(), "blocked").is_err());
    assert!(parse_marker(marker.as_bytes(), "striped").is_err());
    assert!(
        parse_marker(
            marker.replace("1 byte(s)", "0 byte(s)").as_bytes(),
            "blocked"
        )
        .is_err()
    );
    assert!(
        parse_marker(
            marker
                .replace("observation_only=true", "observation_only=false")
                .as_bytes(),
            "blocked"
        )
        .is_err()
    );
}
