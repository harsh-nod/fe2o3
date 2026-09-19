//! Exact diagnostic observations; these values never authenticate a compiler.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct SourceRoot {
    pub(super) name: String,
    pub(super) function: [u8; 32],
    pub(super) body: [u8; 32],
}

pub(super) fn roots(semantic: &AdmittedInertSemanticMirV1) -> Result<Vec<SourceRoot>, String> {
    let mut result = Vec::new();
    let mut unique = BTreeSet::new();
    for root in semantic.roots() {
        let function = semantic
            .functions()
            .get(root.index() as usize)
            .ok_or("source root missing")?;
        let entry = function.kernel_entry().ok_or("source root entry missing")?;
        let selection = semantic
            .select_kernel_body_for_root_v1(*root)
            .filter(|selection| selection.root() == *root)
            .ok_or("exact source body selection missing")?;
        let body = semantic
            .functions()
            .get(selection.body().index() as usize)
            .ok_or("source body missing")?;
        let name = std::str::from_utf8(entry.export_symbol().as_bytes())
            .map_err(|e| e.to_string())?
            .to_owned();
        if !unique.insert(name.clone()) {
            return Err("duplicate actual source root".into());
        }
        result.push(SourceRoot {
            name,
            function: *function.identity().as_bytes(),
            body: *body.identity().as_bytes(),
        });
    }
    if result.is_empty() {
        return Err("actual source root roster is empty".into());
    }
    Ok(result)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn run_id(args: &[String], correlation: &str) -> String {
    let bytes = serde_json::to_vec(&("fixed-driver-census-test-v1", args, correlation)).unwrap();
    hex(&Sha256::digest(bytes))
}

pub(super) fn configure(command: &mut Command, enabled: Option<(&Path, &str)>) {
    use crate::collector::source_census_v1::{OUTPUT_ENV, RUN_ID_ENV};
    command.env_remove(OUTPUT_ENV).env_remove(RUN_ID_ENV);
    if let Some((path, run_id)) = enabled {
        command.env(OUTPUT_ENV, path).env(RUN_ID_ENV, run_id);
    }
}

pub(super) fn read_report(path: &Path) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("fresh source census unavailable: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid source census JSON: {e}"))
}

#[allow(
    clippy::too_many_arguments,
    reason = "independent exact invocation subjects"
)]
pub(super) fn check_header(
    report: &Value,
    args: &[String],
    cwd: &Path,
    policy: u16,
    target: &str,
    run_id: &str,
    success: bool,
) -> Result<(), String> {
    if report["schema"] != "fe2o3-diagnostic-source-census-v1"
        || report["diagnosticOnly"] != true
        || report["qualified"] != false
        || report["authenticatesCompilerExecution"] != false
        || report["extractionSucceeded"] != success
        || report["arguments"] != serde_json::to_value(args).map_err(|e| e.to_string())?
        || report["workingDirectory"] != cwd.to_str().ok_or("non-UTF8 test working directory")?
        || report["extractionMode"] != json!({"kind":"fixed-checked-output","policy":policy})
        || report["runId"] != run_id
    {
        return Err(
            "census lost exact invocation, compiled mode, status or diagnostic-only boundary"
                .into(),
        );
    }
    match report["selection"]["status"].as_str() {
        Some("available") if report["selection"]["value"]["target"] == target => Ok(()),
        Some("unavailable")
            if !success
                && report["selection"]["value"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()) =>
        {
            Ok(())
        }
        _ => Err("census selection status or canonical target changed".into()),
    }
}

fn anchor(anchor: &Value, files: &[Value]) -> Result<(), String> {
    match anchor["status"].as_str() {
        Some("unavailable")
            if anchor["value"]
                .as_str()
                .is_some_and(|value| !value.is_empty()) =>
        {
            Ok(())
        }
        Some("available") => {
            let span = &anchor["value"];
            if span["expansionChainSha256"]
                .as_str()
                .is_none_or(|value| !is_digest(value))
                || span["expansionDepth"]
                    .as_u64()
                    .is_none_or(|value| value > 64)
            {
                return Err("census expansion provenance framing".into());
            }
            for field in ["expansion", "callSite"] {
                let origin = &span[field];
                let file = origin["file"]
                    .as_u64()
                    .and_then(|id| usize::try_from(id).ok())
                    .and_then(|id| files.get(id))
                    .ok_or("census anchor has no exact source file")?;
                let coordinates = &origin["coordinates"];
                for (start, end, extent) in [
                    ("normalized_start", "normalized_end", "normalizedBytes"),
                    ("original_start", "original_end", "originalBytes"),
                ] {
                    let start = coordinates[start]
                        .as_u64()
                        .ok_or("census anchor start missing")?;
                    let end = coordinates[end]
                        .as_u64()
                        .ok_or("census anchor end missing")?;
                    let extent = file[extent]
                        .as_u64()
                        .ok_or("census source extent missing")?;
                    if start > end || end > extent {
                        return Err("census anchor outside its exact source file".into());
                    }
                }
            }
            Ok(())
        }
        _ => Err("census merged or omitted definition/identifier observations".into()),
    }
}

pub(super) fn check_selected(
    report: &Value,
    expected: &[SourceRoot],
    required_source_hashes: &[[u8; 32]],
) -> Result<(), String> {
    if report["selection"]["status"] != "available"
        || expected.is_empty()
        || required_source_hashes.is_empty()
    {
        return Err(
            "census lacks actual source selection or independently retained subjects".into(),
        );
    }
    let selection = &report["selection"]["value"];
    let functions = selection["functions"]
        .as_array()
        .ok_or("census function roster missing")?;
    let files = selection["files"]
        .as_array()
        .ok_or("census source-file roster missing")?;
    let roots = functions
        .iter()
        .filter(|row| row["role"] == "kernel-entry")
        .collect::<Vec<_>>();
    if roots.len() != expected.len() {
        return Err("census exact root count differs from actual source".into());
    }
    let mut unique_names = BTreeSet::new();
    let mut unique_functions = BTreeSet::new();
    for function in functions {
        let identity = function["functionIdentity"]
            .as_str()
            .ok_or("census function identity missing")?;
        if !is_digest(identity) || !unique_functions.insert(identity) {
            return Err("census duplicate or malformed function identity".into());
        }
        anchor(&function["definition"], files)?;
        anchor(&function["identifier"], files)?;
    }
    for source in expected {
        if !unique_names.insert(source.name.as_str()) {
            return Err("duplicate independent source root".into());
        }
        let matching = roots
            .iter()
            .filter(|row| row["exportName"] == source.name)
            .collect::<Vec<_>>();
        let [actual] = matching.as_slice() else {
            return Err("census root export missing, duplicate or foreign".into());
        };
        if actual["functionIdentity"] != hex(&source.function)
            || !functions
                .iter()
                .any(|row| row["functionIdentity"] == hex(&source.body))
        {
            return Err("census replaced actual source root or selected-body identity".into());
        }
    }
    let expected_hashes = required_source_hashes
        .iter()
        .map(|hash| hex(hash))
        .collect::<BTreeSet<_>>();
    if !expected_hashes.iter().all(|expected| {
        files
            .iter()
            .any(|file| file["originalSha256"].as_str() == Some(expected.as_str()))
    }) {
        return Err("census file roster lacks required active source bytes".into());
    }
    Ok(())
}

#[test]
fn fixed_census_oracle_binds_exact_request_and_preserves_unavailable_generated_identifiers() {
    let args = vec!["rustc".to_owned(), "-Coverflow-checks=on".to_owned()];
    let cwd = Path::new("/test-census-owner");
    let id = run_id(&args, "actual-case");
    let roots = vec![SourceRoot {
        name: "root".into(),
        function: [1; 32],
        body: [1; 32],
    }];
    let report = json!({
        "schema":"fe2o3-diagnostic-source-census-v1","diagnosticOnly":true,"qualified":false,
        "authenticatesCompilerExecution":false,"extractionSucceeded":true,
        "arguments":args,"workingDirectory":cwd,"extractionMode":{"kind":"fixed-checked-output","policy":6},"runId":id,
        "selection":{"status":"available","value":{"target":"gfx942:xnack-",
            "functions":[{"functionIdentity":hex(&[1;32]),"role":"kernel-entry","exportName":"root",
                "definition":{"status":"unavailable","value":"synthetic definition"},
                "identifier":{"status":"unavailable","value":"generated identifier token unavailable"}}],
            "files":[{"originalSha256":hex(&[2;32])}]}}
    });
    check_header(&report, &args, cwd, 6, "gfx942:xnack-", &id, true).unwrap();
    check_selected(&report, &roots, &[[2; 32]]).unwrap();
    // An unrelated known file cannot stand in for the active fixture source.
    assert!(check_selected(&report, &roots, &[[2; 32], [9; 32]]).is_err());
    assert!(check_selected(&report, &roots, &[[9; 32], [2; 32]]).is_err());
    let mut both_files = report.clone();
    both_files["selection"]["value"]["files"]
        .as_array_mut()
        .unwrap()
        .push(json!({"originalSha256":hex(&[9;32])}));
    check_selected(&both_files, &roots, &[[2; 32], [9; 32]]).unwrap();
    for field in ["qualified", "authenticatesCompilerExecution"] {
        let mut bad = report.clone();
        bad[field] = json!(true);
        assert!(check_header(&bad, &args, cwd, 6, "gfx942:xnack-", &id, true).is_err());
    }
    for bad in [
        vec!["rustc".into()],
        vec!["-Coverflow-checks=on".into(), "rustc".into()],
    ] {
        assert!(check_header(&report, &bad, cwd, 6, "gfx942:xnack-", &id, true).is_err());
    }
    assert!(check_header(&report, &args, cwd, 5, "gfx942:xnack-", &id, true).is_err());
    assert!(check_header(&report, &args, cwd, 6, "gfx950:xnack-", &id, true).is_err());
    assert!(check_header(&report, &args, cwd, 6, "gfx942:xnack-", "stale", true).is_err());
    let mut bad = report.clone();
    bad["selection"]["value"]["functions"][0]["functionIdentity"] = json!(hex(&[3; 32]));
    assert!(check_selected(&bad, &roots, &[[2; 32]]).is_err());
    let mut bad = report.clone();
    bad["selection"]["value"]["functions"]
        .as_array_mut()
        .unwrap()
        .push(report["selection"]["value"]["functions"][0].clone());
    assert!(check_selected(&bad, &roots, &[[2; 32]]).is_err());
    assert!(check_selected(&report, &roots, &[[9; 32]]).is_err());
    let mut bad = report;
    bad["selection"]["value"]["functions"][0]
        .as_object_mut()
        .unwrap()
        .remove("identifier");
    assert!(check_selected(&bad, &roots, &[[2; 32]]).is_err());
}

#[test]
fn fixed_census_oracle_keeps_definition_identifier_and_coordinate_frames_distinct() {
    let origins = json!({
        "file":0,"coordinates":{"normalized_start":1,"normalized_end":4,
            "original_start":2,"original_end":5}
    });
    let available = json!({"status":"available","value":{
        "expansion":origins,"callSite":origins,
        "expansionChainSha256":hex(&[4;32]),"expansionDepth":0
    }});
    let roots = vec![SourceRoot {
        name: "root".into(),
        function: [1; 32],
        body: [3; 32],
    }];
    let report = json!({"selection":{"status":"available","value":{
        "functions":[
            {"functionIdentity":hex(&[1;32]),"role":"kernel-entry","exportName":"root",
                "definition":available,"identifier":{"status":"unavailable","value":"generated token"}},
            {"functionIdentity":hex(&[3;32]),"role":"internal-helper","exportName":null,
                "definition":available,"identifier":available}],
        "files":[{"originalSha256":hex(&[2;32]),"normalizedBytes":7,"originalBytes":8}]
    }}});
    check_selected(&report, &roots, &[[2; 32]]).unwrap();
    for field in ["definition", "identifier"] {
        for origin in ["expansion", "callSite"] {
            for coordinate in ["normalized_end", "original_end"] {
                let mut bad = report.clone();
                bad["selection"]["value"]["functions"][1][field]["value"][origin]["coordinates"]
                    [coordinate] = json!(9);
                assert!(check_selected(&bad, &roots, &[[2; 32]]).is_err());
            }
            let mut bad = report.clone();
            bad["selection"]["value"]["functions"][1][field]["value"][origin]["file"] = json!(1);
            assert!(check_selected(&bad, &roots, &[[2; 32]]).is_err());
        }
    }
    let mut bad = report.clone();
    bad["selection"]["value"]["functions"][1]["definition"]["value"]["expansionChainSha256"] =
        json!("z".repeat(64));
    assert!(check_selected(&bad, &roots, &[[2; 32]]).is_err());
    let mut bad = report.clone();
    bad["selection"]["value"]["functions"][1]["functionIdentity"] = json!("z".repeat(64));
    assert!(check_selected(&bad, &roots, &[[2; 32]]).is_err());
    let mut bad = report;
    bad["selection"]["value"]["functions"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(check_selected(&bad, &roots, &[[2; 32]]).is_err());
}
