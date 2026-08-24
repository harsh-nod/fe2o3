#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent,
    Module, ScalarType, Signature, Terminator, Type, ValueId, VerifiedCanonicalKernelIrV6,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-kir-sim-command-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim"))
}

fn canonical_noop_with_buffer() -> Vec<u8> {
    let element = Type::Scalar(ScalarType::U8);
    let slice = Type::slice(element, AddressSpace::Global, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![slice], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("cli-command-test");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    VerifiedCanonicalKernelIrV6::from_module(module)
        .unwrap()
        .into_canonical_bytes()
}

fn write_success_fixture(directory: &TestDirectory) -> (PathBuf, PathBuf) {
    let kir = directory.path().join("kernel.kir");
    let request = directory.path().join("request.json");
    fs::write(&kir, canonical_noop_with_buffer()).unwrap();
    fs::write(
        &request,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"kernel","grid":[2,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u8","access":"read_only","alignment":1,"bytes":"0x2a"}]}"#,
    )
    .unwrap();
    (kir, request)
}

#[test]
fn help_is_a_successful_input_free_command() {
    for argument in ["--help", "-h"] {
        let output = binary().arg(argument).env_clear().output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "usage: fe2o3-kir-sim --kir-v6 PATH --request PATH [--output PATH]\n"
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn successful_stdout_is_complete_machine_readable_json() {
    let directory = TestDirectory::new();
    let (kir, request) = write_success_fixture(&directory);
    let output = binary()
        .arg("--kir-v6")
        .arg(kir)
        .arg("--request")
        .arg(request)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema"], "fe2o3-simulation-result-v1");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["authority"], "observation_only");
    assert_eq!(value["counts"]["invocations_executed"], 2);
    assert_eq!(value["counts"]["scheduled_slots_visited"], 2);
    assert_eq!(value["arguments"][0]["value"]["bytes"], "0x2a");
}

#[test]
fn successful_output_is_private_durable_no_replace_json() {
    let directory = TestDirectory::new();
    let (kir, request) = write_success_fixture(&directory);
    let result = directory.path().join("result.json");
    let output = binary()
        .arg("--kir-v6")
        .arg(&kir)
        .arg("--request")
        .arg(&request)
        .arg("--output")
        .arg(&result)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let bytes = fs::read(&result).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["status"], "ok");
    assert_eq!(
        fs::metadata(&result).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let second = binary()
        .arg("--kir-v6")
        .arg(kir)
        .arg("--request")
        .arg(request)
        .arg("--output")
        .arg(&result)
        .output()
        .unwrap();
    assert!(!second.status.success());
    let error: serde_json::Value = serde_json::from_slice(&second.stderr).unwrap();
    assert_eq!(error["stage"], "output");
    assert_eq!(error["kind"], "output_already_exists");
    assert_eq!(fs::read(result).unwrap(), bytes);
}

#[test]
fn failures_are_stable_json_with_input_identity() {
    let output = binary().output().unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["stage"], "arguments");
    assert_eq!(value["kind"], "invalid_command_line");

    let output = binary()
        .args(["--kir-v6", "/dev/null", "--request", "/dev/null"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["stage"], "input");
    assert_eq!(value["kind"], "input_not_regular");
    assert_eq!(value["input"], "kir_v6");

    let directory = TestDirectory::new();
    let oversized = directory.path().join("oversized.kir");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let output = binary()
        .arg("--kir-v6")
        .arg(&oversized)
        .args(["--request", "/dev/null"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["kind"], "input_too_large");
    assert_eq!(value["input"], "kir_v6");
}

#[test]
fn closed_stdout_reports_real_epipe_without_gpu_runtime() {
    let directory = TestDirectory::new();
    let kir = directory.path().join("kernel.kir");
    let request = directory.path().join("request.json");
    fs::write(&kir, canonical_noop_with_buffer()).unwrap();
    let bytes = "00".repeat(4 * 1024 * 1024);
    fs::write(
        &request,
        format!(
            "{{\"schema\":\"fe2o3-simulation-request-v1\",\"kernel\":\"kernel\",\"grid\":[1,1,1],\"workgroup\":[1,1,1],\"arguments\":[{{\"kind\":\"buffer\",\"element\":\"u8\",\"access\":\"read_only\",\"alignment\":1,\"bytes\":\"0x{bytes}\"}}]}}"
        ),
    )
    .unwrap();

    let mut child = binary()
        .arg("--kir-v6")
        .arg(&kir)
        .arg("--request")
        .arg(&request)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["stage"], "output");
    assert_eq!(value["kind"], "output_write_failed");
}
