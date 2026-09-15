//! Actual-target UI checks using the example's ordinary nested Cargo pattern.

use super::Scratch;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TOOLCHAIN: &str = "nightly-2026-04-03";
const TARGET: &str = "amdgcn-amd-amdhsa";

pub(super) struct DeviceCompilation {
    output: Output,
    source: PathBuf,
    crate_name: String,
    metadata_emitted: bool,
}

fn device_target_dir(manifest: &Path, outer: Option<&Path>, cwd: &Path) -> PathBuf {
    let root = match outer {
        Some(root) if root.is_absolute() => root.to_path_buf(),
        Some(root) => cwd.join(root),
        None => manifest.join("target"),
    };
    root.join("tutorial-ui/fe2o3-tiled-gemm-general-v1/amdgpu")
}

fn device_command(source_root: &Path, target: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(source_root)
        .env("RUSTUP_TOOLCHAIN", TOOLCHAIN)
        .env("CARGO_TARGET_DIR", target)
        // Same local UI label as capability_compile.rs, not a session-custody proof.
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Ctarget-cpu=gfx950 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Cpanic=abort",
        );
    // Preserve RUSTC/CARGO_BUILD_RUSTC: the ordinary cargo-fe2o3 test runner
    // supplies its pinned compiler handoff to trusted nested Cargo tests.
    for name in [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "FE2O3_SIMULATION_MODE_V1",
        "FE2O3_SIMULATION_ATTEMPT_V1",
    ] {
        command.env_remove(name);
    }
    command.args([
        "check",
        "--offline",
        "--lib",
        "-Zbuild-std=core",
        "--target",
        TARGET,
        "--message-format=json",
    ]);
    command
}

fn cargo_records(output: &Output) -> Vec<Value> {
    std::str::from_utf8(&output.stdout)
        .expect("Cargo JSON output is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("Cargo JSON message"))
        .collect()
}

pub(super) fn compile(case: &str, source: &str) -> DeviceCompilation {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("canonical example directory");
    let repo = manifest
        .join("../..")
        .canonicalize()
        .expect("canonical repository");
    // CARGO_TARGET_DIR survives the host-test runner's FE2O3_* scrub. Resolve
    // a relative value before changing cwd and never lock the outer target.
    let outer = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    let target = device_target_dir(
        &manifest,
        outer.as_deref(),
        &std::env::current_dir().expect("test working directory"),
    );
    let scratch = Scratch::new(case);
    let source_root = scratch.0.canonicalize().expect("canonical source scratch");
    let package = format!("fe2o3-general-gemm-amd-ui-{case}");
    let crate_name = package.replace('-', "_");
    // The real host crate resolves the macro's target-conditional host import.
    // Only actual AMD dependencies are compiled, with no host stub or fallback.
    std::fs::write(
        source_root.join("Cargo.toml"),
        format!(
            "[package]\nname = {package:?}\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\n[target.'cfg(not(target_arch = \"amdgpu\"))'.dependencies]\nfe2o3-host = {{ path = {:?} }}\n",
            repo.join("crates/fe2o3-device"),
            repo.join("crates/fe2o3-host"),
        ),
    )
    .expect("write device UI manifest");
    let source_path = source_root.join("src/lib.rs");
    std::fs::write(&source_path, source).expect("write exact device UI source");
    let output = device_command(&source_root, &target)
        .output()
        .expect("Cargo typechecks actual AMD UI source and provisions its dependencies");
    // Consume Cargo's exact artifact message instead of guessing hashed filenames.
    let metadata_emitted = cargo_records(&output).iter().any(|record| {
        record["reason"] == "compiler-artifact"
            && is_fixture_target(record, &crate_name, &source_path)
            && record["filenames"].as_array().is_some_and(|files| {
                files.iter().filter_map(Value::as_str).any(|file| {
                    Path::new(file)
                        .extension()
                        .is_some_and(|extension| extension == "rmeta")
                        && std::fs::metadata(file).is_ok_and(|metadata| metadata.len() != 0)
                })
            })
    });
    DeviceCompilation {
        output,
        source: source_path,
        crate_name,
        metadata_emitted,
    }
}

fn is_fixture_target(record: &Value, crate_name: &str, source: &Path) -> bool {
    record["target"]["name"] == crate_name
        && record["target"]["src_path"]
            .as_str()
            .is_some_and(|path| Path::new(path) == source)
}

fn is_fixture_span(span: &Value, source: &Path, source_line: &str) -> bool {
    span["is_primary"] == true
        && span["file_name"].as_str().is_some_and(|file| {
            let file = Path::new(file);
            file == source || file == Path::new("src/lib.rs")
        })
        && span["text"].as_array().is_some_and(|text| {
            text.iter().any(|line| {
                line["text"]
                    .as_str()
                    .is_some_and(|line| line.contains(source_line))
            })
        })
}

impl DeviceCompilation {
    pub(super) fn assert_success(&self, case: &str) {
        assert!(
            self.output.status.success() && self.metadata_emitted,
            "{case}: AMD device typechecking did not emit metadata:\n{}\n{}",
            String::from_utf8_lossy(&self.output.stderr),
            String::from_utf8_lossy(&self.output.stdout)
        );
    }

    pub(super) fn assert_rejected(
        &self,
        case: &str,
        code: &str,
        source_line: &str,
        expected: &[&str],
    ) {
        let stderr = format!(
            "{}\n{}",
            String::from_utf8_lossy(&self.output.stderr),
            String::from_utf8_lossy(&self.output.stdout)
        );
        assert!(
            !self.output.status.success(),
            "{case}: hostile device fixture compiled"
        );
        let mut matched = 0;
        for record in cargo_records(&self.output) {
            if record["reason"] != "compiler-message" || record["message"]["level"] != "error" {
                continue;
            }
            assert!(
                is_fixture_target(&record, &self.crate_name, &self.source),
                "{case}: dependency failed before the exact fixture:\n{stderr}"
            );
            let diagnostic = &record["message"];
            let Some(actual) = diagnostic["code"]["code"].as_str() else {
                assert!(
                    diagnostic["message"]
                        .as_str()
                        .is_some_and(|message| message.starts_with("aborting due to ")),
                    "{case}: unrelated unnumbered error:\n{stderr}"
                );
                continue;
            };
            assert_eq!(actual, code, "{case}: unrelated compiler error:\n{stderr}");
            assert!(
                diagnostic["spans"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|span| { is_fixture_span(span, &self.source, source_line) }),
                "{case}: rejection did not point at the exact capability substitution:\n{stderr}"
            );
            let rendered = diagnostic["rendered"]
                .as_str()
                .expect("rendered diagnostic");
            for expected in expected {
                assert!(
                    rendered.contains(expected),
                    "{case}: missing {expected:?}:\n{stderr}"
                );
            }
            matched += 1;
        }
        assert!(matched != 0, "{case}: no exact {code} rejection:\n{stderr}");
    }
}

#[cfg(test)]
#[path = "amdgpu_tests.rs"]
mod tests;
