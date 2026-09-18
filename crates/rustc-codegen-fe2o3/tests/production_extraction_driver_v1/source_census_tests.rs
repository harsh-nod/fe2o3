mod source_census_tests {
    use super::{ScratchTarget, materialize_source_safety_fixture};
    use std::path::Path;
    use std::process::Command;

    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};

    const CENSUS_ENV: &str = "FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1";
    const RUN_ID_ENV: &str = "FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1";
    const CRATE: &str = "fe2o3_production_source_safety_fixture";

    enum CensusOutput {
        Disabled,
        Fresh,
        Stale,
        CrateBindingAlias,
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target"]
    fn diagnostic_source_census_preserves_selection_and_extraction_result() {
        let target = ScratchTarget::new();
        let source = census_fixture_source();
        let fixture = materialize_source_safety_fixture(&target, &source);
        let manifest_path = fixture.join("Cargo.toml");
        let mut manifest = std::fs::read_to_string(&manifest_path).expect("read fixture manifest");
        // A real default catches accidental feature unification on the beta run.
        manifest.push_str("\n[features]\ndefault = [\"alpha\"]\nalpha = []\nbeta = []\n");
        std::fs::write(&manifest_path, manifest).expect("add census fixture features");

        let mut failure_status = None;
        for (case, feature, ranked, census) in [
            ("alpha-ranked", "alpha", true, CensusOutput::Fresh),
            ("beta-ranked", "beta", true, CensusOutput::Fresh),
            ("disabled-failure", "beta", false, CensusOutput::Disabled),
            ("enabled-failure", "beta", false, CensusOutput::Fresh),
            ("stale-success", "beta", true, CensusOutput::Stale),
            ("stale-failure", "beta", false, CensusOutput::Stale),
            (
                "crate-binding-alias",
                "beta",
                true,
                CensusOutput::CrateBindingAlias,
            ),
        ] {
            let path = target.path().join(format!("{case}.json"));
            assert!(!path.exists(), "{case}: census path must be unique");
            let stale = b"{\"schema\":\"stale-census-sentinel\",\"extractionSucceeded\":true}\n";
            let mut command = clean_census_command(&fixture, &target, feature, case);
            if ranked {
                command.env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1");
            }
            match census {
                CensusOutput::Disabled => {}
                CensusOutput::Fresh => {
                    command.env(CENSUS_ENV, &path);
                }
                CensusOutput::Stale => {
                    std::fs::write(&path, stale).expect("seed stale census output");
                    command.env(CENSUS_ENV, &path);
                }
                CensusOutput::CrateBindingAlias => {
                    command.env(CENSUS_ENV, &path);
                    command.env("FE2O3_EXTRACT_CRATE_BINDING_PATH_V1", &path);
                }
            }
            let output = command
                .output()
                .unwrap_or_else(|error| panic!("{case}: run production cargo wrapper: {error}"));
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                output.status.success(),
                ranked,
                "{case}: census changed extraction result ({:?}):\n{stderr}",
                output.status,
            );
            if ranked {
                assert_eq!(
                    stderr
                        .matches("safety-verified lowering input for `census_kernel`")
                        .count(),
                    1,
                    "{case}: expected one selected ranked kernel:\n{stderr}",
                );
                for expected in [
                    "all mandatory kernel checks clean true",
                    "bounds clean true",
                    "artifact/launch authority false",
                ] {
                    assert!(
                        stderr.contains(expected),
                        "{case}: missing {expected}:\n{stderr}"
                    );
                }
            } else {
                for expected in [
                    "semantic importer authenticated rustc identity inventory",
                    "target-neutral lowering remains pending",
                    "no fallback or artifact emission was entered",
                ] {
                    assert!(
                        stderr.contains(expected),
                        "{case}: missing {expected}:\n{stderr}"
                    );
                }
                if let Some(baseline) = failure_status {
                    assert_eq!(
                        output.status, baseline,
                        "{case}: failure status changed:\n{stderr}"
                    );
                } else {
                    failure_status = Some(output.status);
                }
            }
            assert!(
                !stderr.contains("artifact/launch authority true"),
                "{case}: diagnostic census granted artifact authority:\n{stderr}",
            );
            match census {
                CensusOutput::Disabled => {
                    assert!(!path.exists(), "disabled census wrote an output:\n{stderr}");
                }
                CensusOutput::Fresh => {
                    let bytes = std::fs::read(&path).unwrap_or_else(|error| {
                        panic!("{case}: census output unavailable: {error}:\n{stderr}")
                    });
                    let report: Value = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                        panic!("{case}: invalid census JSON: {error}:\n{stderr}")
                    });
                    assert_report(&report, &fixture, feature, case, ranked, &stderr);
                    assert_eq!(report["runId"], run_id(&target, case), "{stderr}");
                    assert_selected_source(
                        &report["selection"]["value"],
                        &fixture,
                        &source,
                        feature,
                        &stderr,
                    );
                    assert!(
                        fe2o3_kernel_ir::VerifiedSimulationBundleV6::from_canonical_bytes(bytes)
                            .is_err(),
                        "{case}: diagnostic census was accepted as an executable bundle:\n{stderr}",
                    );
                }
                CensusOutput::Stale => {
                    let after = std::fs::read(&path).unwrap_or_else(|error| {
                        panic!("{case}: stale output disappeared: {error}:\n{stderr}")
                    });
                    assert_eq!(
                        after, stale,
                        "{case}: stale census was overwritten:\n{stderr}"
                    );
                    assert!(
                        stderr.contains("diagnostic source census unavailable"),
                        "{case}: stale output refusal was not diagnosed:\n{stderr}",
                    );
                }
                CensusOutput::CrateBindingAlias => {
                    let binding = std::fs::read(&path).unwrap_or_else(|error| {
                        panic!("{case}: crate-binding output unavailable: {error}:\n{stderr}")
                    });
                    assert_eq!(
                        binding.len(),
                        65,
                        "{case}: invalid crate-binding output:\n{stderr}"
                    );
                    assert_eq!(
                        binding[64], b'\n',
                        "{case}: missing binding newline:\n{stderr}"
                    );
                    let hex = std::str::from_utf8(&binding[..64]).unwrap_or_else(|error| {
                        panic!("{case}: crate binding is not UTF-8: {error}:\n{stderr}")
                    });
                    assert_sha256(&json!(hex), &stderr);
                    assert!(
                        stderr.contains("diagnostic source census unavailable"),
                        "{case}: output alias refusal was not diagnosed:\n{stderr}",
                    );
                }
            }
        }

        std::fs::write(fixture.join("src/lib.rs"), "#![no_std]\npub fn broken( {\n").unwrap();
        let mut fatal_status = None;
        for enabled in [false, true] {
            let case = if enabled {
                "fatal-enabled"
            } else {
                "fatal-disabled"
            };
            let path = target.path().join(format!("{case}.json"));
            let mut command = clean_census_command(&fixture, &target, "beta", case);
            command.env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1");
            if enabled {
                command.env(CENSUS_ENV, &path);
            }
            let output = command.output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{case}: {stderr}");
            assert!(stderr.contains("unclosed delimiter"), "{case}: {stderr}");
            if let Some(status) = fatal_status {
                assert_eq!(output.status, status, "{stderr}");
            }
            fatal_status = Some(output.status);
            if enabled {
                let report: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                assert_eq!(report["runId"], run_id(&target, case));
                assert_eq!(report["extractionSucceeded"], false);
                assert_eq!(report["qualified"], false);
                assert_eq!(report["selection"]["status"], "unavailable");
                assert_eq!(report["selection"]["value"], "collection not reached");
            } else {
                assert!(!path.exists());
            }
        }
    }

    fn clean_census_command(
        fixture: &Path,
        target: &ScratchTarget,
        feature: &str,
        case: &str,
    ) -> Command {
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
            CENSUS_ENV,
            RUN_ID_ENV,
        ] {
            command.env_remove(variable);
        }
        command
            .current_dir(fixture)
            .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
            .env("FE2O3_EXTRACT_CRATE_V1", CRATE)
            .env(RUN_ID_ENV, run_id(target, case))
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TERM_COLOR", "never")
            .env(
                "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
            )
            .args([
                "rustc",
                "--release",
                "--offline",
                "-Zbuild-std=core",
                "--lib",
                "--no-default-features",
                "--features",
                feature,
                "--target",
                "amdgcn-amd-amdhsa",
                "--target-dir",
            ])
            .arg(target.path().join("cargo"))
            // Cargo does not fingerprint the census environment. Change only the
            // final crate's arguments so each invocation runs, reusing dependencies.
            .args(["--", "-Zsrc-hash-algorithm=sha256", "--cfg"])
            .arg(format!("fe2o3_source_census_run=\"{case}\""));
        command
    }

    fn run_id(target: &ScratchTarget, case: &str) -> String {
        sha256(format!("{}/{case}", target.path().display()).as_bytes())
    }

    fn sha256(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn census_fixture_source() -> String {
        let mut source = String::from(concat!(
            "#![no_std]\nuse fe2o3_device::kernel;\n",
            "// Multibyte prefix: \u{00e9}\u{03bb}\u{1f600}\n",
            "#[cfg(any(all(feature = \"alpha\", feature = \"beta\"), ",
            "not(any(feature = \"alpha\", feature = \"beta\"))))]\n",
            "compile_error!(\"select exactly one census feature\");\n",
        ));
        for (feature, other) in [("alpha", "beta"), ("beta", "alpha")] {
            source.push_str(&format!(
                "\n// {feature}: \u{00e9}\u{1f600}\n\
                 #[cfg(all(feature = \"{feature}\", not(feature = \"{other}\")))]\n\
                 #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\n\
                 pub fn census_kernel(_{feature}: u32) {{\n    ordinary_helper(_{feature});\n}}\n\
                 #[cfg(all(feature = \"{feature}\", not(feature = \"{other}\")))]\n\
                 #[inline(never)]\n\
                 fn r#ordinary_helper(_{feature}: u32) {{}}\n",
            ));
        }
        format!("\u{feff}{}", source.replace('\n', "\r\n"))
    }

    fn assert_report(
        report: &Value,
        fixture: &Path,
        feature: &str,
        case: &str,
        succeeded: bool,
        stderr: &str,
    ) {
        assert_eq!(
            report["schema"], "fe2o3-diagnostic-source-census-v1",
            "{stderr}"
        );
        assert_eq!(report["diagnosticOnly"], true, "{stderr}");
        assert_eq!(report["qualified"], false, "{stderr}");
        assert_eq!(report["authenticatesCompilerExecution"], false, "{stderr}");
        assert_eq!(report["extractionSucceeded"], succeeded, "{stderr}");
        assert_eq!(
            report["extractionMode"]["kind"],
            if succeeded {
                "ranked-memory"
            } else {
                "semantic-mir"
            }
        );
        assert_eq!(
            report["workingDirectory"],
            fixture.to_str().unwrap(),
            "{stderr}"
        );
        assert_eq!(
            report["selection"]["status"], "available",
            "{report:#}:\n{stderr}"
        );
        assert_eq!(
            report["selection"]["value"]["target"], "gfx942:xnack-",
            "{stderr}"
        );

        let arguments: Vec<String> = serde_json::from_value(report["arguments"].clone())
            .unwrap_or_else(|error| panic!("arguments are not Vec<String>: {error}:\n{stderr}"));
        for (option, value) in [
            ("--crate-name", CRATE.to_owned()),
            ("--target", "amdgcn-amd-amdhsa".to_owned()),
            ("--cfg", format!("feature=\"{feature}\"")),
            ("--cfg", format!("fe2o3_source_census_run=\"{case}\"")),
        ] {
            assert!(
                arguments
                    .windows(2)
                    .any(|pair| pair[0] == option && pair[1] == value),
                "actual driver arguments omitted {option} {value}: {arguments:?}:\n{stderr}",
            );
        }
        for expected in ["-Zsrc-hash-algorithm=sha256", "-Coverflow-checks=on"] {
            assert!(
                arguments.iter().any(|argument| argument == expected),
                "driver arguments omitted {expected}: {arguments:?}:\n{stderr}",
            );
        }
        assert!(
            arguments.iter().any(|argument| argument == "src/lib.rs"
                || argument == fixture.join("src/lib.rs").to_str().unwrap()),
            "driver arguments omitted actual source input: {arguments:?}:\n{stderr}",
        );
        let other = if feature == "alpha" { "beta" } else { "alpha" };
        for forbidden in [
            format!("feature=\"{other}\""),
            "feature=\"default\"".to_owned(),
        ] {
            assert!(
                !arguments
                    .windows(2)
                    .any(|pair| pair[0] == "--cfg" && pair[1] == forbidden),
                "unexpected enabled feature {forbidden}: {arguments:?}:\n{stderr}",
            );
        }
    }

    fn assert_selected_source(
        selection: &Value,
        fixture: &Path,
        source: &str,
        feature: &str,
        stderr: &str,
    ) {
        let normalized = source.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let files = selection["files"]
            .as_array()
            .unwrap_or_else(|| panic!("missing census files: {selection:#}:\n{stderr}"));
        assert_eq!(
            files.len(),
            1,
            "expected only selected fixture source: {selection:#}:\n{stderr}"
        );
        let file = &files[0];
        assert_sha256(&file["identity"], stderr);
        let display_path = file["displayPath"]
            .as_str()
            .unwrap_or_else(|| panic!("missing source display path:\n{stderr}"));
        assert_eq!(
            fixture.join(display_path),
            fixture.join("src/lib.rs"),
            "{stderr}"
        );
        let hash = sha256(source.as_bytes());
        assert_eq!(
            file["originalSha256"], hash,
            "original BOM/CRLF/UTF-8 hash:\n{stderr}"
        );
        assert_eq!(
            file["compiledSourceHash"],
            format!("sha256={hash}"),
            "{stderr}"
        );
        assert_eq!(file["originalBytes"], json!(source.len()), "{stderr}");
        assert_eq!(file["normalizedBytes"], json!(normalized.len()), "{stderr}");

        let functions = selection["functions"]
            .as_array()
            .unwrap_or_else(|| panic!("missing selected functions: {selection:#}:\n{stderr}"));
        assert_eq!(
            functions.len(),
            2,
            "expected selected kernel and ordinary helper: {selection:#}:\n{stderr}"
        );
        let roots: Vec<_> = functions
            .iter()
            .filter(|function| function["role"] == "kernel-entry")
            .collect();
        assert_eq!(
            roots.len(),
            1,
            "cfg selected an unexpected root roster: {selection:#}:\n{stderr}"
        );
        let root = roots[0];
        assert_eq!(root["logicalName"], "census_kernel", "{stderr}");
        assert_eq!(root["exportName"], "census_kernel", "{stderr}");
        let helper = functions
            .iter()
            .find(|function| function["role"] == "internal-helper")
            .unwrap_or_else(|| panic!("ordinary helper not collected: {selection:#}:\n{stderr}"));
        assert!(helper["logicalName"].is_null(), "{stderr}");
        assert!(
            helper["exportName"]
                .as_str()
                .is_some_and(|name| !name.is_empty()),
            "{stderr}"
        );
        for function in functions {
            for identity in [
                "functionIdentity",
                "definitionIdentity",
                "monomorphizationIdentity",
            ] {
                assert_sha256(&function[identity], stderr);
            }
        }
        assert_ne!(
            root["functionIdentity"], helper["functionIdentity"],
            "{stderr}"
        );

        // rustc's definition anchor covers the signature, not the full body.
        let kernel_text = format!("pub fn census_kernel(_{feature}: u32)");
        let helper_text = format!("fn r#ordinary_helper(_{feature}: u32)");
        assert_span(&root["definition"], source, &kernel_text, None, stderr);
        assert_span(&helper["definition"], source, &helper_text, None, stderr);
        assert_span(
            &helper["identifier"],
            source,
            &helper_text,
            Some("r#ordinary_helper"),
            stderr,
        );
        // The typed macro synthesizes the compiled root's identifier. Its
        // logical name is a registration fact, not an exact source token.
        assert_eq!(
            root["identifier"]["status"], "unavailable",
            "{root:#}:\n{stderr}"
        );
        assert_eq!(
            root["identifier"]["value"], "identifier token does not match compiled definition",
            "unexpected missing root identifier: {root:#}:\n{stderr}",
        );
    }

    fn assert_span(
        observation: &Value,
        source: &str,
        definition: &str,
        identifier: Option<&str>,
        stderr: &str,
    ) {
        assert_eq!(
            observation["status"], "available",
            "missing ordinary source span: {observation:#}:\n{stderr}"
        );
        let span = &observation["value"];
        assert_sha256(&span["expansionChainSha256"], stderr);
        assert_eq!(
            span["expansionDepth"], 0,
            "ordinary tokens acquired macro ancestry: {span:#}:\n{stderr}"
        );
        let normalized = source.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let normalized_definition = definition.replace("\r\n", "\n");
        let original_definition_start = source.find(definition).expect("unique fixture definition");
        let normalized_definition_start = normalized
            .find(&normalized_definition)
            .expect("normalized fixture definition");
        let (original_start, normalized_start, original_text, normalized_text) = match identifier {
            Some(name) => (
                original_definition_start + definition.find(name).unwrap(),
                normalized_definition_start + normalized_definition.find(name).unwrap(),
                name,
                name,
            ),
            None => (
                original_definition_start,
                normalized_definition_start,
                definition,
                normalized_definition.as_str(),
            ),
        };
        let original_end = original_start + original_text.len();
        let normalized_end = normalized_start + normalized_text.len();
        assert!(
            original_start > normalized_start,
            "fixture must exercise BOM/CRLF offsets"
        );
        assert!(
            source[..original_start].chars().count() < original_start,
            "fixture must exercise UTF-8 byte offsets"
        );
        for origin in ["expansion", "callSite"] {
            assert_eq!(
                span[origin]["file"], 0,
                "span refers to the wrong source file:\n{stderr}"
            );
            assert_eq!(
                span[origin]["coordinates"],
                json!({
                    "normalized_start": normalized_start,
                    "normalized_end": normalized_end,
                    "original_start": original_start,
                    "original_end": original_end,
                }),
                "{origin} did not select the exact enabled definition/identifier: {definition:?}:\n{stderr}",
            );
            assert_eq!(
                &source[original_start..original_end],
                original_text,
                "{stderr}"
            );
            assert_eq!(
                &normalized[normalized_start..normalized_end],
                normalized_text,
                "{stderr}"
            );
        }
    }

    fn assert_sha256(value: &Value, stderr: &str) {
        let text = value
            .as_str()
            .unwrap_or_else(|| panic!("missing SHA-256 identity: {value}:\n{stderr}"));
        assert!(
            text.len() == 64
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "invalid SHA-256 identity {text:?}:\n{stderr}",
        );
    }
}
