use std::io::Write;
use std::process::{Command, Output, Stdio};

const BUNDLE: &[u8] = include_bytes!("../tutorial/fill-v6/fill-v6.fe2sim");
const SNAPSHOT: &str = include_str!("../tutorial/fill-v6/snapshot.json");
const SELECTOR: &str = include_str!("../tutorial/fill-v6/selector.json");

fn query(arguments: &[&str], bytes: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-author"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn retained_ordinary_source_bundle_is_inspected_and_selected_without_authority() {
    let expected = json(SNAPSHOT.as_bytes());
    let output = query(&["inspect"], BUNDLE);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json(&output.stdout), expected);
    assert_eq!(expected["operation_count"], 8);
    assert_eq!(expected["authority"]["grants_production_resume"], false);
    let identity = expected["bundle_identity"].as_str().unwrap();
    let page = query(
        &[
            "operations",
            "--bundle-identity",
            identity,
            "--start",
            "0",
            "--limit",
            "64",
        ],
        BUNDLE,
    );
    assert!(page.status.success());
    assert_eq!(
        json(&page.stdout),
        json(include_bytes!("../tutorial/fill-v6/operations.json"))
    );
    let region = query(&["select", "--selector", SELECTOR.trim()], BUNDLE);
    assert!(region.status.success());
    assert_eq!(
        json(&region.stdout),
        json(include_bytes!("../tutorial/fill-v6/region.json"))
    );
}

#[test]
fn stale_corrupt_and_unsupported_inputs_fail_without_candidate_output() {
    let mut stale = json(SELECTOR.as_bytes());
    stale["canonical_kir_digest"] = serde_json::Value::String("0".repeat(64));
    let rejected = query(&["select", "--selector", &stale.to_string()], BUNDLE);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("stale or malformed exact canonical V11 identity")
    );
    let unsupported = query(
        &[
            "materialize",
            "--selector",
            SELECTOR.trim(),
            "--helper",
            "compute",
        ],
        BUNDLE,
    );
    assert_eq!(unsupported.status.code(), Some(1));
    assert!(unsupported.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&unsupported.stderr)
            .contains("outside the diagnostic u32 gfx942 typed-ISA draft profile")
    );
    let mut corrupt = BUNDLE.to_vec();
    *corrupt.last_mut().unwrap() ^= 1;
    let rejected = query(&["inspect"], &corrupt);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("bundle rejected"));
}
