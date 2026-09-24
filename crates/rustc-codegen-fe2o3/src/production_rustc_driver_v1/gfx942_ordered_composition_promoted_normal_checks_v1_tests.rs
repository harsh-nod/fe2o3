//! Closed selectors and retained output joins; no DTO can create a compiler owner.
use super::*;
pub(super) const CLI_CASES: [&str; 13] = [
    "diagnostic",
    "copy",
    "preserve",
    "edit",
    "stale-semantic",
    "stale-canonical",
    "stale-source",
    "diagnostic-const",
    "capture-const",
    "diagnostic-local",
    "capture-local",
    "diagnostic-wrapper",
    "capture-wrapper",
];
pub(super) fn is_diagnostic(case: &str) -> bool {
    matches!(
        case,
        "diagnostic" | "diagnostic-const" | "diagnostic-local" | "diagnostic-wrapper"
    )
}
pub(super) fn is_publication(case: &str) -> bool {
    matches!(case, "copy" | "preserve" | "edit")
}
fn refusal(case: &str) -> &'static str {
    match case {
        "stale-semantic" | "stale-canonical" | "stale-source" => action::checks::refusal(case),
        "capture-const" => "publisher refuses non-marker local const items",
        "capture-local" => "publisher refuses local, constant or captured operands",
        "capture-wrapper" => "publisher refuses wrapper macros or substituted expansions",
        _ => panic!("closed source refusal"),
    }
}
pub(super) fn request(case: &str, initial: &Value, source: &str) -> Value {
    assert!(CLI_CASES.contains(&case) && !is_diagnostic(case));
    let base = if case.starts_with("capture-") {
        "copy"
    } else {
        case
    };
    let mut value = action::checks::request(base, initial, source);
    value["candidate_path"] = json!(inputs::candidate(case));
    value
}
pub(super) fn actual_cli(
    root: &Path,
    case: &str,
    build: &pins::Build,
    started: std::time::Instant,
) -> Value {
    let session = std::time::Instant::now();
    let record = inputs::invocation(root, case, "original");
    let original = inputs::snapshot(&inputs::source(root, "original"), 64 * 1024);
    let sources = ["copy", "preserve", "edit"].map(|variant| {
        let path = inputs::source(root, variant);
        (
            path.clone(),
            path.exists().then(|| inputs::snapshot(&path, 72 * 1024)),
        )
    });
    inputs::write_invocation(root, &format!("cli-{case}"), &record);
    let out = inputs::output(root, case);
    assert!(!out.exists());
    let initial = (!is_diagnostic(case))
        .then(|| action::checks::read_report(&inputs::diagnostic(root, case)));
    let request_path = root.join(format!("{case}.request.json"));
    let request_bytes = initial.as_ref().map(|initial| {
        let bytes = serde_json::to_vec(&request(case, initial, &original.sha256)).unwrap();
        publisher::create(&request_path, &bytes, 8192);
        bytes
    });
    let candidate = (!is_diagnostic(case)).then(|| root.join(inputs::candidate(case)));
    if let Some(candidate) = &candidate {
        assert!(!candidate.exists());
    }
    build.recheck();
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    let mut command = Command::new(pins::extractor());
    inputs::environment(&mut command, root, &record);
    command
        .args(&record.args)
        .env("FE2O3_EXTRACT_CRATE_V1", staging::LIB_NAME)
        .env(DIAGNOSTIC_ENV, &out)
        .env_remove(CRATE_BINDING_ID_ENV_V1)
        .env_remove(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2);
    if request_bytes.is_some() {
        command.env(REQUEST_ENV, &request_path);
    }
    let (status, stdout, stderr) =
        capture_cli_status_v1(&mut command, root, &format!("cli-{case}")).unwrap();
    // Raw terminal facts survive subsequent semantic-oracle refusal.
    publish_json(
        root,
        &format!("cli-{case}.terminal.json"),
        &json!({
            "case":case,"exit":status,"stdout_sha256":digest(&stdout),"stdout_bytes":stdout.len(),
            "stderr_sha256":digest(&stderr),"stderr_bytes":stderr.len(),
            "candidate_exists":candidate.as_ref().is_some_and(|p|p.exists()),"acceptance":false
        }),
    );
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    assert_eq!(
        status,
        Some(if is_diagnostic(case) || is_publication(case) {
            0
        } else {
            1
        })
    );
    let result = if is_diagnostic(case) {
        let observed = action::checks::read_report(&out);
        assert!(observed["source_promotion"].is_null());
        assert_eq!(action::checks::diagnostic_files(&out).len(), 3);
        json!({"stage":"actual_cli_diagnostic","report":observed})
    } else if is_publication(case) {
        let observed = action::checks::published(
            case,
            root,
            &out,
            initial.as_ref().unwrap(),
            request_bytes.as_ref().unwrap(),
        );
        let snapshot = inputs::snapshot(candidate.as_ref().unwrap(), 72 * 1024);
        let rendered = serde_json::to_value(&snapshot).unwrap();
        let row = &observed["source_promotion"];
        for (left, right) in [
            ("bytes", "candidate_bytes"),
            ("sha256", "candidate_sha256"),
            ("device", "candidate_device"),
            ("inode", "candidate_inode"),
        ] {
            assert_eq!(rendered[left], row[right]);
        }
        json!({"stage":"actual_cli_published","report":observed,"candidate":snapshot})
    } else {
        let text = std::str::from_utf8(&stderr).unwrap();
        assert!(text.contains(refusal(case)), "wrong refusal: {text}");
        assert!(text.contains("NotAttempted:"), "wrong effect: {text}");
        assert!(!candidate.as_ref().unwrap().exists());
        action::checks::unchanged_original_owner(
            &out,
            &inputs::diagnostic(root, case),
            initial.as_ref().unwrap(),
            false,
        );
        json!({"stage":"exact_cli_source_refusal","expected_fragment":refusal(case),
            "diagnostic":text,"candidate_created":false,"publication_effect":"NotAttempted"})
    };
    for (path, before) in sources {
        if path == candidate.clone().unwrap_or_default() && is_publication(case) {
            continue;
        }
        if let Some(before) = before {
            assert_eq!(inputs::snapshot(&path, 72 * 1024), before);
        } else {
            assert!(!path.exists());
        }
    }
    if let Some(bytes) = request_bytes {
        assert_eq!(read_bounded(&request_path, 8192).unwrap(), bytes);
    }
    inputs::recheck(root, case, "original", &record, &original);
    build.recheck();
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    let observed = json!({"case":case,"invocation":record,"original_source":original,"result":result,
        "actual_extractor":true,"exit":status,"source_custody_from_files":false,
        "hardware_observed":false,"protected_authority":false});
    publish_json(root, &format!("cli-{case}.accepted.json"), &observed);
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    observed
}
pub(super) fn normal_case(variant: &str, mode: &str) -> Result<String, &'static str> {
    if !["original", "copy", "preserve", "edit"].contains(&variant)
        || !["observe", "llvm", "handoff"].contains(&mode)
        || (variant == "original" && mode != "observe")
    {
        return Err("closed promoted normal case");
    }
    Ok(format!("{variant}-{mode}"))
}
pub(super) fn join(root: &Path, publications: &[Value], rows: &[Value]) -> Value {
    assert_eq!(publications.len(), 13);
    assert_eq!(rows.len(), 10);
    let baseline = &rows[0]["observation"];
    assert_eq!(rows[0]["variant"], "original");
    assert_eq!(baseline["cpu"]["cases"], 32);
    let baseline_hash = baseline["cpu"]["output_sha256"].as_str().unwrap();
    let mut joins = Vec::new();
    for variant in ["copy", "preserve", "edit"] {
        let get = |mode: &str| {
            rows.iter()
                .find(|v| v["variant"] == variant && v["mode"] == mode)
                .unwrap()
        };
        let owner = get("observe");
        let llvm = get("llvm");
        let handoff = get("handoff");
        let publication = publications.iter().find(|v| v["case"] == variant).unwrap();
        let snapshot = inputs::snapshot(&inputs::source(root, variant), 72 * 1024);
        let rendered = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(rendered, publication["result"]["candidate"]);
        for row in [owner, llvm, handoff] {
            assert_eq!(row["source"], rendered);
            assert_eq!(row["invocation"], owner["invocation"]);
            assert_eq!(row["invocation"]["leaf_sha256"], rendered["sha256"]);
        }
        let actual = &owner["observation"];
        assert_eq!(actual["cpu"]["cases"], 32);
        if variant == "edit" {
            assert_ne!(actual["cpu"]["output_sha256"], baseline_hash);
        } else {
            assert_eq!(actual["cpu"]["output_sha256"], baseline_hash);
        }
        assert_ne!(actual["canonical_sha256"], baseline["canonical_sha256"]);
        assert_eq!(actual["llvm_sha256"], llvm["observation"]["llvm_sha256"]);
        assert_eq!(
            actual["handoff_sha256"],
            handoff["observation"]["handoff_sha256"]
        );
        let dir = root.join(format!("{variant}-observe.output"));
        for (file, field) in [
            ("canonical-v17.bin", "canonical_sha256"),
            ("canonical.ll", "llvm_sha256"),
            ("handoff-v2.bin", "handoff_sha256"),
            ("descriptor-v1.bin", "descriptor_sha256"),
        ] {
            assert_eq!(
                digest(&read_bounded(&dir.join(file), 4 * 1024 * 1024).unwrap()),
                actual[field]
            );
        }
        let canonical = read_bounded(&dir.join("canonical.ll"), 4 * 1024 * 1024).unwrap();
        let worker = read_bounded(&dir.join("worker.ll"), 4 * 1024 * 1024).unwrap();
        let descriptor = fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(
            &read_bounded(&dir.join("descriptor-v1.bin"), 64 * 1024).unwrap(),
        )
        .unwrap();
        let relation = crate::kernel_ir_codegen::exact_ordered_composition_descriptor_extension_v1;
        let c = std::str::from_utf8(&canonical).unwrap();
        let w = std::str::from_utf8(&worker).unwrap();
        assert!(relation(c, w, &descriptor));
        let foreign_variant = if variant == "edit" { "copy" } else { "edit" };
        let foreign = read_bounded(
            &root.join(format!("{foreign_variant}-observe.output/canonical.ll")),
            4 * 1024 * 1024,
        )
        .unwrap();
        assert_ne!(foreign, canonical);
        assert!(!relation(
            std::str::from_utf8(&foreign).unwrap(),
            w,
            &descriptor
        ));
        joins.push(
            json!({"variant":variant,"published_source":snapshot,"fresh_checked":actual,
            "actual_normal_llvm_handoff_join":true,"cross_candidate_prefix_refused":true}),
        );
    }
    assert_eq!(
        read_bounded(&inputs::source(root, "copy"), 72 * 1024).unwrap(),
        read_bounded(&inputs::source(root, "preserve"), 72 * 1024).unwrap()
    );
    json!({"joins":joins,"cpu_cases":128,"descriptor_extension_refusals":12,
        "same_digest_fresh_owners_allowed":true,"abi_identical_descriptors_allowed":true})
}
#[test]
fn closed_matrix_has_exact_counts_and_no_old_feature_aliases() {
    assert_eq!(CLI_CASES.len(), 13);
    assert_eq!(CLI_CASES.iter().filter(|c| is_diagnostic(c)).count(), 4);
    assert_eq!(CLI_CASES.iter().filter(|c| is_publication(c)).count(), 3);
    assert_eq!(
        CLI_CASES
            .iter()
            .filter(|c| !is_diagnostic(c) && !is_publication(c))
            .count(),
        6
    );
    assert!(normal_case("original", "observe").is_ok());
    assert!(normal_case("original", "llvm").is_err());
    assert!(normal_case("ordered-composition-helper", "observe").is_err());
    assert!(normal_case("copy", "native").is_err());
}
#[test]
fn captured_operand_requests_use_their_actual_selected_owner() {
    let initial = json!({"definitions":[{"root_region":true,"definition_ordinal":0}],
        "semantic_identity":"1".repeat(64),"canonical_identity":"2".repeat(64)});
    for case in ["capture-const", "capture-local", "capture-wrapper"] {
        let request = request(case, &initial, &"3".repeat(64));
        assert_eq!(request["semantic_sha256"], initial["semantic_identity"]);
        assert_eq!(request["canonical_sha256"], initial["canonical_identity"]);
        assert_eq!(request["candidate_path"], format!("refused-{case}.rs"));
        assert!(!request.as_object().unwrap().contains_key("edit"));
        assert!(!"unrelated compiler error".contains(refusal(case)));
    }
}
