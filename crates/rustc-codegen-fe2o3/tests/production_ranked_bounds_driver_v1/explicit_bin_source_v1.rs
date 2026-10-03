// Exact Cargo target selection only. Source export and simulation are separate
// required gates; these helpers grant no native or executable authority.
fn check_explicit_bin_declaration(
    source: SimulationExportPackageV1<'_>,
    manifest: &Path,
    bound_source: &Path,
    feature: &str,
) -> Result<Option<Vec<String>>, String> {
    if matches!(source.target, SimulationExportTargetV1::Library { .. }) { return Ok(None); }
    let file = std::fs::File::open(manifest).map_err(|error| format!("open bin manifest: {error}"))?;
    let bytes = tutorial_source_read_bounded(file, 1024 * 1024)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "bin manifest must be UTF-8")?;
    let document: toml::Value = toml::from_str(text)
        .map_err(|error| format!("parse bin manifest: {error}"))?;
    check_explicit_bin_manifest_value(&document, source, manifest, bound_source, feature, |path| {
        let path = std::fs::canonicalize(path).map_err(|error| format!("canonicalize bin root: {error}"))?;
        if !path.is_file() { return Err("bin root must be a regular file".into()); }
        Ok(path)
    }).map(Some)
}

fn check_explicit_bin_manifest_value(
    document: &toml::Value,
    source: SimulationExportPackageV1<'_>,
    manifest: &Path,
    bound_source: &Path,
    feature: &str,
    mut canonicalize: impl FnMut(&Path) -> Result<PathBuf, String>,
) -> Result<Vec<String>, String> {
    let SimulationExportTargetV1::ExplicitBin { name, source_path } = source.target
    else { return Err("explicit bin declaration checker requires a bin".into()); };
    let package = document.get("package").and_then(toml::Value::as_table)
        .ok_or("missing bin package")?;
    if package.get("name").and_then(toml::Value::as_str) != Some(source.package)
        || name.replace('-', "_") != source.rustc_crate
    { return Err("bin package or rustc crate identity differs".into()); }
    if package.get("autobins").is_some_and(|value| value.as_bool().is_none()) {
        return Err("invalid autobins policy".into());
    }
    let bins = document.get("bin").and_then(toml::Value::as_array)
        .ok_or("bin must have an explicit [[bin]] declaration")?;
    if bins.is_empty() || bins.len() > 256 { return Err("invalid bin declaration count".into()); }
    let parent = manifest.parent().ok_or("manifest has no parent")?;
    let mut names = std::collections::BTreeSet::new();
    let mut selected = None;
    for bin in bins {
        let bin = bin.as_table().ok_or("bin declaration must be a table")?;
        let target_name = bin.get("name").and_then(toml::Value::as_str)
            .filter(|name| valid_bin_identifier(name)).ok_or("bin name must be explicit and bounded")?;
        let path = bin.get("path").and_then(toml::Value::as_str)
            .filter(|path| !path.is_empty() && path.len() <= 4096).ok_or("bin path must be explicit")?;
        if !names.insert(target_name) { return Err("duplicate explicit bin name".into()); }
        let path_value = Path::new(path);
        if path_value.is_absolute() || path_value.components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        { return Err("bin path must be a normalized package-relative path".into()); }
        if target_name != name { continue; }
        if path != source_path || canonicalize(&parent.join(path))? != bound_source {
            return Err("explicit bin source path differs".into());
        }
        selected = Some(bin);
    }
    let selected = selected.ok_or("selected explicit bin is absent")?;
    let mut required = Vec::new();
    if let Some(value) = selected.get("required-features") {
        let values = value.as_array().ok_or("required-features must be an array")?;
        if values.len() > 4096 { return Err("too many required features".into()); }
        for value in values {
            let value = value.as_str().filter(|value| valid_bin_identifier(value))
                .ok_or("unsupported required feature")?;
            if required.iter().any(|prior| prior == value) { return Err("duplicate required feature".into()); }
            required.push(value.to_owned());
        }
    }
    let table = document.get("features").and_then(toml::Value::as_table)
        .ok_or("explicit bin source selection requires declared local features")?;
    if table.len() > 4096 { return Err("too many local features".into()); }
    let mut queue = vec![feature.to_owned()];
    if source.default_features && table.contains_key("default") { queue.push("default".into()); }
    let mut enabled = std::collections::BTreeSet::new();
    while let Some(feature) = queue.pop() {
        if !valid_bin_identifier(&feature) { return Err("unsupported feature selection".into()); }
        if !enabled.insert(feature.clone()) { continue; }
        if enabled.len() > 4096 { return Err("feature closure exceeds bound".into()); }
        let dependencies = table.get(&feature).and_then(toml::Value::as_array)
            .ok_or("selected feature is not an explicit local feature")?;
        if dependencies.len() > 4096 { return Err("feature edge count exceeds bound".into()); }
        for dependency in dependencies {
            let dependency = dependency.as_str().filter(|value| valid_bin_identifier(value))
                .ok_or("dependency/weak-feature inference is unsupported for explicit bin selection")?;
            if !table.contains_key(dependency) { return Err("feature dependency is not local".into()); }
            if !enabled.contains(dependency) {
                if queue.len() >= 4096 { return Err("feature worklist exceeds bound".into()); }
                queue.push(dependency.to_owned());
            }
        }
    }
    if required.iter().any(|feature| !enabled.contains(feature)) {
        return Err("selected feature closure omits an explicit bin required-feature".into());
    }
    Ok(required)
}

fn valid_bin_identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn check_explicit_bin_metadata_target(
    targets: &[Value], name: &str, source: &Path,
    mut canonicalize: impl FnMut(&Path) -> Result<PathBuf, String>,
) -> Result<(), String> {
    let mut found = false;
    for target in targets {
        let kinds = target["kind"].as_array().ok_or("invalid target kinds")?;
        if kinds.is_empty() || kinds.iter().any(|kind| kind.as_str().is_none()) {
            return Err("invalid target kind".into());
        }
        let target_name = target["name"].as_str().filter(|name| !name.is_empty())
            .ok_or("invalid target name")?;
        let path = Path::new(target["src_path"].as_str().ok_or("missing target source")?);
        if !path.is_absolute() { return Err("target source must be absolute".into()); }
        if target_name != name || !kinds.iter().any(|kind| kind == "bin") { continue; }
        if std::mem::replace(&mut found, true) || kinds.len() != 1
            || canonicalize(path)? != source
        { return Err("explicit bin metadata identity differs or is duplicated".into()); }
    }
    if !found { return Err("explicit bin metadata target is absent".into()); }
    Ok(())
}

fn check_explicit_bin_metadata_features(
    metadata: &Value, source: SimulationExportPackageV1<'_>, required: &[String],
) -> Result<(), String> {
    let target_name = source.target.name(source.rustc_crate);
    let packages = metadata["packages"].as_array().ok_or("missing metadata packages")?;
    let package = packages.iter().find(|row| row["name"] == source.package)
        .ok_or("selected package disappeared")?;
    let targets = package["targets"].as_array().ok_or("missing metadata targets")?;
    let target = targets.iter().find(|row| row["name"] == target_name && row["kind"] == json!(["bin"]))
        .ok_or("selected bin disappeared")?;
    let actual = match target.get("required-features") {
        None => Vec::new(),
        Some(value) => value.as_array().ok_or("invalid metadata required-features")?
            .iter().map(|value| value.as_str().map(str::to_owned).ok_or("invalid metadata feature"))
            .collect::<Result<Vec<_>, _>>()?,
    };
    if actual != required { return Err("Cargo metadata required-features differ from explicit declaration".into()); }
    Ok(())
}

const EXPLICIT_BIN_SOURCE_V1: TutorialSourceInputV1<'static> = TutorialSourceInputV1 {
    lesson_id: "unassociated-source-selector-regression",
    tab_ordinal: 0,
    source_path: "crates/rustc-codegen-fe2o3/tests/fixtures/production-explicit-bin-device/src/explicit_bin_export.rs",
    export: SimulationExportPackageV1 {
        target: SimulationExportTargetV1::ExplicitBin {
            name: "fe2o3-production-explicit-bin-fixture",
            source_path: "src/explicit_bin_export.rs",
        },
        manifest_path: "crates/rustc-codegen-fe2o3/tests/fixtures/production-explicit-bin-device/Cargo.toml",
        package: "fe2o3-production-explicit-bin-fixture",
        rustc_crate: "fe2o3_production_explicit_bin_fixture",
        default_features: false,
    },
};

#[test]
fn explicit_bin_driver_command_keeps_lib_default_and_exact_source_selector() {
    let source = EXPLICIT_BIN_SOURCE_V1.export;
    let command = simulation_export_command_for_package_with_exporter(
        Path::new("unused-exporter"), source, "gfx942", Path::new("result.fe2sim"),
        Path::new("target"), Some(1), "explicit_bin_export",
    );
    let args = command.get_args().collect::<Vec<_>>();
    let root = workspace().join(source.manifest_path).parent().unwrap()
        .join(source.target.source_path());
    let source_index = args.iter().position(|arg| *arg == "--bin-source").unwrap();
    assert_eq!(args[source_index + 1], root.as_os_str());
    assert_eq!(args.iter().filter(|arg| **arg == "--bin-name").count(), 1);
    assert!(!args.iter().any(|arg| *arg == "--lib" || *arg == "--bin"));
    assert!(args.iter().any(|arg| *arg == "--no-default-features"));
    let legacy = simulation_export_command_for_package_with_exporter(
        Path::new("unused-exporter"), RANKED_BOUNDS_EXPORT_PACKAGE_V1,
        "gfx942", Path::new("result.fe2sim"), Path::new("target"), Some(1), "explicit_bin_export",
    );
    assert!(legacy.get_args().any(|arg| arg == "--lib"));
    assert!(!legacy.get_args().any(|arg| arg == "--bin-name" || arg == "--bin-source"));
}

fn bin_manifest_fixture_v1() -> toml::Value {
    toml::from_str(r#"
[package]
name = "fe2o3-production-explicit-bin-fixture"
autobins = true
[features]
explicit_bin_export = ["leaf"]
leaf = []
unused = []
[[bin]]
name = "fe2o3-production-explicit-bin-fixture"
path = "src/explicit_bin_export.rs"
required-features = ["leaf"]
"#).unwrap()
}

#[test]
fn explicit_bin_driver_checks_real_manifest_declaration_and_local_feature_closure() {
    let original = bin_manifest_fixture_v1();
    let manifest = Path::new("/workspace/package/Cargo.toml");
    let root = Path::new("/workspace/package/src/explicit_bin_export.rs");
    let check = |value: &toml::Value, feature| check_explicit_bin_manifest_value(
        value, EXPLICIT_BIN_SOURCE_V1.export, manifest, root, feature, |path| Ok(path.to_owned()),
    );
    assert_eq!(check(&original, "explicit_bin_export").unwrap(), ["leaf"]);
    let mut manual_only = original.clone();
    manual_only["package"]["autobins"] = toml::Value::Boolean(false);
    assert_eq!(check(&manual_only, "explicit_bin_export").unwrap(), ["leaf"]);
    assert!(check(&original, "unused").is_err());
    for attack in 0..10 {
        let mut value = original.clone();
        match attack {
            0 => { value.as_table_mut().unwrap().remove("bin"); }
            1 => { value["bin"][0].as_table_mut().unwrap().remove("path"); }
            2 => { value["bin"][0].as_table_mut().unwrap().remove("name"); }
            3 => { value["bin"][0]["path"] = "src/lib.rs".into(); }
            4 => { value["bin"][0]["name"] = "wrong".into(); }
            5 => { let row = value["bin"][0].clone(); value["bin"].as_array_mut().unwrap().push(row); }
            6 => { value["package"]["name"] = "wrong".into(); }
            7 => { value["features"]["explicit_bin_export"] = toml::Value::Array(vec!["dep:foreign".into()]); }
            8 => { value["bin"][0]["required-features"] = toml::Value::Array(vec!["unused".into()]); }
            9 => { value["package"]["autobins"] = "true".into(); }
            _ => unreachable!(),
        }
        assert!(check(&value, "explicit_bin_export").is_err(), "attack {attack}");
    }
}

#[test]
fn explicit_bin_metadata_allows_same_name_library_but_refuses_target_substitution() {
    let source = EXPLICIT_BIN_SOURCE_V1;
    let manifest = Path::new("/workspace/package/Cargo.toml");
    let root = Path::new("/workspace/package/src/explicit_bin_export.rs");
    let bin = json!({"kind":["bin"], "name":source.export.target.name(source.export.rustc_crate),
        "src_path":root, "required-features":["explicit_bin_export"]});
    let original = json!({"version":1,"packages":[{"name":source.export.package,"manifest_path":manifest,
        "targets":[{"kind":["lib"],"name":source.export.rustc_crate,"src_path":root},bin]}]});
    let check = |metadata: &Value| check_tutorial_source_package_metadata(
        metadata, source, manifest, root, |path| Ok(path.to_owned()),
    );
    check(&original).unwrap();
    check_explicit_bin_metadata_features(&original, source.export, &["explicit_bin_export".into()]).unwrap();
    for (path, value) in [
        ("/packages/0/targets/1/kind", json!(["lib"])),
        ("/packages/0/targets/1/kind", json!(["bin","lib"])),
        ("/packages/0/targets/1/name", json!("wrong")),
        ("/packages/0/targets/1/src_path", json!("/workspace/package/src/lib.rs")),
        ("/packages/0/targets/1/src_path", json!("src/explicit_bin_export.rs")),
        ("/packages/0/name", json!("wrong")),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(check(&changed).is_err(), "{path}");
    }
    let mut duplicate = original.clone();
    duplicate["packages"][0]["targets"].as_array_mut().unwrap().push(bin);
    assert!(check(&duplicate).is_err());
    assert!(check_explicit_bin_metadata_features(&original, source.export, &[]).is_err());
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn explicit_bin_source_exports_and_simulates_its_own_kernel_not_the_same_named_library() {
    let source = EXPLICIT_BIN_SOURCE_V1;
    bind_tutorial_source_package(source, "explicit_bin_export")
        .expect("bind actual explicit bin manifest, metadata and source root");
    let target = ScratchTarget::new();
    let bundle_path = target.path().join("explicit-bin.fe2sim");
    let result = output(simulation_export_command_for_package_with_exporter(
        Path::new(env!("CARGO_BIN_EXE_fe2o3-export-sim")), source.export, "gfx942",
        &bundle_path, target.path(), Some(1), "explicit_bin_export",
    ), "export actual explicitly selected bin");
    assert!(result.status.success(), "{}", result.stderr);
    let bundle = fe2o3_kernel_ir::VerifiedSimulationBundleV1::from_canonical_bytes(
        std::fs::read(&bundle_path).unwrap(),
    ).unwrap();
    assert_eq!(bundle.target(), "gfx942:xnack-");
    assert_eq!(bundle.kernel_count(), 1);
    let module = fe2o3_kernel_ir::decode_module_v7(bundle.canonical_kir_v7()).unwrap();
    assert_eq!(module.kernels[0].id.as_str(), "explicit_bin_export");
    assert_eq!(bundle.compiler_execution_binding(),
        &fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly);
    assert!(bundle.require_canonical_compiler_execution_association().is_err());
    assert!(!bundle.authenticates_compiler_execution());
    assert!(!bundle.grants_proof_authority());
    assert!(!bundle.grants_artifact_authority());
    assert!(!bundle.grants_compiler_authority());
    assert!(!bundle.grants_hardware_authority());
    assert!(!bundle.grants_load_authority());
    assert!(!bundle.grants_launch_authority());
    let map = fe2o3_kernel_ir::DebugSourceMapDocumentV1::from_json_bytes(bundle.debug_map().unwrap()).unwrap();
    let file = map.files().iter().find(|file| file.display_path()
        .ends_with("production-explicit-bin-device/src/explicit_bin_export.rs")).unwrap();
    assert_eq!(file.byte_len(), std::fs::metadata(workspace().join(source.source_path)).unwrap().len());
    let original = vec![0xa5_u8; 80 * 4];
    for value in [0_u32, 19, u32::MAX] {
        let request = target.path().join(format!("request-{value}.json"));
        std::fs::write(&request, serde_json::to_vec(&json!({
            "schema":"fe2o3-simulation-request-v1", "kernel":"explicit_bin_export",
            "grid":[64,1,1], "workgroup":[64,1,1], "arguments":[
                {"kind":"scalar","type":"u32","bits":format!("0x{value:08x}")},
                {"kind":"buffer","element":"u32","access":"read_write","alignment":4,
                 "bytes":format!("0x{}", "a5".repeat(original.len()))}
            ],
        })).unwrap()).unwrap();
        let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(&bundle_path, &request).unwrap();
        let original_request = admitted.input().request.clone();
        let execute = || admitted.input().module.simulate(
            &admitted.input().request, admitted.input().simulation_target(), admitted.input().simulation_limits,
        ).unwrap();
        let first = execute();
        let second = execute();
        assert_eq!(first.invocations_executed(), 64);
        assert_eq!(admitted.input().request, original_request);
        assert_eq!(first.buffer(1).unwrap().bytes(), second.buffer(1).unwrap().bytes());
        let bytes = first.buffer(1).unwrap().bytes();
        let expected = value.wrapping_add(17).to_le_bytes();
        assert!(bytes[..64 * 4].chunks_exact(4).all(|word| word == expected));
        assert_eq!(&bytes[64 * 4..], &original[64 * 4..]);
    }
}
