//! Closed requests and observations; their bytes remain inert selectors.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;

pub(super) const ORIGINAL: &str = "package-original/src/ordered_composition_publish_v1.rs";
pub(super) fn package(case: &str) -> &'static str {
    match case {
        "recompile-copy" => "package-promoted-copy",
        "recompile-preserve" => "package-promoted-preserve",
        "recompile-edit" => "package-promoted-edit",
        _ => "package-original",
    }
}
pub(super) fn candidate(case: &str) -> &'static str {
    match case {
        "copy" | "reused-candidate" => {
            "package-promoted-copy/src/ordered_composition_publish_v1.rs"
        }
        "preserve" => "package-promoted-preserve/src/ordered_composition_publish_v1.rs",
        "edit" => "package-promoted-edit/src/ordered_composition_publish_v1.rs",
        "stale-semantic" => "refused-stale-semantic.rs",
        "stale-canonical" => "refused-stale-canonical.rs",
        "stale-source" => "refused-stale-source.rs",
        "unknown-json" => "refused-unknown-json.rs",
        "unknown-instruction" => "refused-unknown-instruction.rs",
        "input-destination" => "refused-input-destination.rs",
        "register-overlap" => "refused-register-overlap.rs",
        "reused-output" => "refused-reused-output.rs",
        "missing-request" => "refused-missing-request.rs",
        _ => panic!("not a source action case"),
    }
}
pub(super) fn pre_frontend(case: &str) -> bool {
    matches!(
        case,
        "unknown-json"
            | "unknown-instruction"
            | "input-destination"
            | "register-overlap"
            | "missing-request"
    )
}
pub(super) fn refusal(case: &str) -> &'static str {
    match case {
        "stale-semantic" | "stale-canonical" => {
            "NotAttempted: stale promotion semantic/canonical selection"
        }
        "stale-source" => "NotAttempted: publisher current source size or digest differs",
        "unknown-json" | "unknown-instruction" | "input-destination" => {
            "closed promotion request JSON:"
        }
        "register-overlap" => "promotion registers:",
        "missing-request" => "promotion request open:",
        "reused-output" => "fresh ordered composition directory:",
        "reused-candidate" => "MayHaveCreatedCandidate:",
        _ => panic!("not a public refusal"),
    }
}
fn flip(s: &str) -> String {
    assert!(
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    let mut bytes = s.as_bytes().to_vec();
    bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
    String::from_utf8(bytes).unwrap()
}
pub(super) fn request(case: &str, initial: &Value, source: &str) -> Value {
    let definitions = initial["definitions"].as_array().unwrap();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0]["root_region"], true);
    let mut request = json!({
        "schema":"fe2o3-ordered-composition-source-promotion-request-v1",
        "semantic_sha256":initial["semantic_identity"],"canonical_sha256":initial["canonical_identity"],
        "definition_ordinal":definitions[0]["definition_ordinal"],
        "original_path":ORIGINAL,"original_sha256":source,
        "candidate_path":candidate(case),"helper_name":"__fe2o3_region_0123456789abcdef",
        // Omitted below in copy: optional edit absence preserves actual program.
        "edit":null,
    });
    let preserved = json!({"registers":[10,11,12,8,9],"instructions":[
        {"kind":"binary","opcode":"xor","destination":"scratch","left":"input0","right":"input1"},
        {"kind":"binary","opcode":"and","destination":"output","left":"scratch","right":"input2"},
    ]});
    match case {
        "copy" => {
            request.as_object_mut().unwrap().remove("edit");
        }
        "preserve" => request["edit"] = preserved,
        "edit" => {
            request["edit"] = json!({"registers":[10,11,12,8,9],"instructions":[
            {"kind":"move","destination":"output","source":"input2"}]})
        }
        "stale-semantic" => {
            request["semantic_sha256"] = json!(flip(initial["semantic_identity"].as_str().unwrap()))
        }
        "stale-canonical" => {
            request["canonical_sha256"] =
                json!(flip(initial["canonical_identity"].as_str().unwrap()))
        }
        "stale-source" => request["original_sha256"] = json!(flip(source)),
        "unknown-json" => request["extra_authority"] = json!(true),
        "unknown-instruction" => {
            request["edit"] = preserved;
            request["edit"]["instructions"][0]["kind"] = json!("store");
        }
        "input-destination" => {
            request["edit"] = preserved;
            request["edit"]["instructions"][0]["destination"] = json!("input0");
        }
        "register-overlap" => {
            request["edit"] = preserved;
            request["edit"]["registers"] = json!([10, 11, 12, 8, 8]);
        }
        "missing-request" | "reused-output" | "reused-candidate" => {}
        _ => panic!("unknown action request"),
    }
    request
}
pub(super) fn read_report(path: &Path) -> Value {
    let bytes = read_bounded(&path.join("observation.json"), 16 * 1024).unwrap();
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["schema"], "fe2o3-ordered-composition-diagnostic-v1");
    assert_eq!(report["actual_rustc_callback"], true);
    assert_eq!(report["semantic_mir_version"], 32);
    assert_eq!(report["canonical_kir_version"], 17);
    assert_eq!(report["target"], "gfx942:xnack-");
    assert_eq!(report["wave_width"], 64);
    for key in [
        "ranked_checks",
        "formal_memory_admission",
        "functional_proof",
        "source_custody_exported",
        "compiler_custody_exported",
        "native_execution",
        "hardware_observed",
        "grants_artifact_or_launch_authority",
    ] {
        assert_eq!(report[key], false, "{key}");
    }
    for key in [
        "semantic_identity",
        "canonical_identity",
        "source_inventory_identity",
        "source_preflight_identity",
    ] {
        let s = report[key].as_str().unwrap();
        assert_eq!(s.len(), 64);
        assert!(
            s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_ne!(s, "0".repeat(64));
    }
    report
}
pub(super) fn diagnostic_files(path: &Path) -> BTreeMap<String, String> {
    let mut pins = BTreeMap::new();
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        let cap = match name.as_str() {
            "observation.json" => 16 * 1024,
            "canonical-v17.bin" | "canonical.ll" => 256 * 1024,
            _ => panic!("unexpected diagnostic output"),
        };
        assert!(
            pins.insert(name, digest(&read_bounded(&entry.path(), cap).unwrap()))
                .is_none()
        );
    }
    assert!(pins.contains_key("canonical-v17.bin") && pins.contains_key("canonical.ll"));
    pins
}
pub(super) fn unchanged_original_owner(
    output: &Path,
    initial_path: &Path,
    initial: &Value,
    full: bool,
) {
    let pins = diagnostic_files(output);
    let previous = diagnostic_files(initial_path);
    assert_eq!(pins.len(), if full { 3 } else { 2 });
    for name in ["canonical-v17.bin", "canonical.ll"] {
        assert_eq!(pins[name], previous[name]);
    }
    if full {
        let report = read_report(output);
        for key in [
            "semantic_identity",
            "canonical_identity",
            "source_inventory_identity",
            "source_preflight_identity",
            "static_region_definitions",
            "definitions",
            "helper_definitions",
            "helper_calls",
            "root_qualified_region_occurrences",
        ] {
            assert_eq!(report[key], initial[key], "{key}");
        }
    } else {
        assert!(!output.join("observation.json").exists());
    }
}
pub(super) fn published(
    case: &str,
    root: &Path,
    output: &Path,
    initial: &Value,
    request_bytes: &[u8],
) -> Value {
    unchanged_original_owner(output, &root.join("diagnostic-initial"), initial, true);
    let report = read_report(output);
    let row = &report["source_promotion"];
    assert_eq!(row["request_sha256"], digest(request_bytes));
    let source = read_bounded(&root.join(ORIGINAL), 64 * 1024).unwrap();
    let file = root.join(candidate(case));
    let bytes = read_bounded(&file, 72 * 1024).unwrap();
    let metadata = fs::symlink_metadata(&file).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert_eq!(row["original_sha256"], digest(&source));
    assert_eq!(row["candidate_sha256"], digest(&bytes));
    assert_eq!(row["original_bytes"], source.len());
    assert_eq!(row["candidate_bytes"], bytes.len());
    assert_eq!(row["candidate_device"], metadata.dev());
    assert_eq!(row["candidate_inode"], metadata.ino());
    assert_eq!(row["program_edited"], case == "edit");
    if case == "preserve" {
        assert_eq!(
            bytes,
            read_bounded(&root.join(candidate("copy")), 72 * 1024).unwrap(),
            "an exact typed preserving request renders the same candidate bytes"
        );
    }
    for key in ["created_new", "fresh_compilation_required"] {
        assert_eq!(row[key], true);
    }
    for key in [
        "original_overwritten",
        "fresh_compilation_observed",
        "source_custody_exported",
        "grants_artifact_or_launch_authority",
    ] {
        assert_eq!(row[key], false);
    }
    report
}
#[test]
fn public_request_mutations_preserve_closed_stage_distinctions() {
    let initial = json!({"definitions":[{"root_region":true,"definition_ordinal":0}],
        "semantic_identity":"1".repeat(64),"canonical_identity":"2".repeat(64)});
    let copy = request("copy", &initial, &"3".repeat(64));
    assert!(copy.get("edit").is_none());
    assert_eq!(
        request("preserve", &initial, &"3".repeat(64))["edit"]["instructions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        request("edit", &initial, &"3".repeat(64))["edit"]["instructions"][0]["source"],
        "input2"
    );
    for case in ["stale-semantic", "stale-canonical", "stale-source"] {
        assert!(!pre_frontend(case));
        assert!(!"generic compiler failed".contains(refusal(case)));
    }
    for case in [
        "unknown-json",
        "unknown-instruction",
        "input-destination",
        "register-overlap",
        "missing-request",
    ] {
        assert!(pre_frontend(case));
    }
}
