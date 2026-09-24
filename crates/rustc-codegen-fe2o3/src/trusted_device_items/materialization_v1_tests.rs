const CARGO_VENDOR_DEVICE_MANIFEST_V1: &[u8] =
    include_bytes!("fixtures/fe2o3-device-cargo-vendor-v1.toml");

#[test]
fn cargo_vendor_fixture_matches_actual_sdk_cargo_test_target_roster() {
    let package_root = Path::new(super::REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT)
        .canonicalize()
        .unwrap();
    // Auto-discovered targets can change the vendor manifest without changing Cargo.toml.
    let output =
        std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version=1",
            ])
            .arg("--manifest-path")
            .arg(package_root.join("Cargo.toml"))
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "Cargo target discovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "fe2o3-device")
        .unwrap();
    assert_eq!(
        Path::new(package["manifest_path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        package_root.join("Cargo.toml")
    );
    let mut stanzas = Vec::new();
    for target in package["targets"].as_array().unwrap() {
        if !target["kind"]
            .as_array()
            .unwrap()
            .iter()
            .any(|kind| kind == "test")
        {
            continue;
        }
        let path = Path::new(target["src_path"].as_str().unwrap())
            .strip_prefix(&package_root)
            .unwrap()
            .to_str()
            .unwrap();
        stanzas.push(format!(
            "[[test]]\nname = {}\npath = {}\n",
            serde_json::to_string(target["name"].as_str().unwrap()).unwrap(),
            serde_json::to_string(path).unwrap()
        ));
    }
    assert!(!stanzas.is_empty());
    let matches_roster = |manifest: &str| {
        manifest.lines().filter(|line| *line == "[[test]]").count() == stanzas.len()
            && stanzas
                .iter()
                .all(|stanza| manifest.matches(stanza.as_str()).count() == 1)
    };
    let manifest = std::str::from_utf8(CARGO_VENDOR_DEVICE_MANIFEST_V1).unwrap();
    assert!(
        matches_roster(manifest),
        "review the exact Cargo-vendored manifest and source closure after SDK target changes"
    );
    for stanza in &stanzas {
        assert!(!matches_roster(&manifest.replacen(stanza.as_str(), "", 1)));
    }
    assert!(!matches_roster(&format!("{manifest}\n{}", stanzas[0])));
}

fn reviewed_materialization_fixture(vendored: bool) -> ProviderPackageFixture {
    let fixture = ProviderPackageFixture::new();
    fs::remove_dir_all(fixture.source_root()).unwrap();
    let original = Path::new(super::REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT);
    let mut files = Vec::new();
    super::collect_reviewed_source_files(&original.join("src"), &mut files).unwrap();
    assert_eq!(files.len(), 32);
    for file in files {
        let target = fixture.root.join(file.strip_prefix(original).unwrap());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(file, target).unwrap();
    }
    let manifest = if vendored {
        CARGO_VENDOR_DEVICE_MANIFEST_V1.to_vec()
    } else {
        fs::read(original.join("Cargo.toml")).unwrap()
    };
    fs::write(fixture.root.join("Cargo.toml"), manifest).unwrap();
    fixture
}

fn admit_reviewed_materialization(
    fixture: &ProviderPackageFixture,
) -> Result<super::ReviewedProviderSourceClosureV1, String> {
    reviewed_provider_source_closure_from_definition(
        &fixture.definition(),
        WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
        &super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURES_V1,
    )
}

#[test]
fn canonical_and_cargo_vendor_materializations_preserve_actual_identities() {
    use sha2::{Digest as _, Sha256};

    let manifest_sha: [u8; 32] = Sha256::digest(CARGO_VENDOR_DEVICE_MANIFEST_V1).into();
    assert_eq!(
        manifest_sha,
        digest("a5505445b6b63f1e46b7fca58aa25450de19f3cec1c8d44ce444e64d441a22b4")
    );
    let item = TrustedDeviceItem::WriteOnlyDisjointSliceLen;
    let path = exact_provider_compiler_definition_path_v1(item)
        .unwrap()
        .strip_prefix("fe2o3_device::")
        .unwrap();
    let mut definitions = Vec::new();
    for (vendored, expected) in [
        (false, super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1),
        (
            true,
            super::REVIEWED_SAFE_EXECUTION_CARGO_VENDOR_SOURCE_CLOSURE_V1,
        ),
    ] {
        let fixture = reviewed_materialization_fixture(vendored);
        let admitted = admit_reviewed_materialization(&fixture).unwrap();
        assert_eq!(admitted.identity, expected);
        let other = if vendored {
            super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1
        } else {
            super::REVIEWED_SAFE_EXECUTION_CARGO_VENDOR_SOURCE_CLOSURE_V1
        };
        assert!(
            reviewed_provider_source_closure_from_definition(
                &fixture.definition(),
                WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
                &[other],
            )
            .is_err()
        );
        assert_eq!(
            admitted.source_root,
            fs::canonicalize(fixture.source_root()).unwrap()
        );
        assert!(!fixture.root.join("Cargo.toml.orig").exists());
        let definition = semantic_definition(path, admitted.identity, [6; 32]);
        validate_reviewed_fe2o3_device_provider_definition_v1(item, &definition).unwrap();
        let mut changed = definition.clone();
        changed.source_closure_identity[0] ^= 1;
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &changed).is_err());
        changed = definition.clone();
        changed.provider.crate_name = "impostor".into();
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &changed).is_err());
        let wrong_path = semantic_definition("wrong::len", admitted.identity, [6; 32]);
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &wrong_path).is_err());
        definitions.push(definition);
    }
    assert_ne!(
        definitions[0].source_closure_identity,
        definitions[1].source_closure_identity
    );
    assert_ne!(
        definitions[0]
            .durable_semantic_identity(item.canonical_path())
            .unwrap(),
        definitions[1]
            .durable_semantic_identity(item.canonical_path())
            .unwrap()
    );
}

#[test]
fn reviewed_materializations_reject_manifest_and_source_mutations() {
    for vendored in [false, true] {
        for mutation in 0..12 {
            let fixture = reviewed_materialization_fixture(vendored);
            admit_reviewed_materialization(&fixture).unwrap();
            match mutation {
                0 => {
                    let path = fixture.root.join("Cargo.toml");
                    let mut bytes = fs::read(&path).unwrap();
                    bytes.push(b'\n');
                    fs::write(path, bytes).unwrap();
                }
                1 => {
                    let path = fixture.root.join("Cargo.toml");
                    let manifest = fs::read_to_string(&path).unwrap();
                    fs::write(path, manifest.replace("fe2o3-macros", "unreviewed-macros")).unwrap();
                }
                2 => fs::write(fixture.definition(), b"// substituted source\n").unwrap(),
                3 => fs::write(fixture.source_root().join("extra.rs"), b"// extra\n").unwrap(),
                4 => fs::remove_file(fixture.source_root().join("thread.rs")).unwrap(),
                5 => fs::rename(
                    fixture.source_root().join("thread.rs"),
                    fixture.source_root().join("renamed.rs"),
                )
                .unwrap(),
                6 => fs::write(fixture.root.join("build.rs"), b"fn main() {}\n").unwrap(),
                7 => {
                    let path = fixture.root.join("Cargo.toml");
                    let manifest = fs::read_to_string(&path).unwrap();
                    fs::write(
                        path,
                        manifest.replace("name = \"fe2o3_device\"", "name = \"unreviewed\""),
                    )
                    .unwrap();
                }
                8 => {
                    let path = fixture.root.join("Cargo.toml");
                    fs::copy(&path, fixture.root.join("Cargo.toml.orig")).unwrap();
                    fs::write(path, b"[package]\nname = 'unreviewed'\n").unwrap();
                }
                9 => fs::write(
                    fixture.source_root().join("physical_entry_v1.rs"),
                    b"// substituted physical provider\n",
                )
                .unwrap(),
                10 => fs::remove_file(fixture.source_root().join("physical_entry_v1.rs")).unwrap(),
                11 => fs::rename(
                    fixture.source_root().join("physical_entry_v1.rs"),
                    fixture.source_root().join("physical_entry_renamed.rs"),
                )
                .unwrap(),
                _ => unreachable!(),
            }
            assert!(
                admit_reviewed_materialization(&fixture).is_err(),
                "accepted mutation {mutation} for vendored={vendored}"
            );
            let identity = reviewed_provider_source_closure_identity(
                &fixture.root,
                WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
            )
            .unwrap();
            let definition = semantic_definition("wave::{impl#4}::current", identity, [6; 32]);
            assert!(super::validate_safe_execution_provider_definition_v1(&definition).is_err());
        }
    }
}

#[test]
fn physical_entry_provider_uses_the_exact_reviewed_materialization() {
    for vendored in [false, true] {
        let fixture = reviewed_materialization_fixture(vendored);
        let admitted = reviewed_provider_source_closure_from_definition(
            &fixture.source_root().join("physical_entry_v1.rs"),
            WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
            &super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURES_V1,
        )
        .unwrap();
        assert_eq!(
            admitted.identity,
            admit_reviewed_materialization(&fixture).unwrap().identity
        );
        for item in [
            TrustedDeviceItem::AmdGpuPhysicalEntryBeginGfx942,
            TrustedDeviceItem::AmdGpuPhysicalEntryLabelGfx942,
            TrustedDeviceItem::AmdGpuPhysicalEntryStepGfx942,
        ] {
            let path = exact_provider_compiler_definition_path_v1(item)
                .unwrap()
                .strip_prefix("fe2o3_device::")
                .unwrap();
            assert!(path.starts_with("physical_entry_v1::"));
            let definition = semantic_definition(path, admitted.identity, [6; 32]);
            validate_reviewed_fe2o3_device_provider_definition_v1(item, &definition).unwrap();
        }
    }
}

#[test]
fn physical_entry_refresh_preserves_the_exact_diagnostics_leaf() {
    use sha2::{Digest as _, Sha256};
    let source =
        fs::read(Path::new(super::REVIEWED_FE2O3_DEVICE_SOURCE_ROOT).join("diagnostics.rs"))
            .unwrap();
    assert_eq!(source.len(), 7490);
    let actual: [u8; 32] = Sha256::digest(source).into();
    assert_eq!(
        actual,
        digest("803b2178d789c18875b8d3a816af355abfd6a3f8f29cbe7b3262c1ba6e75111d")
    );
}

#[test]
fn materialization_policy_rejects_empty_or_zero_identities() {
    let fixture = reviewed_materialization_fixture(true);
    for policy in [
        &[][..],
        &[[0; 32]][..],
        &[super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1, [0; 32]][..],
    ] {
        assert!(
            reviewed_provider_source_closure_from_definition(
                &fixture.definition(),
                WORKGROUP_SYNC_PROVIDER_SOURCE_CLOSURE_DOMAIN_V1,
                policy,
            )
            .is_err()
        );
    }
}
