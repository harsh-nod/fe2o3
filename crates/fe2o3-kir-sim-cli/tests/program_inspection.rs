//! Actual CLI behavior using explicitly synthetic canonical fixtures.
//! Real Rust export qualification remains separate.
#![cfg(target_os = "linux")]

#[path = "fixtures/diagnostic_kir_v17.rs"]
mod fixture;
#[path = "fixtures/diagnostic_kir_v16.rs"]
mod fixture_v16;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const FILES: [&str; 4] = [
    "program.kir",
    "request.json",
    "redirect.kir",
    "version16.kir",
];

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-program-inspection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str) -> PathBuf {
        assert!(FILES.contains(&name));
        self.0.join(name)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        for name in FILES {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}
fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fe2o3-program-inspect"));
    command.env_clear();
    command
}
fn run(kir: &Path, request: &Path) -> Output {
    command().args([kir, request]).output().unwrap()
}
fn run_text(kir: &Path, request: &Path) -> Output {
    command()
        .arg("--text")
        .args([kir, request])
        .output()
        .unwrap()
}
fn refused_both(kir: &Path, request: &Path) {
    let original = run(kir, request);
    let text = run_text(kir, request);
    assert_eq!(
        original.stderr, text.stderr,
        "formats must share admission/preflight"
    );
    refused(original);
    refused(text);
}
fn refused(output: Output) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = std::str::from_utf8(&output.stderr).unwrap();
    assert!(error.starts_with("inspection refused: "));
    assert!(error.len() <= 256);
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::new();
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

#[test]
fn installed_binary_reports_one_three_sixteen_used_and_unused_programs() {
    let directory = Directory::new();
    let kir = directory.file("program.kir");
    let request = directory.file("request.json");
    let request_bytes = fixture::request([19, 23, 42]);
    fs::write(&request, &request_bytes).unwrap();
    let programs: [&[u16]; 3] = [
        &[8],
        &[133, 307, 413],
        &[
            0, 141, 323, 188, 321, 58, 64, 181, 60, 331, 73, 194, 56, 333, 16, 72,
        ],
    ];
    for descriptors in programs {
        for used in [false, true] {
            let owner = fixture::owner(&fixture::module_with_program(
                used,
                fixture::program(descriptors),
            ));
            fs::write(&kir, owner.canonical_bytes()).unwrap();
            let output = run(&kir, &request);
            assert!(output.status.success(), "{:?}", output);
            assert!(output.stderr.is_empty());
            assert!(output.stdout.len() <= 8192);
            let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            // Keep the original example report kind/shape for compatibility.
            assert_eq!(
                value["kind"],
                "diagnostic_ordered_program_inspection_example"
            );
            assert_eq!(value["canonical"]["sha256"], hex(owner.identity().digest()));
            assert_eq!(value["canonical"]["wire_version"], 17);
            assert_eq!(value["canonical"]["bytes"], owner.canonical_bytes().len());
            assert_eq!(value["coordinate"]["function_ordinal"], 0);
            assert_eq!(value["coordinate"]["block_ordinal"], 0);
            assert_eq!(value["coordinate"]["operation_ordinal"], 0);
            assert_eq!(value["raw_block_id"], 7);
            assert_eq!(value["input_value_ids"], serde_json::json!([0, 1, 2]));
            assert_eq!(value["result_value_id"], 4);
            assert_eq!(value["declared_program"]["count"], descriptors.len());
            assert_eq!(
                value["declared_instruction_steps"]
                    .as_array()
                    .unwrap()
                    .len(),
                descriptors.len()
            );
            assert_eq!(
                value["register_plan"],
                serde_json::json!({
                    "scratch": 32, "output": 33, "inputs": [34, 35, 36], "vgpr_high_water": 37,
                })
            );
            assert_eq!(value["authority"], "observation_only");
            for flag in [
                "physical_register_values_available",
                "instruction_microsteps_available",
                "source_authentication",
                "artifact_authority",
                "production_resume_authority",
                "hardware_execution",
                "proof_authority",
            ] {
                assert_eq!(value[flag], false);
            }
            assert_eq!(fs::read(&kir).unwrap(), owner.canonical_bytes());
            assert_eq!(fs::read(&request).unwrap(), request_bytes);
            let listing = run_text(&kir, &request);
            assert!(listing.status.success(), "{listing:?}");
            assert!(listing.stderr.is_empty());
            assert!(listing.stdout.len() <= 8192);
            let text = std::str::from_utf8(&listing.stdout).unwrap();
            assert!(text.starts_with("Declared ordered-program syntax (not native disassembly)\n"));
            assert!(text.contains(&format!(
                "canonical: V17 sha256={} bytes={}",
                hex(owner.identity().digest()),
                owner.canonical_bytes().len()
            )));
            assert!(text.contains("coordinate: function=0 block=0 operation=0 raw_block_id=7"));
            assert!(text.contains("kernel: \"program\"\nfunction: \"program_entry\""));
            let expected: Vec<_> = value["declared_instruction_steps"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(index, step)| {
                    let mut line = format!(
                        "  {index:02}: {} v{}",
                        step["instruction"].as_str().unwrap(),
                        step["output"]
                    );
                    for register in step["inputs"].as_array().unwrap() {
                        line.push_str(&format!(", v{register}"));
                    }
                    line
                })
                .collect();
            assert_eq!(
                text.lines()
                    .filter(|line| line.starts_with("  "))
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(text.contains("CPU preflight only, no execution"));
            assert!(text.contains("proof/artifact/resume/hardware authority"));
            assert_eq!(run(&kir, &request).stdout, output.stdout);
            assert_eq!(fs::read(&kir).unwrap(), owner.canonical_bytes());
            assert_eq!(fs::read(&request).unwrap(), request_bytes);
        }
    }
}

#[test]
fn malformed_wrong_version_request_and_redirected_inputs_refuse() {
    let directory = Directory::new();
    let kir = directory.file("program.kir");
    let request = directory.file("request.json");
    let owner = fixture::owner(&fixture::module(true));
    let request_bytes = fixture::request([19, 23, 42]);
    fs::write(&request, &request_bytes).unwrap();
    fs::write(&kir, b"not canonical").unwrap();
    refused_both(&kir, &request);
    let older = fixture_v16::owner(&fixture_v16::module(true));
    let old_path = directory.file("version16.kir");
    fs::write(&old_path, older.canonical_bytes()).unwrap();
    refused_both(&old_path, &request);
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    let redirected = directory.file("redirect.kir");
    symlink(&kir, &redirected).unwrap();
    refused_both(&redirected, &request);
    refused_both(&directory.0, &request);
    let mut wrong: serde_json::Value = serde_json::from_slice(&request_bytes).unwrap();
    wrong["workgroup"] = serde_json::json!([32, 1, 1]);
    fs::write(&request, serde_json::to_vec(&wrong).unwrap()).unwrap();
    refused_both(&kir, &request);
    fs::write(&request, b"{\"schema\":\"x\",\"schema\":\"x\"}").unwrap();
    refused_both(&kir, &request);
    fs::write(&request, b"\xff").unwrap();
    refused_both(&kir, &request);
}

#[test]
fn help_and_exact_bounded_arguments() {
    let help = command().arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    assert!(
        std::str::from_utf8(&help.stdout)
            .unwrap()
            .starts_with("usage: fe2o3-program-inspect KIR_PATH REQUEST_PATH\n")
    );
    assert!(help.stdout.len() < 512);
    refused(command().output().unwrap());
    refused(command().arg("missing").output().unwrap());
    refused(command().args(["a", "b", "c"]).output().unwrap());
    refused(command().args(["a", &"x".repeat(4097)]).output().unwrap());
}

#[test]
fn text_arguments_are_closed_before_input_admission() {
    for args in [
        vec!["--text"],
        vec!["--text", "--text", "a", "b"],
        vec!["--text", "a", "b", "extra"],
    ] {
        let output = command().args(args).output().unwrap();
        assert!(
            std::str::from_utf8(&output.stderr)
                .unwrap()
                .contains("usage:")
        );
        refused(output);
    }
    for first in ["a", "--json"] {
        let output = command().args([first, "b", "extra"]).output().unwrap();
        assert_eq!(
            output.stderr,
            b"inspection refused: exactly two paths of at most 4096 bytes are required\n"
        );
        refused(output);
    }
    for args in [
        vec!["--text".to_owned(), "a".to_owned(), "x".repeat(4097)],
        vec!["--text".to_owned(), "x".repeat(4097), "b".to_owned()],
    ] {
        let output = command().args(args).output().unwrap();
        assert!(
            std::str::from_utf8(&output.stderr)
                .unwrap()
                .contains("paths of at most 4096 bytes")
        );
        refused(output);
    }
}

#[test]
fn escaped_name_growth_refuses_without_publishing_partial_text() {
    let directory = Directory::new();
    let kir = directory.file("program.kir");
    let request = directory.file("request.json");
    let mut module = fixture::module(true);
    let escaped = "\u{1b}".repeat(1024);
    module.functions[0].id = escaped.as_str().into();
    module.kernels[0].entry = module.functions[0].id.clone();
    module.kernels[0].id = escaped.as_str().into();
    let owner = fixture::owner(&module);
    fs::write(&kir, owner.canonical_bytes()).unwrap();
    let mut data: serde_json::Value =
        serde_json::from_slice(&fixture::request([19, 23, 42])).unwrap();
    data["kernel"] = escaped.into();
    let request_bytes = serde_json::to_vec(&data).unwrap();
    fs::write(&request, &request_bytes).unwrap();
    let output = run_text(&kir, &request);
    assert_eq!(
        output.stderr,
        b"inspection refused: bounded inspection text serialization failed\n"
    );
    refused(output);
    assert_eq!(fs::read(&kir).unwrap(), owner.canonical_bytes());
    assert_eq!(fs::read(&request).unwrap(), request_bytes);
}
