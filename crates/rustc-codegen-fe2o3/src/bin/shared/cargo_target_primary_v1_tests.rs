
fn primary_package_selection() -> (Files, Selection, fe2o3_rustc_invocation::PortablePackageIdentityV1) {
    let files = Files::new();
    std::fs::write(files.0.join("Cargo.toml"), b"[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n").unwrap();
    let mut selection = files.selection();
    selection.bind_primary_metadata(&primary_metadata(&files, "different_library", "lib.rs")).unwrap();
    let package = selection.primary.as_ref().unwrap();
    let identity = fe2o3_rustc_invocation::PortablePackageIdentityV1::new(
        &package.name, &package.version, package.manifest_sha256,
    ).unwrap();
    (files, selection, identity)
}

fn primary_metadata(files: &Files, library: &str, source: &str) -> serde_json::Value {
    serde_json::json!({"packages":[{
        "name":"fixture", "version":"0.1.0", "manifest_path":files.0.join("Cargo.toml"),
        "targets":[
            {"kind":["bin"], "crate_types":["bin"], "name":"shared-name", "src_path":files.0.join("main.rs")},
            {"kind":["lib"], "crate_types":["lib"], "name":library, "src_path":files.0.join(source)}
        ]
    }]})
}

#[test]
fn primary_package_binding_matches_actual_sibling_kind_source_and_package_not_crate_collision() {
    let (files, mut selection, identity) = primary_package_selection();
    for (name, source) in [("different_library", "lib.rs"), ("shared_name", "lib.rs"), ("shared_name", "main.rs")] {
        selection.bind_primary_metadata(&primary_metadata(&files, name, source)).unwrap();
        let mut args = files.args(source, "lib");
        args[2] = name.into();
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&args).unwrap() else { panic!() };
        assert!(!selection.matches(compile).unwrap());
        assert!(selection.binding_only_matches(
            compile, Some(OsStr::new("1")), &files.0.join("Cargo.toml"), &identity,
        ).unwrap());
        let encoded = selection.encode().unwrap();
        assert_eq!(Selection::decode(&encoded, "shared_name").unwrap(), selection);
    }
    let args = files.args("main.rs", "bin");
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&args).unwrap() else { panic!() };
    assert!(selection.matches(compile).unwrap());
    assert!(!selection.binding_only_matches(
        compile, Some(OsStr::new("1")), &files.0.join("Cargo.toml"), &identity,
    ).unwrap());
}

#[test]
fn primary_package_binding_refuses_foreign_custody_and_preserves_dependency_passthrough() {
    let (files, selection, identity) = primary_package_selection();
    let mut args = files.args("lib.rs", "lib");
    args[2] = "different_library".into();
    let check = |args: &[OsString], marker: Option<&OsStr>, manifest: &std::path::Path, identity: &fe2o3_rustc_invocation::PortablePackageIdentityV1| {
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(args).unwrap() else { panic!() };
        selection.binding_only_matches(compile, marker, manifest, identity)
    };
    let manifest = files.0.join("Cargo.toml");
    for marker in [None, Some(OsStr::new("0")), Some(OsStr::new("true"))] {
        assert!(!check(&args, marker, &manifest, &identity).unwrap());
    }
    std::fs::write(files.0.join("foreign.toml"), b"[package]\nname=\"foreign\"\n").unwrap();
    assert!(!check(&args, Some(OsStr::new("1")), &files.0.join("foreign.toml"), &identity).unwrap());
    for changed in [
        fe2o3_rustc_invocation::PortablePackageIdentityV1::new("foreign", identity.package_version(), *identity.manifest_sha256()).unwrap(),
        fe2o3_rustc_invocation::PortablePackageIdentityV1::new(identity.package_name(), "9.0.0", *identity.manifest_sha256()).unwrap(),
        fe2o3_rustc_invocation::PortablePackageIdentityV1::new(identity.package_name(), identity.package_version(), [0;32]).unwrap(),
    ] { assert!(check(&args, Some(OsStr::new("1")), &manifest, &changed).is_err()); }
    for fault in 0..4 {
        let mut changed = args.clone();
        match fault {
            0 => changed[2] = "wrong_library".into(),
            1 => changed[4] = "bin".into(),
            2 => changed[5] = files.0.join("main.rs").into_os_string(),
            3 => changed.push("--test".into()),
            _ => unreachable!(),
        }
        assert!(!check(&changed, Some(OsStr::new("1")), &manifest, &identity).unwrap());
    }
    std::fs::write(&manifest, b"[package]\nname=\"changed\"\n").unwrap();
    assert!(check(&args, Some(OsStr::new("1")), &manifest, &identity).is_err());
}

#[test]
fn primary_package_metadata_requires_unique_exact_bin_and_bounded_manifest_selection() {
    let (files, _, _) = primary_package_selection();
    let original = primary_metadata(&files, "different_library", "lib.rs");
    for fault in 0..4 {
        let mut changed = original.clone();
        match fault {
            0 => changed["packages"][0]["targets"][0]["kind"] = serde_json::json!(["lib"]),
            1 => changed["packages"][0]["targets"][0]["src_path"] = serde_json::json!(files.0.join("lib.rs")),
            2 => changed["packages"][0]["targets"][0]["name"] = "wrong-bin".into(),
            3 => {
                let row = changed["packages"][0].clone();
                changed["packages"].as_array_mut().unwrap().push(row);
            },
            _ => unreachable!(),
        }
        assert!(files.selection().bind_primary_metadata(&changed).is_err(), "{fault}");
    }
    assert_eq!(cargo_manifest_argument(&[]).unwrap(), None);
    assert_eq!(cargo_manifest_argument(&["--manifest-path".into(), "Cargo.toml".into()]).unwrap(), Some("Cargo.toml".into()));
    assert_eq!(cargo_manifest_argument(&["--manifest-path=Cargo.toml".into()]).unwrap(), Some("Cargo.toml".into()));
    for args in [
        vec!["--manifest-path".into()],
        vec!["--manifest-path=".into()],
        vec!["--manifest-path=a".into(), "--manifest-path".into(), "b".into()],
    ] { assert!(cargo_manifest_argument(&args).is_err()); }
}
