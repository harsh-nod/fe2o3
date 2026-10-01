//! Exact test-driver association; production validation must also check the closure.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
pub(super) struct ExpectedSource<'a> {
    pub lesson_id: &'a str,
    pub tab_ordinal: usize,
    pub package_manifest: &'a str,
    pub crate_name: &'a str,
    pub library_path: &'a str,
    pub source_path: &'a str,
    pub default_features: bool,
    pub features: &'a [&'a str],
    pub kernel_symbol: &'a str,
    pub target: &'a str,
    pub driver_package: &'a str,
    pub driver_target: &'a str,
    pub test_function: &'a str,
    pub orders: &'a [&'a str],
}

fn exact_keys(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    })
}

fn digest(value: &Value, length: usize) -> bool {
    value.as_str().is_some_and(|text| {
        text.len() == length
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn source_digest(source: &[u8]) -> String {
    Sha256::digest(source)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn expected_case(input: ExpectedSource<'_>) -> Value {
    json!({
        "features":input.features, "kernelSymbol":input.kernel_symbol, "target":input.target,
        "displayedFragmentOrdinal":0, "testFunction":input.test_function,
        "expectation":{
            "kind":"diagnostic-kir-export-v1", "canonicalKirVersion":18,
            "diagnosticTileOrders":input.orders, "authority":"observation_only"
        }
    })
}

pub(super) fn check(
    document: &Value,
    input: ExpectedSource<'_>,
    source: &[u8],
) -> Result<Value, &'static str> {
    if !matches!(
        input.orders,
        ["blocked"] | ["striped"] | ["blocked", "striped"]
    ) || source.is_empty()
        || source.len() > 4 * 1024 * 1024
        || std::str::from_utf8(source).is_err()
    {
        return Err("invalid independent source-driver inputs");
    }
    let curriculum = &document["curriculum"];
    if curriculum["schema"] != "fe2o3-tutorial-curriculum-obligations-v2"
        || curriculum["status"] != "pending"
    {
        return Err("expected pending V2 curriculum");
    }
    let lessons = curriculum["lessons"].as_array().ok_or("missing lessons")?;
    let mut matching = lessons
        .iter()
        .filter(|lesson| lesson["lessonId"] == input.lesson_id);
    let lesson = matching.next().ok_or("missing diagnostic source lesson")?;
    if matching.next().is_some() || lesson["role"] != "executable" {
        return Err("diagnostic source lesson identity differs");
    }
    let tabs = lesson["codeTabs"].as_array().ok_or("missing source tabs")?;
    let tab = tabs
        .get(input.tab_ordinal)
        .ok_or("missing diagnostic source tab")?;
    if tab["ordinal"] != json!(input.tab_ordinal)
        || tabs
            .iter()
            .filter(|tab| tab["ordinal"] == json!(input.tab_ordinal))
            .count()
            != 1
        || tab["kind"] != "kernel"
        || tab["language"] != "rust"
        || tab["sourcePath"] != input.source_path
        || tab["sourceDigestScope"] != "file"
        || !tab["sourceFragmentsSha256"].is_null()
        || !digest(&tab["sourceCommit"], 40)
        || tab["sourceSha256"] != source_digest(source)
        || tab["displayedSha256"] != tab["sourceSha256"]
        || tab["displayedUtf8Bytes"] != json!(source.len())
        || tab["sourceItemStatus"] != "contract-bound"
    {
        return Err("diagnostic source tab or actual bytes differ");
    }
    let item = &tab["sourceItem"];
    if !exact_keys(
        item,
        &[
            "kind",
            "compilerInput",
            "driver",
            "sourceRanges",
            "cases",
            "contractSha256",
        ],
    ) || item["kind"] != "source-driver"
        || !digest(&item["contractSha256"], 64)
        || item["sourceRanges"] != json!([{"byteOffset":0,"byteLength":source.len()}])
        || item["driver"]
            != json!({
                "package":input.driver_package, "target":input.driver_target,
                "path":format!("crates/{}/tests/{}.rs",input.driver_package,input.driver_target)
            })
    {
        return Err("diagnostic source driver or whole-file range differs");
    }
    let shared = &item["compilerInput"];
    let lock = input
        .package_manifest
        .strip_suffix("Cargo.toml")
        .ok_or("expected exact Cargo manifest path")?
        .to_owned()
        + "Cargo.lock";
    if !exact_keys(
        shared,
        &[
            "packageManifest",
            "packageManifestSha256",
            "cargoLockPath",
            "cargoLockSha256",
            "sourcePaths",
            "sourceClosureSha256",
            "cargoTarget",
            "defaultFeatures",
        ],
    ) || shared["packageManifest"] != input.package_manifest
        || shared["cargoLockPath"] != lock
        || shared["sourcePaths"] != json!([input.source_path])
        || shared["defaultFeatures"] != json!(input.default_features)
        || shared["cargoTarget"]
            != json!({
                "kind":"lib", "name":input.crate_name, "sourcePath":input.library_path
            })
        || [
            "packageManifestSha256",
            "cargoLockSha256",
            "sourceClosureSha256",
        ]
        .iter()
        .any(|key| !digest(&shared[*key], 64))
    {
        return Err("diagnostic compiler input differs from executed arguments");
    }
    let cases = item["cases"]
        .as_array()
        .ok_or("missing diagnostic source cases")?;
    let mut matching = cases
        .iter()
        .enumerate()
        .filter(|(_, case)| case["kernelSymbol"] == input.kernel_symbol);
    let (ordinal, case) = matching
        .next()
        .ok_or("missing exact diagnostic source case")?;
    if matching.next().is_some() || *case != expected_case(input) {
        return Err("diagnostic source case differs from executed arguments");
    }
    Ok(json!({
        "lessonId":input.lesson_id, "tabOrdinal":input.tab_ordinal, "caseOrdinal":ordinal,
        "sourceItemContractSha256":item["contractSha256"],
        "sourcePath":input.source_path, "sourceSha256":tab["sourceSha256"],
        "compilerInput":shared, "driver":item["driver"], "case":case,
        "authority":"observation_only", "source_authentication":false,
        "qualified":false, "hardware_observed":false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: ExpectedSource<'static> = ExpectedSource {
        lesson_id: "other-lesson",
        tab_ordinal: 0,
        package_manifest: "examples/other/Cargo.toml",
        crate_name: "other",
        library_path: "src/lib.rs",
        source_path: "examples/other/src/kernel.rs",
        default_features: false,
        features: &["selected"],
        kernel_symbol: "other_kernel",
        target: "gfx950",
        driver_package: "rustc-codegen-fe2o3",
        driver_target: "other_driver",
        test_function: "ordinary_source",
        orders: &["blocked", "striped"],
    };
    const SOURCE: &[u8] = b"#[kernel]\nfn other_kernel() {}\n";

    fn document() -> Value {
        json!({"curriculum":{
            "schema":"fe2o3-tutorial-curriculum-obligations-v2", "status":"pending",
            "lessons":[{"lessonId":INPUT.lesson_id,"role":"executable","codeTabs":[{
                "ordinal":0,"kind":"kernel","language":"rust","sourcePath":INPUT.source_path,
                "sourceCommit":"1".repeat(40),"sourceSha256":source_digest(SOURCE),
                "sourceDigestScope":"file","sourceFragmentsSha256":null,
                "displayedSha256":source_digest(SOURCE),"displayedUtf8Bytes":SOURCE.len(),
                "sourceItemStatus":"contract-bound","sourceItem":{
                    "kind":"source-driver","sourceRanges":[{"byteOffset":0,"byteLength":SOURCE.len()}],
                    "contractSha256":"2".repeat(64),"driver":{
                        "package":INPUT.driver_package,"target":INPUT.driver_target,
                        "path":"crates/rustc-codegen-fe2o3/tests/other_driver.rs"
                    },"compilerInput":{
                        "packageManifest":INPUT.package_manifest,"packageManifestSha256":"3".repeat(64),
                        "cargoLockPath":"examples/other/Cargo.lock","cargoLockSha256":"4".repeat(64),
                        "sourcePaths":[INPUT.source_path],"sourceClosureSha256":"5".repeat(64),
                        "cargoTarget":{"kind":"lib","name":INPUT.crate_name,"sourcePath":INPUT.library_path},
                        "defaultFeatures":INPUT.default_features
                    },"cases":[expected_case(INPUT)]
                }
            }]}]
        }})
    }

    fn tab(document: &mut Value) -> &mut Value {
        &mut document["curriculum"]["lessons"][0]["codeTabs"][0]
    }

    #[test]
    fn generic_source_association_retains_diagnostic_only_identity() {
        let document = document();
        let before = document.clone();
        let binding = check(&document, INPUT, SOURCE).unwrap();
        assert_eq!(binding["lessonId"], INPUT.lesson_id);
        assert_eq!(binding["case"], expected_case(INPUT));
        assert_eq!(binding["authority"], "observation_only");
        assert_eq!(binding["source_authentication"], false);
        assert_eq!(binding["qualified"], false);
        assert_eq!(binding["hardware_observed"], false);
        assert_eq!(document, before);
    }

    #[test]
    fn source_association_rejects_missing_duplicate_and_reclassified_records() {
        for mutation in 0..7 {
            let mut document = document();
            match mutation {
                0 => document["curriculum"]["status"] = json!("qualified"),
                1 => {
                    document["curriculum"]["schema"] =
                        json!("fe2o3-tutorial-curriculum-obligations-v1")
                }
                2 => tab(&mut document)["sourceItem"] = Value::Null,
                3 => tab(&mut document)["sourceItemStatus"] = json!("pending"),
                4 => {
                    let duplicate = document["curriculum"]["lessons"][0].clone();
                    document["curriculum"]["lessons"]
                        .as_array_mut()
                        .unwrap()
                        .push(duplicate);
                }
                5 => {
                    let item = &mut tab(&mut document)["sourceItem"];
                    let duplicate = item["cases"][0].clone();
                    item["cases"].as_array_mut().unwrap().push(duplicate);
                }
                6 => tab(&mut document)["sourceItem"]["cases"] = json!([]),
                _ => unreachable!(),
            }
            assert!(
                check(&document, INPUT, SOURCE).is_err(),
                "accepted identity mutation {mutation}"
            );
        }
    }

    #[test]
    fn executed_arguments_must_match_the_independent_expected_case() {
        for (key, value) in [
            ("features", json!(["other"])),
            ("kernelSymbol", json!("substitute")),
            ("target", json!("gfx942")),
            ("testFunction", json!("different_test")),
            ("displayedFragmentOrdinal", json!(1)),
            (
                "expectation",
                json!({"kind":"verified-bundle-export","bundleVersion":5}),
            ),
        ] {
            let mut document = document();
            tab(&mut document)["sourceItem"]["cases"][0][key] = value;
            assert!(
                check(&document, INPUT, SOURCE).is_err(),
                "accepted case mutation {key}"
            );
        }
        for (key, value) in [
            ("canonicalKirVersion", json!(true)),
            ("canonicalKirVersion", json!(17)),
            ("authority", json!("verified")),
            ("bundleVersion", json!(18)),
            ("diagnosticTileOrders", json!(["blocked"])),
            ("diagnosticTileOrders", json!(["striped", "blocked"])),
            ("diagnosticTileOrders", json!(["blocked", "blocked"])),
        ] {
            let mut document = document();
            tab(&mut document)["sourceItem"]["cases"][0]["expectation"][key] = value;
            assert!(
                check(&document, INPUT, SOURCE).is_err(),
                "accepted expectation mutation {key}"
            );
        }
    }

    #[test]
    fn compiler_target_and_physical_source_binding_cannot_be_substituted() {
        for (key, value) in [
            ("packageManifest", json!("examples/elsewhere/Cargo.toml")),
            ("cargoLockPath", json!("examples/elsewhere/Cargo.lock")),
            ("sourcePaths", json!(["other.rs"])),
            ("defaultFeatures", json!(true)),
            (
                "cargoTarget",
                json!({"kind":"bin","name":"other","sourcePath":"src/lib.rs"}),
            ),
            ("sourceClosureSha256", json!("stale")),
            ("features", json!(["selected"])),
        ] {
            let mut document = document();
            tab(&mut document)["sourceItem"]["compilerInput"][key] = value;
            assert!(
                check(&document, INPUT, SOURCE).is_err(),
                "accepted compiler mutation {key}"
            );
        }
        for mutation in 0..5 {
            let mut document = document();
            match mutation {
                0 => tab(&mut document)["sourcePath"] = json!("other.rs"),
                1 => tab(&mut document)["sourceSha256"] = json!("0".repeat(64)),
                2 => tab(&mut document)["sourceItem"]["driver"]["target"] = json!("another"),
                3 => tab(&mut document)["sourceItem"]["sourceRanges"][0]["byteOffset"] = json!(1),
                4 => tab(&mut document)["sourceItem"]["contractSha256"] = json!("stale"),
                _ => unreachable!(),
            }
            assert!(
                check(&document, INPUT, SOURCE).is_err(),
                "accepted physical mutation {mutation}"
            );
        }
        assert!(check(&document(), INPUT, b"changed source").is_err());
        assert!(check(&document(), INPUT, b"\xff").is_err());
    }
}
