//! Ordinary source variants; these are task-owned inputs, never graph fixtures.
use super::*;

pub(super) const CASES: [&str; 10] = [
    "positive",
    "renamed",
    "ambiguous",
    "local-alias",
    "changed-operator",
    "duplicate-input",
    "wrong-target",
    "wrong-launch",
    "normalized",
    "stale-source",
];
const SOURCE: &str = r#"#![no_std]
use fe2o3_device::{DisjointSlice, kernel, thread};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn choose_order(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32, d: u32) {
    let result = (a ^ b) & (c | d);
    let index = thread::index_1d();
    if let Some(slot) = output.get_mut(index) {
        *slot = result;
    }
}
"#;

pub(super) fn source(case: &str) -> String {
    assert!(CASES.contains(&case));
    let result = match case {
        "renamed" => SOURCE
            .replace("a: u32", "renamed_a: u32")
            .replace(
                "    let result",
                "    // λ: renamed binding and moved source coordinates\n    let result",
            )
            .replace("(a ^ b)", "(renamed_a ^ b)"),
        "ambiguous" => SOURCE
            .replace(
                "    let index",
                "    let second = (a ^ b) & (c | d);\n    let index",
            )
            .replace("*slot = result;", "*slot = result ^ second;"),
        "local-alias" => SOURCE
            .replace("    let result", "    let alias = a;\n    let result")
            .replace("(a ^ b)", "(alias ^ b)"),
        "changed-operator" => SOURCE.replace("(a ^ b) & (c | d)", "(a ^ b) | (c | d)"),
        "duplicate-input" => SOURCE.replace("(c | d)", "(c | c)"),
        "wrong-launch" => SOURCE.replace(
            "required = [64, 1, 1], max = [64, 1, 1]",
            "required = [128, 1, 1], max = [128, 1, 1]",
        ),
        "normalized" => format!("\u{feff}{SOURCE}"),
        _ => SOURCE.to_owned(),
    };
    assert!(result.len() <= 64 * 1024);
    result
}

pub(super) fn refusal(case: &str) -> Option<&'static str> {
    match case {
        "positive" | "renamed" => None,
        "ambiguous" => Some("local-order ambiguous source initializers"),
        "local-alias" => Some("local-order local alias is not a parameter"),
        "changed-operator" => Some("local-order exact source initializer unavailable"),
        "duplicate-input" => Some("local-order requires four distinct parameter bindings"),
        "wrong-target" => Some("source-candidate requires authenticated gfx942 xnack-off wave64"),
        "wrong-launch" => Some("source-candidate requires required and maximum 64x1x1 bounds"),
        "normalized" => Some("source-boundary normalization changes original offsets"),
        "stale-source" => Some("source-candidate retained bytes differ from parsed compiler input"),
        _ => panic!("closed local-order fixture roster"),
    }
}

pub(super) fn relative(directory: &Path, case: &str) -> PathBuf {
    assert!(CASES.contains(&case));
    super::super::paths::relative_root(directory)
        .join(case)
        .join("source.rs")
}

pub(super) fn absolute(directory: &Path, case: &str) -> PathBuf {
    let path = repository().join(relative(directory, case));
    let parent = path.parent().unwrap();
    assert!(fs::symlink_metadata(parent).unwrap().file_type().is_dir());
    assert_eq!(parent.canonicalize().unwrap(), parent);
    super::super::paths::checked_source_file(&path);
    assert!(fs::metadata(&path).unwrap().len() <= 64 * 1024);
    path
}

#[test]
fn source_local_order_fixture_negatives_reach_distinct_named_boundaries() {
    assert_eq!(refusal("positive"), None);
    assert_eq!(refusal("renamed"), None);
    assert!(source("wrong-launch").contains("required = [128, 1, 1], max = [128, 1, 1]"));
    assert!(source("renamed").contains("(renamed_a ^ b) & (c | d)"));
    assert!(source("ambiguous").matches("(a ^ b) & (c | d)").count() == 2);
    assert!(source("normalized").starts_with('\u{feff}'));
    assert_eq!(
        CASES.iter().filter(|case| refusal(case).is_some()).count(),
        8
    );
}
