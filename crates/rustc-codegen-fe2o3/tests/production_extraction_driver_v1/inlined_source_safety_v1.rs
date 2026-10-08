mod inlined_source_safety_v1 {
    use super::{ScratchTarget, materialize_source_safety_fixture};
    use serde_json::Value;
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    const CENSUS: &str = "FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1";
    const RUN_ID: &str = "FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1";
    const CRATE: &str = "fe2o3_production_source_safety_fixture";

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target"]
    fn ordinary_inline_helper_collection_rejects_erased_empty_unsafe_blocks() {
        const ROOT_NAME: &str = "inline_source_kernel";
        const HELPER_NAME: &str = "ordinary_inline_helper";
        const BASELINE: &str = r#"#![no_std]
use fe2o3_device::kernel;

#[inline(always)]
fn ordinary_inline_helper(value: u32) -> u32 {
    value
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn inline_source_kernel(value: u32) {
    let _ = ordinary_inline_helper(value);
}
"#;
        for optimized in [false, true] {
            let target = ScratchTarget::new();
            let fixture = materialize_source_safety_fixture(&target, BASELINE);
            let mode = if optimized { "optimized" } else { "normal" };
            let (output, report) = extract(
                &target,
                &fixture,
                &format!("plain-inline-baseline-{mode}"),
                optimized,
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                report["selection"]["status"], "available",
                "{report:#}:\n{stderr}"
            );
            let selection = &report["selection"]["value"];
            assert_eq!(selection["target"], "gfx942:xnack-");
            let files = selection["files"].as_array().unwrap();
            let source_path = fixture.join("src/lib.rs").canonicalize().unwrap();
            let local_files: Vec<_> = files
                .iter()
                .enumerate()
                .filter(|(_, file)| {
                    let path = PathBuf::from(file["displayPath"].as_str().unwrap());
                    let path = if path.is_absolute() {
                        path
                    } else {
                        fixture.join(path)
                    };
                    path.canonicalize().unwrap() == source_path
                })
                .collect();
            assert_eq!(
                local_files.len(),
                1,
                "exact original source row:\n{report:#}"
            );
            let (file_index, file) = local_files[0];
            assert_eq!(std::fs::read(&source_path).unwrap(), BASELINE.as_bytes());
            assert_eq!(file["originalSha256"], sha256(BASELINE.as_bytes()));
            assert_eq!(
                file["originalBytes"].as_u64().unwrap(),
                BASELINE.len() as u64
            );
            let functions = selection["functions"].as_array().unwrap();
            let roots: Vec<_> = functions
                .iter()
                .filter(|f| f["role"] == "kernel-entry")
                .collect();
            assert_eq!(roots.len(), 1);
            assert_eq!(roots[0]["logicalName"], ROOT_NAME);
            assert_root_definition(roots[0], BASELINE, file_index, ROOT_NAME);
            let mut tokens = BTreeSet::new();
            for function in functions {
                if function["role"] == "kernel-entry" {
                    continue;
                }
                if function["identifier"]["status"] != "available" {
                    continue;
                }
                let origin = &function["identifier"]["value"]["expansion"];
                if origin["file"].as_u64().unwrap() != file_index as u64 {
                    continue;
                }
                let coordinates = &origin["coordinates"];
                let start =
                    usize::try_from(coordinates["original_start"].as_u64().unwrap()).unwrap();
                let end = usize::try_from(coordinates["original_end"].as_u64().unwrap()).unwrap();
                let token = &BASELINE[start..end];
                let expected = BASELINE.find(&format!("fn {token}(")).unwrap() + 3;
                assert_eq!((start, end), (expected, expected + token.len()));
                assert!(tokens.insert(token));
            }
            if optimized {
                assert_eq!(
                    functions.len(),
                    1,
                    "optimized reproduction must inline the helper: {report:#}"
                );
            } else {
                assert!(tokens.contains(HELPER_NAME), "{report:#}");
            }

            assert_eq!(BASELINE.matches("    value\n}").count(), 1);
            let hostile = BASELINE.replace("    value\n}", "    unsafe {};\n    value\n}");
            std::fs::write(&source_path, &hostile).unwrap();
            let (output, report) = extract(
                &target,
                &fixture,
                &format!("plain-inline-empty-unsafe-{mode}"),
                optimized,
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{mode}: {stderr}");
            for expected in [
                &format!("ordinary production kernel `{ROOT_NAME}`"),
                "user-provided unsafe block",
                HELPER_NAME,
                "src/lib.rs",
            ] {
                assert!(
                    stderr.contains(expected),
                    "{mode}: missing {expected}:\n{stderr}"
                );
            }
            if optimized {
                for expected in ["original source", "inlined instance"] {
                    assert!(
                        stderr.contains(expected),
                        "optimized refusal must retain {expected}:\n{stderr}"
                    );
                }
            }
            assert_eq!(report["selection"]["status"], "unavailable", "{stderr}");
            assert_eq!(
                report["selection"]["value"], "collection not reached",
                "{stderr}"
            );
            assert_eq!(report["extractionSucceeded"], false, "{stderr}");
        }
    }

    pub(super) fn extract(
        target: &ScratchTarget,
        fixture: &Path,
        case: &str,
        optimized: bool,
    ) -> (Output, Value) {
        let census = target.path().join(format!("{case}.json"));
        assert!(
            !census.exists(),
            "each invocation requires a fresh census path"
        );
        let run_id = sha256(format!("{}/{case}", target.path().display()).as_bytes());
        let mut command = Command::new(env!("CARGO"));
        for variable in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_RUSTC_WRAPPER",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "FE2O3_CRATE_BINDING_ID_V1",
            "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1",
            "FE2O3_EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX",
            "FE2O3_EXTRACT_RANKED_MEMORY_V1",
            "FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1",
            "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1",
            "FE2O3_EXTRACT_AMDGPU_COMPILER_HANDOFF_PATH_V1",
            "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1",
            "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1",
            "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V2",
            "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V3",
            "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V4",
            "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V5",
            "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6",
            CENSUS,
            RUN_ID,
        ] {
            command.env_remove(variable);
        }
        let optimization = if optimized {
            "-Copt-level=3 -Zmir-opt-level=2"
        } else {
            "-Copt-level=0 -Zmir-opt-level=0"
        };
        let output = command
            .current_dir(fixture)
            .env(
                "RUSTC_WORKSPACE_WRAPPER",
                env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
            )
            .env("FE2O3_EXTRACT_CRATE_V1", CRATE)
            .env(CENSUS, &census)
            .env(RUN_ID, &run_id)
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TERM_COLOR", "never")
            .env(
                "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                format!(
                    "-Zalways-encode-mir -Ctarget-cpu=gfx942 \
                 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 {optimization}"
                ),
            )
            .args([
                "rustc",
                "--release",
                "--offline",
                "-Zbuild-std=core",
                "--lib",
                "--target",
                "amdgcn-amd-amdhsa",
                "--target-dir",
            ])
            .arg(target.path().join("cargo"))
            // Force only this final crate to execute; Cargo does not track census env.
            .args(["--", "-Zsrc-hash-algorithm=sha256", "--cfg"])
            .arg(format!("fe2o3_source_census_run=\"{case}\""))
            .output()
            .expect("run the actual production extraction wrapper");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let bytes = std::fs::read(&census)
            .unwrap_or_else(|error| panic!("{case}: no diagnostic census: {error}:\n{stderr}"));
        assert!(
            bytes.len() <= 4 * 1024 * 1024,
            "{case}: diagnostic output bound"
        );
        let report: Value = serde_json::from_slice(&bytes).expect("actual census JSON");
        assert_eq!(
            report["schema"], "fe2o3-diagnostic-source-census-v1",
            "{stderr}"
        );
        assert_eq!(report["diagnosticOnly"], true, "{stderr}");
        assert_eq!(report["qualified"], false, "{stderr}");
        assert_eq!(report["authenticatesCompilerExecution"], false, "{stderr}");
        assert_eq!(
            report["extractionSucceeded"],
            output.status.success(),
            "{stderr}"
        );
        assert_eq!(report["extractionMode"]["kind"], "semantic-mir", "{stderr}");
        assert_eq!(report["runId"], run_id, "{stderr}");
        assert_eq!(
            report["workingDirectory"],
            fixture.to_str().unwrap(),
            "{stderr}"
        );
        let args: Vec<String> = serde_json::from_value(report["arguments"].clone()).unwrap();
        for (option, value) in [
            ("--crate-name", CRATE.to_owned()),
            ("--target", "amdgcn-amd-amdhsa".to_owned()),
            ("--cfg", format!("fe2o3_source_census_run=\"{case}\"")),
        ] {
            assert!(
                args.windows(2)
                    .any(|pair| pair[0] == option && pair[1] == value),
                "{args:?}"
            );
        }
        assert!(
            args.iter()
                .any(|arg| arg == "src/lib.rs"
                    || arg == fixture.join("src/lib.rs").to_str().unwrap()),
            "{args:?}"
        );
        assert!(
            args.iter().any(|arg| arg == "-Zsrc-hash-algorithm=sha256"),
            "{args:?}"
        );
        for flag in optimization.split_whitespace() {
            assert!(
                args.iter().any(|arg| arg == flag),
                "missing actual {flag}: {args:?}"
            );
        }
        // Report the downstream outcome separately; collection is not source export.
        eprintln!(
            "source admission {case}: extraction status {}, census selection {}, functions {:?}\n{stderr}",
            output.status,
            report["selection"]["status"],
            report["selection"]["value"]["functions"]
                .as_array()
                .map(Vec::len),
        );
        (output, report)
    }

    pub(super) fn assert_root_definition(root: &Value, source: &str, file: usize, name: &str) {
        assert_eq!(root["role"], "kernel-entry");
        assert_eq!(root["logicalName"], name);
        assert_eq!(root["exportName"], name);
        for field in [
            "functionIdentity",
            "definitionIdentity",
            "monomorphizationIdentity",
        ] {
            let identity = root[field].as_str().unwrap();
            assert_eq!(identity.len(), 64);
            assert!(
                identity
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            );
        }
        let declaration = format!("pub fn {name}(value: u32)");
        assert_eq!(source.matches(&declaration).count(), 1);
        let start = source.find(&declaration).unwrap();
        assert_eq!(root["definition"]["status"], "available", "{root:#}");
        for origin in ["expansion", "callSite"] {
            let observed = &root["definition"]["value"][origin];
            assert_eq!(observed["file"].as_u64().unwrap(), file as u64);
            let coordinates = &observed["coordinates"];
            assert_eq!(
                coordinates["original_start"].as_u64().unwrap(),
                start as u64
            );
            assert_eq!(
                coordinates["original_end"].as_u64().unwrap(),
                (start + declaration.len()) as u64
            );
            assert_eq!(
                &source.as_bytes()[start..start + declaration.len()],
                declaration.as_bytes()
            );
        }
        // The kernel macro can rename the compiled definition while preserving
        // its original signature span; its optional identifier is not authority.
        if root["identifier"]["status"] == "available" {
            let identifier = &root["identifier"]["value"]["expansion"];
            assert_eq!(identifier["file"].as_u64().unwrap(), file as u64);
            let coordinates = &identifier["coordinates"];
            let token_start = start + "pub fn ".len();
            assert_eq!(
                coordinates["original_start"].as_u64().unwrap(),
                token_start as u64
            );
            assert_eq!(
                coordinates["original_end"].as_u64().unwrap(),
                (token_start + name.len()) as u64
            );
        } else {
            assert_eq!(root["identifier"]["status"], "unavailable", "{root:#}");
            assert!(!root["identifier"]["value"].as_str().unwrap().is_empty());
        }
    }

    pub(super) fn sha256(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}
