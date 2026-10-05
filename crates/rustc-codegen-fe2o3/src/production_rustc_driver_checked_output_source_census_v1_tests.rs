//! Same-invocation diagnostic inputs around the existing ordinary-source P4 run.
use super::*;
use std::io::Read as _;

const MAX_JSON_BYTES: u64 = 4 * 1024 * 1024;

fn census_target(cpu: &str) -> Result<&'static str, SourceFailure> {
    fe2o3_amd_target::ProductionAmdTargetProfileV1::from_cpu(cpu)
        .map(|profile| profile.device_target())
        .ok_or_else(|| fail(SourceStage::Manifest, "unknown census target profile"))
}

pub(super) struct Pending {
    request: PathBuf,
    path: PathBuf,
    run_id: String,
    before: serde_json::Value,
}

fn bounded_json(path: &Path) -> Result<serde_json::Value, SourceFailure> {
    let input = std::fs::File::open(path).map_err(|e| fail(SourceStage::Observation, e))?;
    if !input
        .metadata()
        .map_err(|e| fail(SourceStage::Observation, e))?
        .is_file()
    {
        return Err(fail(
            SourceStage::Observation,
            "census is not a regular file",
        ));
    }
    let mut bytes = Vec::new();
    input
        .take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| fail(SourceStage::Observation, e))?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(fail(SourceStage::Observation, "census JSON bound exceeded"));
    }
    serde_json::from_slice(&bytes).map_err(|e| fail(SourceStage::Observation, e))
}

fn snapshot(workspace: &Path, request: &Path) -> Result<serde_json::Value, SourceFailure> {
    let output = clean_command("python3")
        .current_dir(workspace)
        .args(["-I", "-B"])
        .arg(workspace.join("scripts/validate-tutorial-kernel-manifest.py"))
        .arg("--repo-root")
        .arg(workspace)
        .arg("--emit-source-input-snapshot")
        .arg(request)
        .output()
        .map_err(|e| fail(SourceStage::Manifest, e))?;
    if !output.status.success() || output.stdout.len() as u64 > MAX_JSON_BYTES {
        return Err(fail(
            SourceStage::Manifest,
            corpus_cargo::diagnostics(&output),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| fail(SourceStage::Manifest, e))
}

impl Pending {
    pub(super) fn prepare(
        workspace: &Path,
        fixture: &Fixture,
        case: &Path,
    ) -> Result<Self, SourceFailure> {
        let request = case.join("source-census-input.json");
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&request)
            .map_err(|e| fail(SourceStage::Manifest, e))?;
        serde_json::to_writer(&mut output, fixture).map_err(|e| fail(SourceStage::Manifest, e))?;
        drop(output);
        let before = snapshot(workspace, &request)?;
        let mut nonce = [0_u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut input| input.read_exact(&mut nonce))
            .map_err(|e| fail(SourceStage::Invocation, e))?;
        Ok(Self {
            request,
            path: case.join("source-census.json"),
            run_id: crate::encode_hex(&nonce),
            before,
        })
    }

    pub(super) fn configure(&self, command: &mut Command) {
        fixed_census_observation::configure(command, Some((&self.path, &self.run_id)));
    }

    pub(super) fn finish(
        self,
        workspace: &Path,
        fixture: &Fixture,
        args: &[String],
        cwd: &Path,
    ) -> Result<serde_json::Value, SourceFailure> {
        let after = snapshot(workspace, &self.request)?;
        if self.before != after {
            return Err(fail(
                SourceStage::Manifest,
                "inputs changed across actual Cargo/compiler invocation",
            ));
        }
        let census = bounded_json(&self.path)?;
        let success = census["extractionSucceeded"].as_bool().ok_or_else(|| {
            fail(
                SourceStage::Observation,
                "census has no terminal extraction status",
            )
        })?;
        fixed_census_observation::check_header(
            &census,
            args,
            cwd,
            4,
            census_target(&fixture.target)?,
            &self.run_id,
            success,
        )
        .map_err(|error| fail(SourceStage::Observation, error))?;
        let selected = census["selection"]["value"]["functions"]
            .as_array()
            .filter(|_| census["selection"]["status"] == "available")
            .ok_or_else(|| {
                fail(
                    SourceStage::Observation,
                    "source census collection incomplete",
                )
            })?;
        let roots = selected
            .iter()
            .filter(|function| function["role"] == "kernel-entry")
            .map(|function| function["exportName"].as_str())
            .collect::<Vec<_>>();
        let expected = fixture
            .compiler_input
            .kernel_symbols
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if roots.len() != expected.len()
            || roots.iter().copied().collect::<Option<BTreeSet<_>>>() != Some(expected)
        {
            return Err(fail(
                SourceStage::Observation,
                "census selected root roster differs",
            ));
        }
        Ok(serde_json::json!({
            "schema": "fe2o3-tutorial-source-census-observation-v1",
            "diagnosticOnly": true, "qualified": false,
            "authenticatesCompilerExecution": false,
            "runId": self.run_id, "arguments": args,
            "workingDirectory": cwd.to_str().ok_or_else(|| fail(SourceStage::Invocation, "non-UTF8 working directory"))?,
            "before": self.before, "after": after,
            "censusSha256": digest(&serde_json::to_vec(&census).map_err(|e| fail(SourceStage::Observation, e))?),
            "census": census,
        }))
    }
}

#[test]
fn census_target_resolves_exact_profiles_and_refuses_non_cpu_spellings() {
    for (cpu, expected) in [("gfx942", "gfx942:xnack-"), ("gfx950", "gfx950:xnack-")] {
        assert_eq!(census_target(cpu).unwrap(), expected);
    }
    for cpu in [
        "",
        "gfx90a",
        "GFX942",
        "gfx942 ",
        "gfx942:xnack-",
        "gfx950:xnack+",
    ] {
        let error = census_target(cpu).unwrap_err();
        assert_eq!(error.stage, SourceStage::Manifest);
    }
}

#[test]
fn census_header_keeps_exact_target_features_after_profile_resolution() {
    let args = vec!["rustc".to_owned()];
    let cwd = Path::new("/tutorial-census-profile-test");
    let run_id = "census-target-profile-test";
    for cpu in ["gfx942", "gfx950"] {
        let expected = census_target(cpu).unwrap();
        let mut report = serde_json::json!({
            "schema": "fe2o3-diagnostic-source-census-v1",
            "diagnosticOnly": true, "qualified": false,
            "authenticatesCompilerExecution": false, "extractionSucceeded": true,
            "arguments": args, "workingDirectory": cwd,
            "extractionMode": {"kind": "fixed-checked-output", "policy": 4},
            "runId": run_id,
            "selection": {"status": "available", "value": {"target": expected}},
        });
        fixed_census_observation::check_header(&report, &args, cwd, 4, expected, run_id, true)
            .unwrap();
        for wrong in [
            "gfx942",
            "gfx950",
            "gfx942:xnack+",
            "gfx950:xnack+",
            "gfx90a:xnack-",
        ] {
            report["selection"]["value"]["target"] = wrong.into();
            assert!(
                fixed_census_observation::check_header(
                    &report, &args, cwd, 4, expected, run_id, true,
                )
                .is_err()
            );
        }
        let other = if cpu == "gfx942" {
            "gfx950:xnack-"
        } else {
            "gfx942:xnack-"
        };
        report["selection"]["value"]["target"] = other.into();
        assert!(
            fixed_census_observation::check_header(&report, &args, cwd, 4, expected, run_id, true,)
                .is_err()
        );
    }
}

#[test]
#[ignore = "real ordinary-source Cargo/callback census with current input snapshots; not compilation qualification"]
fn ordinary_fill_census_retains_exact_current_inputs_and_roots() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let manifest = std::fs::read(workspace.join(MANIFEST)).unwrap();
    let fixture = fixtures(&manifest)
        .unwrap()
        .into_iter()
        .find(|fixture| fixture.fixture_id == "gfx942-fill-simulation")
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-tutorial-census-inputs");
    let result = run_case(
        &workspace,
        &fixture,
        &scratch.path().join("case"),
        &scratch.path().join("dependencies"),
    );
    let observation = result
        .source_census
        .as_ref()
        .unwrap_or_else(|| panic!("real callback census did not complete: {result:#?}"));
    assert_eq!(observation["before"], observation["after"]);
    assert_eq!(observation["before"]["manifestSha256"], digest(&manifest));
    assert_eq!(
        observation["census"]["extractionMode"],
        serde_json::json!({"kind":"fixed-checked-output","policy":4})
    );
    assert_ne!(observation["runId"], serde_json::json!("0".repeat(64)));
    assert!(!observation["qualified"].as_bool().unwrap());
    assert!(result.source_census_error.is_none());
}
