const CARGO_VENDOR_DEVICE_MANIFEST_V1: &[u8] =
    include_bytes!("fixtures/fe2o3-device-cargo-vendor-v1.toml");

fn reviewed_materialization_fixture(vendored: bool) -> ProviderPackageFixture {
    let fixture = ProviderPackageFixture::new();
    fs::remove_dir_all(fixture.source_root()).unwrap();
    let original = Path::new(super::REVIEWED_FE2O3_DEVICE_PACKAGE_ROOT);
    let mut files = Vec::new();
    super::collect_reviewed_source_files(&original.join("src"), &mut files).unwrap();
    assert_eq!(files.len(), 26);
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
        digest("8ffc8a52272ff0866b3f68d78d0365f50af2da1be2e305891175539d5cde65b3")
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
        for mutation in 0..9 {
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
